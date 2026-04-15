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
//! # EIP-2612 Permit Pre-Interaction Hook
//!
//! Signs an EIP-2612 permit so a CoW Protocol order can grant the
//! Vault Relayer its allowance atomically, in the same transaction
//! that settles the order — no separate `approve()` tx is needed.
//!
//! Produces:
//!   1. the 65-byte ECDSA signature over the EIP-712 permit digest
//!   2. the ABI-encoded `permit(...)` calldata (260 bytes)
//!   3. a `CowHook` ready to embed in the order's app-data `pre` hooks
//!
//! Everything happens offline — no RPC, no orderbook call.
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example permit_eip2612
//! ```

use alloy_primitives::{U256, address};
use alloy_signer_local::PrivateKeySigner;
use cow_chains::{SupportedChainId, cow_protocol_vault_relayer_address};
use cow_permit::{Erc20PermitInfo, PermitInfo, build_permit_hook};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Signer ───────────────────────────────────────────────────────────────
    //
    // Deterministic Anvil test key 0. Never ship a hard-coded key in real code.
    let signer: PrivateKeySigner =
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".parse()?;
    let owner = signer.address();

    // ── Permit target: USDC on Mainnet ───────────────────────────────────────
    //
    // USDC implements EIP-2612 with domain version "2" (not "1").
    // Every token has its own `name` / `version` — don't assume; read them
    // on-chain with `OnchainReader` in production.
    let chain_id = SupportedChainId::Mainnet;
    let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let spender = cow_protocol_vault_relayer_address(chain_id);

    // Allowance: exactly 1000 USDC (6 decimals).
    let allowance = U256::from(1_000_000_000u64);

    // Deadline: one hour from now. Reads of `block.timestamp` must be ≤ this.
    let deadline =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() + 3600;

    // ── Assemble the permit ──────────────────────────────────────────────────
    //
    // `nonce` must match the token's `nonces(owner)` view return. Here we
    // hard-code 0 — in production, fetch it via an `eth_call` first.
    let info = PermitInfo::new(usdc, owner, spender, allowance)
        .with_nonce(U256::ZERO)
        .with_deadline(deadline);

    let erc20_info = Erc20PermitInfo::new("USD Coin", "2", chain_id.as_u64());

    println!("Building EIP-2612 permit:");
    println!("  Token:    USDC ({})", info.token_address);
    println!("  Owner:    {}", info.owner);
    println!("  Spender:  {} (Vault Relayer)", info.spender);
    println!("  Value:    1000 USDC ({} atoms)", info.value);
    println!("  Nonce:    {}", info.nonce);
    println!("  Deadline: {} (unix)", info.deadline);
    println!();

    // ── Sign + encode the hook ───────────────────────────────────────────────
    let hook = build_permit_hook(&info, &erc20_info, &signer).await?;

    println!("Permit hook ready:");
    println!("  Target:       {}", hook.target);
    println!("  Calldata len: {} bytes", hook.calldata_len());
    println!("  Gas limit:    {}", hook.gas_limit);
    println!();

    // ── Wrap as a CowHook for app-data embedding ─────────────────────────────
    //
    // Drop this into `OrderInteractionHooks::pre` so the solver calls
    // `permit(...)` on USDC right before settling the order.
    let cow_hook = hook.into_cow_hook();
    println!("As CowHook (for OrderInteractionHooks::pre):");
    println!("  target:    {}", cow_hook.target);
    println!("  gasLimit:  {}", cow_hook.gas_limit);
    println!("  callData:  {}...", &cow_hook.call_data[..26]);

    Ok(())
}
