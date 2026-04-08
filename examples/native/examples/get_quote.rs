//! # Get a Price Quote
//!
//! Fetches a swap quote from the CoW Protocol order book API without
//! submitting an order.  This is the Rust equivalent of the TypeScript:
//!
//! ```ts
//! const quoteAndPost = await sdk.getQuote({
//!     kind: OrderKind.SELL,
//!     sellToken: WETH.address,
//!     buyToken: USDC.address,
//!     amount: "1000000000000000000",
//!     ...
//! });
//! ```
//!
//! No private key is required — quotes are anonymous.
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example get_quote
//! ```

use alloy_primitives::Address;
use cow_rs::{
    Env, OrderBookApi, QuoteSide, SupportedChainId, order_book::OrderQuoteRequest,
    wrapped_native_currency,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Configuration ────────────────────────────────────────────────────────
    //
    // Tokens used in this example:
    //   - WETH on Mainnet (sell)
    //   - USDC on Mainnet (buy)
    let chain_id = SupportedChainId::Mainnet;
    let weth = wrapped_native_currency(chain_id);
    let usdc: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse()?;

    // Quotes don't require a funded wallet — use a dummy address.
    let from = Address::ZERO;

    // Sell exactly 1 WETH (18 decimals).
    let sell_amount = "1000000000000000000";

    // ── Build the quote request ──────────────────────────────────────────────
    //
    // `OrderQuoteRequest::new` fills in sensible defaults:
    //   - `sell_token_balance`: ERC-20
    //   - `price_quality`: Optimal
    //   - `signing_scheme`: EIP-712
    let request = OrderQuoteRequest::new(weth.address, usdc, from, QuoteSide::sell(sell_amount));

    println!("Requesting quote on {:?}:", chain_id);
    println!("  Sell: 1 {} ({sell_amount} atoms)", weth.symbol);
    println!("  Buy:  USDC");
    println!();

    // ── Fetch the quote ──────────────────────────────────────────────────────
    let api = OrderBookApi::new(chain_id, Env::Prod);
    let response = api.get_quote(&request).await?;

    // ── Display results ──────────────────────────────────────────────────────
    let quote = &response.quote;

    // Convert raw USDC atoms (6 decimals) to a human-readable amount.
    let buy_atoms: u128 = quote.buy_amount.parse()?;
    let usdc_human = buy_atoms as f64 / 1e6;

    println!("Quote received:");
    println!("  Sell amount:     {} atoms", quote.sell_amount);
    println!("  Buy amount:      {usdc_human:.2} USDC ({} atoms)", quote.buy_amount);
    println!("  Fee amount:      {} atoms", quote.fee_amount);
    println!("  Valid to:        {} (unix timestamp)", quote.valid_to);
    println!("  Kind:            {:?}", quote.kind);
    println!("  Fill or Kill:    {}", !quote.partially_fillable);
    println!("  Verified:        {}", response.verified);
    println!("  Expires:         {}", response.expiration);

    if let Some(ref fee_bps) = response.protocol_fee_bps {
        println!("  Protocol fee:    {fee_bps} bps");
    }
    if let Some(id) = response.id {
        println!("  Quote ID:        {id}");
    }

    // ── Buy-side quote example ───────────────────────────────────────────────
    //
    // Instead of "sell exactly X", you can request "buy exactly Y":
    println!();
    println!("--- Buy-side quote ---");

    let buy_request = OrderQuoteRequest::new(
        weth.address,
        usdc,
        from,
        QuoteSide::buy("2500000000"), // buy exactly 2500 USDC
    );

    let buy_response = api.get_quote(&buy_request).await?;
    let sell_atoms: u128 = buy_response.quote.sell_amount.parse()?;
    let weth_human = sell_atoms as f64 / 1e18;

    println!("  To buy 2500 USDC, sell: {weth_human:.6} WETH");
    println!("  Fee: {} atoms", buy_response.quote.fee_amount);

    Ok(())
}
