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
//! # On-Chain Token Reader
//!
//! Reads ERC-20 token data directly from the blockchain via JSON-RPC
//! `eth_call`.  Useful for checking balances, allowances, and token
//! metadata before placing an order.
//!
//! This is the Rust equivalent of checking allowances in the TypeScript
//! Wagmi example:
//!
//! ```ts
//! const allowance = await tradingSdk.getCowProtocolAllowance({
//!     tokenAddress: sellToken.address,
//!     owner: account,
//!     chainId,
//! });
//! ```
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example onchain_reader
//! ```
//!
//! Optionally set a custom RPC URL:
//!
//! ```sh
//! RPC_URL=https://eth.llamarpc.com cargo run --example onchain_reader
//! ```

use alloy_primitives::Address;
use cow_rs::{OnchainReader, SupportedChainId, vault_relayer, wrapped_native_currency};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Configuration ────────────────────────────────────────────────────────
    let chain_id = SupportedChainId::Mainnet;
    let rpc_url =
        std::env::var("RPC_URL").unwrap_or_else(|_| "https://eth.llamarpc.com".to_owned());

    let weth = wrapped_native_currency(chain_id);
    let usdc: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse()?;

    // A well-known address with USDC holdings (Binance hot wallet).
    let holder: Address = "0xF977814e90dA44bFA03b6295A0616a897441aceC".parse()?;

    // The Vault Relayer is the spender that CoW Protocol uses to pull tokens.
    let vault = vault_relayer(chain_id);

    println!("Reading on-chain data on {:?}:", chain_id);
    println!("  RPC: {rpc_url}");
    println!();

    let reader = OnchainReader::new(&rpc_url);

    // ── 1. Token metadata ────────────────────────────────────────────────────
    let usdc_name = reader.erc20_name(usdc).await?;
    let usdc_decimals = reader.erc20_decimals(usdc).await?;
    println!("=== USDC ===");
    println!("  Name:     {usdc_name}");
    println!("  Decimals: {usdc_decimals}");
    println!("  Address:  {usdc}");
    println!();

    let weth_name = reader.erc20_name(weth.address).await?;
    println!("=== WETH ===");
    println!("  Name:     {weth_name}");
    println!("  Decimals: {}", weth.decimals);
    println!("  Address:  {}", weth.address);
    println!();

    // ── 2. Balance query ─────────────────────────────────────────────────────
    let usdc_balance = reader.erc20_balance(usdc, holder).await?;
    let balance_human = usdc_balance.to::<u128>() as f64 / 10f64.powi(usdc_decimals.into());

    println!("=== USDC Balance ===");
    println!("  Holder:  {holder}");
    println!("  Balance: {balance_human:.2} USDC ({usdc_balance} atoms)");
    println!();

    // ── 3. Allowance check ───────────────────────────────────────────────────
    //
    // Before an order can be filled, the Vault Relayer must have sufficient
    // allowance to spend the sell token.  This is what the Wagmi example
    // checks with `tradingSdk.getCowProtocolAllowance()`.
    let allowance = reader.erc20_allowance(usdc, holder, vault).await?;
    let allowance_human = allowance.to::<u128>() as f64 / 10f64.powi(usdc_decimals.into());

    println!("=== Vault Relayer Allowance ===");
    println!("  Spender:   {vault}");
    println!("  Allowance: {allowance_human:.2} USDC ({allowance} atoms)");

    if allowance.is_zero() {
        println!();
        println!("  Tip: The holder has not approved the Vault Relayer.");
        println!("  Call `approve(vaultRelayer, amount)` on the token contract");
        println!("  before submitting an order.");
    }

    Ok(())
}
