//! Conformance tests: verify that the Rust SDK produces identical results
//! to the TypeScript SDK at the pinned upstream commit.
//!
//! Fixtures in `conformance/fixtures/*.json` contain test vectors extracted
//! from the TypeScript SDK. Each test loads a fixture case by ID and
//! asserts the Rust output matches the expected TypeScript output.

use cow_rs::{
    Env, OrderKind, SigningScheme, SupportedChainId, TokenBalance,
    app_data::{
        cid::{appdata_hex_to_cid, cid_to_appdata_hex, parse_cid},
        hash::appdata_hex,
        types::{AppDataDoc, LATEST_APP_DATA_VERSION, Referrer},
    },
    config::{
        chain::{api_base_url, order_explorer_link},
        contracts::{
            BUY_ETH_ADDRESS, IMPLEMENTATION_STORAGE_SLOT, MAX_VALID_TO_EPOCH,
            OWNER_STORAGE_SLOT, SETTLEMENT_CONTRACT, VAULT_RELAYER,
            deterministic_deployment_address, settlement_contract,
        },
    },
    order_signing::{
        eip712::{domain_separator, order_hash, signing_digest},
        utils::{compute_order_uid, presign_result},
    },
};

// ── Fixture loader ──────────────────────────────────────────────────────────

fn load_fixture(surface: &str) -> serde_json::Value {
    let path = format!(
        "{}/conformance/fixtures/{surface}.json",
        env!("CARGO_MANIFEST_DIR").trim_end_matches("/crates/cow-rs")
    );
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {path}: {e}"));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse fixture {path}: {e}"))
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
    assert_eq!(format!("{:#x}", SETTLEMENT_CONTRACT), expected.to_lowercase());
}

#[test]
fn conformance_core_vault_relayer_address() {
    let fixture = load_fixture("core");
    let case = find_case(&fixture, "core-vault-relayer-address");
    let expected = case["expected"]["address"].as_str().unwrap();
    assert_eq!(format!("{:#x}", VAULT_RELAYER), expected.to_lowercase());
}

#[test]
fn conformance_core_buy_eth_address() {
    let fixture = load_fixture("core");
    let case = find_case(&fixture, "core-buy-eth-address");
    let expected = case["expected"]["address"].as_str().unwrap();
    assert_eq!(format!("{:#x}", BUY_ETH_ADDRESS), expected.to_lowercase());
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
    ).with_valid_to(1999999999);
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
    assert_eq!(result.signing_scheme.as_str(), case["expected"]["signing_scheme"].as_str().unwrap());
    assert!(result.signature.to_lowercase().contains(&format!("{:x}", owner).to_lowercase()));
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

    let recovered = cid_to_appdata_hex(&cid).unwrap();
    assert_eq!(recovered.len(), case["expected"]["roundtrip_hex_length"].as_u64().unwrap() as usize);
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
    assert_eq!(
        u64::from(MAX_VALID_TO_EPOCH),
        case["expected"]["value"].as_u64().unwrap()
    );
}
