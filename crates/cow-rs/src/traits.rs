//! Injectable trait abstractions for testing and composition.
//!
//! These traits decouple the SDK's high-level orchestration logic from
//! concrete HTTP clients, signers, and RPC providers. Production code
//! uses the real implementations ([`OrderBookApi`], [`PrivateKeySigner`],
//! [`OnchainReader`]), while test code can inject lightweight mocks.
//!
//! [`OrderBookApi`]: crate::order_book::OrderBookApi
//! [`PrivateKeySigner`]: alloy_signer_local::PrivateKeySigner
//! [`OnchainReader`]: crate::onchain::OnchainReader

use alloy_primitives::{Address, B256};

use crate::{
    error::CowError,
    order_book::types::{
        Order, OrderCancellations, OrderCreation, OrderQuoteRequest, OrderQuoteResponse, Trade,
    },
};

/// Abstraction over the `CoW` Protocol orderbook HTTP API.
///
/// [`OrderBookApi`](crate::order_book::OrderBookApi) implements this trait
/// by delegating to its existing async methods. Tests can inject mocks
/// that return canned responses without any network I/O.
///
/// Every method mirrors a core orderbook operation used by the
/// [`TradingSdk`](crate::trading::TradingSdk) internally.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait OrderbookClient: Send + Sync {
    /// Obtain a price quote for an order.
    ///
    /// Mirrors [`OrderBookApi::get_quote`](crate::order_book::OrderBookApi::get_quote).
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the quote request fails or is rejected.
    async fn get_quote(&self, request: &OrderQuoteRequest) -> Result<OrderQuoteResponse, CowError>;

    /// Submit a signed order and return the assigned order UID.
    ///
    /// Mirrors [`OrderBookApi::send_order`](crate::order_book::OrderBookApi::send_order).
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the order is rejected or the request fails.
    async fn send_order(&self, creation: &OrderCreation) -> Result<String, CowError>;

    /// Fetch an order by its unique identifier.
    ///
    /// Mirrors [`OrderBookApi::get_order`](crate::order_book::OrderBookApi::get_order).
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the order is not found or the request fails.
    async fn get_order(&self, order_uid: &str) -> Result<Order, CowError>;

    /// List trades for a given order UID.
    ///
    /// Mirrors [`OrderBookApi::get_trades`](crate::order_book::OrderBookApi::get_trades)
    /// with a fixed `order_uid` filter and default limit.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the request fails.
    async fn get_trades(&self, order_uid: &str) -> Result<Vec<Trade>, CowError>;

    /// Cancel one or more orders (best-effort off-chain cancellation).
    ///
    /// Mirrors [`OrderBookApi::cancel_orders`](crate::order_book::OrderBookApi::cancel_orders).
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the cancellation is rejected or the request fails.
    async fn cancel_orders(&self, cancellation: &OrderCancellations) -> Result<(), CowError>;
}

/// Abstraction over ECDSA signing used by the SDK.
///
/// [`PrivateKeySigner`](alloy_signer_local::PrivateKeySigner) implements
/// this trait. Tests can inject a mock signer that returns deterministic
/// signatures without a real private key.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait CowSigner: Send + Sync {
    /// Return the signer's Ethereum address.
    fn address(&self) -> Address;

    /// Sign an EIP-712 typed-data digest.
    ///
    /// `domain_separator` and `struct_hash` are the two 32-byte components;
    /// the implementor must hash them with the `\x19\x01` prefix and sign
    /// the result.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] on signing failure.
    async fn sign_typed_data(
        &self,
        domain_separator: B256,
        struct_hash: B256,
    ) -> Result<Vec<u8>, CowError>;

    /// Sign a raw message using EIP-191 personal-sign semantics.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] on signing failure.
    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, CowError>;
}

/// Abstraction over JSON-RPC `eth_call` and `eth_getStorageAt`.
///
/// [`OnchainReader`](crate::onchain::OnchainReader) implements this trait.
/// Tests can inject a mock that returns pre-computed ABI-encoded results.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait RpcProvider: Send + Sync {
    /// Execute a read-only `eth_call` against a contract.
    ///
    /// # Arguments
    ///
    /// * `to` - The contract address to call.
    /// * `data` - ABI-encoded calldata (selector + arguments).
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the RPC request fails.
    async fn eth_call(&self, to: Address, data: &[u8]) -> Result<Vec<u8>, CowError>;

    /// Read a single storage slot at block `"latest"`.
    ///
    /// # Arguments
    ///
    /// * `address` - The contract address whose storage to read.
    /// * `slot` - The storage slot position as a 32-byte value.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the RPC request fails.
    async fn eth_get_storage_at(&self, address: Address, slot: B256) -> Result<B256, CowError>;
}

// ── Blanket impl: OrderbookClient for OrderBookApi ──────────────────────────

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[allow(clippy::use_self, reason = "fully qualified calls needed to avoid infinite recursion")]
impl OrderbookClient for crate::order_book::OrderBookApi {
    async fn get_quote(&self, request: &OrderQuoteRequest) -> Result<OrderQuoteResponse, CowError> {
        crate::order_book::OrderBookApi::get_quote(self, request).await
    }

    async fn send_order(&self, creation: &OrderCreation) -> Result<String, CowError> {
        crate::order_book::OrderBookApi::send_order(self, creation).await
    }

    async fn get_order(&self, order_uid: &str) -> Result<Order, CowError> {
        crate::order_book::OrderBookApi::get_order(self, order_uid).await
    }

    async fn get_trades(&self, order_uid: &str) -> Result<Vec<Trade>, CowError> {
        crate::order_book::OrderBookApi::get_trades(self, Some(order_uid), None).await
    }

    async fn cancel_orders(&self, cancellation: &OrderCancellations) -> Result<(), CowError> {
        crate::order_book::OrderBookApi::cancel_orders(self, cancellation).await
    }
}

// ── Blanket impl: CowSigner for PrivateKeySigner ────────────────────────────

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl CowSigner for alloy_signer_local::PrivateKeySigner {
    fn address(&self) -> Address {
        alloy_signer::Signer::address(self)
    }

    async fn sign_typed_data(
        &self,
        domain_separator: B256,
        struct_hash: B256,
    ) -> Result<Vec<u8>, CowError> {
        // Reconstruct the EIP-712 digest: keccak256("\x19\x01" || domain_separator || struct_hash)
        use alloy_primitives::keccak256;
        let mut msg = [0u8; 66];
        msg[0] = 0x19;
        msg[1] = 0x01;
        msg[2..34].copy_from_slice(domain_separator.as_ref());
        msg[34..66].copy_from_slice(struct_hash.as_ref());
        let digest = keccak256(msg);
        let sig = alloy_signer::Signer::sign_hash(self, &digest)
            .await
            .map_err(|e| CowError::Signing(e.to_string()))?;
        Ok(sig.as_bytes().to_vec())
    }

    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, CowError> {
        let sig = alloy_signer::Signer::sign_message(self, message)
            .await
            .map_err(|e| CowError::Signing(e.to_string()))?;
        Ok(sig.as_bytes().to_vec())
    }
}

// ── Blanket impl: RpcProvider for OnchainReader ─────────────────────────────

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[allow(clippy::use_self, reason = "fully qualified calls needed to avoid infinite recursion")]
impl RpcProvider for crate::onchain::OnchainReader {
    async fn eth_call(&self, to: Address, data: &[u8]) -> Result<Vec<u8>, CowError> {
        crate::onchain::OnchainReader::eth_call(self, to, data).await
    }

    async fn eth_get_storage_at(&self, address: Address, slot: B256) -> Result<B256, CowError> {
        let slot_hex = format!("{slot:#x}");
        crate::onchain::OnchainReader::eth_get_storage_at(self, address, &slot_hex).await
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use alloy_primitives::address;

    use super::*;

    // ── Mock: OrderbookClient ───────────────────────────────────────────

    /// A mock orderbook client that returns canned responses.
    struct MockOrderbook {
        /// The quote response returned by [`get_quote`].
        quote_response: OrderQuoteResponse,
        /// The order UID returned by [`send_order`].
        send_order_uid: String,
        /// The order returned by [`get_order`].
        order: Order,
        /// The trades returned by [`get_trades`].
        trades: Vec<Trade>,
    }

    impl MockOrderbook {
        /// Build a minimal mock with zeroed/empty canned responses.
        fn minimal() -> Self {
            Self {
                quote_response: serde_json::from_str(MINIMAL_QUOTE_RESPONSE_JSON)
                    .unwrap_or_else(|e| panic!("bad fixture: {e}")),
                send_order_uid: "0xmockuid".to_owned(),
                order: serde_json::from_str(MINIMAL_ORDER_JSON)
                    .unwrap_or_else(|e| panic!("bad fixture: {e}")),
                trades: Vec::new(),
            }
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl OrderbookClient for MockOrderbook {
        async fn get_quote(
            &self,
            _request: &OrderQuoteRequest,
        ) -> Result<OrderQuoteResponse, CowError> {
            Ok(self.quote_response.clone())
        }

        async fn send_order(&self, _creation: &OrderCreation) -> Result<String, CowError> {
            Ok(self.send_order_uid.clone())
        }

        async fn get_order(&self, _order_uid: &str) -> Result<Order, CowError> {
            Ok(self.order.clone())
        }

        async fn get_trades(&self, _order_uid: &str) -> Result<Vec<Trade>, CowError> {
            Ok(self.trades.clone())
        }

        async fn cancel_orders(&self, _cancellation: &OrderCancellations) -> Result<(), CowError> {
            Ok(())
        }
    }

    // ── Mock: CowSigner ────────────────────────────────────────────────

    /// A mock signer that returns a fixed address and dummy signatures.
    struct MockSigner {
        addr: Address,
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl CowSigner for MockSigner {
        fn address(&self) -> Address {
            self.addr
        }

        async fn sign_typed_data(
            &self,
            _domain_separator: B256,
            _struct_hash: B256,
        ) -> Result<Vec<u8>, CowError> {
            Ok(vec![0u8; 65])
        }

        async fn sign_message(&self, _message: &[u8]) -> Result<Vec<u8>, CowError> {
            Ok(vec![0u8; 65])
        }
    }

    // ── Mock: RpcProvider ──────────────────────────────────────────────

    /// A mock RPC provider that returns zeroed responses.
    struct MockRpcProvider;

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl RpcProvider for MockRpcProvider {
        async fn eth_call(&self, _to: Address, _data: &[u8]) -> Result<Vec<u8>, CowError> {
            Ok(vec![0u8; 32])
        }

        async fn eth_get_storage_at(
            &self,
            _address: Address,
            _slot: B256,
        ) -> Result<B256, CowError> {
            Ok(B256::ZERO)
        }
    }

    // ── JSON fixtures ──────────────────────────────────────────────────

    const MINIMAL_QUOTE_RESPONSE_JSON: &str = r#"{
        "quote": {
            "sellToken": "0xfff9976782d46cc05630d1f6ebab18b2324d6b14",
            "buyToken": "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238",
            "receiver": "0x0000000000000000000000000000000000000000",
            "sellAmount": "1000000000000000",
            "buyAmount": "500000",
            "validTo": 1700000000,
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "feeAmount": "1000000000000",
            "kind": "sell",
            "partiallyFillable": false,
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20"
        },
        "from": "0x0000000000000000000000000000000000000000",
        "expiration": "2099-01-01T00:00:00Z",
        "id": 12345,
        "verified": false
    }"#;

    const MINIMAL_ORDER_JSON: &str = r#"{
        "uid": "0xmockuid",
        "sellToken": "0xfff9976782d46cc05630d1f6ebab18b2324d6b14",
        "buyToken": "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238",
        "receiver": "0x0000000000000000000000000000000000000000",
        "sellAmount": "1000000000000000",
        "buyAmount": "500000",
        "validTo": 1700000000,
        "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "feeAmount": "1000000000000",
        "kind": "sell",
        "partiallyFillable": false,
        "sellTokenBalance": "erc20",
        "buyTokenBalance": "erc20",
        "creationDate": "2024-01-01T00:00:00Z",
        "owner": "0x0000000000000000000000000000000000000000",
        "availableBalance": null,
        "executedSellAmount": "0",
        "executedSellAmountBeforeFees": "0",
        "executedBuyAmount": "0",
        "executedFeeAmount": "0",
        "invalidated": false,
        "status": "open",
        "signingScheme": "eip712",
        "signature": "0x",
        "fullAppData": null,
        "class": "market",
        "executedSurplusFee": "0"
    }"#;

    // ── Tests ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn mock_orderbook_get_quote() {
        let mock = MockOrderbook::minimal();
        let req: OrderQuoteRequest = serde_json::from_value(serde_json::json!({
            "sellToken": "0xfff9976782d46cc05630d1f6ebab18b2324d6b14",
            "buyToken": "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238",
            "from": "0x0000000000000000000000000000000000000000",
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "partiallyFillable": false,
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20",
            "priceQuality": "optimal",
            "signingScheme": "eip712",
            "kind": "sell",
            "sellAmountBeforeFee": "1000000000000000"
        }))
        .unwrap_or_else(|e| panic!("bad request fixture: {e}"));

        let resp = mock.get_quote(&req).await;
        assert!(resp.is_ok(), "mock get_quote should succeed");
        assert_eq!(resp.unwrap_or_else(|e| panic!("{e}")).id, Some(12345));
    }

    #[tokio::test]
    async fn mock_orderbook_send_order() {
        let mock = MockOrderbook::minimal();
        let creation: OrderCreation = serde_json::from_value(serde_json::json!({
            "sellToken": "0xfff9976782d46cc05630d1f6ebab18b2324d6b14",
            "buyToken": "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238",
            "receiver": "0x0000000000000000000000000000000000000000",
            "sellAmount": "1000000000000000",
            "buyAmount": "500000",
            "validTo": 1700000000,
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "feeAmount": "0",
            "kind": "sell",
            "partiallyFillable": false,
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20",
            "signingScheme": "eip712",
            "signature": "0x",
            "from": "0x0000000000000000000000000000000000000000"
        }))
        .unwrap_or_else(|e| panic!("bad creation fixture: {e}"));

        let uid = mock.send_order(&creation).await;
        assert!(uid.is_ok(), "mock send_order should succeed");
        assert_eq!(uid.unwrap_or_else(|e| panic!("{e}")), "0xmockuid");
    }

    #[tokio::test]
    async fn mock_orderbook_get_order() {
        let mock = MockOrderbook::minimal();
        let order = mock.get_order("0xmockuid").await;
        assert!(order.is_ok(), "mock get_order should succeed");
        assert_eq!(order.unwrap_or_else(|e| panic!("{e}")).uid, "0xmockuid");
    }

    #[tokio::test]
    async fn mock_orderbook_get_trades() {
        let mock = MockOrderbook::minimal();
        let trades = mock.get_trades("0xmockuid").await;
        assert!(trades.is_ok(), "mock get_trades should succeed");
        assert!(trades.unwrap_or_else(|e| panic!("{e}")).is_empty());
    }

    #[tokio::test]
    async fn mock_orderbook_cancel_orders() {
        let mock = MockOrderbook::minimal();
        let cancellation = OrderCancellations {
            order_uids: vec!["0xmockuid".to_owned()],
            signature: "0x".to_owned(),
            signing_scheme: crate::types::EcdsaSigningScheme::Eip712,
        };
        let result = mock.cancel_orders(&cancellation).await;
        assert!(result.is_ok(), "mock cancel_orders should succeed");
    }

    #[tokio::test]
    async fn mock_signer_address() {
        let signer = MockSigner { addr: address!("1111111111111111111111111111111111111111") };
        assert_eq!(signer.address(), address!("1111111111111111111111111111111111111111"));
    }

    #[tokio::test]
    async fn mock_signer_sign_typed_data() {
        let signer = MockSigner { addr: Address::ZERO };
        let sig = signer.sign_typed_data(B256::ZERO, B256::ZERO).await;
        assert!(sig.is_ok(), "mock sign_typed_data should succeed");
        assert_eq!(sig.unwrap_or_else(|e| panic!("{e}")).len(), 65);
    }

    #[tokio::test]
    async fn mock_signer_sign_message() {
        let signer = MockSigner { addr: Address::ZERO };
        let sig = signer.sign_message(b"test message").await;
        assert!(sig.is_ok(), "mock sign_message should succeed");
        assert_eq!(sig.unwrap_or_else(|e| panic!("{e}")).len(), 65);
    }

    #[tokio::test]
    async fn mock_rpc_provider_eth_call() {
        let provider = MockRpcProvider;
        let result = provider.eth_call(Address::ZERO, &[0u8; 4]).await;
        assert!(result.is_ok(), "mock eth_call should succeed");
        assert_eq!(result.unwrap_or_else(|e| panic!("{e}")).len(), 32);
    }

    #[tokio::test]
    async fn mock_rpc_provider_eth_get_storage_at() {
        let provider = MockRpcProvider;
        let result = provider.eth_get_storage_at(Address::ZERO, B256::ZERO).await;
        assert!(result.is_ok(), "mock eth_get_storage_at should succeed");
        assert_eq!(result.unwrap_or_else(|e| panic!("{e}")), B256::ZERO);
    }

    #[tokio::test]
    async fn trait_object_orderbook_client() {
        // Verify the trait is object-safe and works behind Arc<dyn>.
        let mock: std::sync::Arc<dyn OrderbookClient> =
            std::sync::Arc::new(MockOrderbook::minimal());
        let order = mock.get_order("0xmockuid").await;
        assert!(order.is_ok(), "trait object get_order should succeed");
    }

    #[tokio::test]
    async fn trait_object_cow_signer() {
        let mock: std::sync::Arc<dyn CowSigner> =
            std::sync::Arc::new(MockSigner { addr: Address::ZERO });
        assert_eq!(mock.address(), Address::ZERO);
    }

    #[tokio::test]
    async fn trait_object_rpc_provider() {
        let mock: std::sync::Arc<dyn RpcProvider> = std::sync::Arc::new(MockRpcProvider);
        let result = mock.eth_call(Address::ZERO, &[]).await;
        assert!(result.is_ok(), "trait object eth_call should succeed");
    }

    #[test]
    fn real_private_key_signer_implements_cow_signer() {
        // Compile-time check: PrivateKeySigner implements CowSigner.
        fn _assert_cow_signer<T: CowSigner>() {}
        _assert_cow_signer::<alloy_signer_local::PrivateKeySigner>();
    }

    #[test]
    fn real_orderbook_api_implements_orderbook_client() {
        // Compile-time check: OrderBookApi implements OrderbookClient.
        fn _assert_orderbook_client<T: OrderbookClient>() {}
        _assert_orderbook_client::<crate::order_book::OrderBookApi>();
    }

    #[test]
    fn real_onchain_reader_implements_rpc_provider() {
        // Compile-time check: OnchainReader implements RpcProvider.
        fn _assert_rpc_provider<T: RpcProvider>() {}
        _assert_rpc_provider::<crate::onchain::OnchainReader>();
    }
}
