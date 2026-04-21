#![allow(
    clippy::allow_attributes_without_reason,
    clippy::disallowed_macros,
    clippy::print_stdout,
    clippy::disallowed_methods,
    clippy::uninlined_format_args,
    clippy::literal_string_with_formatting_args,
    clippy::doc_markdown,
    clippy::type_complexity,
    clippy::missing_assert_message,
    clippy::print_literal,
    clippy::redundant_clone
)]
//! # Cross-Chain Post via a Hook-Based Bridge
//!
//! Walks the full quote → post flow for a hook-based bridge
//! (Across / Bungee). The quote request targets USDC on Arbitrum from
//! USDC on Mainnet; the orchestrator picks the intermediate swap
//! token, fetches a bridge quote, signs the post-hook with
//! `cow-shed`, then posts the order through `TradingSdk`.
//!
//! This example is **gated on environment variables** — no network is
//! hit unless the caller provides real credentials:
//!
//! ```sh
//! BUNGEE_API_KEY=your-key \
//! PRIV_KEY=0xabcdef... \
//! cargo run --example bridging_cross_chain_post
//! ```
//!
//! Without `BUNGEE_API_KEY` the example prints the request + provider
//! setup and exits cleanly, making it safe for CI / doc rendering.

use alloy_primitives::{U256, address};
use cow_bridging::{
    BridgingSdk, QuoteBridgeRequest, bungee::BungeeProvider, sdk::GetQuoteWithBridgeParams,
};
use cow_chains::SupportedChainId;
use cow_types::OrderKind;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── 1. Request shape ────────────────────────────────────────────────
    let sell_chain = SupportedChainId::Mainnet;
    let buy_chain = SupportedChainId::ArbitrumOne;
    let usdc_mainnet = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let usdc_arbitrum = address!("af88d065e77c8cC2239327C5EDb3A432268e5831");
    let account = address!("3333333333333333333333333333333333333333");

    let request = QuoteBridgeRequest {
        sell_chain_id: sell_chain.as_u64(),
        buy_chain_id: buy_chain.as_u64(),
        sell_token: usdc_mainnet,
        sell_token_decimals: 6,
        buy_token: usdc_arbitrum.into(),
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

    println!("Cross-chain post request");
    println!(
        "  {} USDC on chain {} → chain {}",
        request.sell_amount,
        sell_chain.as_u64(),
        buy_chain.as_u64()
    );
    println!("  slippage: {} bps", request.slippage_bps);

    // ── 2. Providers ────────────────────────────────────────────────────
    //
    // Hook-based bridge (Bungee via BungeeProvider). Register directly
    // through `BridgingSdk::with_bungee` or construct a
    // `BungeeProvider` explicitly for custom options / a CowShedSdk
    // signer.
    let Ok(api_key) = std::env::var("BUNGEE_API_KEY") else {
        println!();
        println!("Skipping quote — set BUNGEE_API_KEY to actually hit the network.");
        return Ok(());
    };
    let _sdk = BridgingSdk::new().with_bungee(&api_key);
    let provider = BungeeProvider::new(&api_key);

    // ── 3. Orchestrate the quote ────────────────────────────────────────
    //
    // `get_quote_with_bridge` dispatches to the hook branch (Bungee
    // implements `HookBridgeProvider`). Pass a `&dyn SwapQuoter`
    // (typically `cow_rs::TradingSwapQuoter` wrapping your
    // `TradingSdk`). This example skips the real quote because
    // constructing a `TradingSdk` needs a signer — the README at
    // `examples/native/README.md` has a complete sample.
    let params = GetQuoteWithBridgeParams {
        swap_and_bridge_request: request.clone(),
        slippage_bps: 50,
        advanced_settings_metadata: None,
        quote_signer: None,
        hook_deadline: None,
    };
    println!();
    println!("Would call `get_quote_with_bridge(&params, &provider, &quoter)` here.");
    println!("`quoter` is typically `cow_rs::TradingSwapQuoter::new(Arc::new(trading_sdk))`.");
    println!();
    println!("After a successful quote, post the order via");
    println!("  `cow_rs::cross_chain_post::post_cross_chain_order(ctx)`");
    println!("which signs the real bridge hook and submits the order to the orderbook.");

    // Suppress the unused-variable warnings; in the real flow the
    // quoter is threaded here.
    let _ = (params, provider);

    Ok(())
}
