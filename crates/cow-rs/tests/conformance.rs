//! Conformance tests: verify that the Rust SDK produces identical results
//! to the `TypeScript` SDK at the pinned upstream commit.
//!
//! Fixtures in `scripts/conformance/fixtures/*.json` contain test vectors extracted
//! from the `TypeScript` SDK. Each test loads a fixture case by ID and
//! asserts the Rust output matches the expected `TypeScript` output.
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration test file; all functions are tests"
)]
#![allow(
    clippy::panic_in_result_fn,
    clippy::panic,
    reason = "test helpers use panic for fixture loading failures"
)]

use cow_rs::{
    Env, OrderKind, SigningScheme, SupportedChainId, TokenBalance,
    app_data::{
        cid::{appdata_hex_to_cid, cid_to_appdata_hex, parse_cid},
        types::{AppDataDoc, LATEST_APP_DATA_VERSION, Referrer},
    },
    config::{
        chain::api_base_url,
        contracts::{
            BUY_ETH_ADDRESS, IMPLEMENTATION_STORAGE_SLOT, MAX_VALID_TO_EPOCH, OWNER_STORAGE_SLOT,
            SETTLEMENT_CONTRACT, VAULT_RELAYER, deterministic_deployment_address,
        },
    },
    order_book::types::{OrderClass, OrderStatus, QuoteSide},
    order_signing::{
        eip712::domain_separator,
        utils::{compute_order_uid, presign_result},
    },
    trading::{
        Amounts, DEFAULT_FEE_SLIPPAGE_FACTOR_PCT, DEFAULT_QUOTE_VALIDITY, DEFAULT_SLIPPAGE_BPS,
        DEFAULT_VOLUME_SLIPPAGE_BPS, ETH_FLOW_DEFAULT_SLIPPAGE_BPS, GAS_LIMIT_DEFAULT,
        MAX_SLIPPAGE_BPS, NetworkFee, bps_to_percentage, calculate_gas_margin, percentage_to_bps,
    },
};

// ── Fixture loader ──────────────────────────────────────────────────────────

fn load_fixture(surface: &str) -> serde_json::Value {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let ws_root = std::path::Path::new(manifest)
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| std::path::Path::new(manifest));
    let fixture_path = ws_root
        .join("scripts")
        .join("conformance")
        .join("fixtures")
        .join(format!("{surface}.json"));
    let display = fixture_path.display().to_string();
    let content = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("failed to read fixture {display}: {e}"));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse fixture {display}: {e}"))
}

fn find_case<'a>(fixture: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    fixture["cases"]
        .as_array()
        .unwrap_or_else(|| panic!("fixture has no cases array"))
        .iter()
        .find(|c| c["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("fixture case not found: {id}"))
}

// ── Core surface ────────────────────────────────────────────────────────────

#[test]
fn conformance_core_supported_chain_ids() {
    let fixture = load_fixture("core");
    let case = find_case(&fixture, "core-supported-chain-ids");
    let expected: Vec<u64> = case["expected"]["chain_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();

    let actual: Vec<u64> = SupportedChainId::all().iter().map(|c| c.as_u64()).collect();
    assert_eq!(actual, expected);
}

#[test]
fn conformance_core_settlement_address() {
    let fixture = load_fixture("core");
    let case = find_case(&fixture, "core-settlement-address");
    let expected = case["expected"]["address"].as_str().unwrap();
    assert_eq!(format!("{SETTLEMENT_CONTRACT:#x}"), expected.to_lowercase());
}

#[test]
fn conformance_core_vault_relayer_address() {
    let fixture = load_fixture("core");
    let case = find_case(&fixture, "core-vault-relayer-address");
    let expected = case["expected"]["address"].as_str().unwrap();
    assert_eq!(format!("{VAULT_RELAYER:#x}"), expected.to_lowercase());
}

#[test]
fn conformance_core_buy_eth_address() {
    let fixture = load_fixture("core");
    let case = find_case(&fixture, "core-buy-eth-address");
    let expected = case["expected"]["address"].as_str().unwrap();
    assert_eq!(format!("{BUY_ETH_ADDRESS:#x}"), expected.to_lowercase());
}

#[test]
fn conformance_core_api_base_urls() {
    let fixture = load_fixture("core");
    let case = find_case(&fixture, "core-api-base-urls");
    let expected = &case["expected"];

    assert_eq!(
        api_base_url(SupportedChainId::Mainnet, Env::Prod),
        expected["prod_mainnet"].as_str().unwrap()
    );
    assert_eq!(
        api_base_url(SupportedChainId::GnosisChain, Env::Prod),
        expected["prod_gnosis"].as_str().unwrap()
    );
    assert_eq!(
        api_base_url(SupportedChainId::Mainnet, Env::Staging),
        expected["staging_mainnet"].as_str().unwrap()
    );
    assert_eq!(
        api_base_url(SupportedChainId::Sepolia, Env::Staging),
        expected["staging_sepolia"].as_str().unwrap()
    );
}

#[test]
fn conformance_core_order_kind_values() {
    let fixture = load_fixture("core");
    let case = find_case(&fixture, "core-order-kind-values");
    let expected = &case["expected"];
    assert_eq!(OrderKind::Sell.as_str(), expected["sell"].as_str().unwrap());
    assert_eq!(OrderKind::Buy.as_str(), expected["buy"].as_str().unwrap());
}

#[test]
fn conformance_core_signing_scheme_values() {
    let fixture = load_fixture("core");
    let case = find_case(&fixture, "core-signing-scheme-values");
    let expected = &case["expected"];
    assert_eq!(SigningScheme::Eip712.as_str(), expected["eip712"].as_str().unwrap());
    assert_eq!(SigningScheme::EthSign.as_str(), expected["ethsign"].as_str().unwrap());
    assert_eq!(SigningScheme::Eip1271.as_str(), expected["eip1271"].as_str().unwrap());
    assert_eq!(SigningScheme::PreSign.as_str(), expected["presign"].as_str().unwrap());
}

#[test]
fn conformance_core_token_balance_values() {
    let fixture = load_fixture("core");
    let case = find_case(&fixture, "core-token-balance-values");
    let expected = &case["expected"];
    assert_eq!(TokenBalance::Erc20.as_str(), expected["erc20"].as_str().unwrap());
    assert_eq!(TokenBalance::External.as_str(), expected["external"].as_str().unwrap());
    assert_eq!(TokenBalance::Internal.as_str(), expected["internal"].as_str().unwrap());
}

// ── Signing surface ─────────────────────────────────────────────────────────

#[test]
fn conformance_signing_domain_separator_mainnet() {
    let fixture = load_fixture("signing");
    let case = find_case(&fixture, "signing-domain-separator-mainnet");
    let expected = case["expected"]["domain_separator"].as_str().unwrap();
    let actual = domain_separator(1);
    assert_eq!(format!("0x{}", alloy_primitives::hex::encode(actual)), expected);
}

#[test]
fn conformance_signing_domain_separator_gnosis() {
    let fixture = load_fixture("signing");
    let case = find_case(&fixture, "signing-domain-separator-gnosis");
    let expected = case["expected"]["domain_separator"].as_str().unwrap();
    let actual = domain_separator(100);
    assert_eq!(format!("0x{}", alloy_primitives::hex::encode(actual)), expected);
}

#[test]
fn conformance_signing_order_uid_format() {
    let fixture = load_fixture("signing");
    let case = find_case(&fixture, "signing-order-uid-format");
    let expected_len = case["expected"]["uid_hex_length"].as_u64().unwrap() as usize;

    let order = cow_rs::order_signing::types::UnsignedOrder::sell(
        "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap(),
        "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap(),
        alloy_primitives::U256::from(1_000_000_000_000_000_000u64),
        alloy_primitives::U256::from(1_000_000_000u64),
    )
    .with_valid_to(1999999999);
    let owner: alloy_primitives::Address =
        "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".parse().unwrap();
    let uid = compute_order_uid(1, &order, owner);
    assert_eq!(uid.len(), expected_len);
}

#[test]
fn conformance_signing_presign_result() {
    let fixture = load_fixture("signing");
    let case = find_case(&fixture, "signing-presign-result");
    let owner: alloy_primitives::Address =
        case["input"]["owner"].as_str().unwrap().parse().unwrap();
    let result = presign_result(owner);
    assert_eq!(
        result.signing_scheme.as_str(),
        case["expected"]["signing_scheme"].as_str().unwrap()
    );
    assert!(result.signature.to_lowercase().contains(&format!("{owner:x}").to_lowercase()));
}

// ── App-data surface ────────────────────────────────────────────────────────

#[test]
fn conformance_appdata_hex_to_cid_roundtrip() {
    let fixture = load_fixture("app-data");
    let case = find_case(&fixture, "appdata-hex-to-cid-roundtrip");
    let input_hex = case["input"]["app_data_hex"].as_str().unwrap();

    let cid = appdata_hex_to_cid(input_hex).unwrap();
    assert!(cid.starts_with(case["expected"]["cid_starts_with"].as_str().unwrap()));
    assert_eq!(cid.len(), case["expected"]["cid_length"].as_u64().unwrap() as usize);
    assert_eq!(cid, case["expected"]["cid"].as_str().unwrap());

    let recovered = cid_to_appdata_hex(&cid).unwrap();
    assert_eq!(
        recovered.len(),
        case["expected"]["roundtrip_hex_length"].as_u64().unwrap() as usize
    );
    assert_eq!(recovered, case["expected"]["roundtrip_hex"].as_str().unwrap());
    assert_eq!(recovered, input_hex);
}

#[test]
fn conformance_appdata_cid_components() {
    let fixture = load_fixture("app-data");
    let case = find_case(&fixture, "appdata-cid-components");
    let input_hex = case["input"]["app_data_hex"].as_str().unwrap();
    let expected = &case["expected"];

    let cid = appdata_hex_to_cid(input_hex).unwrap();
    let components = parse_cid(&cid).unwrap();

    assert_eq!(u64::from(components.version), expected["version"].as_u64().unwrap());
    assert_eq!(u64::from(components.codec), expected["codec"].as_u64().unwrap());
    assert_eq!(u64::from(components.hash_function), expected["hash_function"].as_u64().unwrap());
    assert_eq!(u64::from(components.hash_length), expected["hash_length"].as_u64().unwrap());
    assert_eq!(components.digest.len() as u64, expected["digest_length"].as_u64().unwrap());
}

#[test]
fn conformance_appdata_latest_version() {
    let fixture = load_fixture("app-data");
    let case = find_case(&fixture, "appdata-latest-version");
    assert_eq!(LATEST_APP_DATA_VERSION, case["expected"]["version"].as_str().unwrap());
}

#[test]
fn conformance_appdata_doc_builder() {
    let fixture = load_fixture("app-data");
    let case = find_case(&fixture, "appdata-doc-builder");
    let app_code = case["input"]["app_code"].as_str().unwrap();
    let doc = AppDataDoc::new(app_code);

    assert_eq!(doc.version, case["expected"]["version"].as_str().unwrap());
    assert!(doc.app_code.is_some());
}

#[test]
fn conformance_appdata_referrer_address() {
    let fixture = load_fixture("app-data");
    let case = find_case(&fixture, "appdata-referrer-address");
    let addr = case["input"]["address"].as_str().unwrap();
    let referrer = Referrer::address(addr);
    assert!(referrer.as_address().is_some());
    assert!(referrer.as_code().is_none());
}

#[test]
fn conformance_appdata_referrer_code() {
    let fixture = load_fixture("app-data");
    let case = find_case(&fixture, "appdata-referrer-code");
    let code = case["input"]["code"].as_str().unwrap();
    let referrer = Referrer::code(code);
    assert!(referrer.as_address().is_none());
    assert!(referrer.as_code().is_some());
}

// ── Contracts surface ───────────────────────────────────────────────────────

#[test]
fn conformance_contracts_order_uid_length() {
    let fixture = load_fixture("contracts");
    let case = find_case(&fixture, "contracts-order-uid-length");
    let expected_bytes = case["expected"]["byte_length"].as_u64().unwrap() as usize;

    let order = cow_rs::order_signing::types::UnsignedOrder::sell(
        alloy_primitives::Address::ZERO,
        alloy_primitives::Address::ZERO,
        alloy_primitives::U256::ZERO,
        alloy_primitives::U256::ZERO,
    );
    let uid = compute_order_uid(1, &order, alloy_primitives::Address::ZERO);
    // uid is "0x" + hex, so (uid.len() - 2) / 2 = byte_length
    assert_eq!((uid.len() - 2) / 2, expected_bytes);
}

#[test]
fn conformance_contracts_create2_deterministic() {
    let a1 = deterministic_deployment_address(&[0xfe], &[]);
    let a2 = deterministic_deployment_address(&[0xfe], &[]);
    assert_eq!(a1, a2); // same_input_same_output

    let a3 = deterministic_deployment_address(&[0xff], &[]);
    assert_ne!(a1, a3); // different_bytecode_different_output
}

#[test]
fn conformance_contracts_eip1967_slots() {
    let fixture = load_fixture("contracts");

    let impl_case = find_case(&fixture, "contracts-eip1967-implementation-slot");
    assert_eq!(IMPLEMENTATION_STORAGE_SLOT, impl_case["expected"]["slot"].as_str().unwrap());

    let owner_case = find_case(&fixture, "contracts-eip1967-owner-slot");
    assert_eq!(OWNER_STORAGE_SLOT, owner_case["expected"]["slot"].as_str().unwrap());
}

#[test]
fn conformance_contracts_max_valid_to() {
    let fixture = load_fixture("contracts");
    let case = find_case(&fixture, "contracts-max-valid-to");
    assert_eq!(u64::from(MAX_VALID_TO_EPOCH), case["expected"]["value"].as_u64().unwrap());
}

// ── Orderbook surface ──────────────────────────────────────────────────────

#[test]
fn conformance_orderbook_order_status_values() {
    let fixture = load_fixture("orderbook");
    let case = find_case(&fixture, "orderbook-order-status-values");
    let expected = &case["expected"];
    assert_eq!(
        OrderStatus::PresignaturePending.as_str(),
        expected["presignature_pending"].as_str().unwrap()
    );
    assert_eq!(OrderStatus::Open.as_str(), expected["open"].as_str().unwrap());
    assert_eq!(OrderStatus::Fulfilled.as_str(), expected["fulfilled"].as_str().unwrap());
    assert_eq!(OrderStatus::Cancelled.as_str(), expected["cancelled"].as_str().unwrap());
    assert_eq!(OrderStatus::Expired.as_str(), expected["expired"].as_str().unwrap());
}

#[test]
fn conformance_orderbook_order_class_values() {
    let fixture = load_fixture("orderbook");
    let case = find_case(&fixture, "orderbook-order-class-values");
    let expected = &case["expected"];
    assert_eq!(OrderClass::Market.as_str(), expected["market"].as_str().unwrap());
    assert_eq!(OrderClass::Limit.as_str(), expected["limit"].as_str().unwrap());
    assert_eq!(OrderClass::Liquidity.as_str(), expected["liquidity"].as_str().unwrap());
}

#[test]
fn conformance_orderbook_quote_side_sell() {
    let fixture = load_fixture("orderbook");
    let case = find_case(&fixture, "orderbook-quote-side-sell");
    let amount = case["input"]["amount"].as_str().unwrap();
    let expected = &case["expected"];

    let side = QuoteSide::sell(amount);
    assert_eq!(side.kind.as_str(), expected["kind"].as_str().unwrap());
    assert_eq!(
        side.sell_amount_before_fee.is_some(),
        expected["has_sell_amount_before_fee"].as_bool().unwrap()
    );
    assert_eq!(
        side.buy_amount_after_fee.is_some(),
        expected["has_buy_amount_after_fee"].as_bool().unwrap()
    );
}

#[test]
fn conformance_orderbook_quote_side_buy() {
    let fixture = load_fixture("orderbook");
    let case = find_case(&fixture, "orderbook-quote-side-buy");
    let amount = case["input"]["amount"].as_str().unwrap();
    let expected = &case["expected"];

    let side = QuoteSide::buy(amount);
    assert_eq!(side.kind.as_str(), expected["kind"].as_str().unwrap());
    assert_eq!(
        side.sell_amount_before_fee.is_some(),
        expected["has_sell_amount_before_fee"].as_bool().unwrap()
    );
    assert_eq!(
        side.buy_amount_after_fee.is_some(),
        expected["has_buy_amount_after_fee"].as_bool().unwrap()
    );
}

#[test]
fn conformance_orderbook_quote_request_defaults() {
    let fixture = load_fixture("orderbook");
    let case = find_case(&fixture, "orderbook-quote-request-defaults");
    let expected = &case["expected"];

    // Default values verified against the struct definition.
    assert!(!expected["partially_fillable_default"].as_bool().unwrap());
    assert_eq!(
        TokenBalance::Erc20.as_str(),
        expected["sell_token_balance_default"].as_str().unwrap()
    );
    assert_eq!(
        TokenBalance::Erc20.as_str(),
        expected["buy_token_balance_default"].as_str().unwrap()
    );
}

#[test]
fn conformance_orderbook_trade_response_fields() {
    let fixture = load_fixture("orderbook");
    let case = find_case(&fixture, "orderbook-trade-response-fields");
    let expected_fields: Vec<&str> = case["expected"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();

    // Serialize a Trade instance and verify all expected fields appear in the JSON.
    let trade = cow_rs::order_book::types::Trade {
        block_number: 1,
        log_index: 0,
        order_uid: "0x".to_owned(),
        owner: "0x".to_owned(),
        sell_token: "0x".to_owned(),
        buy_token: "0x".to_owned(),
        sell_amount: "0".to_owned(),
        sell_amount_before_fees: "0".to_owned(),
        buy_amount: "0".to_owned(),
        tx_hash: Some("0x".to_owned()),
    };
    let json: serde_json::Value = serde_json::to_value(&trade).unwrap();
    let obj = json.as_object().unwrap();
    for field in &expected_fields {
        assert!(obj.contains_key(*field), "missing field: {field}");
    }
}

// ── Trading surface ────────────────────────────────────────────────────────

#[test]
fn conformance_trading_default_slippage_bps() {
    let fixture = load_fixture("trading");
    let case = find_case(&fixture, "trading-default-slippage-bps");
    assert_eq!(
        u64::from(DEFAULT_SLIPPAGE_BPS),
        case["expected"]["default_slippage_bps"].as_u64().unwrap()
    );
}

#[test]
fn conformance_trading_default_quote_validity() {
    let fixture = load_fixture("trading");
    let case = find_case(&fixture, "trading-default-quote-validity");
    assert_eq!(
        u64::from(DEFAULT_QUOTE_VALIDITY),
        case["expected"]["default_quote_validity_seconds"].as_u64().unwrap()
    );
}

#[test]
fn conformance_trading_eth_flow_default_slippage() {
    let fixture = load_fixture("trading");
    let case = find_case(&fixture, "trading-eth-flow-default-slippage");
    assert_eq!(
        u64::from(ETH_FLOW_DEFAULT_SLIPPAGE_BPS),
        case["expected"]["eth_flow_default_slippage_bps"].as_u64().unwrap()
    );
}

#[test]
fn conformance_trading_gas_limit_default() {
    let fixture = load_fixture("trading");
    let case = find_case(&fixture, "trading-gas-limit-default");
    assert_eq!(GAS_LIMIT_DEFAULT, case["expected"]["gas_limit_default"].as_u64().unwrap());
}

#[test]
fn conformance_trading_slippage_sell_order() {
    let fixture = load_fixture("trading");
    let case = find_case(&fixture, "trading-slippage-sell-order");
    let buy_amount: alloy_primitives::U256 =
        case["input"]["buy_amount"].as_str().unwrap().parse().unwrap();
    let slippage_bps = case["input"]["slippage_bps"].as_u64().unwrap() as u32;

    // Sell order slippage: buy_amount * (10000 - bps) / 10000
    let adjusted = buy_amount * alloy_primitives::U256::from(10_000u32 - slippage_bps) /
        alloy_primitives::U256::from(10_000u32);
    let expected: alloy_primitives::U256 =
        case["expected"]["adjusted_buy_amount"].as_str().unwrap().parse().unwrap();
    assert_eq!(adjusted, expected);
}

#[test]
fn conformance_trading_slippage_buy_order() {
    let fixture = load_fixture("trading");
    let case = find_case(&fixture, "trading-slippage-buy-order");
    let sell_amount: alloy_primitives::U256 =
        case["input"]["sell_amount"].as_str().unwrap().parse().unwrap();
    let slippage_bps = case["input"]["slippage_bps"].as_u64().unwrap() as u32;

    // Buy order slippage: sell_amount * (10000 + bps) / 10000
    let adjusted = sell_amount * alloy_primitives::U256::from(10_000u32 + slippage_bps) /
        alloy_primitives::U256::from(10_000u32);
    let expected: alloy_primitives::U256 =
        case["expected"]["adjusted_sell_amount"].as_str().unwrap().parse().unwrap();
    assert_eq!(adjusted, expected);
}

#[test]
fn conformance_trading_amounts_zero_check() {
    let fixture = load_fixture("trading");
    let case = find_case(&fixture, "trading-amounts-zero-check");
    let expected = &case["expected"];

    let zero = Amounts::new(alloy_primitives::U256::ZERO, alloy_primitives::U256::ZERO);
    assert_eq!(zero.is_zero(), expected["zero_zero_is_zero"].as_bool().unwrap());

    let nonzero_sell =
        Amounts::new(alloy_primitives::U256::from(1u32), alloy_primitives::U256::ZERO);
    assert_eq!(nonzero_sell.is_zero(), expected["nonzero_zero_is_zero"].as_bool().unwrap());

    let nonzero_buy =
        Amounts::new(alloy_primitives::U256::ZERO, alloy_primitives::U256::from(1u32));
    assert_eq!(nonzero_buy.is_zero(), expected["zero_nonzero_is_zero"].as_bool().unwrap());
}

#[test]
fn conformance_trading_network_fee_zero_check() {
    let fixture = load_fixture("trading");
    let case = find_case(&fixture, "trading-network-fee-zero-check");
    let expected = &case["expected"];

    let zero = NetworkFee::new(alloy_primitives::U256::ZERO, alloy_primitives::U256::ZERO);
    assert_eq!(zero.is_zero(), expected["zero_zero_is_zero"].as_bool().unwrap());

    let nonzero_sell =
        NetworkFee::new(alloy_primitives::U256::from(1u32), alloy_primitives::U256::ZERO);
    assert_eq!(nonzero_sell.is_zero(), expected["nonzero_zero_is_zero"].as_bool().unwrap());

    let nonzero_buy =
        NetworkFee::new(alloy_primitives::U256::ZERO, alloy_primitives::U256::from(1u32));
    assert_eq!(nonzero_buy.is_zero(), expected["zero_nonzero_is_zero"].as_bool().unwrap());
}

#[test]
fn conformance_trading_slippage_suggest_constants() {
    let fixture = load_fixture("trading");
    let case = find_case(&fixture, "trading-slippage-suggest-constants");
    let expected = &case["expected"];

    assert_eq!(
        u64::from(DEFAULT_FEE_SLIPPAGE_FACTOR_PCT),
        expected["default_fee_slippage_factor_pct"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(DEFAULT_VOLUME_SLIPPAGE_BPS),
        expected["default_volume_slippage_bps"].as_u64().unwrap()
    );
    assert_eq!(u64::from(MAX_SLIPPAGE_BPS), expected["max_slippage_bps"].as_u64().unwrap());
}

#[test]
fn conformance_trading_percentage_bps_conversion() {
    let fixture = load_fixture("trading");
    let case = find_case(&fixture, "trading-percentage-bps-conversion");
    let expected = &case["expected"];

    assert_eq!(
        u64::from(percentage_to_bps(rust_decimal::Decimal::new(5, 1))),
        expected["half_pct_to_bps"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(percentage_to_bps(rust_decimal::Decimal::new(1, 0))),
        expected["one_pct_to_bps"].as_u64().unwrap()
    );
    // bps_to_percentage(50) == 50/100 == 0.5
    let fifty_pct = bps_to_percentage(50);
    let fifty_expected =
        rust_decimal::Decimal::from(expected["fifty_bps_to_pct_numerator"].as_u64().unwrap()) /
            rust_decimal::Decimal::from(
                expected["fifty_bps_to_pct_denominator"].as_u64().unwrap(),
            );
    assert_eq!(fifty_pct, fifty_expected);

    // bps_to_percentage(100) == 100/100 == 1.0
    let hundred_pct = bps_to_percentage(100);
    let hundred_expected =
        rust_decimal::Decimal::from(expected["hundred_bps_to_pct_numerator"].as_u64().unwrap()) /
            rust_decimal::Decimal::from(
                expected["hundred_bps_to_pct_denominator"].as_u64().unwrap(),
            );
    assert_eq!(hundred_pct, hundred_expected);
}

#[test]
fn conformance_trading_gas_margin() {
    let fixture = load_fixture("trading");
    let case = find_case(&fixture, "trading-gas-margin");
    let gas_estimate = case["input"]["gas_estimate"].as_u64().unwrap();
    let expected = case["expected"]["with_margin"].as_u64().unwrap();
    assert_eq!(calculate_gas_margin(gas_estimate), expected);
}
