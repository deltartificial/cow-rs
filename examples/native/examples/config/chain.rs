#![allow(
    clippy::allow_attributes_without_reason,
    clippy::disallowed_macros,
    clippy::print_stdout,
    clippy::disallowed_methods,
    clippy::uninlined_format_args,
    clippy::literal_string_with_formatting_args,
    clippy::doc_markdown,
    clippy::type_complexity,
    clippy::map_err_ignore,
    clippy::missing_assert_message,
    clippy::single_match_else,
    clippy::print_literal
)]
//! # Chain Configuration & Multi-Chain Support
//!
//! Demonstrates the SDK's multi-chain configuration: supported chains,
//! contract addresses, token registries, and explorer links.
//!
//! CoW Protocol is deployed on 12 chains. This example shows how to
//! query chain info, resolve contract addresses, and build explorer URLs.
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example chain_config
//! ```

use cow_rs::{
    SupportedChainId, all_supported_chain_ids, is_supported_chain, order_explorer_link,
    settlement_contract, supported_chain_info, vault_relayer, wrapped_native_currency,
};

fn main() {
    // ── 1. List all supported chains ─────────────────────────────────────────
    println!("=== Supported Chains ({} total) ===", all_supported_chain_ids().len());
    println!();

    for chain_id in all_supported_chain_ids() {
        let weth = wrapped_native_currency(chain_id);
        let settlement = settlement_contract(chain_id);
        let vault = vault_relayer(chain_id);

        println!("  {:?} (chain_id = {})", chain_id, chain_id as u64);
        println!("    Native wrapped: {} ({})", weth.symbol, weth.address);
        println!("    Settlement:     {settlement}");
        println!("    Vault Relayer:  {vault}");
        println!();
    }

    // ── 2. Chain info lookup ─────────────────────────────────────────────────
    println!("=== Chain Info (Mainnet) ===");
    let info = supported_chain_info(SupportedChainId::Mainnet);
    println!("  {info:?}");
    println!();

    // ── 3. Check chain support ───────────────────────────────────────────────
    println!("=== Chain Support Checks ===");
    println!("  Mainnet (1):    {}", is_supported_chain(1));
    println!("  Sepolia (11M):  {}", is_supported_chain(11_155_111));
    println!("  Optimism (10):  {}", is_supported_chain(10));
    println!("  Arbitrum (42k): {}", is_supported_chain(42_161));
    println!();

    // ── 4. Explorer links ────────────────────────────────────────────────────
    let dummy_uid = "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12345678";

    println!("=== Explorer Links ===");
    println!("  Mainnet:  {}", order_explorer_link(SupportedChainId::Mainnet, dummy_uid));
    println!("  Gnosis:   {}", order_explorer_link(SupportedChainId::GnosisChain, dummy_uid));
    println!("  Sepolia:  {}", order_explorer_link(SupportedChainId::Sepolia, dummy_uid));
    println!("  Arbitrum: {}", order_explorer_link(SupportedChainId::ArbitrumOne, dummy_uid));
    println!();

    // ── 5. All chains summary table ──────────────────────────────────────────
    println!("=== Summary ===");
    println!("{:<15} {:>10}  {}", "Chain", "ID", "Native Token");
    println!("{:-<15} {:-<10}  {:-<12}", "", "", "");
    for chain_id in all_supported_chain_ids() {
        let weth = wrapped_native_currency(chain_id);
        println!("{:<15} {:>10}  {}", format!("{chain_id:?}"), chain_id as u64, weth.symbol);
    }
}
