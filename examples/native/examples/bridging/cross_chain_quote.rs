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
//! # Cross-Chain Bridging Quote
//!
//! Builds a cross-chain bridge quote request through the `BridgingSdk`.
//! The SDK aggregates multiple providers (Bungee, Across, …) and returns
//! the best quote by `net_buy_amount` (buy amount minus bridge fee).
//!
//! This example:
//!   1. Constructs a 1000 USDC Mainnet → Arbitrum quote request
//!   2. Registers the Bungee provider with an API key
//!   3. Shows how to call `get_best_quote` (commented out — needs network)
//!
//! ## Usage
//!
//! ```sh
//! BUNGEE_API_KEY=your-key cargo run --example bridging_quote
//! ```
//!
//! Without `BUNGEE_API_KEY`, the example falls back to a placeholder and
//! skips the network call.

use alloy_primitives::{U256, address};
use cow_rs::{BridgingSdk, OrderKind, QuoteBridgeRequest, SupportedChainId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sell_chain = SupportedChainId::Mainnet;
    let buy_chain = SupportedChainId::ArbitrumOne;

    // Same token symbol on both chains, but different contract addresses.
    let usdc_mainnet = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let usdc_arbitrum = address!("af88d065e77c8cC2239327C5EDb3A432268e5831");

    // Hard-coded trader address — purely illustrative.
    let account = address!("3333333333333333333333333333333333333333");

    // ── Build the quote request ──────────────────────────────────────────────
    //
    // 1000 USDC (6 decimals). `slippage_bps = 50` means 0.5%.
    let request = QuoteBridgeRequest {
        sell_chain_id: sell_chain.as_u64(),
        buy_chain_id: buy_chain.as_u64(),
        sell_token: usdc_mainnet,
        sell_token_decimals: 6,
        buy_token: usdc_arbitrum,
        buy_token_decimals: 6,
        sell_amount: U256::from(1_000_000_000u64), // 1000 USDC
        account,
        owner: None,
        receiver: None,
        bridge_recipient: None,
        slippage_bps: 50,
        bridge_slippage_bps: None,
        kind: OrderKind::Sell,
    };

    println!("Cross-chain bridge quote request:");
    println!("  From:      chain {} (Mainnet)", request.sell_chain_id);
    println!("  To:        chain {} (Arbitrum One)", request.buy_chain_id);
    println!("  Sell:      1000 USDC ({} atoms)", request.sell_amount);
    println!("  Slippage:  {} bps", request.slippage_bps);
    println!();

    // ── Register providers ───────────────────────────────────────────────────
    //
    // `BridgingSdk::with_bungee` adds the Bungee (Socket) provider. Additional
    // providers can be added via `add_provider`.
    let api_key = std::env::var("BUNGEE_API_KEY").unwrap_or_else(|_| "demo-key".to_owned());
    let sdk = BridgingSdk::new().with_bungee(&api_key);

    println!("BridgingSdk: {} provider(s) registered", sdk.provider_count());

    // ── Fetch the best quote ─────────────────────────────────────────────────
    //
    // This is a real network call. Gate it on a real key so CI never hits it.
    if std::env::var("BUNGEE_API_KEY").is_ok() {
        match sdk.get_best_quote(&request).await {
            Ok(quote) => {
                println!();
                println!("Best quote:");
                println!("  Provider:   {}", quote.provider_ref());
                println!("  Sell:       {} atoms", quote.sell_amount);
                println!("  Buy:        {} atoms", quote.buy_amount);
                println!("  Fee:        {} atoms", quote.fee_amount);
                println!("  Net buy:    {} atoms", quote.net_buy_amount());
                println!("  ETA:        ~{} seconds", quote.estimated_secs);
                println!("  Has hook:   {}", quote.has_bridge_hook());
            }
            Err(e) => {
                println!();
                println!("Quote failed: {e}");
            }
        }
    } else {
        println!();
        println!("(Skipping network call — set BUNGEE_API_KEY to fetch a real quote.)");
    }

    Ok(())
}
