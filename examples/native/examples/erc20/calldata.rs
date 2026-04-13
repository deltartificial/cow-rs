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
//! # ERC-20 Calldata Builders
//!
//! Hand-rolled builders for the standard ERC-20 selectors. Useful when
//! you need to:
//!
//!   - build a pre-interaction hook that approves, transfers, or queries a token
//!   - prepare an `eth_call` payload for `OnchainReader`
//!   - embed raw calldata inside a CowShed / Weiroll bundle
//!
//! All builders are pure functions — no async, no allocations beyond the
//! returned `Vec<u8>`, no RPC.
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example erc20_calldata
//! ```

use alloy_primitives::{U256, address, hex};
use cow_rs::{
    SupportedChainId, build_eip2612_nonces_calldata, build_erc20_allowance_calldata,
    build_erc20_approve_calldata, build_erc20_balance_of_calldata, build_erc20_decimals_calldata,
    build_erc20_name_calldata, build_erc20_transfer_calldata, build_erc20_transfer_from_calldata,
    cow_protocol_vault_relayer_address,
};

fn print_call(label: &str, cd: &[u8]) {
    let sel = format!("{:02x}{:02x}{:02x}{:02x}", cd[0], cd[1], cd[2], cd[3]);
    println!("  {label:<26} {} bytes  selector=0x{sel}", cd.len());
    if cd.len() <= 68 {
        println!("                             data=0x{}", hex::encode(cd));
    }
}

fn main() {
    let relayer = cow_protocol_vault_relayer_address(SupportedChainId::Mainnet);
    let alice = address!("1111111111111111111111111111111111111111");
    let bob = address!("2222222222222222222222222222222222222222");
    let amount = U256::from(1_000_000u64); // 1 USDC (6 decimals)

    println!("ERC-20 calldata builders:");
    println!();

    // ── Unlimited approval to the Vault Relayer ──────────────────────────────
    //
    // The most common pre-interaction hook: lets CoW's solver pull funds.
    let approve_max = build_erc20_approve_calldata(relayer, U256::MAX);
    print_call("approve(relayer, MAX)", &approve_max);

    // ── Zero approval (revocation) ───────────────────────────────────────────
    let approve_zero = build_erc20_approve_calldata(relayer, U256::ZERO);
    print_call("approve(relayer, 0)", &approve_zero);

    // ── Balance and allowance reads ──────────────────────────────────────────
    //
    // Pair these with `OnchainReader.eth_call()` for lightweight state queries.
    let balance_of = build_erc20_balance_of_calldata(alice);
    print_call("balanceOf(alice)", &balance_of);

    let allowance = build_erc20_allowance_calldata(alice, relayer);
    print_call("allowance(alice, relayer)", &allowance);

    // ── Transfer / transferFrom ──────────────────────────────────────────────
    let transfer = build_erc20_transfer_calldata(bob, amount);
    print_call("transfer(bob, 1 USDC)", &transfer);

    let transfer_from = build_erc20_transfer_from_calldata(alice, bob, amount);
    print_call("transferFrom(alice, bob, 1 USDC)", &transfer_from);

    // ── Metadata reads (0 args → 4-byte selector only) ───────────────────────
    let decimals = build_erc20_decimals_calldata();
    print_call("decimals()", &decimals);

    let name = build_erc20_name_calldata();
    print_call("name()", &name);

    // ── EIP-2612 permit nonce read ───────────────────────────────────────────
    //
    // Needed to build a valid `PermitInfo` — the token's per-owner nonce
    // is monotonically incremented with each successful `permit` call.
    let nonces = build_eip2612_nonces_calldata(alice);
    print_call("nonces(alice) [EIP-2612]", &nonces);

    // ── Sanity: selector matches the classic 0x095ea7b3 for approve ──────────
    //
    // `keccak256("approve(address,uint256)")[0..4] == 0x095ea7b3`.
    assert_eq!(&approve_max[0..4], &[0x09, 0x5e, 0xa7, 0xb3]);
    println!();
    println!("(All calldata built offline, zero RPC calls.)");
}
