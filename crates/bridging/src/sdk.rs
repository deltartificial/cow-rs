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

/// Create a signed bridge hook from a bridge quote.
///
/// In the `TypeScript` SDK this calls `provider.getQuote`, `provider.getUnsignedBridgeCall`,
/// and `provider.getSignedHook` using the signer. This requires a full `HookBridgeProvider`
/// implementation and a signer.
///
/// This is a stub — the full implementation requires weiroll script construction
/// and `CowShedSdk` signing infrastructure that is not yet ported.
///
/// # Errors
///
/// Always returns [`BridgeError::TxBuildError`] until the signing infrastructure
/// is ported.
pub async fn get_bridge_signed_hook(
    _quote: &BridgeQuoteResult,
    _signer: &[u8],
) -> Result<BridgeHook, BridgeError> {
    // TODO: Requires CowShedSdk signing + weiroll delegate-call script generation.
    // The TS implementation:
    //   1. Gets a bridge quote from the provider
    //   2. Gets the unsigned bridge call from the provider
    //   3. Computes a nonce via keccak256(calldata || deadline)
    //   4. Calls provider.getSignedHook with the nonce, deadline, and signer
    Err(BridgeError::TxBuildError(
        "get_bridge_signed_hook requires CowShedSdk signing infrastructure (not yet ported)"
            .to_owned(),
    ))
}

/// Parameters for [`get_quote_with_bridge`].
#[derive(Debug, Clone)]
pub struct GetQuoteWithBridgeParams {
    /// The swap-and-bridge request.
    pub swap_and_bridge_request: QuoteBridgeRequest,
    /// Slippage tolerance in basis points for the swap leg.
    pub slippage_bps: u32,
}

/// Get a quote that includes bridging (cross-chain).
///
/// In the `TypeScript` SDK, this orchestrates:
/// 1. Determine the intermediate token
/// 2. Get a swap quote (sell token → intermediate token)
/// 3. Get a bridge quote (intermediate token on source → buy token on dest)
/// 4. Optionally sign hooks via `CowShedSdk`
///
/// This stub documents the required flow. Full implementation requires
/// `TradingSdk` and `BridgeProvider` orchestration.
///
/// # Errors
///
/// Always returns [`BridgeError::TxBuildError`] until the trading SDK is ported.
pub async fn get_quote_with_bridge(
    _params: &GetQuoteWithBridgeParams,
) -> Result<BridgeQuoteAndPost, BridgeError> {
    // TODO: Requires TradingSdk for swap quoting and BridgeProvider for bridge quoting.
    // Flow:
    //   1. Validate kind == Sell
    //   2. Get intermediate tokens from provider
    //   3. Determine best intermediate token (determine_intermediate_token)
    //   4. Get swap quote via TradingSdk
    //   5. Get bridge quote from provider
    //   6. For hook providers: sign the hook via CowShedSdk
    //   7. For receiver-account providers: set receiver override
    //   8. Return BridgeQuoteAndPost
    Err(BridgeError::TxBuildError(
        "get_quote_with_bridge requires TradingSdk orchestration (not yet ported)".to_owned(),
    ))
}

/// Get a quote without bridging (same-chain swap).
///
/// In the `TypeScript` SDK, this delegates directly to `TradingSdk.getQuote`.
/// The Rust version is a stub that documents the required parameters.
///
/// # Errors
///
/// Always returns [`BridgeError::TxBuildError`] until the trading SDK is ported.
pub async fn get_quote_without_bridge(
    _request: &QuoteBridgeRequest,
) -> Result<QuoteAndPost, BridgeError> {
    // TODO: Requires TradingSdk.getQuote delegation.
    // Flow:
    //   1. Map QuoteBridgeRequest fields to TradeParameters
    //   2. Call tradingSdk.getQuote(swapParams, advancedSettings)
    //   3. Return QuoteAndPost wrapping the result
    Err(BridgeError::TxBuildError(
        "get_quote_without_bridge requires TradingSdk (not yet ported)".to_owned(),
    ))
}

/// Get a swap quote from the order book.
///
/// In the `TypeScript` SDK, this calls `TradingSdk.getQuoteResults` with
/// the intermediate token as the buy token.
///
/// # Errors
///
/// Always returns [`BridgeError::TxBuildError`] until the trading SDK is ported.
pub async fn get_swap_quote(
    _request: &QuoteBridgeRequest,
) -> Result<QuoteBridgeResponse, BridgeError> {
    // TODO: Requires TradingSdk.getQuoteResults.
    // Flow:
    //   1. Build swap params (sellToken → intermediateToken, on sellChain)
    //   2. Call tradingSdk.getQuoteResults
    //   3. Return the swap result with intermediate token amounts
    Err(BridgeError::TxBuildError("get_swap_quote requires TradingSdk (not yet ported)".to_owned()))
}

/// Build an order from a completed bridge quote.
///
/// In the `TypeScript` SDK, this is `createPostSwapOrderFromQuote` which returns
/// a closure that re-fetches the bridge quote with the real signer, then posts
/// via `postSwapOrderFromQuoteTrading`.
///
/// # Errors
///
/// Always returns [`BridgeError::TxBuildError`] until the trading/signing SDKs
/// are ported.
pub async fn create_post_swap_order_from_quote(
    _quote: &BridgeQuoteAndPost,
) -> Result<(), BridgeError> {
    // TODO: Requires TradingSdk + OrderBookApi + signer.
    // Flow:
    //   1. Optionally re-fetch bridge quote with real signer
    //   2. Update trade parameters (receiver, appData)
    //   3. Call postSwapOrderFromQuoteTrading
    Err(BridgeError::TxBuildError(
        "create_post_swap_order_from_quote requires TradingSdk + OrderBookApi (not yet ported)"
            .to_owned(),
    ))
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

/// Get a quote for a hook-based bridge.
///
/// For providers that use a post-swap hook (e.g. Across, Bungee), this
/// function orchestrates the intermediate swap + bridge hook flow.
///
/// Mirrors `getQuoteWithHookBridge` from `getQuoteWithBridge.ts`.
///
/// Currently delegates to the existing [`get_quote_with_bridge`] stub since
/// both hook-based and receiver-account providers follow the same top-level
/// quote path.
///
/// # Errors
///
/// Returns [`BridgeError::TxBuildError`] until the full orchestration is ported.
pub async fn get_quote_with_hook_bridge(
    params: &GetQuoteWithBridgeParams,
) -> Result<BridgeQuoteAndPost, BridgeError> {
    get_quote_with_bridge(params).await
}

/// Get a quote for a receiver-account-based bridge.
///
/// For providers that send tokens to a specific deposit address (receiver
/// override), this function sets the swap receiver to the deposit account.
///
/// Mirrors `getQuoteWithReceiverAccountBridge` from `getQuoteWithBridge.ts`.
///
/// Currently delegates to the existing [`get_quote_with_bridge`] stub since
/// both hook-based and receiver-account providers follow the same top-level
/// quote path.
///
/// # Errors
///
/// Returns [`BridgeError::TxBuildError`] until the full orchestration is ported.
pub async fn get_quote_with_receiver_account_bridge(
    params: &GetQuoteWithBridgeParams,
) -> Result<BridgeQuoteAndPost, BridgeError> {
    get_quote_with_bridge(params).await
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
