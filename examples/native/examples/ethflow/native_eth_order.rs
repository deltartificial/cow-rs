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
//! # EthFlow: Sell Native ETH for USDC
//!
//! Selling native ETH (as opposed to WETH) cannot go through the standard
//! `GPv2Settlement` flow because it has no `approve()` path. Instead, the
//! user calls `EthFlow.createOrder{value: sellAmount}(order)` — the contract
//! custodies the ETH and registers the order with CoW Protocol.
//!
//! This example builds the `createOrder` calldata and the full
//! `EthFlowTransaction` (to + data + value) for a 1 ETH → USDC order.
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example ethflow_order
//! ```

use alloy_primitives::{B256, U256, address};
use cow_chains::{Env, SupportedChainId, eth_flow_for_env};
use cow_ethflow::{EthFlowOrderData, build_eth_flow_transaction, encode_eth_flow_create_order};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chain_id = SupportedChainId::Mainnet;
    let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let receiver = address!("1111111111111111111111111111111111111111");

    // ── Order parameters ─────────────────────────────────────────────────────
    //
    // Sell 1 ETH for at least 2500 USDC (6 decimals).
    let sell_amount = U256::from(10u64).pow(U256::from(18u64)); // 1 ETH in wei
    let buy_amount = U256::from(2_500_000_000u64); // 2500 USDC

    // `valid_to` is a u32 — be careful to stay under 2106.
    let valid_to: u32 =
        (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() + 900) // 15 minutes
            .try_into()?;

    // `quote_id` is returned by the orderbook's `/quote` endpoint.
    // Hard-coded here for illustration.
    let quote_id: i64 = 42;

    let order = EthFlowOrderData {
        buy_token: usdc,
        receiver,
        sell_amount,
        buy_amount,
        app_data: B256::ZERO,
        fee_amount: U256::ZERO, // EthFlow does not charge an explicit fee
        valid_to,
        partially_fillable: false,
        quote_id,
    };

    println!("EthFlow order:");
    println!("  Sell:       1 ETH ({} wei)", order.sell_amount);
    println!("  Buy:        2500 USDC ({} atoms)", order.buy_amount);
    println!("  Receiver:   {}", order.receiver);
    println!("  Valid to:   {} (unix)", order.valid_to);
    println!("  Quote ID:   {}", order.quote_id);
    println!();

    // ── Encode calldata ──────────────────────────────────────────────────────
    //
    // `createOrder((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64))`
    // Fixed-size payload: 4-byte selector + 9 × 32-byte words = 292 bytes.
    let calldata = encode_eth_flow_create_order(&order);
    println!("Encoded calldata: {} bytes", calldata.len());
    println!(
        "  selector: 0x{:02x}{:02x}{:02x}{:02x}",
        calldata[0], calldata[1], calldata[2], calldata[3]
    );
    println!();

    // ── Full transaction ─────────────────────────────────────────────────────
    //
    // The `value` field MUST equal `sell_amount`. The EthFlow contract
    // reverts otherwise.
    let contract = eth_flow_for_env(chain_id, Env::Prod);
    let tx = build_eth_flow_transaction(contract, &order);

    println!("EthFlowTransaction (send this to the chain):");
    println!("  to:      {}", tx.to);
    println!("  value:   {} wei (== sell_amount)", tx.value);
    println!("  data:    {} bytes", tx.data.len());
    assert_eq!(tx.value, order.sell_amount, "value must equal sell_amount");

    Ok(())
}
