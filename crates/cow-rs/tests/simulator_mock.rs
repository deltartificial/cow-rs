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
//! Wiremock-based integration tests for [`TradeSimulator`] and [`SettlementReader`]
//! async methods.

use alloy_primitives::{U256, address};
use cow_rs::{
    SupportedChainId,
    settlement::{
        reader::{AllowListReader, SettlementReader},
        simulator::TradeSimulator,
    },
};
use serde_json::json;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

// ── Helpers ────────────────────────────────────────────────────────────────

/// Encode a u64 as a 0x-prefixed hex string.
fn hex_u64(val: u64) -> String {
    format!("0x{val:x}")
}

/// Build a JSON-RPC 2.0 success response.
fn jsonrpc_ok(result: &str) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": result
    })
}

/// Build a JSON-RPC 2.0 error response.
fn jsonrpc_err(code: i64, message: &str) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": { "code": code, "message": message }
    })
}

/// Build an ABI-encoded uint256 result hex string.
fn u256_result(val: u64) -> String {
    let bytes = U256::from(val).to_be_bytes::<32>();
    format!("0x{}", alloy_primitives::hex::encode(bytes))
}

// ══════════════════════════════════════════════════════════════════════════════
// TradeSimulator tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn estimate_gas_success() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&hex_u64(150_000))))
        .mount(&server)
        .await;

    let sim = TradeSimulator::new(server.uri(), SupportedChainId::Mainnet);
    let gas = sim.estimate_gas(&[0xde, 0xad, 0xbe, 0xef]).await.unwrap();
    assert_eq!(gas, 150_000);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn estimate_gas_rpc_error_in_response() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jsonrpc_err(-32000, "execution reverted")),
        )
        .mount(&server)
        .await;

    let sim = TradeSimulator::new(server.uri(), SupportedChainId::Mainnet);
    let result = sim.estimate_gas(&[0xab]).await;
    assert!(result.is_err());
    match result {
        Err(cow_rs::CowError::Rpc { code, message }) => {
            assert_eq!(code, -32000);
            assert!(message.contains("reverted"));
        }
        other => panic!("expected Rpc error, got {other:?}"),
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn estimate_gas_http_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
        .mount(&server)
        .await;

    let sim = TradeSimulator::new(server.uri(), SupportedChainId::Mainnet);
    let result = sim.estimate_gas(&[0x01]).await;
    assert!(result.is_err());
    match result {
        Err(cow_rs::CowError::Rpc { code, .. }) => {
            assert_eq!(code, 500);
        }
        other => panic!("expected Rpc error, got {other:?}"),
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn estimate_gas_missing_result_field() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": 1
        })))
        .mount(&server)
        .await;

    let sim = TradeSimulator::new(server.uri(), SupportedChainId::Mainnet);
    let result = sim.estimate_gas(&[0x01]).await;
    assert!(result.is_err());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn estimate_gas_invalid_hex_result() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok("0xZZZZ")))
        .mount(&server)
        .await;

    let sim = TradeSimulator::new(server.uri(), SupportedChainId::Mainnet);
    let result = sim.estimate_gas(&[0x01]).await;
    assert!(result.is_err());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn simulate_success() {
    let server = MockServer::start().await;
    // First call: eth_call returns hex result; second call: eth_estimateGas returns gas.
    // Both are POST to the same URL so we use expect() to count them.
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok("0xdeadbeef")))
        .expect(2)
        .mount(&server)
        .await;

    let sim = TradeSimulator::new(server.uri(), SupportedChainId::Mainnet);
    let result = sim.simulate(&[0xab, 0xcd]).await.unwrap();
    assert!(result.is_success());
    assert!(!result.is_revert());
    assert_eq!(result.return_data, vec![0xde, 0xad, 0xbe, 0xef]);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn simulate_revert_via_rpc_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jsonrpc_err(-32000, "execution reverted")),
        )
        .mount(&server)
        .await;

    let sim = TradeSimulator::new(server.uri(), SupportedChainId::Mainnet);
    let result = sim.simulate(&[0xab]).await.unwrap();
    assert!(result.is_revert());
    assert!(!result.is_success());
    assert_eq!(result.gas_used, 0);
    assert!(!result.return_data.is_empty());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn simulate_http_error_propagates() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(502).set_body_string("bad gateway"))
        .mount(&server)
        .await;

    let sim = TradeSimulator::new(server.uri(), SupportedChainId::Mainnet);
    let result = sim.simulate(&[0x01]).await;
    assert!(result.is_err());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn simulate_missing_result_field() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": 1
        })))
        .mount(&server)
        .await;

    let sim = TradeSimulator::new(server.uri(), SupportedChainId::Mainnet);
    let result = sim.simulate(&[0x01]).await;
    assert!(result.is_err());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn estimate_settlement_delegates_to_estimate_gas() {
    use cow_rs::settlement::encoder::SettlementEncoder;

    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&hex_u64(200_000))))
        .mount(&server)
        .await;

    let sim = TradeSimulator::new(server.uri(), SupportedChainId::Mainnet);
    let encoder = SettlementEncoder::new();
    let gas = sim.estimate_settlement(&encoder).await.unwrap();
    assert_eq!(gas, 200_000);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn estimate_gas_empty_calldata() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&hex_u64(21_000))))
        .mount(&server)
        .await;

    let sim = TradeSimulator::new(server.uri(), SupportedChainId::Mainnet);
    let gas = sim.estimate_gas(&[]).await.unwrap();
    assert_eq!(gas, 21_000);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn simulate_with_empty_return_data() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok("0x")))
        .expect(2)
        .mount(&server)
        .await;

    let sim = TradeSimulator::new(server.uri(), SupportedChainId::Mainnet);
    let result = sim.simulate(&[0x01]).await.unwrap();
    assert!(result.is_success());
    assert!(result.return_data.is_empty());
}

// ══════════════════════════════════════════════════════════════════════════════
// SettlementReader tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn settlement_reader_filled_amount_returns_value() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(42_000))))
        .mount(&server)
        .await;

    let reader = SettlementReader::new(server.uri(), SupportedChainId::Mainnet);
    let uid_hex = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    let amount = reader.filled_amount(uid_hex).await.unwrap();
    assert_eq!(amount, U256::from(42_000u64));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn settlement_reader_filled_amount_zero() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(0))))
        .mount(&server)
        .await;

    let reader = SettlementReader::new(server.uri(), SupportedChainId::Mainnet);
    let uid_hex = "0xabcd";
    let amount = reader.filled_amount(uid_hex).await.unwrap();
    assert_eq!(amount, U256::ZERO);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn settlement_reader_filled_amount_rpc_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jsonrpc_err(-32000, "execution reverted")),
        )
        .mount(&server)
        .await;

    let reader = SettlementReader::new(server.uri(), SupportedChainId::Mainnet);
    let result = reader.filled_amount("0xdeadbeef").await;
    assert!(result.is_err());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn settlement_reader_filled_amount_invalid_uid() {
    let reader = SettlementReader::new("http://localhost:1", SupportedChainId::Mainnet);
    let result = reader.filled_amount("not_hex_gg").await;
    assert!(result.is_err());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn settlement_reader_pre_signature_true() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(1))))
        .mount(&server)
        .await;

    let reader = SettlementReader::new(server.uri(), SupportedChainId::Mainnet);
    let signed = reader.pre_signature("0xdeadbeef").await.unwrap();
    assert!(signed);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn settlement_reader_pre_signature_false() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(0))))
        .mount(&server)
        .await;

    let reader = SettlementReader::new(server.uri(), SupportedChainId::Mainnet);
    let signed = reader.pre_signature("0xdeadbeef").await.unwrap();
    assert!(!signed);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn settlement_reader_domain_separator_returns_b256() {
    let server = MockServer::start().await;
    let mut sep_bytes = [0u8; 32];
    sep_bytes[0] = 0xAA;
    sep_bytes[31] = 0xBB;
    let hex = format!("0x{}", alloy_primitives::hex::encode(sep_bytes));
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&hex)))
        .mount(&server)
        .await;

    let reader = SettlementReader::new(server.uri(), SupportedChainId::Mainnet);
    let sep = reader.domain_separator().await.unwrap();
    assert_eq!(sep[0], 0xAA);
    assert_eq!(sep[31], 0xBB);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn settlement_reader_domain_separator_too_short() {
    let server = MockServer::start().await;
    let short_hex = format!("0x{}", alloy_primitives::hex::encode([0u8; 16]));
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&short_hex)))
        .mount(&server)
        .await;

    let reader = SettlementReader::new(server.uri(), SupportedChainId::Mainnet);
    let result = reader.domain_separator().await;
    assert!(result.is_err());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn settlement_reader_domain_separator_http_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(503).set_body_string("unavailable"))
        .mount(&server)
        .await;

    let reader = SettlementReader::new(server.uri(), SupportedChainId::Mainnet);
    let result = reader.domain_separator().await;
    assert!(result.is_err());
}

// ── AllowListReader tests ──────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn allow_list_reader_is_solver_true() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(1))))
        .mount(&server)
        .await;

    let allow_list = address!("2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE");
    let reader = AllowListReader::new(server.uri(), allow_list);
    let solver = address!("1111111111111111111111111111111111111111");
    let is_solver = reader.is_solver(solver).await.unwrap();
    assert!(is_solver);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn allow_list_reader_is_solver_false() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(0))))
        .mount(&server)
        .await;

    let allow_list = address!("2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE");
    let reader = AllowListReader::new(server.uri(), allow_list);
    let solver = address!("1111111111111111111111111111111111111111");
    let is_solver = reader.is_solver(solver).await.unwrap();
    assert!(!is_solver);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn allow_list_reader_is_solver_rpc_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jsonrpc_err(-32000, "execution reverted")),
        )
        .mount(&server)
        .await;

    let allow_list = address!("2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE");
    let reader = AllowListReader::new(server.uri(), allow_list);
    let solver = address!("1111111111111111111111111111111111111111");
    let result = reader.is_solver(solver).await;
    assert!(result.is_err());
}

// ── SettlementReader with custom address ───────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn settlement_reader_with_custom_address_filled_amount() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(99_999))))
        .mount(&server)
        .await;

    let custom = address!("1111111111111111111111111111111111111111");
    let reader = SettlementReader::with_address(server.uri(), custom);
    assert_eq!(reader.settlement_address(), custom);

    let amount = reader.filled_amount("0xabcd").await.unwrap();
    assert_eq!(amount, U256::from(99_999u64));
}
