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
//! # Composable Orders: TWAP
//!
//! Demonstrates building a Time-Weighted Average Price (TWAP) order.
//! TWAP orders split a large trade into smaller parts executed over time,
//! reducing price impact.
//!
//! For example, selling 10 WETH as 5 parts over 1 hour means:
//! - 5 individual orders of 2 WETH each
//! - One part becomes active every 12 minutes
//! - Each part fills independently at the best available price
//!
//! This example builds and ABI-encodes the TWAP struct (offline), which
//! can then be submitted to the ComposableCow contract on-chain.
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example composable_twap
//! ```

use alloy_primitives::{Address, B256, U256};
use cow_rs::{
    COMPOSABLE_COW_ADDRESS, DurationOfPart, OrderKind, SupportedChainId, TWAP_HANDLER_ADDRESS,
    TwapData, TwapOrder, TwapStartTime, data_to_struct, decode_twap_struct, encode_twap_struct,
    struct_to_data, wrapped_native_currency,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chain_id = SupportedChainId::Mainnet;
    let weth = wrapped_native_currency(chain_id);
    let usdc: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse()?;

    // ── 1. Build TWAP data ───────────────────────────────────────────────────
    //
    // Sell 10 WETH over 1 hour in 5 parts (2 WETH each, 12 min apart).
    let twap_data = TwapData {
        sell_token: weth.address,
        buy_token: usdc,
        receiver: Address::ZERO, // tokens go to the order owner
        sell_amount: U256::from(10_000_000_000_000_000_000_u128), // 10 WETH total
        buy_amount: U256::from(25_000_000_000_u64), // min 25000 USDC total
        start_time: TwapStartTime::AtMiningTime, // start immediately
        part_duration: 720,      // 12 min per part
        num_parts: 5,
        app_data: B256::ZERO,
        partially_fillable: false,
        kind: OrderKind::Sell,
        duration_of_part: DurationOfPart::Auto, // each part valid for full window
    };

    println!("=== TWAP Order Configuration ===");
    println!("  Sell:          10 {} (total)", weth.symbol);
    println!("  Buy:           >= 25000 USDC (total)");
    println!("  Parts:         {}", twap_data.num_parts);
    println!("  Per part:      {} WETH", 10.0 / twap_data.num_parts as f64);
    println!(
        "  Part duration: {} seconds ({} min)",
        twap_data.part_duration,
        twap_data.part_duration / 60
    );
    println!(
        "  Total time:    {} seconds ({} min)",
        twap_data.part_duration * twap_data.num_parts,
        twap_data.part_duration * twap_data.num_parts / 60,
    );
    println!("  Start:         {:?}", twap_data.start_time);
    println!();

    // ── 2. Create the TWAP order (with deterministic salt) ───────────────────
    let order = TwapOrder::new(twap_data.clone());
    println!("TWAP Order:");
    println!("  Salt: {}", order.salt);
    println!();

    // ── 3. Convert to on-chain struct and ABI-encode ─────────────────────────
    //
    // TwapData (high-level) → TwapStruct (on-chain format) → ABI bytes.
    // The struct splits total amounts into per-part amounts.
    let twap_struct = data_to_struct(&twap_data)?;
    println!("On-chain struct (per-part):");
    println!("  part_sell_amount: {}", twap_struct.part_sell_amount);
    println!("  min_part_limit:   {}", twap_struct.min_part_limit);
    println!("  n (parts):        {}", twap_struct.n);
    println!("  t (duration):     {} sec", twap_struct.t);
    println!();

    let encoded = encode_twap_struct(&twap_struct);
    println!("ABI-encoded TWAP ({} bytes):", encoded.len());
    println!("  0x{}...", alloy_primitives::hex::encode(&encoded[..64.min(encoded.len())]));
    println!();

    // ── 4. Decode round-trip ─────────────────────────────────────────────────
    let decoded_struct = decode_twap_struct(&encoded)?;
    let decoded_data = struct_to_data(&decoded_struct);

    assert_eq!(decoded_data.sell_token, twap_data.sell_token);
    assert_eq!(decoded_data.buy_token, twap_data.buy_token);
    assert_eq!(decoded_data.num_parts, twap_data.num_parts);
    assert_eq!(decoded_data.sell_amount, twap_data.sell_amount);
    println!("ABI round-trip: OK");
    println!();

    // ── 5. Relevant contract addresses ───────────────────────────────────────
    println!("=== Contract Addresses ===");
    println!("  ComposableCow: {COMPOSABLE_COW_ADDRESS}");
    println!("  TWAP Handler:  {TWAP_HANDLER_ADDRESS}");

    Ok(())
}
