#![allow(
    clippy::allow_attributes_without_reason,
    clippy::tests_outside_test_module,
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines
)]
//! Wiremock-backed integration tests for [`NearIntentsApi`].
//!
//! Exercises every endpoint with happy-path + error-path fixtures and
//! covers the `"amount is too low"` → `SellAmountTooSmall` mapping
//! that mirrors the TS SDK.

use cow_bridging::{
    BridgeError,
    near_intents::{
        NearIntentsApi,
        types::{
            NearAttestationRequest, NearDepositMode, NearDepositType, NearExecutionStatus,
            NearQuoteRequest, NearRecipientType, NearRefundType, NearSwapType,
        },
    },
};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

fn sample_quote_request() -> NearQuoteRequest {
    NearQuoteRequest {
        dry: false,
        swap_type: NearSwapType::ExactInput,
        deposit_mode: NearDepositMode::Simple,
        slippage_tolerance: 50,
        origin_asset: "nep141:eth".into(),
        deposit_type: NearDepositType::OriginChain,
        destination_asset: "nep141:btc".into(),
        amount: "1000000".into(),
        refund_to: "0xabc".into(),
        refund_type: NearRefundType::OriginChain,
        recipient: "bc1q...".into(),
        recipient_type: NearRecipientType::DestinationChain,
        deadline: "2099-01-01T00:00:00.000Z".into(),
        app_fees: None,
        quote_waiting_time_ms: None,
        referral: None,
        virtual_chain_recipient: None,
        virtual_chain_refund_recipient: None,
        custom_recipient_msg: None,
        session_id: None,
        connected_wallets: None,
    }
}

fn sample_tokens_response() -> serde_json::Value {
    serde_json::json!([
        {
            "assetId": "nep141:eth",
            "decimals": 18,
            "blockchain": "eth",
            "symbol": "ETH",
            "price": 4463.25,
            "priceUpdatedAt": "2025-09-05T12:00:38.695Z"
        },
        {
            "assetId": "nep141:usdc.e",
            "decimals": 6,
            "blockchain": "eth",
            "symbol": "USDC",
            "price": 1.0,
            "priceUpdatedAt": "2025-09-05T12:00:38.695Z",
            "contractAddress": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        },
        {
            "assetId": "nep141:btc",
            "decimals": 8,
            "blockchain": "btc",
            "symbol": "BTC",
            "price": 60000.0,
            "priceUpdatedAt": "2025-09-05T12:00:38.695Z"
        }
    ])
}

fn sample_quote_response_json() -> serde_json::Value {
    serde_json::json!({
        "quote": {
            "amountIn":            "1000000",
            "amountInFormatted":   "1.0",
            "amountInUsd":         "4463.25",
            "minAmountIn":         "1000000",
            "amountOut":           "1176580",
            "amountOutFormatted":  "0.011765806672337253",
            "amountOutUsd":        "700.0",
            "minAmountOut":        "1170000",
            "timeEstimate":        120,
            "deadline":            "2099-01-01T00:00:00.000Z",
            "timeWhenInactive":    "2099-01-01T01:00:00.000Z",
            "depositAddress":      "0xdead000000000000000000000000000000000000"
        },
        "quoteRequest": {
            "dry": false,
            "swapType": "EXACT_INPUT",
            "depositMode": "SIMPLE",
            "slippageTolerance": 50,
            "originAsset": "nep141:eth",
            "depositType": "ORIGIN_CHAIN",
            "destinationAsset": "nep141:btc",
            "amount": "1000000",
            "refundTo": "0xabc",
            "refundType": "ORIGIN_CHAIN",
            "recipient": "bc1q...",
            "recipientType": "DESTINATION_CHAIN",
            "deadline": "2099-01-01T00:00:00.000Z"
        },
        "signature": "ed25519:testSignature",
        "timestamp": "2025-09-05T12:00:40.000Z"
    })
}

fn sample_execution_status_json() -> serde_json::Value {
    serde_json::json!({
        "status": "SUCCESS",
        "updatedAt": "2025-09-05T12:00:50.000Z",
        "swapDetails": {
            "intentHashes": ["0xhash1"],
            "nearTxHashes": ["0xnear1"],
            "amountIn":            "1000000",
            "amountInFormatted":   "1.0",
            "amountInUsd":         "4463.25",
            "amountOut":           "1176580",
            "amountOutFormatted":  "0.011765806672337253",
            "amountOutUsd":        "700.0",
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

fn sample_attestation_response_json() -> serde_json::Value {
    serde_json::json!({
        "signature": "0x66edc32e2ab001213321ab7d959a2207fcef5190cc9abb6da5b0d2a8a9af2d4d2b0700e2c317c4106f337fd934fbbb0bf62efc8811a78603b33a8265d3b8f8cb1c",
        "version": 1
    })
}

// ── GET /v0/tokens ────────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_tokens_parses_happy_path() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v0/tokens"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_tokens_response()))
        .mount(&server)
        .await;

    let api = NearIntentsApi::new().with_base_url(server.uri());
    let tokens = api.get_tokens().await.unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].symbol, "ETH");
    assert!(tokens[0].contract_address.is_none(), "ETH is native — no contract address");
    assert_eq!(
        tokens[1].contract_address.as_deref(),
        Some("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
    );
    assert_eq!(tokens[2].blockchain, "btc");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_tokens_maps_5xx_to_api_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v0/tokens"))
        .respond_with(ResponseTemplate::new(503).set_body_string("unavailable"))
        .mount(&server)
        .await;

    let api = NearIntentsApi::new().with_base_url(server.uri());
    let err = api.get_tokens().await.unwrap_err();
    if let BridgeError::ApiError(msg) = err {
        assert!(msg.contains("503"));
    } else {
        panic!("expected ApiError, got {err:?}");
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_tokens_maps_malformed_body_to_invalid_api_response() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v0/tokens"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let api = NearIntentsApi::new().with_base_url(server.uri());
    let err = api.get_tokens().await.unwrap_err();
    assert!(matches!(err, BridgeError::InvalidApiResponse(_)), "got {err:?}");
}

// ── POST /v0/quote ────────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_parses_happy_path() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_quote_response_json()))
        .mount(&server)
        .await;

    let api = NearIntentsApi::new().with_base_url(server.uri());
    let resp = api.get_quote(&sample_quote_request()).await.unwrap();
    assert_eq!(resp.quote.amount_in, "1000000");
    assert_eq!(resp.quote.deposit_address, "0xdead000000000000000000000000000000000000");
    assert_eq!(resp.signature, "ed25519:testSignature");
    assert_eq!(resp.quote_request.swap_type, NearSwapType::ExactInput);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_maps_amount_too_low_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/quote"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_string("{\"error\":\"amount is too low, try at least 10000\"}"),
        )
        .mount(&server)
        .await;

    let api = NearIntentsApi::new().with_base_url(server.uri());
    let err = api.get_quote(&sample_quote_request()).await.unwrap_err();
    assert!(matches!(err, BridgeError::SellAmountTooSmall), "got {err:?}");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_amount_too_low_matches_case_insensitively() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/quote"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_string("{\"error\":\"Amount Is Too Low for this bridge route\"}"),
        )
        .mount(&server)
        .await;

    let api = NearIntentsApi::new().with_base_url(server.uri());
    let err = api.get_quote(&sample_quote_request()).await.unwrap_err();
    assert!(matches!(err, BridgeError::SellAmountTooSmall), "got {err:?}");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_other_errors_do_not_map_to_sell_amount_too_small() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/quote"))
        .respond_with(ResponseTemplate::new(400).set_body_string("unrelated error"))
        .mount(&server)
        .await;

    let api = NearIntentsApi::new().with_base_url(server.uri());
    let err = api.get_quote(&sample_quote_request()).await.unwrap_err();
    assert!(matches!(err, BridgeError::ApiError(_)), "got {err:?}");
}

// ── GET /v0/execution-status/{addr} ──────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_execution_status_parses_success() {
    let server = MockServer::start().await;
    let addr = "0xdead000000000000000000000000000000000000";
    Mock::given(matchers::method("GET"))
        .and(matchers::path(format!("/v0/execution-status/{addr}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_execution_status_json()))
        .mount(&server)
        .await;

    let api = NearIntentsApi::new().with_base_url(server.uri());
    let resp = api.get_execution_status(addr).await.unwrap();
    assert_eq!(resp.status, NearExecutionStatus::Success);
    assert_eq!(resp.swap_details.intent_hashes, vec!["0xhash1"]);
    assert_eq!(resp.swap_details.origin_chain_tx_hashes[0].hash, "0xorigin1");
    assert!(resp.quote_response.is_none());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_execution_status_propagates_404() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v0/execution-status/0xnotfound"))
        .respond_with(ResponseTemplate::new(404).set_body_string("deposit not found"))
        .mount(&server)
        .await;

    let api = NearIntentsApi::new().with_base_url(server.uri());
    let err = api.get_execution_status("0xnotfound").await.unwrap_err();
    if let BridgeError::ApiError(msg) = err {
        assert!(msg.contains("404"));
        assert!(msg.contains("deposit not found"));
    } else {
        panic!("expected ApiError, got {err:?}");
    }
}

// ── POST /v0/attestation ──────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_attestation_parses_happy_path() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/attestation"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_attestation_response_json()))
        .mount(&server)
        .await;

    let api = NearIntentsApi::new().with_base_url(server.uri());
    let req = NearAttestationRequest {
        deposit_address: "0xdead000000000000000000000000000000000000".into(),
        quote_hash: "0xabc".into(),
    };
    let resp = api.get_attestation(&req).await.unwrap();
    assert!(resp.signature.starts_with("0x"));
    assert_eq!(resp.signature.len(), 2 + 65 * 2);
    assert_eq!(resp.version, 1);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_attestation_maps_5xx_to_api_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v0/attestation"))
        .respond_with(ResponseTemplate::new(502).set_body_string("bad gateway"))
        .mount(&server)
        .await;

    let api = NearIntentsApi::new().with_base_url(server.uri());
    let req = NearAttestationRequest {
        deposit_address: "0xdead000000000000000000000000000000000000".into(),
        quote_hash: "0xabc".into(),
    };
    let err = api.get_attestation(&req).await.unwrap_err();
    assert!(matches!(err, BridgeError::ApiError(_)), "got {err:?}");
}

// ── Bearer auth ───────────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn api_key_is_forwarded_as_bearer_token() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v0/tokens"))
        .and(matchers::header("authorization", "Bearer test-key-42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_tokens_response()))
        .mount(&server)
        .await;
    // Without the Authorization header the mock wouldn't match and the
    // request would 404, turning the assertion below into a failure.
    let api = NearIntentsApi::new().with_base_url(server.uri()).with_api_key("test-key-42");
    let tokens = api.get_tokens().await.unwrap();
    assert!(!tokens.is_empty());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn no_api_key_omits_authorization_header() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v0/tokens"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_tokens_response()))
        .mount(&server)
        .await;

    let api = NearIntentsApi::new().with_base_url(server.uri());
    let tokens = api.get_tokens().await.unwrap();
    assert!(!tokens.is_empty());
    // Inspect the recorded request to confirm no Authorization header
    // was sent.
    let received = server.received_requests().await.expect("request capture enabled");
    assert!(!received.is_empty(), "mock server should have seen 1+ requests");
    let last = &received[0];
    assert!(
        last.headers.get("authorization").is_none(),
        "unexpected Authorization header: {:?}",
        last.headers.get("authorization"),
    );
}
