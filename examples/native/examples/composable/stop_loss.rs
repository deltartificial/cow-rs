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
//! # Composable Orders: Stop-Loss
//!
//! Builds a stop-loss conditional order that triggers when the
//! WETH/USDC price (reported by Chainlink oracles) falls to or below
//! a strike price. Unlike TWAP, this is a *reactive* order — it only
//! becomes valid once the price condition is met.
//!
//! The `StopLoss` handler reads prices from two Chainlink-compatible
//! feeds and compares them against the 18-decimal `strike_price` field.
//!
//! This example:
//!   1. Configures a 10 WETH → USDC stop-loss at ~2000 USDC/WETH
//!   2. Encodes the 416-byte `staticInput` for the handler
//!   3. Builds `ConditionalOrderParams` ready for `ComposableCow.create`
//!   4. Round-trips through `decode_stop_loss_static_input` to verify
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example composable_stop_loss
//! ```

use alloy_primitives::{Address, B256, U256, address};
use cow_rs::{
    STOP_LOSS_HANDLER_ADDRESS, StopLossData, StopLossOrder, SupportedChainId,
    decode_stop_loss_static_input, encode_stop_loss_struct, wrapped_native_currency,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chain_id = SupportedChainId::Mainnet;
    let weth = wrapped_native_currency(chain_id);
    let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");

    // ── Chainlink feeds on Mainnet ───────────────────────────────────────────
    //
    // These are the canonical ETH/USD and USDC/USD price feeds. The handler
    // divides one by the other to derive the WETH/USDC spot price.
    let eth_usd_feed = address!("5f4eC3Df9cbd43714FE2740f5E3616155c5b8419");
    let usdc_usd_feed = address!("8fFfFfd4AfB6115b954Bd326cbe7B4BA576818f6");

    // ── Stop-loss parameters ─────────────────────────────────────────────────
    //
    // Sell 10 WETH for at least 18 000 USDC if the price falls to 1800 USD.
    // `strike_price` is 18-decimal fixed-point: 1800e18.
    let sell_amount = U256::from(10u64) * U256::from(10u64).pow(U256::from(18u64));
    let buy_amount = U256::from(18_000u64) * U256::from(10u64).pow(U256::from(6u64));
    let strike = U256::from(1_800u64) * U256::from(10u64).pow(U256::from(18u64));

    let valid_to: u32 =
        (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() +
            7 * 24 * 3600) // one week
            .try_into()?;

    let data = StopLossData {
        sell_token: weth.address,
        buy_token: usdc,
        sell_amount,
        buy_amount,
        app_data: B256::ZERO,
        receiver: Address::ZERO, // = order owner
        is_sell_order: true,
        is_partially_fillable: false,
        valid_to,
        strike_price: strike,
        sell_token_price_oracle: eth_usd_feed,
        buy_token_price_oracle: usdc_usd_feed,
        token_amount_in_eth: false,
    };

    let order = StopLossOrder::new(data);

    println!("Stop-loss order:");
    println!("  Sell:         10 WETH");
    println!("  Buy (min):    18 000 USDC");
    println!("  Strike:       1800 USD/ETH (18-decimal: {})", order.data.strike_price);
    println!("  Sell feed:    {} (Chainlink ETH/USD)", order.data.sell_token_price_oracle);
    println!("  Buy feed:     {} (Chainlink USDC/USD)", order.data.buy_token_price_oracle);
    println!("  Valid to:     {} (unix)", order.data.valid_to);
    println!("  Valid:        {}", order.is_valid());
    println!("  Salt:         {}", order.salt);
    println!();

    // ── Encode for on-chain submission ───────────────────────────────────────
    //
    // 13 fields × 32 bytes = 416-byte static input passed to the handler.
    let static_input = encode_stop_loss_struct(&order.data);
    println!("Encoded staticInput: {} bytes", static_input.len());
    assert_eq!(static_input.len(), 416);

    let params = order.to_params()?;
    println!();
    println!("ConditionalOrderParams (for ComposableCow.create):");
    println!("  handler:       {}", params.handler);
    assert_eq!(params.handler, STOP_LOSS_HANDLER_ADDRESS);
    println!("  salt:          {}", params.salt);
    println!("  static_input:  {} bytes", params.static_input.len());
    println!();

    // ── Round-trip decode (sanity check) ─────────────────────────────────────
    let decoded = decode_stop_loss_static_input(&static_input)?;
    assert_eq!(decoded.sell_amount, order.data.sell_amount);
    assert_eq!(decoded.strike_price, order.data.strike_price);
    assert_eq!(decoded.sell_token_price_oracle, eth_usd_feed);
    println!("Round-trip decode: OK");

    Ok(())
}
