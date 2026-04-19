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
    clippy::single_match,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::items_after_statements,
    clippy::needless_raw_strings,
    clippy::unreadable_literal
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
            BungeeApiUrlOptions, bungee_to_bridge_quote_result, decode_amounts_bungee_tx_data,
            decode_bungee_bridge_tx_data, get_bridging_status_from_events,
            get_bungee_bridge_from_display_name, is_valid_bungee_events_response,
            is_valid_quote_response, resolve_api_endpoint_from_options,
        },
        provider::QuoteFuture,
        types::{
            AcrossDepositStatus, AcrossPctFee, AcrossSuggestedFeesLimits,
            AcrossSuggestedFeesResponse, BridgeError, BridgeStatus, BungeeBridge, BungeeBridgeName,
            BungeeEvent, BungeeEventStatus,
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

    let route = json_val["result"]["routes"].as_array().unwrap().first().unwrap();

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
    let tx_data =
        "0x11223344aabbccdd0000000000000000000000000000000000000000000000000000000000000001";
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
    assert_eq!(get_bungee_bridge_from_display_name("Across"), Some(BungeeBridge::Across));
    assert_eq!(get_bungee_bridge_from_display_name("Circle CCTP"), Some(BungeeBridge::CircleCctp));
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
    assert_eq!(result.amounts_and_costs.costs.bridging_fee.amount_in_buy_currency, U256::ZERO);
}

// ── Bridging status from events ──────────────────────────────────────────────

#[tokio::test]
async fn status_from_events_returns_unknown_when_no_events() {
    let dummy_across = |_: &str| async { Ok("pending".to_owned()) };

    let result = get_bridging_status_from_events(None, dummy_across).await.unwrap();
    assert!(matches!(result.status, BridgeStatus::Unknown));
}

#[tokio::test]
async fn status_from_events_returns_unknown_for_empty_slice() {
    let dummy_across = |_: &str| async { Ok("pending".to_owned()) };

    let events: Vec<BungeeEvent> = vec![];
    let result = get_bridging_status_from_events(Some(&events), dummy_across).await.unwrap();
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
    let result = get_bridging_status_from_events(Some(&[event]), dummy_across).await.unwrap();
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
    let result = get_bridging_status_from_events(Some(&[event]), dummy_across).await.unwrap();
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
    let result = get_bridging_status_from_events(Some(&[event]), across_expired).await.unwrap();
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
    let result = get_bridging_status_from_events(Some(&[event]), across_refunded).await.unwrap();
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
    let result = get_bridging_status_from_events(Some(&[event]), dummy_across).await.unwrap();
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
        result.amounts_and_costs.after_fee.buy_amount <
            result.amounts_and_costs.before_fee.buy_amount
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
    let options = BungeeApiUrlOptions {
        api_base_url: "https://overridden.example.com".to_owned(),
        ..BungeeApiUrlOptions::default()
    };

    let result = resolve_api_endpoint_from_options("api_base_url", &options, true, None);
    // Fallback always returns the hard-coded default.
    let defaults = BungeeApiUrlOptions::default();
    assert_eq!(result, defaults.api_base_url);
}

#[test]
fn resolve_api_endpoint_uses_options_value() {
    let options = BungeeApiUrlOptions {
        manual_api_base_url: "https://manual.example.com".to_owned(),
        ..BungeeApiUrlOptions::default()
    };

    let result = resolve_api_endpoint_from_options("manual_api_base_url", &options, false, None);
    assert_eq!(result, "https://manual.example.com");
}

#[test]
fn resolve_api_endpoint_falls_back_when_empty_option() {
    let options = BungeeApiUrlOptions {
        events_api_base_url: String::new(),
        ..BungeeApiUrlOptions::default()
    };

    let result = resolve_api_endpoint_from_options("events_api_base_url", &options, false, None);
    let defaults = BungeeApiUrlOptions::default();
    assert_eq!(result, defaults.events_api_base_url);
}

// ── resolve_api_endpoint across key ─────────────────────────────────────────

#[test]
fn resolve_api_endpoint_across_key() {
    let options = BungeeApiUrlOptions {
        across_api_base_url: "https://across.custom.com".to_owned(),
        ..BungeeApiUrlOptions::default()
    };

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
    use cow_rs::bridging::across::get_across_deposit_events;
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
    let idx = bungee_tx_data_bytes_index(BungeeBridge::Across, "0xcc54d224")
        .expect("expected Some for Across bridge");
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
    assert_eq!(get_display_name_from_bungee_bridge(BungeeBridge::GnosisNative), "Gnosis Native");
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
    let result = get_bridging_status_from_events(Some(&[event]), across_error).await.unwrap();
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
    info: cow_rs::bridging::BridgeProviderInfo,
}

impl MockProvider {
    fn new(buy_amount: U256, should_fail: bool) -> Self {
        Self { buy_amount, should_fail, info: mock_info("mock") }
    }
}

impl std::fmt::Debug for MockProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockProvider").finish()
    }
}

fn mock_info(name: &str) -> cow_rs::bridging::BridgeProviderInfo {
    cow_rs::bridging::BridgeProviderInfo {
        name: name.to_owned(),
        logo_url: String::new(),
        dapp_id: format!("cow-sdk://bridging/providers/{name}"),
        website: String::new(),
        provider_type: cow_rs::bridging::BridgeProviderType::HookBridgeProvider,
    }
}

fn mock_unimpl_status<'a>() -> cow_rs::bridging::provider::BridgeStatusFuture<'a> {
    Box::pin(async {
        Ok(cow_rs::bridging::BridgeStatusResult {
            status: cow_rs::bridging::BridgeStatus::Unknown,
            fill_time_in_seconds: None,
            deposit_tx_hash: None,
            fill_tx_hash: None,
        })
    })
}

impl BridgeProvider for MockProvider {
    fn info(&self) -> &cow_rs::bridging::BridgeProviderInfo {
        &self.info
    }

    fn supports_route(&self, _sell_chain: u64, _buy_chain: u64) -> bool {
        true
    }

    fn get_networks<'a>(&'a self) -> cow_rs::bridging::provider::NetworksFuture<'a> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get_buy_tokens<'a>(
        &'a self,
        _params: cow_rs::bridging::BuyTokensParams,
    ) -> cow_rs::bridging::provider::BuyTokensFuture<'a> {
        let info = self.info.clone();
        Box::pin(async move {
            Ok(cow_rs::bridging::GetProviderBuyTokens { provider_info: info, tokens: vec![] })
        })
    }

    fn get_intermediate_tokens<'a>(
        &'a self,
        _request: &'a QuoteBridgeRequest,
    ) -> cow_rs::bridging::provider::IntermediateTokensFuture<'a> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get_quote<'a>(&'a self, req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
        let buy_amount = self.buy_amount;
        let should_fail = self.should_fail;
        Box::pin(async move {
            if should_fail {
                return Err(cow_rs::CowError::Api { status: 500, body: "mock error".to_owned() });
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

    fn get_bridging_params<'a>(
        &'a self,
        _chain_id: u64,
        _order: &'a cow_orderbook::types::Order,
        _tx_hash: alloy_primitives::B256,
        _settlement_override: Option<alloy_primitives::Address>,
    ) -> cow_rs::bridging::provider::BridgingParamsFuture<'a> {
        Box::pin(async { Ok(None) })
    }

    fn get_explorer_url(&self, bridging_id: &str) -> String {
        format!("https://example.com/mock/{bridging_id}")
    }

    fn get_status<'a>(
        &'a self,
        _bridging_id: &'a str,
        _origin_chain_id: u64,
    ) -> cow_rs::bridging::provider::BridgeStatusFuture<'a> {
        mock_unimpl_status()
    }
}

struct UnsupportedProvider {
    info: cow_rs::bridging::BridgeProviderInfo,
}

impl Default for UnsupportedProvider {
    fn default() -> Self {
        Self { info: mock_info("unsupported") }
    }
}

impl std::fmt::Debug for UnsupportedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnsupportedProvider").finish()
    }
}

impl BridgeProvider for UnsupportedProvider {
    fn info(&self) -> &cow_rs::bridging::BridgeProviderInfo {
        &self.info
    }

    fn supports_route(&self, _sell_chain: u64, _buy_chain: u64) -> bool {
        false
    }

    fn get_networks<'a>(&'a self) -> cow_rs::bridging::provider::NetworksFuture<'a> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get_buy_tokens<'a>(
        &'a self,
        _params: cow_rs::bridging::BuyTokensParams,
    ) -> cow_rs::bridging::provider::BuyTokensFuture<'a> {
        let info = self.info.clone();
        Box::pin(async move {
            Ok(cow_rs::bridging::GetProviderBuyTokens { provider_info: info, tokens: vec![] })
        })
    }

    fn get_intermediate_tokens<'a>(
        &'a self,
        _request: &'a QuoteBridgeRequest,
    ) -> cow_rs::bridging::provider::IntermediateTokensFuture<'a> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
        Box::pin(async { unreachable!() })
    }

    fn get_bridging_params<'a>(
        &'a self,
        _chain_id: u64,
        _order: &'a cow_orderbook::types::Order,
        _tx_hash: alloy_primitives::B256,
        _settlement_override: Option<alloy_primitives::Address>,
    ) -> cow_rs::bridging::provider::BridgingParamsFuture<'a> {
        Box::pin(async { Ok(None) })
    }

    fn get_explorer_url(&self, bridging_id: &str) -> String {
        format!("https://example.com/unsupported/{bridging_id}")
    }

    fn get_status<'a>(
        &'a self,
        _bridging_id: &'a str,
        _origin_chain_id: u64,
    ) -> cow_rs::bridging::provider::BridgeStatusFuture<'a> {
        mock_unimpl_status()
    }
}

#[tokio::test]
async fn sdk_get_best_quote_returns_highest_buy_amount() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(MockProvider::new(U256::from(100u64), false));
    sdk.add_provider(MockProvider::new(U256::from(200u64), false));

    let req = sample_request();
    let best = sdk.get_best_quote(&req).await.unwrap();
    assert_eq!(best.buy_amount, U256::from(200u64));
}

#[tokio::test]
async fn sdk_get_best_quote_skips_failed_providers() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(MockProvider::new(U256::from(100u64), false));
    sdk.add_provider(MockProvider::new(U256::ZERO, true));

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
    sdk.add_provider(MockProvider::new(U256::ZERO, true));

    let req = sample_request();
    let result = sdk.get_best_quote(&req).await;
    assert!(matches!(result, Err(BridgeError::NoQuote)));
}

#[tokio::test]
async fn sdk_get_best_quote_returns_no_providers_when_none_support_route() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(UnsupportedProvider::default());

    let req = sample_request();
    let result = sdk.get_best_quote(&req).await;
    assert!(matches!(result, Err(BridgeError::NoProviders)));
}

#[tokio::test]
async fn sdk_get_all_quotes_returns_both_successes_and_errors() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(MockProvider::new(U256::from(100u64), false));
    sdk.add_provider(MockProvider::new(U256::ZERO, true));

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

// ══════════════════════════════════════════════════════════════════════════════
// NEW COVERAGE TESTS — across.rs
// ══════════════════════════════════════════════════════════════════════════════

// ── Across event parsing with real log structures ──────────────────────────

#[test]
fn get_across_deposit_events_parses_valid_log() {
    use alloy_primitives::{B256, U256, keccak256};
    use cow_rs::bridging::{
        EvmLogEntry,
        across::{across_spoke_pool_addresses, get_across_deposit_events},
    };

    let spoke_pools = across_spoke_pool_addresses();
    let spoke_pool = *spoke_pools.get(&1).unwrap();

    let topic0 = keccak256(
        "FundsDeposited(bytes32,bytes32,uint256,uint256,uint256,uint256,uint32,uint32,uint32,bytes32,bytes32,bytes32,bytes)",
    );

    // Build indexed topics: destinationChainId=42161, depositId=7, depositor=0x3333..
    let mut dest_chain_bytes = [0u8; 32];
    dest_chain_bytes[24..32].copy_from_slice(&42161u64.to_be_bytes());
    let topic1 = B256::from(dest_chain_bytes);

    let mut deposit_id_bytes = [0u8; 32];
    deposit_id_bytes[31] = 7;
    let topic2 = B256::from(deposit_id_bytes);

    let mut depositor_bytes = [0u8; 32];
    depositor_bytes[12..32].copy_from_slice(&[0x33u8; 20]);
    let topic3 = B256::from(depositor_bytes);

    // Build non-indexed data: 9 x 32-byte words
    let mut data = vec![0u8; 9 * 32];
    // inputToken (word 0) - address at bytes 12..32
    data[12..32].copy_from_slice(&[0xAAu8; 20]);
    // outputToken (word 1) - address at bytes 44..64
    data[44..64].copy_from_slice(&[0xBBu8; 20]);
    // inputAmount (word 2)
    data[64 + 31] = 100;
    // outputAmount (word 3)
    data[96 + 31] = 95;
    // quoteTimestamp (word 4) - u32 at bytes 156..160
    data[156..160].copy_from_slice(&1700000000u32.to_be_bytes());
    // fillDeadline (word 5) - u32 at bytes 188..192
    data[188..192].copy_from_slice(&1700003600u32.to_be_bytes());
    // exclusivityDeadline (word 6) - u32 at bytes 220..224
    data[220..224].copy_from_slice(&0u32.to_be_bytes());
    // recipient (word 7) - address at bytes 236..256
    data[236..256].copy_from_slice(&[0xCCu8; 20]);
    // exclusiveRelayer (word 8) - address at bytes 268..288
    data[268..288].copy_from_slice(&[0x00u8; 20]);

    let log =
        EvmLogEntry { address: spoke_pool, topics: vec![topic0, topic1, topic2, topic3], data };

    let events = get_across_deposit_events(1, &[log]);
    assert_eq!(events.len(), 1);
    let evt = &events[0];
    assert_eq!(evt.destination_chain_id, 42161);
    assert_eq!(evt.deposit_id, U256::from(7u64));
    assert_eq!(evt.input_amount, U256::from(100u64));
    assert_eq!(evt.output_amount, U256::from(95u64));
    assert_eq!(evt.quote_timestamp, 1700000000);
    assert_eq!(evt.fill_deadline, 1700003600);
}

#[test]
fn get_across_deposit_events_skips_log_with_wrong_address() {
    use alloy_primitives::{Address, B256, keccak256};
    use cow_rs::bridging::{EvmLogEntry, across::get_across_deposit_events};

    let topic0 = keccak256(
        "FundsDeposited(bytes32,bytes32,uint256,uint256,uint256,uint256,uint32,uint32,uint32,bytes32,bytes32,bytes32,bytes)",
    );

    let log = EvmLogEntry {
        address: Address::ZERO,
        topics: vec![topic0, B256::ZERO, B256::ZERO, B256::ZERO],
        data: vec![0u8; 9 * 32],
    };

    let events = get_across_deposit_events(1, &[log]);
    assert!(events.is_empty());
}

#[test]
fn get_across_deposit_events_skips_log_with_insufficient_topics() {
    use alloy_primitives::keccak256;
    use cow_rs::bridging::{
        EvmLogEntry,
        across::{across_spoke_pool_addresses, get_across_deposit_events},
    };

    let spoke_pools = across_spoke_pool_addresses();
    let spoke_pool = *spoke_pools.get(&1).unwrap();
    let topic0 = keccak256(
        "FundsDeposited(bytes32,bytes32,uint256,uint256,uint256,uint256,uint32,uint32,uint32,bytes32,bytes32,bytes32,bytes)",
    );

    let log = EvmLogEntry {
        address: spoke_pool,
        topics: vec![topic0], // only 1 topic, need 4
        data: vec![0u8; 9 * 32],
    };

    let events = get_across_deposit_events(1, &[log]);
    assert!(events.is_empty());
}

#[test]
fn get_across_deposit_events_skips_log_with_insufficient_data() {
    use alloy_primitives::{B256, keccak256};
    use cow_rs::bridging::{
        EvmLogEntry,
        across::{across_spoke_pool_addresses, get_across_deposit_events},
    };

    let spoke_pools = across_spoke_pool_addresses();
    let spoke_pool = *spoke_pools.get(&1).unwrap();
    let topic0 = keccak256(
        "FundsDeposited(bytes32,bytes32,uint256,uint256,uint256,uint256,uint32,uint32,uint32,bytes32,bytes32,bytes32,bytes)",
    );

    let log = EvmLogEntry {
        address: spoke_pool,
        topics: vec![topic0, B256::ZERO, B256::ZERO, B256::ZERO],
        data: vec![0u8; 32], // too short, need 9*32
    };

    let events = get_across_deposit_events(1, &[log]);
    assert!(events.is_empty());
}

// ── CoW Trade event parsing ────────────────────────────────────────────────

#[test]
fn get_cow_trade_events_parses_valid_log() {
    use alloy_primitives::{Address, B256, U256, keccak256};
    use cow_rs::bridging::{EvmLogEntry, across::get_cow_trade_events};

    let topic0 = keccak256("Trade(address,address,address,uint256,uint256,uint256,bytes)");

    // Use the actual settlement contract address for mainnet
    let settlement: Address = "0x9008D19f58AAbD9eD0D60971565AA8510560ab41".parse().unwrap();

    // owner in topic1
    let mut owner_bytes = [0u8; 32];
    owner_bytes[12..32].copy_from_slice(&[0x11u8; 20]);
    let topic1 = B256::from(owner_bytes);

    // Non-indexed data: sellToken, buyToken, sellAmount, buyAmount, feeAmount, orderUid
    // orderUid is dynamic bytes: offset + length + data
    let uid_data = b"order-uid-data-0x1234";
    let uid_padded_len = uid_data.len().div_ceil(32) * 32;
    let mut data = vec![0u8; 7 * 32 + uid_padded_len];

    // sellToken (word 0)
    data[12..32].copy_from_slice(&[0xAAu8; 20]);
    // buyToken (word 1)
    data[44..64].copy_from_slice(&[0xBBu8; 20]);
    // sellAmount (word 2)
    data[64 + 31] = 100;
    // buyAmount (word 3)
    data[96 + 31] = 95;
    // feeAmount (word 4)
    data[128 + 31] = 5;
    // offset to bytes (word 5) = 6*32 = 192
    data[160 + 31] = 192;
    // length of uid (word 6)
    data[192 + 31] = uid_data.len() as u8;
    // uid data
    data[224..224 + uid_data.len()].copy_from_slice(uid_data);

    let log = EvmLogEntry { address: settlement, topics: vec![topic0, topic1], data };

    let events = get_cow_trade_events(1, &[log], None);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].sell_amount, U256::from(100u64));
    assert_eq!(events[0].buy_amount, U256::from(95u64));
    assert_eq!(events[0].fee_amount, U256::from(5u64));
}

#[test]
fn get_cow_trade_events_with_settlement_override() {
    use alloy_primitives::{Address, B256, keccak256};
    use cow_rs::bridging::{EvmLogEntry, across::get_cow_trade_events};

    let topic0 = keccak256("Trade(address,address,address,uint256,uint256,uint256,bytes)");

    let custom_settlement: Address = "0x1234567890abcdef1234567890abcdef12345678".parse().unwrap();

    let mut owner_bytes = [0u8; 32];
    owner_bytes[12..32].copy_from_slice(&[0x11u8; 20]);
    let topic1 = B256::from(owner_bytes);

    let uid_data = b"uid";
    let uid_padded_len = uid_data.len().div_ceil(32) * 32;
    let mut data = vec![0u8; 7 * 32 + uid_padded_len];
    data[160 + 31] = 192; // offset
    data[192 + 31] = uid_data.len() as u8; // length
    data[224..224 + uid_data.len()].copy_from_slice(uid_data);

    let log = EvmLogEntry { address: custom_settlement, topics: vec![topic0, topic1], data };

    // Without override: not matched (wrong address for chain 99999)
    let events = get_cow_trade_events(99999, std::slice::from_ref(&log), None);
    assert!(events.is_empty());

    // With override: matched
    let events = get_cow_trade_events(99999, std::slice::from_ref(&log), Some(custom_settlement));
    assert_eq!(events.len(), 1);
}

#[test]
fn get_cow_trade_events_skips_log_with_too_few_topics() {
    use alloy_primitives::{Address, keccak256};
    use cow_rs::bridging::{EvmLogEntry, across::get_cow_trade_events};

    let topic0 = keccak256("Trade(address,address,address,uint256,uint256,uint256,bytes)");
    let settlement: Address = "0x9008D19f58AAbD9eD0D60971565AA8510560ab41".parse().unwrap();

    let log = EvmLogEntry {
        address: settlement,
        topics: vec![topic0], // only 1 topic, need 2
        data: vec![0u8; 7 * 32],
    };

    let events = get_cow_trade_events(1, &[log], None);
    assert!(events.is_empty());
}

#[test]
fn get_cow_trade_events_skips_log_with_insufficient_data() {
    use alloy_primitives::{B256, keccak256};
    use cow_rs::bridging::{EvmLogEntry, across::get_cow_trade_events};

    let topic0 = keccak256("Trade(address,address,address,uint256,uint256,uint256,bytes)");
    let settlement: alloy_primitives::Address =
        "0x9008D19f58AAbD9eD0D60971565AA8510560ab41".parse().unwrap();

    let log = EvmLogEntry {
        address: settlement,
        topics: vec![topic0, B256::ZERO],
        data: vec![0u8; 32], // too short
    };

    let events = get_cow_trade_events(1, &[log], None);
    assert!(events.is_empty());
}

// ── get_deposit_params with matching events ────────────────────────────────

#[test]
fn get_deposit_params_matches_trade_and_deposit() {
    use alloy_primitives::{Address, B256, U256, keccak256};
    use cow_rs::bridging::{
        EvmLogEntry,
        across::{across_spoke_pool_addresses, get_deposit_params},
    };

    let spoke_pools = across_spoke_pool_addresses();
    let spoke_pool = *spoke_pools.get(&1).unwrap();
    let settlement: Address = "0x9008D19f58AAbD9eD0D60971565AA8510560ab41".parse().unwrap();

    // Build Across FundsDeposited log
    let deposit_topic0 = keccak256(
        "FundsDeposited(bytes32,bytes32,uint256,uint256,uint256,uint256,uint32,uint32,uint32,bytes32,bytes32,bytes32,bytes)",
    );
    let mut dest_chain_bytes = [0u8; 32];
    dest_chain_bytes[24..32].copy_from_slice(&42161u64.to_be_bytes());
    let topic1 = B256::from(dest_chain_bytes);
    let mut deposit_id_bytes = [0u8; 32];
    deposit_id_bytes[31] = 1;
    let topic2 = B256::from(deposit_id_bytes);
    let mut depositor_bytes = [0u8; 32];
    depositor_bytes[12..32].copy_from_slice(&[0x33u8; 20]);
    let topic3 = B256::from(depositor_bytes);

    let mut deposit_data = vec![0u8; 9 * 32];
    deposit_data[12..32].copy_from_slice(&[0xAAu8; 20]); // inputToken
    deposit_data[44..64].copy_from_slice(&[0xBBu8; 20]); // outputToken
    deposit_data[64 + 31] = 100; // inputAmount
    deposit_data[96 + 31] = 95; // outputAmount
    deposit_data[156..160].copy_from_slice(&1700000000u32.to_be_bytes()); // quoteTimestamp
    deposit_data[188..192].copy_from_slice(&1700003600u32.to_be_bytes()); // fillDeadline
    deposit_data[236..256].copy_from_slice(&[0xCCu8; 20]); // recipient

    let deposit_log = EvmLogEntry {
        address: spoke_pool,
        topics: vec![deposit_topic0, topic1, topic2, topic3],
        data: deposit_data,
    };

    // Build CoW Trade log
    let trade_topic0 = keccak256("Trade(address,address,address,uint256,uint256,uint256,bytes)");
    let mut owner_bytes = [0u8; 32];
    owner_bytes[12..32].copy_from_slice(&[0x11u8; 20]);
    let trade_topic1 = B256::from(owner_bytes);

    let order_uid = "0xdeadbeef";
    let uid_bytes = alloy_primitives::hex::decode("deadbeef").unwrap();
    let uid_padded_len = uid_bytes.len().div_ceil(32) * 32;
    let mut trade_data = vec![0u8; 7 * 32 + uid_padded_len];
    trade_data[160 + 31] = 192; // offset
    trade_data[192 + 31] = uid_bytes.len() as u8; // length
    trade_data[224..224 + uid_bytes.len()].copy_from_slice(&uid_bytes);

    let trade_log = EvmLogEntry {
        address: settlement,
        topics: vec![trade_topic0, trade_topic1],
        data: trade_data,
    };

    let result = get_deposit_params(1, order_uid, &[deposit_log, trade_log], None);
    assert!(result.is_some());
    let params = result.unwrap();
    assert_eq!(params.destination_chain_id, 42161);
    assert_eq!(params.input_amount, U256::from(100u64));
    assert_eq!(params.source_chain_id, 1);
}

#[test]
fn get_deposit_params_returns_none_when_order_not_found() {
    use alloy_primitives::{Address, B256, keccak256};
    use cow_rs::bridging::{
        EvmLogEntry,
        across::{across_spoke_pool_addresses, get_deposit_params},
    };

    let spoke_pools = across_spoke_pool_addresses();
    let spoke_pool = *spoke_pools.get(&1).unwrap();
    let settlement: Address = "0x9008D19f58AAbD9eD0D60971565AA8510560ab41".parse().unwrap();

    let deposit_topic0 = keccak256(
        "FundsDeposited(bytes32,bytes32,uint256,uint256,uint256,uint256,uint32,uint32,uint32,bytes32,bytes32,bytes32,bytes)",
    );

    let deposit_log = EvmLogEntry {
        address: spoke_pool,
        topics: vec![deposit_topic0, B256::ZERO, B256::ZERO, B256::ZERO],
        data: vec![0u8; 9 * 32],
    };

    let trade_topic0 = keccak256("Trade(address,address,address,uint256,uint256,uint256,bytes)");
    let uid_bytes = b"different";
    let uid_padded_len = uid_bytes.len().div_ceil(32) * 32;
    let mut trade_data = vec![0u8; 7 * 32 + uid_padded_len];
    trade_data[160 + 31] = 192;
    trade_data[192 + 31] = uid_bytes.len() as u8;
    trade_data[224..224 + uid_bytes.len()].copy_from_slice(uid_bytes);

    let trade_log = EvmLogEntry {
        address: settlement,
        topics: vec![trade_topic0, B256::ZERO],
        data: trade_data,
    };

    // Looking for an order that doesn't match
    let result = get_deposit_params(1, "0xnotfound", &[deposit_log, trade_log], None);
    assert!(result.is_none());
}

// ── create_across_deposit_call ─────────────────────────────────────────────

#[test]
fn create_across_deposit_call_success() {
    use cow_rs::bridging::across::{AcrossDepositCallParams, create_across_deposit_call};

    let params = AcrossDepositCallParams {
        request: sample_request(),
        suggested_fees: sample_suggested_fees(),
        cow_shed_account: address!("1111111111111111111111111111111111111111"),
    };

    let result = create_across_deposit_call(&params);
    assert!(result.is_ok());
    let call = result.unwrap();
    assert!(!call.data.is_empty());
    // Data should start with depositV3 selector (4 bytes)
    assert!(call.data.len() > 4);
    // Value should be zero (not native currency)
    assert_eq!(call.value, U256::ZERO);
}

#[test]
fn create_across_deposit_call_with_receiver() {
    use cow_rs::bridging::across::{AcrossDepositCallParams, create_across_deposit_call};

    let mut req = sample_request();
    req.receiver = Some("0x4444444444444444444444444444444444444444".to_owned());

    let params = AcrossDepositCallParams {
        request: req,
        suggested_fees: sample_suggested_fees(),
        cow_shed_account: address!("1111111111111111111111111111111111111111"),
    };

    let result = create_across_deposit_call(&params);
    assert!(result.is_ok());
}

#[test]
fn create_across_deposit_call_unknown_chain_fails() {
    use cow_rs::bridging::across::{AcrossDepositCallParams, create_across_deposit_call};

    let mut req = sample_request();
    req.sell_chain_id = 99999;

    let params = AcrossDepositCallParams {
        request: req,
        suggested_fees: sample_suggested_fees(),
        cow_shed_account: address!("1111111111111111111111111111111111111111"),
    };

    let result = create_across_deposit_call(&params);
    assert!(result.is_err());
}

// ── Across token mapping for all chains ────────────────────────────────────

#[test]
fn across_token_mapping_contains_optimism() {
    use cow_rs::bridging::across::across_token_mapping;
    let mapping = across_token_mapping();
    assert!(mapping.contains_key(&10)); // Optimism
    let optimism = mapping.get(&10).unwrap();
    assert!(optimism.tokens.contains_key("usdc"));
    assert!(optimism.tokens.contains_key("weth"));
}

#[test]
fn across_token_mapping_base_tokens() {
    use cow_rs::bridging::across::across_token_mapping;
    let mapping = across_token_mapping();
    let base = mapping.get(&8453).unwrap();
    assert!(base.tokens.contains_key("usdc"));
    assert!(base.tokens.contains_key("weth"));
    assert!(base.tokens.contains_key("dai"));
}

// ── Across spoke pool for all chains ───────────────────────────────────────

#[test]
fn across_spoke_pool_addresses_contains_all_chains() {
    use cow_rs::bridging::across::across_spoke_pool_addresses;
    let pools = across_spoke_pool_addresses();
    // Check that all expected chains have pools
    assert!(pools.contains_key(&137)); // Polygon
    assert!(pools.contains_key(&10)); // Optimism
    assert!(pools.contains_key(&11155111)); // Sepolia
}

// ── get_token_by_address_and_chain_id for various chains ───────────────────

#[test]
fn get_token_by_address_and_chain_id_returns_none_for_unknown_address() {
    use alloy_primitives::Address;
    use cow_rs::bridging::across::get_token_by_address_and_chain_id;
    let result = get_token_by_address_and_chain_id(Address::ZERO, 1);
    assert!(result.is_none());
}

// ── to_bridge_quote_result with buy kind ───────────────────────────────────

#[test]
fn across_to_bridge_quote_result_buy_kind() {
    let mut req = sample_request();
    req.kind = OrderKind::Buy;
    let fees = sample_suggested_fees();
    let result = to_bridge_quote_result(&req, 50, &fees).unwrap();
    assert!(!result.is_sell);
}

// ── to_bridge_quote_result with different decimals ─────────────────────────

#[test]
fn across_to_bridge_quote_result_different_decimals() {
    let mut req = sample_request();
    req.sell_token_decimals = 18;
    req.buy_token_decimals = 6;
    let fees = sample_suggested_fees();
    let result = to_bridge_quote_result(&req, 50, &fees).unwrap();
    // buy amount before fee should be scaled down due to decimal difference
    assert!(result.amounts_and_costs.before_fee.buy_amount < req.sell_amount);
}

// ══════════════════════════════════════════════════════════════════════════════
// NEW COVERAGE TESTS — sdk.rs
// ══════════════════════════════════════════════════════════════════════════════

// ── Type guard functions ───────────────────────────────────────────────────

#[test]
fn is_bridge_quote_and_post_returns_true_for_cross_chain() {
    use cow_rs::bridging::sdk::{
        BridgeQuoteAndPost, CrossChainQuoteAndPost, QuoteAndPost, is_bridge_quote_and_post,
        is_quote_and_post,
    };

    let cross_chain = CrossChainQuoteAndPost::CrossChain(Box::new(BridgeQuoteAndPost {
        swap: QuoteBridgeResponse {
            provider: "mock".to_owned(),
            sell_amount: U256::ZERO,
            buy_amount: U256::ZERO,
            fee_amount: U256::ZERO,
            estimated_secs: 0,
            bridge_hook: None,
        },
        bridge: cow_rs::bridging::types::BridgeQuoteResults {
            provider_info: cow_rs::bridging::types::BridgeProviderInfo {
                name: "test".to_owned(),
                logo_url: String::new(),
                dapp_id: "test-dapp".to_owned(),
                website: String::new(),
                provider_type: cow_rs::bridging::types::BridgeProviderType::HookBridgeProvider,
            },
            quote: cow_rs::bridging::types::BridgeQuoteResult {
                id: None,
                signature: None,
                attestation_signature: None,
                quote_body: None,
                is_sell: true,
                amounts_and_costs: cow_rs::bridging::types::BridgeQuoteAmountsAndCosts {
                    before_fee: cow_rs::bridging::types::BridgeAmounts {
                        sell_amount: U256::ZERO,
                        buy_amount: U256::ZERO,
                    },
                    after_fee: cow_rs::bridging::types::BridgeAmounts {
                        sell_amount: U256::ZERO,
                        buy_amount: U256::ZERO,
                    },
                    after_slippage: cow_rs::bridging::types::BridgeAmounts {
                        sell_amount: U256::ZERO,
                        buy_amount: U256::ZERO,
                    },
                    costs: cow_rs::bridging::types::BridgeCosts {
                        bridging_fee: cow_rs::bridging::types::BridgingFee {
                            fee_bps: 0,
                            amount_in_sell_currency: U256::ZERO,
                            amount_in_buy_currency: U256::ZERO,
                        },
                    },
                    slippage_bps: 0,
                },
                expected_fill_time_seconds: None,
                quote_timestamp: 0,
                fees: cow_rs::bridging::types::BridgeFees {
                    bridge_fee: U256::ZERO,
                    destination_gas_fee: U256::ZERO,
                },
                limits: cow_rs::bridging::types::BridgeLimits {
                    min_deposit: U256::ZERO,
                    max_deposit: U256::ZERO,
                },
            },
            bridge_call_details: None,
            bridge_receiver_override: None,
        },
    }));

    assert!(is_bridge_quote_and_post(&cross_chain));
    assert!(!is_quote_and_post(&cross_chain));

    let same_chain = CrossChainQuoteAndPost::SameChain(Box::new(QuoteAndPost {
        quote: QuoteBridgeResponse {
            provider: "mock".to_owned(),
            sell_amount: U256::ZERO,
            buy_amount: U256::ZERO,
            fee_amount: U256::ZERO,
            estimated_secs: 0,
            bridge_hook: None,
        },
    }));

    assert!(!is_bridge_quote_and_post(&same_chain));
    assert!(is_quote_and_post(&same_chain));
}

// ── assert_is_* functions ──────────────────────────────────────────────────

#[test]
fn assert_is_bridge_quote_and_post_errors_on_same_chain() {
    use cow_rs::bridging::sdk::{
        CrossChainQuoteAndPost, QuoteAndPost, assert_is_bridge_quote_and_post,
    };

    let same_chain = CrossChainQuoteAndPost::SameChain(Box::new(QuoteAndPost {
        quote: QuoteBridgeResponse {
            provider: "mock".to_owned(),
            sell_amount: U256::ZERO,
            buy_amount: U256::ZERO,
            fee_amount: U256::ZERO,
            estimated_secs: 0,
            bridge_hook: None,
        },
    }));

    let result = assert_is_bridge_quote_and_post(&same_chain);
    assert!(result.is_err());
}

#[test]
fn assert_is_quote_and_post_errors_on_cross_chain() {
    use cow_rs::bridging::sdk::{
        BridgeQuoteAndPost, CrossChainQuoteAndPost, assert_is_quote_and_post,
    };

    let cross_chain = CrossChainQuoteAndPost::CrossChain(Box::new(BridgeQuoteAndPost {
        swap: QuoteBridgeResponse {
            provider: "mock".to_owned(),
            sell_amount: U256::ZERO,
            buy_amount: U256::ZERO,
            fee_amount: U256::ZERO,
            estimated_secs: 0,
            bridge_hook: None,
        },
        bridge: cow_rs::bridging::types::BridgeQuoteResults {
            provider_info: cow_rs::bridging::types::BridgeProviderInfo {
                name: "test".to_owned(),
                logo_url: String::new(),
                dapp_id: String::new(),
                website: String::new(),
                provider_type: cow_rs::bridging::types::BridgeProviderType::HookBridgeProvider,
            },
            quote: cow_rs::bridging::types::BridgeQuoteResult {
                id: None,
                signature: None,
                attestation_signature: None,
                quote_body: None,
                is_sell: true,
                amounts_and_costs: cow_rs::bridging::types::BridgeQuoteAmountsAndCosts {
                    before_fee: cow_rs::bridging::types::BridgeAmounts {
                        sell_amount: U256::ZERO,
                        buy_amount: U256::ZERO,
                    },
                    after_fee: cow_rs::bridging::types::BridgeAmounts {
                        sell_amount: U256::ZERO,
                        buy_amount: U256::ZERO,
                    },
                    after_slippage: cow_rs::bridging::types::BridgeAmounts {
                        sell_amount: U256::ZERO,
                        buy_amount: U256::ZERO,
                    },
                    costs: cow_rs::bridging::types::BridgeCosts {
                        bridging_fee: cow_rs::bridging::types::BridgingFee {
                            fee_bps: 0,
                            amount_in_sell_currency: U256::ZERO,
                            amount_in_buy_currency: U256::ZERO,
                        },
                    },
                    slippage_bps: 0,
                },
                expected_fill_time_seconds: None,
                quote_timestamp: 0,
                fees: cow_rs::bridging::types::BridgeFees {
                    bridge_fee: U256::ZERO,
                    destination_gas_fee: U256::ZERO,
                },
                limits: cow_rs::bridging::types::BridgeLimits {
                    min_deposit: U256::ZERO,
                    max_deposit: U256::ZERO,
                },
            },
            bridge_call_details: None,
            bridge_receiver_override: None,
        },
    }));

    let result = assert_is_quote_and_post(&cross_chain);
    assert!(result.is_err());
}

// ── Stub async functions return expected errors ────────────────────────────

#[tokio::test]
async fn get_bridge_signed_hook_returns_tx_build_error() {
    use cow_rs::bridging::sdk::get_bridge_signed_hook;

    let quote = cow_rs::bridging::types::BridgeQuoteResult {
        id: None,
        signature: None,
        attestation_signature: None,
        quote_body: None,
        is_sell: true,
        amounts_and_costs: cow_rs::bridging::types::BridgeQuoteAmountsAndCosts {
            before_fee: cow_rs::bridging::types::BridgeAmounts {
                sell_amount: U256::ZERO,
                buy_amount: U256::ZERO,
            },
            after_fee: cow_rs::bridging::types::BridgeAmounts {
                sell_amount: U256::ZERO,
                buy_amount: U256::ZERO,
            },
            after_slippage: cow_rs::bridging::types::BridgeAmounts {
                sell_amount: U256::ZERO,
                buy_amount: U256::ZERO,
            },
            costs: cow_rs::bridging::types::BridgeCosts {
                bridging_fee: cow_rs::bridging::types::BridgingFee {
                    fee_bps: 0,
                    amount_in_sell_currency: U256::ZERO,
                    amount_in_buy_currency: U256::ZERO,
                },
            },
            slippage_bps: 0,
        },
        expected_fill_time_seconds: None,
        quote_timestamp: 0,
        fees: cow_rs::bridging::types::BridgeFees {
            bridge_fee: U256::ZERO,
            destination_gas_fee: U256::ZERO,
        },
        limits: cow_rs::bridging::types::BridgeLimits {
            min_deposit: U256::ZERO,
            max_deposit: U256::ZERO,
        },
    };

    let result = get_bridge_signed_hook(&quote, &[]).await;
    assert!(result.is_err());
    assert!(matches!(result, Err(BridgeError::TxBuildError(_))));
}

#[tokio::test]
async fn get_quote_with_bridge_returns_tx_build_error() {
    use cow_rs::bridging::sdk::{GetQuoteWithBridgeParams, get_quote_with_bridge};

    let params =
        GetQuoteWithBridgeParams { swap_and_bridge_request: sample_request(), slippage_bps: 50 };

    let result = get_quote_with_bridge(&params).await;
    assert!(result.is_err());
    assert!(matches!(result, Err(BridgeError::TxBuildError(_))));
}

#[tokio::test]
async fn get_quote_without_bridge_returns_tx_build_error() {
    use cow_rs::bridging::sdk::get_quote_without_bridge;
    let result = get_quote_without_bridge(&sample_request()).await;
    assert!(result.is_err());
    assert!(matches!(result, Err(BridgeError::TxBuildError(_))));
}

#[tokio::test]
async fn get_swap_quote_returns_tx_build_error() {
    use cow_rs::bridging::sdk::get_swap_quote;
    let result = get_swap_quote(&sample_request()).await;
    assert!(result.is_err());
    assert!(matches!(result, Err(BridgeError::TxBuildError(_))));
}

#[tokio::test]
async fn create_post_swap_order_from_quote_returns_tx_build_error() {
    use cow_rs::bridging::sdk::{BridgeQuoteAndPost, create_post_swap_order_from_quote};

    let quote = BridgeQuoteAndPost {
        swap: QuoteBridgeResponse {
            provider: "mock".to_owned(),
            sell_amount: U256::ZERO,
            buy_amount: U256::ZERO,
            fee_amount: U256::ZERO,
            estimated_secs: 0,
            bridge_hook: None,
        },
        bridge: cow_rs::bridging::types::BridgeQuoteResults {
            provider_info: cow_rs::bridging::types::BridgeProviderInfo {
                name: "test".to_owned(),
                logo_url: String::new(),
                dapp_id: String::new(),
                website: String::new(),
                provider_type: cow_rs::bridging::types::BridgeProviderType::HookBridgeProvider,
            },
            quote: cow_rs::bridging::types::BridgeQuoteResult {
                id: None,
                signature: None,
                attestation_signature: None,
                quote_body: None,
                is_sell: true,
                amounts_and_costs: cow_rs::bridging::types::BridgeQuoteAmountsAndCosts {
                    before_fee: cow_rs::bridging::types::BridgeAmounts {
                        sell_amount: U256::ZERO,
                        buy_amount: U256::ZERO,
                    },
                    after_fee: cow_rs::bridging::types::BridgeAmounts {
                        sell_amount: U256::ZERO,
                        buy_amount: U256::ZERO,
                    },
                    after_slippage: cow_rs::bridging::types::BridgeAmounts {
                        sell_amount: U256::ZERO,
                        buy_amount: U256::ZERO,
                    },
                    costs: cow_rs::bridging::types::BridgeCosts {
                        bridging_fee: cow_rs::bridging::types::BridgingFee {
                            fee_bps: 0,
                            amount_in_sell_currency: U256::ZERO,
                            amount_in_buy_currency: U256::ZERO,
                        },
                    },
                    slippage_bps: 0,
                },
                expected_fill_time_seconds: None,
                quote_timestamp: 0,
                fees: cow_rs::bridging::types::BridgeFees {
                    bridge_fee: U256::ZERO,
                    destination_gas_fee: U256::ZERO,
                },
                limits: cow_rs::bridging::types::BridgeLimits {
                    min_deposit: U256::ZERO,
                    max_deposit: U256::ZERO,
                },
            },
            bridge_call_details: None,
            bridge_receiver_override: None,
        },
    };

    let result = create_post_swap_order_from_quote(&quote).await;
    assert!(result.is_err());
    assert!(matches!(result, Err(BridgeError::TxBuildError(_))));
}

#[tokio::test]
async fn get_intermediate_swap_result_returns_tx_build_error() {
    use cow_rs::bridging::sdk::get_intermediate_swap_result;
    let result = get_intermediate_swap_result(&sample_request()).await;
    assert!(result.is_err());
    assert!(matches!(result, Err(BridgeError::TxBuildError(_))));
}

#[tokio::test]
async fn get_quote_with_hook_bridge_returns_tx_build_error() {
    use cow_rs::bridging::sdk::{GetQuoteWithBridgeParams, get_quote_with_hook_bridge};
    let params =
        GetQuoteWithBridgeParams { swap_and_bridge_request: sample_request(), slippage_bps: 50 };
    let result = get_quote_with_hook_bridge(&params).await;
    assert!(result.is_err());
    assert!(matches!(result, Err(BridgeError::TxBuildError(_))));
}

#[tokio::test]
async fn get_quote_with_receiver_account_bridge_returns_tx_build_error() {
    use cow_rs::bridging::sdk::{GetQuoteWithBridgeParams, get_quote_with_receiver_account_bridge};
    let params =
        GetQuoteWithBridgeParams { swap_and_bridge_request: sample_request(), slippage_bps: 50 };
    let result = get_quote_with_receiver_account_bridge(&params).await;
    assert!(result.is_err());
    assert!(matches!(result, Err(BridgeError::TxBuildError(_))));
}

// ── QuoteStrategy ──────────────────────────────────────────────────────────

#[test]
fn quote_strategy_names() {
    use cow_rs::bridging::sdk::QuoteStrategy;
    assert_eq!(QuoteStrategy::Single.name(), "SingleQuoteStrategy");
    assert_eq!(QuoteStrategy::Multi.name(), "MultiQuoteStrategy");
    assert_eq!(QuoteStrategy::Best.name(), "BestQuoteStrategy");
}

#[test]
fn create_strategies_returns_three() {
    use cow_rs::bridging::sdk::{QuoteStrategy, create_strategies};
    let strategies = create_strategies();
    assert_eq!(strategies.len(), 3);
    assert_eq!(strategies[0], QuoteStrategy::Single);
    assert_eq!(strategies[1], QuoteStrategy::Multi);
    assert_eq!(strategies[2], QuoteStrategy::Best);
}

// ── get_cache_key ──────────────────────────────────────────────────────────

#[test]
fn get_cache_key_format() {
    use cow_rs::bridging::sdk::get_cache_key;
    let req = sample_request();
    let key = get_cache_key(&req);
    assert!(key.contains("1-42161-"));
    assert!(key.contains("0x"));
}

#[test]
fn get_cache_key_deterministic() {
    use cow_rs::bridging::sdk::get_cache_key;
    let req = sample_request();
    assert_eq!(get_cache_key(&req), get_cache_key(&req));
}

// ── safe_call_best_quote_callback ──────────────────────────────────────────

#[test]
fn safe_call_best_quote_callback_invokes_callback() {
    use cow_rs::bridging::sdk::safe_call_best_quote_callback;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);

    let result = cow_rs::bridging::types::MultiQuoteResult {
        provider_dapp_id: "test".to_owned(),
        quote: None,
        error: None,
    };

    safe_call_best_quote_callback(
        Some(move |_: &cow_rs::bridging::types::MultiQuoteResult| {
            called_clone.store(true, Ordering::SeqCst);
        }),
        &result,
    );

    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn safe_call_best_quote_callback_none_is_noop() {
    use cow_rs::bridging::sdk::safe_call_best_quote_callback;

    let result = cow_rs::bridging::types::MultiQuoteResult {
        provider_dapp_id: "test".to_owned(),
        quote: None,
        error: None,
    };

    safe_call_best_quote_callback(None::<fn(&cow_rs::bridging::types::MultiQuoteResult)>, &result);
}

#[test]
fn safe_call_progressive_callback_invokes_callback() {
    use cow_rs::bridging::sdk::safe_call_progressive_callback;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);

    let result = cow_rs::bridging::types::MultiQuoteResult {
        provider_dapp_id: "test".to_owned(),
        quote: None,
        error: None,
    };

    safe_call_progressive_callback(
        Some(move |_: &cow_rs::bridging::types::MultiQuoteResult| {
            called_clone.store(true, Ordering::SeqCst);
        }),
        &result,
    );

    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn safe_call_progressive_callback_none_is_noop() {
    use cow_rs::bridging::sdk::safe_call_progressive_callback;

    let result = cow_rs::bridging::types::MultiQuoteResult {
        provider_dapp_id: "test".to_owned(),
        quote: None,
        error: None,
    };

    safe_call_progressive_callback(None::<fn(&cow_rs::bridging::types::MultiQuoteResult)>, &result);
}

// ── BridgingSdk Debug and builder ──────────────────────────────────────────

#[test]
fn bridging_sdk_debug_format() {
    let sdk = BridgingSdk::new();
    let debug = format!("{sdk:?}");
    assert!(debug.contains("BridgingSdk"));
    assert!(debug.contains("provider_count"));
}

#[test]
fn bridging_sdk_with_bungee_adds_provider() {
    let sdk = BridgingSdk::new().with_bungee("test-key");
    assert_eq!(sdk.provider_count(), 1);
}

#[test]
fn bridging_sdk_default_has_no_providers() {
    let sdk = BridgingSdk::default();
    assert_eq!(sdk.provider_count(), 0);
}

// ── get_cross_chain_order ──────────────────────────────────────────────────

#[test]
fn get_cross_chain_order_returns_error_for_empty_logs() {
    use cow_rs::bridging::sdk::{GetCrossChainOrderParams, get_cross_chain_order};

    let params = GetCrossChainOrderParams {
        chain_id: 1,
        order_id: "0xdeadbeef".to_owned(),
        full_app_data: None,
        trade_tx_hash: "0xabc".to_owned(),
        logs: &[],
        settlement_override: None,
    };

    let result = get_cross_chain_order(&params);
    assert!(result.is_err());
}

// ── get_all_quotes with unsupported providers ──────────────────────────────

#[tokio::test]
async fn sdk_get_all_quotes_empty_for_unsupported_providers() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(UnsupportedProvider::default());

    let req = sample_request();
    let results = sdk.get_all_quotes(&req).await;
    assert!(results.is_empty());
}

// ══════════════════════════════════════════════════════════════════════════════
// NEW COVERAGE TESTS — bungee.rs
// ══════════════════════════════════════════════════════════════════════════════

// ── BungeeProvider name and supports_route ─────────────────────────────────

#[test]
fn bungee_provider_name_is_bungee() {
    use cow_rs::bridging::bungee::BungeeProvider;
    let provider = BungeeProvider::new("test-key");
    assert_eq!(BridgeProvider::name(&provider), "bungee");
}

#[test]
fn bungee_provider_supports_any_route() {
    use cow_rs::bridging::bungee::BungeeProvider;
    let provider = BungeeProvider::new("test-key");
    assert!(BridgeProvider::supports_route(&provider, 1, 42161));
    assert!(BridgeProvider::supports_route(&provider, 99999, 0));
}

// ── decode_amounts for GnosisNative ────────────────────────────────────────

#[test]
fn decode_amounts_for_gnosis_native_3bf5c228() {
    // For GnosisNative 0x3bf5c228: bytes_start_index=136, amount at hex offset 2+16=18
    // But the amount is at bytes_string_start_index=18 from start of tx_data
    let amount_hex = "0000000000000000000000000000000000000000000000000000000000000064"; // 100
    // Need route_id (4 bytes) + selector (4 bytes) + enough padding before the amount at offset 136
    // The bytes_string indices are relative to the tx_data string
    // bytes_string_start_index = 2 + 8*2 = 18, so amount starts at char 18 of tx_data
    let tx_data = format!("0x112233443bf5c228{amount_hex}");
    let decoded = decode_amounts_bungee_tx_data(&tx_data, BungeeBridge::GnosisNative).unwrap();
    assert_eq!(decoded.input_amount, U256::from(100u64));
}

#[test]
fn decode_amounts_for_gnosis_native_fcb23eb0() {
    let amount_hex = "0000000000000000000000000000000000000000000000000000000000000032"; // 50
    let tx_data = format!("0x11223344fcb23eb0{amount_hex}");
    let decoded = decode_amounts_bungee_tx_data(&tx_data, BungeeBridge::GnosisNative).unwrap();
    assert_eq!(decoded.input_amount, U256::from(50u64));
}

// ── decode_amounts truncated data ──────────────────────────────────────────

#[test]
fn decode_amounts_rejects_truncated_amount_field() {
    // Valid route_id + selector, but data too short for the amount
    let tx_data = "0x11223344cc54d2240000000000000000";
    let result = decode_amounts_bungee_tx_data(tx_data, BungeeBridge::Across);
    assert!(result.is_err());
}

// ── create_bungee_deposit_call for CircleCctp ──────────────────────────────

#[test]
fn create_bungee_deposit_call_cctp() {
    use cow_rs::bridging::bungee::{BungeeDepositCallParams, create_bungee_deposit_call};
    let amount_hex = "0000000000000000000000000000000000000000000000000000000000000064";
    let build_tx_data = format!("0x11223344b7dfe9d0{amount_hex}");

    let params = BungeeDepositCallParams {
        request: sample_request(),
        build_tx_data,
        input_amount: U256::from(100u64),
        bridge: BungeeBridge::CircleCctp,
    };

    let result = create_bungee_deposit_call(&params);
    assert!(result.is_ok());
    let call = result.unwrap();
    assert!(!call.data.is_empty());
    assert_eq!(call.value, U256::ZERO);
}

// ── create_bungee_deposit_call for GnosisNative ────────────────────────────

#[test]
fn create_bungee_deposit_call_gnosis_native() {
    use cow_rs::bridging::bungee::{BungeeDepositCallParams, create_bungee_deposit_call};
    let amount_hex = "0000000000000000000000000000000000000000000000000000000000000064";
    let build_tx_data = format!("0x112233443bf5c228{amount_hex}");

    let mut req = sample_request();
    req.sell_chain_id = 100; // GnosisChain

    let params = BungeeDepositCallParams {
        request: req,
        build_tx_data,
        input_amount: U256::from(100u64),
        bridge: BungeeBridge::GnosisNative,
    };

    let result = create_bungee_deposit_call(&params);
    assert!(result.is_ok());
}

// ── create_bungee_deposit_call with native token ───────────────────────────

#[test]
fn create_bungee_deposit_call_native_token_has_value() {
    use cow_rs::bridging::bungee::{BungeeDepositCallParams, create_bungee_deposit_call};
    let amount_hex = "0000000000000000000000000000000000000000000000000000000000000064";
    let build_tx_data = format!("0x11223344cc54d224{amount_hex}");

    let mut req = sample_request();
    // Set sell_token to native currency address
    req.sell_token = cow_rs::config::NATIVE_CURRENCY_ADDRESS;

    let params = BungeeDepositCallParams {
        request: req,
        build_tx_data,
        input_amount: U256::from(100u64),
        bridge: BungeeBridge::Across,
    };

    let result = create_bungee_deposit_call(&params);
    assert!(result.is_ok());
    let call = result.unwrap();
    assert_eq!(call.value, U256::from(100u64));
}

// ── create_bungee_deposit_call with unsupported selector ───────────────────

#[test]
fn create_bungee_deposit_call_unsupported_selector_fails() {
    use cow_rs::bridging::bungee::{BungeeDepositCallParams, create_bungee_deposit_call};
    let amount_hex = "0000000000000000000000000000000000000000000000000000000000000064";
    let build_tx_data = format!("0x1122334400000000{amount_hex}");

    let params = BungeeDepositCallParams {
        request: sample_request(),
        build_tx_data,
        input_amount: U256::from(100u64),
        bridge: BungeeBridge::Across,
    };

    let result = create_bungee_deposit_call(&params);
    assert!(result.is_err());
}

// ── bungee_to_bridge_quote_result edge cases ───────────────────────────────

#[test]
fn bungee_to_bridge_quote_result_with_quote_body() {
    let req = sample_request();
    let result = bungee_to_bridge_quote_result(
        &req,
        100,
        U256::from(900_000_000u64),
        U256::from(100_000_000u64),
        1700000000,
        300,
        Some("q-2".to_owned()),
        Some("{\"route\": \"test\"}".to_owned()),
    )
    .unwrap();

    assert_eq!(result.id, Some("q-2".to_owned()));
    assert_eq!(result.quote_body, Some("{\"route\": \"test\"}".to_owned()));
    assert_eq!(result.amounts_and_costs.slippage_bps, 100);
    assert_eq!(result.fees.bridge_fee, U256::from(100_000_000u64));
    assert_eq!(result.fees.destination_gas_fee, U256::ZERO);
}

#[test]
fn bungee_to_bridge_quote_result_buy_kind() {
    let mut req = sample_request();
    req.kind = OrderKind::Buy;

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

    assert!(!result.is_sell);
}

// ── is_valid_quote_response edge cases ─────────────────────────────────────

#[test]
fn is_valid_quote_response_rejects_missing_success() {
    let resp = json!({ "result": { "manualRoutes": [] } });
    assert!(!is_valid_quote_response(&resp));
}

#[test]
fn is_valid_quote_response_rejects_non_bool_success() {
    let resp = json!({ "success": "yes", "result": { "manualRoutes": [] } });
    assert!(!is_valid_quote_response(&resp));
}

#[test]
fn is_valid_quote_response_rejects_non_array_routes() {
    let resp = json!({
        "success": true,
        "result": { "manualRoutes": "not an array" }
    });
    assert!(!is_valid_quote_response(&resp));
}

#[test]
fn is_valid_quote_response_rejects_route_without_output() {
    let resp = json!({
        "success": true,
        "result": {
            "manualRoutes": [{
                "quoteId": "q-1",
                "estimatedTime": 60,
                "routeDetails": { "routeFee": { "amount": "5" } }
            }]
        }
    });
    assert!(!is_valid_quote_response(&resp));
}

#[test]
fn is_valid_quote_response_rejects_route_without_estimated_time() {
    let resp = json!({
        "success": true,
        "result": {
            "manualRoutes": [{
                "quoteId": "q-1",
                "output": { "amount": "100" },
                "routeDetails": { "routeFee": { "amount": "5" } }
            }]
        }
    });
    assert!(!is_valid_quote_response(&resp));
}

#[test]
fn is_valid_quote_response_rejects_route_without_route_fee() {
    let resp = json!({
        "success": true,
        "result": {
            "manualRoutes": [{
                "quoteId": "q-1",
                "output": { "amount": "100" },
                "estimatedTime": 60,
                "routeDetails": {}
            }]
        }
    });
    assert!(!is_valid_quote_response(&resp));
}

// ── is_valid_bungee_events_response edge cases ─────────────────────────────

#[test]
fn is_valid_bungee_events_response_rejects_missing_success() {
    let resp = json!({ "result": [] });
    assert!(!is_valid_bungee_events_response(&resp));
}

#[test]
fn is_valid_bungee_events_response_rejects_non_array_result() {
    let resp = json!({ "success": true, "result": "not array" });
    assert!(!is_valid_bungee_events_response(&resp));
}

#[test]
fn is_valid_bungee_events_response_accepts_empty_result() {
    let resp = json!({ "success": true, "result": [] });
    assert!(is_valid_bungee_events_response(&resp));
}

// ── Bungee constants ───────────────────────────────────────────────────────

#[test]
fn bungee_constants_non_empty() {
    use cow_rs::bridging::bungee::{
        BUNGEE_APPROVE_AND_BRIDGE_V1_ADDRESS, BUNGEE_COWSWAP_LIB_ADDRESS, SOCKET_VERIFIER_ADDRESS,
    };
    assert!(!BUNGEE_APPROVE_AND_BRIDGE_V1_ADDRESS.is_empty());
    assert!(!BUNGEE_COWSWAP_LIB_ADDRESS.is_empty());
    assert!(!SOCKET_VERIFIER_ADDRESS.is_empty());
}

// ── SDK constants ──────────────────────────────────────────────────────────

#[test]
fn sdk_constants_are_set() {
    use cow_rs::bridging::sdk::*;
    assert!(!BUNGEE_API_PATH.is_empty());
    assert!(!BUNGEE_MANUAL_API_PATH.is_empty());
    assert!(!BUNGEE_BASE_URL.is_empty());
    assert!(!BUNGEE_API_URL.is_empty());
    assert!(!BUNGEE_MANUAL_API_URL.is_empty());
    assert!(!BUNGEE_EVENTS_API_URL.is_empty());
    assert!(!ACROSS_API_URL.is_empty());
    assert_eq!(DEFAULT_BRIDGE_SLIPPAGE_BPS, 50);
    assert!(DEFAULT_GAS_COST_FOR_HOOK_ESTIMATION > 0);
    assert!(DEFAULT_EXTRA_GAS_FOR_HOOK_ESTIMATION > 0);
    assert!(DEFAULT_EXTRA_GAS_PROXY_CREATION > 0);
    assert!(!HOOK_DAPP_BRIDGE_PROVIDER_PREFIX.is_empty());
    assert!(!BUNGEE_HOOK_DAPP_ID.is_empty());
    assert!(!ACROSS_HOOK_DAPP_ID.is_empty());
    assert!(!NEAR_INTENTS_HOOK_DAPP_ID.is_empty());
    assert!(BUNGEE_API_FALLBACK_TIMEOUT > 0);
    assert!(DEFAULT_TOTAL_TIMEOUT_MS > 0);
    assert!(DEFAULT_PROVIDER_TIMEOUT_MS > 0);
}

// ── Across event interface constants ───────────────────────────────────────

#[test]
fn across_event_interface_constants() {
    use cow_rs::bridging::across::{
        ACROSS_DEPOSIT_EVENT_INTERFACE, ACROSS_FUNDS_DEPOSITED_TOPIC, COW_TRADE_EVENT_INTERFACE,
        COW_TRADE_EVENT_SIGNATURE,
    };
    assert_eq!(ACROSS_DEPOSIT_EVENT_INTERFACE, ACROSS_FUNDS_DEPOSITED_TOPIC);
    assert_eq!(COW_TRADE_EVENT_INTERFACE, COW_TRADE_EVENT_SIGNATURE);
    assert!(ACROSS_FUNDS_DEPOSITED_TOPIC.contains("FundsDeposited"));
    assert!(COW_TRADE_EVENT_SIGNATURE.contains("Trade"));
}

// ── BridgingSdk type guard functions ────────────────────────────────────────

#[test]
fn is_bridge_quote_and_post_cross_chain() {
    use cow_rs::bridging::{
        sdk::{
            BridgeQuoteAndPost, CrossChainQuoteAndPost, assert_is_bridge_quote_and_post,
            assert_is_quote_and_post, is_bridge_quote_and_post, is_quote_and_post,
        },
        types::{
            BridgeAmounts, BridgeCosts, BridgeFees, BridgeLimits, BridgeProviderInfo,
            BridgeProviderType, BridgeQuoteAmountsAndCosts, BridgeQuoteResult, BridgeQuoteResults,
            BridgingFee,
        },
    };

    let quote_result = BridgeQuoteResult {
        id: None,
        signature: None,
        attestation_signature: None,
        quote_body: None,
        is_sell: true,
        amounts_and_costs: BridgeQuoteAmountsAndCosts {
            before_fee: BridgeAmounts { sell_amount: U256::ZERO, buy_amount: U256::ZERO },
            after_fee: BridgeAmounts { sell_amount: U256::ZERO, buy_amount: U256::ZERO },
            after_slippage: BridgeAmounts { sell_amount: U256::ZERO, buy_amount: U256::ZERO },
            costs: BridgeCosts {
                bridging_fee: BridgingFee {
                    fee_bps: 0,
                    amount_in_sell_currency: U256::ZERO,
                    amount_in_buy_currency: U256::ZERO,
                },
            },
            slippage_bps: 0,
        },
        expected_fill_time_seconds: None,
        quote_timestamp: 0,
        fees: BridgeFees { bridge_fee: U256::ZERO, destination_gas_fee: U256::ZERO },
        limits: BridgeLimits { min_deposit: U256::ZERO, max_deposit: U256::ZERO },
    };

    let cross = CrossChainQuoteAndPost::CrossChain(Box::new(BridgeQuoteAndPost {
        swap: QuoteBridgeResponse {
            provider: "mock".to_owned(),
            sell_amount: U256::ZERO,
            buy_amount: U256::ZERO,
            fee_amount: U256::ZERO,
            estimated_secs: 0,
            bridge_hook: None,
        },
        bridge: BridgeQuoteResults {
            provider_info: BridgeProviderInfo {
                name: "mock".into(),
                logo_url: String::new(),
                dapp_id: "mock-dapp".into(),
                website: String::new(),
                provider_type: BridgeProviderType::HookBridgeProvider,
            },
            quote: quote_result,
            bridge_call_details: None,
            bridge_receiver_override: None,
        },
    }));

    assert!(is_bridge_quote_and_post(&cross));
    assert!(!is_quote_and_post(&cross));
    assert!(assert_is_bridge_quote_and_post(&cross).is_ok());
    assert!(assert_is_quote_and_post(&cross).is_err());
}

#[test]
fn is_quote_and_post_same_chain() {
    use cow_rs::bridging::sdk::{
        CrossChainQuoteAndPost, QuoteAndPost, assert_is_bridge_quote_and_post,
        assert_is_quote_and_post, is_bridge_quote_and_post, is_quote_and_post,
    };

    let same = CrossChainQuoteAndPost::SameChain(Box::new(QuoteAndPost {
        quote: QuoteBridgeResponse {
            provider: "mock".to_owned(),
            sell_amount: U256::ZERO,
            buy_amount: U256::ZERO,
            fee_amount: U256::ZERO,
            estimated_secs: 0,
            bridge_hook: None,
        },
    }));

    assert!(is_quote_and_post(&same));
    assert!(!is_bridge_quote_and_post(&same));
    assert!(assert_is_quote_and_post(&same).is_ok());
    assert!(assert_is_bridge_quote_and_post(&same).is_err());
}

// ── Stub async functions return errors ─────────────────────────────────────

#[tokio::test]
async fn get_bridge_signed_hook_returns_error() {
    use cow_rs::bridging::{
        sdk::get_bridge_signed_hook,
        types::{
            BridgeAmounts, BridgeCosts, BridgeFees, BridgeLimits, BridgeQuoteAmountsAndCosts,
            BridgeQuoteResult, BridgingFee,
        },
    };

    let quote = BridgeQuoteResult {
        id: None,
        signature: None,
        attestation_signature: None,
        quote_body: None,
        is_sell: true,
        amounts_and_costs: BridgeQuoteAmountsAndCosts {
            before_fee: BridgeAmounts { sell_amount: U256::ZERO, buy_amount: U256::ZERO },
            after_fee: BridgeAmounts { sell_amount: U256::ZERO, buy_amount: U256::ZERO },
            after_slippage: BridgeAmounts { sell_amount: U256::ZERO, buy_amount: U256::ZERO },
            costs: BridgeCosts {
                bridging_fee: BridgingFee {
                    fee_bps: 0,
                    amount_in_sell_currency: U256::ZERO,
                    amount_in_buy_currency: U256::ZERO,
                },
            },
            slippage_bps: 0,
        },
        expected_fill_time_seconds: None,
        quote_timestamp: 0,
        fees: BridgeFees { bridge_fee: U256::ZERO, destination_gas_fee: U256::ZERO },
        limits: BridgeLimits { min_deposit: U256::ZERO, max_deposit: U256::ZERO },
    };
    let result = get_bridge_signed_hook(&quote, &[]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_quote_with_bridge_returns_error() {
    use cow_rs::bridging::sdk::{GetQuoteWithBridgeParams, get_quote_with_bridge};
    let params =
        GetQuoteWithBridgeParams { swap_and_bridge_request: sample_request(), slippage_bps: 50 };
    let result = get_quote_with_bridge(&params).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_quote_without_bridge_returns_error() {
    use cow_rs::bridging::sdk::get_quote_without_bridge;
    let result = get_quote_without_bridge(&sample_request()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_swap_quote_returns_error() {
    use cow_rs::bridging::sdk::get_swap_quote;
    let result = get_swap_quote(&sample_request()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_intermediate_swap_result_returns_error() {
    use cow_rs::bridging::sdk::get_intermediate_swap_result;
    let result = get_intermediate_swap_result(&sample_request()).await;
    assert!(result.is_err());
}

// ── QuoteStrategy equality ─────────────────────────────────────────────────

#[test]
fn quote_strategy_equality_and_create() {
    use cow_rs::bridging::sdk::{QuoteStrategy, create_strategies};
    assert_eq!(QuoteStrategy::Single, QuoteStrategy::Single);
    assert_ne!(QuoteStrategy::Single, QuoteStrategy::Multi);

    let strategies = create_strategies();
    assert_eq!(strategies.len(), 3);
    assert_eq!(strategies[0], QuoteStrategy::Single);
    assert_eq!(strategies[1], QuoteStrategy::Multi);
    assert_eq!(strategies[2], QuoteStrategy::Best);
}

// ── BridgingSdk Debug impl ─────────────────────────────────────────────────

#[test]
fn bridging_sdk_debug_with_bungee() {
    let sdk = BridgingSdk::new().with_bungee("test-key");
    let debug = format!("{sdk:?}");
    assert!(debug.contains("BridgingSdk"));
    assert!(debug.contains("provider_count"));
}

// ── BungeeProvider name and supports_route ─────────────────────────────────

#[test]
fn bungee_provider_name_and_supports_route() {
    use cow_rs::bridging::bungee::BungeeProvider;
    let provider = BungeeProvider::new("test-key");
    assert_eq!(provider.name(), "bungee");
    assert!(provider.supports_route(1, 42161));
    assert!(provider.supports_route(1, 8453));
}

// ── create_bungee_deposit_call with native token ───────────────────────────

#[test]
fn create_bungee_deposit_call_native_token() {
    use cow_rs::bridging::bungee::{BungeeDepositCallParams, create_bungee_deposit_call};
    let amount_hex = "0000000000000000000000000000000000000000000000000000000000000064";
    let build_tx_data = format!("0x11223344cc54d224{amount_hex}");

    let mut req = sample_request();
    req.sell_token = cow_rs::NATIVE_CURRENCY_ADDRESS;

    let params = BungeeDepositCallParams {
        request: req,
        build_tx_data,
        input_amount: U256::from(100u64),
        bridge: BungeeBridge::Across,
    };

    let result = create_bungee_deposit_call(&params);
    assert!(result.is_ok());
    let call = result.unwrap();
    // Native token should have non-zero value
    assert_eq!(call.value, U256::from(100u64));
}

// ── Gnosis native bridge decode ────────────────────────────────────────────

#[test]
fn decode_amounts_for_gnosis_native_3bf5c228_padded() {
    // GnosisNative with 0x3bf5c228 has bytes_start_index=136, string_start_index=2+8*2=18
    let mut tx_data = String::from("0x11223344");
    tx_data.push_str("3bf5c228");
    // Pad to reach the amount position at string offset 18..18+64
    // The amount should be at tx_data[18..82] which is right after "0x" + routeId(8) + selector(8)
    // Wait, bytes_string_start_index = 2 + 8*2 = 18 (same for all)
    // But bytes_start_index = 136, which means the amount bytes are at offset 136 from the start of
    // raw data The hex string offset: 2 + 136*2 = 274
    // Actually let me re-read: bytes_string_start_index is always 2 + 8*2 = 18
    // So the amount hex is at tx_data[18..18+64]
    // That's right after "0x11223344" which is 10 chars, then "3bf5c228" is 8 more = 18 chars
    // So the next 64 chars are the amount
    let amount_hex = "0000000000000000000000000000000000000000000000000000000000000064";
    // Already have 18 chars, need to add the rest up to offset 18+64=82
    // We need data starting from position 18 in the tx_data string
    // tx_data = "0x11223344" (10) + "3bf5c228" (8) = 18 chars so far
    // The amount starts at position 18, so add it directly
    tx_data.push_str(amount_hex);
    // Pad enough so the string is long enough
    tx_data.push_str(&"00".repeat(200));

    let decoded = decode_amounts_bungee_tx_data(&tx_data, BungeeBridge::GnosisNative).unwrap();
    assert_eq!(decoded.input_amount, U256::from(100u64));
}

// ── Across status with non-Across bridge and Across fallback ───────────────

#[tokio::test]
async fn status_from_events_across_with_unknown_status_returns_in_progress() {
    let across_unknown = |_: &str| async { Ok("some-unknown-status".to_owned()) };
    let event = make_bungee_event(
        BungeeEventStatus::Completed,
        BungeeEventStatus::Pending,
        BungeeBridgeName::Across,
    );
    let result = get_bridging_status_from_events(Some(&[event]), across_unknown).await.unwrap();
    assert!(matches!(result.status, BridgeStatus::InProgress));
}

// ── BridgeProviderType type guards ─────────────────────────────────────────

#[test]
fn bridge_provider_type_guards() {
    use cow_rs::bridging::types::{BridgeProviderInfo, BridgeProviderType};
    let hook = BridgeProviderType::HookBridgeProvider;
    let receiver = BridgeProviderType::ReceiverAccountBridgeProvider;

    assert!(hook.is_hook_bridge_provider());
    assert!(!hook.is_receiver_account_bridge_provider());
    assert!(!receiver.is_hook_bridge_provider());
    assert!(receiver.is_receiver_account_bridge_provider());

    let info = BridgeProviderInfo {
        name: "Test".into(),
        logo_url: "https://example.com/logo.png".into(),
        dapp_id: "test-dapp".into(),
        website: "https://example.com".into(),
        provider_type: BridgeProviderType::HookBridgeProvider,
    };
    assert!(info.is_hook_bridge_provider());
    assert!(!info.is_receiver_account_bridge_provider());
}
