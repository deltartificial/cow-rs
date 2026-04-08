//! Tests for `EIP-2612` permit calldata, domain separator, and type hash.

use alloy_primitives::{Address, B256, U256, address, keccak256};
use cow_rs::{
    Erc20PermitInfo, PERMIT_GAS_LIMIT, PermitHookData, PermitInfo, build_permit_calldata,
    permit_digest, permit_domain_separator, permit_type_hash,
};

fn token() -> Address {
    address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
}

fn owner() -> Address {
    address!("1111111111111111111111111111111111111111")
}

fn spender() -> Address {
    address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110")
}

fn basic_permit_info() -> PermitInfo {
    PermitInfo::new(token(), owner(), spender(), U256::from(1_000_000u64))
        .with_nonce(U256::ZERO)
        .with_deadline(9_999_999_999u64)
}

fn usdc_erc20_info() -> Erc20PermitInfo {
    Erc20PermitInfo::new("USD Coin", "2", 1)
}

// ── permit_type_hash ──────────────────────────────────────────────────────────

#[test]
fn permit_type_hash_is_non_zero() {
    assert_ne!(permit_type_hash(), B256::ZERO);
}

#[test]
fn permit_type_hash_is_deterministic() {
    assert_eq!(permit_type_hash(), permit_type_hash());
}

#[test]
fn permit_type_hash_matches_expected_string() {
    let expected = keccak256(
        b"Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)",
    );
    assert_eq!(permit_type_hash(), expected);
}

// ── permit_domain_separator ───────────────────────────────────────────────────

#[test]
fn permit_domain_sep_is_deterministic() {
    let ds1 = permit_domain_separator("USD Coin", "2", 1, token());
    let ds2 = permit_domain_separator("USD Coin", "2", 1, token());
    assert_eq!(ds1, ds2);
}

#[test]
fn permit_domain_sep_differs_by_chain() {
    let mainnet = permit_domain_separator("USD Coin", "2", 1, token());
    let other = permit_domain_separator("USD Coin", "2", 5, token());
    assert_ne!(mainnet, other);
}

#[test]
fn permit_domain_sep_differs_by_token() {
    let t1 = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let t2 = address!("dAC17F958D2ee523a2206206994597C13D831ec7");
    let ds1 = permit_domain_separator("USD Coin", "2", 1, t1);
    let ds2 = permit_domain_separator("USD Coin", "2", 1, t2);
    assert_ne!(ds1, ds2);
}

#[test]
fn permit_domain_sep_differs_by_name() {
    let ds1 = permit_domain_separator("Token A", "1", 1, token());
    let ds2 = permit_domain_separator("Token B", "1", 1, token());
    assert_ne!(ds1, ds2);
}

#[test]
fn permit_domain_sep_differs_by_version() {
    let ds1 = permit_domain_separator("USD Coin", "1", 1, token());
    let ds2 = permit_domain_separator("USD Coin", "2", 1, token());
    assert_ne!(ds1, ds2);
}

// ── permit_digest ──────────────────────────────────────────────────────────────

#[test]
fn permit_digest_is_deterministic() {
    let ds = permit_domain_separator("USD Coin", "2", 1, token());
    let info = basic_permit_info();
    let d1 = permit_digest(ds, &info);
    let d2 = permit_digest(ds, &info);
    assert_eq!(d1, d2);
}

#[test]
fn permit_digest_differs_by_nonce() {
    let ds = permit_domain_separator("USD Coin", "2", 1, token());
    let info1 = basic_permit_info().with_nonce(U256::ZERO);
    let info2 = basic_permit_info().with_nonce(U256::from(1u64));
    assert_ne!(permit_digest(ds, &info1), permit_digest(ds, &info2));
}

#[test]
fn permit_digest_is_non_zero() {
    let ds = permit_domain_separator("USD Coin", "2", 1, token());
    let info = basic_permit_info();
    assert_ne!(permit_digest(ds, &info), B256::ZERO);
}

// ── build_permit_calldata ──────────────────────────────────────────────────────

#[test]
fn permit_calldata_len_is_260() {
    // 4 (selector) + 8 × 32 (params) = 260
    let cd = build_permit_calldata(&basic_permit_info(), [0u8; 65]);
    assert_eq!(cd.len(), 260);
}

#[test]
fn permit_calldata_selector_correct() {
    let cd = build_permit_calldata(&basic_permit_info(), [0u8; 65]);
    let sel =
        &keccak256(b"permit(address,address,uint256,uint256,uint256,uint8,bytes32,bytes32)")[..4];
    assert_eq!(&cd[..4], sel);
}

#[test]
fn permit_calldata_different_sigs_produce_different_calldata() {
    let info = basic_permit_info();
    let sig1 = [1u8; 65];
    let sig2 = [2u8; 65];
    let cd1 = build_permit_calldata(&info, sig1);
    let cd2 = build_permit_calldata(&info, sig2);
    assert_ne!(cd1, cd2);
}

// ── PermitInfo builder ────────────────────────────────────────────────────────

#[test]
fn permit_info_is_expired() {
    let info = basic_permit_info();
    assert!(!info.is_expired(info.deadline));
    assert!(info.is_expired(info.deadline + 1));
}

#[test]
fn permit_info_is_zero_allowance() {
    let zero = PermitInfo::new(token(), owner(), spender(), U256::ZERO);
    assert!(zero.is_zero_allowance());
    assert!(!basic_permit_info().is_zero_allowance());
}

#[test]
fn permit_info_is_unlimited_allowance() {
    let unlimited = PermitInfo::new(token(), owner(), spender(), U256::MAX);
    assert!(unlimited.is_unlimited_allowance());
    assert!(!basic_permit_info().is_unlimited_allowance());
}

// ── Erc20PermitInfo ───────────────────────────────────────────────────────────

#[test]
fn erc20_permit_info_fields() {
    let info = usdc_erc20_info();
    assert_eq!(info.name, "USD Coin");
    assert_eq!(info.version, "2");
    assert_eq!(info.chain_id, 1);
}

// ── PermitHookData ────────────────────────────────────────────────────────────

#[test]
fn permit_hook_data_has_calldata() {
    let hook = PermitHookData::new(token(), vec![1, 2, 3, 4], PERMIT_GAS_LIMIT);
    assert!(hook.has_calldata());
    assert_eq!(hook.calldata_len(), 4);
}

#[test]
fn permit_hook_data_empty_calldata() {
    let hook = PermitHookData::new(token(), vec![], PERMIT_GAS_LIMIT);
    assert!(!hook.has_calldata());
    assert_eq!(hook.calldata_len(), 0);
}

#[test]
fn permit_hook_data_into_cow_hook() {
    let calldata = build_permit_calldata(&basic_permit_info(), [0u8; 65]);
    let hook = PermitHookData::new(token(), calldata, PERMIT_GAS_LIMIT);
    let cow_hook = hook.into_cow_hook();
    assert!(cow_hook.call_data.starts_with("0x"));
    assert_eq!(cow_hook.gas_limit, PERMIT_GAS_LIMIT.to_string());
}

#[test]
fn permit_gas_limit_is_100k() {
    assert_eq!(PERMIT_GAS_LIMIT, 100_000);
}
