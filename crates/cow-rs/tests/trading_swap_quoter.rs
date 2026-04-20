#![allow(
    clippy::allow_attributes_without_reason,
    clippy::tests_outside_test_module,
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::assertions_on_constants,
    clippy::missing_assert_message,
    clippy::unwrap_used,
    clippy::expect_used
)]
//! Wiremock-based integration tests for the [`TradingSwapQuoter`] adapter.
//!
//! These cover the async path of `TradingSwapQuoter::quote_swap`, which is
//! skipped by the unit tests in `trading_swap_quoter.rs` because they only
//! exercise construction + trait-object coercion.

use std::sync::Arc;

use alloy_primitives::{Address, U256};
use cow_rs::{
    OrderKind, SupportedChainId, TradingSdk, TradingSdkConfig,
    bridging::{SwapQuoteParams, SwapQuoter},
    trading_swap_quoter::TradingSwapQuoter,
};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

const TEST_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const SELL_TOKEN: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
const BUY_TOKEN: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";

fn make_quoter(server: &MockServer) -> TradingSwapQuoter {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new_with_url(config, TEST_KEY, server.uri()).expect("valid test key");
    TradingSwapQuoter::new(Arc::new(sdk))
}

fn sample_params() -> SwapQuoteParams {
    SwapQuoteParams {
        owner: "0x2222222222222222222222222222222222222222".parse().unwrap(),
        chain_id: 1,
        sell_token: SELL_TOKEN.parse().unwrap(),
        sell_token_decimals: 6,
        buy_token: BUY_TOKEN.parse().unwrap(),
        buy_token_decimals: 18,
        amount: U256::from(1_000_000u64),
        kind: OrderKind::Sell,
        slippage_bps: 50,
        app_data_json: None,
    }
}

fn quote_response_json() -> serde_json::Value {
    serde_json::json!({
        "quote": {
            "sellToken":         SELL_TOKEN,
            "buyToken":          BUY_TOKEN,
            "receiver":          null,
            "sellAmount":        "1000000",
            "buyAmount":         "500000000000000",
            "validTo":           9_999_999,
            "appData":           "0x0000000000000000000000000000000000000000000000000000000000000000",
            "feeAmount":         "1000",
            "kind":              "sell",
            "partiallyFillable": false,
            "sellTokenBalance":  "erc20",
            "buyTokenBalance":   "erc20"
        },
        "from":       "0x0000000000000000000000000000000000000000",
        "expiration": "2099-01-01T00:00:00.000Z",
        "id":         42,
        "verified":   false
    })
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn quote_swap_returns_outcome_without_app_data() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_response_json()))
        .mount(&server)
        .await;

    let quoter = make_quoter(&server);
    let outcome = quoter.quote_swap(sample_params()).await.unwrap();

    // Outcome field mapping from QuoteResults.
    assert_eq!(outcome.sell_amount, U256::from(1_000_000u64));
    assert!(!outcome.buy_amount_after_slippage.is_zero());
    // The trading SDK rewrites `valid_to` using `now + validity`, so we
    // just sanity-check that it's populated.
    assert!(outcome.valid_to > 0);
    assert!(outcome.app_data_hex.starts_with("0x"));
    assert!(!outcome.full_app_data.is_empty());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn quote_swap_passes_caller_app_data_as_settings() {
    // When the caller threads an app_data_json through, the adapter must
    // wrap it in a SwapAdvancedSettings before calling get_quote_only_with_settings.
    // Verify the call succeeds and the response is unwrapped correctly.
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_response_json()))
        .mount(&server)
        .await;

    let quoter = make_quoter(&server);
    let app_data = serde_json::json!({
        "version": "1.4.0",
        "appCode": "CoW Bridging",
        "metadata": { "bridging": { "providerId": "cow-sdk://bridging/providers/foo" } }
    });
    let mut params = sample_params();
    params.app_data_json = Some(serde_json::to_string(&app_data).unwrap());

    let outcome = quoter.quote_swap(params).await.unwrap();
    assert_eq!(outcome.sell_amount, U256::from(1_000_000u64));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn quote_swap_rejects_malformed_app_data_json() {
    // The adapter parses the caller-supplied app_data_json via serde_json.
    // Malformed JSON must surface as CowError::AppData, not panic or swallow.
    let server = MockServer::start().await;
    // No mock mounted — we should fail before hitting the network.

    let quoter = make_quoter(&server);
    let mut params = sample_params();
    params.app_data_json = Some("{ this is not json".to_owned());

    let err = quoter.quote_swap(params).await.unwrap_err();
    assert!(
        err.to_string().contains("app-data") || err.to_string().contains("invalid"),
        "unexpected error: {err}"
    );
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn quote_swap_propagates_orderbook_400() {
    // Use a 400 instead of 5xx because the orderbook client retries on
    // 5xx (making the test take ~50s). 400 is not retried and surfaces
    // immediately.
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
        .mount(&server)
        .await;

    let quoter = make_quoter(&server);
    let err = quoter.quote_swap(sample_params()).await.unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("400") || err.to_string().contains("bad request"),
        "unexpected err: {err}"
    );
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn quote_swap_owner_is_threaded_to_trading_sdk() {
    // get_quote_only uses the owner for `receiver` when no custom receiver
    // is set on the TradeParameters. The adapter sets receiver=None so the
    // trading SDK should fall back to the owner. Verify by reading the
    // request body via wiremock's capture and asserting the `from` field
    // (passed by the SDK on POST /api/v1/quote) matches our owner.
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_response_json()))
        .mount(&server)
        .await;

    let quoter = make_quoter(&server);
    let mut params = sample_params();
    let custom_owner: Address = "0x7777777777777777777777777777777777777777".parse().unwrap();
    params.owner = custom_owner;
    quoter.quote_swap(params).await.unwrap();

    // Fetch captured requests from the mock server and assert the body
    // contains the owner as the `from` field.
    let requests = server.received_requests().await.expect("requests capture enabled");
    let body_text = String::from_utf8(requests[0].body.clone()).unwrap();
    assert!(
        body_text.to_lowercase().contains(&format!("{custom_owner:#x}")),
        "request body should mention owner; got: {body_text}"
    );
}
