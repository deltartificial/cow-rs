//! Tests for `EIP-712` order hashing, domain separation, and signing.

use alloy_primitives::{Address, B256, U256, address, keccak256};
use cow_rs::{
    OrderDomain, OrderTypedData, SigningResult, SigningScheme, TokenBalance, UnsignedOrder,
    cancellations_hash, compute_order_uid, domain_separator, order_hash, signing_digest,
};

fn zero_order() -> UnsignedOrder {
    UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::from(1_000u64), U256::from(900u64))
}

// ── domain_separator ──────────────────────────────────────────────────────────

#[test]
fn domain_sep_is_deterministic() {
    let ds1 = domain_separator(1);
    let ds2 = domain_separator(1);
    assert_eq!(ds1, ds2);
}

#[test]
fn domain_sep_differs_by_chain_id() {
    let mainnet = domain_separator(1);
    let sepolia = domain_separator(11_155_111);
    assert_ne!(mainnet, sepolia);
}

#[test]
fn domain_sep_is_non_zero() {
    assert_ne!(domain_separator(1), B256::ZERO);
}

// ── OrderDomain ───────────────────────────────────────────────────────────────

#[test]
fn order_domain_for_chain() {
    let domain = OrderDomain::for_chain(1);
    assert_eq!(domain.chain_id, 1);
}

#[test]
fn order_domain_separator_matches_standalone_fn() {
    let domain = OrderDomain::for_chain(1);
    let ds_domain = domain.domain_separator();
    let ds_fn = domain_separator(1);
    assert_eq!(ds_domain, ds_fn);
}

// ── UnsignedOrder ─────────────────────────────────────────────────────────────

#[test]
fn unsigned_order_sell_is_sell() {
    let order = zero_order();
    assert!(order.is_sell());
    assert!(!order.is_buy());
}

#[test]
fn unsigned_order_buy_is_buy() {
    let order =
        UnsignedOrder::buy(Address::ZERO, Address::ZERO, U256::from(900u64), U256::from(1_000u64));
    assert!(order.is_buy());
    assert!(!order.is_sell());
}

#[test]
fn unsigned_order_has_app_data_false_by_default() {
    assert!(!zero_order().has_app_data());
}

#[test]
fn unsigned_order_with_app_data_sets_flag() {
    let app_data = keccak256(b"test");
    let order = zero_order().with_app_data(app_data);
    assert!(order.has_app_data());
}

#[test]
fn unsigned_order_has_custom_receiver_false_for_zero() {
    let order = zero_order().with_receiver(Address::ZERO);
    assert!(!order.has_custom_receiver());
}

#[test]
fn unsigned_order_has_custom_receiver_true_for_nonzero() {
    let recv = address!("1111111111111111111111111111111111111111");
    let order = zero_order().with_receiver(recv);
    assert!(order.has_custom_receiver());
}

#[test]
fn unsigned_order_has_fee_false_when_zero() {
    assert!(!zero_order().has_fee());
}

#[test]
fn unsigned_order_has_fee_true_when_set() {
    let order = zero_order().with_fee_amount(U256::from(100u64));
    assert!(order.has_fee());
}

#[test]
fn unsigned_order_is_expired() {
    let order = zero_order().with_valid_to(1000);
    assert!(!order.is_expired(999));
    assert!(!order.is_expired(1000));
    assert!(order.is_expired(1001));
}

#[test]
fn unsigned_order_total_amount_is_sell_plus_buy() {
    // total_amount = sell_amount + buy_amount (not fee-adjusted)
    let order =
        UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::from(1000u64), U256::from(900u64));
    assert_eq!(order.total_amount(), U256::from(1900u64));
}

// ── order_hash ────────────────────────────────────────────────────────────────

#[test]
fn order_hash_is_deterministic() {
    let order = zero_order();
    let h1 = order_hash(&order);
    let h2 = order_hash(&order);
    assert_eq!(h1, h2);
}

#[test]
fn order_hash_is_non_zero() {
    let order = zero_order();
    assert_ne!(order_hash(&order), B256::ZERO);
}

#[test]
fn order_hash_differs_for_different_kinds() {
    let sell = zero_order();
    let buy =
        UnsignedOrder::buy(Address::ZERO, Address::ZERO, U256::from(1_000u64), U256::from(900u64));
    assert_ne!(order_hash(&sell), order_hash(&buy));
}

// ── signing_digest ────────────────────────────────────────────────────────────

#[test]
fn signing_digest_is_deterministic() {
    let order = zero_order();
    let ds = domain_separator(1);
    let h = order_hash(&order);
    let d1 = signing_digest(ds, h);
    let d2 = signing_digest(ds, h);
    assert_eq!(d1, d2);
}

#[test]
fn signing_digest_differs_from_order_struct_hash() {
    let order = zero_order();
    let hash = order_hash(&order);
    let digest = signing_digest(domain_separator(1), hash);
    assert_ne!(B256::from(hash), digest);
}

#[test]
fn signing_digest_differs_by_chain() {
    let order = zero_order();
    let h = order_hash(&order);
    let d1 = signing_digest(domain_separator(1), h);
    let d2 = signing_digest(domain_separator(11_155_111), h);
    assert_ne!(d1, d2);
}

// ── OrderTypedData ────────────────────────────────────────────────────────────

#[test]
fn order_typed_data_refs() {
    let domain = OrderDomain::for_chain(1);
    let order = zero_order();
    let typed = OrderTypedData::new(domain, order);
    assert!(typed.order_ref().is_sell());
    assert_eq!(typed.domain_ref().chain_id, 1);
}

#[test]
fn order_typed_data_digest_matches_standalone() {
    let domain = OrderDomain::for_chain(1);
    let order = zero_order();
    let ds = domain.domain_separator();
    let typed = OrderTypedData::new(domain, order);
    let td_digest = typed.signing_digest();
    let standalone = signing_digest(ds, order_hash(typed.order_ref()));
    assert_eq!(td_digest, standalone);
}

// ── SigningResult ─────────────────────────────────────────────────────────────

#[test]
fn signing_result_is_eip712() {
    let r = SigningResult::new("0xabcd", SigningScheme::Eip712);
    assert!(r.is_eip712());
    assert!(!r.is_eth_sign());
}

#[test]
fn signing_result_len_matches_signature_str() {
    let r = SigningResult::new("0xabcd", SigningScheme::Eip712);
    assert_eq!(r.signature_len(), 6); // "0xabcd".len()
}

#[test]
fn signing_result_ref_is_borrowed_sig() {
    let r = SigningResult::new("0xdeadbeef", SigningScheme::Eip712);
    assert_eq!(r.signature_ref(), "0xdeadbeef");
}

// ── compute_order_uid ─────────────────────────────────────────────────────────

#[test]
fn compute_order_uid_is_114_chars_with_0x_prefix() {
    let owner = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let order = zero_order().with_valid_to(u32::MAX);
    let uid = compute_order_uid(1, &order, owner);
    // 0x + 56 hex bytes = 56*2 + 2 = 114 chars
    assert_eq!(uid.len(), 114);
    assert!(uid.starts_with("0x"));
}

#[test]
fn compute_order_uid_is_deterministic() {
    let owner = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let order = zero_order().with_valid_to(9999);
    let uid1 = compute_order_uid(1, &order, owner);
    let uid2 = compute_order_uid(1, &order, owner);
    assert_eq!(uid1, uid2);
}

#[test]
fn compute_order_uid_differs_by_chain() {
    let owner = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let order = zero_order().with_valid_to(9999);
    let uid1 = compute_order_uid(1, &order, owner);
    let uid2 = compute_order_uid(11_155_111, &order, owner);
    assert_ne!(uid1, uid2);
}

// ── cancellations_hash ────────────────────────────────────────────────────────

#[test]
fn cancellations_hash_single_uid_is_non_zero() {
    let uid = "0x".to_owned() + &"ab".repeat(56);
    let hash = cancellations_hash(&[uid.as_str()]).unwrap();
    assert_ne!(hash, B256::ZERO);
}

#[test]
fn cancellations_hash_empty_is_deterministic() {
    let h1 = cancellations_hash(&[]).unwrap();
    let h2 = cancellations_hash(&[]).unwrap();
    assert_eq!(h1, h2);
}

// ── TokenBalance ──────────────────────────────────────────────────────────────

#[test]
fn token_balance_variants_exist() {
    let _erc20 = TokenBalance::Erc20;
    let _internal = TokenBalance::Internal;
    let _external = TokenBalance::External;
}
