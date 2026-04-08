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
//! Tests for chain config, contract addresses, and URL helpers.

use cow_rs::{
    Env, SETTLEMENT_CONTRACT, SupportedChainId, VAULT_RELAYER, api_base_url, order_explorer_link,
    settlement_contract, vault_relayer, wrapped_native_currency,
};

// ── SupportedChainId discriminants ────────────────────────────────────────────

#[test]
fn mainnet_chain_id_is_1() {
    assert_eq!(SupportedChainId::Mainnet.as_u64(), 1);
}

#[test]
fn gnosis_chain_id_is_100() {
    assert_eq!(SupportedChainId::GnosisChain.as_u64(), 100);
}

#[test]
fn sepolia_chain_id_is_11155111() {
    assert_eq!(SupportedChainId::Sepolia.as_u64(), 11_155_111);
}

#[test]
fn arbitrum_chain_id_is_42161() {
    assert_eq!(SupportedChainId::ArbitrumOne.as_u64(), 42_161);
}

#[test]
fn base_chain_id_is_8453() {
    assert_eq!(SupportedChainId::Base.as_u64(), 8_453);
}

// ── try_from_u64 ──────────────────────────────────────────────────────────────

#[test]
fn try_from_u64_mainnet() {
    assert_eq!(SupportedChainId::try_from_u64(1), Some(SupportedChainId::Mainnet));
}

#[test]
fn try_from_u64_sepolia() {
    assert_eq!(SupportedChainId::try_from_u64(11_155_111), Some(SupportedChainId::Sepolia));
}

#[test]
fn try_from_u64_unknown_returns_none() {
    assert_eq!(SupportedChainId::try_from_u64(9999), None);
}

// ── is_testnet / is_mainnet ────────────────────────────────────────────────────

#[test]
fn sepolia_is_testnet() {
    assert!(SupportedChainId::Sepolia.is_testnet());
}

#[test]
fn mainnet_is_not_testnet() {
    assert!(!SupportedChainId::Mainnet.is_testnet());
}

#[test]
fn mainnet_is_mainnet() {
    assert!(SupportedChainId::Mainnet.is_mainnet());
}

#[test]
fn sepolia_is_not_mainnet() {
    assert!(!SupportedChainId::Sepolia.is_mainnet());
}

// ── is_layer2 ─────────────────────────────────────────────────────────────────

#[test]
fn arbitrum_is_layer2() {
    assert!(SupportedChainId::ArbitrumOne.is_layer2());
}

#[test]
fn base_is_layer2() {
    assert!(SupportedChainId::Base.is_layer2());
}

#[test]
fn polygon_is_layer2() {
    assert!(SupportedChainId::Polygon.is_layer2());
}

#[test]
fn mainnet_is_not_layer2() {
    assert!(!SupportedChainId::Mainnet.is_layer2());
}

#[test]
fn gnosis_is_not_layer2() {
    assert!(!SupportedChainId::GnosisChain.is_layer2());
}

// ── as_str ────────────────────────────────────────────────────────────────────

#[test]
fn mainnet_as_str() {
    assert_eq!(SupportedChainId::Mainnet.as_str(), "mainnet");
}

#[test]
fn gnosis_as_str_is_xdai() {
    assert_eq!(SupportedChainId::GnosisChain.as_str(), "xdai");
}

#[test]
fn sepolia_as_str() {
    assert_eq!(SupportedChainId::Sepolia.as_str(), "sepolia");
}

// ── api_base_url ──────────────────────────────────────────────────────────────

#[test]
fn api_base_url_starts_with_https() {
    let url = api_base_url(SupportedChainId::Mainnet, Env::Prod);
    assert!(url.starts_with("https://"));
}

#[test]
fn api_base_url_prod_vs_staging_differ() {
    let prod = api_base_url(SupportedChainId::Mainnet, Env::Prod);
    let staging = api_base_url(SupportedChainId::Mainnet, Env::Staging);
    assert_ne!(prod, staging);
}

#[test]
fn api_base_url_mainnet_contains_mainnet() {
    let url = api_base_url(SupportedChainId::Mainnet, Env::Prod);
    assert!(url.contains("mainnet"));
}

// ── order_explorer_link ───────────────────────────────────────────────────────

#[test]
fn explorer_link_starts_with_cow_fi() {
    let link = order_explorer_link(SupportedChainId::Mainnet, "0xabc");
    assert!(link.contains("cow.fi"));
}

#[test]
fn explorer_link_ends_with_uid() {
    let uid = "0xdeadbeef";
    let link = order_explorer_link(SupportedChainId::Sepolia, uid);
    assert!(link.ends_with(uid));
}

// ── contract addresses ────────────────────────────────────────────────────────

#[test]
fn settlement_contract_is_same_for_all_chains() {
    let addr_mainnet = settlement_contract(SupportedChainId::Mainnet);
    let addr_sepolia = settlement_contract(SupportedChainId::Sepolia);
    assert_eq!(addr_mainnet, addr_sepolia);
}

#[test]
fn settlement_contract_matches_constant() {
    assert_eq!(settlement_contract(SupportedChainId::Mainnet), SETTLEMENT_CONTRACT);
}

#[test]
fn vault_relayer_is_same_for_all_chains() {
    let addr_mainnet = vault_relayer(SupportedChainId::Mainnet);
    let addr_sepolia = vault_relayer(SupportedChainId::Sepolia);
    assert_eq!(addr_mainnet, addr_sepolia);
}

#[test]
fn vault_relayer_matches_constant() {
    assert_eq!(vault_relayer(SupportedChainId::Mainnet), VAULT_RELAYER);
}

#[test]
fn settlement_contract_is_non_zero() {
    assert_ne!(SETTLEMENT_CONTRACT, alloy_primitives::Address::ZERO);
}

#[test]
fn vault_relayer_is_non_zero() {
    assert_ne!(VAULT_RELAYER, alloy_primitives::Address::ZERO);
}

// ── wrapped native currency ───────────────────────────────────────────────────

#[test]
fn wrapped_native_mainnet_is_weth() {
    let weth = wrapped_native_currency(SupportedChainId::Mainnet);
    // TokenInfo doesn't implement PartialEq; compare the address field
    assert_ne!(weth.address, alloy_primitives::Address::ZERO);
}

#[test]
fn wrapped_native_differs_across_chains() {
    let mainnet = wrapped_native_currency(SupportedChainId::Mainnet);
    let gnosis = wrapped_native_currency(SupportedChainId::GnosisChain);
    assert_ne!(mainnet.address, gnosis.address);
}

// ── all() helper ──────────────────────────────────────────────────────────────

#[test]
fn all_chains_not_empty() {
    assert!(!SupportedChainId::all().is_empty());
}

#[test]
fn all_chains_contains_mainnet_and_sepolia() {
    let all = SupportedChainId::all();
    assert!(all.contains(&SupportedChainId::Mainnet));
    assert!(all.contains(&SupportedChainId::Sepolia));
}

#[test]
fn all_chain_ids_unique() {
    let all = SupportedChainId::all();
    let mut ids: Vec<u64> = all.iter().map(|c| c.as_u64()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), all.len());
}
