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
//! Wiremock-based integration tests for [`TradingSdk`] high-level trading methods.

use alloy_primitives::{Address, U256};
use cow_rs::{
    Env, OrderKind, SupportedChainId, TokenBalance, TradeParameters, TradingSdk, TradingSdkConfig,
};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

/// Well-known test private key (Hardhat #0).
const TEST_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

/// Deterministic sell / buy token addresses used throughout the tests.
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

// ── TradingSdk::get_quote ────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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
    assert!(quote.app_data_info.has_full_app_data());
    assert!(quote.app_data_info.keccak256_ref().starts_with("0x"));
}

// ── TradingSdk::post_swap_order_from_quote ───────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn post_swap_order_from_quote_returns_order_id() {
    let server = MockServer::start().await;

    // Mock the quote endpoint.
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    // Mock the order submission endpoint.
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

// ── TradingSdk::post_swap_order (quote + submit in one call) ─────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_order ────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_native_price ─────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_auction ──────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_trades ───────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_orders_for_account ───────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_version ──────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_total_surplus ────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_app_data ─────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::upload_app_data ──────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::upload_app_data_auto ─────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_order_status ─────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_orders_by_tx ─────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_solver_competition_latest ────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::post_limit_order ─────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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
    let params = cow_rs::LimitTradeParameters {
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

// ── TradingSdk::address ──────────────────────────────────────────────────────

#[test]
fn address_returns_signer_address() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let addr = sdk.address();
    // Hardhat #0 address.
    assert_eq!(
        format!("{addr:#x}"),
        "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
    );
}

// ── TradingSdk::get_order_link ───────────────────────────────────────────────

#[test]
fn get_order_link_contains_uid() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let link = sdk.get_order_link("0xmyuid");
    assert!(link.contains("0xmyuid"));
}

// ── TradingSdk::get_pre_sign_transaction ─────────────────────────────────────

#[test]
fn get_pre_sign_transaction_returns_calldata() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let uid = "0x".to_owned() + &"ab".repeat(56);
    let tx = sdk.get_pre_sign_transaction(&uid, true).unwrap();
    assert!(!tx.data.is_empty());
    assert_eq!(tx.value, U256::ZERO);
    assert_eq!(tx.gas_limit, cow_rs::GAS_LIMIT_DEFAULT);
}

// ── TradingSdk::get_on_chain_cancellation ────────────────────────────────────

#[test]
fn get_on_chain_cancellation_returns_calldata() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let uid = "0x".to_owned() + &"cd".repeat(56);
    let tx = sdk.get_on_chain_cancellation(&uid).unwrap();
    assert!(!tx.data.is_empty());
    assert_eq!(tx.value, U256::ZERO);
}

// ── TradingSdk::get_vault_relayer_approve_transaction ────────────────────────

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

// ── TradingSdk::get_cow_protocol_allowance (no RPC) ──────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_cow_protocol_allowance_without_rpc_returns_error() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let owner: Address = "0x1111111111111111111111111111111111111111".parse().unwrap();
    let token: Address = SELL_TOKEN.parse().unwrap();
    let result = sdk.get_cow_protocol_allowance(owner, token).await;
    assert!(result.is_err());
}

// ── TradingSdk::get_limit_trade_parameters_from_quote ────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::post_presign_order ───────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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
    let order = cow_rs::UnsignedOrder::sell(
        SELL_TOKEN.parse().unwrap(),
        BUY_TOKEN.parse().unwrap(),
        U256::from(1_000_000u64),
        U256::from(500_000_000_000_000u64),
    );
    let result = sdk.post_presign_order(&order).await.unwrap();
    assert_eq!(result.order_id, uid);
}

// ── TradingSdk::post_eip1271_order ───────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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
    let order = cow_rs::UnsignedOrder::sell(
        SELL_TOKEN.parse().unwrap(),
        BUY_TOKEN.parse().unwrap(),
        U256::from(1_000_000u64),
        U256::from(500_000_000_000_000u64),
    );
    let sig = [0xABu8; 65];
    let result = sdk.post_eip1271_order(&order, &sig).await.unwrap();
    assert_eq!(result.order_id, uid);
}

// ── TradingSdk::get_quote_with_settings ──────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_with_settings_uses_overrides() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let settings = cow_rs::SwapAdvancedSettings {
        app_data: None,
        slippage_bps: Some(100),
        partner_fee: None,
    };
    let quote = sdk
        .get_quote_with_settings(default_trade_params(), &settings)
        .await
        .unwrap();
    // The suggested slippage should reflect the override.
    assert_eq!(quote.suggested_slippage_bps, 100);
}

// ── TradingSdk::post_swap_order_with_settings ────────────────────────────────

#[cfg_attr(miri, ignore)]
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
    let settings = cow_rs::SwapAdvancedSettings {
        app_data: None,
        slippage_bps: Some(100),
        partner_fee: None,
    };
    let result = sdk
        .post_swap_order_with_settings(default_trade_params(), &settings)
        .await
        .unwrap();
    assert_eq!(result.order_id, uid);
}

// ── TradingSdk::get_eth_flow_transaction ─────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_eth_flow_transaction_returns_tx_params() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let order_data = cow_rs::ethflow::EthFlowOrderData {
        buy_token: BUY_TOKEN.parse().unwrap(),
        receiver: Address::ZERO,
        sell_amount: U256::from(1_000_000_000_000_000_000u64),
        buy_amount: U256::from(500_000_000_000_000u64),
        app_data: alloy_primitives::B256::ZERO,
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
    assert_eq!(config.slippage_bps, cow_rs::DEFAULT_SLIPPAGE_BPS);
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

// ── Error path: invalid private key ──────────────────────────────────────────

#[test]
fn new_with_invalid_key_returns_error() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let result = TradingSdk::new(config, "not-a-valid-key");
    assert!(result.is_err());
}

// ── Error path: quote API returns 400 ────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_propagates_api_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(
            ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "errorType": "InvalidOrderPlacement",
                "description": "sell amount too low"
            })),
        )
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let result = sdk.get_quote(default_trade_params()).await;
    assert!(result.is_err());
}

// ── Standalone functions ─────────────────────────────────────────────────────

#[test]
fn get_is_eth_flow_order_detects_native_currency() {
    assert!(cow_rs::trading::get_is_eth_flow_order(cow_rs::NATIVE_CURRENCY_ADDRESS));
    assert!(!cow_rs::trading::get_is_eth_flow_order(Address::ZERO));
}

#[test]
fn get_default_utm_params_has_source() {
    let utm = cow_rs::trading::get_default_utm_params();
    assert_eq!(utm.utm_source.as_deref(), Some("web"));
}

#[test]
fn swap_params_to_limit_order_params_extracts_amounts() {
    use cow_rs::order_book::{OrderQuoteResponse, QuoteData};
    let params = default_trade_params();
    let quote = OrderQuoteResponse {
        quote: QuoteData {
            sell_token: SELL_TOKEN.parse().unwrap(),
            buy_token: BUY_TOKEN.parse().unwrap(),
            receiver: None,
            sell_amount: "1000000".to_owned(),
            buy_amount: "500000000000000".to_owned(),
            valid_to: 9_999_999,
            app_data: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            fee_amount: "1000".to_owned(),
            kind: OrderKind::Sell,
            partially_fillable: false,
            sell_token_balance: TokenBalance::Erc20,
            buy_token_balance: TokenBalance::Erc20,
        },
        from: Address::ZERO,
        expiration: "2099-01-01T00:00:00.000Z".to_owned(),
        id: Some(42),
        verified: false,
        protocol_fee_bps: None,
    };
    let limit = cow_rs::trading::swap_params_to_limit_order_params(&params, &quote);
    assert_eq!(limit.sell_amount, U256::from(1_000_000u64));
    assert_eq!(limit.buy_amount, U256::from(500_000_000_000_000u64));
}

#[test]
fn calculate_gas_margin_adds_twenty_percent() {
    assert_eq!(cow_rs::calculate_gas_margin(100_000), 120_000);
    assert_eq!(cow_rs::calculate_gas_margin(0), 0);
}

#[test]
fn build_app_data_returns_valid_info() {
    let info = cow_rs::trading::build_app_data("MyDApp", 50, cow_rs::OrderClassKind::Market, None);
    assert!(!info.full_app_data.is_empty());
    assert!(info.app_data_keccak256.starts_with("0x"));
}

#[test]
fn generate_app_data_from_doc_returns_hash() {
    let doc = serde_json::json!({"version": "1.1.0", "metadata": {}});
    let info = cow_rs::trading::generate_app_data_from_doc(&doc).unwrap();
    assert!(info.app_data_keccak256.starts_with("0x"));
    assert!(info.full_app_data.contains("version"));
}

#[test]
fn get_default_slippage_bps_returns_correct_values() {
    let normal = cow_rs::trading::get_default_slippage_bps(SupportedChainId::Mainnet, false);
    let eth_flow = cow_rs::trading::get_default_slippage_bps(SupportedChainId::Mainnet, true);
    assert_eq!(normal, cow_rs::DEFAULT_SLIPPAGE_BPS);
    assert_eq!(eth_flow, cow_rs::ETH_FLOW_DEFAULT_SLIPPAGE_BPS);
}

#[test]
fn get_slippage_percent_sell_order() {
    let result = cow_rs::trading::get_slippage_percent(
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
fn get_slippage_percent_zero_amount_errors() {
    let result = cow_rs::trading::get_slippage_percent(
        true,
        U256::ZERO,
        U256::ZERO,
        U256::from(1u64),
    );
    assert!(result.is_err());
}

#[test]
fn resolve_signer_valid_key() {
    let signer = cow_rs::trading::resolve_signer(Some(TEST_KEY)).unwrap();
    let addr = alloy_signer::Signer::address(&signer);
    assert_ne!(addr, Address::ZERO);
}

#[test]
fn resolve_signer_none_errors() {
    assert!(cow_rs::trading::resolve_signer(None).is_err());
}

#[test]
fn get_eth_flow_contract_non_zero() {
    let addr = cow_rs::trading::get_eth_flow_contract(SupportedChainId::Mainnet, Env::Prod);
    assert_ne!(addr, Address::ZERO);
}

#[test]
fn get_settlement_contract_non_zero() {
    let addr = cow_rs::trading::get_settlement_contract(SupportedChainId::Mainnet, Env::Prod);
    assert_ne!(addr, Address::ZERO);
}

#[test]
fn calculate_unique_order_id_returns_valid_hex() {
    let order = cow_rs::UnsignedOrder::sell(
        Address::ZERO,
        Address::ZERO,
        U256::from(1u64),
        U256::from(1u64),
    );
    let uid =
        cow_rs::trading::calculate_unique_order_id(SupportedChainId::Mainnet, &order, Env::Prod);
    assert!(uid.starts_with("0x"));
    assert_eq!(uid.len(), 2 + 112);
}

#[test]
fn resolve_order_book_api_creates_new_when_none() {
    let api = cow_rs::trading::resolve_order_book_api(SupportedChainId::Mainnet, Env::Prod, None);
    // Smoke test: the API should be usable.
    let _link = api.get_order_link("0xtest");
}

#[test]
fn unsigned_order_for_signing_is_identity() {
    let order = cow_rs::UnsignedOrder::sell(
        Address::ZERO,
        Address::ZERO,
        U256::from(1u64),
        U256::from(1u64),
    );
    let same = cow_rs::trading::unsigned_order_for_signing(order.clone());
    assert_eq!(same.sell_amount, order.sell_amount);
    assert_eq!(same.buy_amount, order.buy_amount);
}
