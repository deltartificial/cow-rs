//! Tests for the `CowShed` proxy contract helpers.

use alloy_primitives::{Address, B256, U256, address};
use cow_rs::{
    CowShedCall, CowShedHookParams, CowShedSdk,
    cow_shed::{COW_SHED_FACTORY_GNOSIS, COW_SHED_FACTORY_MAINNET},
};

// ── factory addresses ─────────────────────────────────────────────────────────

#[test]
fn cow_shed_factory_mainnet_is_non_zero() {
    assert_ne!(COW_SHED_FACTORY_MAINNET, Address::ZERO);
}

#[test]
fn cow_shed_factory_gnosis_is_non_zero() {
    assert_ne!(COW_SHED_FACTORY_GNOSIS, Address::ZERO);
}

#[test]
fn cow_shed_factory_mainnet_from_sdk() {
    let sdk = CowShedSdk::new(1);
    assert_eq!(sdk.factory_address(), Some(COW_SHED_FACTORY_MAINNET));
}

#[test]
fn cow_shed_factory_gnosis_from_sdk() {
    let sdk = CowShedSdk::new(100);
    assert_eq!(sdk.factory_address(), Some(COW_SHED_FACTORY_GNOSIS));
}

#[test]
fn cow_shed_factory_unknown_chain_is_none() {
    let sdk = CowShedSdk::new(999);
    assert!(sdk.factory_address().is_none());
}

// ── CowShedCall ───────────────────────────────────────────────────────────────

#[test]
fn cow_shed_call_new_default_value_is_zero() {
    let call = CowShedCall::new(Address::ZERO, vec![]);
    assert_eq!(call.value, U256::ZERO);
}

#[test]
fn cow_shed_call_with_value() {
    let call = CowShedCall::new(Address::ZERO, vec![]).with_value(U256::from(1_000_u64));
    assert_eq!(call.value, U256::from(1_000_u64));
}

#[test]
fn cow_shed_call_allowing_failure() {
    let call = CowShedCall::new(Address::ZERO, vec![]).allowing_failure();
    assert!(call.allow_failure);
}

#[test]
fn cow_shed_call_has_value_true_when_nonzero() {
    let call = CowShedCall::new(Address::ZERO, vec![1]).with_value(U256::from(1_u64));
    assert!(call.has_value());
}

#[test]
fn cow_shed_call_has_value_false_when_zero() {
    let call = CowShedCall::new(Address::ZERO, vec![]);
    assert!(!call.has_value());
}

// ── CowShedHookParams ─────────────────────────────────────────────────────────

fn make_params(n: usize) -> CowShedHookParams {
    let calls = (0..n).map(|_| CowShedCall::new(Address::ZERO, vec![])).collect();
    CowShedHookParams::new(calls, B256::ZERO, U256::from(9_999_u64))
}

#[test]
fn cow_shed_hook_params_call_count() {
    assert_eq!(make_params(3).call_count(), 3);
}

#[test]
fn cow_shed_hook_params_is_empty() {
    assert!(make_params(0).is_empty());
}

#[test]
fn cow_shed_hook_params_is_not_empty() {
    assert!(!make_params(1).is_empty());
}

#[test]
fn cow_shed_hook_params_nonce_ref() {
    let params = make_params(0);
    assert_eq!(params.nonce_ref(), &B256::ZERO);
}

// ── encode_execute_hooks_calldata ─────────────────────────────────────────────

#[test]
fn cow_shed_encode_calldata_min_length() {
    let params = make_params(0);
    // 4 selector + 32 nonce + 32 deadline = 68
    let cd = CowShedSdk::encode_execute_hooks_calldata(&params);
    assert_eq!(cd.len(), 68);
}

#[test]
fn cow_shed_build_hook_gas_limit_scales_with_calls() {
    let sdk = CowShedSdk::new(1);
    let proxy = address!("1111111111111111111111111111111111111111");
    let user = address!("2222222222222222222222222222222222222222");
    let params = make_params(2);
    let hook = sdk.build_hook(user, proxy, &params).unwrap();
    // 100_000 + 50_000 * 2 = 200_000
    assert_eq!(hook.gas_limit, "200000");
}
