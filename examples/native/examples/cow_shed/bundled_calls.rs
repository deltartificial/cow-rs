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
//! # CowShed Bundled Pre-Interaction Calls
//!
//! CowShed is a user-owned proxy contract that executes an arbitrary
//! bundle of calls on behalf of the order owner. It's the mechanism
//! behind "do X, Y, Z before my order settles" workflows — approvals,
//! claims, unwrap, vault exits, etc.
//!
//! This example builds a 3-call bundle:
//!   1. `approve(VaultRelayer, MAX)` on USDC
//!   2. `transfer(to, 100)` on DAI (allowed to fail)
//!   3. `claimRewards()` on some rewards contract
//!
//! and encodes it as a `CowHook` that targets the user's CowShed proxy.
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example cow_shed_bundle
//! ```

use alloy_primitives::{B256, U256, address};
use cow_rs::{
    CowShedCall, CowShedHookParams, CowShedSdk, SupportedChainId, build_erc20_approve_calldata,
    build_erc20_transfer_calldata, cow_protocol_vault_relayer_address,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chain_id = SupportedChainId::Mainnet;

    // ── User and proxy addresses ─────────────────────────────────────────────
    //
    // The proxy is normally computed deterministically from the user EOA +
    // CowShed factory via CREATE2. For this example we use a placeholder.
    let user = address!("1111111111111111111111111111111111111111");
    let proxy = address!("2222222222222222222222222222222222222222");

    // ── Call 1: unlimited USDC approval for the Vault Relayer ────────────────
    let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let relayer = cow_protocol_vault_relayer_address(chain_id);
    let approve_cd = build_erc20_approve_calldata(relayer, U256::MAX);
    let call_approve = CowShedCall::new(usdc, approve_cd);

    // ── Call 2: token transfer that may fail without aborting the bundle ─────
    //
    // `allowing_failure()` is the CowShed primitive that lets a bundle
    // continue even if one leg reverts — useful when a claim might already
    // have been processed, for example.
    let dai = address!("6B175474E89094C44Da98b954EedeAC495271d0F");
    let recipient = address!("deaddeaddeaddeaddeaddeaddeaddeaddeaddead");
    let transfer_cd = build_erc20_transfer_calldata(recipient, U256::from(100u64));
    let call_transfer =
        CowShedCall::new(dai, transfer_cd).with_value(U256::ZERO).allowing_failure();

    // ── Call 3: arbitrary external call (e.g. claimRewards()) ────────────────
    //
    // `0x372500ab` is `claimRewards()` from a random rewards contract —
    // we pass raw bytes to illustrate that any 4-byte selector works.
    let rewards = address!("cccccccccccccccccccccccccccccccccccccccc");
    let claim_cd = vec![0x37, 0x25, 0x00, 0xab];
    let call_claim = CowShedCall::new(rewards, claim_cd);

    // ── Assemble the bundle ──────────────────────────────────────────────────
    //
    // `nonce` must be unique per user+proxy. In production, fetch the
    // proxy's `nonces(user)` value on-chain first.
    let nonce = B256::repeat_byte(0x01);
    let deadline = U256::from(
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() + 3600,
    );

    let params =
        CowShedHookParams::new(vec![call_approve, call_transfer, call_claim], nonce, deadline);

    println!("CowShed bundle:");
    println!("  User:      {}", user);
    println!("  Proxy:     {}", proxy);
    println!("  Calls:     {}", params.call_count());
    println!("  Nonce:     {}", params.nonce_ref());
    println!("  Deadline:  {}", params.deadline);
    for (i, call) in params.calls.iter().enumerate() {
        println!(
            "    [{i}] target={} value={} allow_failure={} calldata_len={}",
            call.target,
            call.value,
            call.allow_failure,
            call.calldata.len()
        );
    }
    println!();

    // ── Encode the hook ──────────────────────────────────────────────────────
    let sdk = CowShedSdk::new(chain_id.as_u64());
    if let Some(factory) = sdk.factory_address() {
        println!("CowShed factory on chain {}: {}", chain_id.as_u64(), factory);
    }

    let hook = sdk.build_hook(user, proxy, &params)?;
    println!();
    println!("CowHook (attach to OrderInteractionHooks::pre):");
    println!("  target:    {}", hook.target);
    println!("  gasLimit:  {}", hook.gas_limit);
    println!("  calldata:  0x{}...", &hook.call_data[..24]);

    Ok(())
}
