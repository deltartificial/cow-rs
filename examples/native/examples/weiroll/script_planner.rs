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
//! # Weiroll Script Planner
//!
//! Weiroll is a tiny VM that executes a sequence of packed 32-byte
//! commands against a state array — think "calldata-level batching"
//! without deploying a custom contract. CoW uses it to chain multiple
//! interactions inside a single pre- or post-hook.
//!
//! This example plans a two-step script:
//!   1. `balanceOf(account)` on USDC (STATICCALL, result → state slot)
//!   2. `transfer(recipient, <from state>)` on USDC (CALL)
//!
//! The output is the exact `execute(bytes32[],bytes[])` calldata you
//! would send to the Weiroll executor contract.
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example weiroll_script
//! ```

use alloy_primitives::{Address, address, hex, keccak256};
use cow_weiroll::{
    WEIROLL_ADDRESS, WeirollCommand, WeirollCommandFlags, WeirollPlanner,
    create_weiroll_delegate_call,
};

fn selector(sig: &[u8]) -> [u8; 4] {
    let h = keccak256(sig);
    [h[0], h[1], h[2], h[3]]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    let account = address!("1111111111111111111111111111111111111111");
    let recipient = address!("2222222222222222222222222222222222222222");

    println!("Planning a 2-command Weiroll script on USDC ({})", usdc);
    println!();

    // ── Build the script directly ────────────────────────────────────────────
    //
    // State slot 0: account (used as arg to balanceOf)
    // State slot 1: recipient (used as arg to transfer)
    let mut planner = WeirollPlanner::new();

    // State slot 0 — `account` padded to 32 bytes.
    let mut slot0 = [0u8; 32];
    slot0[12..].copy_from_slice(account.as_slice());
    let slot_account = planner.add_state_slot(slot0.to_vec());

    // State slot 1 — `recipient` padded.
    let mut slot1 = [0u8; 32];
    slot1[12..].copy_from_slice(recipient.as_slice());
    let slot_recipient = planner.add_state_slot(slot1.to_vec());

    // ── Command 0: balanceOf(account) ────────────────────────────────────────
    //
    // `in_out` maps: byte 0 = slot_account (input), byte 3 = 0x00 (return → slot 0).
    // Real Weiroll encoding uses 7-bit slot indices; we keep it simple here.
    let balance_of_sel = selector(b"balanceOf(address)");
    planner.add_command(WeirollCommand {
        flags: WeirollCommandFlags::StaticCall as u8,
        value: 0,
        gas: 0,
        target: usdc,
        selector: balance_of_sel,
        in_out: [slot_account as u8, 0xff, 0xff, 0x00],
    });

    // ── Command 1: transfer(recipient, <slot 0 = balance>) ───────────────────
    let transfer_sel = selector(b"transfer(address,uint256)");
    planner.add_command(WeirollCommand {
        flags: WeirollCommandFlags::Call as u8,
        value: 0,
        gas: 0,
        target: usdc,
        selector: transfer_sel,
        in_out: [slot_recipient as u8, 0x00, 0xff, 0xff],
    });

    println!("Planner state:");
    println!("  Commands:     {}", planner.command_count());
    println!("  State slots:  {}", planner.state_slot_count());
    println!();

    let script = planner.plan();
    println!("Packed script:");
    for (i, cmd) in script.commands.iter().enumerate() {
        println!("  cmd[{i}]: 0x{}", hex::encode(cmd));
    }
    println!();

    // ── Shortcut: build the full EvmCall in one go ───────────────────────────
    //
    // `create_weiroll_delegate_call` is the ergonomic entry point — it
    // returns a ready-to-send EvmCall targeting the canonical executor.
    let weiroll_exec: Address = WEIROLL_ADDRESS.parse()?;
    let evm_call = create_weiroll_delegate_call(|p| {
        p.add_command(WeirollCommand {
            flags: WeirollCommandFlags::StaticCall as u8,
            value: 0,
            gas: 0,
            target: usdc,
            selector: selector(b"totalSupply()"),
            in_out: [0xff, 0xff, 0xff, 0x00],
        });
    });

    println!("create_weiroll_delegate_call output:");
    println!("  to:        {}", evm_call.to);
    println!("  executor:  {} (canonical)", weiroll_exec);
    println!("  value:     {}", evm_call.value);
    println!("  data len:  {} bytes", evm_call.data.len());

    Ok(())
}
