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
//! # Flash Loan Pre-Interaction Hook (Balancer)
//!
//! Builds a `CowHook` that triggers a Balancer Vault flash loan before
//! the order settles. The receiver contract must implement
//! `IFlashLoanRecipient.receiveFlashLoan(...)` and repay the loan +
//! premium in the same transaction.
//!
//! Balancer flash loans are free (zero premium), which is why the CoW
//! solver uses them as the default provider for composable strategies
//! that need upfront capital (e.g. leverage, collateral swaps).
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example flash_loan_balancer
//! ```

use alloy_primitives::{U256, address};
use cow_flash_loans::{FlashLoanParams, FlashLoanProvider, FlashLoanSdk};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Flash loan: 100 000 DAI on Mainnet ───────────────────────────────────
    //
    // Any ERC-20 held by the Balancer Vault can be borrowed. DAI is one of
    // the deepest — suitable for large flash loans.
    let dai = address!("6B175474E89094C44Da98b954EedeAC495271d0F");
    let amount = U256::from(100_000u64) * U256::from(10u64).pow(U256::from(18u64));

    // The receiver is a user-deployed contract that implements
    // `IFlashLoanRecipient`. Its fallback / hook logic is responsible for
    // using the borrowed funds and repaying the Vault in the same tx.
    let receiver = address!("4242424242424242424242424242424242424242");

    let params = FlashLoanParams::new(
        FlashLoanProvider::Balancer,
        dai,
        amount,
        1, // mainnet
    );

    println!("Flash loan configuration:");
    println!("  Provider:     {}", params.provider_name());
    println!("  Token:        DAI ({})", params.token);
    println!("  Amount:       100 000 DAI ({} atoms)", params.amount);
    println!("  Chain:        {}", params.chain_id);
    println!("  Supported:    {}", params.is_provider_supported());
    println!();

    // ── Build the pre-interaction hook ───────────────────────────────────────
    //
    // `user_data` is opaque bytes forwarded to the receiver's callback.
    // Use it to pass trade context (target price, collateral ratio, etc.).
    let user_data: &[u8] = b"stop-loss-unwind-v1";
    let hook = FlashLoanSdk::build_flash_loan_hook(&params, receiver, user_data)?;

    println!("Flash-loan CowHook (attach to order pre-interactions):");
    println!("  target:     {}", hook.target);
    println!("  gasLimit:   {}", hook.gas_limit);
    println!("  calldata:   0x{}...", &hook.call_data[..24]);
    println!(
        "  calldata len: {} hex chars ({} bytes)",
        hook.call_data.len(),
        hook.call_data.len() / 2
    );
    println!();

    // ── Sanity check: encode the raw calldata directly ───────────────────────
    //
    // `build_flash_loan_hook` wraps this plus a gas estimate and contract lookup.
    let raw = FlashLoanSdk::encode_balancer_flash_loan(receiver, dai, amount, user_data);
    println!("Raw Balancer.flashLoan calldata: {} bytes", raw.len());
    println!("Selector: 0x{:02x}{:02x}{:02x}{:02x}", raw[0], raw[1], raw[2], raw[3]);

    // ── Example of an unsupported provider ───────────────────────────────────
    //
    // Aave V3 is a supported *provider* but the calldata encoder is not yet
    // implemented — the SDK surfaces this clearly via `CowError::Unsupported`.
    let aave_params = FlashLoanParams::new(FlashLoanProvider::AaveV3, dai, amount, 1);
    match FlashLoanSdk::build_flash_loan_hook(&aave_params, receiver, &[]) {
        Ok(_) => println!("(unexpected: aave v3 succeeded)"),
        Err(e) => println!("\nAave V3 attempt (expected failure): {e}"),
    }

    Ok(())
}
