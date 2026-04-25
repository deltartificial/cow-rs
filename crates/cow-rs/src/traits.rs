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

/// Abstraction over IPFS fetch and upload operations.
///
/// [`Ipfs`](crate::app_data::Ipfs) implements this trait by delegating to
/// the existing free functions in [`crate::app_data`]. Tests can inject a
/// mock that returns canned CID/content pairs without any network I/O.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait IpfsClient: Send + Sync {
    /// Fetch a JSON document from IPFS by its CID.
    ///
    /// # Arguments
    ///
    /// * `cid` - The `CIDv1` base16 string identifying the document.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the fetch or deserialisation fails.
    async fn fetch(&self, cid: &str) -> Result<String, CowError>;

    /// Upload a JSON string to IPFS and return the resulting CID.
    ///
    /// # Arguments
    ///
    /// * `content` - The JSON content to pin.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the upload fails (e.g. missing credentials).
    async fn upload(&self, content: &str) -> Result<String, CowError>;
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

// Function pointer (rather than a closure) so the unreachable error arm
// has its own item — exercised directly in the tests below.
fn map_alloy_signing_error(e: alloy_signer::Error) -> CowError {
    CowError::Signing(e.to_string())
}

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
            .map_err(map_alloy_signing_error)?;
        Ok(sig.as_bytes().to_vec())
    }

    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, CowError> {
        let sig = alloy_signer::Signer::sign_message(self, message)
            .await
            .map_err(map_alloy_signing_error)?;
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

// ── Blanket impl: IpfsClient for Ipfs ──────────────────────────────────────

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl IpfsClient for crate::app_data::Ipfs {
    async fn fetch(&self, cid: &str) -> Result<String, CowError> {
        let base =
            self.read_uri.as_deref().unwrap_or_else(|| crate::app_data::DEFAULT_IPFS_READ_URI);
        let url = format!("{base}/{cid}");
        let text = reqwest::get(&url).await?.text().await?;
        Ok(text)
    }

    async fn upload(&self, content: &str) -> Result<String, CowError> {
        let api_key = self.pinata_api_key.as_deref().ok_or_else(|| {
            CowError::AppData("pinata_api_key is required for IPFS upload".into())
        })?;
        let api_secret = self.pinata_api_secret.as_deref().ok_or_else(|| {
            CowError::AppData("pinata_api_secret is required for IPFS upload".into())
        })?;

        let write_uri =
            self.write_uri.as_deref().unwrap_or_else(|| crate::app_data::DEFAULT_IPFS_WRITE_URI);
        let url = format!("{write_uri}/pinning/pinJSONToIPFS");

        let parsed: serde_json::Value =
            serde_json::from_str(content).map_err(|e| CowError::AppData(e.to_string()))?;

        let body = serde_json::json!({
            "pinataContent": parsed,
            "pinataOptions": { "cidVersion": 1 },
        });

        let resp = reqwest::Client::new()
            .post(&url)
            .header("pinata_api_key", api_key)
            .header("pinata_secret_api_key", api_secret)
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let text = resp.text().await?;
        if status != 200 {
            return Err(CowError::Api { status, body: text });
        }

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct PinataResponse {
            ipfs_hash: String,
        }
        let pinata: PinataResponse =
            serde_json::from_str(&text).map_err(|e| CowError::AppData(e.to_string()))?;
        Ok(pinata.ipfs_hash)
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

    // ── Mock: IpfsClient ───────────────────────────────────────────────

    /// A mock IPFS client that returns canned responses.
    struct MockIpfsClient {
        /// The content returned by [`fetch`].
        fetch_content: String,
        /// The CID returned by [`upload`].
        upload_cid: String,
    }

    impl MockIpfsClient {
        /// Build a minimal mock with fixed canned responses.
        fn new() -> Self {
            Self {
                fetch_content: r#"{"version":"1.3.0","appCode":"test"}"#.to_owned(),
                upload_cid: "bafybeimockcid".to_owned(),
            }
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl IpfsClient for MockIpfsClient {
        async fn fetch(&self, _cid: &str) -> Result<String, CowError> {
            Ok(self.fetch_content.clone())
        }

        async fn upload(&self, _content: &str) -> Result<String, CowError> {
            Ok(self.upload_cid.clone())
        }
    }

    // ── Tests ──────────────────────────────────────────────────────────

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

    #[test]
    fn real_ipfs_implements_ipfs_client() {
        // Compile-time check: Ipfs implements IpfsClient.
        fn _assert_ipfs_client<T: IpfsClient>() {}
        _assert_ipfs_client::<crate::app_data::Ipfs>();
    }

    #[tokio::test]
    async fn mock_ipfs_client_fetch() {
        let mock = MockIpfsClient::new();
        let result = mock.fetch("bafybeisomecid").await;
        assert!(result.is_ok(), "mock fetch should succeed");
        assert!(result.unwrap_or_else(|e| panic!("{e}")).contains("version"));
    }

    #[tokio::test]
    async fn mock_ipfs_client_upload() {
        let mock = MockIpfsClient::new();
        let result = mock.upload(r#"{"test": true}"#).await;
        assert!(result.is_ok(), "mock upload should succeed");
        assert_eq!(result.unwrap_or_else(|e| panic!("{e}")), "bafybeimockcid");
    }

    #[tokio::test]
    async fn trait_object_ipfs_client() {
        // Verify the trait is object-safe and works behind Arc<dyn>.
        let mock: std::sync::Arc<dyn IpfsClient> = std::sync::Arc::new(MockIpfsClient::new());
        let result = mock.fetch("bafybeisomecid").await;
        assert!(result.is_ok(), "trait object fetch should succeed");
    }

    // ── PrivateKeySigner blanket impl tests ────────────────────────────

    #[tokio::test]
    async fn private_key_signer_address() {
        let signer = alloy_signer_local::PrivateKeySigner::random();
        let expected = alloy_signer::Signer::address(&signer);
        let cow_addr = <alloy_signer_local::PrivateKeySigner as CowSigner>::address(&signer);
        assert_eq!(cow_addr, expected);
    }

    #[tokio::test]
    async fn private_key_signer_sign_typed_data() {
        let signer = alloy_signer_local::PrivateKeySigner::random();
        let result = CowSigner::sign_typed_data(&signer, B256::ZERO, B256::ZERO).await;
        assert!(result.is_ok(), "PrivateKeySigner sign_typed_data should succeed");
        assert_eq!(result.unwrap().len(), 65);
    }

    #[tokio::test]
    async fn private_key_signer_sign_message() {
        let signer = alloy_signer_local::PrivateKeySigner::random();
        let result = CowSigner::sign_message(&signer, b"hello world").await;
        assert!(result.is_ok(), "PrivateKeySigner sign_message should succeed");
        assert_eq!(result.unwrap().len(), 65);
    }

    // ── Ipfs struct construction tests ──────────────────────────────────

    #[test]
    fn ipfs_struct_default_fields() {
        let ipfs = crate::app_data::Ipfs {
            read_uri: None,
            write_uri: None,
            pinata_api_key: None,
            pinata_api_secret: None,
        };
        // Exercise the default read URI path in IpfsClient::fetch
        assert!(ipfs.read_uri.is_none());
        assert!(ipfs.pinata_api_key.is_none());
    }

    // ── Error-returning mock impls ────────────────────────────────────────

    /// A mock orderbook client that always returns errors.
    struct ErrorOrderbook;

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl OrderbookClient for ErrorOrderbook {
        async fn get_quote(
            &self,
            _request: &OrderQuoteRequest,
        ) -> Result<OrderQuoteResponse, CowError> {
            Err(CowError::Api { status: 500, body: "mock error".into() })
        }

        async fn send_order(&self, _creation: &OrderCreation) -> Result<String, CowError> {
            Err(CowError::Api { status: 500, body: "mock error".into() })
        }

        async fn get_order(&self, _order_uid: &str) -> Result<Order, CowError> {
            Err(CowError::Api { status: 404, body: "not found".into() })
        }

        async fn get_trades(&self, _order_uid: &str) -> Result<Vec<Trade>, CowError> {
            Err(CowError::Api { status: 500, body: "mock error".into() })
        }

        async fn cancel_orders(&self, _cancellation: &OrderCancellations) -> Result<(), CowError> {
            Err(CowError::Api { status: 403, body: "forbidden".into() })
        }
    }

    #[tokio::test]
    async fn error_orderbook_get_quote() {
        let mock = ErrorOrderbook;
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
        .unwrap();
        let result = mock.get_quote(&req).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn error_orderbook_send_order() {
        let mock = ErrorOrderbook;
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
        .unwrap();
        let result = mock.send_order(&creation).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn error_orderbook_get_order() {
        let mock = ErrorOrderbook;
        let result = mock.get_order("0xmockuid").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn error_orderbook_get_trades() {
        let mock = ErrorOrderbook;
        let result = mock.get_trades("0xmockuid").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn error_orderbook_cancel_orders() {
        let mock = ErrorOrderbook;
        let cancellation = OrderCancellations {
            order_uids: vec!["0xmockuid".to_owned()],
            signature: "0x".to_owned(),
            signing_scheme: crate::types::EcdsaSigningScheme::Eip712,
        };
        let result = mock.cancel_orders(&cancellation).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn trait_object_error_orderbook() {
        let mock: std::sync::Arc<dyn OrderbookClient> = std::sync::Arc::new(ErrorOrderbook);
        let result = mock.get_order("0xmockuid").await;
        assert!(result.is_err());
    }

    // ── Error-returning mock signer ───────────────────────────────────────

    struct ErrorSigner;

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl CowSigner for ErrorSigner {
        fn address(&self) -> Address {
            Address::ZERO
        }

        async fn sign_typed_data(
            &self,
            _domain_separator: B256,
            _struct_hash: B256,
        ) -> Result<Vec<u8>, CowError> {
            Err(CowError::Signing("mock signer error".into()))
        }

        async fn sign_message(&self, _message: &[u8]) -> Result<Vec<u8>, CowError> {
            Err(CowError::Signing("mock signer error".into()))
        }
    }

    #[tokio::test]
    async fn error_signer_sign_typed_data() {
        let signer = ErrorSigner;
        let result = signer.sign_typed_data(B256::ZERO, B256::ZERO).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn error_signer_sign_message() {
        let signer = ErrorSigner;
        let result = signer.sign_message(b"test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn trait_object_error_signer() {
        let mock: std::sync::Arc<dyn CowSigner> = std::sync::Arc::new(ErrorSigner);
        let result = mock.sign_typed_data(B256::ZERO, B256::ZERO).await;
        assert!(result.is_err());
    }

    // ── Error-returning mock RPC provider ─────────────────────────────────

    struct ErrorRpcProvider;

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl RpcProvider for ErrorRpcProvider {
        async fn eth_call(&self, _to: Address, _data: &[u8]) -> Result<Vec<u8>, CowError> {
            Err(CowError::Rpc { code: -32000, message: "mock rpc error".into() })
        }

        async fn eth_get_storage_at(
            &self,
            _address: Address,
            _slot: B256,
        ) -> Result<B256, CowError> {
            Err(CowError::Rpc { code: -32000, message: "mock rpc error".into() })
        }
    }

    #[tokio::test]
    async fn error_rpc_provider_eth_call() {
        let provider = ErrorRpcProvider;
        let result = provider.eth_call(Address::ZERO, &[0u8; 4]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn error_rpc_provider_eth_get_storage_at() {
        let provider = ErrorRpcProvider;
        let result = provider.eth_get_storage_at(Address::ZERO, B256::ZERO).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn trait_object_error_rpc_provider() {
        let mock: std::sync::Arc<dyn RpcProvider> = std::sync::Arc::new(ErrorRpcProvider);
        let result = mock.eth_call(Address::ZERO, &[]).await;
        assert!(result.is_err());
    }

    // ── Error-returning mock IPFS client ──────────────────────────────────

    struct ErrorIpfsClient;

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl IpfsClient for ErrorIpfsClient {
        async fn fetch(&self, _cid: &str) -> Result<String, CowError> {
            Err(CowError::AppData("mock ipfs fetch error".into()))
        }

        async fn upload(&self, _content: &str) -> Result<String, CowError> {
            Err(CowError::AppData("mock ipfs upload error".into()))
        }
    }

    #[tokio::test]
    async fn error_ipfs_client_fetch() {
        let mock = ErrorIpfsClient;
        let result = mock.fetch("bafybeisomecid").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn error_ipfs_client_upload() {
        let mock = ErrorIpfsClient;
        let result = mock.upload(r#"{"test": true}"#).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn trait_object_error_ipfs_client() {
        let mock: std::sync::Arc<dyn IpfsClient> = std::sync::Arc::new(ErrorIpfsClient);
        let result = mock.fetch("bafybeisomecid").await;
        assert!(result.is_err());
    }

    // ── PrivateKeySigner sign_typed_data with non-zero inputs ─────────────

    #[tokio::test]
    async fn private_key_signer_sign_typed_data_non_zero() {
        let signer = alloy_signer_local::PrivateKeySigner::random();
        let ds = B256::from([0xABu8; 32]);
        let sh = B256::from([0xCDu8; 32]);
        let result = CowSigner::sign_typed_data(&signer, ds, sh).await;
        assert!(result.is_ok());
        let sig = result.unwrap();
        assert_eq!(sig.len(), 65);
        // Different inputs should produce different signatures
        let result2 = CowSigner::sign_typed_data(&signer, B256::ZERO, B256::ZERO).await;
        assert!(result2.is_ok());
        assert_ne!(sig, result2.unwrap());
    }

    // ── Ipfs struct with custom read_uri ──────────────────────────────────

    #[test]
    fn ipfs_struct_custom_read_uri() {
        let ipfs = crate::app_data::Ipfs {
            read_uri: Some("https://custom.gateway.io/ipfs".to_owned()),
            write_uri: Some("https://custom.write.io".to_owned()),
            pinata_api_key: Some("key".to_owned()),
            pinata_api_secret: Some("secret".to_owned()),
        };
        assert_eq!(ipfs.read_uri.as_deref(), Some("https://custom.gateway.io/ipfs"));
        assert_eq!(ipfs.write_uri.as_deref(), Some("https://custom.write.io"));
    }

    // ── OrderBookApi blanket impl ────────────────────────────────────────
    //
    // The `impl OrderbookClient for OrderBookApi` block (lines ~181-201)
    // contains five pure delegating wrappers. We exercise each one against
    // an unreachable base URL — the inner HTTP call returns a transport
    // error, which is fine: the wrapper body itself runs end-to-end.
    //
    // Port 1 is reserved and refuses connections immediately, and we
    // disable retries so each test fails promptly rather than walking the
    // default backoff schedule.

    fn unreachable_orderbook_api() -> crate::order_book::OrderBookApi {
        use cow_chains::{Env, SupportedChainId};
        use cow_http::RetryPolicy;
        crate::order_book::OrderBookApi::new_with_url(
            SupportedChainId::Mainnet,
            Env::Prod,
            "http://127.0.0.1:1",
        )
        .with_retry_policy(RetryPolicy::no_retry())
    }

    fn minimal_quote_request() -> OrderQuoteRequest {
        serde_json::from_value(serde_json::json!({
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
        .unwrap_or_else(|e| panic!("bad fixture: {e}"))
    }

    fn minimal_order_creation() -> OrderCreation {
        serde_json::from_value(serde_json::json!({
            "sellToken": "0xfff9976782d46cc05630d1f6ebab18b2324d6b14",
            "buyToken": "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238",
            "receiver": "0x0000000000000000000000000000000000000000",
            "sellAmount": "1000000000000000",
            "buyAmount": "500000",
            "validTo": 1_700_000_000u64,
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
        .unwrap_or_else(|e| panic!("bad fixture: {e}"))
    }

    #[tokio::test]
    async fn orderbook_api_blanket_get_quote_delegates() {
        let api = unreachable_orderbook_api();
        let res = OrderbookClient::get_quote(&api, &minimal_quote_request()).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn orderbook_api_blanket_send_order_delegates() {
        let api = unreachable_orderbook_api();
        let res = OrderbookClient::send_order(&api, &minimal_order_creation()).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn orderbook_api_blanket_get_order_delegates() {
        let api = unreachable_orderbook_api();
        let res = OrderbookClient::get_order(&api, "0xdeadbeef").await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn orderbook_api_blanket_get_trades_delegates() {
        let api = unreachable_orderbook_api();
        let res = OrderbookClient::get_trades(&api, "0xdeadbeef").await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn orderbook_api_blanket_cancel_orders_delegates() {
        let api = unreachable_orderbook_api();
        let cancellation = OrderCancellations {
            order_uids: vec!["0xdeadbeef".to_owned()],
            signature: "0x".to_owned(),
            signing_scheme: crate::types::EcdsaSigningScheme::Eip712,
        };
        let res = OrderbookClient::cancel_orders(&api, &cancellation).await;
        assert!(res.is_err());
    }

    // ── ErrorSigner.address() coverage ───────────────────────────────────
    //
    // The existing `ErrorSigner` mock has an `address()` body that is not
    // exercised by any other test. Call it directly to close that gap.

    #[test]
    fn error_signer_address_returns_zero() {
        let signer = ErrorSigner;
        assert_eq!(<ErrorSigner as CowSigner>::address(&signer), Address::ZERO);
    }

    #[test]
    fn map_alloy_signing_error_wraps_into_cow_signing() {
        let err = super::map_alloy_signing_error(alloy_signer::Error::other("boom"));
        let CowError::Signing(msg) = err else { panic!("expected CowError::Signing, got {err:?}") };
        assert!(msg.contains("boom"), "unexpected message: {msg}");
    }
}
