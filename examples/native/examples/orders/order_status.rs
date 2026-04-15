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
//! # Query Order Status
//!
//! Fetches the current status and details of an existing order from the
//! CoW Protocol order book API.  This is the Rust equivalent of:
//!
//! ```ts
//! const order = await tradingSdk.getOrder({ orderUid, chainId });
//! console.log(`Status: ${order.status}`);
//! ```
//!
//! ## Usage
//!
//! ```sh
//! ORDER_UID=0x... cargo run --example order_status
//! ```
//!
//! Or query multiple orders for an address:
//!
//! ```sh
//! OWNER=0x... cargo run --example order_status
//! ```

use alloy_primitives::Address;
use cow_chains::{Env, SupportedChainId, order_explorer_link};
use cow_orderbook::{GetOrdersRequest, OrderBookApi};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chain_id = SupportedChainId::Mainnet;
    let api = OrderBookApi::new(chain_id, Env::Prod);

    // ── Option A: Query a specific order by UID ──────────────────────────────
    if let Ok(order_uid) = std::env::var("ORDER_UID") {
        println!("Fetching order {order_uid}...");
        println!();

        let order = api.get_order(&order_uid).await?;

        println!("=== Order Details ===");
        println!("  Status:           {:?}", order.status);
        println!("  Kind:             {:?}", order.kind);
        println!("  Sell token:       {}", order.sell_token);
        println!("  Buy token:        {}", order.buy_token);
        println!("  Sell amount:      {}", order.sell_amount);
        println!("  Buy amount:       {}", order.buy_amount);
        println!("  Fee amount:       {}", order.fee_amount);
        println!("  Owner:            {}", order.owner);
        println!("  Valid to:         {}", order.valid_to);
        println!("  Partially filled: {}", order.partially_fillable);
        println!("  Explorer:         {}", order_explorer_link(chain_id, &order_uid));

        if let Some(ref receiver) = order.receiver {
            println!("  Receiver:         {receiver}");
        }
        println!("  Executed sell:    {}", order.executed_sell_amount);
        println!("  Executed buy:     {}", order.executed_buy_amount);

        return Ok(());
    }

    // ── Option B: Query all orders for an address ────────────────────────────
    if let Ok(owner_str) = std::env::var("OWNER") {
        let owner: Address = owner_str.parse()?;

        println!("Fetching orders for {owner} on {:?}...", chain_id);
        println!();

        let request = GetOrdersRequest { owner, limit: Some(10), offset: Some(0) };

        let orders = api.get_orders(&request).await?;

        println!("Found {} orders (showing up to 10):", orders.len());
        println!();

        for (i, order) in orders.iter().enumerate() {
            println!(
                "  [{}] {:?} — {} {} → {} {}",
                i + 1,
                order.status,
                order.sell_amount,
                order.sell_token,
                order.buy_amount,
                order.buy_token,
            );
        }

        return Ok(());
    }

    // ── No env vars set ──────────────────────────────────────────────────────
    println!("Usage:");
    println!("  ORDER_UID=0x... cargo run --example order_status");
    println!("  OWNER=0x...     cargo run --example order_status");

    Ok(())
}
