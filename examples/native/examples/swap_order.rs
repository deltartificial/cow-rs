//! # Post a Swap Order
//!
//! Submits a market sell order to the CoW Protocol using the high-level
//! [`TradingSdk`].  This is the Rust equivalent of the TypeScript:
//!
//! ```ts
//! const sdk = new TradingSdk({ chainId, appCode: 'MyApp', signer });
//! const quoteAndPost = await sdk.getQuote({
//!     kind: OrderKind.SELL,
//!     owner,
//!     amount: "10000000000000000",
//!     sellToken: WETH.address,
//!     buyToken: USDC.address,
//!     slippageBps: 50,
//!     ...
//! });
//! const result = await quoteAndPost.postSwapOrderFromQuote({});
//! ```
//!
//! ## Usage
//!
//! ```sh
//! COW_PRIVATE_KEY=0x... cargo run --example swap_order
//! ```
//!
//! **WARNING**: This example submits a real order on Sepolia testnet.
//! Make sure your wallet holds Sepolia WETH.

use alloy_primitives::U256;
use cow_rs::{
    OrderKind, SupportedChainId, TradeParameters, TradingSdk, TradingSdkConfig,
    order_explorer_link, wrapped_native_currency,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Configuration ────────────────────────────────────────────────────────
    let chain_id = SupportedChainId::Sepolia;
    let weth = wrapped_native_currency(chain_id);

    // Sepolia USDC test token.
    let usdc: alloy_primitives::Address = "0xbe72E441BF55620febc26715db68d3494213D8Cb".parse()?;

    let private_key = std::env::var("COW_PRIVATE_KEY")
        .map_err(|_| "Set COW_PRIVATE_KEY env var (0x-prefixed hex private key)")?;

    // ── Create SDK ───────────────────────────────────────────────────────────
    //
    // `TradingSdkConfig::prod` sets the production API URLs and default
    // slippage (50 bps = 0.5%).  The private key is used for EIP-712 signing.
    let sdk = TradingSdk::new(TradingSdkConfig::prod(chain_id, "CowRsSwapExample"), &private_key)?;

    // ── Build trade parameters ───────────────────────────────────────────────
    //
    // Sell 0.01 WETH for USDC with 0.5% slippage tolerance.
    // The SDK handles:
    //   1. Fetching a quote from the order book
    //   2. Applying slippage to the buy amount
    //   3. Signing the order with EIP-712
    //   4. Submitting the signed order
    let sell_amount = U256::from(10_000_000_000_000_000_u64); // 0.01 WETH

    let params = TradeParameters {
        kind: OrderKind::Sell,
        sell_token: weth.address,
        sell_token_decimals: weth.decimals,
        buy_token: usdc,
        buy_token_decimals: 18,
        amount: sell_amount,
        slippage_bps: Some(50), // 0.5% slippage tolerance
        receiver: None,         // tokens go to the signer
        valid_for: None,        // default: 30 minutes
        valid_to: None,
        partially_fillable: None, // fill-or-kill
        partner_fee: None,
    };

    println!("Posting swap order on {:?}:", chain_id);
    println!("  Sell:     0.01 {}", weth.symbol);
    println!("  Buy:      USDC");
    println!("  Slippage: 0.5%");
    println!();

    // ── Submit order ─────────────────────────────────────────────────────────
    let result = sdk.post_swap_order(params).await?;

    let explorer = order_explorer_link(chain_id, &result.order_id);

    println!("Order posted successfully!");
    println!("  Order ID:       {}", result.order_id);
    println!("  Signing scheme: {:?}", result.signing_scheme);
    println!("  Signature:      {}...", &result.signature[..20]);
    println!("  Explorer:       {explorer}");

    Ok(())
}
