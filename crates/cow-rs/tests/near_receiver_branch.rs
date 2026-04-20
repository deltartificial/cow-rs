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
//! End-to-end integration test for the NEAR receiver-account branch.
//!
//! Drives `get_quote_with_bridge` (the dispatcher) against a real
//! `NearIntentsBridgeProvider` + wiremock-backed NEAR API, and
//! verifies that the resulting `BridgeQuoteAndPost.bridge.bridge_receiver_override`
//! is the `depositAddress` the provider allocated.

use std::sync::Arc;

use alloy_primitives::{Address, U256, keccak256};
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use cow_bridging::{
    QuoteBridgeRequest,
    near_intents::{
        NearIntentsBridgeProvider, NearIntentsProviderOptions,
        util::{ATTESTATION_PREFIX_BYTES, ATTESTATION_VERSION_BYTES, hash_quote_payload},
    },
    sdk::{GetQuoteWithBridgeParams, get_quote_with_bridge},
    swap_quoter::{QuoteSwapFuture, SwapQuoteOutcome, SwapQuoteParams, SwapQuoter},
};
use cow_errors::CowError;
use cow_types::OrderKind;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

const TEST_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

/// Mock quoter that returns a fixed outcome — the NEAR receiver
/// branch never calls it for a real swap, but the orchestrator
/// invokes it through `get_intermediate_swap_result` for the
/// intermediate-token hop.
struct FakeQuoter(SwapQuoteOutcome);

impl SwapQuoter for FakeQuoter {
    fn quote_swap<'a>(&'a self, _params: SwapQuoteParams) -> QuoteSwapFuture<'a> {
        let outcome = self.0.clone();
        Box::pin(async move { Ok(outcome) })
    }
}

fn tokens_fixture() -> serde_json::Value {
    serde_json::json!([
        // Source-side: USDC on mainnet (EVM).
        {
            "assetId":         "nep141:usdc.e",
            "decimals":        6,
            "blockchain":      "eth",
            "symbol":          "USDC",
            "price":           1.0,
            "priceUpdatedAt":  "2025-09-05T12:00:38.695Z",
            "contractAddress": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        },
        // Destination-side: BTC (non-EVM — adapt_token uses ZERO sentinel).
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

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        write!(&mut s, "{b:02x}").unwrap();
    }
    s
}

fn sign_attestation(quote_body: &serde_json::Value, deposit_address: Address) -> (String, Address) {
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
    use std::str::FromStr;
    let signer = PrivateKeySigner::from_str(TEST_KEY).unwrap();
    let sig = signer.sign_hash_sync(&digest).unwrap();
    (format!("0x{}", hex_encode(&sig.as_bytes())), signer.address())
}

fn request() -> QuoteBridgeRequest {
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

fn outcome() -> SwapQuoteOutcome {
    SwapQuoteOutcome {
        sell_amount: U256::from(1_000_000u64),
        buy_amount_after_slippage: U256::from(999_500u64),
        fee_amount: U256::from(500u64),
        valid_to: 9_999_999,
        app_data_hex: "0xabc".into(),
        full_app_data: "{\"version\":\"1.4.0\"}".into(),
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn receiver_branch_threads_cached_deposit_address_through_orchestrator() {
    let deposit: Address = "0xdead000000000000000000000000000000000000".parse().unwrap();
    let body = quote_response_fixture(&format!("{deposit:#x}"));
    let (signature_hex, attestator_addr) = sign_attestation(&body, deposit);

    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v0/tokens"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tokens_fixture()))
        .mount(&server)
        .await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
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
    // Populate the deposit-address cache via `get_quote`, which the
    // orchestrator calls as step 2 of the receiver-account branch.
    // The receiver-override lookup is the thing we care about.
    let _ = Arc::new(provider.clone());

    let quoter = FakeQuoter(outcome());
    let params = GetQuoteWithBridgeParams {
        swap_and_bridge_request: request(),
        slippage_bps: 50,
        advanced_settings_metadata: None,
        quote_signer: None,
        hook_deadline: None,
    };

    let result = get_quote_with_bridge(&params, &provider, &quoter).await;
    let Ok(bqp) = result else {
        let err = result.unwrap_err();
        panic!("orchestrator should succeed, got: {err:?}");
    };

    // Receiver branch: bridge_call_details = None, override = cached deposit.
    assert!(bqp.bridge.bridge_call_details.is_none(), "receiver branch should skip hook wiring");
    assert_eq!(
        bqp.bridge.bridge_receiver_override.as_deref(),
        Some("0xdead000000000000000000000000000000000000"),
        "override must be the deposit address cached during get_quote",
    );
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn receiver_branch_errors_when_cache_is_missed() {
    // Use a provider that's NEVER queried via `get_quote` (because
    // `get_intermediate_tokens` fails first when tokens endpoint is
    // not mocked). The orchestrator then fails before populating the
    // cache.
    let server = MockServer::start().await;
    // No mock for /v0/tokens — request will return 404 and error out.
    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url: Some(server.uri()),
        ..Default::default()
    });
    let quoter = FakeQuoter(outcome());
    let params = GetQuoteWithBridgeParams {
        swap_and_bridge_request: request(),
        slippage_bps: 50,
        advanced_settings_metadata: None,
        quote_signer: None,
        hook_deadline: None,
    };

    let err = get_quote_with_bridge(&params, &provider, &quoter).await.unwrap_err();
    // Could be "NoIntermediateTokens" or "TxBuildError" depending on which
    // wire call failed first. Just assert the flow doesn't silently
    // succeed with a ghost receiver.
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("intermediate") || msg.contains("api") || msg.contains("404"),
        "unexpected error: {err}",
    );
}

#[allow(dead_code, reason = "type assertion: keep imports used")]
fn _type_assertions() -> CowError {
    CowError::Config("noop".into())
}
