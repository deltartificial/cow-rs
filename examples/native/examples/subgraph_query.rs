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
//! # Subgraph Queries
//!
//! Queries historical protocol data from the CoW Protocol subgraph:
//! aggregate totals (orders, volume, fees) and time-series snapshots.
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example subgraph_query
//! ```

use cow_rs::{Env, SubgraphApi, SupportedChainId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chain_id = SupportedChainId::Mainnet;

    println!("Querying CoW Protocol subgraph on {:?}...", chain_id);
    println!();

    let api = SubgraphApi::new(chain_id, Env::Prod)?;

    // ── 1. Protocol totals ───────────────────────────────────────────────────
    //
    // Aggregate statistics since protocol launch.
    let totals = api.get_totals().await?;

    println!("=== Protocol Totals ===");
    if let Some(t) = totals.first() {
        println!("  Tokens:      {}", t.tokens);
        println!("  Orders:      {}", t.orders);
        println!("  Traders:     {}", t.traders);
        println!("  Settlements: {}", t.settlements);
        println!("  Volume USD:  ${}", t.volume_usd);
        println!("  Volume ETH:  {} ETH", t.volume_eth);
        println!("  Fees USD:    ${}", t.fees_usd);
        println!("  Fees ETH:    {} ETH", t.fees_eth);
    }
    println!();

    // ── 2. Last 7 days of daily volume ───────────────────────────────────────
    //
    // Returns one entry per day, most recent first.
    let daily = api.get_last_days_volume(7).await?;

    println!("=== Daily Volume (last 7 days) ===");
    for d in &daily {
        println!("  {} — ${}", d.timestamp, d.volume_usd);
    }
    println!();

    // ── 3. Last 24 hours of hourly volume ────────────────────────────────────
    let hourly = api.get_last_hours_volume(24).await?;

    println!("=== Hourly Volume (last 24 hours) ===");
    for h in &hourly {
        println!("  {} — ${}", h.timestamp, h.volume_usd);
    }

    Ok(())
}
