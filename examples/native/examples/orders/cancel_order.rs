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
//! # Cancel an Order
//!
//! Demonstrates off-chain order cancellation via the order book API.
//!
//! CoW Protocol supports two cancellation methods:
//!
//! - **Off-chain** (soft cancel): Sign an EIP-712 cancellation message and submit it to the API.
//!   Free and fast — the order is removed from the solver auction.  This is what
//!   `tradingSdk.offChainCancelOrder()` does in the TypeScript SDK.
//!
//! - **On-chain** (hard cancel): Call `invalidateOrder()` on the settlement contract.  Costs gas
//!   but provides an on-chain guarantee.
//!
//! This example implements the off-chain path.
//!
//! ## Usage
//!
//! ```sh
//! COW_PRIVATE_KEY=0x... ORDER_UID=0x... cargo run --example cancel_order
//! ```

use alloy_signer_local::PrivateKeySigner;
use cow_chains::{Env, SupportedChainId};
use cow_orderbook::{OrderBookApi, OrderCancellations};
use cow_signing::sign_order_cancellations;
use cow_types::EcdsaSigningScheme;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Configuration ────────────────────────────────────────────────────────
    let chain_id = SupportedChainId::Sepolia;

    let private_key = std::env::var("COW_PRIVATE_KEY")
        .map_err(|_| "Set COW_PRIVATE_KEY env var (0x-prefixed hex private key)")?;

    let order_uid = std::env::var("ORDER_UID")
        .map_err(|_| "Set ORDER_UID env var (the 0x-prefixed order UID to cancel)")?;

    let signer: PrivateKeySigner = private_key.parse()?;

    println!("Cancelling order on {:?}:", chain_id);
    println!("  Order UID: {order_uid}");
    println!("  Signer:    {}", signer.address());
    println!();

    // ── 1. Sign the cancellation message ─────────────────────────────────────
    //
    // The cancellation is an EIP-712 signed message over the order UID(s).
    // You can cancel multiple orders in a single call.
    let sig = sign_order_cancellations(
        &[order_uid.as_str()],
        chain_id as u64,
        &signer,
        EcdsaSigningScheme::Eip712,
    )
    .await?;

    println!("Cancellation signed:");
    println!("  Signature: {}...", &sig.signature[..20]);
    println!("  Scheme:    {:?}", sig.signing_scheme);
    println!();

    // ── 2. Submit to the order book API ──────────────────────────────────────
    let api = OrderBookApi::new(chain_id, Env::Prod);
    let cancellation = OrderCancellations::new(
        vec![order_uid.clone()],
        &sig.signature,
        EcdsaSigningScheme::Eip712,
    );

    api.cancel_orders(&cancellation).await?;

    println!("Order {order_uid} cancelled successfully!");
    println!();
    println!("Tip: Verify by querying the order status:");
    println!("  curl https://api.cow.fi/sepolia/api/v1/orders/{order_uid}");

    Ok(())
}
