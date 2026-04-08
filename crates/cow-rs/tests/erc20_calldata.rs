//! Tests for all nine `ERC-20` / `EIP-2612` calldata builder functions.

use alloy_primitives::{Address, U256, address, keccak256};
use cow_rs::{
    VAULT_RELAYER, build_erc20_allowance_calldata as reexport_allowance,
    erc20::{
        build_eip2612_nonces_calldata, build_eip2612_version_calldata,
        build_erc20_allowance_calldata, build_erc20_approve_calldata,
        build_erc20_balance_of_calldata, build_erc20_decimals_calldata, build_erc20_name_calldata,
        build_erc20_transfer_calldata, build_erc20_transfer_from_calldata,
    },
};

fn owner() -> Address {
    address!("1111111111111111111111111111111111111111")
}

fn spender() -> Address {
    address!("2222222222222222222222222222222222222222")
}

fn recipient() -> Address {
    address!("3333333333333333333333333333333333333333")
}

fn sel(sig: &str) -> [u8; 4] {
    let h = keccak256(sig.as_bytes());
    [h[0], h[1], h[2], h[3]]
}

// ── approve ───────────────────────────────────────────────────────────────────

#[test]
fn approve_len_is_68() {
    assert_eq!(build_erc20_approve_calldata(spender(), U256::ONE).len(), 68);
}

#[test]
fn approve_selector_correct() {
    let cd = build_erc20_approve_calldata(spender(), U256::ONE);
    assert_eq!(&cd[..4], &sel("approve(address,uint256)"));
}

#[test]
fn approve_encodes_max_allowance() {
    let cd = build_erc20_approve_calldata(spender(), U256::MAX);
    let amount = U256::from_be_slice(&cd[36..68]);
    assert_eq!(amount, U256::MAX);
}

// ── balanceOf ─────────────────────────────────────────────────────────────────

#[test]
fn balance_of_len_is_36() {
    assert_eq!(build_erc20_balance_of_calldata(owner()).len(), 36);
}

#[test]
fn balance_of_selector_correct() {
    let cd = build_erc20_balance_of_calldata(owner());
    assert_eq!(&cd[..4], &sel("balanceOf(address)"));
}

#[test]
fn balance_of_encodes_address_in_last_20_bytes() {
    let cd = build_erc20_balance_of_calldata(owner());
    assert_eq!(&cd[16..36], owner().as_slice());
}

// ── allowance ─────────────────────────────────────────────────────────────────

#[test]
fn allowance_len_is_68() {
    assert_eq!(build_erc20_allowance_calldata(owner(), spender()).len(), 68);
}

#[test]
fn allowance_selector_correct() {
    let cd = build_erc20_allowance_calldata(owner(), spender());
    assert_eq!(&cd[..4], &sel("allowance(address,address)"));
}

#[test]
fn allowance_reexport_matches_direct() {
    let direct = build_erc20_allowance_calldata(owner(), spender());
    let reexport = reexport_allowance(owner(), spender());
    assert_eq!(direct, reexport);
}

// ── transfer ──────────────────────────────────────────────────────────────────

#[test]
fn transfer_len_is_68() {
    assert_eq!(build_erc20_transfer_calldata(recipient(), U256::from(100u64)).len(), 68);
}

#[test]
fn transfer_selector_correct() {
    let cd = build_erc20_transfer_calldata(recipient(), U256::from(100u64));
    assert_eq!(&cd[..4], &sel("transfer(address,uint256)"));
}

// ── transferFrom ──────────────────────────────────────────────────────────────

#[test]
fn transfer_from_len_is_100() {
    let cd = build_erc20_transfer_from_calldata(owner(), recipient(), U256::from(50u64));
    assert_eq!(cd.len(), 100);
}

#[test]
fn transfer_from_selector_correct() {
    let cd = build_erc20_transfer_from_calldata(owner(), recipient(), U256::from(50u64));
    assert_eq!(&cd[..4], &sel("transferFrom(address,address,uint256)"));
}

// ── decimals ──────────────────────────────────────────────────────────────────

#[test]
fn decimals_len_is_4() {
    assert_eq!(build_erc20_decimals_calldata().len(), 4);
}

#[test]
fn decimals_selector_correct() {
    let cd = build_erc20_decimals_calldata();
    assert_eq!(&cd[..], &sel("decimals()"));
}

// ── name ──────────────────────────────────────────────────────────────────────

#[test]
fn name_len_is_4() {
    assert_eq!(build_erc20_name_calldata().len(), 4);
}

#[test]
fn name_selector_correct() {
    let cd = build_erc20_name_calldata();
    assert_eq!(&cd[..], &sel("name()"));
}

#[test]
fn name_and_decimals_selectors_differ() {
    assert_ne!(build_erc20_name_calldata(), build_erc20_decimals_calldata());
}

// ── EIP-2612 nonces ───────────────────────────────────────────────────────────

#[test]
fn nonces_len_is_36() {
    assert_eq!(build_eip2612_nonces_calldata(owner()).len(), 36);
}

#[test]
fn nonces_selector_correct() {
    let cd = build_eip2612_nonces_calldata(owner());
    assert_eq!(&cd[..4], &sel("nonces(address)"));
}

#[test]
fn nonces_encodes_owner() {
    let cd = build_eip2612_nonces_calldata(owner());
    assert_eq!(&cd[16..36], owner().as_slice());
}

#[test]
fn nonces_different_owners_produce_different_calldata() {
    let cd1 = build_eip2612_nonces_calldata(owner());
    let cd2 = build_eip2612_nonces_calldata(spender());
    assert_ne!(cd1, cd2);
}

// ── EIP-2612 version ──────────────────────────────────────────────────────────

#[test]
fn version_len_is_4() {
    assert_eq!(build_eip2612_version_calldata().len(), 4);
}

#[test]
fn version_selector_correct() {
    let cd = build_eip2612_version_calldata();
    assert_eq!(&cd[..], &sel("version()"));
}

// ── vault relayer is usable ───────────────────────────────────────────────────

#[test]
fn vault_relayer_approve_has_correct_selector() {
    let cd = build_erc20_approve_calldata(VAULT_RELAYER, U256::MAX);
    assert_eq!(&cd[..4], &sel("approve(address,uint256)"));
}
