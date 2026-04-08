//! # Post a Limit Order
//!
//! Submits a limit order with exact sell/buy amounts to the CoW Protocol.
//! Unlike market swaps, the buy amount is fixed — the order only fills if
//! the on-chain price meets or exceeds the specified limit.
//!
//! Limit orders support partial fills, meaning the order can be executed
//! in multiple batches as liquidity becomes available.
//!
//! ## Usage
//!
//! ```sh
//! COW_PRIVATE_KEY=0x... cargo run --example limit_order
//! ```
//!
//! **WARNING**: This example submits a real order on Sepolia testnet.

use alloy_primitives::U256;
use cow_rs::{
    LimitTradeParameters, OrderKind, SupportedChainId, TradingSdk, TradingSdkConfig,
    order_explorer_link, wrapped_native_currency,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Configuration ────────────────────────────────────────────────────────
    let chain_id = SupportedChainId::Sepolia;
    let weth = wrapped_native_currency(chain_id);

    // Sepolia USDC test token (18 decimals on testnet).
    let usdc: alloy_primitives::Address = "0xbe72E441BF55620febc26715db68d3494213D8Cb".parse()?;

    let private_key = std::env::var("COW_PRIVATE_KEY")
        .map_err(|_| "Set COW_PRIVATE_KEY env var (0x-prefixed hex private key)")?;

    // ── Create SDK ───────────────────────────────────────────────────────────
    let sdk = TradingSdk::new(TradingSdkConfig::prod(chain_id, "CowRsLimitExample"), &private_key)?;

    // ── Build limit order parameters ─────────────────────────────────────────
    //
    // Key difference from swap: you specify exact sell AND buy amounts.
    // The order only fills at this price or better (no slippage parameter).
    let sell_amount = U256::from(10_000_000_000_000_000_u64); // 0.01 WETH
    let buy_amount = U256::from(25_000_000_000_000_000_000_u128); // 25 USDC (18 dec)

    let params = LimitTradeParameters {
        kind: OrderKind::Sell,
        sell_token: weth.address,
        buy_token: usdc,
        sell_amount,
        buy_amount,
        receiver: None,        // tokens go to the signer
        valid_for: Some(3600), // 1 hour validity
        valid_to: None,
        partially_fillable: true, // allow partial fills
        app_data: None,           // default zero app-data
        partner_fee: None,
    };

    println!("Posting limit order on {:?}:", chain_id);
    println!("  Sell:     0.01 {} (exact)", weth.symbol);
    println!("  Buy:      >= 25 USDC");
    println!("  Validity: 1 hour");
    println!("  Partial:  yes");
    println!();

    // ── Submit order ─────────────────────────────────────────────────────────
    let result = sdk.post_limit_order(params, None).await?;

    let explorer = order_explorer_link(chain_id, &result.order_id);

    println!("Limit order posted!");
    println!("  Order ID:       {}", result.order_id);
    println!("  Signing scheme: {:?}", result.signing_scheme);
    println!("  Explorer:       {explorer}");

    Ok(())
}
