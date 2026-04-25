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
    clippy::unwrap_used,
    clippy::expect_used
)]
//! Wiremock-based integration tests for [`TradingSdk`] high-level trading methods.
//!
//! These tests live in the `cow-trading` crate (rather than `cow-rs`) so that
//! `cargo llvm-cov -p cow-trading` exercises every code path in `sdk.rs`.
//! They mirror the structure of `crates/cow-rs/tests/trading_mock.rs` while
//! depending only on the lower-level workspace crates.

use alloy_primitives::{Address, B256, U256};
use cow_app_data::types::{OrderClassKind, PartnerFee, PartnerFeeEntry, Utm};
use cow_chains::{Env, NATIVE_CURRENCY_ADDRESS, SupportedChainId};
use cow_orderbook::{
    OrderBookApi, OrderbookClient,
    types::{
        GetOrdersRequest, GetTradesRequest, OrderQuoteRequest, OrderQuoteResponse, QuoteData,
        QuoteSide,
    },
};
use cow_signing::types::UnsignedOrder;
use cow_trading::{
    DEFAULT_SLIPPAGE_BPS, ETH_FLOW_DEFAULT_SLIPPAGE_BPS, GAS_LIMIT_DEFAULT, LimitTradeParameters,
    PostTradeAdditionalParams, SwapAdvancedSettings, TradeParameters, TradingAppDataInfo,
    TradingSdk, TradingSdkConfig, adjust_eth_flow_limit_order_params, adjust_eth_flow_order_params,
    apply_settings_to_limit_trade_parameters, build_app_data, calculate_gas_margin,
    calculate_unique_order_id, generate_app_data_from_doc, get_default_slippage_bps,
    get_default_utm_params, get_eth_flow_cancellation, get_eth_flow_contract,
    get_is_eth_flow_order, get_order_deadline_from_now, get_order_to_sign, get_order_typed_data,
    get_quote_raw, get_quote_with_signer, get_quote_without_signer, get_settlement_cancellation,
    get_settlement_contract, get_slippage_percent, get_trade_parameters_after_quote, get_trader,
    post_cow_protocol_trade, post_sell_native_currency_order, resolve_order_book_api,
    resolve_signer, resolve_slippage_suggestion, swap_params_to_limit_order_params,
    types::{LimitOrderAdvancedSettings, QuoteResults},
    unsigned_order_for_signing,
};
use cow_types::{EcdsaSigningScheme, OrderKind, SigningScheme, TokenBalance};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

/// Well-known Hardhat #0 test private key. Safe to commit; never use elsewhere.
const TEST_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

const SELL_TOKEN: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
const BUY_TOKEN: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";

fn make_sdk(server: &MockServer) -> TradingSdk {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    TradingSdk::new_with_url(config, TEST_KEY, server.uri()).expect("valid test key")
}

fn default_trade_params() -> TradeParameters {
    TradeParameters {
        kind: OrderKind::Sell,
        sell_token: SELL_TOKEN.parse().unwrap(),
        sell_token_decimals: 6,
        buy_token: BUY_TOKEN.parse().unwrap(),
        buy_token_decimals: 18,
        amount: U256::from(1_000_000u64),
        slippage_bps: Some(50),
        receiver: None,
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: None,
        partner_fee: None,
    }
}

fn make_quote_response_json() -> serde_json::Value {
    serde_json::json!({
        "quote": {
            "sellToken":        SELL_TOKEN,
            "buyToken":         BUY_TOKEN,
            "receiver":         null,
            "sellAmount":       "1000000",
            "buyAmount":        "500000000000000",
            "validTo":          9_999_999,
            "appData":          "0x0000000000000000000000000000000000000000000000000000000000000000",
            "feeAmount":        "1000",
            "kind":             "sell",
            "partiallyFillable": false,
            "sellTokenBalance": "erc20",
            "buyTokenBalance":  "erc20"
        },
        "from":       "0x0000000000000000000000000000000000000000",
        "expiration": "2099-01-01T00:00:00.000Z",
        "id":         42,
        "verified":   false
    })
}

fn make_order_json(uid: &str) -> serde_json::Value {
    serde_json::json!({
        "uid":                          uid,
        "owner":                        "0x1111111111111111111111111111111111111111",
        "creationDate":                 "2024-01-01T00:00:00.000Z",
        "status":                       "open",
        "class":                        "limit",
        "sellToken":                    SELL_TOKEN,
        "buyToken":                     BUY_TOKEN,
        "receiver":                     null,
        "sellAmount":                   "1000000",
        "buyAmount":                    "1000000000000000",
        "validTo":                      9999,
        "appData":                      "0x0000000000000000000000000000000000000000000000000000000000000000",
        "fullAppData":                  null,
        "feeAmount":                    "0",
        "kind":                         "sell",
        "partiallyFillable":            false,
        "executedSellAmount":           "0",
        "executedBuyAmount":            "0",
        "executedSellAmountBeforeFees": "0",
        "executedFeeAmount":            "0",
        "invalidated":                  false,
        "signingScheme":                "eip712",
        "signature":                    "0xabcd"
    })
}

fn make_quote_response_struct(
    sell: &str,
    buy: &str,
    fee: &str,
    kind: OrderKind,
) -> OrderQuoteResponse {
    OrderQuoteResponse {
        quote: QuoteData {
            sell_token: SELL_TOKEN.parse().unwrap(),
            buy_token: BUY_TOKEN.parse().unwrap(),
            receiver: None,
            sell_amount: sell.to_owned(),
            buy_amount: buy.to_owned(),
            valid_to: 9_999_999,
            app_data: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            fee_amount: fee.to_owned(),
            kind,
            partially_fillable: false,
            sell_token_balance: TokenBalance::Erc20,
            buy_token_balance: TokenBalance::Erc20,
        },
        from: Address::ZERO,
        expiration: "2099-01-01T00:00:00.000Z".to_owned(),
        id: Some(42),
        verified: false,
        protocol_fee_bps: None,
    }
}

// ── TradingSdk::get_quote ────────────────────────────────────────────────────

#[tokio::test]
async fn get_quote_returns_quote_results() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let quote = sdk.get_quote(default_trade_params()).await.unwrap();

    assert!(!quote.order_to_sign.sell_amount.is_zero());
    assert!(!quote.order_to_sign.buy_amount.is_zero());
    assert_eq!(quote.suggested_slippage_bps, 50);

    // Exercise the QuoteResults Display + accessor surface, which previously
    // had no coverage because no test built a real QuoteResults instance.
    let display = format!("{quote}");
    assert!(display.contains("quote slippage="));
    assert!(std::ptr::eq(quote.order_ref(), &raw const quote.order_to_sign));
    assert!(std::ptr::eq(quote.quote_ref(), &raw const quote.quote_response));
}

#[tokio::test]
async fn get_quote_only_uses_given_owner() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let owner: Address = "0x2222222222222222222222222222222222222222".parse().unwrap();
    let quote = sdk.get_quote_only(owner, default_trade_params()).await.unwrap();

    assert!(!quote.order_to_sign.sell_amount.is_zero());
    assert_eq!(quote.order_to_sign.receiver, owner);
}

#[tokio::test]
async fn get_quote_only_with_settings_applies_slippage_override() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let owner: Address = "0x3333333333333333333333333333333333333333".parse().unwrap();
    let settings = SwapAdvancedSettings { slippage_bps: Some(123), ..Default::default() };
    let quote =
        sdk.get_quote_only_with_settings(owner, default_trade_params(), &settings).await.unwrap();

    assert_eq!(quote.suggested_slippage_bps, 123);
}

#[tokio::test]
async fn get_quote_without_signer_does_not_need_a_signer() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let owner: Address = "0x4444444444444444444444444444444444444444".parse().unwrap();
    let quote =
        get_quote_without_signer(&config, &api, owner, default_trade_params(), None).await.unwrap();
    assert_eq!(quote.order_to_sign.receiver, owner);
}

// ── TradingSdk::post_swap_order_from_quote ───────────────────────────────────

#[tokio::test]
async fn post_swap_order_from_quote_returns_order_id() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let uid = "0x".to_owned() + &"aa".repeat(56);
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let quote = sdk.get_quote(default_trade_params()).await.unwrap();
    let result = sdk.post_swap_order_from_quote(&quote, None).await.unwrap();

    assert!(result.order_id.starts_with("0x"));
    assert!(!result.signature.is_empty());
}

#[tokio::test]
async fn post_swap_order_quotes_and_submits() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let uid = "0x".to_owned() + &"bb".repeat(56);
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let result = sdk.post_swap_order(default_trade_params()).await.unwrap();
    assert_eq!(result.order_id, uid);
}

#[tokio::test]
async fn post_swap_order_from_quote_with_eth_sign() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let uid = "0x".to_owned() + &"70".repeat(56);
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let quote = sdk.get_quote(default_trade_params()).await.unwrap();
    let result =
        sdk.post_swap_order_from_quote(&quote, Some(EcdsaSigningScheme::EthSign)).await.unwrap();
    assert_eq!(result.order_id, uid);
}

// ── post_swap_order_with_settings ───────────────────────────────────────────

#[tokio::test]
async fn post_swap_order_with_settings_submits() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let uid = "0x".to_owned() + &"44".repeat(56);
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let settings =
        SwapAdvancedSettings { app_data: None, slippage_bps: Some(100), partner_fee: None };
    let result =
        sdk.post_swap_order_with_settings(default_trade_params(), &settings).await.unwrap();
    assert_eq!(result.order_id, uid);
}

// ── TradingSdk::get_order ────────────────────────────────────────────────────

#[tokio::test]
async fn get_order_returns_order() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"cc".repeat(56);
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/orders/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_order_json(&uid)))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let order = sdk.get_order(&uid).await.unwrap();
    assert_eq!(order.uid, uid);
}

#[tokio::test]
async fn get_order_multi_env_returns_order() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"58".repeat(56);
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/orders/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_order_json(&uid)))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let order = sdk.get_order_multi_env(&uid).await.unwrap();
    assert_eq!(order.uid, uid);
}

#[tokio::test]
async fn get_orders_paginated_returns_list() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/account/.*/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let req =
        GetOrdersRequest::for_owner("0x1111111111111111111111111111111111111111".parse().unwrap());
    let orders = sdk.get_orders(&req).await.unwrap();
    assert!(orders.is_empty());
}

#[tokio::test]
async fn get_trades_with_request_returns_list() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v2/trades"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let req = GetTradesRequest {
        owner: Some("0x1111111111111111111111111111111111111111".parse().unwrap()),
        order_uid: None,
        limit: Some(5),
        offset: None,
    };
    let trades = sdk.get_trades_with_request(&req).await.unwrap();
    assert!(trades.is_empty());
}

#[tokio::test]
async fn get_native_price_returns_float() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/token/.*/native_price"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "price": 0.000_4 })),
        )
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let token: Address = SELL_TOKEN.parse().unwrap();
    let price = sdk.get_native_price(token).await.unwrap();
    assert!(price > 0.0);
}

#[tokio::test]
async fn get_auction_parses_response() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/auction"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id":                     1,
            "block":                  100,
            "latestSettlementBlock":  99,
            "orders":                 [],
            "prices":                 {}
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let auction = sdk.get_auction().await.unwrap();
    assert_eq!(auction.id, Some(1));
}

#[tokio::test]
async fn get_trades_returns_list() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v2/trades"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let owner: Address = "0x1111111111111111111111111111111111111111".parse().unwrap();
    let trades = sdk.get_trades(owner, Some(10)).await.unwrap();
    assert!(trades.is_empty());
}

#[tokio::test]
async fn get_orders_for_account_returns_list() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/account/.*/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let owner: Address = "0x2222222222222222222222222222222222222222".parse().unwrap();
    let orders = sdk.get_orders_for_account(owner, None).await.unwrap();
    assert!(orders.is_empty());
}

#[tokio::test]
async fn get_version_returns_string() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!("1.2.3")))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let version = sdk.get_version().await.unwrap();
    assert_eq!(version, "1.2.3");
}

#[tokio::test]
async fn get_total_surplus_parses_amount() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/users/.*/total_surplus"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "totalSurplus": "100000000"
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let addr: Address = "0x1111111111111111111111111111111111111111".parse().unwrap();
    let surplus = sdk.get_total_surplus(addr).await.unwrap();
    assert!(!surplus.total_surplus.is_empty());
}

#[tokio::test]
async fn get_app_data_returns_document() {
    let server = MockServer::start().await;
    let hash = "0x".to_owned() + &"dd".repeat(32);
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/app_data/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fullAppData": "{\"appCode\":\"TestApp\"}"
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let result = sdk.get_app_data(&hash).await.unwrap();
    assert!(result.full_app_data.contains("TestApp"));
}

#[tokio::test]
async fn upload_app_data_returns_object() {
    let server = MockServer::start().await;
    let hash = "0x".to_owned() + &"ee".repeat(32);
    Mock::given(matchers::method("PUT"))
        .and(matchers::path_regex(r"/api/v1/app_data/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fullAppData": "{\"appCode\":\"TestApp\"}"
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let result = sdk.upload_app_data(&hash, r#"{"appCode":"TestApp"}"#).await.unwrap();
    assert!(result.full_app_data.contains("TestApp"));
}

#[tokio::test]
async fn upload_app_data_auto_returns_object() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("PUT"))
        .and(matchers::path("/api/v1/app_data"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fullAppData": "{\"appCode\":\"TestApp\"}"
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let result = sdk.upload_app_data_auto(r#"{"appCode":"TestApp"}"#).await.unwrap();
    assert!(result.full_app_data.contains("TestApp"));
}

#[tokio::test]
async fn get_order_status_returns_open() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"ff".repeat(56);
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/orders/.*/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "type": "open",
            "value": null
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let status = sdk.get_order_status(&uid).await.unwrap();
    assert!(status.kind.is_open());
}

#[tokio::test]
async fn get_orders_by_tx_returns_list() {
    let server = MockServer::start().await;
    let tx_hash = "0x".to_owned() + &"ab".repeat(32);
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/transactions/.*/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let orders = sdk.get_orders_by_tx(&tx_hash).await.unwrap();
    assert!(orders.is_empty());
}

#[tokio::test]
async fn get_solver_competition_latest_parses() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/solver_competition/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "auctionId": 1,
            "transactionHash": null,
            "gasPrice": null,
            "liquidityCollectedBlock": null,
            "competitionSimulationBlock": null,
            "auctionStartBlock": 100,
            "solutions": []
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let competition = sdk.get_solver_competition_latest().await.unwrap();
    assert_eq!(competition.auction_id, Some(1));
}

#[tokio::test]
async fn get_solver_competition_by_id_parses() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/solver_competition/\d+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "auctionId": 42,
            "transactionHash": null,
            "gasPrice": null,
            "liquidityCollectedBlock": null,
            "competitionSimulationBlock": null,
            "auctionStartBlock": 100,
            "solutions": []
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let competition = sdk.get_solver_competition(42).await.unwrap();
    assert_eq!(competition.auction_id, Some(42));
}

#[tokio::test]
async fn get_solver_competition_by_tx_parses() {
    let server = MockServer::start().await;
    let tx_hash = "0x".to_owned() + &"ab".repeat(32);
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/solver_competition/by_tx_hash/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "auctionId": 99,
            "transactionHash": null,
            "gasPrice": null,
            "liquidityCollectedBlock": null,
            "competitionSimulationBlock": null,
            "auctionStartBlock": 200,
            "solutions": []
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let competition = sdk.get_solver_competition_by_tx(&tx_hash).await.unwrap();
    assert_eq!(competition.auction_id, Some(99));
}

#[tokio::test]
async fn get_solver_competition_latest_v2_parses() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v2/solver_competition/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "auctionId": 77,
            "transactionHash": null,
            "gasPrice": null,
            "liquidityCollectedBlock": null,
            "competitionSimulationBlock": null,
            "auctionStartBlock": 300,
            "solutions": []
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let competition = sdk.get_solver_competition_latest_v2().await.unwrap();
    assert_eq!(competition.auction_id, Some(77));
}

#[tokio::test]
async fn get_solver_competition_v2_by_id_parses() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v2/solver_competition/\d+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "auctionId": 88,
            "transactionHash": null,
            "gasPrice": null,
            "liquidityCollectedBlock": null,
            "competitionSimulationBlock": null,
            "auctionStartBlock": 400,
            "solutions": []
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let competition = sdk.get_solver_competition_v2(88).await.unwrap();
    assert_eq!(competition.auction_id, Some(88));
}

#[tokio::test]
async fn get_solver_competition_by_tx_v2_parses() {
    let server = MockServer::start().await;
    let tx_hash = "0x".to_owned() + &"cd".repeat(32);
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v2/solver_competition/by_tx_hash/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "auctionId": 55,
            "transactionHash": null,
            "gasPrice": null,
            "liquidityCollectedBlock": null,
            "competitionSimulationBlock": null,
            "auctionStartBlock": 500,
            "solutions": []
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let competition = sdk.get_solver_competition_by_tx_v2(&tx_hash).await.unwrap();
    assert_eq!(competition.auction_id, Some(55));
}

#[tokio::test]
async fn post_limit_order_returns_order_id() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"11".repeat(56);
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: None,
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let result = sdk.post_limit_order(params, None).await.unwrap();
    assert_eq!(result.order_id, uid);
    assert!(!result.signature.is_empty());
}

#[tokio::test]
async fn post_limit_order_with_custom_app_data() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"68".repeat(56);
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: Some("0x1111111111111111111111111111111111111111".parse().unwrap()),
        valid_for: Some(3600),
        valid_to: None,
        partially_fillable: false,
        app_data: Some(
            "0x0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        ),
        partner_fee: Some(PartnerFee::single(PartnerFeeEntry::volume(
            100,
            "0x1111111111111111111111111111111111111111",
        ))),
    };
    let result = sdk.post_limit_order(params, Some(EcdsaSigningScheme::EthSign)).await.unwrap();
    assert_eq!(result.order_id, uid);
}

#[tokio::test]
async fn post_limit_order_with_config_partner_fee() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"69".repeat(56);
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let fee = PartnerFee::single(PartnerFeeEntry::volume(
        50,
        "0x2222222222222222222222222222222222222222",
    ));
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp").with_partner_fee(fee);
    let sdk = TradingSdk::new_with_url(config, TEST_KEY, server.uri()).unwrap();
    let params = LimitTradeParameters {
        kind: OrderKind::Buy,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: None,
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let result = sdk.post_limit_order(params, None).await.unwrap();
    assert_eq!(result.order_id, uid);
}

// ── synchronous SDK methods ─────────────────────────────────────────────────

#[test]
fn address_returns_signer_address() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let addr = sdk.address();
    // Hardhat #0 deterministic address.
    assert_eq!(format!("{addr:#x}"), "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266");
}

#[test]
fn get_order_link_contains_uid() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let link = sdk.get_order_link("0xmyuid");
    assert!(link.contains("0xmyuid"));
}

#[test]
fn get_pre_sign_transaction_returns_calldata() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let uid = "0x".to_owned() + &"ab".repeat(56);
    let tx = sdk.get_pre_sign_transaction(&uid, true).unwrap();
    assert!(!tx.data.is_empty());
    assert_eq!(tx.value, U256::ZERO);
    assert_eq!(tx.gas_limit, GAS_LIMIT_DEFAULT);
}

#[test]
fn get_on_chain_cancellation_returns_calldata() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let uid = "0x".to_owned() + &"cd".repeat(56);
    let tx = sdk.get_on_chain_cancellation(&uid).unwrap();
    assert!(!tx.data.is_empty());
    assert_eq!(tx.value, U256::ZERO);
}

#[test]
fn get_vault_relayer_approve_transaction_returns_calldata() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let token: Address = SELL_TOKEN.parse().unwrap();
    let tx = sdk.get_vault_relayer_approve_transaction(token, U256::MAX);
    assert!(!tx.data.is_empty());
    assert_eq!(tx.to, token);
    assert_eq!(tx.value, U256::ZERO);
}

#[tokio::test]
async fn get_cow_protocol_allowance_without_rpc_returns_error() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let owner: Address = "0x1111111111111111111111111111111111111111".parse().unwrap();
    let token: Address = SELL_TOKEN.parse().unwrap();
    let result = sdk.get_cow_protocol_allowance(owner, token).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_cow_protocol_allowance_with_rpc_attempts_call() {
    // With an unreachable RPC URL we still construct the OnchainReader (covering
    // line 1600) before the on-chain call inevitably fails.
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp")
        .with_rpc_url("http://127.0.0.1:1");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let owner: Address = "0x1111111111111111111111111111111111111111".parse().unwrap();
    let token: Address = SELL_TOKEN.parse().unwrap();
    let result = sdk.get_cow_protocol_allowance(owner, token).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_limit_trade_parameters_from_quote_extracts_amounts() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let quote = sdk.get_quote(default_trade_params()).await.unwrap();
    let limit = sdk.get_limit_trade_parameters_from_quote(&quote);

    assert_eq!(limit.sell_token, SELL_TOKEN.parse::<Address>().unwrap());
    assert_eq!(limit.buy_token, BUY_TOKEN.parse::<Address>().unwrap());
    assert!(!limit.sell_amount.is_zero());
    assert!(!limit.buy_amount.is_zero());
}

#[tokio::test]
async fn get_limit_trade_parameters_from_api() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let limit = sdk.get_limit_trade_parameters(default_trade_params()).await.unwrap();
    assert_eq!(limit.kind, OrderKind::Sell);
    assert!(!limit.sell_amount.is_zero());
}

#[tokio::test]
async fn post_presign_order_returns_order_id() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"22".repeat(56);
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let order = UnsignedOrder::sell(
        SELL_TOKEN.parse().unwrap(),
        BUY_TOKEN.parse().unwrap(),
        U256::from(1_000_000u64),
        U256::from(500_000_000_000_000u64),
    );
    let result = sdk.post_presign_order(&order).await.unwrap();
    assert_eq!(result.order_id, uid);
}

#[tokio::test]
async fn post_eip1271_order_returns_order_id() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"33".repeat(56);
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let order = UnsignedOrder::sell(
        SELL_TOKEN.parse().unwrap(),
        BUY_TOKEN.parse().unwrap(),
        U256::from(1_000_000u64),
        U256::from(500_000_000_000_000u64),
    );
    let sig = [0xABu8; 65];
    let result = sdk.post_eip1271_order(&order, &sig).await.unwrap();
    assert_eq!(result.order_id, uid);
}

#[tokio::test]
async fn get_quote_with_settings_uses_overrides() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let settings =
        SwapAdvancedSettings { app_data: None, slippage_bps: Some(100), partner_fee: None };
    let quote = sdk.get_quote_with_settings(default_trade_params(), &settings).await.unwrap();
    assert_eq!(quote.suggested_slippage_bps, 100);
}

#[tokio::test]
async fn get_eth_flow_transaction_returns_tx_params() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let order_data = cow_ethflow::EthFlowOrderData {
        buy_token: BUY_TOKEN.parse().unwrap(),
        receiver: Address::ZERO,
        sell_amount: U256::from(1_000_000_000_000_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        app_data: B256::ZERO,
        fee_amount: U256::ZERO,
        valid_to: 9_999_999,
        partially_fillable: false,
        quote_id: 0,
    };
    let tx = sdk.get_eth_flow_transaction(&order_data).await.unwrap();
    assert!(!tx.data.is_empty());
}

#[tokio::test]
async fn get_eth_flow_transaction_staging_returns_tx_params() {
    let config = TradingSdkConfig::staging(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let order_data = cow_ethflow::EthFlowOrderData {
        buy_token: BUY_TOKEN.parse().unwrap(),
        receiver: Address::ZERO,
        sell_amount: U256::from(1_000_000_000_000_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        app_data: B256::ZERO,
        fee_amount: U256::ZERO,
        valid_to: 9_999_999,
        partially_fillable: false,
        quote_id: 0,
    };
    let tx = sdk.get_eth_flow_transaction(&order_data).await.unwrap();
    assert!(!tx.data.is_empty());
}

// ── TradingSdkConfig builders ────────────────────────────────────────────────

#[test]
fn config_prod_sets_defaults() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "MyApp");
    assert_eq!(config.slippage_bps, DEFAULT_SLIPPAGE_BPS);
    assert!(matches!(config.env, Env::Prod));
    assert_eq!(config.app_code, "MyApp");
    assert!(config.utm.is_none());
    assert!(config.partner_fee.is_none());
    assert!(config.rpc_url.is_none());
}

#[test]
fn config_staging_sets_barn_env() {
    let config = TradingSdkConfig::staging(SupportedChainId::Sepolia, "MyApp");
    assert!(matches!(config.env, Env::Staging));
}

#[test]
fn config_builder_methods_chain() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "MyApp")
        .with_slippage_bps(100)
        .with_rpc_url("https://rpc.example.com");
    assert_eq!(config.slippage_bps, 100);
    assert_eq!(config.rpc_url.as_deref(), Some("https://rpc.example.com"));
}

#[test]
fn config_with_utm() {
    let utm = get_default_utm_params();
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp").with_utm(utm);
    assert!(config.utm.is_some());
}

#[test]
fn config_with_partner_fee() {
    let entry = PartnerFeeEntry::volume(50, "0x1111111111111111111111111111111111111111");
    let fee = PartnerFee::single(entry);
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp").with_partner_fee(fee);
    assert!(config.partner_fee.is_some());
}

#[test]
fn config_orderbook_client_is_none_by_default() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    assert!(config.orderbook_client.is_none());
}

#[test]
fn config_with_orderbook_client_sets_field() {
    use std::sync::Arc;
    let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    let arc: Arc<dyn OrderbookClient> = Arc::new(api);
    let config =
        TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp").with_orderbook_client(arc);
    assert!(config.orderbook_client.is_some());
    // The Debug impl elides the orderbook client body but advertises its presence.
    let debug = format!("{config:?}");
    assert!(debug.contains("orderbook_client"));
}

#[test]
fn sdk_with_orderbook_replaces_injected_client() {
    use std::sync::Arc;
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    let injected: Arc<dyn OrderbookClient> = Arc::new(api);
    let with_client = sdk.with_orderbook(injected);
    // The Debug impl shows the injected client placeholder.
    let debug = format!("{with_client:?}");
    assert!(debug.contains("orderbook_client"));
}

#[test]
fn config_debug_impl() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let debug = format!("{config:?}");
    assert!(debug.contains("TradingSdkConfig"));
    assert!(debug.contains("Mainnet"));
}

#[test]
fn trading_sdk_debug_impl() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let debug = format!("{sdk:?}");
    assert!(debug.contains("TradingSdk"));
}

#[test]
fn trading_sdk_clone() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let cloned = sdk.clone();
    assert_eq!(cloned.address(), sdk.address());
}

#[test]
fn config_staging_values() {
    let config = TradingSdkConfig::staging(SupportedChainId::Sepolia, "StageApp");
    assert!(matches!(config.env, Env::Staging));
    assert_eq!(config.app_code, "StageApp");
    assert_eq!(config.slippage_bps, DEFAULT_SLIPPAGE_BPS);
}

#[test]
fn config_with_utm_and_partner_fee() {
    let utm = Utm {
        utm_source: Some("test".to_owned()),
        utm_medium: None,
        utm_campaign: None,
        utm_term: None,
        utm_content: None,
    };
    let fee = PartnerFee::single(PartnerFeeEntry::volume(
        10,
        "0x1111111111111111111111111111111111111111",
    ));
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "MyApp")
        .with_utm(utm)
        .with_partner_fee(fee);
    assert!(config.utm.is_some());
    assert!(config.partner_fee.is_some());
}

// ── Error paths ─────────────────────────────────────────────────────────────

#[test]
fn new_with_invalid_key_returns_error() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let result = TradingSdk::new(config, "not-a-valid-key");
    assert!(result.is_err());
}

#[test]
fn new_with_url_invalid_key_returns_error() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let result = TradingSdk::new_with_url(config, "not-a-key", "http://localhost:1234");
    assert!(result.is_err());
}

#[tokio::test]
async fn get_quote_propagates_api_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorType": "InvalidOrderPlacement",
            "description": "sell amount too low"
        })))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let result = sdk.get_quote(default_trade_params()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_quote_422_returns_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(422).set_body_string("unprocessable entity"))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let result = sdk.get_quote(default_trade_params()).await;
    assert!(result.is_err());
}

// ── off-chain cancellations ──────────────────────────────────────────────────

#[tokio::test]
async fn off_chain_cancel_order_sends_delete_request() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"55".repeat(56);
    Mock::given(matchers::method("DELETE"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_string("\"Cancelled\""))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let result = sdk.off_chain_cancel_order(uid, EcdsaSigningScheme::Eip712).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn off_chain_cancel_orders_sends_delete_request() {
    let server = MockServer::start().await;
    let uid1 = "0x".to_owned() + &"56".repeat(56);
    let uid2 = "0x".to_owned() + &"57".repeat(56);
    Mock::given(matchers::method("DELETE"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_string("\"Cancelled\""))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let result = sdk.off_chain_cancel_orders(vec![uid1, uid2], EcdsaSigningScheme::Eip712).await;
    assert!(result.is_ok());
}

// ── Standalone functions ────────────────────────────────────────────────────

#[test]
fn get_is_eth_flow_order_detects_native_currency() {
    assert!(get_is_eth_flow_order(NATIVE_CURRENCY_ADDRESS));
    assert!(!get_is_eth_flow_order(Address::ZERO));
}

#[test]
fn get_default_utm_params_has_source() {
    let utm = get_default_utm_params();
    assert_eq!(utm.utm_source.as_deref(), Some("web"));
}

#[test]
fn swap_params_to_limit_order_params_extracts_amounts() {
    let params = default_trade_params();
    let quote = make_quote_response_struct("1000000", "500000000000000", "1000", OrderKind::Sell);
    let limit = swap_params_to_limit_order_params(&params, &quote);
    assert_eq!(limit.sell_amount, U256::from(1_000_000u64));
    assert_eq!(limit.buy_amount, U256::from(500_000_000_000_000u64));
}

#[test]
fn swap_params_to_limit_order_params_invalid_amounts_default_to_zero() {
    let params = default_trade_params();
    let quote = make_quote_response_struct("not-a-number", "also-bad", "0", OrderKind::Sell);
    let limit = swap_params_to_limit_order_params(&params, &quote);
    assert_eq!(limit.sell_amount, U256::ZERO);
    assert_eq!(limit.buy_amount, U256::ZERO);
}

#[test]
fn calculate_gas_margin_adds_twenty_percent() {
    assert_eq!(calculate_gas_margin(100_000), 120_000);
    assert_eq!(calculate_gas_margin(0), 0);
}

#[test]
fn build_app_data_returns_valid_info() {
    let info = build_app_data("MyDApp", 50, OrderClassKind::Market, None);
    assert!(!info.full_app_data.is_empty());
    assert!(info.app_data_keccak256.starts_with("0x"));
}

#[test]
fn build_app_data_with_partner_fee() {
    let fee = PartnerFee::single(PartnerFeeEntry::volume(
        100,
        "0x1111111111111111111111111111111111111111",
    ));
    let info = build_app_data("MyDApp", 50, OrderClassKind::Limit, Some(&fee));
    assert!(!info.full_app_data.is_empty());
    assert!(info.app_data_keccak256.starts_with("0x"));
}

#[test]
fn generate_app_data_from_doc_returns_hash() {
    let doc = serde_json::json!({"version": "1.1.0", "metadata": {}});
    let info = generate_app_data_from_doc(&doc).unwrap();
    assert!(info.app_data_keccak256.starts_with("0x"));
    assert!(info.full_app_data.contains("version"));
}

#[test]
fn generate_app_data_from_doc_sorts_keys_recursively() {
    // Drives the recursive Object/Array branches in `sort_keys_value`.
    let doc = serde_json::json!({
        "z": [{"b": 1, "a": 2}, "string", 3],
        "a": null,
        "m": true
    });
    let info = generate_app_data_from_doc(&doc).unwrap();
    let pos_a = info.full_app_data.find("\"a\"").unwrap();
    let pos_z = info.full_app_data.find("\"z\"").unwrap();
    assert!(pos_a < pos_z);
}

#[test]
fn get_default_slippage_bps_returns_correct_values() {
    let normal = get_default_slippage_bps(SupportedChainId::Mainnet, false);
    let eth_flow = get_default_slippage_bps(SupportedChainId::Mainnet, true);
    assert_eq!(normal, DEFAULT_SLIPPAGE_BPS);
    assert_eq!(eth_flow, ETH_FLOW_DEFAULT_SLIPPAGE_BPS);
}

#[test]
fn get_slippage_percent_sell_order() {
    let result = get_slippage_percent(
        true,
        U256::from(1_000_000u64),
        U256::from(999_000u64),
        U256::from(5_000u64),
    )
    .unwrap();
    assert!(result > 0.0);
    assert!(result < 1.0);
}

#[test]
fn get_slippage_percent_buy_order() {
    let result = get_slippage_percent(
        false,
        U256::from(1_000_000u64),
        U256::from(999_000u64),
        U256::from(5_000u64),
    )
    .unwrap();
    assert!(result > 0.0);
    assert!(result < 1.0);
}

#[test]
fn get_slippage_percent_zero_amount_errors() {
    let result = get_slippage_percent(true, U256::ZERO, U256::ZERO, U256::from(1u64));
    assert!(result.is_err());
}

#[test]
fn resolve_signer_valid_key() {
    let signer = resolve_signer(Some(TEST_KEY)).unwrap();
    let addr = alloy_signer::Signer::address(&signer);
    assert_ne!(addr, Address::ZERO);
}

#[test]
fn resolve_signer_none_errors() {
    assert!(resolve_signer(None).is_err());
}

#[test]
fn resolve_signer_invalid_key_errors() {
    assert!(resolve_signer(Some("not-a-key")).is_err());
}

#[test]
fn get_eth_flow_contract_non_zero() {
    let addr = get_eth_flow_contract(SupportedChainId::Mainnet, Env::Prod);
    assert_ne!(addr, Address::ZERO);
}

#[test]
fn get_eth_flow_contract_staging_non_zero() {
    let addr = get_eth_flow_contract(SupportedChainId::Mainnet, Env::Staging);
    assert_ne!(addr, Address::ZERO);
}

#[test]
fn get_settlement_contract_non_zero() {
    let addr = get_settlement_contract(SupportedChainId::Mainnet, Env::Prod);
    assert_ne!(addr, Address::ZERO);
}

#[test]
fn get_settlement_contract_staging_non_zero() {
    let addr = get_settlement_contract(SupportedChainId::Mainnet, Env::Staging);
    assert_ne!(addr, Address::ZERO);
}

#[test]
fn calculate_unique_order_id_returns_valid_hex() {
    let order =
        UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::from(1u64), U256::from(1u64));
    let uid = calculate_unique_order_id(SupportedChainId::Mainnet, &order, Env::Prod);
    assert!(uid.starts_with("0x"));
    assert_eq!(uid.len(), 2 + 112);
}

#[test]
fn resolve_order_book_api_creates_new_when_none() {
    let api = resolve_order_book_api(SupportedChainId::Mainnet, Env::Prod, None);
    let _link = api.get_order_link("0xtest");
}

#[test]
fn resolve_order_book_api_returns_existing_when_provided() {
    let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    let link_before = api.get_order_link("0xtest");
    let returned = resolve_order_book_api(SupportedChainId::Sepolia, Env::Staging, Some(api));
    let link_after = returned.get_order_link("0xtest");
    assert_eq!(link_before, link_after);
}

#[test]
fn unsigned_order_for_signing_is_identity() {
    let order =
        UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::from(1u64), U256::from(1u64));
    let same = unsigned_order_for_signing(order.clone());
    assert_eq!(same.sell_amount, order.sell_amount);
    assert_eq!(same.buy_amount, order.buy_amount);
}

#[test]
fn get_eth_flow_cancellation_returns_calldata() {
    let uid = "0x".to_owned() + &"ab".repeat(56);
    let tx = get_eth_flow_cancellation(SupportedChainId::Mainnet, Env::Prod, &uid).unwrap();
    assert!(!tx.data.is_empty());
    assert_eq!(tx.value, U256::ZERO);
    assert_eq!(tx.gas_limit, GAS_LIMIT_DEFAULT);
    assert_ne!(tx.to, Address::ZERO);
}

#[test]
fn get_settlement_cancellation_returns_calldata() {
    let uid = "0x".to_owned() + &"cd".repeat(56);
    let tx = get_settlement_cancellation(SupportedChainId::Mainnet, Env::Prod, &uid).unwrap();
    assert!(!tx.data.is_empty());
    assert_eq!(tx.value, U256::ZERO);
    assert_ne!(tx.to, Address::ZERO);
}

#[test]
fn get_order_to_sign_returns_valid_order() {
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: None,
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let from: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap();
    let order = get_order_to_sign(
        SupportedChainId::Mainnet,
        from,
        false,
        U256::ZERO,
        false,
        &params,
        "0x0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_eq!(order.sell_token, SELL_TOKEN.parse::<Address>().unwrap());
    assert_eq!(order.valid_to, 9_999_999);
    assert_eq!(order.receiver, from);
    assert_eq!(order.fee_amount, U256::ZERO);
}

#[test]
fn get_order_to_sign_with_slippage_adjustment() {
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: None,
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let from: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap();
    let order = get_order_to_sign(
        SupportedChainId::Mainnet,
        from,
        false,
        U256::from(1000u64),
        true,
        &params,
        "0x0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert!(order.buy_amount < U256::from(500_000_000_000_000u64));
}

#[test]
fn get_order_to_sign_with_valid_for_only() {
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: None,
        valid_for: Some(3600),
        valid_to: None,
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let from: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap();
    let order = get_order_to_sign(
        SupportedChainId::Mainnet,
        from,
        false,
        U256::ZERO,
        false,
        &params,
        "0x0000000000000000000000000000000000000000000000000000000000000000",
    );
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        as u32;
    assert!(order.valid_to > now);
}

#[test]
fn get_order_to_sign_buy_order_with_slippage() {
    let params = LimitTradeParameters {
        kind: OrderKind::Buy,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: Some("0x1111111111111111111111111111111111111111".parse().unwrap()),
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let from: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap();
    let order = get_order_to_sign(
        SupportedChainId::Mainnet,
        from,
        false,
        U256::from(1000u64),
        true,
        &params,
        "0x0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert!(order.sell_amount > U256::from(1_000_000u64));
    assert_eq!(
        order.receiver,
        "0x1111111111111111111111111111111111111111".parse::<Address>().unwrap()
    );
}

#[test]
fn get_order_to_sign_eth_flow() {
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: NATIVE_CURRENCY_ADDRESS,
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: None,
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let from: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap();
    let order = get_order_to_sign(
        SupportedChainId::Mainnet,
        from,
        true,
        U256::ZERO,
        true,
        &params,
        "0x0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert!(order.buy_amount < U256::from(500_000_000_000_000u64));
}

#[test]
fn get_order_typed_data_returns_typed_data() {
    let order = UnsignedOrder::sell(
        SELL_TOKEN.parse().unwrap(),
        BUY_TOKEN.parse().unwrap(),
        U256::from(1_000_000u64),
        U256::from(500_000_000_000_000u64),
    );
    let typed = get_order_typed_data(SupportedChainId::Mainnet, order);
    assert_eq!(typed.domain.chain_id, SupportedChainId::Mainnet.as_u64());
}

#[test]
fn adjust_eth_flow_order_params_replaces_sell_token() {
    let params = TradeParameters {
        kind: OrderKind::Sell,
        sell_token: NATIVE_CURRENCY_ADDRESS,
        sell_token_decimals: 18,
        buy_token: BUY_TOKEN.parse().unwrap(),
        buy_token_decimals: 18,
        amount: U256::from(1u64),
        slippage_bps: Some(50),
        receiver: None,
        valid_for: None,
        valid_to: None,
        partially_fillable: None,
        partner_fee: None,
    };
    let adjusted = adjust_eth_flow_order_params(SupportedChainId::Mainnet, params);
    assert_ne!(adjusted.sell_token, NATIVE_CURRENCY_ADDRESS);
}

#[test]
fn adjust_eth_flow_limit_order_params_replaces_sell_token() {
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: NATIVE_CURRENCY_ADDRESS,
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1u64),
        buy_amount: U256::from(1u64),
        receiver: None,
        valid_for: None,
        valid_to: None,
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let adjusted = adjust_eth_flow_limit_order_params(SupportedChainId::Mainnet, params);
    assert_ne!(adjusted.sell_token, NATIVE_CURRENCY_ADDRESS);
}

#[test]
fn get_trade_parameters_after_quote_restores_sell_token() {
    let params = TradeParameters {
        kind: OrderKind::Sell,
        sell_token: Address::ZERO,
        sell_token_decimals: 18,
        buy_token: BUY_TOKEN.parse().unwrap(),
        buy_token_decimals: 18,
        amount: U256::from(1u64),
        slippage_bps: Some(50),
        receiver: None,
        valid_for: None,
        valid_to: None,
        partially_fillable: None,
        partner_fee: None,
    };
    let restored = get_trade_parameters_after_quote(params, NATIVE_CURRENCY_ADDRESS);
    assert_eq!(restored.sell_token, NATIVE_CURRENCY_ADDRESS);
}

#[test]
fn get_order_deadline_from_now_is_in_future() {
    let deadline = get_order_deadline_from_now(1800);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        as u32;
    assert!(deadline > now);
}

#[test]
fn eth_flow_slippage_equals_default() {
    assert_eq!(ETH_FLOW_DEFAULT_SLIPPAGE_BPS, DEFAULT_SLIPPAGE_BPS);
}

// ── Standalone post functions ───────────────────────────────────────────────

#[tokio::test]
async fn post_cow_protocol_trade_submits_order() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"66".repeat(56);

    Mock::given(matchers::method("PUT"))
        .and(matchers::path_regex(r"/api/v1/app_data/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fullAppData": "{}"
        })))
        .mount(&server)
        .await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let signer = resolve_signer(Some(TEST_KEY)).unwrap();
    let app_data = build_app_data("TestApp", 50, OrderClassKind::Market, None);
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: None,
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let additional = PostTradeAdditionalParams {
        signing_scheme: None,
        network_costs_amount: None,
        apply_costs_slippage_and_fees: None,
    };
    let result = post_cow_protocol_trade(
        &api,
        &signer,
        &app_data,
        &params,
        SupportedChainId::Mainnet,
        &additional,
    )
    .await
    .unwrap();
    assert_eq!(result.order_id, uid);
}

#[tokio::test]
async fn post_cow_protocol_trade_with_eth_sign() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"67".repeat(56);

    Mock::given(matchers::method("PUT"))
        .and(matchers::path_regex(r"/api/v1/app_data/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fullAppData": "{}"
        })))
        .mount(&server)
        .await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let signer = resolve_signer(Some(TEST_KEY)).unwrap();
    let app_data = build_app_data("TestApp", 50, OrderClassKind::Market, None);
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: None,
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let additional = PostTradeAdditionalParams {
        signing_scheme: Some(SigningScheme::EthSign),
        network_costs_amount: Some("5000".to_owned()),
        apply_costs_slippage_and_fees: Some(false),
    };
    let result = post_cow_protocol_trade(
        &api,
        &signer,
        &app_data,
        &params,
        SupportedChainId::Mainnet,
        &additional,
    )
    .await
    .unwrap();
    assert_eq!(result.order_id, uid);
}

#[tokio::test]
async fn post_cow_protocol_trade_with_eip1271_scheme_falls_back_to_eip712() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"71".repeat(56);

    Mock::given(matchers::method("PUT"))
        .and(matchers::path_regex(r"/api/v1/app_data/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fullAppData": "{}"
        })))
        .mount(&server)
        .await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let signer = resolve_signer(Some(TEST_KEY)).unwrap();
    let app_data = build_app_data("TestApp", 50, OrderClassKind::Market, None);
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: Some("0x1111111111111111111111111111111111111111".parse().unwrap()),
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let additional = PostTradeAdditionalParams {
        signing_scheme: Some(SigningScheme::Eip1271),
        network_costs_amount: None,
        apply_costs_slippage_and_fees: None,
    };
    let result = post_cow_protocol_trade(
        &api,
        &signer,
        &app_data,
        &params,
        SupportedChainId::Mainnet,
        &additional,
    )
    .await
    .unwrap();
    assert_eq!(result.order_id, uid);
}

#[tokio::test]
async fn post_cow_protocol_trade_with_presign_scheme_falls_back_to_eip712() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"72".repeat(56);

    Mock::given(matchers::method("PUT"))
        .and(matchers::path_regex(r"/api/v1/app_data/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fullAppData": "{}"
        })))
        .mount(&server)
        .await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let signer = resolve_signer(Some(TEST_KEY)).unwrap();
    let app_data = build_app_data("TestApp", 50, OrderClassKind::Market, None);
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: None,
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let additional = PostTradeAdditionalParams {
        signing_scheme: Some(SigningScheme::PreSign),
        network_costs_amount: None,
        apply_costs_slippage_and_fees: None,
    };
    let result = post_cow_protocol_trade(
        &api,
        &signer,
        &app_data,
        &params,
        SupportedChainId::Mainnet,
        &additional,
    )
    .await
    .unwrap();
    assert_eq!(result.order_id, uid);
}

#[tokio::test]
async fn post_cow_protocol_trade_rejects_eth_flow() {
    let server = MockServer::start().await;
    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let signer = resolve_signer(Some(TEST_KEY)).unwrap();
    let app_data = build_app_data("TestApp", 50, OrderClassKind::Market, None);
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: NATIVE_CURRENCY_ADDRESS,
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: None,
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let additional = PostTradeAdditionalParams {
        signing_scheme: None,
        network_costs_amount: None,
        apply_costs_slippage_and_fees: None,
    };
    let result = post_cow_protocol_trade(
        &api,
        &signer,
        &app_data,
        &params,
        SupportedChainId::Mainnet,
        &additional,
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn post_sell_native_currency_order_builds_eth_flow_tx() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("PUT"))
        .and(matchers::path_regex(r"/api/v1/app_data/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fullAppData": "{}"
        })))
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let app_data = build_app_data("TestApp", 50, OrderClassKind::Market, None);
    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: NATIVE_CURRENCY_ADDRESS,
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1_000_000_000_000_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        receiver: None,
        valid_for: None,
        valid_to: Some(9_999_999),
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let (result, tx) = post_sell_native_currency_order(
        &api,
        &app_data,
        &params,
        SupportedChainId::Mainnet,
        Env::Prod,
    )
    .await
    .unwrap();
    assert!(result.order_id.starts_with("0x"));
    assert!(!tx.data.is_empty());
    assert!(tx.value > U256::ZERO);
}

#[tokio::test]
async fn get_quote_raw_returns_response() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let req = OrderQuoteRequest::new(
        SELL_TOKEN.parse().unwrap(),
        BUY_TOKEN.parse().unwrap(),
        Address::ZERO,
        QuoteSide::sell("1000000"),
    );
    let resp = get_quote_raw(&api, &req).await.unwrap();
    assert!(!resp.quote.sell_amount.is_empty());
}

#[tokio::test]
async fn get_quote_with_signer_returns_result_and_signer() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let result =
        get_quote_with_signer(&config, &api, TEST_KEY, default_trade_params(), None).await.unwrap();
    assert!(!result.result.order_to_sign.sell_amount.is_zero());
    assert_ne!(alloy_signer::Signer::address(&result.signer), Address::ZERO);
}

#[tokio::test]
async fn get_quote_with_signer_invalid_key_errors() {
    let server = MockServer::start().await;
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let res = get_quote_with_signer(&config, &api, "not-hex", default_trade_params(), None).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn get_quote_with_signer_with_settings() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let settings =
        SwapAdvancedSettings { app_data: None, slippage_bps: Some(200), partner_fee: None };
    let result =
        get_quote_with_signer(&config, &api, TEST_KEY, default_trade_params(), Some(&settings))
            .await
            .unwrap();
    assert_eq!(result.result.suggested_slippage_bps, 200);
}

#[test]
fn get_trader_with_explicit_owner() {
    let signer = resolve_signer(Some(TEST_KEY)).unwrap();
    let explicit: Address = "0x1111111111111111111111111111111111111111".parse().unwrap();
    let trader = get_trader(SupportedChainId::Mainnet, "MyApp", Some(explicit), &signer);
    assert_eq!(trader.account, explicit);
    assert_eq!(trader.app_code, "MyApp");
}

#[test]
fn get_trader_without_owner_uses_signer() {
    let signer = resolve_signer(Some(TEST_KEY)).unwrap();
    let trader = get_trader(SupportedChainId::Mainnet, "MyApp", None, &signer);
    assert_eq!(trader.account, alloy_signer::Signer::address(&signer));
}

#[test]
fn resolve_slippage_suggestion_returns_none_when_sufficient() {
    let quote = make_quote_response_struct("1000000", "500000000000000", "1000", OrderKind::Sell);
    // With a very high slippage, no suggestion should be needed.
    let suggestion = resolve_slippage_suggestion(SupportedChainId::Mainnet, false, &quote, 10_000);
    assert!(suggestion.is_none());
}

#[test]
fn resolve_slippage_suggestion_with_low_slippage() {
    let quote = make_quote_response_struct("1000000", "500000000000000", "100000", OrderKind::Sell);
    // Just verify that it doesn't panic; the value depends on the heuristic.
    let _suggestion = resolve_slippage_suggestion(SupportedChainId::Mainnet, false, &quote, 0);
}

#[test]
fn resolve_slippage_suggestion_eth_flow() {
    let quote = make_quote_response_struct("1000000", "500000000000000", "1000", OrderKind::Sell);
    let _suggestion = resolve_slippage_suggestion(SupportedChainId::Mainnet, true, &quote, 50);
}

// ── get_quote with various parameter combinations ──────────────────────────

#[tokio::test]
async fn get_quote_with_buy_order_kind() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let mut params = default_trade_params();
    params.kind = OrderKind::Buy;
    let quote = sdk.get_quote(params).await.unwrap();
    assert!(!quote.order_to_sign.sell_amount.is_zero());
}

#[tokio::test]
async fn get_quote_with_settings_partner_fee_override() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let fee = PartnerFee::single(PartnerFeeEntry::volume(
        100,
        "0x1111111111111111111111111111111111111111",
    ));
    let sdk = make_sdk(&server);
    let settings =
        SwapAdvancedSettings { app_data: None, slippage_bps: None, partner_fee: Some(fee) };
    let quote = sdk.get_quote_with_settings(default_trade_params(), &settings).await.unwrap();
    assert!(!quote.order_to_sign.sell_amount.is_zero());
}

#[tokio::test]
async fn get_quote_with_config_partner_fee() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let fee = PartnerFee::single(PartnerFeeEntry::volume(
        50,
        "0x2222222222222222222222222222222222222222",
    ));
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp").with_partner_fee(fee);
    let sdk = TradingSdk::new_with_url(config, TEST_KEY, server.uri()).unwrap();
    let quote = sdk.get_quote(default_trade_params()).await.unwrap();
    assert!(!quote.order_to_sign.sell_amount.is_zero());
}

#[tokio::test]
async fn get_quote_with_config_utm() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let utm = Utm {
        utm_source: Some("test".to_owned()),
        utm_medium: None,
        utm_campaign: None,
        utm_term: None,
        utm_content: None,
    };
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp").with_utm(utm);
    let sdk = TradingSdk::new_with_url(config, TEST_KEY, server.uri()).unwrap();
    let quote = sdk.get_quote(default_trade_params()).await.unwrap();
    assert!(!quote.order_to_sign.sell_amount.is_zero());
}

#[tokio::test]
async fn get_quote_partially_fillable() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let mut params = default_trade_params();
    params.partially_fillable = Some(true);
    let quote = sdk.get_quote(params).await.unwrap();
    assert!(!quote.order_to_sign.sell_amount.is_zero());
}

#[tokio::test]
async fn get_quote_with_params_partner_fee() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let mut params = default_trade_params();
    params.partner_fee = Some(PartnerFee::single(PartnerFeeEntry::volume(
        100,
        "0x1111111111111111111111111111111111111111",
    )));
    let quote = sdk.get_quote(params).await.unwrap();
    assert!(!quote.order_to_sign.sell_amount.is_zero());
}

#[tokio::test]
async fn get_quote_with_valid_for() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let mut params = default_trade_params();
    params.valid_to = None;
    params.valid_for = Some(3600);
    let quote = sdk.get_quote(params).await.unwrap();
    assert!(!quote.order_to_sign.sell_amount.is_zero());
}

// ── apply_settings_to_limit_trade_parameters: cover line 881 partner-fee branch

#[test]
fn apply_settings_to_limit_trade_parameters_covers_all_fields() {
    let base = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1u64),
        buy_amount: U256::from(1u64),
        receiver: None,
        valid_for: None,
        valid_to: None,
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    // Drives the `partner_fee.is_some()` branch (line 881) in addition to the
    // other field overrides — none of these are otherwise reached without
    // settings populating each field.
    let fee = PartnerFee::single(PartnerFeeEntry::volume(
        25,
        "0x1111111111111111111111111111111111111111",
    ));
    let settings = LimitOrderAdvancedSettings {
        receiver: Some(Address::ZERO),
        valid_to: Some(123),
        partner_fee: Some(fee),
        partially_fillable: Some(true),
        app_data: Some("0x".to_owned()),
    };
    let updated = apply_settings_to_limit_trade_parameters(base.clone(), Some(&settings));
    assert_eq!(updated.receiver, Some(Address::ZERO));
    assert_eq!(updated.valid_to, Some(123));
    assert!(updated.partially_fillable);
    assert!(updated.partner_fee.is_some());
    assert_eq!(updated.app_data.as_deref(), Some("0x"));
}

#[test]
fn apply_settings_to_limit_trade_parameters_returns_unchanged_when_none() {
    let base = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: SELL_TOKEN.parse().unwrap(),
        buy_token: BUY_TOKEN.parse().unwrap(),
        sell_amount: U256::from(1u64),
        buy_amount: U256::from(1u64),
        receiver: None,
        valid_for: None,
        valid_to: None,
        partially_fillable: false,
        app_data: None,
        partner_fee: None,
    };
    let updated = apply_settings_to_limit_trade_parameters(base.clone(), None);
    assert_eq!(updated.receiver, base.receiver);
    assert_eq!(updated.partially_fillable, base.partially_fillable);
}

// ── QuoteResults Display + accessors via a hand-built instance ─────────────

#[tokio::test]
async fn quote_results_display_and_accessors_via_real_quote() {
    // We deliberately go through `get_quote` rather than constructing a
    // QuoteResults by hand: that's both more representative and exercises the
    // surrounding helpers (`build_unsigned_order`, `parse_app_data_hex`, etc.)
    // along with the Display/order_ref/quote_ref methods on QuoteResults.
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;
    let sdk = make_sdk(&server);
    let quote: QuoteResults = sdk.get_quote(default_trade_params()).await.unwrap();
    let s = format!("{quote}");
    assert!(s.starts_with("quote slippage="));
    let order_ptr: *const _ = quote.order_ref();
    let owned_ptr: *const _ = &raw const quote.order_to_sign;
    assert_eq!(order_ptr, owned_ptr);
    let quote_ptr: *const _ = quote.quote_ref();
    let owned_quote_ptr: *const _ = &raw const quote.quote_response;
    assert_eq!(quote_ptr, owned_quote_ptr);
}

// ── TradingAppDataInfo helpers ─────────────────────────────────────────────

#[test]
fn trading_app_data_info_methods() {
    let info = TradingAppDataInfo::new("{\"test\": true}", "0xdeadbeef");
    assert!(info.has_full_app_data());
    assert_eq!(info.full_app_data_ref(), "{\"test\": true}");
    assert_eq!(info.keccak256_ref(), "0xdeadbeef");
    assert_eq!(format!("{info}"), "app-data(0xdeadbeef)");
}

// ── PostTradeAdditionalParams helpers ──────────────────────────────────────

#[test]
fn post_trade_additional_params_builder() {
    let params = PostTradeAdditionalParams {
        signing_scheme: None,
        network_costs_amount: None,
        apply_costs_slippage_and_fees: None,
    }
    .with_signing_scheme(SigningScheme::EthSign)
    .with_network_costs_amount("5000")
    .with_apply_costs_slippage_and_fees(true);

    assert!(params.has_signing_scheme());
    assert!(params.has_network_costs());
    assert!(params.should_apply_costs());
    assert_eq!(format!("{params}"), "post-trade-params");
}
