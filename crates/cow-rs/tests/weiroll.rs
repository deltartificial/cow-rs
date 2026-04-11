#![allow(
    clippy::allow_attributes_without_reason,
    clippy::tests_outside_test_module,
    clippy::doc_markdown,
    clippy::type_complexity,
    clippy::missing_const_for_fn,
    clippy::assertions_on_constants,
    clippy::missing_assert_message,
    clippy::map_err_ignore,
    clippy::deref_by_slicing,
    clippy::redundant_clone,
    clippy::single_match_else,
    clippy::single_match
)]
//! Tests for the Weiroll script builder.

use alloy_primitives::{Address, address};
use cow_rs::weiroll::{WeirollCommand, WeirollPlanner};

const fn sample_command() -> WeirollCommand {
    WeirollCommand {
        flags: 0x01,
        value: 0x02,
        gas: 21_000,
        target: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        selector: [0xde, 0xad, 0xbe, 0xef],
        in_out: [0x00, 0x01, 0x02, 0x03],
    }
}

// ── WeirollCommand::pack ──────────────────────────────────────────────────────

#[test]
fn weiroll_command_pack_has_correct_length() {
    let packed = sample_command().pack();
    assert_eq!(packed.len(), 32);
}

#[test]
fn weiroll_command_flags_at_byte_0() {
    let cmd = sample_command();
    let packed = cmd.pack();
    assert_eq!(packed[0], cmd.flags);
}

#[test]
fn weiroll_command_value_at_byte_1() {
    let cmd = sample_command();
    let packed = cmd.pack();
    assert_eq!(packed[1], cmd.value);
}

#[test]
fn weiroll_command_gas_at_bytes_2_3() {
    let cmd = sample_command();
    let packed = cmd.pack();
    let gas = u16::from_be_bytes([packed[2], packed[3]]);
    assert_eq!(gas, cmd.gas);
}

#[test]
fn weiroll_command_target_at_bytes_4_23() {
    let cmd = sample_command();
    let packed = cmd.pack();
    assert_eq!(&packed[4..24], cmd.target.as_slice());
}

#[test]
fn weiroll_command_selector_at_bytes_24_27() {
    let cmd = sample_command();
    let packed = cmd.pack();
    assert_eq!(&packed[24..28], &cmd.selector);
}

#[test]
fn weiroll_command_in_out_at_bytes_28_31() {
    let cmd = sample_command();
    let packed = cmd.pack();
    assert_eq!(&packed[28..32], &cmd.in_out);
}

// ── WeirollPlanner ────────────────────────────────────────────────────────────

#[test]
fn weiroll_planner_new_is_empty() {
    let planner = WeirollPlanner::new();
    assert_eq!(planner.command_count(), 0);
    assert_eq!(planner.state_slot_count(), 0);
}

#[test]
fn weiroll_planner_add_command_increments_count() {
    let mut planner = WeirollPlanner::new();
    planner.add_command(sample_command());
    assert_eq!(planner.command_count(), 1);
    planner.add_command(sample_command());
    assert_eq!(planner.command_count(), 2);
}

#[test]
fn weiroll_planner_add_state_slot_returns_index() {
    let mut planner = WeirollPlanner::new();
    let idx0 = planner.add_state_slot(vec![1, 2, 3]);
    let idx1 = planner.add_state_slot(vec![4, 5]);
    assert_eq!(idx0, 0);
    assert_eq!(idx1, 1);
}

#[test]
fn weiroll_planner_plan_produces_correct_command_count() {
    let mut planner = WeirollPlanner::new();
    planner.add_command(sample_command());
    planner.add_command(sample_command());
    let script = planner.plan();
    assert_eq!(script.command_count(), 2);
}

// ── WeirollScript ─────────────────────────────────────────────────────────────

#[test]
fn weiroll_script_is_empty_when_no_commands() {
    let script = WeirollPlanner::new().plan();
    assert!(script.is_empty());
}

#[test]
fn weiroll_script_command_count_matches() {
    let mut planner = WeirollPlanner::new();
    for _ in 0..5 {
        planner.add_command(sample_command());
    }
    let script = planner.plan();
    assert_eq!(script.command_count(), 5);
}

#[test]
fn weiroll_script_packed_command_roundtrip() {
    let cmd = sample_command();
    let expected = cmd.pack();
    let mut planner = WeirollPlanner::new();
    planner.add_command(cmd);
    let script = planner.plan();
    assert_eq!(script.commands[0], expected);
}

// ── accessor helpers ──────────────────────────────────────────────────────────

#[test]
fn weiroll_command_flags_ref() {
    let cmd = sample_command();
    assert_eq!(cmd.flags_ref(), cmd.flags);
}

#[test]
fn weiroll_command_target_ref() {
    let cmd = sample_command();
    assert_eq!(cmd.target_ref(), &cmd.target);
}

#[test]
fn weiroll_command_selector_ref() {
    let cmd = sample_command();
    assert_eq!(cmd.selector_ref(), &cmd.selector);
}

// ── zero address target ───────────────────────────────────────────────────────

#[test]
fn weiroll_command_zero_target_packed_correctly() {
    let cmd = WeirollCommand {
        flags: 0,
        value: 0,
        gas: 0,
        target: Address::ZERO,
        selector: [0; 4],
        in_out: [0; 4],
    };
    let packed = cmd.pack();
    assert!(packed[4..24].iter().all(|&b| b == 0));
}

// ── WeirollScript state_slot_count ──────────────────────────────────────────

#[test]
fn weiroll_script_state_slot_count() {
    let mut planner = WeirollPlanner::new();
    planner.add_state_slot(vec![1, 2, 3]);
    planner.add_state_slot(vec![4, 5]);
    planner.add_command(sample_command());
    let script = planner.plan();
    assert_eq!(script.state_slot_count(), 2);
    assert_eq!(script.command_count(), 1);
}

// ── WeirollPlanner chaining ─────────────────────────────────────────────────

#[test]
fn weiroll_planner_add_command_returns_self() {
    let mut planner = WeirollPlanner::new();
    planner.add_command(sample_command()).add_command(sample_command());
    assert_eq!(planner.command_count(), 2);
}

// ── create_weiroll_delegate_call with state slots ───────────────────────────

#[test]
fn delegate_call_with_state_slots() {
    use cow_rs::weiroll::create_weiroll_delegate_call;

    let evm_call = create_weiroll_delegate_call(|planner| {
        planner.add_state_slot(vec![0xAA; 32]);
        planner.add_state_slot(vec![0xBB; 64]);
        planner.add_command(WeirollCommand {
            flags: 0x01,
            value: 0,
            gas: 0,
            target: Address::ZERO,
            selector: [0xDE, 0xAD, 0xBE, 0xEF],
            in_out: [0, 1, 0xFF, 0xFF],
        });
    });
    // The encoded calldata should contain the state slots
    assert!(evm_call.data.len() > 4 + 64); // at least selector + head + some data
    assert_eq!(&evm_call.data[..4], &[0xde, 0x79, 0x2b, 0xe1]);
}

// ── create_weiroll_contract and library with ABI ────────────────────────────

#[test]
fn create_weiroll_contract_with_abi() {
    use cow_rs::weiroll::{WeirollCommandFlags, create_weiroll_contract};
    let abi = b"[{\"name\":\"test\"}]".to_vec();
    let contract = create_weiroll_contract(Address::ZERO, abi.clone(), None);
    assert_eq!(contract.command_flags, WeirollCommandFlags::Call);
    assert_eq!(contract.abi, abi);
}

#[test]
fn create_weiroll_library_with_abi() {
    use cow_rs::weiroll::{WeirollCommandFlags, create_weiroll_library};
    let abi = b"[{\"name\":\"lib_fn\"}]".to_vec();
    let lib = create_weiroll_library(Address::ZERO, abi.clone());
    assert_eq!(lib.command_flags, WeirollCommandFlags::DelegateCall);
    assert_eq!(lib.abi, abi);
}

// ── WeirollCommandFlags bit operations ──────────────────────────────────────

#[test]
fn command_flags_combine_with_extended() {
    use cow_rs::weiroll::WeirollCommandFlags;
    let combined = WeirollCommandFlags::Call as u8 | WeirollCommandFlags::EXTENDED_COMMAND;
    assert_eq!(combined & WeirollCommandFlags::CALLTYPE_MASK, WeirollCommandFlags::Call as u8);
    assert_ne!(combined & WeirollCommandFlags::EXTENDED_COMMAND, 0);
}

#[test]
fn command_flags_combine_with_tuple_return() {
    use cow_rs::weiroll::WeirollCommandFlags;
    let combined = WeirollCommandFlags::StaticCall as u8 | WeirollCommandFlags::TUPLE_RETURN;
    assert_eq!(
        combined & WeirollCommandFlags::CALLTYPE_MASK,
        WeirollCommandFlags::StaticCall as u8
    );
    assert_ne!(combined & WeirollCommandFlags::TUPLE_RETURN, 0);
}

// ── WeirollContractRef debug and clone ──────────────────────────────────────

#[test]
fn weiroll_contract_ref_debug_and_clone() {
    use cow_rs::weiroll::{WeirollCommandFlags, WeirollContractRef};
    let contract = WeirollContractRef {
        address: Address::ZERO,
        abi: vec![],
        command_flags: WeirollCommandFlags::Call,
    };
    let cloned = contract.clone();
    assert_eq!(cloned.command_flags, WeirollCommandFlags::Call);
    let debug = format!("{contract:?}");
    assert!(debug.contains("WeirollContractRef"));
}

// ── WeirollCommand clone ────────────────────────────────────────────────────

#[test]
fn weiroll_command_clone() {
    let cmd = sample_command();
    let cloned = cmd.clone();
    assert_eq!(cloned.pack(), cmd.pack());
}

// ── Multiple state slots ────────────────────────────────────────────────────

#[test]
fn weiroll_planner_multiple_state_slots() {
    let mut planner = WeirollPlanner::new();
    for i in 0..10 {
        let idx = planner.add_state_slot(vec![i as u8; 32]);
        assert_eq!(idx, i);
    }
    assert_eq!(planner.state_slot_count(), 10);
    let script = planner.plan();
    assert_eq!(script.state_slot_count(), 10);
    assert_eq!(script.state[5], vec![5u8; 32]);
}

// ── define_read_only with string ────────────────────────────────────────────

#[test]
fn define_read_only_with_string() {
    use cow_rs::weiroll::define_read_only;
    let s = define_read_only(String::new(), |s| s.push_str("hello"));
    assert_eq!(s, "hello");
}

// ── get_static with duplicate keys ──────────────────────────────────────────

#[test]
fn get_static_returns_first_match() {
    use cow_rs::weiroll::get_static;
    let entries: &[(&str, i32)] = &[("a", 1), ("a", 2), ("b", 3)];
    assert_eq!(get_static(entries, "a"), Some(1));
}
