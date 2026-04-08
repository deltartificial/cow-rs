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
//! Wiremock-based integration tests for [`OnchainReader`] JSON-RPC methods.
//!
//! Each test starts a local `wiremock` server, configures it to return a
//! deterministic JSON-RPC response, and then verifies that the correct value is
//! decoded by the reader.
//!
//! JSON-RPC response format:
//! `{"jsonrpc":"2.0","id":1,"result":"0x..."}`

use alloy_primitives::{Address, U256, address, keccak256};
use cow_rs::OnchainReader;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

// ── Helper builders ───────────────────────────────────────────────────────────

/// Build an ABI-encoded `uint256` result hex string.
fn u256_result(val: u64) -> String {
    let bytes = U256::from(val).to_be_bytes::<32>();
    format!("0x{}", alloy_primitives::hex::encode(bytes))
}

/// Build an ABI-encoded `uint8` result hex string (32 bytes, value in last byte).
fn u8_result(val: u8) -> String {
    let mut bytes = [0u8; 32];
    bytes[31] = val;
    format!("0x{}", alloy_primitives::hex::encode(bytes))
}

/// Build an ABI-encoded dynamic `string` result hex string.
fn string_result(s: &str) -> String {
    let padded_len = s.len().div_ceil(32) * 32;
    let total = 64 + padded_len;
    let mut buf = vec![0u8; total];
    // offset word: 0x20
    buf[31] = 32;
    // length word
    let len = s.len();
    buf[32 + 28] = (len >> 24) as u8;
    buf[32 + 29] = (len >> 16) as u8;
    buf[32 + 30] = (len >> 8) as u8;
    buf[32 + 31] = len as u8;
    // UTF-8 data
    buf[64..64 + len].copy_from_slice(s.as_bytes());
    format!("0x{}", alloy_primitives::hex::encode(&buf))
}

/// Wrap a hex result into a JSON-RPC 2.0 success response body.
fn jsonrpc_ok(result: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id":      1,
        "result":  result
    })
}

fn make_reader(server: &MockServer) -> OnchainReader {
    OnchainReader::new(server.uri())
}

const fn token() -> Address {
    address!("fFf9976782d46CC05630D1f6eBAb18b2324d6B14")
}

const fn owner() -> Address {
    address!("1111111111111111111111111111111111111111")
}

const fn spender() -> Address {
    address!("2222222222222222222222222222222222222222")
}

// ── erc20_balance ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn erc20_balance_returns_parsed_u256() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(1_000_000))))
        .mount(&server)
        .await;

    let reader = make_reader(&server);
    let bal = reader.erc20_balance(token(), owner()).await.unwrap();
    assert_eq!(bal, U256::from(1_000_000u64));
}

#[tokio::test]
async fn erc20_balance_zero_returns_zero() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(0))))
        .mount(&server)
        .await;

    let bal = make_reader(&server).erc20_balance(token(), owner()).await.unwrap();
    assert_eq!(bal, U256::ZERO);
}

#[tokio::test]
async fn erc20_balance_large_value() {
    let server = MockServer::start().await;
    let large: u64 = u64::MAX;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(large))))
        .mount(&server)
        .await;

    let bal = make_reader(&server).erc20_balance(token(), owner()).await.unwrap();
    assert_eq!(bal, U256::from(large));
}

// ── erc20_allowance ───────────────────────────────────────────────────────────

#[tokio::test]
async fn erc20_allowance_returns_parsed_u256() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(500_000))))
        .mount(&server)
        .await;

    let allowance =
        make_reader(&server).erc20_allowance(token(), owner(), spender()).await.unwrap();
    assert_eq!(allowance, U256::from(500_000u64));
}

#[tokio::test]
async fn erc20_allowance_zero_means_no_approval() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(0))))
        .mount(&server)
        .await;

    let allowance =
        make_reader(&server).erc20_allowance(token(), owner(), spender()).await.unwrap();
    assert!(allowance.is_zero());
}

// ── erc20_decimals ────────────────────────────────────────────────────────────

#[tokio::test]
async fn erc20_decimals_returns_18() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u8_result(18))))
        .mount(&server)
        .await;

    let dec = make_reader(&server).erc20_decimals(token()).await.unwrap();
    assert_eq!(dec, 18u8);
}

#[tokio::test]
async fn erc20_decimals_returns_6() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u8_result(6))))
        .mount(&server)
        .await;

    let dec = make_reader(&server).erc20_decimals(token()).await.unwrap();
    assert_eq!(dec, 6u8);
}

// ── erc20_name ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn erc20_name_returns_wrapped_ether() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&string_result("Wrapped Ether"))),
        )
        .mount(&server)
        .await;

    let name = make_reader(&server).erc20_name(token()).await.unwrap();
    assert_eq!(name, "Wrapped Ether");
}

#[tokio::test]
async fn erc20_name_returns_usd_coin() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&string_result("USD Coin"))),
        )
        .mount(&server)
        .await;

    let name = make_reader(&server).erc20_name(token()).await.unwrap();
    assert_eq!(name, "USD Coin");
}

// ── eip2612_nonce ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn eip2612_nonce_returns_zero_for_new_account() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(0))))
        .mount(&server)
        .await;

    let nonce = make_reader(&server).eip2612_nonce(token(), owner()).await.unwrap();
    assert_eq!(nonce, U256::ZERO);
}

#[tokio::test]
async fn eip2612_nonce_increments_after_permit() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&u256_result(3))))
        .mount(&server)
        .await;

    let nonce = make_reader(&server).eip2612_nonce(token(), owner()).await.unwrap();
    assert_eq!(nonce, U256::from(3u64));
}

// ── eip2612_version ───────────────────────────────────────────────────────────

#[tokio::test]
async fn eip2612_version_returns_one() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&string_result("1"))))
        .mount(&server)
        .await;

    let version = make_reader(&server).eip2612_version(token()).await.unwrap();
    assert_eq!(version, "1");
}

#[tokio::test]
async fn eip2612_version_returns_two() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jsonrpc_ok(&string_result("2"))))
        .mount(&server)
        .await;

    let version = make_reader(&server).eip2612_version(token()).await.unwrap();
    assert_eq!(version, "2");
}

// ── JSON-RPC error propagation ─────────────────────────────────────────────────

#[tokio::test]
async fn rpc_error_in_response_is_propagated() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": { "code": -32000, "message": "execution reverted" }
        })))
        .mount(&server)
        .await;

    let result = make_reader(&server).erc20_balance(token(), owner()).await;
    match result {
        Err(cow_rs::CowError::Rpc { code, .. }) => assert_eq!(code, -32000),
        other => panic!("expected Rpc error, got {other:?}"),
    }
}

#[tokio::test]
async fn http_500_returns_rpc_error() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
        .mount(&server)
        .await;

    let result = make_reader(&server).erc20_balance(token(), owner()).await;
    assert!(result.is_err());
}

// ── Calldata selector verification ────────────────────────────────────────────

#[test]
fn balance_of_selector_matches_keccak256() {
    use cow_rs::build_erc20_balance_of_calldata;
    let cd = build_erc20_balance_of_calldata(owner());
    let expected = &keccak256(b"balanceOf(address)")[..4];
    assert_eq!(&cd[..4], expected);
}

#[test]
fn allowance_selector_matches_keccak256() {
    use cow_rs::build_erc20_allowance_calldata;
    let cd = build_erc20_allowance_calldata(owner(), spender());
    let expected = &keccak256(b"allowance(address,address)")[..4];
    assert_eq!(&cd[..4], expected);
}

#[test]
fn decimals_selector_matches_keccak256() {
    use cow_rs::build_erc20_decimals_calldata;
    let cd = build_erc20_decimals_calldata();
    let expected = &keccak256(b"decimals()")[..4];
    assert_eq!(&*cd, expected);
}

#[test]
fn name_selector_matches_keccak256() {
    use cow_rs::build_erc20_name_calldata;
    let cd = build_erc20_name_calldata();
    let expected = &keccak256(b"name()")[..4];
    assert_eq!(&*cd, expected);
}
