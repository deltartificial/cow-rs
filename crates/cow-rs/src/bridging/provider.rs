//! [`BridgeProvider`] trait for bridge integrations.

use std::pin::Pin;

use crate::CowError;

use super::types::{QuoteBridgeRequest, QuoteBridgeResponse};

/// Boxed future returned by [`BridgeProvider::get_quote`].
///
/// On native targets the future is `Send`; on WASM targets it is not, because
/// the browser `fetch` API is single-threaded.
#[cfg(not(target_arch = "wasm32"))]
pub type QuoteFuture<'a> =
    Pin<Box<dyn std::future::Future<Output = Result<QuoteBridgeResponse, CowError>> + Send + 'a>>;

/// Boxed future returned by [`BridgeProvider::get_quote`].
///
/// On WASM targets the future is not `Send` because the browser `fetch` API
/// is single-threaded.
#[cfg(target_arch = "wasm32")]
pub type QuoteFuture<'a> =
    Pin<Box<dyn std::future::Future<Output = Result<QuoteBridgeResponse, CowError>> + 'a>>;

/// Trait implemented by cross-chain bridge providers (Bungee, Li.Fi, etc.).
#[cfg(not(target_arch = "wasm32"))]
pub trait BridgeProvider: Send + Sync {
    /// A short identifier for this provider (e.g. `"bungee"`).
    ///
    /// # Returns
    ///
    /// A string slice with the provider's name, used for logging and
    /// provider selection.
    fn name(&self) -> &str;

    /// Returns `true` if this provider supports the given route.
    ///
    /// # Arguments
    ///
    /// * `sell_chain` - The chain ID of the source (sell) chain.
    /// * `buy_chain` - The chain ID of the destination (buy) chain.
    ///
    /// # Returns
    ///
    /// `true` if this provider can bridge assets from `sell_chain` to
    /// `buy_chain`, `false` otherwise.
    fn supports_route(&self, sell_chain: u64, buy_chain: u64) -> bool;

    /// Fetch a bridge quote for `req`.
    ///
    /// # Arguments
    ///
    /// * `req` - The quote request containing source/destination chains, token addresses, and the
    ///   amount to bridge.
    ///
    /// # Returns
    ///
    /// A pinned, boxed future that resolves to a [`QuoteBridgeResponse`] on
    /// success, or a [`CowError`] if the provider is unreachable or the
    /// route is unsupported.
    fn get_quote<'a>(&'a self, req: &'a QuoteBridgeRequest) -> QuoteFuture<'a>;
}

/// Trait implemented by cross-chain bridge providers (Bungee, Li.Fi, etc.).
#[cfg(target_arch = "wasm32")]
pub trait BridgeProvider {
    /// A short identifier for this provider (e.g. `"bungee"`).
    ///
    /// # Returns
    ///
    /// A string slice with the provider's name, used for logging and
    /// provider selection.
    fn name(&self) -> &str;

    /// Returns `true` if this provider supports the given route.
    ///
    /// # Arguments
    ///
    /// * `sell_chain` - The chain ID of the source (sell) chain.
    /// * `buy_chain` - The chain ID of the destination (buy) chain.
    ///
    /// # Returns
    ///
    /// `true` if this provider can bridge assets from `sell_chain` to
    /// `buy_chain`, `false` otherwise.
    fn supports_route(&self, sell_chain: u64, buy_chain: u64) -> bool;

    /// Fetch a bridge quote for `req`.
    ///
    /// # Arguments
    ///
    /// * `req` - The quote request containing source/destination chains, token addresses, and the
    ///   amount to bridge.
    ///
    /// # Returns
    ///
    /// A pinned, boxed future that resolves to a [`QuoteBridgeResponse`] on
    /// success, or a [`CowError`] if the provider is unreachable or the
    /// route is unsupported.
    fn get_quote<'a>(&'a self, req: &'a QuoteBridgeRequest) -> QuoteFuture<'a>;
}
