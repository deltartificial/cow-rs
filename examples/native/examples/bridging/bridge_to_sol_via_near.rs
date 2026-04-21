#![allow(
    clippy::allow_attributes_without_reason,
    clippy::disallowed_macros,
    clippy::print_stdout,
    clippy::disallowed_methods,
    clippy::uninlined_format_args,
    clippy::literal_string_with_formatting_args,
    clippy::doc_markdown,
    clippy::missing_assert_message,
    clippy::print_literal,
    clippy::type_complexity
)]
//! # Bridge to Solana via NEAR Intents
//!
//! NEAR Intents is a *receiver-account* bridge: the `CoW` order
//! transfers the swap output to a deposit address the NEAR API
//! allocates, and NEAR relays it to Solana. Unlike hook-based bridges
//! (Across, Bungee) there is no on-chain post-hook — attestation
//! verification replaces it as the integrity guarantee.
//!
//! This example:
//!   1. Builds a quote request — 100 USDC on Ethereum → native SOL.
//!   2. Constructs a `NearIntentsBridgeProvider`.
//!   3. Shows how to call `get_quote` and retrieve the deposit address later via
//!      `get_bridge_receiver_override`.
//!
//! Non-EVM destinations use `Address::ZERO` as a sentinel on the
//! `QuoteBridgeRequest.buy_token` — the real Solana recipient rides
//! through `bridge_recipient`.
//!
//! ```sh
//! cargo run --example bridging_near_sol
//! ```
//!
//! The example is self-contained and does **not** hit the network unless
//! you point it at a wiremock / staging URL via `NEAR_INTENTS_BASE_URL`.

use alloy_primitives::{Address, U256, address};
use cow_bridging::{
    BridgeProvider, QuoteBridgeRequest,
    near_intents::{NearIntentsBridgeProvider, NearIntentsProviderOptions},
};
use cow_types::OrderKind;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── 1. Request: 100 USDC on Mainnet → SOL on Solana ────────────────
    let usdc_mainnet = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let account = address!("4444444444444444444444444444444444444444");
    let sol_recipient = "7Np41oeYqPefeNQEHSv1UDhYrehxin3NStELsSKCT4K2"; // example SOL address

    let request = QuoteBridgeRequest {
        sell_chain_id: 1,            // Ethereum
        buy_chain_id: 1_000_000_001, // Solana (workspace AdditionalTargetChainId)
        sell_token: usdc_mainnet,
        sell_token_decimals: 6,
        buy_token: Address::ZERO, // non-EVM → ZERO sentinel
        buy_token_decimals: 9,
        sell_amount: U256::from(100_000_000u64), // 100 USDC
        account,
        owner: None,
        receiver: None,
        bridge_recipient: Some(sol_recipient.into()),
        slippage_bps: 50,
        bridge_slippage_bps: Some(50),
        kind: OrderKind::Sell,
    };

    println!("NEAR Intents: EVM → Solana");
    println!("  100 USDC on Mainnet → SOL on Solana");
    println!("  Bridge recipient: {}", sol_recipient);

    // ── 2. Provider setup ──────────────────────────────────────────────
    let base_url = std::env::var("NEAR_INTENTS_BASE_URL").ok();
    let options = NearIntentsProviderOptions { base_url, ..Default::default() };
    let provider = NearIntentsBridgeProvider::new(options);

    println!();
    println!("Provider info: {}", provider.info().name);
    println!("Supports route (1 → 1_000_000_001): {}", provider.supports_route(1, 1_000_000_001));

    // ── 3. Quote + receiver override ───────────────────────────────────
    //
    // The flow looks like this (pseudo-code):
    //
    //   let response = provider.get_quote(&request).await?;
    //   // attestation verified inside `get_quote` — mismatches
    //   // rejected with `BridgeError::QuoteDoesNotMatchDepositAddress`.
    //
    //   let deposit_addr = provider
    //       .as_receiver_account_bridge_provider()
    //       .unwrap()
    //       .get_bridge_receiver_override(&request, &response)
    //       .await?;
    //   // `deposit_addr` is the EVM address the user's CoW order must
    //   // transfer the USDC output to. NEAR relays it to Solana.
    //
    // The TradingSdk post then uses that `deposit_addr` as the order's
    // `receiver` and submits via `post_swap_order_from_quote`. No
    // `cow-shed` signing step — receiver-account bridges need no hook.
    println!();
    println!("Real network call skipped — see README for a full end-to-end script.");

    // Suppress unused-variable warnings.
    let _ = request;
    Ok(())
}
