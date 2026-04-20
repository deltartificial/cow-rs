//! [`SwapQuoter`] — trait abstraction letting the bridging layer request
//! an intermediate swap quote without depending on `cow-trading` (both
//! crates live on workspace layer 5 and cannot depend on each other).
//!
//! The canonical implementor is `cow_trading::TradingSdk`; an
//! `impl SwapQuoter for TradingSdk` lives in the `cow-rs` façade crate
//! where both sides are reachable. Tests in this crate use in-module
//! fakes.

use alloy_primitives::{Address, U256};
use cow_errors::CowError;
use cow_types::OrderKind;

use crate::provider::MaybeSendSync;

/// Minimal swap-quote request passed from the bridging orchestrator to
/// a [`SwapQuoter`]. Mirrors the subset of `TradeParameters` the
/// bridging layer needs for the intermediate hop.
#[derive(Debug, Clone)]
pub struct SwapQuoteParams {
    /// Account paying for the swap.
    pub owner: Address,
    /// Chain where the swap happens (source chain in a bridge flow).
    pub chain_id: u64,
    /// Sell-side token.
    pub sell_token: Address,
    /// Decimals of `sell_token`.
    pub sell_token_decimals: u8,
    /// Buy-side token (the bridge intermediate token).
    pub buy_token: Address,
    /// Decimals of `buy_token`.
    pub buy_token_decimals: u8,
    /// Amount atoms (sell for `Sell`, buy for `Buy`).
    pub amount: U256,
    /// Order kind — bridging only supports `Sell` today, but the param
    /// is passed through so the quoter can reject non-sell bundles.
    pub kind: OrderKind,
    /// Slippage tolerance in basis points.
    pub slippage_bps: u32,
    /// Pre-built, caller-supplied `appData` JSON to merge into the
    /// eventual app-data document. `None` means "use the quoter's
    /// default metadata only".
    pub app_data_json: Option<String>,
}

/// Minimal swap-quote response returned by a [`SwapQuoter`]. Carries
/// just enough to build the outer
/// [`QuoteBridgeResponse`](crate::types::QuoteBridgeResponse) and upstream
/// app-data attribution.
#[derive(Debug, Clone)]
pub struct SwapQuoteOutcome {
    /// Sell amount as quoted by the orderbook (atoms).
    pub sell_amount: U256,
    /// Buy amount after slippage — corresponds to the TS SDK's
    /// `afterSlippage.buyAmount`, which becomes the intermediate-token
    /// amount passed on to the bridge.
    pub buy_amount_after_slippage: U256,
    /// Network fee (sell-token atoms) baked into the quote.
    pub fee_amount: U256,
    /// Expected validity deadline as a UNIX timestamp.
    pub valid_to: u32,
    /// `0x…` app-data hash.
    pub app_data_hex: String,
    /// Full app-data JSON document.
    pub full_app_data: String,
}

/// Trait implemented by any "thing that can quote an intermediate swap".
///
/// The default implementor in the cow-rs façade crate wraps
/// `cow_trading::TradingSdk::get_quote_only` / `get_quote_only_with_settings`
/// (added in cow-rs v0.2.0 for exactly this kind of flow).
///
/// # Async & object-safety
///
/// Returns a boxed future so the trait stays object-safe without
/// `async_trait`. On native targets the future is `Send`; on WASM
/// it's `!Send` because the browser `fetch` API is single-threaded.
pub trait SwapQuoter: MaybeSendSync {
    /// Fetch an intermediate-swap quote.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the underlying orderbook call fails.
    fn quote_swap<'a>(&'a self, params: SwapQuoteParams) -> QuoteSwapFuture<'a>;
}

#[cfg(not(target_arch = "wasm32"))]
/// Future returned by [`SwapQuoter::quote_swap`].
pub type QuoteSwapFuture<'a> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<SwapQuoteOutcome, CowError>> + Send + 'a>,
>;

#[cfg(target_arch = "wasm32")]
/// Future returned by [`SwapQuoter::quote_swap`].
pub type QuoteSwapFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<SwapQuoteOutcome, CowError>> + 'a>>;
