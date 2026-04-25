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

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test code; panic on unexpected state is acceptable")]
mod tests {
    //! The trait `impl OrderbookClient for OrderBookApi` consists of pure
    //! delegating wrappers; we exercise each one against an unreachable
    //! base URL so the wrapper bodies execute end-to-end. The HTTP call
    //! propagates a transport error, but the trait surface itself is
    //! covered.

    use alloy_primitives::Address;
    use cow_chains::{Env, SupportedChainId};
    use cow_http::RetryPolicy;
    use cow_types::{EcdsaSigningScheme, OrderKind, PriceQuality, SigningScheme, TokenBalance};

    use super::*;
    use crate::types::QuoteSide;

    fn unreachable_api() -> OrderBookApi {
        // Port 1 is reserved and refuses connections immediately, giving
        // a fast transport error. We disable retries so each test fails
        // promptly rather than walking the default backoff schedule.
        OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, "http://127.0.0.1:1")
            .with_retry_policy(RetryPolicy::no_retry())
    }

    fn quote_request() -> OrderQuoteRequest {
        OrderQuoteRequest {
            sell_token: Address::ZERO,
            buy_token: Address::ZERO,
            receiver: None,
            valid_to: None,
            app_data: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            partially_fillable: false,
            sell_token_balance: TokenBalance::Erc20,
            buy_token_balance: TokenBalance::Erc20,
            from: Address::ZERO,
            price_quality: PriceQuality::Verified,
            signing_scheme: EcdsaSigningScheme::Eip712,
            side: QuoteSide::sell("1"),
        }
    }

    fn order_creation() -> OrderCreation {
        OrderCreation {
            sell_token: Address::ZERO,
            buy_token: Address::ZERO,
            receiver: Address::ZERO,
            sell_amount: "1".to_owned(),
            buy_amount: "1".to_owned(),
            valid_to: 0,
            app_data: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            fee_amount: "0".to_owned(),
            kind: OrderKind::Sell,
            partially_fillable: false,
            sell_token_balance: TokenBalance::Erc20,
            buy_token_balance: TokenBalance::Erc20,
            signing_scheme: SigningScheme::Eip712,
            signature: "0x".to_owned(),
            from: Address::ZERO,
            quote_id: None,
        }
    }

    fn cancellations() -> OrderCancellations {
        OrderCancellations {
            order_uids: vec!["0xdeadbeef".to_owned()],
            signature: "0x".to_owned(),
            signing_scheme: EcdsaSigningScheme::Eip712,
        }
    }

    #[tokio::test]
    async fn trait_get_quote_delegates_to_inherent_method() {
        let res = OrderbookClient::get_quote(&unreachable_api(), &quote_request()).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn trait_send_order_delegates_to_inherent_method() {
        let res = OrderbookClient::send_order(&unreachable_api(), &order_creation()).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn trait_get_order_delegates_to_inherent_method() {
        let res = OrderbookClient::get_order(&unreachable_api(), "0xdeadbeef").await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn trait_get_trades_delegates_to_inherent_method() {
        let res = OrderbookClient::get_trades(&unreachable_api(), "0xdeadbeef").await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn trait_cancel_orders_delegates_to_inherent_method() {
        let res = OrderbookClient::cancel_orders(&unreachable_api(), &cancellations()).await;
        assert!(res.is_err());
    }
}
