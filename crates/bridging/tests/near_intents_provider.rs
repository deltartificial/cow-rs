#![allow(
    clippy::allow_attributes_without_reason,
    clippy::tests_outside_test_module,
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines
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
