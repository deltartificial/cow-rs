//! Abstraction over the orderbook HTTP API for dependency injection.
//!
//! [`OrderbookClient`] is the trait implemented by [`OrderBookApi`]; tests
//! (or alternative backends) can provide a mock implementation that
//! returns canned responses without network I/O.

use cow_errors::CowError;

use crate::{
    OrderBookApi,
    types::{
        Order, OrderCancellations, OrderCreation, OrderQuoteRequest, OrderQuoteResponse, Trade,
    },
};

/// Abstraction over the `CoW` Protocol orderbook HTTP API.
///
/// [`OrderBookApi`] implements this trait by delegating to its existing async
/// methods. Consumers (notably `cow-trading`) accept an
/// `Arc<dyn OrderbookClient>` so tests can inject mocks.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait OrderbookClient: Send + Sync {
    /// Obtain a price quote for an order.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the quote request fails or is rejected.
    async fn get_quote(&self, request: &OrderQuoteRequest) -> Result<OrderQuoteResponse, CowError>;

    /// Submit a signed order and return the assigned order UID.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the order is rejected or the request fails.
    async fn send_order(&self, creation: &OrderCreation) -> Result<String, CowError>;

    /// Fetch an order by its unique identifier.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the order is not found or the request fails.
    async fn get_order(&self, order_uid: &str) -> Result<Order, CowError>;

    /// List trades for a given order UID.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the request fails.
    async fn get_trades(&self, order_uid: &str) -> Result<Vec<Trade>, CowError>;

    /// Cancel one or more orders (best-effort off-chain cancellation).
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the cancellation is rejected or the request fails.
    async fn cancel_orders(&self, cancellation: &OrderCancellations) -> Result<(), CowError>;
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[allow(clippy::use_self, reason = "fully qualified calls needed to avoid infinite recursion")]
impl OrderbookClient for OrderBookApi {
    async fn get_quote(&self, request: &OrderQuoteRequest) -> Result<OrderQuoteResponse, CowError> {
        OrderBookApi::get_quote(self, request).await
    }

    async fn send_order(&self, creation: &OrderCreation) -> Result<String, CowError> {
        OrderBookApi::send_order(self, creation).await
    }

    async fn get_order(&self, order_uid: &str) -> Result<Order, CowError> {
        OrderBookApi::get_order(self, order_uid).await
    }

    async fn get_trades(&self, order_uid: &str) -> Result<Vec<Trade>, CowError> {
        OrderBookApi::get_trades(self, Some(order_uid), None).await
    }

    async fn cancel_orders(&self, cancellation: &OrderCancellations) -> Result<(), CowError> {
        OrderBookApi::cancel_orders(self, cancellation).await
    }
}
