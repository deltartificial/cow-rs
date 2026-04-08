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
