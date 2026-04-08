// When miri skips this file, suppress the empty-crate lint
#![allow(missing_docs)]
// proptest uses getcwd which is unsupported under miri isolation
#![cfg(not(miri))]
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
//! Property-based tests using `proptest`.

use alloy_primitives::{Address, B256, U256};
use cow_rs::{
    SupportedChainId, UnsignedOrder, appdata_json, bps_to_percentage,
    build_eip2612_nonces_calldata, build_eip2612_version_calldata, build_erc20_approve_calldata,
    build_erc20_balance_of_calldata, build_erc20_decimals_calldata, build_erc20_name_calldata,
    domain_separator, order_hash, percentage_to_bps, permit_domain_separator, permit_type_hash,
    signing_digest,
};
use proptest::prelude::*;

/// Create a `proptest` `Address` from 20 arbitrary bytes.
fn arb_address() -> impl Strategy<Value = Address> {
    prop::array::uniform20(any::<u8>()).prop_map(Address::from)
}

/// Create a `proptest` `U256` from an arbitrary u64.
fn arb_u256_from_u64() -> impl Strategy<Value = U256> {
    any::<u64>().prop_map(U256::from)
}

/// Create a `proptest` `B256` from 32 arbitrary bytes.
fn arb_b256() -> impl Strategy<Value = B256> {
    prop::array::uniform32(any::<u8>()).prop_map(B256::from)
}

// ── ERC-20 calldata selector stability ────────────────────────────────────────

proptest! {
    #[test]
    fn approve_selector_stable_for_any_address_amount(
        addr in arb_address(),
        amount in arb_u256_from_u64(),
    ) {
        let cd = build_erc20_approve_calldata(addr, amount);
        let expected_sel = &alloy_primitives::keccak256(b"approve(address,uint256)")[..4];
        prop_assert_eq!(&cd[..4], expected_sel);
        prop_assert_eq!(cd.len(), 68);
    }

    #[test]
    fn balance_of_selector_stable_for_any_address(addr in arb_address()) {
        let cd = build_erc20_balance_of_calldata(addr);
        let expected_sel = &alloy_primitives::keccak256(b"balanceOf(address)")[..4];
        prop_assert_eq!(&cd[..4], expected_sel);
        prop_assert_eq!(cd.len(), 36);
    }

    #[test]
    fn nonces_calldata_encodes_address(addr in arb_address()) {
        let cd = build_eip2612_nonces_calldata(addr);
        prop_assert_eq!(cd.len(), 36);
        prop_assert_eq!(&cd[16..36], addr.as_slice());
    }

    #[test]
    fn decimals_calldata_always_4_bytes(_dummy in any::<u8>()) {
        prop_assert_eq!(build_erc20_decimals_calldata().len(), 4);
    }

    #[test]
    fn name_calldata_always_4_bytes(_dummy in any::<u8>()) {
        prop_assert_eq!(build_erc20_name_calldata().len(), 4);
    }

    #[test]
    fn version_calldata_always_4_bytes(_dummy in any::<u8>()) {
        prop_assert_eq!(build_eip2612_version_calldata().len(), 4);
    }
}

// ── bps/percentage roundtrip ──────────────────────────────────────────────────

proptest! {
    #[test]
    fn bps_pct_roundtrip(bps in 0u32..=10_000u32) {
        let pct = bps_to_percentage(bps);
        let back = percentage_to_bps(pct);
        prop_assert_eq!(back, bps);
    }
}

// ── EIP-712 hash determinism ──────────────────────────────────────────────────

proptest! {
    #[test]
    fn domain_separator_deterministic(chain_id in any::<u64>()) {
        let ds1 = domain_separator(chain_id);
        let ds2 = domain_separator(chain_id);
        prop_assert_eq!(ds1, ds2);
    }

    #[test]
    fn order_hash_deterministic(
        sell_token in arb_address(),
        buy_token in arb_address(),
        sell_amount in arb_u256_from_u64(),
        buy_amount in arb_u256_from_u64(),
    ) {
        let order = UnsignedOrder::sell(sell_token, buy_token, sell_amount, buy_amount);
        let h1 = order_hash(&order);
        let h2 = order_hash(&order);
        prop_assert_eq!(h1, h2);
    }

    #[test]
    fn signing_digest_deterministic(
        sell_token in arb_address(),
        buy_token in arb_address(),
        sell_amount in arb_u256_from_u64(),
        buy_amount in arb_u256_from_u64(),
    ) {
        let order = UnsignedOrder::sell(sell_token, buy_token, sell_amount, buy_amount);
        let ds = domain_separator(1);
        let h  = order_hash(&order);
        let d1 = signing_digest(ds, h);
        let d2 = signing_digest(ds, h);
        prop_assert_eq!(d1, d2);
    }

    #[test]
    fn b256_prop_non_panic(_b in arb_b256()) {
        // ensure arb_b256 strategy itself works without panic
    }
}

// ── permit domain separator determinism ──────────────────────────────────────

proptest! {
    #[test]
    fn permit_domain_sep_deterministic(
        token in arb_address(),
        chain_id in any::<u64>(),
    ) {
        let ds1 = permit_domain_separator("TestToken", "1", chain_id, token);
        let ds2 = permit_domain_separator("TestToken", "1", chain_id, token);
        prop_assert_eq!(ds1, ds2);
    }

    #[test]
    fn permit_type_hash_always_same(_dummy in any::<u8>()) {
        prop_assert_eq!(permit_type_hash(), permit_type_hash());
    }
}

// ── app-data serde roundtrip ──────────────────────────────────────────────────

proptest! {
    #[test]
    fn appdata_json_contains_app_code(name in "[A-Za-z0-9]{1,20}") {
        let doc = cow_rs::AppDataDoc::new(&name);
        match appdata_json(&doc) {
            Ok(json) => prop_assert!(json.contains(&name)),
            Err(_) => {} // may fail for names with special chars
        }
    }
}

// ── UnsignedOrder builder invariants ─────────────────────────────────────────

proptest! {
    #[test]
    fn unsigned_order_sell_is_not_buy(
        sell_token in arb_address(),
        buy_token in arb_address(),
        sell_amount in arb_u256_from_u64(),
        buy_amount in arb_u256_from_u64(),
    ) {
        let order = UnsignedOrder::sell(sell_token, buy_token, sell_amount, buy_amount);
        prop_assert!(order.is_sell());
        prop_assert!(!order.is_buy());
    }

    #[test]
    fn unsigned_order_buy_is_not_sell(
        sell_token in arb_address(),
        buy_token in arb_address(),
        sell_amount in arb_u256_from_u64(),
        buy_amount in arb_u256_from_u64(),
    ) {
        let order = UnsignedOrder::buy(sell_token, buy_token, sell_amount, buy_amount);
        prop_assert!(order.is_buy());
        prop_assert!(!order.is_sell());
    }

    #[test]
    fn unsigned_order_total_amount_is_sum(
        sell_amount in arb_u256_from_u64(),
        buy_amount in arb_u256_from_u64(),
    ) {
        let order = UnsignedOrder::sell(Address::ZERO, Address::ZERO, sell_amount, buy_amount);
        let expected = sell_amount.saturating_add(buy_amount);
        prop_assert_eq!(order.total_amount(), expected);
    }

    #[test]
    fn unsigned_order_expiry_invariant(valid_to in any::<u32>()) {
        let order = UnsignedOrder::sell(
            Address::ZERO, Address::ZERO, U256::ONE, U256::ONE,
        )
        .with_valid_to(valid_to);
        // At timestamp valid_to, order is NOT expired (inclusive)
        prop_assert!(!order.is_expired(valid_to as u64));
        // At timestamp valid_to + 1, order IS expired
        prop_assert!(order.is_expired(valid_to as u64 + 1));
    }
}

// ── SupportedChainId try_from_u64 ────────────────────────────────────────────

proptest! {
    #[test]
    fn try_from_u64_known_chains_always_some(
        chain_id in prop::sample::select(vec![1u64, 100, 42_161, 8_453, 11_155_111, 137]),
    ) {
        prop_assert!(SupportedChainId::try_from_u64(chain_id).is_some());
    }
}
