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
//! Wiremock-based integration tests for [`OrderBookApi`] HTTP endpoints.

use cow_rs::{
    EcdsaSigningScheme, Env, OrderBookApi, OrderCancellations, OrderCreation, OrderKind,
    OrderQuoteRequest, SigningScheme, SupportedChainId, TokenBalance, Trade, order_book::QuoteSide,
};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

fn make_api(server: &MockServer) -> OrderBookApi {
    OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri())
}

// ── GET /api/v1/version ───────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_version_returns_version_string() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!("1.2.3")))
        .mount(&server)
        .await;
    let api = make_api(&server);
    let version = api.get_version().await.unwrap();
    assert_eq!(version, "1.2.3");
}

// ── GET /api/v1/token/{address}/native_price ───────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_native_price_parses_float() {
    let server = MockServer::start().await;
    let token = alloy_primitives::address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/token/.*/native_price"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "price": 0.000_4 })),
        )
        .mount(&server)
        .await;
    let price = make_api(&server).get_native_price(token).await.unwrap();
    assert!(price > 0.0);
}

// ── POST /api/v1/orders ───────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_order_returns_uid() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"aa".repeat(56);
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&uid))
        .mount(&server)
        .await;

    let order = OrderCreation {
        sell_token: alloy_primitives::Address::ZERO,
        buy_token: alloy_primitives::Address::ZERO,
        receiver: alloy_primitives::Address::ZERO,
        sell_amount: "1000".to_owned(),
        buy_amount: "900".to_owned(),
        valid_to: 9999,
        app_data: "0x0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        fee_amount: "0".to_owned(),
        kind: OrderKind::Sell,
        partially_fillable: false,
        sell_token_balance: TokenBalance::Erc20,
        buy_token_balance: TokenBalance::Erc20,
        signing_scheme: SigningScheme::Eip712,
        signature: "0xabcd".into(),
        from: alloy_primitives::Address::ZERO,
        quote_id: None,
    };
    let result = make_api(&server).send_order(&order).await.unwrap();
    assert!(result.starts_with("0x"));
}

// ── GET /api/v1/orders/{uid} ──────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_order_returns_order() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"bb".repeat(56);
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/orders/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_order_json(&uid)))
        .mount(&server)
        .await;
    let order = make_api(&server).get_order(&uid).await.unwrap();
    // Order.uid is a String
    assert_eq!(order.uid, uid);
}

// ── GET /api/v1/auction ───────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_auction_parses_response() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/auction"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id":    1,
            "block": 100,
            "latestSettlementBlock": 99,
            "orders": [],
            "prices": {}
        })))
        .mount(&server)
        .await;
    let auction = make_api(&server).get_auction().await.unwrap();
    assert_eq!(auction.id, Some(1));
}

// ── GET /api/v1/users/{address}/total_surplus ────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_total_surplus_parses_amount() {
    let server = MockServer::start().await;
    let address = alloy_primitives::address!("1111111111111111111111111111111111111111");
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/users/.*/total_surplus"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "totalSurplus": "100000000"
        })))
        .mount(&server)
        .await;
    let surplus = make_api(&server).get_total_surplus(address).await.unwrap();
    assert!(!surplus.total_surplus.is_empty());
}

// ── GET /api/v1/app_data/{hash} ───────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_app_data_returns_full_data() {
    let server = MockServer::start().await;
    let hash = "0x".to_owned() + &"cc".repeat(32);
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/app_data/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fullAppData": "{\"appCode\":\"TestApp\"}"
        })))
        .mount(&server)
        .await;
    let result = make_api(&server).get_app_data(&hash).await.unwrap();
    assert!(result.full_app_data.contains("TestApp"));
}

// ── GET /api/v2/trades?owner= ─────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_trades_for_account_returns_list() {
    let server = MockServer::start().await;
    let address = alloy_primitives::address!("1111111111111111111111111111111111111111");
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v2/trades"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;
    let trades: Vec<Trade> = make_api(&server).get_trades_for_account(address, None).await.unwrap();
    assert!(trades.is_empty());
}

// ── GET /api/v1/orders/{uid}/status ──────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_order_status_open() {
    let server = MockServer::start().await;
    let uid = "0x".to_owned() + &"dd".repeat(56);
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/orders/.*/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "type": "open",
            "value": null
        })))
        .mount(&server)
        .await;
    let status = make_api(&server).get_order_status(&uid).await.unwrap();
    assert!(status.kind.is_open());
}

// ── 4xx errors are surfaced as CowError::Api ──────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_order_404_returns_api_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/orders/.*"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;
    let result = make_api(&server).get_order("0xnonexistent").await;
    match result {
        Err(cow_rs::CowError::Api { status, .. }) => assert_eq!(status, 404),
        other => panic!("expected Api error, got {other:?}"),
    }
}

// ── POST /api/v1/quote ────────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_quote_parses_response() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_quote_response_json()))
        .mount(&server)
        .await;

    let req = OrderQuoteRequest::new(
        alloy_primitives::Address::ZERO,
        alloy_primitives::Address::ZERO,
        alloy_primitives::Address::ZERO,
        QuoteSide::sell("1000000"),
    );
    let resp = make_api(&server).get_quote(&req).await.unwrap();
    assert!(!resp.quote.sell_amount.is_empty());
}

// ── GET /api/v1/account/{address}/orders ─────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_orders_for_account_returns_list() {
    let server = MockServer::start().await;
    let address = alloy_primitives::address!("2222222222222222222222222222222222222222");
    Mock::given(matchers::method("GET"))
        .and(matchers::path_regex(r"/api/v1/account/.*/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;
    let orders = make_api(&server).get_orders_for_account(address, None).await.unwrap();
    assert!(orders.is_empty());
}

// ── DELETE /api/v1/orders (cancel) ────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn cancel_orders_succeeds_on_200() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("DELETE"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let cancels = OrderCancellations {
        order_uids: vec!["0xabc".to_owned()],
        signature: "0xsig".into(),
        signing_scheme: EcdsaSigningScheme::Eip712,
    };
    make_api(&server).cancel_orders(&cancels).await.unwrap();
}

// ── GET order_link ────────────────────────────────────────────────────────────

#[test]
fn get_order_link_contains_uid() {
    let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    let link = api.get_order_link("0xmyuid");
    assert!(link.contains("0xmyuid"));
}

// ── Helper JSON builders ──────────────────────────────────────────────────────

fn make_order_json(uid: &str) -> serde_json::Value {
    serde_json::json!({
        "uid":                         uid,
        "owner":                       "0x1111111111111111111111111111111111111111",
        "creationDate":                "2024-01-01T00:00:00.000Z",
        "status":                      "open",
        "class":                       "limit",
        "sellToken":                   "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        "buyToken":                    "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        "receiver":                    null,
        "sellAmount":                  "1000000",
        "buyAmount":                   "1000000000000000",
        "validTo":                     9999,
        "appData":                     "0x0000000000000000000000000000000000000000000000000000000000000000",
        "fullAppData":                 null,
        "feeAmount":                   "0",
        "kind":                        "sell",
        "partiallyFillable":           false,
        "executedSellAmount":          "0",
        "executedBuyAmount":           "0",
        "executedSellAmountBeforeFees":"0",
        "executedFeeAmount":           "0",
        "invalidated":                 false,
        "signingScheme":               "eip712",
        "signature":                   "0xabcd"
    })
}

fn make_quote_response_json() -> serde_json::Value {
    serde_json::json!({
        "quote": {
            "sellToken":        "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
            "buyToken":         "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
            "receiver":         null,
            "sellAmount":       "1000000",
            "buyAmount":        "500000000000000",
            "validTo":          9999,
            "appData":          "0x0000000000000000000000000000000000000000000000000000000000000000",
            "feeAmount":        "1000",
            "kind":             "sell",
            "partiallyFillable": false,
            "sellTokenBalance": "erc20",
            "buyTokenBalance":  "erc20"
        },
        "from":       "0x0000000000000000000000000000000000000000",
        "expiration": "2099-01-01T00:00:00.000Z",
        "id":         1,
        "verified":   false
    })
}
