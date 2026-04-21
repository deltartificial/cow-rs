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
//! # Bridge to Bitcoin via NEAR Intents
//!
//! Twin of `bridge_to_sol_via_near.rs` — same receiver-account flow,
//! Bitcoin destination instead of Solana. The `bridge_recipient` is a
//! bech32 / base58 Bitcoin address (the on-chain settlement happens
//! inside NEAR's relayer, so the Rust side just needs the string).
//!
//! Chain-id mapping: the workspace's `AdditionalTargetChainId::Bitcoin`
//! uses `1_000_000_000`. The underlying Defuse API uses its own
//! blockchain-key `"btc"` which
//! [`cow_bridging::near_intents::util::blockchain_key_to_chain_id`]
//! transcodes.
//!
//! ```sh
//! cargo run --example bridging_near_btc
//! ```

use alloy_primitives::{U256, address};
use cow_bridging::{
    BridgeProvider, QuoteBridgeRequest,
    near_intents::{NearIntentsBridgeProvider, NearIntentsProviderOptions},
};
use cow_types::OrderKind;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let usdc_mainnet = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let account = address!("5555555555555555555555555555555555555555");
    let btc_recipient = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"; // example BTC address

    let request = QuoteBridgeRequest {
        sell_chain_id: 1,            // Ethereum
        buy_chain_id: 1_000_000_000, // Bitcoin (workspace AdditionalTargetChainId)
        sell_token: usdc_mainnet,
        sell_token_decimals: 6,
        // Bitcoin has no token contract — use the empty raw variant.
        buy_token: cow_bridging::TokenAddress::Raw(String::new()),
        buy_token_decimals: 8,
        sell_amount: U256::from(500_000_000u64), // 500 USDC
        account,
        owner: None,
        receiver: None,
        bridge_recipient: Some(btc_recipient.into()),
        slippage_bps: 50,
        bridge_slippage_bps: Some(50),
        kind: OrderKind::Sell,
    };

    println!("NEAR Intents: EVM → Bitcoin");
    println!("  500 USDC on Mainnet → BTC on Bitcoin");
    println!("  Bridge recipient: {}", btc_recipient);

    let base_url = std::env::var("NEAR_INTENTS_BASE_URL").ok();
    let provider = NearIntentsBridgeProvider::new(NearIntentsProviderOptions {
        base_url,
        ..Default::default()
    });

    println!();
    println!("Provider info: {}", provider.info().name);
    println!("Supports route (1 → 1_000_000_000): {}", provider.supports_route(1, 1_000_000_000));

    // The rest of the flow mirrors `bridge_to_sol_via_near.rs`.
    println!();
    println!("See `bridge_to_sol_via_near.rs` for the quote + receiver-override pattern.");

    let _ = request;
    Ok(())
}
