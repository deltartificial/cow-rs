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

// ── TradingSdk::get_quote_only (no signer needed) ───────────────────────────

#[cfg_attr(miri, ignore)]
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
    assert!(!quote.order_to_sign.buy_amount.is_zero());
    // Receiver defaults to the quote owner when none is given.
    assert_eq!(quote.order_to_sign.receiver, owner);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_only_with_settings_applies_slippage_override() {
    use cow_rs::SwapAdvancedSettings;

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

    // Settings-level slippage wins over the 50 bps baked into the params.
    assert_eq!(quote.suggested_slippage_bps, 123);
}

// ── get_quote_without_signer free function ──────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_without_signer_does_not_need_a_signer() {
    use cow_rs::{OrderBookApi, get_quote_without_signer};

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
    assert_eq!(format!("{addr:#x}"), "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266");
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
    let settings =
        cow_rs::SwapAdvancedSettings { app_data: None, slippage_bps: Some(100), partner_fee: None };
    let quote = sdk.get_quote_with_settings(default_trade_params(), &settings).await.unwrap();
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
    let settings =
        cow_rs::SwapAdvancedSettings { app_data: None, slippage_bps: Some(100), partner_fee: None };
    let result =
        sdk.post_swap_order_with_settings(default_trade_params(), &settings).await.unwrap();
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
    let result =
        cow_rs::trading::get_slippage_percent(true, U256::ZERO, U256::ZERO, U256::from(1u64));
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

// ── TradingSdk::off_chain_cancel_order ──────────────────────────────────────

#[cfg_attr(miri, ignore)]
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
    let result = sdk.off_chain_cancel_order(uid, cow_rs::EcdsaSigningScheme::Eip712).await;
    assert!(result.is_ok());
}

// ── TradingSdk::off_chain_cancel_orders (multiple) ──────────────────────────

#[cfg_attr(miri, ignore)]
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
    let result =
        sdk.off_chain_cancel_orders(vec![uid1, uid2], cow_rs::EcdsaSigningScheme::Eip712).await;
    assert!(result.is_ok());
}

// ── TradingSdk::get_order_multi_env ─────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_orders (paginated) ──────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_orders_paginated_returns_list() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/account/.*/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let req = cow_rs::GetOrdersRequest::for_owner(
        "0x1111111111111111111111111111111111111111".parse().unwrap(),
    );
    let orders = sdk.get_orders(&req).await.unwrap();
    assert!(orders.is_empty());
}

// ── TradingSdk::get_trades_with_request ─────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_trades_with_request_returns_list() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v2/trades"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let sdk = make_sdk(&server);
    let req = cow_rs::GetTradesRequest {
        owner: Some("0x1111111111111111111111111111111111111111".parse().unwrap()),
        order_uid: None,
        limit: Some(5),
        offset: None,
    };
    let trades = sdk.get_trades_with_request(&req).await.unwrap();
    assert!(trades.is_empty());
}

// ── TradingSdk::get_solver_competition ──────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_solver_competition_by_tx ────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_solver_competition_latest_v2 ────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_solver_competition_v2 ───────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_solver_competition_by_tx_v2 ────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── TradingSdk::get_limit_trade_parameters ──────────────────────────────────

#[cfg_attr(miri, ignore)]
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

    assert_eq!(limit.sell_token, SELL_TOKEN.parse::<Address>().unwrap());
    assert_eq!(limit.buy_token, BUY_TOKEN.parse::<Address>().unwrap());
    assert!(!limit.sell_amount.is_zero());
    assert!(!limit.buy_amount.is_zero());
    assert_eq!(limit.kind, OrderKind::Sell);
}

// ── get_eth_flow_cancellation ───────────────────────────────────────────────

#[test]
fn get_eth_flow_cancellation_returns_calldata() {
    let uid = "0x".to_owned() + &"ab".repeat(56);
    let tx = cow_rs::trading::get_eth_flow_cancellation(SupportedChainId::Mainnet, Env::Prod, &uid)
        .unwrap();
    assert!(!tx.data.is_empty());
    assert_eq!(tx.value, U256::ZERO);
    assert_eq!(tx.gas_limit, cow_rs::GAS_LIMIT_DEFAULT);
    assert_ne!(tx.to, Address::ZERO);
}

// ── get_settlement_cancellation ─────────────────────────────────────────────

#[test]
fn get_settlement_cancellation_returns_calldata() {
    let uid = "0x".to_owned() + &"cd".repeat(56);
    let tx =
        cow_rs::trading::get_settlement_cancellation(SupportedChainId::Mainnet, Env::Prod, &uid)
            .unwrap();
    assert!(!tx.data.is_empty());
    assert_eq!(tx.value, U256::ZERO);
    assert_ne!(tx.to, Address::ZERO);
}

// ── get_order_to_sign ───────────────────────────────────────────────────────

#[test]
fn get_order_to_sign_returns_valid_order() {
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
    let from: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap();
    let order = cow_rs::trading::get_order_to_sign(
        SupportedChainId::Mainnet,
        from,
        false,
        U256::ZERO,
        false,
        &params,
        "0x0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_eq!(order.sell_token, SELL_TOKEN.parse::<Address>().unwrap());
    assert_eq!(order.buy_token, BUY_TOKEN.parse::<Address>().unwrap());
    assert_eq!(order.sell_amount, U256::from(1_000_000u64));
    assert_eq!(order.valid_to, 9_999_999);
    assert_eq!(order.receiver, from);
    assert_eq!(order.fee_amount, U256::ZERO);
}

#[test]
fn get_order_to_sign_with_slippage_adjustment() {
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
    let from: Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap();
    let order = cow_rs::trading::get_order_to_sign(
        SupportedChainId::Mainnet,
        from,
        false,
        U256::from(1000u64),
        true,
        &params,
        "0x0000000000000000000000000000000000000000000000000000000000000000",
    );
    // For sell orders with apply_costs_slippage_and_fees=true, buy_amount should be reduced.
    assert!(order.buy_amount < U256::from(500_000_000_000_000u64));
}

// ── get_order_typed_data ────────────────────────────────────────────────────

#[test]
fn get_order_typed_data_returns_typed_data() {
    let order = cow_rs::UnsignedOrder::sell(
        SELL_TOKEN.parse().unwrap(),
        BUY_TOKEN.parse().unwrap(),
        U256::from(1_000_000u64),
        U256::from(500_000_000_000_000u64),
    );
    let typed_data = cow_rs::trading::get_order_typed_data(SupportedChainId::Mainnet, order);
    assert_eq!(typed_data.domain.chain_id, SupportedChainId::Mainnet.as_u64());
}

// ── get_slippage_percent buy order ──────────────────────────────────────────

#[test]
fn get_slippage_percent_buy_order() {
    let result = cow_rs::trading::get_slippage_percent(
        false,
        U256::from(1_000_000u64),
        U256::from(999_000u64),
        U256::from(5_000u64),
    )
    .unwrap();
    assert!(result > 0.0);
    assert!(result < 1.0);
}

// ── adjust_eth_flow_order_params ────────────────────────────────────────────

#[test]
fn adjust_eth_flow_order_params_replaces_sell_token() {
    let params = TradeParameters {
        kind: OrderKind::Sell,
        sell_token: cow_rs::NATIVE_CURRENCY_ADDRESS,
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
    let adjusted = cow_rs::trading::adjust_eth_flow_order_params(SupportedChainId::Mainnet, params);
    assert_ne!(adjusted.sell_token, cow_rs::NATIVE_CURRENCY_ADDRESS);
}

// ── adjust_eth_flow_limit_order_params ──────────────────────────────────────

#[test]
fn adjust_eth_flow_limit_order_params_replaces_sell_token() {
    let params = cow_rs::LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: cow_rs::NATIVE_CURRENCY_ADDRESS,
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
    let adjusted =
        cow_rs::trading::adjust_eth_flow_limit_order_params(SupportedChainId::Mainnet, params);
    assert_ne!(adjusted.sell_token, cow_rs::NATIVE_CURRENCY_ADDRESS);
}

// ── get_trade_parameters_after_quote ────────────────────────────────────────

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
    let restored =
        cow_rs::trading::get_trade_parameters_after_quote(params, cow_rs::NATIVE_CURRENCY_ADDRESS);
    assert_eq!(restored.sell_token, cow_rs::NATIVE_CURRENCY_ADDRESS);
}

// ── TradingSdkConfig builder with utm and partner_fee ───────────────────────

#[test]
fn config_with_utm_and_partner_fee() {
    let utm = cow_rs::Utm {
        utm_source: Some("test".to_owned()),
        utm_medium: None,
        utm_campaign: None,
        utm_term: None,
        utm_content: None,
    };
    let fee = cow_rs::PartnerFee::single(cow_rs::PartnerFeeEntry::volume(
        10,
        "0x1111111111111111111111111111111111111111",
    ));
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "MyApp")
        .with_utm(utm)
        .with_partner_fee(fee);
    assert!(config.utm.is_some());
    assert!(config.partner_fee.is_some());
}

// ── get_order_deadline_from_now ─────────────────────────────────────────────

#[test]
fn get_order_deadline_from_now_is_in_future() {
    let deadline = cow_rs::trading::get_order_deadline_from_now(1800);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        as u32;
    assert!(deadline > now);
}

// ── build_app_data with partner_fee ─────────────────────────────────────────

#[test]
fn build_app_data_with_partner_fee() {
    let fee = cow_rs::PartnerFee::single(cow_rs::PartnerFeeEntry::volume(
        100,
        "0x1111111111111111111111111111111111111111",
    ));
    let info =
        cow_rs::trading::build_app_data("MyDApp", 50, cow_rs::OrderClassKind::Limit, Some(&fee));
    assert!(!info.full_app_data.is_empty());
    assert!(info.app_data_keccak256.starts_with("0x"));
}

// ── get_settlement_contract staging ─────────────────────────────────────────

#[test]
fn get_settlement_contract_staging_non_zero() {
    let addr = cow_rs::trading::get_settlement_contract(SupportedChainId::Mainnet, Env::Staging);
    assert_ne!(addr, Address::ZERO);
}

// ── get_eth_flow_contract staging ───────────────────────────────────────────

#[test]
fn get_eth_flow_contract_staging_non_zero() {
    let addr = cow_rs::trading::get_eth_flow_contract(SupportedChainId::Mainnet, Env::Staging);
    assert_ne!(addr, Address::ZERO);
}

// ── get_eth_flow_transaction staging env ────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_eth_flow_transaction_staging_returns_tx_params() {
    let config = TradingSdkConfig::staging(SupportedChainId::Mainnet, "TestApp");
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

// ── post_cow_protocol_trade standalone ──────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn post_cow_protocol_trade_submits_order() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"66".repeat(56);

    // Mock app-data upload (non-fatal).
    Mock::given(matchers::method("PUT"))
        .and(matchers::path_regex(r"/api/v1/app_data/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fullAppData": "{}"
        })))
        .mount(&server)
        .await;

    // Mock order submission.
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let api =
        cow_rs::OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let signer = cow_rs::trading::resolve_signer(Some(TEST_KEY)).unwrap();
    let app_data =
        cow_rs::trading::build_app_data("TestApp", 50, cow_rs::OrderClassKind::Market, None);
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
    let additional = cow_rs::PostTradeAdditionalParams {
        signing_scheme: None,
        network_costs_amount: None,
        apply_costs_slippage_and_fees: None,
    };
    let result = cow_rs::trading::post_cow_protocol_trade(
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

// ── post_cow_protocol_trade with EthSign signing scheme ────────────────────

#[cfg_attr(miri, ignore)]
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

    let api =
        cow_rs::OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let signer = cow_rs::trading::resolve_signer(Some(TEST_KEY)).unwrap();
    let app_data =
        cow_rs::trading::build_app_data("TestApp", 50, cow_rs::OrderClassKind::Market, None);
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
    let additional = cow_rs::PostTradeAdditionalParams {
        signing_scheme: Some(cow_rs::SigningScheme::EthSign),
        network_costs_amount: Some("5000".to_owned()),
        apply_costs_slippage_and_fees: Some(false),
    };
    let result = cow_rs::trading::post_cow_protocol_trade(
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

// ── post_cow_protocol_trade rejects ETH-flow orders ────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn post_cow_protocol_trade_rejects_eth_flow() {
    let server = MockServer::start().await;
    let api =
        cow_rs::OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let signer = cow_rs::trading::resolve_signer(Some(TEST_KEY)).unwrap();
    let app_data =
        cow_rs::trading::build_app_data("TestApp", 50, cow_rs::OrderClassKind::Market, None);
    let params = cow_rs::LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: cow_rs::NATIVE_CURRENCY_ADDRESS,
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
    let additional = cow_rs::PostTradeAdditionalParams {
        signing_scheme: None,
        network_costs_amount: None,
        apply_costs_slippage_and_fees: None,
    };
    let result = cow_rs::trading::post_cow_protocol_trade(
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

// ── post_sell_native_currency_order standalone ──────────────────────────────

#[cfg_attr(miri, ignore)]
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

    let api =
        cow_rs::OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let app_data =
        cow_rs::trading::build_app_data("TestApp", 50, cow_rs::OrderClassKind::Market, None);
    let params = cow_rs::LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: cow_rs::NATIVE_CURRENCY_ADDRESS,
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
    let (result, tx) = cow_rs::trading::post_sell_native_currency_order(
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

// ── get_quote_raw standalone ───────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_raw_returns_response() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let api =
        cow_rs::OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let req = cow_rs::OrderQuoteRequest::new(
        SELL_TOKEN.parse().unwrap(),
        BUY_TOKEN.parse().unwrap(),
        Address::ZERO,
        cow_rs::order_book::QuoteSide::sell("1000000"),
    );
    let resp = cow_rs::trading::get_quote_raw(&api, &req).await.unwrap();
    assert!(!resp.quote.sell_amount.is_empty());
}

// ── get_quote_with_signer standalone ───────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_with_signer_returns_result_and_signer() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let api =
        cow_rs::OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let result = cow_rs::trading::get_quote_with_signer(
        &config,
        &api,
        TEST_KEY,
        default_trade_params(),
        None,
    )
    .await
    .unwrap();
    assert!(!result.result.order_to_sign.sell_amount.is_zero());
    assert_ne!(alloy_signer::Signer::address(&result.signer), Address::ZERO);
}

// ── get_quote_with_signer with settings ────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_with_signer_with_settings() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let api =
        cow_rs::OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let settings =
        cow_rs::SwapAdvancedSettings { app_data: None, slippage_bps: Some(200), partner_fee: None };
    let result = cow_rs::trading::get_quote_with_signer(
        &config,
        &api,
        TEST_KEY,
        default_trade_params(),
        Some(&settings),
    )
    .await
    .unwrap();
    assert_eq!(result.result.suggested_slippage_bps, 200);
}

// ── get_trader with explicit owner ─────────────────────────────────────────

#[test]
fn get_trader_with_explicit_owner() {
    let signer = cow_rs::trading::resolve_signer(Some(TEST_KEY)).unwrap();
    let explicit: Address = "0x1111111111111111111111111111111111111111".parse().unwrap();
    let trader =
        cow_rs::trading::get_trader(SupportedChainId::Mainnet, "MyApp", Some(explicit), &signer);
    assert_eq!(trader.account, explicit);
    assert_eq!(trader.app_code, "MyApp");
}

// ── get_trader without owner falls back to signer ──────────────────────────

#[test]
fn get_trader_without_owner_uses_signer() {
    let signer = cow_rs::trading::resolve_signer(Some(TEST_KEY)).unwrap();
    let trader = cow_rs::trading::get_trader(SupportedChainId::Mainnet, "MyApp", None, &signer);
    assert_eq!(trader.account, alloy_signer::Signer::address(&signer));
}

// ── resolve_slippage_suggestion ────────────────────────────────────────────

#[test]
fn resolve_slippage_suggestion_returns_none_when_sufficient() {
    use cow_rs::order_book::{OrderQuoteResponse, QuoteData};
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
    // With a very high slippage, no suggestion should be needed.
    let suggestion = cow_rs::trading::resolve_slippage_suggestion(
        SupportedChainId::Mainnet,
        false,
        &quote,
        10_000,
    );
    assert!(suggestion.is_none());
}

// ── resolve_slippage_suggestion with low slippage may suggest higher ───────

#[test]
fn resolve_slippage_suggestion_with_low_slippage() {
    use cow_rs::order_book::{OrderQuoteResponse, QuoteData};
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
            fee_amount: "100000".to_owned(),
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
    // With a very low slippage and high fee, a suggestion may be returned.
    let _suggestion =
        cow_rs::trading::resolve_slippage_suggestion(SupportedChainId::Mainnet, false, &quote, 0);
    // The result depends on the heuristic; just verify it doesn't panic.
}

// ── resolve_slippage_suggestion for eth_flow ───────────────────────────────

#[test]
fn resolve_slippage_suggestion_eth_flow() {
    use cow_rs::order_book::{OrderQuoteResponse, QuoteData};
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
    let _suggestion =
        cow_rs::trading::resolve_slippage_suggestion(SupportedChainId::Mainnet, true, &quote, 50);
}

// ── get_order_to_sign with valid_for but no valid_to ───────────────────────

#[test]
fn get_order_to_sign_with_valid_for_only() {
    let params = cow_rs::LimitTradeParameters {
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
    let order = cow_rs::trading::get_order_to_sign(
        SupportedChainId::Mainnet,
        from,
        false,
        U256::ZERO,
        false,
        &params,
        "0x0000000000000000000000000000000000000000000000000000000000000000",
    );
    // valid_to should be in the future.
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        as u32;
    assert!(order.valid_to > now);
}

// ── get_order_to_sign buy order with slippage ──────────────────────────────

#[test]
fn get_order_to_sign_buy_order_with_slippage() {
    let params = cow_rs::LimitTradeParameters {
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
    let order = cow_rs::trading::get_order_to_sign(
        SupportedChainId::Mainnet,
        from,
        false,
        U256::from(1000u64),
        true,
        &params,
        "0x0000000000000000000000000000000000000000000000000000000000000000",
    );
    // For buy orders with apply_costs=true, sell_amount should be increased.
    assert!(order.sell_amount > U256::from(1_000_000u64));
    // Receiver should be the explicit one, not `from`.
    assert_eq!(
        order.receiver,
        "0x1111111111111111111111111111111111111111".parse::<Address>().unwrap()
    );
}

// ── get_order_to_sign eth_flow ─────────────────────────────────────────────

#[test]
fn get_order_to_sign_eth_flow() {
    let params = cow_rs::LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: cow_rs::NATIVE_CURRENCY_ADDRESS,
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
    let order = cow_rs::trading::get_order_to_sign(
        SupportedChainId::Mainnet,
        from,
        true,
        U256::ZERO,
        true,
        &params,
        "0x0000000000000000000000000000000000000000000000000000000000000000",
    );
    // ETH-flow slippage should reduce the buy_amount.
    assert!(order.buy_amount < U256::from(500_000_000_000_000u64));
}

// ── post_limit_order with custom app_data and partner_fee ──────────────────

#[cfg_attr(miri, ignore)]
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
    let params = cow_rs::LimitTradeParameters {
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
        partner_fee: Some(cow_rs::PartnerFee::single(cow_rs::PartnerFeeEntry::volume(
            100,
            "0x1111111111111111111111111111111111111111",
        ))),
    };
    let result =
        sdk.post_limit_order(params, Some(cow_rs::EcdsaSigningScheme::EthSign)).await.unwrap();
    assert_eq!(result.order_id, uid);
}

// ── post_limit_order with valid_to and config partner_fee ──────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn post_limit_order_with_config_partner_fee() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"69".repeat(56);
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let fee = cow_rs::PartnerFee::single(cow_rs::PartnerFeeEntry::volume(
        50,
        "0x2222222222222222222222222222222222222222",
    ));
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp").with_partner_fee(fee);
    let sdk = TradingSdk::new_with_url(config, TEST_KEY, server.uri()).unwrap();
    let params = cow_rs::LimitTradeParameters {
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

// ── get_quote with Buy kind ────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── get_quote with partner_fee override in settings ────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_with_settings_partner_fee_override() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let fee = cow_rs::PartnerFee::single(cow_rs::PartnerFeeEntry::volume(
        100,
        "0x1111111111111111111111111111111111111111",
    ));
    let sdk = make_sdk(&server);
    let settings =
        cow_rs::SwapAdvancedSettings { app_data: None, slippage_bps: None, partner_fee: Some(fee) };
    let quote = sdk.get_quote_with_settings(default_trade_params(), &settings).await.unwrap();
    assert!(!quote.order_to_sign.sell_amount.is_zero());
}

// ── get_quote with config-level partner_fee ────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_with_config_partner_fee() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let fee = cow_rs::PartnerFee::single(cow_rs::PartnerFeeEntry::volume(
        50,
        "0x2222222222222222222222222222222222222222",
    ));
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp").with_partner_fee(fee);
    let sdk = TradingSdk::new_with_url(config, TEST_KEY, server.uri()).unwrap();
    let quote = sdk.get_quote(default_trade_params()).await.unwrap();
    assert!(!quote.order_to_sign.sell_amount.is_zero());
}

// ── get_quote with config-level UTM ────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_with_config_utm() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let utm = cow_rs::Utm {
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

// ── get_quote with partially_fillable = true ───────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── get_quote with params-level partner_fee ────────────────────────────────

#[cfg_attr(miri, ignore)]
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
    params.partner_fee = Some(cow_rs::PartnerFee::single(cow_rs::PartnerFeeEntry::volume(
        100,
        "0x1111111111111111111111111111111111111111",
    )));
    let quote = sdk.get_quote(params).await.unwrap();
    assert!(!quote.order_to_sign.sell_amount.is_zero());
}

// ── get_quote with valid_for (no valid_to) ─────────────────────────────────

#[cfg_attr(miri, ignore)]
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

// ── get_quote 422 error ────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_422_returns_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(422).set_body_string("unprocessable entity"))
        .mount(&server)
        .await;

    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new_with_url(config, TEST_KEY, server.uri()).unwrap();
    let result = sdk.get_quote(default_trade_params()).await;
    assert!(result.is_err());
}

// ── post_swap_order_from_quote with EthSign scheme ─────────────────────────

#[cfg_attr(miri, ignore)]
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
    let result = sdk
        .post_swap_order_from_quote(&quote, Some(cow_rs::EcdsaSigningScheme::EthSign))
        .await
        .unwrap();
    assert_eq!(result.order_id, uid);
}

// ── new_with_url invalid key ───────────────────────────────────────────────

#[test]
fn new_with_url_invalid_key_returns_error() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let result = TradingSdk::new_with_url(config, "not-a-key", "http://localhost:1234");
    assert!(result.is_err());
}

// ── swap_params_to_limit_order_params with bad amounts ─────────────────────

#[test]
fn swap_params_to_limit_order_params_bad_amounts_fallback_to_zero() {
    use cow_rs::order_book::{OrderQuoteResponse, QuoteData};
    let params = default_trade_params();
    let quote = OrderQuoteResponse {
        quote: QuoteData {
            sell_token: SELL_TOKEN.parse().unwrap(),
            buy_token: BUY_TOKEN.parse().unwrap(),
            receiver: None,
            sell_amount: "not-a-number".to_owned(),
            buy_amount: "also-bad".to_owned(),
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
    assert_eq!(limit.sell_amount, U256::ZERO);
    assert_eq!(limit.buy_amount, U256::ZERO);
}

// ── resolve_order_book_api with existing ───────────────────────────────────

#[test]
fn resolve_order_book_api_returns_existing_when_provided() {
    let api = cow_rs::OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    let link_before = api.get_order_link("0xtest");
    let returned =
        cow_rs::trading::resolve_order_book_api(SupportedChainId::Sepolia, Env::Staging, Some(api));
    // Should return the same API instance regardless of chain/env args.
    let link_after = returned.get_order_link("0xtest");
    assert_eq!(link_before, link_after);
}

// ── post_cow_protocol_trade with Eip1271 and PreSign signing schemes ──────

#[cfg_attr(miri, ignore)]
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

    let api =
        cow_rs::OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let signer = cow_rs::trading::resolve_signer(Some(TEST_KEY)).unwrap();
    let app_data =
        cow_rs::trading::build_app_data("TestApp", 50, cow_rs::OrderClassKind::Market, None);
    let params = cow_rs::LimitTradeParameters {
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
    let additional = cow_rs::PostTradeAdditionalParams {
        signing_scheme: Some(cow_rs::SigningScheme::Eip1271),
        network_costs_amount: None,
        apply_costs_slippage_and_fees: None,
    };
    let result = cow_rs::trading::post_cow_protocol_trade(
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

#[cfg_attr(miri, ignore)]
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

    let api =
        cow_rs::OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri());
    let signer = cow_rs::trading::resolve_signer(Some(TEST_KEY)).unwrap();
    let app_data =
        cow_rs::trading::build_app_data("TestApp", 50, cow_rs::OrderClassKind::Market, None);
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
    let additional = cow_rs::PostTradeAdditionalParams {
        signing_scheme: Some(cow_rs::SigningScheme::PreSign),
        network_costs_amount: None,
        apply_costs_slippage_and_fees: None,
    };
    let result = cow_rs::trading::post_cow_protocol_trade(
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

// ── PostTradeAdditionalParams helpers ──────────────────────────────────────

#[test]
fn post_trade_additional_params_builder() {
    let params = cow_rs::PostTradeAdditionalParams {
        signing_scheme: None,
        network_costs_amount: None,
        apply_costs_slippage_and_fees: None,
    }
    .with_signing_scheme(cow_rs::SigningScheme::EthSign)
    .with_network_costs_amount("5000")
    .with_apply_costs_slippage_and_fees(true);

    assert!(params.has_signing_scheme());
    assert!(params.has_network_costs());
    assert!(params.should_apply_costs());
    // Display impl
    assert_eq!(format!("{params}"), "post-trade-params");
}

// ── TradingAppDataInfo helpers ─────────────────────────────────────────────

#[test]
fn trading_app_data_info_methods() {
    let info = cow_rs::TradingAppDataInfo::new("{\"test\": true}", "0xdeadbeef");
    assert!(info.has_full_app_data());
    assert_eq!(info.full_app_data_ref(), "{\"test\": true}");
    assert_eq!(info.keccak256_ref(), "0xdeadbeef");
    assert_eq!(format!("{info}"), "app-data(0xdeadbeef)");
}

#[test]
fn trading_app_data_info_empty() {
    let info = cow_rs::TradingAppDataInfo::new("", "");
    assert!(!info.has_full_app_data());
}

// ── mock_get_order ─────────────────────────────────────────────────────────

#[test]
fn mock_get_order_returns_valid_order() {
    let uid = "0xtest123";
    let order = cow_rs::order_book::mock_get_order(uid);
    assert_eq!(order.uid, uid);
    assert_eq!(order.kind, OrderKind::Sell);
    assert!(!order.invalidated);
}

// ── TradingSdkConfig with_utm and with_partner_fee ─────────────────────────

#[test]
fn config_with_utm() {
    let utm = cow_rs::trading::get_default_utm_params();
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp").with_utm(utm);
    assert!(config.utm.is_some());
    assert_eq!(config.utm.as_ref().unwrap().utm_source.as_deref(), Some("web"));
}

#[test]
fn config_with_partner_fee() {
    use cow_rs::app_data::types::{PartnerFee, PartnerFeeEntry};
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

// ── TradingSdkConfig Debug impl ────────────────────────────────────────────

#[test]
fn config_debug_impl() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let debug = format!("{config:?}");
    assert!(debug.contains("TradingSdkConfig"));
    assert!(debug.contains("Mainnet"));
}

// ── TradingSdk Debug impl ──────────────────────────────────────────────────

#[test]
fn trading_sdk_debug_impl() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let debug = format!("{sdk:?}");
    assert!(debug.contains("TradingSdk"));
}

// ── TradingSdk Clone impl ──────────────────────────────────────────────────

#[test]
fn trading_sdk_clone() {
    let config = TradingSdkConfig::prod(SupportedChainId::Mainnet, "TestApp");
    let sdk = TradingSdk::new(config, TEST_KEY).unwrap();
    let cloned = sdk.clone();
    assert_eq!(cloned.address(), sdk.address());
}

// ── swap_params_to_limit_order_params with invalid amounts ─────────────────

#[test]
fn swap_params_to_limit_order_params_invalid_amounts_default_to_zero() {
    use cow_rs::order_book::{OrderQuoteResponse, QuoteData};
    let params = default_trade_params();
    let quote = OrderQuoteResponse {
        quote: QuoteData {
            sell_token: SELL_TOKEN.parse().unwrap(),
            buy_token: BUY_TOKEN.parse().unwrap(),
            receiver: None,
            sell_amount: "not_a_number".to_owned(),
            buy_amount: "also_not_a_number".to_owned(),
            valid_to: 9_999_999,
            app_data: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            fee_amount: "0".to_owned(),
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
    // Invalid amounts should default to zero
    assert_eq!(limit.sell_amount, U256::ZERO);
    assert_eq!(limit.buy_amount, U256::ZERO);
}

// ── ETH_FLOW_DEFAULT_SLIPPAGE_BPS equals DEFAULT_SLIPPAGE_BPS ──────────────

#[test]
fn eth_flow_slippage_equals_default() {
    assert_eq!(cow_rs::ETH_FLOW_DEFAULT_SLIPPAGE_BPS, cow_rs::DEFAULT_SLIPPAGE_BPS);
}

// ── staging config ─────────────────────────────────────────────────────────

#[test]
fn config_staging_values() {
    let config = TradingSdkConfig::staging(SupportedChainId::Sepolia, "StageApp");
    assert!(matches!(config.env, Env::Staging));
    assert_eq!(config.app_code, "StageApp");
    assert_eq!(config.slippage_bps, cow_rs::DEFAULT_SLIPPAGE_BPS);
    assert!(config.rpc_url.is_none());
    assert!(config.utm.is_none());
    assert!(config.partner_fee.is_none());
}
