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

use std::{sync::Arc, time::Duration};

use cow_rs::{
    EcdsaSigningScheme, Env, OrderBookApi, OrderCancellations, OrderCreation, OrderKind,
    OrderQuoteRequest, RateLimiter, RetryPolicy, SigningScheme, SupportedChainId, TokenBalance,
    Trade, order_book::QuoteSide,
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

// ── Rate limiting and retry integration tests ────────────────────────────────

/// Build a retry policy with short delays so tests finish in milliseconds
/// rather than the default 100 ms × 2^N exponential curve.
fn fast_retry(max_attempts: u32) -> RetryPolicy {
    RetryPolicy {
        max_attempts,
        initial_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(10),
        retry_status_codes: cow_rs::order_book::DEFAULT_RETRY_STATUS_CODES,
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_version_retries_on_500_then_succeeds() {
    let server = MockServer::start().await;
    // First two attempts return 500, third returns 200. Each mock is
    // installed with its own expected-call range so we can assert every
    // retry actually landed on the server.
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/version"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(2)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!("1.2.3")))
        .expect(1)
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri())
        .with_retry_policy(fast_retry(5));
    let version = api.get_version().await.expect("third attempt should succeed");
    assert_eq!(version, "1.2.3");
    // Drop asserts the expected-call counts.
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_version_does_not_retry_on_400() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/version"))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
        .expect(1) // exactly one call — no retry
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri())
        .with_retry_policy(fast_retry(5));
    let err = api.get_version().await.expect_err("400 must surface as an error");
    assert!(
        matches!(err, cow_rs::error::CowError::Api { status: 400, .. }),
        "expected CowError::Api {{ status: 400 }}, got {err:?}"
    );
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_version_gives_up_after_max_attempts() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/version"))
        .respond_with(ResponseTemplate::new(503))
        .expect(3) // exactly `max_attempts` calls
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri())
        .with_retry_policy(fast_retry(3));
    let err = api.get_version().await.expect_err("exhausted retries must error");
    assert!(
        matches!(err, cow_rs::error::CowError::Api { status: 503, .. }),
        "expected CowError::Api {{ status: 503 }}, got {err:?}"
    );
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn no_retry_policy_fires_exactly_once() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/version"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri())
        .with_retry_policy(RetryPolicy::no_retry());
    let err = api.get_version().await.expect_err("500 with no retry policy errors");
    assert!(matches!(err, cow_rs::error::CowError::Api { status: 500, .. }));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_order_retries_on_429_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!(format!("0x{}", "ab".repeat(56)))),
        )
        .expect(1)
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri())
        .with_retry_policy(fast_retry(5));
    let order = OrderCreation {
        sell_token: "0x0000000000000000000000000000000000000001".parse().unwrap(),
        buy_token: "0x0000000000000000000000000000000000000002".parse().unwrap(),
        receiver: "0x0000000000000000000000000000000000000003".parse().unwrap(),
        sell_amount: "1".to_owned(),
        buy_amount: "1".to_owned(),
        valid_to: 2_000_000_000,
        app_data: "0x0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        fee_amount: "0".to_owned(),
        kind: OrderKind::Sell,
        partially_fillable: false,
        sell_token_balance: TokenBalance::Erc20,
        buy_token_balance: TokenBalance::Erc20,
        signing_scheme: SigningScheme::Eip712,
        signature: "0x".to_owned(),
        from: "0x0000000000000000000000000000000000000004".parse().unwrap(),
        quote_id: None,
    };
    let uid = api.send_order(&order).await.expect("retry should land on the 200");
    assert!(uid.starts_with("0x"));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn partner_api_header_is_sent_on_every_request() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/version"))
        .and(matchers::header("x-api-key", "secret-partner-key"))
        .and(matchers::header("x-client-version", "cow-rs-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!("1.2.3")))
        .expect(1)
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri())
        .with_headers([("X-API-Key", "secret-partner-key"), ("X-Client-Version", "cow-rs-test")])
        .with_retry_policy(RetryPolicy::no_retry());
    let version = api.get_version().await.expect("expected success");
    assert_eq!(version, "1.2.3");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn partner_api_headers_survive_retry() {
    let server = MockServer::start().await;
    // First attempt: 503 (no header match needed — wiremock will still
    // match the path). Second attempt: 200 with header match required.
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/version"))
        .and(matchers::header("x-api-key", "secret"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/version"))
        .and(matchers::header("x-api-key", "secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!("1.2.3")))
        .expect(1)
        .mount(&server)
        .await;

    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri())
        .with_header("X-API-Key", "secret")
        .with_retry_policy(fast_retry(5));
    let v = api.get_version().await.expect("header must be re-sent on every attempt");
    assert_eq!(v, "1.2.3");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn rate_limiter_serialises_concurrent_get_version() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/v1/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!("1.2.3")))
        .mount(&server)
        .await;

    // Capacity 1, rate ~50/s — only one request at a time, with ~20 ms
    // between them. Four concurrent `get_version()` calls must spend at
    // least 3 refills (~60 ms) waiting. `start_paused` is intentionally
    // NOT used here because `reqwest`'s timeout uses real network I/O
    // which does not mix with the paused tokio clock.
    let api = OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, server.uri())
        .with_rate_limiter(Arc::new(RateLimiter::new(50.0, 1.0)));

    let start = std::time::Instant::now();
    let results: [_; 4] = <[_; 4]>::from(tokio::join!(
        api.get_version(),
        api.get_version(),
        api.get_version(),
        api.get_version()
    ));
    let elapsed = start.elapsed();

    for r in results {
        assert_eq!(r.unwrap(), "1.2.3");
    }
    assert!(
        elapsed >= Duration::from_millis(50),
        "rate limiter should space 4 requests over >=50 ms, got {elapsed:?}"
    );
}
