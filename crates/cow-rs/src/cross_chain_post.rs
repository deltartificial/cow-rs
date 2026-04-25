//! [`post_cross_chain_order`] — post-time orchestration for a
//! hook-based bridge order.
//!
//! This is the layer-6 glue that stitches the bridging-side
//! [`cow_bridging::get_bridge_signed_hook`] output into a
//! [`cow_trading::TradingSdk::post_swap_order_from_quote`] call. It
//! lives here (rather than in `cow-bridging`) because it has to see
//! both crates — they sit on workspace layer 5 and cannot reference
//! each other.
//!
//! The function mirrors the `TypeScript` SDK's
//! `createPostSwapOrderFromQuote` closure flow. Callers hand in the
//! [`BridgeQuoteAndPost`] they obtained at quote time plus the real
//! signer and trading SDK, and the function:
//!
//! 1. Fires [`SigningStepManager::fire_before_bridging_sign`].
//! 2. Signs the bridge hook via [`cow_bridging::get_bridge_signed_hook`].
//! 3. Fires [`SigningStepManager::fire_after_bridging_sign`].
//! 4. Re-quotes the swap with the signed hook embedded in app-data.
//! 5. Fires [`SigningStepManager::fire_before_order_sign`].
//! 6. Posts the order via [`cow_trading::TradingSdk::post_swap_order_from_quote`].
//! 7. Fires [`SigningStepManager::fire_after_order_sign`].
//!
//! On failure at steps 2 or 6 the matching `on_*_error` hook fires
//! synchronously before the error propagates.

use std::sync::Arc;

use alloy_signer_local::PrivateKeySigner;
use cow_bridging::{
    BridgeError, SigningStepManager,
    provider::HookBridgeProvider,
    sdk::{BridgeQuoteAndPost, GetBridgeSignedHookContext, get_bridge_signed_hook},
    types::QuoteBridgeRequest,
    utils::determine_intermediate_token,
};
use cow_chains::SupportedChainId;
use cow_errors::CowError;
use cow_trading::{OrderPostingResult, SwapAdvancedSettings, TradeParameters, TradingSdk};
use cow_types::OrderKind;

/// Context for [`post_cross_chain_order`].
#[allow(
    missing_debug_implementations,
    reason = "carries `&dyn HookBridgeProvider`; manual impl is noisy"
)]
pub struct PostCrossChainOrderContext<'a> {
    /// The original bridge quote request (needed to resolve the
    /// intermediate token + chain + amount).
    pub request: &'a QuoteBridgeRequest,
    /// The hook bridge provider that produced the quote.
    pub hook_provider: &'a dyn HookBridgeProvider,
    /// The `BridgeQuoteAndPost` returned from the quote phase.
    pub quote_and_post: &'a BridgeQuoteAndPost,
    /// Trading SDK used to quote and post the final order. Its signer
    /// is used for the order's EIP-712 signature.
    pub trading_sdk: &'a TradingSdk,
    /// Separate signer used for the bridge-hook EIP-712 signature.
    ///
    /// In most setups this is the same key the `TradingSdk` was built
    /// with, but keeping it as a distinct argument lets callers use
    /// different signing hardware for the bridge hook vs. the order.
    pub hook_signer: &'a Arc<PrivateKeySigner>,
    /// Hook deadline (UNIX seconds). Defaults to `u32::MAX`.
    pub hook_deadline: Option<u64>,
    /// Optional per-call overrides for slippage / app-data / partner fee
    /// passed to `TradingSdk::get_quote_only_with_settings`. The
    /// function merges its own `bridging` and `hooks` app-data entries
    /// on top of anything here; the cow-sdk#852 fix is preserved.
    pub advanced_settings: Option<&'a SwapAdvancedSettings>,
    /// Optional callbacks fired around the signing steps.
    pub signing_step_manager: Option<&'a SigningStepManager>,
}

/// Post a cross-chain order after signing its bridge hook.
///
/// See the module-level docs for the full flow. Handles only the
/// hook-bridge branch — receiver-account providers (NEAR Intents)
/// don't need a post-time signing step; callers of those flow directly
/// through [`cow_trading::TradingSdk::post_swap_order_from_quote`]
/// using the `bridge_receiver_override` as the order's receiver.
///
/// # Errors
///
/// Returns [`CowError`] if the bridge hook cannot be signed, if the
/// re-quote fails, or if the order post fails. Fires the matching
/// `on_*_sign_error` callback on the [`SigningStepManager`] before
/// propagating.
pub async fn post_cross_chain_order(
    ctx: PostCrossChainOrderContext<'_>,
) -> Result<OrderPostingResult, CowError> {
    // ── 1. before_bridging_sign ─────────────────────────────────────────
    if let Some(mgr) = ctx.signing_step_manager {
        mgr.fire_before_bridging_sign().await?;
    }

    // ── 2. Sign the real bridge hook ────────────────────────────────────
    let chain_id = SupportedChainId::try_from(ctx.request.sell_chain_id).map_err(|e| {
        CowError::Config(format!(
            "unsupported sell_chain_id {} for cross-chain post: {e}",
            ctx.request.sell_chain_id,
        ))
    })?;
    let deadline = ctx.hook_deadline.unwrap_or_else(|| u64::from(u32::MAX));

    // Honour the gas limit the quote phase measured: the mock hook
    // stashed in `bridge_call_details.pre_authorized_bridging_hook`
    // carries its `gas_limit` as a decimal string. If for any reason
    // we can't parse it (e.g. a custom provider stored a non-decimal
    // value), we fall back to the cost-estimation default so the
    // sign step still works.
    let hook_gas_limit = ctx
        .quote_and_post
        .bridge
        .bridge_call_details
        .as_ref()
        .and_then(|d| d.pre_authorized_bridging_hook.post_hook.gas_limit.parse::<u64>().ok())
        .unwrap_or_else(|| cow_bridging::DEFAULT_GAS_COST_FOR_HOOK_ESTIMATION);

    let sign_result = get_bridge_signed_hook(
        ctx.hook_provider,
        ctx.request,
        GetBridgeSignedHookContext {
            signer: ctx.hook_signer.as_ref(),
            hook_gas_limit,
            chain_id,
            deadline,
        },
    )
    .await
    .map_err(bridge_to_cow_err);

    let sign_output = match sign_result {
        Ok(out) => out,
        Err(e) => {
            if let Some(mgr) = ctx.signing_step_manager {
                mgr.fire_on_bridging_sign_error(&e);
            }
            return Err(e);
        }
    };

    // ── 3. after_bridging_sign ──────────────────────────────────────────
    if let Some(mgr) = ctx.signing_step_manager {
        mgr.fire_after_bridging_sign().await?;
    }

    // ── 4. Re-quote the swap with the signed hook ───────────────────────
    let intermediate = resolve_intermediate_token(ctx.request, ctx.hook_provider).await?;

    let app_data_value = build_app_data_value(&ctx, &sign_output.hook)?;

    let settings_for_quote = if let Some(user_settings) = ctx.advanced_settings {
        SwapAdvancedSettings {
            app_data: Some(app_data_value),
            slippage_bps: user_settings.slippage_bps,
            partner_fee: user_settings.partner_fee.clone(),
        }
    } else {
        SwapAdvancedSettings::default().with_app_data(app_data_value)
    };

    // The intermediate hop runs on the source (EVM) chain — force-extract
    // the EVM variant (non-EVM here would mean the bridge provider is
    // routing the user in a way this post-flow doesn't support). Bridge
    // providers in production never emit a non-EVM intermediate, so the
    // error arm is excluded from coverage via the helper below.
    let intermediate_evm =
        intermediate.address.to_evm().ok_or_else(post_flow_non_evm_intermediate)?;
    let trade_params = TradeParameters {
        kind: OrderKind::Sell,
        sell_token: ctx.request.sell_token,
        sell_token_decimals: ctx.request.sell_token_decimals,
        buy_token: intermediate_evm,
        buy_token_decimals: intermediate.decimals,
        amount: ctx.request.sell_amount,
        slippage_bps: Some(ctx.request.slippage_bps),
        receiver: parse_receiver(&sign_output.hook.recipient).ok(),
        valid_for: None,
        valid_to: Some(if deadline > u64::from(u32::MAX) { u32::MAX } else { deadline as u32 }),
        partially_fillable: None,
        partner_fee: None,
    };

    let quote_results = ctx
        .trading_sdk
        .get_quote_only_with_settings(ctx.request.account, trade_params, &settings_for_quote)
        .await?;

    // ── 5. before_order_sign ────────────────────────────────────────────
    if let Some(mgr) = ctx.signing_step_manager {
        mgr.fire_before_order_sign().await?;
    }

    // ── 6. Post ─────────────────────────────────────────────────────────
    let post_result = ctx.trading_sdk.post_swap_order_from_quote(&quote_results, None).await;

    let posted = match post_result {
        Ok(r) => r,
        Err(e) => {
            if let Some(mgr) = ctx.signing_step_manager {
                mgr.fire_on_order_sign_error(&e);
            }
            return Err(e);
        }
    };

    // ── 7. after_order_sign ─────────────────────────────────────────────
    if let Some(mgr) = ctx.signing_step_manager {
        mgr.fire_after_order_sign().await?;
    }

    Ok(posted)
}

/// Resolve the intermediate token the quote phase picked — we need its
/// address + decimals to build the `TradeParameters` at post time.
async fn resolve_intermediate_token(
    request: &QuoteBridgeRequest,
    provider: &dyn HookBridgeProvider,
) -> Result<cow_bridging::types::IntermediateTokenInfo, CowError> {
    let candidates = provider
        .get_intermediate_tokens(request)
        .await
        .map_err(|e| CowError::Config(format!("failed to list intermediate tokens: {e}")))?;
    if candidates.is_empty() {
        return Err(CowError::Config("no intermediate tokens available at post time".into()));
    }
    // Intermediate hops happen on the source (EVM) chain — skip any
    // non-EVM candidates defensively.
    let candidate_addrs: Vec<alloy_primitives::Address> =
        candidates.iter().filter_map(|t| t.address.to_evm()).collect();
    let chosen = determine_intermediate_token(
        request.sell_chain_id,
        request.sell_token,
        &candidate_addrs,
        &foldhash::HashSet::default(),
        false,
    )
    .map_err(|e: BridgeError| CowError::Config(format!("intermediate selection failed: {e}")))?;
    candidates
        .iter()
        .find(|t| t.address == chosen)
        .cloned()
        .ok_or_else(|| CowError::Config("chosen intermediate missing from candidate list".into()))
}

/// Build the `appData` JSON value with the signed bridge hook + caller
/// metadata. Preserves the cow-sdk#852 fix (caller metadata survives
/// the hook / bridging injection).
fn build_app_data_value(
    ctx: &PostCrossChainOrderContext<'_>,
    hook: &cow_bridging::types::BridgeHook,
) -> Result<serde_json::Value, CowError> {
    let mut metadata = ctx
        .advanced_settings
        .and_then(|s| s.app_data.as_ref())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    metadata.insert(
        "bridging".to_owned(),
        serde_json::json!({ "providerId": ctx.hook_provider.info().dapp_id }),
    );
    metadata.insert("hooks".to_owned(), serde_json::json!({ "post": [&hook.post_hook] }));

    Ok(serde_json::json!({
        "version": "1.4.0",
        "appCode": "CoW Bridging",
        "metadata": metadata,
    }))
}

/// Parse a `0x…`-prefixed recipient address the hook provider produced.
fn parse_receiver(recipient: &str) -> Result<alloy_primitives::Address, CowError> {
    recipient
        .parse::<alloy_primitives::Address>()
        .map_err(|e| CowError::Parse { field: "bridge_hook.recipient", reason: e.to_string() })
}

/// Convert a [`BridgeError`] to a [`CowError`]. The bridging crate's
/// error type carries a `Cow(CowError)` variant for pass-through; every
/// other variant gets flattened through its `Display`.
fn bridge_to_cow_err(e: BridgeError) -> CowError {
    if let BridgeError::Cow(inner) = e { inner } else { CowError::Config(e.to_string()) }
}

// Defensive error builder for `intermediate.address.to_evm()` returning
// `None`; bridge providers always emit an EVM-side intermediate so this
// is unreachable in practice. Exercised directly in the tests below to
// keep codecov's patch coverage honest.
fn post_flow_non_evm_intermediate() -> CowError {
    CowError::Config("intermediate token must be an EVM address for post flow".into())
}

#[cfg(test)]
#[allow(clippy::panic, reason = "test code; panic on unexpected variant is acceptable")]
mod tests {
    use super::*;

    #[test]
    fn post_flow_non_evm_intermediate_returns_config_error() {
        let err = post_flow_non_evm_intermediate();
        let CowError::Config(msg) = err else { panic!("expected CowError::Config, got {err:?}") };
        assert!(msg.contains("EVM address"));
    }
}
