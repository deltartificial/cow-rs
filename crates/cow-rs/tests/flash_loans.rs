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
//! Tests for the flash loans module.

use alloy_primitives::{Address, U256, address};
use cow_rs::flash_loans::{FlashLoanParams, FlashLoanProvider, FlashLoanSdk};

// Known Balancer vault address (mainnet and Gnosis Chain).
const BALANCER_VAULT: Address = address!("BA12222222228d8Ba445958a75a0704d566BF2C8");
const AAVE_V3_MAINNET: Address = address!("87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2");

// ── FlashLoanProvider::contract_address ───────────────────────────────────────

#[test]
fn flash_loan_provider_balancer_mainnet_address_is_correct() {
    let addr = FlashLoanProvider::Balancer.contract_address(1);
    assert_eq!(addr, Some(BALANCER_VAULT));
}

#[test]
fn flash_loan_provider_balancer_gnosis_address_is_correct() {
    let addr = FlashLoanProvider::Balancer.contract_address(100);
    assert_eq!(addr, Some(BALANCER_VAULT));
}

#[test]
fn flash_loan_provider_aave_mainnet_address_is_correct() {
    let addr = FlashLoanProvider::AaveV3.contract_address(1);
    assert_eq!(addr, Some(AAVE_V3_MAINNET));
}

#[test]
fn flash_loan_provider_maker_dao_unknown_chain() {
    let addr = FlashLoanProvider::MakerDao.contract_address(1);
    assert!(addr.is_none(), "MakerDAO has no registered address on mainnet");
}

// ── FlashLoanParams helpers ───────────────────────────────────────────────────

#[test]
fn flash_loan_params_is_provider_supported_true() {
    let params = FlashLoanParams::new(FlashLoanProvider::Balancer, Address::ZERO, U256::ZERO, 1);
    assert!(params.is_provider_supported());
}

#[test]
fn flash_loan_params_is_provider_supported_false() {
    let params = FlashLoanParams::new(FlashLoanProvider::MakerDao, Address::ZERO, U256::ZERO, 999);
    assert!(!params.is_provider_supported());
}

#[test]
fn flash_loan_provider_name_balancer() {
    assert_eq!(FlashLoanProvider::Balancer.name(), "Balancer");
}

#[test]
fn flash_loan_provider_name_aave() {
    assert_eq!(FlashLoanProvider::AaveV3.name(), "Aave V3");
}

// ── FlashLoanSdk::provider_address ────────────────────────────────────────────

#[test]
fn flash_loan_sdk_provider_address_returns_none_unknown() {
    let addr = FlashLoanSdk::provider_address(FlashLoanProvider::MakerDao, 999);
    assert!(addr.is_none());
}

#[test]
fn flash_loan_sdk_provider_address_returns_balancer_mainnet() {
    let addr = FlashLoanSdk::provider_address(FlashLoanProvider::Balancer, 1);
    assert_eq!(addr, Some(BALANCER_VAULT));
}

// ── FlashLoanSdk::build_flash_loan_hook ───────────────────────────────────────

#[test]
fn build_flash_loan_hook_unsupported_provider_returns_error() {
    let params = FlashLoanParams::new(FlashLoanProvider::MakerDao, Address::ZERO, U256::ZERO, 1);
    let result = FlashLoanSdk::build_flash_loan_hook(&params, Address::ZERO, &[]);
    assert!(result.is_err());
}

#[test]
fn build_flash_loan_hook_balancer_mainnet_succeeds() {
    let token = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let params = FlashLoanParams::new(FlashLoanProvider::Balancer, token, U256::from(1_000_u64), 1);
    let result = FlashLoanSdk::build_flash_loan_hook(&params, Address::ZERO, &[]);
    assert!(result.is_ok());
}

// ── encode_balancer_flash_loan ────────────────────────────────────────────────

#[test]
fn encode_balancer_flash_loan_selector_correct() {
    use alloy_primitives::keccak256;
    let cd =
        FlashLoanSdk::encode_balancer_flash_loan(Address::ZERO, Address::ZERO, U256::ZERO, &[]);
    let h = keccak256(b"flashLoan(address,address[],uint256[],bytes)");
    assert_eq!(&cd[..4], &[h[0], h[1], h[2], h[3]]);
}

#[test]
fn encode_balancer_flash_loan_receiver_encoded() {
    let receiver = address!("1111111111111111111111111111111111111111");
    let cd = FlashLoanSdk::encode_balancer_flash_loan(receiver, Address::ZERO, U256::ZERO, &[]);
    // receiver is in word 0 of the head (bytes 4..36), last 20 bytes
    assert_eq!(&cd[16..36], receiver.as_slice());
}

// ── FlashLoanProvider Aave V3 Gnosis Chain ──────────────────────────────────

#[test]
fn flash_loan_provider_aave_gnosis_address() {
    let addr = FlashLoanProvider::AaveV3.contract_address(100);
    assert!(addr.is_some());
    assert_ne!(addr.unwrap(), AAVE_V3_MAINNET);
}

// ── FlashLoanProvider display name ──────────────────────────────────────────

#[test]
fn flash_loan_provider_name_makerdao() {
    assert_eq!(FlashLoanProvider::MakerDao.name(), "MakerDAO");
}

// ── FlashLoanSdk::build_flash_loan_hook with AaveV3 ─────────────────────────

#[test]
fn build_flash_loan_hook_aave_returns_unsupported_error() {
    let params = FlashLoanParams::new(
        FlashLoanProvider::AaveV3,
        address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
        U256::from(1_000_u64),
        1,
    );
    let result = FlashLoanSdk::build_flash_loan_hook(&params, Address::ZERO, &[]);
    assert!(result.is_err());
}

#[test]
fn build_flash_loan_hook_makerdao_returns_unsupported_error() {
    let params = FlashLoanParams::new(
        FlashLoanProvider::MakerDao,
        address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
        U256::from(1_000_u64),
        1,
    );
    let result = FlashLoanSdk::build_flash_loan_hook(&params, Address::ZERO, &[]);
    assert!(result.is_err());
}

// ── FlashLoanSdk::build_flash_loan_hook Balancer success with user data ─────

#[test]
fn build_flash_loan_hook_balancer_with_user_data() {
    let token = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let params = FlashLoanParams::new(FlashLoanProvider::Balancer, token, U256::from(1_000_u64), 1);
    let user_data = [0xABu8; 64];
    let result = FlashLoanSdk::build_flash_loan_hook(&params, Address::ZERO, &user_data);
    assert!(result.is_ok());
    let hook = result.unwrap();
    assert!(!hook.call_data.is_empty());
    assert_eq!(hook.gas_limit, "500000");
    assert!(hook.target.starts_with("0x"));
}

// ── encode_balancer_flash_loan with user_data ───────────────────────────────

#[test]
fn encode_balancer_flash_loan_with_user_data_has_correct_length() {
    let user_data = vec![0xFFu8; 100];
    let cd = FlashLoanSdk::encode_balancer_flash_loan(
        Address::ZERO,
        Address::ZERO,
        U256::ZERO,
        &user_data,
    );
    // 4 (selector) + 128 (head) + 64 (tokens) + 64 (amounts) + 32 (data len) + 128 (data padded to
    // 128) = 420
    assert_eq!(cd.len(), 420);
}

#[test]
fn encode_balancer_flash_loan_amount_encoded() {
    let amount = U256::from(1_000_000u64);
    let cd = FlashLoanSdk::encode_balancer_flash_loan(Address::ZERO, Address::ZERO, amount, &[]);
    // amounts array: position = 4 (selector) + 128 (head) + 64 (tokens array) + 32 (amounts length)
    // = 228
    let amount_bytes = &cd[228..260];
    let decoded = U256::from_be_bytes::<32>(amount_bytes.try_into().unwrap());
    assert_eq!(decoded, amount);
}

// ── FlashLoanProvider::is_supported_on edge cases ───────────────────────────

#[test]
fn flash_loan_provider_balancer_not_on_arbitrum() {
    assert!(!FlashLoanProvider::Balancer.is_supported_on(42161));
}

#[test]
fn flash_loan_provider_aave_on_gnosis() {
    assert!(FlashLoanProvider::AaveV3.is_supported_on(100));
}

#[test]
fn flash_loan_provider_aave_not_on_arbitrum() {
    assert!(!FlashLoanProvider::AaveV3.is_supported_on(42161));
}

// ── FlashLoanParams debug ───────────────────────────────────────────────────

#[test]
fn flash_loan_params_debug() {
    let params = FlashLoanParams::new(FlashLoanProvider::Balancer, Address::ZERO, U256::ZERO, 1);
    let debug = format!("{params:?}");
    assert!(debug.contains("Balancer"));
}

// ── FlashLoanSdk debug ─────────────────────────────────────────────────────

#[test]
fn flash_loan_sdk_debug() {
    let sdk = FlashLoanSdk;
    let debug = format!("{sdk:?}");
    assert!(debug.contains("FlashLoanSdk"));
}
