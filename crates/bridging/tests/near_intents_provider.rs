#![allow(
    clippy::allow_attributes_without_reason,
    clippy::tests_outside_test_module,
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::disallowed_types
)]
//! Wiremock-backed integration tests for the [`NearIntentsBridgeProvider`].
//!
//! These drive the full `get_quote` flow including the
//! **crypto-critical attestation verification**. The tests override
//! [`NearIntentsProviderOptions::attestator_address`] to a locally-
//! generated signer so we can attest the messages ourselves and
//! confirm the `recovered == attestator` check works end-to-end.

use alloy_primitives::{Address, U256};
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use cow_bridging::{
    BridgeProvider, BridgeStatus, QuoteBridgeRequest,
    near_intents::{
        NearIntentsBridgeProvider, NearIntentsProviderOptions, default_near_intents_info,
        util::{ATTESTATION_PREFIX_BYTES, ATTESTATION_VERSION_BYTES, hash_quote_payload},
    },
};
use cow_types::OrderKind;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

const TEST_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

// ── Fixture builders ─────────────────────────────────────────────────────

fn tokens_fixture() -> serde_json::Value {
    serde_json::json!([
        // Source-side: USDC on mainnet.
        {
            "assetId":         "nep141:usdc.e",
            "decimals":        6,
            "blockchain":      "eth",
            "symbol":          "USDC",
            "price":           1.0,
            "priceUpdatedAt":  "2025-09-05T12:00:38.695Z",
            "contractAddress": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        },
        // Destination-side: BTC (non-EVM — adapt_token drops these).
        {
            "assetId":         "nep141:btc",
            "decimals":        8,
            "blockchain":      "btc",
            "symbol":          "BTC",
            "price":           60000.0,
            "priceUpdatedAt":  "2025-09-05T12:00:38.695Z"
        }
    ])
}

fn quote_response_fixture(deposit_address: &str) -> serde_json::Value {
    serde_json::json!({
        "quote": {
            "amountIn":            "1000000",
            "amountInFormatted":   "1.0",
            "amountInUsd":         "1.0",
            "minAmountIn":         "1000000",
            "amountOut":           "1000000",
            "amountOutFormatted":  "1.0",
            "amountOutUsd":        "1.0",
            "minAmountOut":        "999500",
            "timeEstimate":        120,
            "deadline":            "2099-01-01T00:00:00.000Z",
            "timeWhenInactive":    "2099-01-01T01:00:00.000Z",
            "depositAddress":      deposit_address
        },
        "quoteRequest": {
            "dry":              false,
            "swapType":         "EXACT_INPUT",
            "depositMode":      "SIMPLE",
            "slippageTolerance": 50,
            "originAsset":      "nep141:0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
            "depositType":      "ORIGIN_CHAIN",
            "destinationAsset": "nep141:0x0000000000000000000000000000000000000000",
            "amount":           "1000000",
            "refundTo":         "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
            "refundType":       "ORIGIN_CHAIN",
            "recipient":        "bc1q000000000000000000000000000000000000",
            "recipientType":    "DESTINATION_CHAIN",
            "deadline":          "2099-01-01T00:00:00.000Z"
        },
        "signature": "ed25519:testSignature",
        "timestamp": "2025-09-05T12:00:40.000Z"
    })
}

fn execution_status_fixture() -> serde_json::Value {
    serde_json::json!({
        "status": "SUCCESS",
        "updatedAt": "2025-09-05T12:00:50.000Z",
        "swapDetails": {
            "intentHashes": ["0xhash1"],
            "nearTxHashes": ["0xnear1"],
            "amountIn":            "1000000",
            "amountInFormatted":   "1.0",
            "amountInUsd":         "1.0",
            "amountOut":           "1000000",
            "amountOutFormatted":  "1.0",
            "amountOutUsd":        "1.0",
            "slippage":            0.0,
            "refundedAmount":           "0",
            "refundedAmountFormatted":  "0.0",
            "refundedAmountUsd":        "0.0",
            "originChainTxHashes": [
                { "hash": "0xorigin1", "explorerUrl": "https://etherscan.io/tx/0xorigin1" }
            ],
            "destinationChainTxHashes": [
                { "hash": "bc1tx1", "explorerUrl": "https://mempool.space/tx/bc1tx1" }
            ]
        }
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn test_signer() -> PrivateKeySigner {
    use std::str::FromStr;
    PrivateKeySigner::from_str(TEST_KEY).unwrap()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        write!(&mut s, "{b:02x}").unwrap();
    }
    s
}

/// Compute the exact same canonical hash the provider computes, then
/// sign the attestation message with the local test signer. Returns
/// the `0x`-prefixed signature + recovered attestor address.
fn sign_attestation_for_quote(
    quote_body: &serde_json::Value,
    deposit_address: Address,
) -> (String, Address) {
    use alloy_primitives::keccak256;
    let quote: cow_bridging::near_intents::types::NearQuote =
        serde_json::from_value(quote_body["quote"].clone()).unwrap();
    let quote_request: cow_bridging::near_intents::types::NearQuoteRequest =
        serde_json::from_value(quote_body["quoteRequest"].clone()).unwrap();
    let timestamp = quote_body["timestamp"].as_str().unwrap();

    let (quote_hash, _) = hash_quote_payload(&quote, &quote_request, timestamp).unwrap();

    let mut message = Vec::with_capacity(57);
    message.extend_from_slice(&ATTESTATION_PREFIX_BYTES);
    message.extend_from_slice(&ATTESTATION_VERSION_BYTES);
    message.extend_from_slice(deposit_address.as_slice());
    message.extend_from_slice(quote_hash.as_slice());
    let digest = keccak256(&message);

    let signer = test_signer();
    let sig = signer.sign_hash_sync(&digest).unwrap();
    (format!("0x{}", hex_encode(&sig.as_bytes())), signer.address())
}

fn bridge_request_eth_to_btc() -> QuoteBridgeRequest {
    QuoteBridgeRequest {
        sell_chain_id: 1,
        buy_chain_id: 1_000_000_000, // Bitcoin
        sell_token: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap(),
        sell_token_decimals: 6,
        buy_token: Address::ZERO,
        buy_token_decimals: 8,
        sell_amount: U256::from(1_000_000u64),
        account: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap(),
        owner: None,
        receiver: None,
        bridge_recipient: Some("bc1q000000000000000000000000000000000000".into()),
        slippage_bps: 50,
        bridge_slippage_bps: Some(50),
        kind: OrderKind::Sell,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn info_matches_default_helper() {
    let p = NearIntentsBridgeProvider::default();
    assert_eq!(p.info().dapp_id, default_near_intents_info().dapp_id);
    assert_eq!(p.info().name, "near-intents");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_networks_lists_11_chains() {
    let p = NearIntentsBridgeProvider::default();
    let nets = p.get_networks().await.unwrap();
    assert_eq!(nets.len(), 11);
    assert!(nets.iter().any(|n| n.chain_id == 1));
    assert!(nets.iter().any(|n| n.chain_id == 1_000_000_000)); // Bitcoin
    assert!(nets.iter().any(|n| n.chain_id == 1_000_000_001)); // Solana
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_buy_tokens_filters_by_chain() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v0/tokens"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tokens_fixture()))
        .mount(&server)
        .await;
    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url: Some(server.uri()),
        ..Default::default()
    });

    let buy = provider
        .get_buy_tokens(cow_bridging::BuyTokensParams {
            sell_chain_id: 1,
            buy_chain_id: 1,
            sell_token_address: None,
        })
        .await
        .unwrap();
    // Only USDC on mainnet survives the EVM filter — BTC is dropped
    // because it lives on a non-EVM chain.
    assert_eq!(buy.tokens.len(), 1);
    assert_eq!(buy.tokens[0].symbol, "USDC");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_status_maps_success_to_executed() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v0/execution-status/0xdeadbeef"))
        .respond_with(ResponseTemplate::new(200).set_body_json(execution_status_fixture()))
        .mount(&server)
        .await;
    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url: Some(server.uri()),
        ..Default::default()
    });

    let status = provider.get_status("0xdeadbeef", 1).await.unwrap();
    assert_eq!(status.status, BridgeStatus::Executed);
    assert_eq!(status.deposit_tx_hash.as_deref(), Some("0xorigin1"));
    assert_eq!(status.fill_tx_hash.as_deref(), Some("bc1tx1"));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_succeeds_with_valid_attestation() {
    // Build a full end-to-end fixture: tokens, quote, attestation
    // signature from a local signer, and the matching attestator
    // address threaded into provider options.
    let deposit_address: Address = "0xdead000000000000000000000000000000000000".parse().unwrap();
    let quote_body = quote_response_fixture("0xdead000000000000000000000000000000000000");
    let (signature_hex, attestator_addr) = sign_attestation_for_quote(&quote_body, deposit_address);

    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_body))
        .mount(&server)
        .await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/attestation"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "signature": signature_hex,
            "version":   1,
        })))
        .mount(&server)
        .await;

    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url: Some(server.uri()),
        attestator_address: attestator_addr,
        ..Default::default()
    });

    let resp = provider.get_quote(&bridge_request_eth_to_btc()).await.unwrap();
    assert_eq!(resp.provider, "near-intents");
    assert!(resp.sell_amount > U256::ZERO);
    assert_eq!(resp.estimated_secs, 120);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_rejects_when_attestation_signer_mismatches() {
    // Same fixture, but leave `attestator_address` at the default —
    // the mainnet attestor isn't our local test signer, so the
    // recovery check must reject the quote.
    let deposit_address: Address = "0xdead000000000000000000000000000000000000".parse().unwrap();
    let quote_body = quote_response_fixture("0xdead000000000000000000000000000000000000");
    let (signature_hex, _attestator_addr) =
        sign_attestation_for_quote(&quote_body, deposit_address);

    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_body))
        .mount(&server)
        .await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/attestation"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "signature": signature_hex,
            "version":   1,
        })))
        .mount(&server)
        .await;

    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url: Some(server.uri()),
        // Default attestator — not our test signer → mismatch.
        ..Default::default()
    });

    let err = provider.get_quote(&bridge_request_eth_to_btc()).await.unwrap_err();
    assert!(err.to_string().contains("attestation mismatch"), "unexpected error: {err}",);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_rejects_tampered_deposit_address() {
    // Construct a signature that attests a DIFFERENT deposit address
    // than the one the quote response claims. The recovery runs
    // against the "claimed" address in the quote body, so the
    // recovered signer won't match — attestation mismatch.
    let honest_addr: Address = "0x1111111111111111111111111111111111111111".parse().unwrap();
    let quote_body = quote_response_fixture("0x2222222222222222222222222222222222222222");
    // Sign for `honest_addr`, but the quote claims `0x2222...`.
    let (signature_hex, attestator_addr) = sign_attestation_for_quote(&quote_body, honest_addr);
    // Wait — this signs for `honest_addr` but the quote body deposits
    // to `0x2222...`. The provider will hash the body's `0x2222...`
    // into the message, so recovery returns a different address than
    // attestator_addr.

    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_body))
        .mount(&server)
        .await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/attestation"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "signature": signature_hex,
            "version":   1,
        })))
        .mount(&server)
        .await;

    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url: Some(server.uri()),
        attestator_address: attestator_addr,
        ..Default::default()
    });

    let err = provider.get_quote(&bridge_request_eth_to_btc()).await.unwrap_err();
    assert!(err.to_string().contains("attestation mismatch"), "unexpected: {err}");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_rejects_buy_orders() {
    let provider = NearIntentsBridgeProvider::default();
    let mut req = bridge_request_eth_to_btc();
    req.kind = OrderKind::Buy;
    let err = provider.get_quote(&req).await.unwrap_err();
    assert!(err.to_string().to_lowercase().contains("sell"));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_rejects_missing_bridge_recipient() {
    let provider = NearIntentsBridgeProvider::default();
    let mut req = bridge_request_eth_to_btc();
    req.bridge_recipient = None;
    req.receiver = None;
    let err = provider.get_quote(&req).await.unwrap_err();
    assert!(err.to_string().to_lowercase().contains("recipient"));
}

// ── Coverage follow-up ──────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_intermediate_tokens_returns_source_tokens_when_target_present() {
    // tokens_fixture() declares USDC on mainnet + BTC on non-EVM. When
    // the destination chain request asks for USDC on mainnet (same as
    // source), the target-has-buy-token check passes and the function
    // returns the source-chain tokens.
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v0/tokens"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tokens_fixture()))
        .mount(&server)
        .await;
    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url: Some(server.uri()),
        ..Default::default()
    });

    let mut req = bridge_request_eth_to_btc();
    // Both source and target on mainnet → USDC is on mainnet → ok.
    req.buy_chain_id = 1;
    req.buy_token = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
    let tokens = provider.get_intermediate_tokens(&req).await.unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].symbol, "USDC");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_intermediate_tokens_empty_when_target_missing() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v0/tokens"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tokens_fixture()))
        .mount(&server)
        .await;
    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url: Some(server.uri()),
        ..Default::default()
    });

    let mut req = bridge_request_eth_to_btc();
    req.buy_chain_id = 42_161; // Arbitrum — no token in fixture there
    req.buy_token = "0x1111111111111111111111111111111111111111".parse().unwrap();
    let tokens = provider.get_intermediate_tokens(&req).await.unwrap();
    assert!(tokens.is_empty());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_bridging_params_returns_none() {
    // Per the doc comment, this is wired up by PR #11. For now a call
    // should return `Ok(None)` regardless of the chain / order passed
    // in — we construct a minimal order via the mock helper.
    let provider = NearIntentsBridgeProvider::default();
    let order = cow_orderbook::api::mock_get_order(&format!("0x{}", "aa".repeat(56)));
    let result =
        provider.get_bridging_params(1, &order, alloy_primitives::B256::ZERO, None).await.unwrap();
    assert!(result.is_none());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_bridge_receiver_override_misses_on_unquoted_request() {
    // `get_quote` has not been called for this request shape, so the
    // cache is empty — the override must surface a clear error.
    use cow_bridging::{QuoteBridgeResponse, provider::ReceiverAccountBridgeProvider};
    let provider = NearIntentsBridgeProvider::default();
    let req = bridge_request_eth_to_btc();
    let response = QuoteBridgeResponse {
        provider: "near-intents".into(),
        sell_amount: U256::from(1u64),
        buy_amount: U256::from(1u64),
        fee_amount: U256::from(0u64),
        estimated_secs: 0,
        bridge_hook: None,
    };
    let err = provider.get_bridge_receiver_override(&req, &response).await.unwrap_err();
    assert!(err.to_string().contains("not in cache"), "unexpected: {err}");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_bridge_receiver_override_returns_cached_deposit_address_after_quote() {
    // Full quote → override round-trip. `get_quote` caches the
    // deposit address under the request-shape key;
    // `get_bridge_receiver_override` reads it back.
    use cow_bridging::{
        BridgeProvider, QuoteBridgeResponse, provider::ReceiverAccountBridgeProvider,
    };

    let deposit_address: Address = "0xdead000000000000000000000000000000000000".parse().unwrap();
    let quote_body = quote_response_fixture("0xdead000000000000000000000000000000000000");
    let (signature_hex, attestator_addr) = sign_attestation_for_quote(&quote_body, deposit_address);

    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_body))
        .mount(&server)
        .await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/attestation"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "signature": signature_hex,
            "version":   1,
        })))
        .mount(&server)
        .await;

    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url: Some(server.uri()),
        attestator_address: attestator_addr,
        ..Default::default()
    });

    let req = bridge_request_eth_to_btc();
    // Populate the cache.
    provider.get_quote(&req).await.unwrap();

    // Now the override must return the cached deposit address.
    let response = QuoteBridgeResponse {
        provider: "near-intents".into(),
        sell_amount: U256::from(1u64),
        buy_amount: U256::from(1u64),
        fee_amount: U256::from(0u64),
        estimated_secs: 0,
        bridge_hook: None,
    };
    let recovered = provider.get_bridge_receiver_override(&req, &response).await.unwrap();
    assert_eq!(recovered, "0xdead000000000000000000000000000000000000");
}

#[test]
fn deposit_cache_key_is_stable_for_the_same_request() {
    use cow_bridging::near_intents::NearDepositCacheKey;
    let req = bridge_request_eth_to_btc();
    let k1 = NearDepositCacheKey::from_request(&req);
    let k2 = NearDepositCacheKey::from_request(&req);
    assert_eq!(k1, k2);
}

#[test]
fn deposit_cache_key_differs_for_different_amounts() {
    use cow_bridging::near_intents::NearDepositCacheKey;
    let r1 = bridge_request_eth_to_btc();
    let mut r2 = r1.clone();
    r2.sell_amount = U256::from(2_000_000u64);
    assert_ne!(NearDepositCacheKey::from_request(&r1), NearDepositCacheKey::from_request(&r2));
}

#[test]
fn deposit_cache_handle_is_shared_across_clones() {
    use cow_bridging::near_intents::NearDepositCacheKey;
    let provider = NearIntentsBridgeProvider::default();
    let cloned = provider.clone();
    // Insert via the original's handle, read via the clone's handle.
    let req = bridge_request_eth_to_btc();
    let key = NearDepositCacheKey::from_request(&req);
    {
        let cache = provider.deposit_cache_handle();
        let mut guard = cache.lock().unwrap();
        guard.insert(key.clone(), "0xabc".into());
    }
    let cloned_cache = cloned.deposit_cache_handle();
    let guard = cloned_cache.lock().unwrap();
    assert_eq!(guard.get(&key).map(String::as_str), Some("0xabc"));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn with_api_key_forwards_bearer_on_tokens_endpoint() {
    // Exercises the `with_api_key` branch of the provider's api-init
    // helper (provider.rs line 114).
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v0/tokens"))
        .and(matchers::header("authorization", "Bearer provider-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tokens_fixture()))
        .mount(&server)
        .await;
    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url: Some(server.uri()),
        api_key: Some("provider-key".into()),
        ..Default::default()
    });
    let buy = provider
        .get_buy_tokens(cow_bridging::BuyTokensParams {
            sell_chain_id: 1,
            buy_chain_id: 1,
            sell_token_address: None,
        })
        .await
        .unwrap();
    assert!(!buy.tokens.is_empty());
}

#[test]
fn api_accessor_returns_underlying_client() {
    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url: Some("https://example.test".into()),
        ..Default::default()
    });
    assert_eq!(provider.api().base_url(), "https://example.test");
}

#[test]
fn options_accessor_returns_configuration() {
    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        validity_secs: 42,
        ..Default::default()
    });
    assert_eq!(provider.options().validity_secs, 42);
}

#[test]
fn chain_id_to_supported_maps_known_chain() {
    use cow_bridging::near_intents::chain_id_to_supported;
    use cow_chains::SupportedChainId;
    assert_eq!(chain_id_to_supported(1), Some(SupportedChainId::Mainnet));
    assert_eq!(chain_id_to_supported(0xDEAD_BEEF), None);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_rejects_malformed_deposit_address() {
    // API returns a quote with a deposit address that can't be parsed
    // as a 20-byte EVM address → provider surfaces `InvalidApiResponse`
    // via the attestation flow's `parse_evm_address` helper.
    let server = MockServer::start().await;
    let mut quote_body = quote_response_fixture("not-an-address");
    quote_body["quote"]["depositAddress"] = serde_json::Value::String("not-an-address".into());
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_body))
        .mount(&server)
        .await;
    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url: Some(server.uri()),
        ..Default::default()
    });
    let err = provider.get_quote(&bridge_request_eth_to_btc()).await.unwrap_err();
    assert!(err.to_string().to_lowercase().contains("deposit address"));
}
