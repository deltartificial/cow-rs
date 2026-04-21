//! [`BridgingSdk`] — multi-provider cross-chain bridge aggregator.

use cow_errors::CowError;

use crate::swap_quoter::SwapQuoter;

// ── Bridging constants ──────────────────────────────────────────────────────

/// Bungee bridge backend API path segment.
pub const BUNGEE_API_PATH: &str = "/api/v1/bungee";

/// Bungee bridge manual API path segment.
pub const BUNGEE_MANUAL_API_PATH: &str = "/api/v1/bungee-manual";

/// Bungee (Socket) public backend base URL.
pub const BUNGEE_BASE_URL: &str = "https://public-backend.bungee.exchange";

/// Bungee API URL (base URL + API path).
pub const BUNGEE_API_URL: &str = "https://public-backend.bungee.exchange/api/v1/bungee";

/// Bungee manual API URL (base URL + manual API path).
pub const BUNGEE_MANUAL_API_URL: &str =
    "https://public-backend.bungee.exchange/api/v1/bungee-manual";

/// Bungee events API URL for tracking bridge transactions.
pub const BUNGEE_EVENTS_API_URL: &str = "https://microservices.socket.tech/loki";

/// Across Protocol bridge API base URL.
pub const ACROSS_API_URL: &str = "https://app.across.to/api";

/// Default bridge slippage tolerance in basis points (0.5 %).
pub const DEFAULT_BRIDGE_SLIPPAGE_BPS: u32 = 50;

/// Default gas cost for hook estimation (240 000 gas).
pub const DEFAULT_GAS_COST_FOR_HOOK_ESTIMATION: u64 = 240_000;

/// Default extra gas for hook estimation (350 000 gas).
pub const DEFAULT_EXTRA_GAS_FOR_HOOK_ESTIMATION: u64 = 350_000;

/// Default extra gas cost when creating a proxy (400 000 gas).
pub const DEFAULT_EXTRA_GAS_PROXY_CREATION: u64 = 400_000;

/// URL prefix used to identify bridge hook dapps.
pub const HOOK_DAPP_BRIDGE_PROVIDER_PREFIX: &str = "cow-sdk://bridging/providers";

/// Bungee bridge hook dapp identifier.
pub const BUNGEE_HOOK_DAPP_ID: &str = "cow-sdk://bridging/providers/bungee";

/// Across bridge hook dapp identifier.
pub const ACROSS_HOOK_DAPP_ID: &str = "cow-sdk://bridging/providers/across";

/// Near Intents bridge hook dapp identifier.
pub const NEAR_INTENTS_HOOK_DAPP_ID: &str = "cow-sdk://bridging/providers/near-intents";

/// Bungee API fallback timeout in milliseconds (5 minutes).
pub const BUNGEE_API_FALLBACK_TIMEOUT: u64 = 300_000;

use super::{
    bungee::BungeeProvider,
    provider::BridgeProvider,
    types::{BridgeError, QuoteBridgeRequest, QuoteBridgeResponse},
};

/// High-level cross-chain bridge aggregator.
///
/// Holds a list of [`BridgeProvider`] implementations and queries them
/// concurrently when fetching quotes.
///
/// # Example
///
/// ```rust,no_run
/// use cow_bridging::BridgingSdk;
///
/// let sdk = BridgingSdk::new().with_bungee("my-api-key");
/// assert_eq!(sdk.provider_count(), 1);
/// ```
#[derive(Default)]
pub struct BridgingSdk {
    providers: Vec<Box<dyn BridgeProvider>>,
}

impl std::fmt::Debug for BridgingSdk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgingSdk").field("provider_count", &self.providers.len()).finish()
    }
}

impl BridgingSdk {
    /// Create an empty [`BridgingSdk`] with no providers.
    ///
    /// # Returns
    ///
    /// A new [`BridgingSdk`] instance with an empty provider list.
    ///
    /// # Example
    ///
    /// ```rust
    /// use cow_bridging::BridgingSdk;
    ///
    /// let sdk = BridgingSdk::new();
    /// assert_eq!(sdk.provider_count(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self { providers: vec![] }
    }

    /// Add the Bungee (Socket) bridge provider using the given API key.
    ///
    /// This is a builder-style method that consumes `self` and returns the
    /// modified instance, allowing chained calls.
    ///
    /// # Arguments
    ///
    /// * `api_key` — Bungee (Socket) API key used to authenticate requests.
    ///
    /// # Returns
    ///
    /// The [`BridgingSdk`] instance with the Bungee provider appended.
    #[must_use]
    pub fn with_bungee(mut self, api_key: impl Into<String>) -> Self {
        self.providers.push(Box::new(BungeeProvider::new(api_key)));
        self
    }

    /// Register any custom [`BridgeProvider`] implementation.
    ///
    /// # Arguments
    ///
    /// * `provider` — A type implementing [`BridgeProvider`] that will be boxed and stored
    ///   alongside any existing providers.
    pub fn add_provider(&mut self, provider: impl BridgeProvider + 'static) {
        self.providers.push(Box::new(provider));
    }

    /// Number of registered providers.
    ///
    /// # Returns
    ///
    /// The count of [`BridgeProvider`] instances currently registered with
    /// this SDK.
    #[must_use]
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Query all registered providers concurrently and return the best quote.
    ///
    /// "Best" is defined as the highest [`net_buy_amount`](QuoteBridgeResponse::net_buy_amount).
    ///
    /// # Errors
    ///
    /// - [`BridgeError::NoProviders`] if no providers support the requested route.
    /// - [`BridgeError::NoQuote`] if all providers fail or return no quote.
    pub async fn get_best_quote(
        &self,
        req: &QuoteBridgeRequest,
    ) -> Result<QuoteBridgeResponse, BridgeError> {
        let eligible: Vec<&dyn BridgeProvider> = self
            .providers
            .iter()
            .filter(|p| p.supports_route(req.sell_chain_id, req.buy_chain_id))
            .map(|p| p.as_ref())
            .collect();

        if eligible.is_empty() {
            return Err(BridgeError::NoProviders);
        }

        let futures: Vec<_> = eligible.iter().map(|p| p.get_quote(req)).collect();
        let results = futures::future::join_all(futures).await;

        let best = results
            .into_iter()
            .filter_map(|r| r.ok())
            .max_by_key(QuoteBridgeResponse::net_buy_amount);

        best.ok_or(BridgeError::NoQuote)
    }

    /// Query all registered providers concurrently and return all results.
    ///
    /// Providers that do not support the route are skipped.
    /// Both successful quotes and errors are included in the output.
    ///
    /// # Errors
    ///
    /// Individual provider failures are returned as [`CowError`] entries
    /// in the result vector rather than short-circuiting the entire call.
    pub async fn get_all_quotes(
        &self,
        req: &QuoteBridgeRequest,
    ) -> Vec<Result<QuoteBridgeResponse, CowError>> {
        let eligible: Vec<&dyn BridgeProvider> = self
            .providers
            .iter()
            .filter(|p| p.supports_route(req.sell_chain_id, req.buy_chain_id))
            .map(|p| p.as_ref())
            .collect();

        let futures: Vec<_> = eligible.iter().map(|p| p.get_quote(req)).collect();
        futures::future::join_all(futures).await
    }
}

// ── Type guard result types ──────────────────────────────────────────────────

use super::types::BridgeQuoteResults;

/// A bridge quote paired with a callback-style post function.
///
/// In the `TypeScript` SDK this includes a closure `postSwapOrderFromQuote`.
/// In Rust, the struct holds the data needed to construct the order; the
/// caller orchestrates posting via the order-book API.
#[derive(Debug, Clone)]
pub struct BridgeQuoteAndPost {
    /// Swap quote results (amounts, costs, app-data).
    pub swap: QuoteBridgeResponse,
    /// Bridge quote results.
    pub bridge: BridgeQuoteResults,
}

/// A simple quote-and-post result for same-chain swaps.
///
/// In the `TypeScript` SDK this is `QuoteAndPost` from the trading package.
/// Here it wraps the quote response; order posting is handled separately.
#[derive(Debug, Clone)]
pub struct QuoteAndPost {
    /// The quote response.
    pub quote: QuoteBridgeResponse,
}

/// Union of same-chain and cross-chain quote results.
///
/// Mirrors the `TypeScript` `CrossChainQuoteAndPost = QuoteAndPost | BridgeQuoteAndPost`.
#[derive(Debug, Clone)]
pub enum CrossChainQuoteAndPost {
    /// Same-chain swap (no bridging needed).
    SameChain(Box<QuoteAndPost>),
    /// Cross-chain swap with bridging.
    CrossChain(Box<BridgeQuoteAndPost>),
}

// ── Type guard functions ─────────────────────────────────────────────────────

/// Returns `true` if the result is a [`BridgeQuoteAndPost`] (cross-chain with
/// both swap and bridge data).
///
/// Mirrors the `TypeScript` `isBridgeQuoteAndPost` type guard.
#[must_use]
pub const fn is_bridge_quote_and_post(result: &CrossChainQuoteAndPost) -> bool {
    matches!(result, CrossChainQuoteAndPost::CrossChain(_))
}

/// Returns `true` if the result is a [`QuoteAndPost`] (same-chain swap).
///
/// Mirrors the `TypeScript` `isQuoteAndPost` type guard.
#[must_use]
pub const fn is_quote_and_post(result: &CrossChainQuoteAndPost) -> bool {
    matches!(result, CrossChainQuoteAndPost::SameChain(_))
}

/// Assert that the result is a [`BridgeQuoteAndPost`], returning a reference
/// to it or an error.
///
/// # Errors
///
/// Returns [`BridgeError::QuoteError`] if the result is not a cross-chain quote.
pub fn assert_is_bridge_quote_and_post(
    result: &CrossChainQuoteAndPost,
) -> Result<&BridgeQuoteAndPost, BridgeError> {
    match result {
        CrossChainQuoteAndPost::CrossChain(bqp) => Ok(bqp.as_ref()),
        CrossChainQuoteAndPost::SameChain(_) => {
            Err(BridgeError::QuoteError("expected BridgeQuoteAndPost, got QuoteAndPost".to_owned()))
        }
    }
}

/// Assert that the result is a [`QuoteAndPost`], returning a reference to it
/// or an error.
///
/// # Errors
///
/// Returns [`BridgeError::QuoteError`] if the result is not a same-chain quote.
pub fn assert_is_quote_and_post(
    result: &CrossChainQuoteAndPost,
) -> Result<&QuoteAndPost, BridgeError> {
    match result {
        CrossChainQuoteAndPost::SameChain(qp) => Ok(qp.as_ref()),
        CrossChainQuoteAndPost::CrossChain(_) => {
            Err(BridgeError::QuoteError("expected QuoteAndPost, got BridgeQuoteAndPost".to_owned()))
        }
    }
}

// ── Cross-chain order flow ───────────────────────────────────────────────────

use crate::{
    across::{EvmLogEntry, get_deposit_params},
    types::{BridgeHook, BridgeQuoteResult, BridgeStatus, BridgeStatusResult, CrossChainOrder},
};
use alloy_primitives::Address;

/// Parameters for [`get_cross_chain_order`].
#[derive(Debug)]
pub struct GetCrossChainOrderParams<'a> {
    /// Chain ID where the order was settled.
    pub chain_id: u64,
    /// The `CoW` Protocol order UID.
    pub order_id: String,
    /// Full app-data JSON of the order.
    pub full_app_data: Option<String>,
    /// Transaction hash of the settlement.
    pub trade_tx_hash: String,
    /// Raw log entries from the settlement transaction.
    pub logs: &'a [EvmLogEntry],
    /// Optional settlement contract address override.
    pub settlement_override: Option<Address>,
}

/// Build a [`CrossChainOrder`] from settlement transaction data.
///
/// Parses Across deposit events and `CoW` Trade events from the logs, matches
/// them by index, and constructs the bridging deposit parameters.
///
/// This is a simplified version of the `TypeScript` `getCrossChainOrder` that
/// does not call the `OrderBookApi` (the caller must provide the order data
/// and logs). For full orchestration, use the `OrderBookApi` directly.
///
/// # Errors
///
/// Returns [`BridgeError::QuoteError`] if the deposit parameters cannot be
/// extracted from the logs.
pub fn get_cross_chain_order(
    params: &GetCrossChainOrderParams<'_>,
) -> Result<CrossChainOrder, BridgeError> {
    let bridging_params = get_deposit_params(
        params.chain_id,
        &params.order_id,
        params.logs,
        params.settlement_override,
    )
    .ok_or_else(|| {
        BridgeError::QuoteError(format!(
            "bridging params cannot be derived from transaction: {}",
            params.trade_tx_hash
        ))
    })?;

    Ok(CrossChainOrder {
        chain_id: params.chain_id,
        status_result: BridgeStatusResult::new(BridgeStatus::Unknown),
        bridging_params,
        trade_tx_hash: params.trade_tx_hash.clone(),
        explorer_url: None,
    })
}

/// Context passed to [`get_bridge_signed_hook`].
///
/// Mirrors the `HookBridgeResultContext` struct of the `TypeScript`
/// SDK: the fields carry the pieces of state a [`crate::provider::HookBridgeProvider`]
/// needs to both request the hook and derive its nonce.
#[derive(Debug)]
pub struct GetBridgeSignedHookContext<'a> {
    /// Signer that will EIP-712-sign the hook bundle through `cow-shed`.
    pub signer: &'a alloy_signer_local::PrivateKeySigner,
    /// Gas-limit estimated for the bridge post-hook. Passed verbatim to
    /// [`crate::provider::HookBridgeProvider::get_signed_hook`].
    pub hook_gas_limit: u64,
    /// Source chain of the bridge — picks the right cow-shed factory /
    /// domain separator.
    pub chain_id: cow_chains::SupportedChainId,
    /// Hook validity deadline (UNIX seconds). Usually equals
    /// `order_to_sign.valid_to` from the enclosing swap quote.
    pub deadline: u64,
}

/// Output of [`get_bridge_signed_hook`].
///
/// Bundles the signed hook together with the raw bridge call and the
/// provider's original [`QuoteBridgeResponse`] so the caller can wire
/// the three into a final order.
#[derive(Debug, Clone)]
pub struct GetBridgeSignedHookOutput {
    /// Signed bridge hook ready to be attached as a post-interaction
    /// on the enclosing `CoW` order's app-data.
    pub hook: BridgeHook,
    /// Raw EVM call that the cow-shed proxy will execute.
    pub unsigned_bridge_call: cow_chains::EvmCall,
    /// The bridging quote produced upstream of the signing step.
    pub bridging_quote: QuoteBridgeResponse,
}

/// Produce a signed bridge hook for a [`crate::provider::HookBridgeProvider`].
///
/// Mirrors `getBridgeSignedHook` from
/// `packages/bridging/src/BridgingSdk/getBridgeSignedHook.ts`. The
/// function:
///
/// 1. Asks the provider for a bridge quote ([`crate::provider::BridgeProvider::get_quote`]).
/// 2. Asks the provider for the unsigned EVM call that will initiate the bridge
///    ([`crate::provider::HookBridgeProvider::get_unsigned_bridge_call`]).
/// 3. Derives a deterministic hook nonce from the call data and the order's `valid_to` deadline —
///    `keccak256(abi.encodePacked(data, uint256 deadline))`. This ties the signed hook to a
///    specific combination of bridge call and order deadline.
/// 4. Delegates to [`crate::provider::HookBridgeProvider::get_signed_hook`] to produce the EIP-712
///    signed hook via `cow-shed`.
///
/// # Errors
///
/// Returns [`BridgeError::TxBuildError`] if any of the provider calls
/// fail, wrapping the underlying [`CowError`].
pub async fn get_bridge_signed_hook<P: crate::provider::HookBridgeProvider + ?Sized>(
    hook_provider: &P,
    bridge_request: &QuoteBridgeRequest,
    context: GetBridgeSignedHookContext<'_>,
) -> Result<GetBridgeSignedHookOutput, BridgeError> {
    // 1. Bridge quote from the provider.
    let bridging_quote = hook_provider
        .get_quote(bridge_request)
        .await
        .map_err(|e| BridgeError::TxBuildError(e.to_string()))?;

    // 2. Raw EVM call.
    let unsigned_bridge_call = hook_provider
        .get_unsigned_bridge_call(bridge_request, &bridging_quote)
        .await
        .map_err(|e| BridgeError::TxBuildError(e.to_string()))?;

    // 3. Derive the hook nonce.
    let nonce_hex = derive_hook_nonce(&unsigned_bridge_call.data, context.deadline);

    // 4. Sign the hook.
    let hook = hook_provider
        .get_signed_hook(
            context.chain_id,
            &unsigned_bridge_call,
            &nonce_hex,
            context.deadline,
            context.hook_gas_limit,
            context.signer,
        )
        .await
        .map_err(|e| BridgeError::TxBuildError(e.to_string()))?;

    Ok(GetBridgeSignedHookOutput { hook, unsigned_bridge_call, bridging_quote })
}

/// Derive the bridge-hook nonce as the `TypeScript` SDK does:
///
/// ```text
/// nonce = keccak256( abi.encodePacked(bytes calldata, uint256 deadline) )
/// ```
///
/// `abi.encodePacked` on `(bytes, uint256)` concatenates the raw bytes
/// of `data` with the 32-byte big-endian `deadline` — no 32-byte offset
/// prefix like the non-packed encoding would introduce.
///
/// The returned string is the `0x`-prefixed lowercase hex of the hash,
/// matching the TS `solidityKeccak256` output.
fn derive_hook_nonce(data: &[u8], deadline: u64) -> String {
    let deadline_be: [u8; 32] = alloy_primitives::U256::from(deadline).to_be_bytes();
    let mut packed = Vec::with_capacity(data.len() + 32);
    packed.extend_from_slice(data);
    packed.extend_from_slice(&deadline_be);
    let hash = alloy_primitives::keccak256(&packed);
    format!("{hash:#x}")
}

/// Parameters for [`get_quote_with_bridge`].
#[derive(Clone)]
pub struct GetQuoteWithBridgeParams {
    /// The swap-and-bridge request.
    pub swap_and_bridge_request: QuoteBridgeRequest,
    /// Slippage tolerance in basis points for the swap leg.
    pub slippage_bps: u32,
    /// Optional caller-supplied app-data metadata to merge into the
    /// auto-generated `hooks` / `bridging` metadata.
    ///
    /// Corresponds to `advanced_settings.app_data.metadata` in the
    /// `TypeScript` SDK — the load-bearing bit of the cow-sdk#852 fix.
    pub advanced_settings_metadata: Option<serde_json::Value>,
    /// Optional quote-time signer. When provided on the hook branch,
    /// [`get_quote_with_hook_bridge`] produces a **real** EIP-712 signed
    /// hook via [`get_bridge_signed_hook`] instead of the placeholder
    /// mock used for cost estimation. The receiver-account branch
    /// ignores this field.
    ///
    /// Corresponds to the `quoteSigner` parameter of the TS SDK's
    /// `getQuoteWithHookBridge`. Keep it `None` when the final signing
    /// wallet is not available yet (e.g. hardware wallet flows).
    pub quote_signer: Option<std::sync::Arc<alloy_signer_local::PrivateKeySigner>>,
    /// Hook deadline (UNIX seconds). Defaults to `u32::MAX` when `None`.
    ///
    /// Threaded into [`get_bridge_signed_hook`] so the hook nonce binds
    /// to the same validity as the enclosing order.
    pub hook_deadline: Option<u64>,
}

impl std::fmt::Debug for GetQuoteWithBridgeParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GetQuoteWithBridgeParams")
            .field("swap_and_bridge_request", &self.swap_and_bridge_request)
            .field("slippage_bps", &self.slippage_bps)
            .field("advanced_settings_metadata", &self.advanced_settings_metadata)
            .field("quote_signer", &self.quote_signer.is_some())
            .field("hook_deadline", &self.hook_deadline)
            .finish()
    }
}

/// Get a quote that includes bridging (cross-chain).
///
/// Dispatches to the hook-bridge or receiver-account-bridge branch based on
/// the provider's runtime type, mirroring the `TypeScript`
/// `getQuoteWithBridge` in
/// `packages/bridging/src/BridgingSdk/getQuoteWithBridge.ts`.
///
/// # Flow
///
/// 1. Reject non-sell orders (cross-chain only supports `OrderKind::Sell`).
/// 2. If the provider implements [`crate::provider::HookBridgeProvider`], delegate to
///    [`get_quote_with_hook_bridge`].
/// 3. Otherwise, if the provider implements [`crate::provider::ReceiverAccountBridgeProvider`],
///    delegate to [`get_quote_with_receiver_account_bridge`].
/// 4. Fall through to an error if the provider implements neither.
///
/// # Errors
///
/// * [`BridgeError::OnlySellOrderSupported`] when `kind != Sell`.
/// * [`BridgeError::TxBuildError`] when the provider implements neither sub-trait.
/// * Any error returned by the delegated branch.
pub async fn get_quote_with_bridge(
    params: &GetQuoteWithBridgeParams,
    provider: &dyn BridgeProvider,
    quoter: &dyn SwapQuoter,
) -> Result<BridgeQuoteAndPost, BridgeError> {
    if params.swap_and_bridge_request.kind != cow_types::OrderKind::Sell {
        return Err(BridgeError::OnlySellOrderSupported);
    }

    if let Some(hook_provider) = provider.as_hook_bridge_provider() {
        return get_quote_with_hook_bridge(hook_provider, params, quoter).await;
    }

    if let Some(receiver_provider) = provider.as_receiver_account_bridge_provider() {
        return get_quote_with_receiver_account_bridge(receiver_provider, params, quoter).await;
    }

    Err(BridgeError::TxBuildError(format!(
        "provider {name} implements neither HookBridgeProvider nor ReceiverAccountBridgeProvider",
        name = provider.info().name,
    )))
}

/// Get a quote without bridging (same-chain swap).
///
/// Delegates to the [`SwapQuoter`] — equivalent to calling `TradingSdk::get_quote_only`
/// with the bridge request fields mapped to `TradeParameters`.
///
/// Mirrors `getQuoteWithoutBridge` in the `TypeScript` SDK.
///
/// # Errors
///
/// Returns [`BridgeError::TxBuildError`] when the quoter fails.
pub async fn get_quote_without_bridge(
    request: &QuoteBridgeRequest,
    quoter: &dyn SwapQuoter,
) -> Result<QuoteAndPost, BridgeError> {
    let params = crate::swap_quoter::SwapQuoteParams {
        owner: request.account,
        chain_id: request.sell_chain_id,
        sell_token: request.sell_token,
        sell_token_decimals: request.sell_token_decimals,
        buy_token: request.buy_token,
        buy_token_decimals: request.buy_token_decimals,
        amount: request.sell_amount,
        kind: request.kind,
        slippage_bps: request.slippage_bps,
        app_data_json: None,
    };
    let outcome =
        quoter.quote_swap(params).await.map_err(|e| BridgeError::TxBuildError(e.to_string()))?;

    Ok(QuoteAndPost {
        quote: QuoteBridgeResponse {
            provider: "same-chain".to_owned(),
            sell_amount: outcome.sell_amount,
            buy_amount: outcome.buy_amount_after_slippage,
            fee_amount: outcome.fee_amount,
            estimated_secs: 0,
            bridge_hook: None,
        },
    })
}

/// Get a swap quote for an intermediate hop, as a stand-alone helper.
///
/// Builds swap parameters from the bridge request (using `buy_token` as
/// the intermediate token destination) and asks the [`SwapQuoter`] to
/// price it. Mirrors `getSwapQuote` in the `TypeScript` SDK.
///
/// # Errors
///
/// Returns [`BridgeError::TxBuildError`] when the quoter fails.
pub async fn get_swap_quote(
    request: &QuoteBridgeRequest,
    quoter: &dyn SwapQuoter,
) -> Result<QuoteBridgeResponse, BridgeError> {
    let params = crate::swap_quoter::SwapQuoteParams {
        owner: request.account,
        chain_id: request.sell_chain_id,
        sell_token: request.sell_token,
        sell_token_decimals: request.sell_token_decimals,
        buy_token: request.buy_token,
        buy_token_decimals: request.buy_token_decimals,
        amount: request.sell_amount,
        kind: request.kind,
        slippage_bps: request.slippage_bps,
        app_data_json: None,
    };
    let outcome =
        quoter.quote_swap(params).await.map_err(|e| BridgeError::TxBuildError(e.to_string()))?;

    Ok(QuoteBridgeResponse {
        provider: "swap".to_owned(),
        sell_amount: outcome.sell_amount,
        buy_amount: outcome.buy_amount_after_slippage,
        fee_amount: outcome.fee_amount,
        estimated_secs: 0,
        bridge_hook: None,
    })
}

/// Quote the intermediate swap step of a cross-chain bridge flow.
///
/// Given a bridge request and a [`BridgeProvider`], this:
/// 1. Asks the provider for candidate intermediate tokens
///    ([`BridgeProvider::get_intermediate_tokens`]).
/// 2. Picks the best candidate via [`crate::utils::determine_intermediate_token`].
/// 3. Asks the [`SwapQuoter`] to price the swap from the sell token to the intermediate token.
/// 4. Merges any caller-supplied `app_data.metadata` with the auto-generated `hooks` / `bridging`
///    metadata — the cow-sdk#852 fix: caller-provided partner / UTM metadata must survive the
///    intermediate quote instead of being overwritten.
/// 5. Returns a [`QuoteBridgeResponse`] whose `buy_amount` is the swap's `afterSlippage.buyAmount`
///    — the amount handed off to the bridge.
///
/// # Arguments
///
/// * `request` — the top-level bridge quote request.
/// * `provider` — the [`BridgeProvider`] that will route the bridge step.
/// * `quoter` — a [`SwapQuoter`] that can price the intermediate swap (typically a wrapper around
///   `cow_trading::TradingSdk::get_quote_only`).
/// * `advanced_settings_metadata` — optional caller-supplied app-data metadata JSON. When `Some`,
///   its keys are merged with the auto-generated `hooks` / `bridging` entries (see cow-sdk#852).
///
/// # Errors
///
/// * [`BridgeError::NoIntermediateTokens`] if the provider returns an empty candidate list.
/// * [`BridgeError::TxBuildError`] if the swap quote fails, wrapping the underlying [`CowError`].
pub async fn get_intermediate_swap_result(
    request: &QuoteBridgeRequest,
    provider: &dyn crate::provider::BridgeProvider,
    quoter: &dyn SwapQuoter,
    advanced_settings_metadata: Option<&serde_json::Value>,
) -> Result<QuoteBridgeResponse, BridgeError> {
    use crate::utils::determine_intermediate_token;

    // 1. Ask the provider for candidates.
    let candidates = provider
        .get_intermediate_tokens(request)
        .await
        .map_err(|e| BridgeError::TxBuildError(e.to_string()))?;

    if candidates.is_empty() {
        return Err(BridgeError::NoIntermediateTokens);
    }

    // 2. Pick the best candidate.
    let candidate_addrs: Vec<alloy_primitives::Address> =
        candidates.iter().map(|t| t.address).collect();
    let intermediate = determine_intermediate_token(
        request.sell_chain_id,
        request.sell_token,
        &candidate_addrs,
        &foldhash::HashSet::default(),
        false,
    )?;
    let intermediate_info =
        candidates.iter().find(|t| t.address == intermediate).cloned().ok_or_else(|| {
            BridgeError::TxBuildError("intermediate token not in candidates".into())
        })?;

    // 3. Build the app-data JSON with caller metadata preserved (#852 fix).
    let app_data_json = build_intermediate_app_data_json(advanced_settings_metadata, provider);

    // 4. Quote the swap.
    let params = crate::swap_quoter::SwapQuoteParams {
        owner: request.account,
        chain_id: request.sell_chain_id,
        sell_token: request.sell_token,
        sell_token_decimals: request.sell_token_decimals,
        buy_token: intermediate_info.address,
        buy_token_decimals: intermediate_info.decimals,
        amount: request.sell_amount,
        kind: request.kind,
        slippage_bps: request.slippage_bps,
        app_data_json: Some(app_data_json),
    };
    let outcome =
        quoter.quote_swap(params).await.map_err(|e| BridgeError::TxBuildError(e.to_string()))?;

    // 5. Wrap the outcome in a QuoteBridgeResponse.
    Ok(QuoteBridgeResponse {
        provider: provider.info().name.clone(),
        sell_amount: outcome.sell_amount,
        buy_amount: outcome.buy_amount_after_slippage,
        fee_amount: outcome.fee_amount,
        estimated_secs: 0,
        bridge_hook: None,
    })
}

/// Build the `appData` JSON for the intermediate swap, preserving
/// caller-supplied metadata.
///
/// Implements the cow-sdk#852 fix: when `advanced_settings.app_data.metadata`
/// exists, its keys are spread into the final metadata object *before*
/// the auto-generated `hooks` and `bridging` entries. This matches the
/// `TypeScript` flow:
///
/// ```text
/// appData.metadata = {
///     ...advanced_settings?.app_data?.metadata,
///     hooks,
///     bridging: { providerId: provider.info.dappId },
/// }
/// ```
///
/// The return value is a stringified JSON document ready to be passed
/// through a [`SwapQuoter`].
fn build_intermediate_app_data_json(
    caller_metadata: Option<&serde_json::Value>,
    provider: &dyn crate::provider::BridgeProvider,
) -> String {
    let mut metadata = caller_metadata.and_then(|v| v.as_object().cloned()).unwrap_or_default();

    // Overwrite with auto-generated fields — they are the load-bearing
    // bits for the on-chain bridge flow.
    metadata.insert(
        "bridging".to_owned(),
        serde_json::json!({ "providerId": provider.info().dapp_id }),
    );
    // Hooks are populated by the orchestration layer in PR #7 once the
    // real post-hook is known; for now carry an empty hooks entry so
    // the shape mirrors the TS output.
    if !metadata.contains_key("hooks") {
        metadata.insert("hooks".to_owned(), serde_json::json!({ "post": [] }));
    }

    let doc = serde_json::json!({
        "version": "1.4.0",
        "appCode": "CoW Bridging",
        "metadata": metadata,
    });
    serde_json::to_string(&doc).unwrap_or_else(|_| "{}".to_owned())
}

// ── Timeout ─────────────────────────────────────────────────────────────────

/// Create a bridge request timeout future.
///
/// Returns a future that resolves to a [`BridgeError::Timeout`] after
/// `timeout_ms` milliseconds. This is the Rust equivalent of the `TypeScript`
/// `createBridgeRequestTimeoutPromise(timeoutMs, prefix)`.
///
/// # Example
///
/// ```rust,no_run
/// use cow_bridging::sdk::create_bridge_request_timeout;
///
/// # async fn example() {
/// let timeout = create_bridge_request_timeout(20_000, "Across");
/// // Use with tokio::select! or futures::select! to race against a real request
/// # }
/// ```
#[cfg(feature = "native")]
pub async fn create_bridge_request_timeout(timeout_ms: u64, prefix: &str) -> BridgeError {
    tokio::time::sleep(std::time::Duration::from_millis(timeout_ms)).await;
    BridgeError::ApiError(format!("{prefix} timeout after {timeout_ms}ms"))
}

// ── Strategy factory ────────────────────────────────────────────────────────

/// Strategy variant for quote retrieval.
///
/// Mirrors the `TypeScript` `createStrategies` factory which returns
/// `SingleQuoteStrategy`, `MultiQuoteStrategy`, and `BestQuoteStrategy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteStrategy {
    /// Query a single provider (or fall back to a direct swap for same-chain).
    Single,
    /// Query all providers and return all results.
    Multi,
    /// Query all providers and return the best quote.
    Best,
}

impl QuoteStrategy {
    /// Return the strategy name.
    ///
    /// # Returns
    ///
    /// A static string label for this strategy variant:
    /// `"SingleQuoteStrategy"`, `"MultiQuoteStrategy"`, or `"BestQuoteStrategy"`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use cow_bridging::sdk::QuoteStrategy;
    ///
    /// assert_eq!(QuoteStrategy::Best.name(), "BestQuoteStrategy");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Single => "SingleQuoteStrategy",
            Self::Multi => "MultiQuoteStrategy",
            Self::Best => "BestQuoteStrategy",
        }
    }
}

/// Create all available quote strategies.
///
/// Returns the three strategy variants (Single, Multi, Best). In the
/// `TypeScript` SDK each is a class instance backed by an optional
/// `intermediateTokensCache`; in Rust the strategies are simple enum
/// variants and caching is handled by the caller.
///
/// Mirrors `createStrategies(cache)` from `strategies/createStrategies.ts`.
#[must_use]
pub const fn create_strategies() -> [QuoteStrategy; 3] {
    [QuoteStrategy::Single, QuoteStrategy::Multi, QuoteStrategy::Best]
}

// ── Provider quote execution ────────────────────────────────────────────────

use super::types::MultiQuoteResult;

/// Default total timeout for multi-provider quotes (40 seconds).
pub const DEFAULT_TOTAL_TIMEOUT_MS: u64 = 40_000;

/// Default per-provider timeout (20 seconds).
pub const DEFAULT_PROVIDER_TIMEOUT_MS: u64 = 20_000;

/// Execute quotes across providers concurrently with a global timeout.
///
/// Spawns one future per provider, races them against a global timeout, and
/// returns whatever results completed. Providers that did not respond in time
/// get a timeout error in the results vector.
///
/// Mirrors `executeProviderQuotes` from `BridgingSdk/utils.ts`.
///
/// # Errors
///
/// Does not return an error itself — individual provider errors are captured
/// in the returned [`MultiQuoteResult`] entries.
#[cfg(feature = "native")]
pub async fn execute_provider_quotes(
    sdk: &BridgingSdk,
    request: &QuoteBridgeRequest,
    timeout_ms: u64,
) -> Vec<MultiQuoteResult> {
    use futures::future::join_all;

    let futs: Vec<_> = sdk
        .providers
        .iter()
        .map(|p| {
            let name = p.name().to_owned();
            async move {
                let result = p.get_quote(request).await;
                match result {
                    Ok(quote) => MultiQuoteResult {
                        provider_dapp_id: name,
                        quote: Some(crate::types::BridgeQuoteAmountsAndCosts {
                            before_fee: crate::types::BridgeAmounts {
                                sell_amount: quote.sell_amount,
                                buy_amount: quote.buy_amount,
                            },
                            after_fee: crate::types::BridgeAmounts {
                                sell_amount: quote.sell_amount,
                                buy_amount: quote.buy_amount.saturating_sub(quote.fee_amount),
                            },
                            after_slippage: crate::types::BridgeAmounts {
                                sell_amount: quote.sell_amount,
                                buy_amount: quote.buy_amount.saturating_sub(quote.fee_amount),
                            },
                            costs: crate::types::BridgeCosts {
                                bridging_fee: crate::types::BridgingFee {
                                    fee_bps: 0,
                                    amount_in_sell_currency: quote.fee_amount,
                                    amount_in_buy_currency: quote.fee_amount,
                                },
                            },
                            slippage_bps: request.slippage_bps,
                        }),
                        error: None,
                    },
                    Err(e) => MultiQuoteResult {
                        provider_dapp_id: name,
                        quote: None,
                        error: Some(e.to_string()),
                    },
                }
            }
        })
        .collect();

    // Race all futures against a global timeout
    let fetched_results =
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), join_all(futs)).await;

    match fetched_results {
        Ok(results) => results,
        Err(_timeout) => {
            // Return timeout errors for all providers
            sdk.providers
                .iter()
                .map(|p| MultiQuoteResult {
                    provider_dapp_id: p.name().to_owned(),
                    quote: None,
                    error: Some(format!("Multi-quote timeout after {timeout_ms}ms")),
                })
                .collect()
        }
    }
}

/// Fetch a multi-quote from providers with timeout.
///
/// Executes quotes across the SDK's providers concurrently and returns all
/// results (including errors). Results are sorted by buy amount descending.
///
/// Mirrors `fetchMultiQuote` from `strategies/utils.ts` and the
/// `MultiQuoteStrategy.execute` method.
///
/// # Errors
///
/// Individual provider errors are captured in the results vector.
#[cfg(feature = "native")]
pub async fn fetch_multi_quote(
    sdk: &BridgingSdk,
    request: &QuoteBridgeRequest,
    timeout_ms: Option<u64>,
) -> Vec<MultiQuoteResult> {
    let timeout = timeout_ms.map_or(DEFAULT_TOTAL_TIMEOUT_MS, |v| v);
    let mut results = execute_provider_quotes(sdk, request, timeout).await;

    // Fill timeout results
    let dapp_ids: Vec<String> = sdk.providers.iter().map(|p| p.name().to_owned()).collect();
    crate::utils::fill_timeout_results(&mut results, &dapp_ids);

    // Sort by buy amount after slippage (best first)
    results.sort_by(|a, b| {
        let a_amount =
            a.quote.as_ref().map_or(alloy_primitives::U256::ZERO, |q| q.after_slippage.buy_amount);
        let b_amount =
            b.quote.as_ref().map_or(alloy_primitives::U256::ZERO, |q| q.after_slippage.buy_amount);
        b_amount.cmp(&a_amount)
    });

    results
}

// ── Cache key ───────────────────────────────────────────────────────────────

/// Compute a cache key for a bridge request.
///
/// Produces a deterministic string key from the request's chain IDs and
/// token addresses, suitable for use as a hash-map key.
///
/// Mirrors `getCacheKey` from `BridgingSdk/utils.ts` (which delegates to
/// `hashQuote`).
///
/// # Returns
///
/// A string in the format `"{sell_chain}-{buy_chain}-{sell_token}-{buy_token}"`
/// where token addresses are hex-encoded with a `0x` prefix.
#[must_use]
pub fn get_cache_key(request: &QuoteBridgeRequest) -> String {
    format!(
        "{}-{}-{:#x}-{:#x}",
        request.sell_chain_id, request.buy_chain_id, request.sell_token, request.buy_token,
    )
}

// ── Safe callback invocation ────────────────────────────────────────────────

/// Safely invoke a "best quote" callback, catching panics.
///
/// Mirrors `safeCallBestQuoteCallback` from `BridgingSdk/utils.ts`.
/// If the callback panics, the panic is caught and logged via
/// [`tracing::warn!`]; it does not propagate.
///
/// # Arguments
///
/// * `callback` — An optional closure to invoke with the best quote result. If `None`, this
///   function is a no-op.
/// * `result` — The [`MultiQuoteResult`] to pass to the callback.
pub fn safe_call_best_quote_callback<F: FnOnce(&MultiQuoteResult)>(
    callback: Option<F>,
    result: &MultiQuoteResult,
) {
    if let Some(cb) = callback {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cb(result);
        }));
        if let Err(e) = outcome {
            tracing::warn!("Error in best-quote callback: {:?}", e);
        }
    }
}

/// Safely invoke a "progressive quote" callback, catching panics.
///
/// Mirrors `safeCallProgressiveCallback` from `BridgingSdk/utils.ts`.
/// If the callback panics, the panic is caught and logged via
/// [`tracing::warn!`]; it does not propagate.
///
/// # Arguments
///
/// * `callback` — An optional closure to invoke with the progressive quote result. If `None`, this
///   function is a no-op.
/// * `result` — The [`MultiQuoteResult`] to pass to the callback.
pub fn safe_call_progressive_callback<F: FnOnce(&MultiQuoteResult)>(
    callback: Option<F>,
    result: &MultiQuoteResult,
) {
    if let Some(cb) = callback {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cb(result);
        }));
        if let Err(e) = outcome {
            tracing::warn!("Error in progressive-quote callback: {:?}", e);
        }
    }
}

// ── Hook-based and receiver-account bridge quote ────────────────────────────

/// Orchestrate a cross-chain quote for a hook-based bridge (Across, Bungee, …).
///
/// Mirrors `getQuoteWithHookBridge` from
/// `packages/bridging/src/BridgingSdk/getQuoteWithBridge.ts:209-312`.
///
/// # Flow
///
/// 1. Estimate gas for the bridge post-hook
///    ([`crate::provider::HookBridgeProvider::get_gas_limit_estimation_for_hook`]).
/// 2. Quote the intermediate swap via [`get_intermediate_swap_result`] (app-data carries a
///    cost-estimation mock hook so the swap quote sees realistic gas).
/// 3. Either:
///    - **With a signer** (`params.quote_signer.is_some()`): call [`get_bridge_signed_hook`] to
///      fetch the bridge quote, unsigned call, derive the hook nonce, and produce a real EIP-712
///      signed hook via `cow-shed`.
///    - **Without a signer**: fetch [`BridgeProvider::get_quote`] +
///      [`crate::provider::HookBridgeProvider::get_unsigned_bridge_call`] and package with a
///      placeholder mock hook — the real hook is signed later during the post flow.
/// 4. Package the result in a [`BridgeQuoteAndPost`] whose `bridge.bridge_call_details` carries the
///    unsigned call and either the real or mock pre-authorized hook.
///
/// # Errors
///
/// Returns [`BridgeError::TxBuildError`] if any downstream quoter / provider call fails.
pub async fn get_quote_with_hook_bridge(
    hook_provider: &dyn crate::provider::HookBridgeProvider,
    params: &GetQuoteWithBridgeParams,
    quoter: &dyn SwapQuoter,
) -> Result<BridgeQuoteAndPost, BridgeError> {
    // 1. Gas limit estimation for the post-hook (real value used by the mock hook so that the
    //    intermediate swap sees realistic gas).
    let hook_gas_limit = hook_provider
        .get_gas_limit_estimation_for_hook(
            true,
            Some(DEFAULT_EXTRA_GAS_FOR_HOOK_ESTIMATION),
            Some(DEFAULT_EXTRA_GAS_PROXY_CREATION),
        )
        .await
        .map_err(|e| BridgeError::TxBuildError(e.to_string()))?;

    // 2. Intermediate swap result (reuses PR #6 implementation).
    let swap = get_intermediate_swap_result(
        &params.swap_and_bridge_request,
        hook_provider,
        quoter,
        params.advanced_settings_metadata.as_ref(),
    )
    .await?;

    // 3. Produce the bridge-call details — signed or mock depending on whether a `quote_signer` is
    //    available.
    let (unsigned_bridge_call, bridge_response, pre_authorized_bridging_hook) =
        if let Some(signer) = &params.quote_signer {
            let chain_id = cow_chains::SupportedChainId::try_from(
                params.swap_and_bridge_request.sell_chain_id,
            )
            .map_err(|e| {
                BridgeError::TxBuildError(format!(
                    "unsupported sell_chain_id {} for hook signing: {e}",
                    params.swap_and_bridge_request.sell_chain_id,
                ))
            })?;
            let deadline = params.hook_deadline.unwrap_or_else(|| u64::from(u32::MAX));
            let ctx = GetBridgeSignedHookContext {
                signer: signer.as_ref(),
                hook_gas_limit,
                chain_id,
                deadline,
            };
            let out =
                get_bridge_signed_hook(hook_provider, &params.swap_and_bridge_request, ctx).await?;
            (out.unsigned_bridge_call, out.bridging_quote, out.hook)
        } else {
            let bridge_response = hook_provider
                .get_quote(&params.swap_and_bridge_request)
                .await
                .map_err(|e| BridgeError::TxBuildError(e.to_string()))?;
            let unsigned_call = hook_provider
                .get_unsigned_bridge_call(&params.swap_and_bridge_request, &bridge_response)
                .await
                .map_err(|e| BridgeError::TxBuildError(e.to_string()))?;
            let mock_post_hook = crate::utils::hook_mock_for_cost_estimation(hook_gas_limit);
            let hook = BridgeHook {
                post_hook: mock_post_hook,
                recipient: format!("{:#x}", params.swap_and_bridge_request.account),
            };
            (unsigned_call, bridge_response, hook)
        };

    // 4. Assemble the BridgeQuoteResult + BridgeCallDetails.
    let quote = minimal_bridge_quote_result(&params.swap_and_bridge_request, &bridge_response);

    Ok(BridgeQuoteAndPost {
        swap,
        bridge: crate::types::BridgeQuoteResults {
            provider_info: hook_provider.info().clone(),
            quote,
            bridge_call_details: Some(crate::types::BridgeCallDetails {
                unsigned_bridge_call,
                pre_authorized_bridging_hook,
            }),
            bridge_receiver_override: None,
        },
    })
}

/// Orchestrate a cross-chain quote for a receiver-account-based bridge (NEAR Intents, …).
///
/// Mirrors `getQuoteWithReceiverAccountBridge` from
/// `packages/bridging/src/BridgingSdk/getQuoteWithBridge.ts:145-207`.
///
/// # Flow
///
/// 1. Quote the intermediate swap via [`get_intermediate_swap_result`] (no hook injection — the
///    bridge is triggered by the deposit itself, not a post-hook).
/// 2. Ask the provider for a bridge quote ([`BridgeProvider::get_quote`]).
/// 3. Ask the provider for the deposit-address override
///    ([`crate::provider::ReceiverAccountBridgeProvider::get_bridge_receiver_override`]).
/// 4. Package the result in a [`BridgeQuoteAndPost`] where `bridge.bridge_receiver_override` holds
///    the deposit address and `bridge.bridge_call_details` is `None`.
///
/// # Errors
///
/// Returns [`BridgeError::TxBuildError`] if any downstream quoter / provider call fails.
pub async fn get_quote_with_receiver_account_bridge(
    receiver_provider: &dyn crate::provider::ReceiverAccountBridgeProvider,
    params: &GetQuoteWithBridgeParams,
    quoter: &dyn SwapQuoter,
) -> Result<BridgeQuoteAndPost, BridgeError> {
    // 1. Intermediate swap result — no hook; just the metadata fix (#852).
    let swap = get_intermediate_swap_result(
        &params.swap_and_bridge_request,
        receiver_provider,
        quoter,
        params.advanced_settings_metadata.as_ref(),
    )
    .await?;

    // 2. Bridge quote.
    let bridge_response = receiver_provider
        .get_quote(&params.swap_and_bridge_request)
        .await
        .map_err(|e| BridgeError::TxBuildError(e.to_string()))?;

    // 3. Deposit-address override.
    let receiver_override = receiver_provider
        .get_bridge_receiver_override(&params.swap_and_bridge_request, &bridge_response)
        .await
        .map_err(|e| BridgeError::TxBuildError(e.to_string()))?;

    let quote = minimal_bridge_quote_result(&params.swap_and_bridge_request, &bridge_response);

    Ok(BridgeQuoteAndPost {
        swap,
        bridge: crate::types::BridgeQuoteResults {
            provider_info: receiver_provider.info().clone(),
            quote,
            bridge_call_details: None,
            bridge_receiver_override: Some(receiver_override),
        },
    })
}

/// Build a minimal [`crate::types::BridgeQuoteResult`] from a provider's
/// [`QuoteBridgeResponse`].
///
/// The orchestration layer doesn't have access to the richer
/// provider-specific conversion (`to_bridge_quote_result` for Across,
/// `bungee_to_bridge_quote_result` for Bungee) because those take
/// provider-specific API responses as input. We rebuild the minimal
/// `BridgeQuoteResult` from the `QuoteBridgeResponse` alone so the
/// orchestrator can wrap the result in a [`BridgeQuoteAndPost`].
///
/// The simplification is intentional: `BridgeQuoteResult` holds more
/// granular fee breakdown than `QuoteBridgeResponse` does, so a subset
/// of fields end up defaulted.
fn minimal_bridge_quote_result(
    request: &QuoteBridgeRequest,
    response: &QuoteBridgeResponse,
) -> crate::types::BridgeQuoteResult {
    use crate::types::{
        BridgeAmounts, BridgeCosts, BridgeFees, BridgeLimits, BridgeQuoteAmountsAndCosts,
        BridgingFee,
    };

    let fee = response.fee_amount;
    let before_fee_buy = response.buy_amount.saturating_add(fee);

    BridgeQuoteResult {
        id: None,
        signature: None,
        attestation_signature: None,
        quote_body: None,
        is_sell: request.kind == cow_types::OrderKind::Sell,
        amounts_and_costs: BridgeQuoteAmountsAndCosts {
            before_fee: BridgeAmounts {
                sell_amount: response.sell_amount,
                buy_amount: before_fee_buy,
            },
            after_fee: BridgeAmounts {
                sell_amount: response.sell_amount,
                buy_amount: response.buy_amount,
            },
            after_slippage: BridgeAmounts {
                sell_amount: response.sell_amount,
                buy_amount: response.buy_amount,
            },
            costs: BridgeCosts {
                bridging_fee: BridgingFee {
                    fee_bps: 0,
                    amount_in_sell_currency: fee,
                    amount_in_buy_currency: fee,
                },
            },
            slippage_bps: request.slippage_bps,
        },
        expected_fill_time_seconds: Some(response.estimated_secs),
        quote_timestamp: 0,
        fees: BridgeFees { bridge_fee: fee, destination_gas_fee: alloy_primitives::U256::ZERO },
        limits: BridgeLimits {
            min_deposit: alloy_primitives::U256::ZERO,
            max_deposit: alloy_primitives::U256::MAX,
        },
    }
}

// ── Test utilities ──────────────────────────────────────────────────────────

#[cfg(test)]
pub mod test_helpers {
    //! Test helper utilities ported from the `TypeScript` bridging test package.
    //!
    //! Mirrors:
    //! - `expectToEqual` from `test/utils.ts`
    //! - `getMockSigner` / `getPk` / `getWallet` / `getRpcProvider` from `test/getWallet.ts`

    use alloy_primitives::Address;
    use alloy_signer_local::PrivateKeySigner;

    /// A well-known test private key (DO NOT use in production).
    ///
    /// This is the standard Hardhat account #0 key.
    pub const TEST_PRIVATE_KEY: &str =
        "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    /// Return the test private key hex string.
    ///
    /// Mirrors `getPk()` from the `TypeScript` test utilities.
    #[must_use]
    pub fn get_pk() -> &'static str {
        TEST_PRIVATE_KEY
    }

    /// Create a [`PrivateKeySigner`] from the test private key.
    ///
    /// Mirrors `getMockSigner()` / `getWallet()` from the `TypeScript` test utilities.
    #[must_use]
    pub fn get_mock_signer() -> PrivateKeySigner {
        TEST_PRIVATE_KEY.parse::<PrivateKeySigner>().expect("valid test key")
    }

    /// Alias for [`get_mock_signer`].
    #[must_use]
    pub fn get_wallet() -> PrivateKeySigner {
        get_mock_signer()
    }

    /// Return a test RPC URL.
    ///
    /// Mirrors `getRpcProvider()` from the `TypeScript` test utilities.
    /// Returns the default Ethereum mainnet public RPC endpoint.
    #[must_use]
    pub fn get_rpc_provider() -> &'static str {
        "https://eth.llamarpc.com"
    }

    /// Assert that two serializable values produce the same JSON string.
    ///
    /// Mirrors `expectToEqual(a, b)` from the `TypeScript` test utilities,
    /// which compares `JSON.stringify(a, jsonWithBigintReplacer)` outputs.
    ///
    /// # Panics
    ///
    /// Panics if the serialised forms differ.
    pub fn expect_to_equal<T: serde::Serialize>(actual: &T, expected: &T) {
        let actual_json = serde_json::to_string_pretty(actual).expect("failed to serialise actual");
        let expected_json =
            serde_json::to_string_pretty(expected).expect("failed to serialise expected");
        assert_eq!(actual_json, expected_json);
    }

    /// Return the address corresponding to the test private key.
    #[must_use]
    pub fn test_address() -> Address {
        get_mock_signer().address()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn mock_signer_has_expected_address() {
            let signer = get_mock_signer();
            // Hardhat account #0
            let expected: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap();
            assert_eq!(signer.address(), expected);
        }

        #[test]
        fn expect_to_equal_passes_for_equal_values() {
            expect_to_equal(&42u64, &42u64);
        }

        #[test]
        #[should_panic]
        fn expect_to_equal_panics_for_different_values() {
            expect_to_equal(&42u64, &43u64);
        }

        #[test]
        fn get_pk_returns_key() {
            assert_eq!(get_pk().len(), 64);
        }

        #[test]
        fn get_rpc_provider_returns_url() {
            assert!(get_rpc_provider().starts_with("https://"));
        }
    }
}

#[cfg(test)]
#[allow(clippy::tests_outside_test_module, reason = "inner module pattern")]
mod intermediate_swap_tests {
    use alloy_primitives::{B256, U256};
    use cow_types::OrderKind;

    use super::*;
    use crate::{
        provider::{
            BridgeNetworkInfo, BridgeStatusFuture, BridgingParamsFuture, BuyTokensFuture,
            IntermediateTokensFuture, NetworksFuture, QuoteFuture,
        },
        swap_quoter::{QuoteSwapFuture, SwapQuoteOutcome, SwapQuoteParams},
        types::{
            BridgeProviderInfo, BridgeProviderType, BuyTokensParams, GetProviderBuyTokens,
            IntermediateTokenInfo,
        },
    };

    fn dummy_info(name: &str) -> BridgeProviderInfo {
        BridgeProviderInfo {
            name: name.to_owned(),
            logo_url: String::new(),
            dapp_id: format!("cow-sdk://bridging/providers/{name}"),
            website: String::new(),
            provider_type: BridgeProviderType::HookBridgeProvider,
        }
    }

    struct FixedProvider {
        info: BridgeProviderInfo,
        tokens: Vec<IntermediateTokenInfo>,
    }

    impl BridgeProvider for FixedProvider {
        fn info(&self) -> &BridgeProviderInfo {
            &self.info
        }
        fn supports_route(&self, _s: u64, _b: u64) -> bool {
            true
        }
        fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
            Box::pin(async { Ok(Vec::<BridgeNetworkInfo>::new()) })
        }
        fn get_buy_tokens<'a>(&'a self, _p: BuyTokensParams) -> BuyTokensFuture<'a> {
            let info = self.info.clone();
            Box::pin(
                async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) },
            )
        }
        fn get_intermediate_tokens<'a>(
            &'a self,
            _req: &'a QuoteBridgeRequest,
        ) -> IntermediateTokensFuture<'a> {
            let tokens = self.tokens.clone();
            Box::pin(async move { Ok(tokens) })
        }
        fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
            Box::pin(async {
                Ok(QuoteBridgeResponse {
                    provider: "fixed".into(),
                    sell_amount: U256::ZERO,
                    buy_amount: U256::ZERO,
                    fee_amount: U256::ZERO,
                    estimated_secs: 0,
                    bridge_hook: None,
                })
            })
        }
        fn get_bridging_params<'a>(
            &'a self,
            _c: u64,
            _o: &'a cow_orderbook::types::Order,
            _t: B256,
            _s: Option<Address>,
        ) -> BridgingParamsFuture<'a> {
            Box::pin(async { Ok(None) })
        }
        fn get_explorer_url(&self, _id: &str) -> String {
            String::new()
        }
        fn get_status<'a>(&'a self, _id: &'a str, _c: u64) -> BridgeStatusFuture<'a> {
            Box::pin(async {
                Ok(BridgeStatusResult {
                    status: BridgeStatus::Unknown,
                    fill_time_in_seconds: None,
                    deposit_tx_hash: None,
                    fill_tx_hash: None,
                })
            })
        }
    }

    struct CapturingQuoter {
        captured: std::sync::OnceLock<SwapQuoteParams>,
        outcome: SwapQuoteOutcome,
    }

    impl SwapQuoter for CapturingQuoter {
        fn quote_swap<'a>(&'a self, params: SwapQuoteParams) -> QuoteSwapFuture<'a> {
            self.captured.set(params).ok();
            let outcome = self.outcome.clone();
            Box::pin(async move { Ok(outcome) })
        }
    }

    fn usdc_token() -> IntermediateTokenInfo {
        IntermediateTokenInfo {
            chain_id: 1,
            address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap(),
            decimals: 6,
            symbol: "USDC".into(),
            name: "USD Coin".into(),
            logo_url: None,
        }
    }

    fn sample_request() -> QuoteBridgeRequest {
        QuoteBridgeRequest {
            sell_chain_id: 1,
            buy_chain_id: 42_161,
            sell_token: Address::repeat_byte(0x11),
            sell_token_decimals: 18,
            buy_token: Address::repeat_byte(0x22),
            buy_token_decimals: 6,
            sell_amount: U256::from(1_000_000u64),
            account: Address::repeat_byte(0x33),
            owner: None,
            receiver: None,
            bridge_recipient: None,
            slippage_bps: 50,
            bridge_slippage_bps: None,
            kind: OrderKind::Sell,
        }
    }

    fn default_outcome() -> SwapQuoteOutcome {
        SwapQuoteOutcome {
            sell_amount: U256::from(1_000_000u64),
            buy_amount_after_slippage: U256::from(999_500u64),
            fee_amount: U256::from(500u64),
            valid_to: 9_999_999,
            app_data_hex: "0xabc".into(),
            full_app_data: "{\"version\":\"1.4.0\"}".into(),
        }
    }

    #[tokio::test]
    async fn errors_when_provider_has_no_candidates() {
        let provider = FixedProvider { info: dummy_info("p"), tokens: vec![] };
        let quoter =
            CapturingQuoter { captured: std::sync::OnceLock::new(), outcome: default_outcome() };
        let err = get_intermediate_swap_result(&sample_request(), &provider, &quoter, None)
            .await
            .unwrap_err();
        assert!(matches!(err, BridgeError::NoIntermediateTokens));
    }

    #[tokio::test]
    async fn picks_first_candidate_and_returns_wrapped_outcome() {
        let provider = FixedProvider { info: dummy_info("p"), tokens: vec![usdc_token()] };
        let quoter =
            CapturingQuoter { captured: std::sync::OnceLock::new(), outcome: default_outcome() };
        let resp = get_intermediate_swap_result(&sample_request(), &provider, &quoter, None)
            .await
            .unwrap();
        assert_eq!(resp.provider, "p");
        assert_eq!(resp.buy_amount, U256::from(999_500u64));
        assert_eq!(resp.fee_amount, U256::from(500u64));
    }

    #[tokio::test]
    async fn threads_intermediate_token_to_quoter() {
        let provider = FixedProvider { info: dummy_info("p"), tokens: vec![usdc_token()] };
        let quoter =
            CapturingQuoter { captured: std::sync::OnceLock::new(), outcome: default_outcome() };
        get_intermediate_swap_result(&sample_request(), &provider, &quoter, None).await.unwrap();
        let captured = quoter.captured.get().cloned().expect("quoter called");
        assert_eq!(captured.buy_token, usdc_token().address);
        assert_eq!(captured.buy_token_decimals, 6);
        assert_eq!(captured.chain_id, 1);
    }

    // ── cow-sdk#852 metadata preservation ────────────────────────────────

    #[tokio::test]
    async fn fix_852_preserves_caller_metadata() {
        let provider = FixedProvider { info: dummy_info("cow-prov"), tokens: vec![usdc_token()] };
        let quoter =
            CapturingQuoter { captured: std::sync::OnceLock::new(), outcome: default_outcome() };
        let caller_meta = serde_json::json!({
            "partnerFee":   { "bps": 25, "recipient": "0xpartner" },
            "utm":          { "utmSource": "cow-widget" },
            "orderClass":   { "orderClass": "market" }
        });

        get_intermediate_swap_result(&sample_request(), &provider, &quoter, Some(&caller_meta))
            .await
            .unwrap();

        let captured = quoter.captured.get().cloned().expect("quoter called");
        let app_data_json = captured.app_data_json.expect("app_data threaded through");
        let parsed: serde_json::Value = serde_json::from_str(&app_data_json).unwrap();
        let metadata = parsed.get("metadata").expect("metadata key present");

        // Caller metadata survived.
        assert_eq!(
            metadata.get("partnerFee").and_then(|v| v.get("bps")).and_then(|v| v.as_u64()),
            Some(25)
        );
        assert_eq!(
            metadata.get("utm").and_then(|v| v.get("utmSource")).and_then(|v| v.as_str()),
            Some("cow-widget")
        );
        assert_eq!(
            metadata.get("orderClass").and_then(|v| v.get("orderClass")).and_then(|v| v.as_str()),
            Some("market")
        );

        // Auto-generated bridging entry present.
        assert_eq!(
            metadata.get("bridging").and_then(|v| v.get("providerId")).and_then(|v| v.as_str()),
            Some("cow-sdk://bridging/providers/cow-prov")
        );

        // Hooks default to an empty post list if the caller didn't supply any.
        assert!(metadata.get("hooks").is_some());
    }

    #[tokio::test]
    async fn bridging_entry_overwrites_caller_attempt_to_inject_its_own() {
        let provider = FixedProvider { info: dummy_info("cow-prov"), tokens: vec![usdc_token()] };
        let quoter =
            CapturingQuoter { captured: std::sync::OnceLock::new(), outcome: default_outcome() };
        // Caller tries to inject a different providerId — the auto-generated
        // one must win because that's what the on-chain hook encodes.
        let caller_meta = serde_json::json!({
            "bridging": { "providerId": "caller-spoofed" },
        });

        get_intermediate_swap_result(&sample_request(), &provider, &quoter, Some(&caller_meta))
            .await
            .unwrap();
        let captured = quoter.captured.get().cloned().unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&captured.app_data_json.unwrap()).unwrap();
        assert_eq!(
            parsed.pointer("/metadata/bridging/providerId").and_then(|v| v.as_str()),
            Some("cow-sdk://bridging/providers/cow-prov")
        );
    }

    // ── Error-path coverage ─────────────────────────────────────────────

    #[tokio::test]
    async fn propagates_quoter_error_as_tx_build_error() {
        struct FailingQuoter;
        impl SwapQuoter for FailingQuoter {
            fn quote_swap<'a>(&'a self, _p: SwapQuoteParams) -> QuoteSwapFuture<'a> {
                Box::pin(async {
                    Err(cow_errors::CowError::Api { status: 500, body: "orderbook down".into() })
                })
            }
        }
        let provider = FixedProvider { info: dummy_info("p"), tokens: vec![usdc_token()] };
        let err = get_intermediate_swap_result(&sample_request(), &provider, &FailingQuoter, None)
            .await
            .unwrap_err();
        let msg = if let BridgeError::TxBuildError(s) = err {
            s
        } else {
            panic!("expected TxBuildError, got {err:?}")
        };
        assert!(msg.contains("500"));
        assert!(msg.contains("orderbook down"));
    }

    #[tokio::test]
    async fn errors_when_all_candidates_are_the_sell_token() {
        // `determine_intermediate_token` filters candidates equal to the
        // sell token when `allow_intermediate_eq_sell = false`. With ≥ 2
        // candidates all equal to the sell token the filter empties and
        // the function must surface `NoIntermediateTokens`.
        let req = sample_request();
        let same = |chain| IntermediateTokenInfo {
            chain_id: chain,
            address: req.sell_token,
            decimals: 18,
            symbol: "SELL".into(),
            name: "Sell Token".into(),
            logo_url: None,
        };
        struct Never;
        impl SwapQuoter for Never {
            fn quote_swap<'a>(&'a self, _p: SwapQuoteParams) -> QuoteSwapFuture<'a> {
                Box::pin(async { panic!("quoter should not be called") })
            }
        }
        let provider = FixedProvider {
            info: dummy_info("p"),
            tokens: vec![same(req.sell_chain_id), same(req.sell_chain_id)],
        };
        let err = get_intermediate_swap_result(&req, &provider, &Never, None).await.unwrap_err();
        assert!(matches!(err, BridgeError::NoIntermediateTokens));
    }

    #[tokio::test]
    async fn provider_info_name_is_threaded_into_response() {
        let provider = FixedProvider { info: dummy_info("zany"), tokens: vec![usdc_token()] };
        let quoter =
            CapturingQuoter { captured: std::sync::OnceLock::new(), outcome: default_outcome() };
        let resp = get_intermediate_swap_result(&sample_request(), &provider, &quoter, None)
            .await
            .unwrap();
        assert_eq!(resp.provider, "zany");
    }

    #[tokio::test]
    async fn non_object_caller_metadata_is_ignored_gracefully() {
        let provider = FixedProvider { info: dummy_info("p"), tokens: vec![usdc_token()] };
        let quoter =
            CapturingQuoter { captured: std::sync::OnceLock::new(), outcome: default_outcome() };
        // A JSON scalar is not a metadata object — the function must
        // tolerate it without panicking and still inject the bridging entry.
        let bogus = serde_json::json!("not-an-object");
        get_intermediate_swap_result(&sample_request(), &provider, &quoter, Some(&bogus))
            .await
            .unwrap();
        let captured = quoter.captured.get().cloned().unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&captured.app_data_json.unwrap()).unwrap();
        assert!(parsed.pointer("/metadata/bridging/providerId").is_some());
    }

    #[tokio::test]
    async fn caller_hooks_entry_is_preserved_when_present() {
        // If the caller already supplied a hooks entry, we must not clobber
        // it with the empty default — some flows pre-populate `hooks.pre`.
        let provider = FixedProvider { info: dummy_info("p"), tokens: vec![usdc_token()] };
        let quoter =
            CapturingQuoter { captured: std::sync::OnceLock::new(), outcome: default_outcome() };
        let caller_meta = serde_json::json!({
            "hooks": { "pre": [{ "target": "0xabc", "callData": "0x", "gasLimit": "100000" }], "post": [] },
        });
        get_intermediate_swap_result(&sample_request(), &provider, &quoter, Some(&caller_meta))
            .await
            .unwrap();
        let captured = quoter.captured.get().cloned().unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&captured.app_data_json.unwrap()).unwrap();
        let pre = parsed
            .pointer("/metadata/hooks/pre")
            .and_then(|v| v.as_array())
            .expect("pre array present");
        assert_eq!(pre.len(), 1);
    }

    #[tokio::test]
    async fn fixed_provider_surface_is_callable_for_coverage() {
        // Exercise every `FixedProvider` trait method once — otherwise the
        // trait-impl rows stay uncovered because `get_intermediate_swap_result`
        // only calls `info()` + `get_intermediate_tokens()`.
        use alloy_primitives::{Address, B256};
        let p = FixedProvider { info: dummy_info("surface"), tokens: vec![usdc_token()] };
        assert!(p.supports_route(1, 10));
        assert!(p.get_networks().await.unwrap().is_empty());
        let toks = p
            .get_buy_tokens(BuyTokensParams {
                sell_chain_id: 1,
                buy_chain_id: 10,
                sell_token_address: None,
            })
            .await
            .unwrap();
        assert!(toks.tokens.is_empty());
        assert_eq!(p.get_quote(&sample_request()).await.unwrap().provider, "fixed");
        let order = cow_orderbook::api::mock_get_order(&format!("0x{}", "aa".repeat(56)));
        assert!(p.get_bridging_params(1, &order, B256::ZERO, None).await.unwrap().is_none());
        assert!(p.get_explorer_url("x").is_empty());
        assert_eq!(p.get_status("x", 1).await.unwrap().status, BridgeStatus::Unknown);
        // `Address` is imported to silence the unused-import lint if the
        // local scope ever loses its reference.
        let _ = Address::ZERO;
    }

    #[tokio::test]
    async fn no_caller_metadata_still_produces_bridging_entry() {
        let provider = FixedProvider { info: dummy_info("cow-prov"), tokens: vec![usdc_token()] };
        let quoter =
            CapturingQuoter { captured: std::sync::OnceLock::new(), outcome: default_outcome() };
        get_intermediate_swap_result(&sample_request(), &provider, &quoter, None).await.unwrap();
        let captured = quoter.captured.get().cloned().unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&captured.app_data_json.unwrap()).unwrap();
        assert!(parsed.pointer("/metadata/bridging/providerId").is_some());
        assert!(parsed.pointer("/metadata/hooks").is_some());
    }
}

// ── Orchestration tests (PR #7) ──────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::tests_outside_test_module, reason = "inner module pattern")]
mod orchestration_tests {
    use alloy_primitives::{B256, U256};
    use cow_chains::EvmCall;
    use cow_types::OrderKind;

    use super::*;
    use crate::{
        provider::{
            BridgeNetworkInfo, BridgeStatusFuture, BridgingParamsFuture, BuyTokensFuture,
            GasEstimationFuture, HookBridgeProvider, IntermediateTokensFuture, NetworksFuture,
            QuoteFuture, ReceiverAccountBridgeProvider, ReceiverOverrideFuture, SignedHookFuture,
            UnsignedCallFuture,
        },
        swap_quoter::{QuoteSwapFuture, SwapQuoteOutcome, SwapQuoteParams},
        types::{
            BridgeProviderInfo, BridgeProviderType, BuyTokensParams, GetProviderBuyTokens,
            IntermediateTokenInfo,
        },
    };

    fn hook_info() -> BridgeProviderInfo {
        BridgeProviderInfo {
            name: "mock-hook".into(),
            logo_url: String::new(),
            dapp_id: "cow-sdk://bridging/providers/mock-hook".into(),
            website: String::new(),
            provider_type: BridgeProviderType::HookBridgeProvider,
        }
    }

    fn receiver_info() -> BridgeProviderInfo {
        BridgeProviderInfo {
            name: "mock-receiver".into(),
            logo_url: String::new(),
            dapp_id: "cow-sdk://bridging/providers/mock-receiver".into(),
            website: String::new(),
            provider_type: BridgeProviderType::ReceiverAccountBridgeProvider,
        }
    }

    fn usdc() -> IntermediateTokenInfo {
        IntermediateTokenInfo {
            chain_id: 1,
            address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap(),
            decimals: 6,
            symbol: "USDC".into(),
            name: "USD Coin".into(),
            logo_url: None,
        }
    }

    fn sample_request(kind: OrderKind) -> QuoteBridgeRequest {
        QuoteBridgeRequest {
            sell_chain_id: 1,
            buy_chain_id: 42_161,
            sell_token: Address::repeat_byte(0x11),
            sell_token_decimals: 18,
            buy_token: Address::repeat_byte(0x22),
            buy_token_decimals: 6,
            sell_amount: U256::from(1_000_000u64),
            account: Address::repeat_byte(0x33),
            owner: None,
            receiver: None,
            bridge_recipient: None,
            slippage_bps: 50,
            bridge_slippage_bps: None,
            kind,
        }
    }

    fn sample_outcome() -> SwapQuoteOutcome {
        SwapQuoteOutcome {
            sell_amount: U256::from(1_000_000u64),
            buy_amount_after_slippage: U256::from(999_500u64),
            fee_amount: U256::from(500u64),
            valid_to: 9_999_999,
            app_data_hex: "0xabc".into(),
            full_app_data: "{}".into(),
        }
    }

    fn sample_bridge_response(provider_name: &str) -> QuoteBridgeResponse {
        QuoteBridgeResponse {
            provider: provider_name.to_owned(),
            sell_amount: U256::from(999_500u64),
            buy_amount: U256::from(998_000u64),
            fee_amount: U256::from(1_500u64),
            estimated_secs: 42,
            bridge_hook: None,
        }
    }

    // ── Mock providers ───────────────────────────────────────────────────

    /// A hook provider wired to return fixed bridge + unsigned-call data.
    struct MockHookProvider {
        info: BridgeProviderInfo,
        tokens: Vec<IntermediateTokenInfo>,
        bridge_response: QuoteBridgeResponse,
        unsigned_call: EvmCall,
        gas_limit: u64,
    }

    impl BridgeProvider for MockHookProvider {
        fn info(&self) -> &BridgeProviderInfo {
            &self.info
        }
        fn supports_route(&self, _s: u64, _b: u64) -> bool {
            true
        }
        fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
            Box::pin(async { Ok(Vec::<BridgeNetworkInfo>::new()) })
        }
        fn get_buy_tokens<'a>(&'a self, _p: BuyTokensParams) -> BuyTokensFuture<'a> {
            let info = self.info.clone();
            Box::pin(
                async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) },
            )
        }
        fn get_intermediate_tokens<'a>(
            &'a self,
            _req: &'a QuoteBridgeRequest,
        ) -> IntermediateTokensFuture<'a> {
            let tokens = self.tokens.clone();
            Box::pin(async move { Ok(tokens) })
        }
        fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
            let response = self.bridge_response.clone();
            Box::pin(async move { Ok(response) })
        }
        fn get_bridging_params<'a>(
            &'a self,
            _c: u64,
            _o: &'a cow_orderbook::types::Order,
            _t: B256,
            _s: Option<Address>,
        ) -> BridgingParamsFuture<'a> {
            Box::pin(async { Ok(None) })
        }
        fn get_explorer_url(&self, _id: &str) -> String {
            String::new()
        }
        fn get_status<'a>(&'a self, _id: &'a str, _c: u64) -> BridgeStatusFuture<'a> {
            Box::pin(async {
                Ok(BridgeStatusResult {
                    status: BridgeStatus::Unknown,
                    fill_time_in_seconds: None,
                    deposit_tx_hash: None,
                    fill_tx_hash: None,
                })
            })
        }
        fn as_hook_bridge_provider(&self) -> Option<&dyn HookBridgeProvider> {
            Some(self)
        }
    }

    impl HookBridgeProvider for MockHookProvider {
        fn get_unsigned_bridge_call<'a>(
            &'a self,
            _req: &'a QuoteBridgeRequest,
            _quote: &'a QuoteBridgeResponse,
        ) -> UnsignedCallFuture<'a> {
            let call = self.unsigned_call.clone();
            Box::pin(async move { Ok(call) })
        }
        fn get_gas_limit_estimation_for_hook<'a>(
            &'a self,
            _proxy_deployed: bool,
            _extra_gas: Option<u64>,
            _extra_gas_proxy_creation: Option<u64>,
        ) -> GasEstimationFuture<'a> {
            let gas = self.gas_limit;
            Box::pin(async move { Ok(gas) })
        }
        fn get_signed_hook<'a>(
            &'a self,
            _chain_id: cow_chains::SupportedChainId,
            _unsigned_call: &'a EvmCall,
            _nonce: &'a str,
            _deadline: u64,
            _gas: u64,
            _signer: &'a alloy_signer_local::PrivateKeySigner,
        ) -> SignedHookFuture<'a> {
            Box::pin(async {
                Err(cow_errors::CowError::Signing("not needed in PR #7 tests".into()))
            })
        }
    }

    /// A receiver-account provider wired to return a fixed deposit address.
    struct MockReceiverProvider {
        info: BridgeProviderInfo,
        tokens: Vec<IntermediateTokenInfo>,
        bridge_response: QuoteBridgeResponse,
        deposit_address: String,
    }

    impl BridgeProvider for MockReceiverProvider {
        fn info(&self) -> &BridgeProviderInfo {
            &self.info
        }
        fn supports_route(&self, _s: u64, _b: u64) -> bool {
            true
        }
        fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
            Box::pin(async { Ok(Vec::<BridgeNetworkInfo>::new()) })
        }
        fn get_buy_tokens<'a>(&'a self, _p: BuyTokensParams) -> BuyTokensFuture<'a> {
            let info = self.info.clone();
            Box::pin(
                async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) },
            )
        }
        fn get_intermediate_tokens<'a>(
            &'a self,
            _req: &'a QuoteBridgeRequest,
        ) -> IntermediateTokensFuture<'a> {
            let tokens = self.tokens.clone();
            Box::pin(async move { Ok(tokens) })
        }
        fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
            let response = self.bridge_response.clone();
            Box::pin(async move { Ok(response) })
        }
        fn get_bridging_params<'a>(
            &'a self,
            _c: u64,
            _o: &'a cow_orderbook::types::Order,
            _t: B256,
            _s: Option<Address>,
        ) -> BridgingParamsFuture<'a> {
            Box::pin(async { Ok(None) })
        }
        fn get_explorer_url(&self, _id: &str) -> String {
            String::new()
        }
        fn get_status<'a>(&'a self, _id: &'a str, _c: u64) -> BridgeStatusFuture<'a> {
            Box::pin(async {
                Ok(BridgeStatusResult {
                    status: BridgeStatus::Unknown,
                    fill_time_in_seconds: None,
                    deposit_tx_hash: None,
                    fill_tx_hash: None,
                })
            })
        }
        fn as_receiver_account_bridge_provider(
            &self,
        ) -> Option<&dyn ReceiverAccountBridgeProvider> {
            Some(self)
        }
    }

    impl ReceiverAccountBridgeProvider for MockReceiverProvider {
        fn get_bridge_receiver_override<'a>(
            &'a self,
            _quote_request: &'a QuoteBridgeRequest,
            _quote_result: &'a QuoteBridgeResponse,
        ) -> ReceiverOverrideFuture<'a> {
            let addr = self.deposit_address.clone();
            Box::pin(async move { Ok(addr) })
        }
    }

    /// A provider that implements neither sub-trait.
    struct MockUnknownProvider {
        info: BridgeProviderInfo,
    }

    impl BridgeProvider for MockUnknownProvider {
        fn info(&self) -> &BridgeProviderInfo {
            &self.info
        }
        fn supports_route(&self, _s: u64, _b: u64) -> bool {
            true
        }
        fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
            Box::pin(async { Ok(Vec::<BridgeNetworkInfo>::new()) })
        }
        fn get_buy_tokens<'a>(&'a self, _p: BuyTokensParams) -> BuyTokensFuture<'a> {
            let info = self.info.clone();
            Box::pin(
                async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) },
            )
        }
        fn get_intermediate_tokens<'a>(
            &'a self,
            _req: &'a QuoteBridgeRequest,
        ) -> IntermediateTokensFuture<'a> {
            Box::pin(async { Ok(Vec::<IntermediateTokenInfo>::new()) })
        }
        fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
            Box::pin(async { Ok(sample_bridge_response("unknown")) })
        }
        fn get_bridging_params<'a>(
            &'a self,
            _c: u64,
            _o: &'a cow_orderbook::types::Order,
            _t: B256,
            _s: Option<Address>,
        ) -> BridgingParamsFuture<'a> {
            Box::pin(async { Ok(None) })
        }
        fn get_explorer_url(&self, _id: &str) -> String {
            String::new()
        }
        fn get_status<'a>(&'a self, _id: &'a str, _c: u64) -> BridgeStatusFuture<'a> {
            Box::pin(async {
                Ok(BridgeStatusResult {
                    status: BridgeStatus::Unknown,
                    fill_time_in_seconds: None,
                    deposit_tx_hash: None,
                    fill_tx_hash: None,
                })
            })
        }
    }

    struct FixedQuoter {
        outcome: SwapQuoteOutcome,
        captured: std::sync::OnceLock<SwapQuoteParams>,
    }

    impl SwapQuoter for FixedQuoter {
        fn quote_swap<'a>(&'a self, params: SwapQuoteParams) -> QuoteSwapFuture<'a> {
            self.captured.set(params).ok();
            let outcome = self.outcome.clone();
            Box::pin(async move { Ok(outcome) })
        }
    }

    fn build_unsigned_call() -> EvmCall {
        EvmCall { to: Address::repeat_byte(0xAC), data: vec![0xde, 0xad], value: U256::ZERO }
    }

    fn hook_params_with_metadata(metadata: Option<serde_json::Value>) -> GetQuoteWithBridgeParams {
        GetQuoteWithBridgeParams {
            swap_and_bridge_request: sample_request(OrderKind::Sell),
            slippage_bps: 50,
            advanced_settings_metadata: metadata,
            quote_signer: None,
            hook_deadline: None,
        }
    }

    // ── Dispatcher tests ─────────────────────────────────────────────────

    #[tokio::test]
    async fn get_quote_with_bridge_rejects_buy_orders() {
        let provider = MockHookProvider {
            info: hook_info(),
            tokens: vec![usdc()],
            bridge_response: sample_bridge_response("mock-hook"),
            unsigned_call: build_unsigned_call(),
            gas_limit: 500_000,
        };
        let quoter =
            FixedQuoter { outcome: sample_outcome(), captured: std::sync::OnceLock::new() };
        let params = GetQuoteWithBridgeParams {
            swap_and_bridge_request: sample_request(OrderKind::Buy),
            slippage_bps: 50,
            advanced_settings_metadata: None,
            quote_signer: None,
            hook_deadline: None,
        };
        let err = get_quote_with_bridge(&params, &provider, &quoter).await.unwrap_err();
        assert!(matches!(err, BridgeError::OnlySellOrderSupported));
    }

    #[tokio::test]
    async fn get_quote_with_bridge_dispatches_to_hook_branch() {
        let provider = MockHookProvider {
            info: hook_info(),
            tokens: vec![usdc()],
            bridge_response: sample_bridge_response("mock-hook"),
            unsigned_call: build_unsigned_call(),
            gas_limit: 500_000,
        };
        let quoter =
            FixedQuoter { outcome: sample_outcome(), captured: std::sync::OnceLock::new() };
        let result = get_quote_with_bridge(&hook_params_with_metadata(None), &provider, &quoter)
            .await
            .unwrap();
        // Hook branch populates bridge_call_details, leaves override empty.
        assert!(result.bridge.bridge_call_details.is_some());
        assert!(result.bridge.bridge_receiver_override.is_none());
        assert_eq!(result.bridge.provider_info.name, "mock-hook");
    }

    #[tokio::test]
    async fn get_quote_with_bridge_dispatches_to_receiver_branch() {
        let provider = MockReceiverProvider {
            info: receiver_info(),
            tokens: vec![usdc()],
            bridge_response: sample_bridge_response("mock-receiver"),
            deposit_address: "0xDEA00DEA00DEA00DEA00DEA00DEA00DEA00DEA000".into(),
        };
        let quoter =
            FixedQuoter { outcome: sample_outcome(), captured: std::sync::OnceLock::new() };
        let result = get_quote_with_bridge(&hook_params_with_metadata(None), &provider, &quoter)
            .await
            .unwrap();
        // Receiver branch populates receiver_override, leaves call_details empty.
        assert!(result.bridge.bridge_call_details.is_none());
        assert_eq!(
            result.bridge.bridge_receiver_override.as_deref(),
            Some("0xDEA00DEA00DEA00DEA00DEA00DEA00DEA00DEA000"),
        );
    }

    #[tokio::test]
    async fn get_quote_with_bridge_errors_on_unknown_provider_kind() {
        let provider = MockUnknownProvider { info: hook_info() };
        let quoter =
            FixedQuoter { outcome: sample_outcome(), captured: std::sync::OnceLock::new() };
        let err = get_quote_with_bridge(&hook_params_with_metadata(None), &provider, &quoter)
            .await
            .unwrap_err();
        if let BridgeError::TxBuildError(msg) = err {
            assert!(msg.contains("implements neither"));
        } else {
            panic!("expected TxBuildError, got {err:?}");
        }
    }

    // ── Metadata preservation (#852 at orchestration level) ──────────────

    #[tokio::test]
    async fn hook_branch_preserves_caller_metadata_on_intermediate_swap() {
        let provider = MockHookProvider {
            info: hook_info(),
            tokens: vec![usdc()],
            bridge_response: sample_bridge_response("mock-hook"),
            unsigned_call: build_unsigned_call(),
            gas_limit: 500_000,
        };
        let quoter =
            FixedQuoter { outcome: sample_outcome(), captured: std::sync::OnceLock::new() };
        let caller_meta = serde_json::json!({
            "partnerFee": { "bps": 25, "recipient": "0xpartner" },
        });
        let params = hook_params_with_metadata(Some(caller_meta));

        get_quote_with_bridge(&params, &provider, &quoter).await.unwrap();

        let captured = quoter.captured.get().cloned().expect("quoter called");
        let app_data: serde_json::Value =
            serde_json::from_str(&captured.app_data_json.unwrap()).unwrap();
        assert_eq!(app_data.pointer("/metadata/partnerFee/bps").and_then(|v| v.as_u64()), Some(25),);
        assert_eq!(
            app_data.pointer("/metadata/bridging/providerId").and_then(|v| v.as_str()),
            Some("cow-sdk://bridging/providers/mock-hook"),
        );
    }

    // ── Simple flows ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn get_quote_without_bridge_calls_quoter_with_full_request() {
        let outcome = sample_outcome();
        let quoter = FixedQuoter { outcome: outcome.clone(), captured: std::sync::OnceLock::new() };
        let result =
            get_quote_without_bridge(&sample_request(OrderKind::Sell), &quoter).await.unwrap();
        assert_eq!(result.quote.sell_amount, outcome.sell_amount);
        assert_eq!(result.quote.buy_amount, outcome.buy_amount_after_slippage);
        assert_eq!(result.quote.fee_amount, outcome.fee_amount);
        assert_eq!(result.quote.provider, "same-chain");

        let captured = quoter.captured.get().cloned().unwrap();
        // `get_quote_without_bridge` maps the *final* buy token directly —
        // no intermediate-token substitution.
        assert_eq!(captured.buy_token, sample_request(OrderKind::Sell).buy_token);
    }

    #[tokio::test]
    async fn get_swap_quote_returns_provider_agnostic_response() {
        let outcome = sample_outcome();
        let quoter = FixedQuoter { outcome: outcome.clone(), captured: std::sync::OnceLock::new() };
        let resp = get_swap_quote(&sample_request(OrderKind::Sell), &quoter).await.unwrap();
        assert_eq!(resp.provider, "swap");
        assert_eq!(resp.buy_amount, outcome.buy_amount_after_slippage);
    }

    #[tokio::test]
    async fn get_quote_without_bridge_propagates_quoter_error() {
        struct Failing;
        impl SwapQuoter for Failing {
            fn quote_swap<'a>(&'a self, _p: SwapQuoteParams) -> QuoteSwapFuture<'a> {
                Box::pin(async {
                    Err(cow_errors::CowError::Api { status: 502, body: "upstream".into() })
                })
            }
        }
        let err =
            get_quote_without_bridge(&sample_request(OrderKind::Sell), &Failing).await.unwrap_err();
        assert!(matches!(err, BridgeError::TxBuildError(_)));
    }

    #[tokio::test]
    async fn get_swap_quote_propagates_quoter_error() {
        struct Failing;
        impl SwapQuoter for Failing {
            fn quote_swap<'a>(&'a self, _p: SwapQuoteParams) -> QuoteSwapFuture<'a> {
                Box::pin(async {
                    Err(cow_errors::CowError::Api { status: 500, body: "boom".into() })
                })
            }
        }
        let err = get_swap_quote(&sample_request(OrderKind::Sell), &Failing).await.unwrap_err();
        assert!(matches!(err, BridgeError::TxBuildError(_)));
    }

    // ── Hook branch error paths ──────────────────────────────────────────

    #[tokio::test]
    async fn hook_branch_propagates_gas_estimation_error() {
        /// Hook provider whose gas estimation fails.
        struct FailingGasProvider {
            info: BridgeProviderInfo,
            tokens: Vec<IntermediateTokenInfo>,
        }
        impl BridgeProvider for FailingGasProvider {
            fn info(&self) -> &BridgeProviderInfo {
                &self.info
            }
            fn supports_route(&self, _s: u64, _b: u64) -> bool {
                true
            }
            fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
                Box::pin(async { Ok(Vec::<BridgeNetworkInfo>::new()) })
            }
            fn get_buy_tokens<'a>(&'a self, _p: BuyTokensParams) -> BuyTokensFuture<'a> {
                let info = self.info.clone();
                Box::pin(
                    async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) },
                )
            }
            fn get_intermediate_tokens<'a>(
                &'a self,
                _req: &'a QuoteBridgeRequest,
            ) -> IntermediateTokensFuture<'a> {
                let tokens = self.tokens.clone();
                Box::pin(async move { Ok(tokens) })
            }
            fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
                Box::pin(async { Ok(sample_bridge_response("hook-failing-gas")) })
            }
            fn get_bridging_params<'a>(
                &'a self,
                _c: u64,
                _o: &'a cow_orderbook::types::Order,
                _t: B256,
                _s: Option<Address>,
            ) -> BridgingParamsFuture<'a> {
                Box::pin(async { Ok(None) })
            }
            fn get_explorer_url(&self, _id: &str) -> String {
                String::new()
            }
            fn get_status<'a>(&'a self, _id: &'a str, _c: u64) -> BridgeStatusFuture<'a> {
                Box::pin(async {
                    Ok(BridgeStatusResult {
                        status: BridgeStatus::Unknown,
                        fill_time_in_seconds: None,
                        deposit_tx_hash: None,
                        fill_tx_hash: None,
                    })
                })
            }
            fn as_hook_bridge_provider(&self) -> Option<&dyn HookBridgeProvider> {
                Some(self)
            }
        }
        impl HookBridgeProvider for FailingGasProvider {
            fn get_unsigned_bridge_call<'a>(
                &'a self,
                _req: &'a QuoteBridgeRequest,
                _quote: &'a QuoteBridgeResponse,
            ) -> UnsignedCallFuture<'a> {
                Box::pin(async {
                    Err(cow_errors::CowError::Api { status: 0, body: "not called".into() })
                })
            }
            fn get_gas_limit_estimation_for_hook<'a>(
                &'a self,
                _proxy_deployed: bool,
                _extra_gas: Option<u64>,
                _extra_gas_proxy_creation: Option<u64>,
            ) -> GasEstimationFuture<'a> {
                Box::pin(async {
                    Err(cow_errors::CowError::Api { status: 500, body: "gas oracle down".into() })
                })
            }
            fn get_signed_hook<'a>(
                &'a self,
                _chain_id: cow_chains::SupportedChainId,
                _unsigned_call: &'a EvmCall,
                _nonce: &'a str,
                _deadline: u64,
                _gas: u64,
                _signer: &'a alloy_signer_local::PrivateKeySigner,
            ) -> SignedHookFuture<'a> {
                Box::pin(async { Err(cow_errors::CowError::Signing("n/a".into())) })
            }
        }

        let provider = FailingGasProvider { info: hook_info(), tokens: vec![usdc()] };
        let quoter =
            FixedQuoter { outcome: sample_outcome(), captured: std::sync::OnceLock::new() };
        let err = get_quote_with_bridge(&hook_params_with_metadata(None), &provider, &quoter)
            .await
            .unwrap_err();
        if let BridgeError::TxBuildError(msg) = err {
            assert!(msg.contains("gas oracle down"), "unexpected msg: {msg}");
        } else {
            panic!("expected TxBuildError, got {err:?}");
        }
    }

    #[tokio::test]
    async fn hook_branch_propagates_unsigned_call_error() {
        /// Hook provider whose `get_unsigned_bridge_call` fails.
        struct FailingUnsignedCall {
            info: BridgeProviderInfo,
            tokens: Vec<IntermediateTokenInfo>,
        }
        impl BridgeProvider for FailingUnsignedCall {
            fn info(&self) -> &BridgeProviderInfo {
                &self.info
            }
            fn supports_route(&self, _s: u64, _b: u64) -> bool {
                true
            }
            fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
                Box::pin(async { Ok(Vec::<BridgeNetworkInfo>::new()) })
            }
            fn get_buy_tokens<'a>(&'a self, _p: BuyTokensParams) -> BuyTokensFuture<'a> {
                let info = self.info.clone();
                Box::pin(
                    async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) },
                )
            }
            fn get_intermediate_tokens<'a>(
                &'a self,
                _req: &'a QuoteBridgeRequest,
            ) -> IntermediateTokensFuture<'a> {
                let tokens = self.tokens.clone();
                Box::pin(async move { Ok(tokens) })
            }
            fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
                Box::pin(async { Ok(sample_bridge_response("hook-bad-calldata")) })
            }
            fn get_bridging_params<'a>(
                &'a self,
                _c: u64,
                _o: &'a cow_orderbook::types::Order,
                _t: B256,
                _s: Option<Address>,
            ) -> BridgingParamsFuture<'a> {
                Box::pin(async { Ok(None) })
            }
            fn get_explorer_url(&self, _id: &str) -> String {
                String::new()
            }
            fn get_status<'a>(&'a self, _id: &'a str, _c: u64) -> BridgeStatusFuture<'a> {
                Box::pin(async {
                    Ok(BridgeStatusResult {
                        status: BridgeStatus::Unknown,
                        fill_time_in_seconds: None,
                        deposit_tx_hash: None,
                        fill_tx_hash: None,
                    })
                })
            }
            fn as_hook_bridge_provider(&self) -> Option<&dyn HookBridgeProvider> {
                Some(self)
            }
        }
        impl HookBridgeProvider for FailingUnsignedCall {
            fn get_unsigned_bridge_call<'a>(
                &'a self,
                _req: &'a QuoteBridgeRequest,
                _quote: &'a QuoteBridgeResponse,
            ) -> UnsignedCallFuture<'a> {
                Box::pin(async {
                    Err(cow_errors::CowError::Api { status: 0, body: "bad calldata".into() })
                })
            }
            fn get_gas_limit_estimation_for_hook<'a>(
                &'a self,
                _proxy_deployed: bool,
                _extra_gas: Option<u64>,
                _extra_gas_proxy_creation: Option<u64>,
            ) -> GasEstimationFuture<'a> {
                Box::pin(async move { Ok(500_000u64) })
            }
            fn get_signed_hook<'a>(
                &'a self,
                _chain_id: cow_chains::SupportedChainId,
                _unsigned_call: &'a EvmCall,
                _nonce: &'a str,
                _deadline: u64,
                _gas: u64,
                _signer: &'a alloy_signer_local::PrivateKeySigner,
            ) -> SignedHookFuture<'a> {
                Box::pin(async { Err(cow_errors::CowError::Signing("n/a".into())) })
            }
        }

        let provider = FailingUnsignedCall { info: hook_info(), tokens: vec![usdc()] };
        let quoter =
            FixedQuoter { outcome: sample_outcome(), captured: std::sync::OnceLock::new() };
        let err = get_quote_with_bridge(&hook_params_with_metadata(None), &provider, &quoter)
            .await
            .unwrap_err();
        if let BridgeError::TxBuildError(msg) = err {
            assert!(msg.contains("bad calldata"), "unexpected msg: {msg}");
        } else {
            panic!("expected TxBuildError, got {err:?}");
        }
    }

    // ── Receiver branch error paths ──────────────────────────────────────

    #[tokio::test]
    async fn receiver_branch_propagates_override_error() {
        /// Receiver-account provider whose `get_bridge_receiver_override` fails.
        struct FailingReceiverOverride {
            info: BridgeProviderInfo,
            tokens: Vec<IntermediateTokenInfo>,
        }
        impl BridgeProvider for FailingReceiverOverride {
            fn info(&self) -> &BridgeProviderInfo {
                &self.info
            }
            fn supports_route(&self, _s: u64, _b: u64) -> bool {
                true
            }
            fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
                Box::pin(async { Ok(Vec::<BridgeNetworkInfo>::new()) })
            }
            fn get_buy_tokens<'a>(&'a self, _p: BuyTokensParams) -> BuyTokensFuture<'a> {
                let info = self.info.clone();
                Box::pin(
                    async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) },
                )
            }
            fn get_intermediate_tokens<'a>(
                &'a self,
                _req: &'a QuoteBridgeRequest,
            ) -> IntermediateTokensFuture<'a> {
                let tokens = self.tokens.clone();
                Box::pin(async move { Ok(tokens) })
            }
            fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
                Box::pin(async { Ok(sample_bridge_response("receiver-failing-override")) })
            }
            fn get_bridging_params<'a>(
                &'a self,
                _c: u64,
                _o: &'a cow_orderbook::types::Order,
                _t: B256,
                _s: Option<Address>,
            ) -> BridgingParamsFuture<'a> {
                Box::pin(async { Ok(None) })
            }
            fn get_explorer_url(&self, _id: &str) -> String {
                String::new()
            }
            fn get_status<'a>(&'a self, _id: &'a str, _c: u64) -> BridgeStatusFuture<'a> {
                Box::pin(async {
                    Ok(BridgeStatusResult {
                        status: BridgeStatus::Unknown,
                        fill_time_in_seconds: None,
                        deposit_tx_hash: None,
                        fill_tx_hash: None,
                    })
                })
            }
            fn as_receiver_account_bridge_provider(
                &self,
            ) -> Option<&dyn ReceiverAccountBridgeProvider> {
                Some(self)
            }
        }
        impl ReceiverAccountBridgeProvider for FailingReceiverOverride {
            fn get_bridge_receiver_override<'a>(
                &'a self,
                _quote_request: &'a QuoteBridgeRequest,
                _quote_result: &'a QuoteBridgeResponse,
            ) -> ReceiverOverrideFuture<'a> {
                Box::pin(async {
                    Err(cow_errors::CowError::Api {
                        status: 0,
                        body: "no deposit addr available".into(),
                    })
                })
            }
        }

        let provider = FailingReceiverOverride { info: receiver_info(), tokens: vec![usdc()] };
        let quoter =
            FixedQuoter { outcome: sample_outcome(), captured: std::sync::OnceLock::new() };
        let err = get_quote_with_bridge(&hook_params_with_metadata(None), &provider, &quoter)
            .await
            .unwrap_err();
        if let BridgeError::TxBuildError(msg) = err {
            assert!(msg.contains("no deposit addr available"), "unexpected msg: {msg}");
        } else {
            panic!("expected TxBuildError, got {err:?}");
        }
    }

    // ── Hook branch — shape checks ────────────────────────────────────────

    #[tokio::test]
    async fn hook_branch_bridge_call_details_carry_unsigned_call_bytes() {
        let provider = MockHookProvider {
            info: hook_info(),
            tokens: vec![usdc()],
            bridge_response: sample_bridge_response("mock-hook"),
            unsigned_call: build_unsigned_call(),
            gas_limit: 500_000,
        };
        let quoter =
            FixedQuoter { outcome: sample_outcome(), captured: std::sync::OnceLock::new() };
        let result =
            get_quote_with_hook_bridge(&provider, &hook_params_with_metadata(None), &quoter)
                .await
                .unwrap();
        let details =
            result.bridge.bridge_call_details.expect("hook branch populates call_details");
        assert_eq!(details.unsigned_bridge_call.data, vec![0xde, 0xad]);
        assert_eq!(details.unsigned_bridge_call.to, Address::repeat_byte(0xAC),);
        // The pre-authorized hook uses the mocked post-hook (PR #7 leaves
        // the real signing for PR #8).
        assert_eq!(details.pre_authorized_bridging_hook.post_hook.gas_limit, "500000",);
    }

    // ── Receiver branch — shape checks ───────────────────────────────────

    #[tokio::test]
    async fn receiver_branch_sets_override_and_clears_call_details() {
        let provider = MockReceiverProvider {
            info: receiver_info(),
            tokens: vec![usdc()],
            bridge_response: sample_bridge_response("mock-receiver"),
            deposit_address: "TOPsolanaDepositAddrXXXXXXXXXXXXXXXXXXXXXXX".into(),
        };
        let quoter =
            FixedQuoter { outcome: sample_outcome(), captured: std::sync::OnceLock::new() };
        let result = get_quote_with_receiver_account_bridge(
            &provider,
            &hook_params_with_metadata(None),
            &quoter,
        )
        .await
        .unwrap();
        assert!(result.bridge.bridge_call_details.is_none());
        assert_eq!(
            result.bridge.bridge_receiver_override.as_deref(),
            Some("TOPsolanaDepositAddrXXXXXXXXXXXXXXXXXXXXXXX"),
        );
    }

    // ── minimal_bridge_quote_result ──────────────────────────────────────

    #[test]
    fn minimal_bridge_quote_result_wraps_response_amounts() {
        let req = sample_request(OrderKind::Sell);
        let resp = sample_bridge_response("arb");
        let quote = minimal_bridge_quote_result(&req, &resp);
        assert!(quote.is_sell);
        assert_eq!(quote.amounts_and_costs.after_fee.buy_amount, resp.buy_amount);
        // before_fee.buy_amount must equal buy_amount + fee.
        assert_eq!(
            quote.amounts_and_costs.before_fee.buy_amount,
            resp.buy_amount.saturating_add(resp.fee_amount),
        );
        assert_eq!(quote.fees.bridge_fee, resp.fee_amount);
        assert_eq!(quote.expected_fill_time_seconds, Some(resp.estimated_secs));
    }

    #[test]
    fn minimal_bridge_quote_result_flags_buy_orders_as_non_sell() {
        let req = sample_request(OrderKind::Buy);
        let resp = sample_bridge_response("arb");
        let quote = minimal_bridge_quote_result(&req, &resp);
        assert!(!quote.is_sell);
    }

    // ── get_bridge_signed_hook ───────────────────────────────────────────

    /// Hook provider that captures calls to `get_signed_hook` so the
    /// test can inspect the derived nonce / deadline / gas limit.
    struct SigningCaptureProvider {
        info: BridgeProviderInfo,
        tokens: Vec<IntermediateTokenInfo>,
        bridge_response: QuoteBridgeResponse,
        unsigned_call: EvmCall,
        captured_nonce: std::sync::OnceLock<String>,
        captured_deadline: std::sync::OnceLock<u64>,
        captured_gas: std::sync::OnceLock<u64>,
    }

    impl BridgeProvider for SigningCaptureProvider {
        fn info(&self) -> &BridgeProviderInfo {
            &self.info
        }
        fn supports_route(&self, _s: u64, _b: u64) -> bool {
            true
        }
        fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
            Box::pin(async { Ok(Vec::<BridgeNetworkInfo>::new()) })
        }
        fn get_buy_tokens<'a>(&'a self, _p: BuyTokensParams) -> BuyTokensFuture<'a> {
            let info = self.info.clone();
            Box::pin(
                async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) },
            )
        }
        fn get_intermediate_tokens<'a>(
            &'a self,
            _req: &'a QuoteBridgeRequest,
        ) -> IntermediateTokensFuture<'a> {
            let tokens = self.tokens.clone();
            Box::pin(async move { Ok(tokens) })
        }
        fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
            let resp = self.bridge_response.clone();
            Box::pin(async move { Ok(resp) })
        }
        fn get_bridging_params<'a>(
            &'a self,
            _c: u64,
            _o: &'a cow_orderbook::types::Order,
            _t: B256,
            _s: Option<Address>,
        ) -> BridgingParamsFuture<'a> {
            Box::pin(async { Ok(None) })
        }
        fn get_explorer_url(&self, _id: &str) -> String {
            String::new()
        }
        fn get_status<'a>(&'a self, _id: &'a str, _c: u64) -> BridgeStatusFuture<'a> {
            Box::pin(async {
                Ok(BridgeStatusResult {
                    status: BridgeStatus::Unknown,
                    fill_time_in_seconds: None,
                    deposit_tx_hash: None,
                    fill_tx_hash: None,
                })
            })
        }
        fn as_hook_bridge_provider(&self) -> Option<&dyn HookBridgeProvider> {
            Some(self)
        }
    }

    impl HookBridgeProvider for SigningCaptureProvider {
        fn get_unsigned_bridge_call<'a>(
            &'a self,
            _req: &'a QuoteBridgeRequest,
            _quote: &'a QuoteBridgeResponse,
        ) -> UnsignedCallFuture<'a> {
            let call = self.unsigned_call.clone();
            Box::pin(async move { Ok(call) })
        }
        fn get_gas_limit_estimation_for_hook<'a>(
            &'a self,
            _proxy_deployed: bool,
            _extra_gas: Option<u64>,
            _extra_gas_proxy_creation: Option<u64>,
        ) -> GasEstimationFuture<'a> {
            Box::pin(async move { Ok(500_000u64) })
        }
        fn get_signed_hook<'a>(
            &'a self,
            _chain_id: cow_chains::SupportedChainId,
            _unsigned_call: &'a EvmCall,
            nonce: &'a str,
            deadline: u64,
            hook_gas_limit: u64,
            _signer: &'a alloy_signer_local::PrivateKeySigner,
        ) -> SignedHookFuture<'a> {
            self.captured_nonce.set(nonce.to_owned()).ok();
            self.captured_deadline.set(deadline).ok();
            self.captured_gas.set(hook_gas_limit).ok();
            Box::pin(async {
                Ok(BridgeHook {
                    post_hook: crate::utils::hook_mock_for_cost_estimation(500_000),
                    recipient: "0x0000000000000000000000000000000000000001".into(),
                })
            })
        }
    }

    fn make_signer() -> alloy_signer_local::PrivateKeySigner {
        use std::str::FromStr;
        alloy_signer_local::PrivateKeySigner::from_str(
            "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        )
        .unwrap()
    }

    #[tokio::test]
    async fn get_bridge_signed_hook_threads_context_into_provider() {
        let provider = SigningCaptureProvider {
            info: hook_info(),
            tokens: vec![usdc()],
            bridge_response: sample_bridge_response("sig-capture"),
            unsigned_call: build_unsigned_call(),
            captured_nonce: std::sync::OnceLock::new(),
            captured_deadline: std::sync::OnceLock::new(),
            captured_gas: std::sync::OnceLock::new(),
        };
        let signer = make_signer();
        let ctx = GetBridgeSignedHookContext {
            signer: &signer,
            hook_gas_limit: 123_456,
            chain_id: cow_chains::SupportedChainId::Mainnet,
            deadline: 9_999_999,
        };
        let out =
            get_bridge_signed_hook(&provider, &sample_request(OrderKind::Sell), ctx).await.unwrap();
        // Gas + deadline must match what we threaded in.
        assert_eq!(*provider.captured_gas.get().unwrap(), 123_456);
        assert_eq!(*provider.captured_deadline.get().unwrap(), 9_999_999);
        // The nonce is keccak256(data || deadline_be) — deterministic.
        let expected = derive_hook_nonce(&out.unsigned_bridge_call.data, 9_999_999);
        assert_eq!(provider.captured_nonce.get().unwrap(), &expected);
        assert_eq!(out.bridging_quote.provider, "sig-capture");
    }

    #[test]
    fn derive_hook_nonce_is_deterministic() {
        let data = vec![0xde, 0xad, 0xbe, 0xef];
        let a = derive_hook_nonce(&data, 42);
        let b = derive_hook_nonce(&data, 42);
        assert_eq!(a, b);
        assert!(a.starts_with("0x"));
        assert_eq!(a.len(), 2 + 64); // "0x" + 32 bytes hex
    }

    #[test]
    fn derive_hook_nonce_changes_with_deadline() {
        let data = vec![0x01, 0x02];
        let a = derive_hook_nonce(&data, 42);
        let b = derive_hook_nonce(&data, 43);
        assert_ne!(a, b);
    }

    #[test]
    fn derive_hook_nonce_changes_with_data() {
        let a = derive_hook_nonce(&[0x01], 42);
        let b = derive_hook_nonce(&[0x02], 42);
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn get_bridge_signed_hook_propagates_quote_error() {
        /// Provider whose `get_quote` fails.
        struct QuoteFailing {
            info: BridgeProviderInfo,
        }
        impl BridgeProvider for QuoteFailing {
            fn info(&self) -> &BridgeProviderInfo {
                &self.info
            }
            fn supports_route(&self, _s: u64, _b: u64) -> bool {
                true
            }
            fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
                Box::pin(async { Ok(Vec::new()) })
            }
            fn get_buy_tokens<'a>(&'a self, _p: BuyTokensParams) -> BuyTokensFuture<'a> {
                let info = self.info.clone();
                Box::pin(
                    async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) },
                )
            }
            fn get_intermediate_tokens<'a>(
                &'a self,
                _req: &'a QuoteBridgeRequest,
            ) -> IntermediateTokensFuture<'a> {
                Box::pin(async { Ok(Vec::new()) })
            }
            fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
                Box::pin(async {
                    Err(cow_errors::CowError::Api { status: 500, body: "nope".into() })
                })
            }
            fn get_bridging_params<'a>(
                &'a self,
                _c: u64,
                _o: &'a cow_orderbook::types::Order,
                _t: B256,
                _s: Option<Address>,
            ) -> BridgingParamsFuture<'a> {
                Box::pin(async { Ok(None) })
            }
            fn get_explorer_url(&self, _id: &str) -> String {
                String::new()
            }
            fn get_status<'a>(&'a self, _id: &'a str, _c: u64) -> BridgeStatusFuture<'a> {
                Box::pin(async {
                    Ok(BridgeStatusResult {
                        status: BridgeStatus::Unknown,
                        fill_time_in_seconds: None,
                        deposit_tx_hash: None,
                        fill_tx_hash: None,
                    })
                })
            }
            fn as_hook_bridge_provider(&self) -> Option<&dyn HookBridgeProvider> {
                Some(self)
            }
        }
        impl HookBridgeProvider for QuoteFailing {
            fn get_unsigned_bridge_call<'a>(
                &'a self,
                _req: &'a QuoteBridgeRequest,
                _quote: &'a QuoteBridgeResponse,
            ) -> UnsignedCallFuture<'a> {
                Box::pin(async { Err(cow_errors::CowError::Signing("n/a".into())) })
            }
            fn get_gas_limit_estimation_for_hook<'a>(
                &'a self,
                _proxy_deployed: bool,
                _extra_gas: Option<u64>,
                _extra_gas_proxy_creation: Option<u64>,
            ) -> GasEstimationFuture<'a> {
                Box::pin(async move { Ok(500_000u64) })
            }
            fn get_signed_hook<'a>(
                &'a self,
                _chain_id: cow_chains::SupportedChainId,
                _unsigned_call: &'a EvmCall,
                _nonce: &'a str,
                _deadline: u64,
                _gas: u64,
                _signer: &'a alloy_signer_local::PrivateKeySigner,
            ) -> SignedHookFuture<'a> {
                Box::pin(async { Err(cow_errors::CowError::Signing("n/a".into())) })
            }
        }

        let provider = QuoteFailing { info: hook_info() };
        let signer = make_signer();
        let err = get_bridge_signed_hook(
            &provider,
            &sample_request(OrderKind::Sell),
            GetBridgeSignedHookContext {
                signer: &signer,
                hook_gas_limit: 1_000,
                chain_id: cow_chains::SupportedChainId::Mainnet,
                deadline: 1_234,
            },
        )
        .await
        .unwrap_err();
        if let BridgeError::TxBuildError(msg) = err {
            assert!(msg.contains("nope"), "unexpected: {msg}");
        } else {
            panic!("expected TxBuildError, got {err:?}");
        }
    }

    // ── get_quote_with_hook_bridge with signer ───────────────────────────

    #[tokio::test]
    async fn hook_branch_produces_real_hook_when_signer_provided() {
        let provider = SigningCaptureProvider {
            info: hook_info(),
            tokens: vec![usdc()],
            bridge_response: sample_bridge_response("with-signer"),
            unsigned_call: build_unsigned_call(),
            captured_nonce: std::sync::OnceLock::new(),
            captured_deadline: std::sync::OnceLock::new(),
            captured_gas: std::sync::OnceLock::new(),
        };
        let quoter =
            FixedQuoter { outcome: sample_outcome(), captured: std::sync::OnceLock::new() };
        let signer = std::sync::Arc::new(make_signer());
        let params = GetQuoteWithBridgeParams {
            swap_and_bridge_request: sample_request(OrderKind::Sell),
            slippage_bps: 50,
            advanced_settings_metadata: None,
            quote_signer: Some(std::sync::Arc::clone(&signer)),
            hook_deadline: Some(5_000_000),
        };

        get_quote_with_hook_bridge(&provider, &params, &quoter).await.unwrap();

        // The signer path must have threaded the caller's deadline
        // into get_signed_hook.
        assert_eq!(*provider.captured_deadline.get().unwrap(), 5_000_000);
    }

    #[tokio::test]
    async fn hook_branch_defaults_deadline_to_u32_max_when_unset() {
        let provider = SigningCaptureProvider {
            info: hook_info(),
            tokens: vec![usdc()],
            bridge_response: sample_bridge_response("default-deadline"),
            unsigned_call: build_unsigned_call(),
            captured_nonce: std::sync::OnceLock::new(),
            captured_deadline: std::sync::OnceLock::new(),
            captured_gas: std::sync::OnceLock::new(),
        };
        let quoter =
            FixedQuoter { outcome: sample_outcome(), captured: std::sync::OnceLock::new() };
        let signer = std::sync::Arc::new(make_signer());
        let params = GetQuoteWithBridgeParams {
            swap_and_bridge_request: sample_request(OrderKind::Sell),
            slippage_bps: 50,
            advanced_settings_metadata: None,
            quote_signer: Some(std::sync::Arc::clone(&signer)),
            hook_deadline: None,
        };

        get_quote_with_hook_bridge(&provider, &params, &quoter).await.unwrap();
        assert_eq!(*provider.captured_deadline.get().unwrap(), u64::from(u32::MAX));
    }
}
