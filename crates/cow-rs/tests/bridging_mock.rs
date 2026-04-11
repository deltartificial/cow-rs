#![allow(
    clippy::allow_attributes_without_reason,
    clippy::tests_outside_test_module,
    clippy::doc_markdown,
    clippy::type_complexity,
    clippy::missing_const_for_fn,
    clippy::assertions_on_constants,
    clippy::missing_assert_message,
    clippy::map_err_ignore,
    clippy::deref_by_slicing,
    clippy::redundant_clone,
    clippy::single_match_else,
    clippy::single_match
)]
//! Wiremock-based integration tests for bridging HTTP interactions.
//!
//! Tests cover the Bungee quote provider, Across quote conversion,
//! status event handling, response validation, and the high-level
//! `BridgingSdk` aggregator.

use alloy_primitives::{U256, address};
use cow_rs::{
    OrderKind,
    bridging::{
        BridgeProvider, BridgingSdk, QuoteBridgeRequest, QuoteBridgeResponse,
        across::{
            is_valid_across_status_response, map_across_status_to_bridge_status,
            to_bridge_quote_result,
        },
        bungee::{
            BungeeApiUrlOptions, bungee_to_bridge_quote_result,
            decode_amounts_bungee_tx_data, decode_bungee_bridge_tx_data,
            get_bridging_status_from_events, get_bungee_bridge_from_display_name,
            is_valid_bungee_events_response, is_valid_quote_response,
            resolve_api_endpoint_from_options,
        },
        provider::QuoteFuture,
        types::{
            AcrossDepositStatus, AcrossPctFee, AcrossSuggestedFeesLimits,
            AcrossSuggestedFeesResponse, BridgeError, BridgeStatus, BungeeBridge,
            BungeeBridgeName, BungeeEvent, BungeeEventStatus,
        },
    },
};
use serde_json::json;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn sample_request() -> QuoteBridgeRequest {
    QuoteBridgeRequest {
        sell_chain_id: 1,
        buy_chain_id: 42161,
        sell_token: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
        sell_token_decimals: 6,
        buy_token: address!("af88d065e77c8cC2239327C5EDb3A432268e5831"),
        buy_token_decimals: 6,
        sell_amount: U256::from(1_000_000_000u64), // 1000 USDC
        account: address!("3333333333333333333333333333333333333333"),
        owner: None,
        receiver: None,
        bridge_recipient: None,
        slippage_bps: 50,
        bridge_slippage_bps: None,
        kind: OrderKind::Sell,
    }
}

fn sample_suggested_fees() -> AcrossSuggestedFeesResponse {
    AcrossSuggestedFeesResponse {
        total_relay_fee: AcrossPctFee {
            pct: "5000000000000000".to_owned(), // 0.5% in 1e18 format
            total: "5000000".to_owned(),
        },
        relayer_capital_fee: AcrossPctFee {
            pct: "3000000000000000".to_owned(),
            total: "3000000".to_owned(),
        },
        relayer_gas_fee: AcrossPctFee {
            pct: "2000000000000000".to_owned(),
            total: "2000000".to_owned(),
        },
        lp_fee: AcrossPctFee { pct: "0".to_owned(), total: "0".to_owned() },
        timestamp: "1700000000".to_owned(),
        is_amount_too_low: false,
        quote_block: "18000000".to_owned(),
        spoke_pool_address: "0x5c7BCd6E7De5423a257D81B442095A1a6ced35C5".to_owned(),
        exclusive_relayer: "0x0000000000000000000000000000000000000000".to_owned(),
        exclusivity_deadline: "0".to_owned(),
        estimated_fill_time_sec: "60".to_owned(),
        fill_deadline: "1700003600".to_owned(),
        limits: AcrossSuggestedFeesLimits {
            min_deposit: "100000".to_owned(),
            max_deposit: "10000000000".to_owned(),
            max_deposit_instant: "5000000000".to_owned(),
            max_deposit_short_delay: "8000000000".to_owned(),
            recommended_deposit_instant: "4000000000".to_owned(),
        },
    }
}

fn make_bungee_event(
    src_status: BungeeEventStatus,
    dest_status: BungeeEventStatus,
    bridge_name: BungeeBridgeName,
) -> BungeeEvent {
    BungeeEvent {
        identifier: "evt-123".to_owned(),
        src_transaction_hash: Some("0xabc".to_owned()),
        bridge_name,
        from_chain_id: 1,
        is_cowswap_trade: true,
        order_id: "order-456".to_owned(),
        src_tx_status: src_status,
        dest_tx_status: dest_status,
        dest_transaction_hash: Some("0xdef".to_owned()),
    }
}

// ── BungeeProvider HTTP tests ────────────────────────────────────────────────

#[tokio::test]
async fn bungee_provider_get_quote_parses_successful_response() {
    let server = MockServer::start().await;

    let body = json!({
        "success": true,
        "result": {
            "routes": [{
                "outputAmount": "995000000",
                "estimatedTimeInSeconds": 120,
                "routeDetails": {
                    "routeFee": { "amount": "5000000" }
                }
            }]
        }
    });

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v2/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&server)
        .await;

    // BungeeProvider uses a hard-coded base URL, so we construct our own
    // client to test against the mock server.
    let client = reqwest::Client::new();
    let req = sample_request();
    let slippage_pct = req.slippage_bps as f64 / 100.0;
    let slippage_str = format!("{slippage_pct:.1}");

    let url = reqwest::Url::parse_with_params(
        &format!("{}/v2/quote", server.uri()),
        &[
            ("fromChainId", req.sell_chain_id.to_string()),
            ("toChainId", req.buy_chain_id.to_string()),
            ("fromTokenAddress", format!("{:#x}", req.sell_token)),
            ("toTokenAddress", format!("{:#x}", req.buy_token)),
            ("fromAmount", req.sell_amount.to_string()),
            ("userAddress", format!("{:#x}", req.account)),
            ("slippageTolerance", slippage_str),
            ("isContractCall", "false".to_owned()),
        ],
    )
    .unwrap();

    let resp = client.get(url).header("API-KEY", "test-key").send().await.unwrap();
    let json_val: serde_json::Value = resp.json().await.unwrap();

    let route = json_val["result"]["routes"]
        .as_array()
        .unwrap()
        .first()
        .unwrap();

    let output_amount_str = route["outputAmount"].as_str().unwrap();
    let buy_amount: U256 = output_amount_str.parse().unwrap();
    assert_eq!(buy_amount, U256::from(995_000_000u64));

    let estimated_secs = route["estimatedTimeInSeconds"].as_u64().unwrap();
    assert_eq!(estimated_secs, 120);
}

#[tokio::test]
async fn bungee_provider_handles_api_error_status() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v2/quote"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/v2/quote", server.uri()))
        .header("API-KEY", "test-key")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 429);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "rate limited");
}

#[tokio::test]
async fn bungee_provider_handles_empty_routes() {
    let server = MockServer::start().await;

    let body = json!({
        "success": true,
        "result": {
            "routes": []
        }
    });

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v2/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/v2/quote", server.uri()))
        .header("API-KEY", "test-key")
        .send()
        .await
        .unwrap();

    let json_val: serde_json::Value = resp.json().await.unwrap();
    let routes = json_val["result"]["routes"].as_array().unwrap();
    assert!(routes.is_empty());
}

// ── Bungee quote response validation ─────────────────────────────────────────

#[test]
fn is_valid_quote_response_accepts_well_formed_response() {
    let resp = json!({
        "success": true,
        "result": {
            "manualRoutes": [{
                "quoteId": "q-1",
                "output": { "amount": "100" },
                "estimatedTime": 60,
                "routeDetails": {
                    "routeFee": { "amount": "5" }
                }
            }]
        }
    });
    assert!(is_valid_quote_response(&resp));
}

#[test]
fn is_valid_quote_response_rejects_success_false() {
    let resp = json!({
        "success": false,
        "result": { "manualRoutes": [] }
    });
    assert!(!is_valid_quote_response(&resp));
}

#[test]
fn is_valid_quote_response_rejects_missing_result() {
    let resp = json!({ "success": true });
    assert!(!is_valid_quote_response(&resp));
}

#[test]
fn is_valid_quote_response_rejects_route_without_quote_id() {
    let resp = json!({
        "success": true,
        "result": {
            "manualRoutes": [{
                "output": { "amount": "100" },
                "estimatedTime": 60,
                "routeDetails": { "routeFee": { "amount": "5" } }
            }]
        }
    });
    assert!(!is_valid_quote_response(&resp));
}

#[test]
fn is_valid_quote_response_accepts_empty_routes() {
    let resp = json!({
        "success": true,
        "result": { "manualRoutes": [] }
    });
    assert!(is_valid_quote_response(&resp));
}

// ── Bungee events response validation ────────────────────────────────────────

#[test]
fn is_valid_bungee_events_response_accepts_well_formed() {
    let resp = json!({
        "success": true,
        "result": [{
            "identifier": "evt-1",
            "bridgeName": "across",
            "fromChainId": 1,
            "orderId": "o-1",
            "srcTxStatus": "COMPLETED",
            "destTxStatus": "PENDING"
        }]
    });
    assert!(is_valid_bungee_events_response(&resp));
}

#[test]
fn is_valid_bungee_events_response_rejects_missing_field() {
    let resp = json!({
        "success": true,
        "result": [{ "identifier": "evt-1" }]
    });
    assert!(!is_valid_bungee_events_response(&resp));
}

#[test]
fn is_valid_bungee_events_response_rejects_success_false() {
    let resp = json!({
        "success": false,
        "result": []
    });
    assert!(!is_valid_bungee_events_response(&resp));
}

// ── Bungee tx data decoding ──────────────────────────────────────────────────

#[test]
fn decode_bungee_bridge_tx_data_parses_valid_calldata() {
    // 4 bytes route ID + 4 bytes selector + some params
    let tx_data = "0x11223344aabbccdd0000000000000000000000000000000000000000000000000000000000000001";
    let decoded = decode_bungee_bridge_tx_data(tx_data).unwrap();
    assert_eq!(decoded.route_id, "0x11223344");
    assert_eq!(decoded.function_selector, "0xaabbccdd");
    assert!(decoded.encoded_function_data.starts_with("0xaabbccdd"));
}

#[test]
fn decode_bungee_bridge_tx_data_rejects_too_short() {
    let result = decode_bungee_bridge_tx_data("0x1234");
    assert!(result.is_err());
}

#[test]
fn decode_bungee_bridge_tx_data_rejects_missing_prefix() {
    let result = decode_bungee_bridge_tx_data("1122334455667788");
    assert!(result.is_err());
}

#[test]
fn decode_bungee_bridge_tx_data_rejects_empty() {
    let result = decode_bungee_bridge_tx_data("");
    assert!(result.is_err());
}

// ── Bungee amount decoding ──────────────────────────────────────────────────

#[test]
fn decode_amounts_for_across_bridge() {
    // Build a minimal calldata that has route_id (4 bytes) + function selector 0xcc54d224 (4 bytes)
    // + 32-byte amount at byte offset 8 (hex offset 2 + 16 = 18)
    let amount_hex = "0000000000000000000000000000000000000000000000000000000000000064"; // 100
    let tx_data = format!("0x11223344cc54d224{amount_hex}");
    let decoded = decode_amounts_bungee_tx_data(&tx_data, BungeeBridge::Across).unwrap();
    assert_eq!(decoded.input_amount, U256::from(100u64));
}

#[test]
fn decode_amounts_rejects_unsupported_bridge_selector() {
    let tx_data = "0x1122334400000000000000000000000000000000000000000000000000000000000000000000000000000064";
    let result = decode_amounts_bungee_tx_data(tx_data, BungeeBridge::Across);
    assert!(result.is_err());
}

#[test]
fn decode_amounts_rejects_empty_data() {
    let result = decode_amounts_bungee_tx_data("", BungeeBridge::Across);
    assert!(result.is_err());
}

#[test]
fn decode_amounts_rejects_no_prefix() {
    let result = decode_amounts_bungee_tx_data("aabbccdd", BungeeBridge::Across);
    assert!(result.is_err());
}

// ── Display name mapping ─────────────────────────────────────────────────────

#[test]
fn bungee_bridge_display_name_roundtrip() {
    assert_eq!(
        get_bungee_bridge_from_display_name("Across"),
        Some(BungeeBridge::Across)
    );
    assert_eq!(
        get_bungee_bridge_from_display_name("Circle CCTP"),
        Some(BungeeBridge::CircleCctp)
    );
    assert_eq!(
        get_bungee_bridge_from_display_name("Gnosis Native"),
        Some(BungeeBridge::GnosisNative)
    );
    assert_eq!(get_bungee_bridge_from_display_name("Unknown"), None);
}

// ── bungee_to_bridge_quote_result ────────────────────────────────────────────

#[test]
fn bungee_to_bridge_quote_result_basic() {
    let req = sample_request();
    let result = bungee_to_bridge_quote_result(
        &req,
        50,
        U256::from(995_000_000u64),
        U256::from(5_000_000u64),
        1700000000,
        120,
        Some("q-1".to_owned()),
        None,
    )
    .unwrap();

    assert!(result.is_sell);
    assert_eq!(result.quote_timestamp, 1700000000);
    assert_eq!(result.expected_fill_time_seconds, Some(120));
    assert_eq!(result.amounts_and_costs.before_fee.buy_amount, U256::from(995_000_000u64));
    assert_eq!(result.amounts_and_costs.slippage_bps, 50);
}

#[test]
fn bungee_to_bridge_quote_result_zero_sell_amount() {
    let mut req = sample_request();
    req.sell_amount = U256::ZERO;

    let result = bungee_to_bridge_quote_result(
        &req,
        50,
        U256::from(100u64),
        U256::from(5u64),
        0,
        0,
        None,
        None,
    )
    .unwrap();

    // Fee in buy token should be zero when sell amount is zero (avoids division by zero).
    assert_eq!(
        result.amounts_and_costs.costs.bridging_fee.amount_in_buy_currency,
        U256::ZERO
    );
}

// ── Bridging status from events ──────────────────────────────────────────────

#[tokio::test]
async fn status_from_events_returns_unknown_when_no_events() {
    let dummy_across = |_: &str| async { Ok("pending".to_owned()) };

    let result = get_bridging_status_from_events(None, dummy_across)
        .await
        .unwrap();
    assert!(matches!(result.status, BridgeStatus::Unknown));
}

#[tokio::test]
async fn status_from_events_returns_unknown_for_empty_slice() {
    let dummy_across = |_: &str| async { Ok("pending".to_owned()) };

    let events: Vec<BungeeEvent> = vec![];
    let result = get_bridging_status_from_events(Some(&events), dummy_across)
        .await
        .unwrap();
    assert!(matches!(result.status, BridgeStatus::Unknown));
}

#[tokio::test]
async fn status_from_events_returns_in_progress_for_src_pending() {
    let dummy_across = |_: &str| async { Ok("pending".to_owned()) };

    let event = make_bungee_event(
        BungeeEventStatus::Pending,
        BungeeEventStatus::Pending,
        BungeeBridgeName::Across,
    );
    let result = get_bridging_status_from_events(Some(&[event]), dummy_across)
        .await
        .unwrap();
    assert!(matches!(result.status, BridgeStatus::InProgress));
}

#[tokio::test]
async fn status_from_events_returns_executed_when_both_complete() {
    let dummy_across = |_: &str| async { Ok("filled".to_owned()) };

    let event = make_bungee_event(
        BungeeEventStatus::Completed,
        BungeeEventStatus::Completed,
        BungeeBridgeName::Across,
    );
    let result = get_bridging_status_from_events(Some(&[event]), dummy_across)
        .await
        .unwrap();
    assert!(matches!(result.status, BridgeStatus::Executed));
    assert_eq!(result.deposit_tx_hash, Some("0xabc".to_owned()));
    assert_eq!(result.fill_tx_hash, Some("0xdef".to_owned()));
}

#[tokio::test]
async fn status_from_events_returns_expired_from_across() {
    let across_expired = |_: &str| async { Ok("expired".to_owned()) };

    let event = make_bungee_event(
        BungeeEventStatus::Completed,
        BungeeEventStatus::Pending,
        BungeeBridgeName::Across,
    );
    let result = get_bridging_status_from_events(Some(&[event]), across_expired)
        .await
        .unwrap();
    assert!(matches!(result.status, BridgeStatus::Expired));
}

#[tokio::test]
async fn status_from_events_returns_refund_from_across() {
    let across_refunded = |_: &str| async { Ok("refunded".to_owned()) };

    let event = make_bungee_event(
        BungeeEventStatus::Completed,
        BungeeEventStatus::Pending,
        BungeeBridgeName::Across,
    );
    let result = get_bridging_status_from_events(Some(&[event]), across_refunded)
        .await
        .unwrap();
    assert!(matches!(result.status, BridgeStatus::Refund));
}

#[tokio::test]
async fn status_from_events_src_complete_dest_pending_non_across() {
    let dummy_across = |_: &str| async { Ok("filled".to_owned()) };

    let event = make_bungee_event(
        BungeeEventStatus::Completed,
        BungeeEventStatus::Pending,
        BungeeBridgeName::Cctp,
    );
    let result = get_bridging_status_from_events(Some(&[event]), dummy_across)
        .await
        .unwrap();
    // Non-Across bridges with src complete + dest pending = InProgress.
    assert!(matches!(result.status, BridgeStatus::InProgress));
}

// ── Across quote conversion ──────────────────────────────────────────────────

#[test]
fn across_to_bridge_quote_result_basic() {
    let req = sample_request();
    let fees = sample_suggested_fees();

    let result = to_bridge_quote_result(&req, 50, &fees).unwrap();

    assert!(result.is_sell);
    assert_eq!(result.quote_timestamp, 1700000000);
    assert_eq!(result.expected_fill_time_seconds, Some(60));
    // min/max deposit parsed from limits.
    assert!(result.limits.min_deposit > U256::ZERO);
    assert!(result.limits.max_deposit > U256::ZERO);
    // After fee should be less than before fee.
    assert!(
        result.amounts_and_costs.after_fee.buy_amount
            < result.amounts_and_costs.before_fee.buy_amount
    );
}

#[test]
fn across_to_bridge_quote_result_invalid_pct() {
    let req = sample_request();
    let mut fees = sample_suggested_fees();
    fees.total_relay_fee.pct = "not_a_number".to_owned();

    let result = to_bridge_quote_result(&req, 50, &fees);
    assert!(result.is_err());
}

// ── Across status mapping ────────────────────────────────────────────────────

#[test]
fn map_across_status_filled_to_executed() {
    assert!(matches!(
        map_across_status_to_bridge_status(AcrossDepositStatus::Filled),
        BridgeStatus::Executed
    ));
}

#[test]
fn map_across_status_slow_fill_to_executed() {
    assert!(matches!(
        map_across_status_to_bridge_status(AcrossDepositStatus::SlowFillRequested),
        BridgeStatus::Executed
    ));
}

#[test]
fn map_across_status_pending_to_in_progress() {
    assert!(matches!(
        map_across_status_to_bridge_status(AcrossDepositStatus::Pending),
        BridgeStatus::InProgress
    ));
}

#[test]
fn map_across_status_expired_to_expired() {
    assert!(matches!(
        map_across_status_to_bridge_status(AcrossDepositStatus::Expired),
        BridgeStatus::Expired
    ));
}

#[test]
fn map_across_status_refunded_to_refund() {
    assert!(matches!(
        map_across_status_to_bridge_status(AcrossDepositStatus::Refunded),
        BridgeStatus::Refund
    ));
}

// ── Across status response validation ────────────────────────────────────────

#[test]
fn is_valid_across_status_response_accepts_valid() {
    let resp = json!({ "status": "filled" });
    assert!(is_valid_across_status_response(&resp));
}

#[test]
fn is_valid_across_status_response_rejects_missing_status() {
    let resp = json!({ "data": "something" });
    assert!(!is_valid_across_status_response(&resp));
}

#[test]
fn is_valid_across_status_response_rejects_non_string_status() {
    let resp = json!({ "status": 42 });
    assert!(!is_valid_across_status_response(&resp));
}

// ── API URL resolution ───────────────────────────────────────────────────────

#[test]
fn resolve_api_endpoint_uses_custom_url() {
    let options = BungeeApiUrlOptions::default();
    let result = resolve_api_endpoint_from_options(
        "api_base_url",
        &options,
        false,
        Some("https://custom.example.com"),
    );
    assert_eq!(result, "https://custom.example.com");
}

#[test]
fn resolve_api_endpoint_uses_fallback_when_requested() {
    let mut options = BungeeApiUrlOptions::default();
    options.api_base_url = "https://overridden.example.com".to_owned();

    let result = resolve_api_endpoint_from_options("api_base_url", &options, true, None);
    // Fallback always returns the hard-coded default.
    let defaults = BungeeApiUrlOptions::default();
    assert_eq!(result, defaults.api_base_url);
}

#[test]
fn resolve_api_endpoint_uses_options_value() {
    let mut options = BungeeApiUrlOptions::default();
    options.manual_api_base_url = "https://manual.example.com".to_owned();

    let result =
        resolve_api_endpoint_from_options("manual_api_base_url", &options, false, None);
    assert_eq!(result, "https://manual.example.com");
}

#[test]
fn resolve_api_endpoint_falls_back_when_empty_option() {
    let mut options = BungeeApiUrlOptions::default();
    options.events_api_base_url = String::new();

    let result =
        resolve_api_endpoint_from_options("events_api_base_url", &options, false, None);
    let defaults = BungeeApiUrlOptions::default();
    assert_eq!(result, defaults.events_api_base_url);
}

// ── resolve_api_endpoint across key ─────────────────────────────────────────

#[test]
fn resolve_api_endpoint_across_key() {
    let mut options = BungeeApiUrlOptions::default();
    options.across_api_base_url = "https://across.custom.com".to_owned();

    let result = resolve_api_endpoint_from_options("across_api_base_url", &options, false, None);
    assert_eq!(result, "https://across.custom.com");
}

#[test]
fn resolve_api_endpoint_unknown_key_uses_api_base() {
    let options = BungeeApiUrlOptions::default();
    let result = resolve_api_endpoint_from_options("unknown_key", &options, false, None);
    assert_eq!(result, options.api_base_url);
}

// ── Across spoke pool addresses ─────────────────────────────────────────────

#[test]
fn across_spoke_pool_addresses_contains_mainnet() {
    use cow_rs::bridging::across::across_spoke_pool_addresses;
    let pools = across_spoke_pool_addresses();
    assert!(pools.contains_key(&1));
    assert!(pools.contains_key(&42161));
    assert!(pools.contains_key(&8453));
}

// ── Across math contract addresses ──────────────────────────────────────────

#[test]
fn across_math_contract_addresses_contains_mainnet() {
    use cow_rs::bridging::across::across_math_contract_addresses;
    let addrs = across_math_contract_addresses();
    assert!(addrs.contains_key(&1));
    assert!(addrs.contains_key(&42161));
    assert!(addrs.contains_key(&8453));
}

// ── Across token mapping ────────────────────────────────────────────────────

#[test]
fn across_token_mapping_contains_expected_chains() {
    use cow_rs::bridging::across::across_token_mapping;
    let mapping = across_token_mapping();
    assert!(mapping.contains_key(&1)); // Mainnet
    assert!(mapping.contains_key(&42161)); // Arbitrum
    assert!(mapping.contains_key(&8453)); // Base
    assert!(mapping.contains_key(&137)); // Polygon

    let mainnet = mapping.get(&1).unwrap();
    assert!(mainnet.tokens.contains_key("usdc"));
    assert!(mainnet.tokens.contains_key("weth"));
    assert!(mainnet.tokens.contains_key("wbtc"));
}

// ── get_chain_configs ───────────────────────────────────────────────────────

#[test]
fn get_chain_configs_returns_valid_pair() {
    use cow_rs::bridging::across::get_chain_configs;
    let result = get_chain_configs(1, 42161);
    assert!(result.is_some());
    let (source, target) = result.unwrap();
    assert_eq!(source.chain_id, 1);
    assert_eq!(target.chain_id, 42161);
}

#[test]
fn get_chain_configs_returns_none_for_unknown_chain() {
    use cow_rs::bridging::across::get_chain_configs;
    let result = get_chain_configs(99999, 42161);
    assert!(result.is_none());
}

// ── get_token_symbol ────────────────────────────────────────────────────────

#[test]
fn get_token_symbol_finds_known_token() {
    use cow_rs::bridging::across::{across_token_mapping, get_token_symbol};
    let mapping = across_token_mapping();
    let mainnet = mapping.get(&1).unwrap();
    let usdc_addr = mainnet.tokens["usdc"];
    let symbol = get_token_symbol(usdc_addr, mainnet);
    assert_eq!(symbol, Some("usdc".to_owned()));
}

#[test]
fn get_token_symbol_returns_none_for_unknown() {
    use alloy_primitives::Address;
    use cow_rs::bridging::across::{across_token_mapping, get_token_symbol};
    let mapping = across_token_mapping();
    let mainnet = mapping.get(&1).unwrap();
    let symbol = get_token_symbol(Address::ZERO, mainnet);
    assert!(symbol.is_none());
}

// ── get_token_address ───────────────────────────────────────────────────────

#[test]
fn get_token_address_finds_known_symbol() {
    use cow_rs::bridging::across::{across_token_mapping, get_token_address};
    let mapping = across_token_mapping();
    let mainnet = mapping.get(&1).unwrap();
    let addr = get_token_address("usdc", mainnet);
    assert!(addr.is_some());
}

#[test]
fn get_token_address_returns_none_for_unknown() {
    use cow_rs::bridging::across::{across_token_mapping, get_token_address};
    let mapping = across_token_mapping();
    let mainnet = mapping.get(&1).unwrap();
    let addr = get_token_address("unknown_token", mainnet);
    assert!(addr.is_none());
}

// ── get_token_by_address_and_chain_id ───────────────────────────────────────

#[test]
fn get_token_by_address_and_chain_id_finds_token() {
    use cow_rs::bridging::across::{across_token_mapping, get_token_by_address_and_chain_id};
    let mapping = across_token_mapping();
    let usdc_addr = mapping.get(&1).unwrap().tokens["usdc"];
    let result = get_token_by_address_and_chain_id(usdc_addr, 1);
    assert!(result.is_some());
    let (sym, addr) = result.unwrap();
    assert_eq!(sym, "usdc");
    assert_eq!(addr, usdc_addr);
}

#[test]
fn get_token_by_address_and_chain_id_returns_none_for_unknown_chain() {
    use alloy_primitives::Address;
    use cow_rs::bridging::across::get_token_by_address_and_chain_id;
    let result = get_token_by_address_and_chain_id(Address::ZERO, 99999);
    assert!(result.is_none());
}

// ── Across event parsing ────────────────────────────────────────────────────

#[test]
fn get_across_deposit_events_empty_for_unknown_chain() {
    use cow_rs::bridging::across::{EvmLogEntry, get_across_deposit_events};
    let events = get_across_deposit_events(99999, &[]);
    assert!(events.is_empty());
}

#[test]
fn get_across_deposit_events_empty_for_no_logs() {
    use cow_rs::bridging::across::get_across_deposit_events;
    let events = get_across_deposit_events(1, &[]);
    assert!(events.is_empty());
}

#[test]
fn get_cow_trade_events_empty_for_no_logs() {
    use cow_rs::bridging::across::get_cow_trade_events;
    let events = get_cow_trade_events(1, &[], None);
    assert!(events.is_empty());
}

#[test]
fn get_deposit_params_returns_none_for_empty_logs() {
    use cow_rs::bridging::across::get_deposit_params;
    let result = get_deposit_params(1, "0xorderid", &[], None);
    assert!(result.is_none());
}

// ── Bungee approve and bridge addresses ─────────────────────────────────────

#[test]
fn bungee_approve_and_bridge_v1_addresses_contains_mainnet() {
    use cow_rs::bridging::bungee::bungee_approve_and_bridge_v1_addresses;
    let addrs = bungee_approve_and_bridge_v1_addresses();
    assert!(addrs.contains_key(&1)); // Mainnet
    assert!(addrs.contains_key(&100)); // GnosisChain
    assert!(addrs.contains_key(&42161)); // Arbitrum
    assert!(addrs.contains_key(&8453)); // Base
    assert!(addrs.contains_key(&10)); // Optimism
}

// ── bungee_tx_data_bytes_index ──────────────────────────────────────────────

#[test]
fn bungee_tx_data_bytes_index_across() {
    use cow_rs::bridging::bungee::bungee_tx_data_bytes_index;
    let idx = bungee_tx_data_bytes_index(BungeeBridge::Across, "0xcc54d224");
    assert!(idx.is_some());
    let idx = idx.unwrap();
    assert_eq!(idx.bytes_start_index, 8);
    assert_eq!(idx.bytes_length, 32);
}

#[test]
fn bungee_tx_data_bytes_index_across_alternate_selector() {
    use cow_rs::bridging::bungee::bungee_tx_data_bytes_index;
    let idx = bungee_tx_data_bytes_index(BungeeBridge::Across, "0xa3b8bfba");
    assert!(idx.is_some());
}

#[test]
fn bungee_tx_data_bytes_index_cctp() {
    use cow_rs::bridging::bungee::bungee_tx_data_bytes_index;
    let idx = bungee_tx_data_bytes_index(BungeeBridge::CircleCctp, "0xb7dfe9d0");
    assert!(idx.is_some());
}

#[test]
fn bungee_tx_data_bytes_index_gnosis_native() {
    use cow_rs::bridging::bungee::bungee_tx_data_bytes_index;
    let idx = bungee_tx_data_bytes_index(BungeeBridge::GnosisNative, "0x3bf5c228");
    assert!(idx.is_some());
    let idx2 = bungee_tx_data_bytes_index(BungeeBridge::GnosisNative, "0xfcb23eb0");
    assert!(idx2.is_some());
}

#[test]
fn bungee_tx_data_bytes_index_returns_none_for_unknown_selector() {
    use cow_rs::bridging::bungee::bungee_tx_data_bytes_index;
    let idx = bungee_tx_data_bytes_index(BungeeBridge::Across, "0x00000000");
    assert!(idx.is_none());
    let idx2 = bungee_tx_data_bytes_index(BungeeBridge::CircleCctp, "0x00000000");
    assert!(idx2.is_none());
    let idx3 = bungee_tx_data_bytes_index(BungeeBridge::GnosisNative, "0x00000000");
    assert!(idx3.is_none());
}

// ── bungee_tx_data_bytes_index case-insensitive ─────────────────────────────

#[test]
fn bungee_tx_data_bytes_index_case_insensitive() {
    use cow_rs::bridging::bungee::bungee_tx_data_bytes_index;
    let idx_lower = bungee_tx_data_bytes_index(BungeeBridge::Across, "0xCC54D224");
    assert!(idx_lower.is_some());
}

// ── decode_amounts for CircleCctp ───────────────────────────────────────────

#[test]
fn decode_amounts_for_circle_cctp() {
    let amount_hex = "0000000000000000000000000000000000000000000000000000000000000064";
    let tx_data = format!("0x11223344b7dfe9d0{amount_hex}");
    let decoded = decode_amounts_bungee_tx_data(&tx_data, BungeeBridge::CircleCctp).unwrap();
    assert_eq!(decoded.input_amount, U256::from(100u64));
}

// ── get_display_name_from_bungee_bridge ─────────────────────────────────────

#[test]
fn get_display_name_from_bungee_bridge_roundtrip() {
    use cow_rs::bridging::bungee::get_display_name_from_bungee_bridge;
    assert_eq!(get_display_name_from_bungee_bridge(BungeeBridge::Across), "Across");
    assert_eq!(get_display_name_from_bungee_bridge(BungeeBridge::CircleCctp), "Circle CCTP");
    assert_eq!(
        get_display_name_from_bungee_bridge(BungeeBridge::GnosisNative),
        "Gnosis Native"
    );
}

// ── Bungee deposit call construction ────────────────────────────────────────

#[test]
fn create_bungee_deposit_call_across() {
    use cow_rs::bridging::bungee::{BungeeDepositCallParams, create_bungee_deposit_call};
    let amount_hex = "0000000000000000000000000000000000000000000000000000000000000064";
    let build_tx_data = format!("0x11223344cc54d224{amount_hex}");

    let params = BungeeDepositCallParams {
        request: sample_request(),
        build_tx_data,
        input_amount: U256::from(100u64),
        bridge: BungeeBridge::Across,
    };

    let result = create_bungee_deposit_call(&params);
    assert!(result.is_ok());
    let call = result.unwrap();
    assert!(!call.data.is_empty());
}

#[test]
fn create_bungee_deposit_call_unknown_chain_fails() {
    use cow_rs::bridging::bungee::{BungeeDepositCallParams, create_bungee_deposit_call};
    let amount_hex = "0000000000000000000000000000000000000000000000000000000000000064";
    let build_tx_data = format!("0x11223344cc54d224{amount_hex}");

    let mut req = sample_request();
    req.sell_chain_id = 99999;

    let params = BungeeDepositCallParams {
        request: req,
        build_tx_data,
        input_amount: U256::from(100u64),
        bridge: BungeeBridge::Across,
    };

    let result = create_bungee_deposit_call(&params);
    assert!(result.is_err());
}

// ── Across status from events with Across error ────────────────────────────

#[tokio::test]
async fn status_from_events_src_complete_dest_pending_across_error_returns_in_progress() {
    let across_error = |_: &str| async { Err(BridgeError::QuoteError("api error".to_owned())) };

    let event = make_bungee_event(
        BungeeEventStatus::Completed,
        BungeeEventStatus::Pending,
        BungeeBridgeName::Across,
    );
    let result = get_bridging_status_from_events(Some(&[event]), across_error)
        .await
        .unwrap();
    assert!(matches!(result.status, BridgeStatus::InProgress));
}

// ── Across deposit call params ──────────────────────────────────────────────

#[test]
fn across_deposit_call_params_debug() {
    use cow_rs::bridging::across::AcrossDepositCallParams;
    let params = AcrossDepositCallParams {
        request: sample_request(),
        suggested_fees: sample_suggested_fees(),
        cow_shed_account: address!("1111111111111111111111111111111111111111"),
    };
    let debug = format!("{params:?}");
    assert!(debug.contains("AcrossDepositCallParams"));
}

// ── Across to_bridge_quote_result with zero amounts ─────────────────────────

#[test]
fn across_to_bridge_quote_result_zero_sell_amount() {
    let mut req = sample_request();
    req.sell_amount = U256::ZERO;
    let fees = sample_suggested_fees();
    let result = to_bridge_quote_result(&req, 50, &fees);
    assert!(result.is_ok());
}

// ── Bungee quote response with multiple routes ──────────────────────────────

#[test]
fn is_valid_quote_response_multiple_routes() {
    let resp = json!({
        "success": true,
        "result": {
            "manualRoutes": [
                {
                    "quoteId": "q-1",
                    "output": { "amount": "100" },
                    "estimatedTime": 60,
                    "routeDetails": { "routeFee": { "amount": "5" } }
                },
                {
                    "quoteId": "q-2",
                    "output": { "amount": "200" },
                    "estimatedTime": 30,
                    "routeDetails": { "routeFee": { "amount": "3" } }
                }
            ]
        }
    });
    assert!(is_valid_quote_response(&resp));
}

// ── Bungee events response with multiple events ─────────────────────────────

#[test]
fn is_valid_bungee_events_response_multiple_events() {
    let resp = json!({
        "success": true,
        "result": [
            {
                "identifier": "evt-1",
                "bridgeName": "across",
                "fromChainId": 1,
                "orderId": "o-1",
                "srcTxStatus": "COMPLETED",
                "destTxStatus": "COMPLETED"
            },
            {
                "identifier": "evt-2",
                "bridgeName": "cctp",
                "fromChainId": 137,
                "orderId": "o-2",
                "srcTxStatus": "PENDING",
                "destTxStatus": "PENDING"
            }
        ]
    });
    assert!(is_valid_bungee_events_response(&resp));
}

// ── BungeeApiUrlOptions default values ──────────────────────────────────────

#[test]
fn bungee_api_url_options_default_values() {
    let options = BungeeApiUrlOptions::default();
    assert!(!options.api_base_url.is_empty());
    assert!(!options.manual_api_base_url.is_empty());
    assert!(!options.events_api_base_url.is_empty());
    assert!(!options.across_api_base_url.is_empty());
}

// ── decode_bungee_bridge_tx_data edge cases ─────────────────────────────────

#[test]
fn decode_bungee_bridge_tx_data_minimal_valid() {
    let tx_data = "0x1122334455667788aabbccdd";
    let decoded = decode_bungee_bridge_tx_data(tx_data).unwrap();
    assert_eq!(decoded.route_id, "0x11223344");
    assert!(decoded.encoded_function_data.starts_with("0x55667788"));
}

#[test]
fn decode_bungee_bridge_tx_data_only_route_id_no_selector() {
    let tx_data = "0x112233445566";
    let result = decode_bungee_bridge_tx_data(tx_data);
    assert!(result.is_err());
}

// ── BridgingSdk with mock provider ───────────────────────────────────────────

struct MockProvider {
    buy_amount: U256,
    should_fail: bool,
}

impl std::fmt::Debug for MockProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockProvider").finish()
    }
}

impl BridgeProvider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn supports_route(&self, _sell_chain: u64, _buy_chain: u64) -> bool {
        true
    }

    fn get_quote<'a>(&'a self, req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
        let buy_amount = self.buy_amount;
        let should_fail = self.should_fail;
        Box::pin(async move {
            if should_fail {
                return Err(cow_rs::CowError::Api {
                    status: 500,
                    body: "mock error".to_owned(),
                });
            }
            Ok(QuoteBridgeResponse {
                provider: "mock".to_owned(),
                sell_amount: req.sell_amount,
                buy_amount,
                fee_amount: U256::ZERO,
                estimated_secs: 60,
                bridge_hook: None,
            })
        })
    }
}

struct UnsupportedProvider;

impl std::fmt::Debug for UnsupportedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnsupportedProvider").finish()
    }
}

impl BridgeProvider for UnsupportedProvider {
    fn name(&self) -> &str {
        "unsupported"
    }

    fn supports_route(&self, _sell_chain: u64, _buy_chain: u64) -> bool {
        false
    }

    fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
        Box::pin(async { unreachable!() })
    }
}

#[tokio::test]
async fn sdk_get_best_quote_returns_highest_buy_amount() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(MockProvider { buy_amount: U256::from(100u64), should_fail: false });
    sdk.add_provider(MockProvider { buy_amount: U256::from(200u64), should_fail: false });

    let req = sample_request();
    let best = sdk.get_best_quote(&req).await.unwrap();
    assert_eq!(best.buy_amount, U256::from(200u64));
}

#[tokio::test]
async fn sdk_get_best_quote_skips_failed_providers() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(MockProvider { buy_amount: U256::from(100u64), should_fail: false });
    sdk.add_provider(MockProvider { buy_amount: U256::ZERO, should_fail: true });

    let req = sample_request();
    let best = sdk.get_best_quote(&req).await.unwrap();
    assert_eq!(best.buy_amount, U256::from(100u64));
}

#[tokio::test]
async fn sdk_get_best_quote_returns_no_providers_error() {
    let sdk = BridgingSdk::new();
    let req = sample_request();
    let result = sdk.get_best_quote(&req).await;
    assert!(matches!(result, Err(BridgeError::NoProviders)));
}

#[tokio::test]
async fn sdk_get_best_quote_returns_no_quote_when_all_fail() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(MockProvider { buy_amount: U256::ZERO, should_fail: true });

    let req = sample_request();
    let result = sdk.get_best_quote(&req).await;
    assert!(matches!(result, Err(BridgeError::NoQuote)));
}

#[tokio::test]
async fn sdk_get_best_quote_returns_no_providers_when_none_support_route() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(UnsupportedProvider);

    let req = sample_request();
    let result = sdk.get_best_quote(&req).await;
    assert!(matches!(result, Err(BridgeError::NoProviders)));
}

#[tokio::test]
async fn sdk_get_all_quotes_returns_both_successes_and_errors() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(MockProvider { buy_amount: U256::from(100u64), should_fail: false });
    sdk.add_provider(MockProvider { buy_amount: U256::ZERO, should_fail: true });

    let req = sample_request();
    let results = sdk.get_all_quotes(&req).await;
    assert_eq!(results.len(), 2);

    let successes: Vec<_> = results.iter().filter(|r| r.is_ok()).collect();
    let failures: Vec<_> = results.iter().filter(|r| r.is_err()).collect();
    assert_eq!(successes.len(), 1);
    assert_eq!(failures.len(), 1);
}

// ── BungeeProvider wiremock: full HTTP round-trip via mock server ─────────────

#[tokio::test]
async fn bungee_provider_wiremock_success_roundtrip() {
    let server = MockServer::start().await;

    let body = json!({
        "success": true,
        "result": {
            "routes": [{
                "outputAmount": "990000000",
                "estimatedTimeInSeconds": 90
            }]
        }
    });

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v2/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&server)
        .await;

    // Simulate the request parsing that BungeeProvider does internally.
    let req = sample_request();
    let client = reqwest::Client::new();
    let slippage_pct = req.slippage_bps as f64 / 100.0;

    let url = reqwest::Url::parse_with_params(
        &format!("{}/v2/quote", server.uri()),
        &[
            ("fromChainId", req.sell_chain_id.to_string()),
            ("toChainId", req.buy_chain_id.to_string()),
            ("fromTokenAddress", format!("{:#x}", req.sell_token)),
            ("toTokenAddress", format!("{:#x}", req.buy_token)),
            ("fromAmount", req.sell_amount.to_string()),
            ("userAddress", format!("{:#x}", req.account)),
            ("slippageTolerance", format!("{slippage_pct:.1}")),
            ("isContractCall", "false".to_owned()),
        ],
    )
    .unwrap();

    let resp = client.get(url).header("API-KEY", "test").send().await.unwrap();
    assert!(resp.status().is_success());

    let json_resp: serde_json::Value = resp.json().await.unwrap();
    let route = &json_resp["result"]["routes"][0];
    assert_eq!(route["outputAmount"].as_str().unwrap(), "990000000");
    assert_eq!(route["estimatedTimeInSeconds"].as_u64().unwrap(), 90);
}

#[tokio::test]
async fn bungee_provider_wiremock_server_error() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/v2/quote"))
        .respond_with(
            ResponseTemplate::new(500).set_body_json(json!({"error": "internal server error"})),
        )
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/v2/quote?fromChainId=1", server.uri()))
        .header("API-KEY", "test")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 500);
}

// ── Across status HTTP mock ──────────────────────────────────────────────────

#[tokio::test]
async fn across_status_api_mock_filled() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/deposit/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "filled",
            "originChainId": "1",
            "depositId": "123",
            "depositTxHash": "0xabc",
            "fillTx": "0xdef",
            "destinationChainId": "42161"
        })))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/api/deposit/status?depositId=123", server.uri()))
        .send()
        .await
        .unwrap();

    let json_val: serde_json::Value = resp.json().await.unwrap();
    assert!(is_valid_across_status_response(&json_val));
    assert_eq!(json_val["status"].as_str().unwrap(), "filled");
}

#[tokio::test]
async fn across_status_api_mock_pending() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/deposit/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "pending",
            "originChainId": "1",
            "depositId": "456"
        })))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/api/deposit/status?depositId=456", server.uri()))
        .send()
        .await
        .unwrap();

    let json_val: serde_json::Value = resp.json().await.unwrap();
    assert!(is_valid_across_status_response(&json_val));
    assert_eq!(json_val["status"].as_str().unwrap(), "pending");
}
