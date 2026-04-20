//! [`TradingSwapQuoter`] — newtype wrapper that adapts
//! [`cow_trading::TradingSdk`] to the [`cow_bridging::SwapQuoter`] trait.
//!
//! The `SwapQuoter` trait lives in `cow-bridging` and `TradingSdk` lives
//! in `cow-trading`. Both sit on workspace layer 5 and neither can see
//! the other, so the concrete adapter has to live in a crate that
//! depends on both — this façade crate, `cow-rs` (layer 6). A blanket
//! `impl SwapQuoter for TradingSdk` would violate the orphan rule, so
//! we wrap the SDK in a thin newtype instead. Users construct it with
//! [`TradingSwapQuoter::new`] and pass a `&TradingSwapQuoter` wherever
//! a `&dyn SwapQuoter` is expected.

use std::sync::Arc;

use cow_bridging::swap_quoter::{QuoteSwapFuture, SwapQuoteOutcome, SwapQuoteParams, SwapQuoter};
use cow_errors::CowError;
use cow_trading::{SwapAdvancedSettings, TradeParameters, TradingSdk};

/// Adapter that makes a [`TradingSdk`] usable as a
/// [`cow_bridging::SwapQuoter`].
///
/// Cheap to clone (stores an `Arc<TradingSdk>` internally). Intended to
/// be built once alongside the `TradingSdk` and handed to the bridging
/// orchestrator.
///
/// # Example
///
/// ```rust,no_run
/// use std::sync::Arc;
///
/// use cow_rs::{
///     SupportedChainId, TradingSdk, TradingSdkConfig, trading_swap_quoter::TradingSwapQuoter,
/// };
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let sdk = TradingSdk::new(
///     TradingSdkConfig::prod(SupportedChainId::Mainnet, "MyApp"),
///     "0x0000000000000000000000000000000000000000000000000000000000000001",
/// )?;
/// let quoter = TradingSwapQuoter::new(Arc::new(sdk));
/// // `&quoter` now satisfies `&dyn cow_bridging::SwapQuoter`.
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct TradingSwapQuoter {
    inner: Arc<TradingSdk>,
}

impl TradingSwapQuoter {
    /// Wrap a shared [`TradingSdk`] so it can be used as a
    /// [`cow_bridging::SwapQuoter`].
    ///
    /// # Arguments
    ///
    /// * `sdk` — an `Arc`-shared trading SDK.
    ///
    /// # Returns
    ///
    /// A new [`TradingSwapQuoter`] instance.
    #[must_use]
    pub const fn new(sdk: Arc<TradingSdk>) -> Self {
        Self { inner: sdk }
    }

    /// Return a reference to the underlying [`TradingSdk`].
    #[must_use]
    pub fn sdk(&self) -> &TradingSdk {
        &self.inner
    }
}

impl std::fmt::Debug for TradingSwapQuoter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TradingSwapQuoter").finish_non_exhaustive()
    }
}

impl SwapQuoter for TradingSwapQuoter {
    fn quote_swap<'a>(&'a self, params: SwapQuoteParams) -> QuoteSwapFuture<'a> {
        let sdk = Arc::clone(&self.inner);
        Box::pin(async move {
            let trade_params = TradeParameters {
                kind: params.kind,
                sell_token: params.sell_token,
                sell_token_decimals: params.sell_token_decimals,
                buy_token: params.buy_token,
                buy_token_decimals: params.buy_token_decimals,
                amount: params.amount,
                slippage_bps: Some(params.slippage_bps),
                receiver: None,
                valid_for: None,
                valid_to: None,
                partially_fillable: None,
                partner_fee: None,
            };

            let results = if let Some(json) = params.app_data_json.as_deref() {
                let app_data: serde_json::Value = serde_json::from_str(json).map_err(|err| {
                    CowError::AppData(format!("invalid SwapQuoteParams.app_data_json: {err}"))
                })?;
                let settings = SwapAdvancedSettings::default().with_app_data(app_data);
                sdk.get_quote_only_with_settings(params.owner, trade_params, &settings).await?
            } else {
                sdk.get_quote_only(params.owner, trade_params).await?
            };

            Ok(SwapQuoteOutcome {
                sell_amount: results.amounts_and_costs.after_slippage.sell_amount,
                buy_amount_after_slippage: results.amounts_and_costs.after_slippage.buy_amount,
                fee_amount: results.amounts_and_costs.network_fee.amount_in_sell_currency,
                valid_to: results.order_to_sign.valid_to,
                app_data_hex: results.app_data_info.app_data_keccak256,
                full_app_data: results.app_data_info.full_app_data,
            })
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(clippy::tests_outside_test_module, reason = "inner module + cfg guard for WASM test skip")]
mod tests {
    use super::*;
    use cow_chains::SupportedChainId;
    use cow_trading::TradingSdkConfig;

    const TEST_PRIVATE_KEY: &str =
        "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    #[allow(clippy::panic, reason = "test helper — fallible ctor should never fail here")]
    fn build_sdk() -> Arc<TradingSdk> {
        let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "CoWRsTest");
        let Ok(sdk) = TradingSdk::new(config, TEST_PRIVATE_KEY) else {
            panic!("failed to build test TradingSdk from valid key")
        };
        Arc::new(sdk)
    }

    #[test]
    fn wraps_and_exposes_sdk() {
        let sdk = build_sdk();
        let quoter = TradingSwapQuoter::new(Arc::clone(&sdk));
        assert!(std::ptr::eq(quoter.sdk(), sdk.as_ref()));
    }

    #[test]
    fn clone_shares_inner_arc() {
        let sdk = build_sdk();
        let quoter = TradingSwapQuoter::new(Arc::clone(&sdk));
        let cloned = quoter.clone();
        assert!(std::ptr::eq(cloned.sdk(), quoter.sdk()));
    }

    #[test]
    fn debug_impl_does_not_leak_signer() {
        let sdk = build_sdk();
        let quoter = TradingSwapQuoter::new(sdk);
        let rendered = format!("{quoter:?}");
        assert!(rendered.contains("TradingSwapQuoter"));
        // The debug impl uses `finish_non_exhaustive` — no signer material.
        assert!(!rendered.contains("priv"));
    }

    #[test]
    fn is_swap_quoter_trait_object() {
        let sdk = build_sdk();
        let quoter = TradingSwapQuoter::new(sdk);
        // Compile-time check: coercing into a trait object succeeds.
        let _ref: &dyn SwapQuoter = &quoter;
    }
}
