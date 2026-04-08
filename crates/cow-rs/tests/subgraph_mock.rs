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
//! Wiremock-based integration tests for [`SubgraphApi`] `GraphQL` methods.
//!
//! Each test mounts a local mock server that accepts `POST /graphql` and
//! returns a canned `{"data": {...}}` response.  The test then calls the
//! corresponding [`SubgraphApi`] method and asserts the parsed result.

use cow_rs::{Bundle, SubgraphApi, SubgraphOrder, SubgraphToken, SubgraphTrade};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

fn make_api(server: &MockServer) -> SubgraphApi {
    SubgraphApi::new_with_url(server.uri())
}

// ── Helper JSON builders ──────────────────────────────────────────────────────

fn token_json() -> serde_json::Value {
    serde_json::json!({
        "id":                 "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
        "address":            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        "firstTradeTimestamp": "1609459200",
        "name":               "Wrapped Ether",
        "symbol":             "WETH",
        "decimals":           "18",
        "totalVolume":        "1000000000000000000",
        "priceEth":           "1.0",
        "priceUsd":           "2500.00",
        "numberOfTrades":     "42"
    })
}

fn user_json() -> serde_json::Value {
    serde_json::json!({
        "id":                  "0x1111111111111111111111111111111111111111",
        "address":             "0x1111111111111111111111111111111111111111",
        "firstTradeTimestamp": "1609459200",
        "numberOfTrades":      "5",
        "solvedAmountEth":     "2.5",
        "solvedAmountUsd":     "6250.00"
    })
}

fn graphql_data(data: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "data": data })
}

// ── get_totals ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_totals_returns_parsed_totals() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "totals": [{
                "tokens":      "500",
                "orders":      "100000",
                "traders":     "20000",
                "settlements": "5000",
                "volumeUsd":   "1000000000",
                "volumeEth":   "400000",
                "feesUsd":     "500000",
                "feesEth":     "200"
            }]
        }))))
        .mount(&server)
        .await;

    let totals = make_api(&server).get_totals().await.unwrap();
    assert_eq!(totals.len(), 1);
    assert_eq!(totals[0].orders, "100000");
    assert_eq!(totals[0].traders, "20000");
    assert_eq!(totals[0].tokens, "500");
}

#[tokio::test]
async fn get_totals_empty_returns_empty_vec() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "totals": []
        }))))
        .mount(&server)
        .await;

    let totals = make_api(&server).get_totals().await.unwrap();
    assert!(totals.is_empty());
}

// ── get_last_days_volume ──────────────────────────────────────────────────────

#[tokio::test]
async fn get_last_days_volume_returns_two_entries() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "dailyTotals": [
                { "timestamp": "1700000000", "volumeUsd": "1000000" },
                { "timestamp": "1699913600", "volumeUsd": "800000" }
            ]
        }))))
        .mount(&server)
        .await;

    let vols = make_api(&server).get_last_days_volume(2).await.unwrap();
    assert_eq!(vols.len(), 2);
    assert_eq!(vols[0].timestamp, "1700000000");
    assert_eq!(vols[1].volume_usd, "800000");
}

// ── get_last_hours_volume ─────────────────────────────────────────────────────

#[tokio::test]
async fn get_last_hours_volume_returns_hourly_entries() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "hourlyTotals": [
                { "timestamp": "1700000000", "volumeUsd": "100000" }
            ]
        }))))
        .mount(&server)
        .await;

    let vols = make_api(&server).get_last_hours_volume(1).await.unwrap();
    assert_eq!(vols.len(), 1);
    assert_eq!(vols[0].volume_usd, "100000");
}

// ── get_daily_totals ──────────────────────────────────────────────────────────

#[tokio::test]
async fn get_daily_totals_parses_all_fields() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "dailyTotals": [{
                "timestamp":   "1700000000",
                "orders":      "100",
                "traders":     "50",
                "tokens":      "20",
                "settlements": "10",
                "volumeEth":   "100.0",
                "volumeUsd":   "250000.0",
                "feesEth":     "0.5",
                "feesUsd":     "1250.0"
            }]
        }))))
        .mount(&server)
        .await;

    let totals = make_api(&server).get_daily_totals(1).await.unwrap();
    assert_eq!(totals.len(), 1);
    assert_eq!(totals[0].orders, "100");
    assert_eq!(totals[0].traders, "50");
}

// ── get_tokens ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_tokens_returns_token_list() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "tokens": [token_json()]
        }))))
        .mount(&server)
        .await;

    let tokens: Vec<SubgraphToken> = make_api(&server).get_tokens(1).await.unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].symbol_ref(), "WETH");
    assert_eq!(tokens[0].decimals, "18");
}

#[tokio::test]
async fn get_tokens_empty_returns_empty_vec() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "tokens": []
        }))))
        .mount(&server)
        .await;

    let tokens: Vec<SubgraphToken> = make_api(&server).get_tokens(10).await.unwrap();
    assert!(tokens.is_empty());
}

// ── get_token ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_token_returns_single_token() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "token": token_json()
        }))))
        .mount(&server)
        .await;

    let token: SubgraphToken =
        make_api(&server).get_token("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").await.unwrap();
    assert_eq!(token.symbol_ref(), "WETH");
    assert_eq!(token.address_ref(), "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
}

// ── get_eth_price ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_eth_price_parses_bundle() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "bundle": { "id": "1", "ethPriceUsd": "2500.00" }
        }))))
        .mount(&server)
        .await;

    let bundle: Bundle = make_api(&server).get_eth_price().await.unwrap();
    assert_eq!(bundle.id, "1");
    assert_eq!(bundle.eth_price_usd_ref(), "2500.00");
}

// ── get_trades ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_trades_returns_trade_list() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "trades": [{
                "id":                    "0xabc-0",
                "timestamp":             "1700000000",
                "gasPrice":              "10000000000",
                "feeAmount":             "1000",
                "txHash":                "0xabcdef",
                "settlement":            "0xabcdef",
                "buyAmount":             "500000000000000000",
                "sellAmount":            "999000",
                "sellAmountBeforeFees":  "1000000",
                "buyToken":  token_json(),
                "sellToken": token_json(),
                "owner":     user_json(),
                "order":     "0xorder"
            }]
        }))))
        .mount(&server)
        .await;

    let trades: Vec<SubgraphTrade> = make_api(&server).get_trades(1).await.unwrap();
    assert_eq!(trades.len(), 1);
    assert!(trades[0].has_tx_hash());
    assert_eq!(trades[0].tx_hash, "0xabcdef");
}

// ── get_orders_for_owner ──────────────────────────────────────────────────────

#[tokio::test]
async fn get_orders_for_owner_returns_order_list() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "orders": [{
                "id":                           "0xababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab",
                "owner":                        user_json(),
                "sellToken":                    token_json(),
                "buyToken":                     token_json(),
                "receiver":                     null,
                "sellAmount":                   "1000000",
                "buyAmount":                    "1000000000000000",
                "validTo":                      "9999999",
                "appData":                      "0x0000000000000000000000000000000000000000000000000000000000000000",
                "feeAmount":                    "0",
                "kind":                         "sell",
                "partiallyFillable":            false,
                "status":                       "open",
                "executedSellAmount":           "0",
                "executedSellAmountBeforeFees": "0",
                "executedBuyAmount":            "0",
                "executedFeeAmount":            "0",
                "timestamp":                    "1700000000",
                "txHash":                       "0x",
                "isSignerSafe":                 false,
                "signingScheme":                "eip712",
                "uid":                          "0xababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab"
            }]
        }))))
        .mount(&server)
        .await;

    let orders: Vec<SubgraphOrder> = make_api(&server)
        .get_orders_for_owner("0x1111111111111111111111111111111111111111", 10)
        .await
        .unwrap();
    assert_eq!(orders.len(), 1);
    assert!(orders[0].is_sell());
    assert!(orders[0].is_open());
    assert!(!orders[0].is_filled());
}

#[tokio::test]
async fn subgraph_order_kind_predicates() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "orders": [{
                "id":                           "0xorder",
                "owner":                        user_json(),
                "sellToken":                    token_json(),
                "buyToken":                     token_json(),
                "receiver":                     null,
                "sellAmount":                   "500",
                "buyAmount":                    "400",
                "validTo":                      "9999999",
                "appData":                      "0x00",
                "feeAmount":                    "0",
                "kind":                         "buy",
                "partiallyFillable":            false,
                "status":                       "expired",
                "executedSellAmount":           "0",
                "executedSellAmountBeforeFees": "0",
                "executedBuyAmount":            "0",
                "executedFeeAmount":            "0",
                "timestamp":                    "1700000000",
                "txHash":                       "0x",
                "isSignerSafe":                 false,
                "signingScheme":                "eip712",
                "uid":                          "0xorder"
            }]
        }))))
        .mount(&server)
        .await;

    let orders: Vec<SubgraphOrder> = make_api(&server)
        .get_orders_for_owner("0x1111111111111111111111111111111111111111", 1)
        .await
        .unwrap();
    assert_eq!(orders.len(), 1);
    assert!(orders[0].is_buy());
    assert!(!orders[0].is_sell());
    assert!(orders[0].is_expired());
    assert!(orders[0].is_terminal());
}

// ── get_settlements ───────────────────────────────────────────────────────────

#[tokio::test]
async fn get_settlements_returns_settlement_list() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_data(serde_json::json!({
            "settlements": [{
                "id":                  "0xsettlement",
                "txHash":              "0xsettlement",
                "firstTradeTimestamp": "1700000000",
                "solver":              "0xsolver",
                "txCost":              null,
                "txFeeInEth":          null
            }]
        }))))
        .mount(&server)
        .await;

    let settlements = make_api(&server).get_settlements(1).await.unwrap();
    assert_eq!(settlements.len(), 1);
    assert_eq!(settlements[0].tx_hash, "0xsettlement");
    assert!(!settlements[0].has_gas_cost());
    assert!(!settlements[0].has_tx_fee());
}

// ── GraphQL error propagation ─────────────────────────────────────────────────

#[tokio::test]
async fn graphql_errors_field_returns_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "errors": [{ "message": "field not found" }]
        })))
        .mount(&server)
        .await;

    let result = make_api(&server).get_totals().await;
    match result {
        Err(cow_rs::CowError::Api { status, .. }) => assert_eq!(status, 200),
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn http_500_returns_api_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
        .mount(&server)
        .await;

    let result = make_api(&server).get_totals().await;
    assert!(result.is_err());
}

// ── SubgraphApi constructor ────────────────────────────────────────────────────

#[test]
fn subgraph_api_new_with_url_accepts_any_url() {
    let _api = SubgraphApi::new_with_url("http://localhost:9999/graphql");
}

#[test]
fn subgraph_api_new_unsupported_chain_returns_error() {
    use cow_rs::{Env, SupportedChainId};
    // Polygon has no subgraph endpoint
    let result = SubgraphApi::new(SupportedChainId::Polygon, Env::Prod);
    assert!(result.is_err());
}

#[test]
fn subgraph_api_new_mainnet_succeeds() {
    use cow_rs::{Env, SupportedChainId};
    let result = SubgraphApi::new(SupportedChainId::Mainnet, Env::Prod);
    assert!(result.is_ok());
}
