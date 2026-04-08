//! Tests for slippage helpers, bps/percentage conversions, and fee suggestions.

use alloy_primitives::U256;
use cow_rs::{
    DEFAULT_SLIPPAGE_BPS, DEFAULT_VOLUME_SLIPPAGE_BPS, MAX_SLIPPAGE_BPS, apply_percentage,
    bps_to_percentage, percentage_to_bps, suggest_slippage_from_fee, suggest_slippage_from_volume,
};
use rust_decimal::Decimal;

// ── bps_to_percentage / percentage_to_bps ────────────────────────────────────

#[test]
fn bps_to_percentage_100_bps_is_one_percent() {
    let pct = bps_to_percentage(100);
    assert_eq!(pct, Decimal::new(1, 0)); // 1.0
}

#[test]
fn bps_to_percentage_50_is_half_percent() {
    let pct = bps_to_percentage(50);
    assert_eq!(pct, Decimal::new(5, 1)); // 0.5
}

#[test]
fn bps_to_percentage_0_is_zero() {
    assert_eq!(bps_to_percentage(0), Decimal::ZERO);
}

#[test]
fn percentage_to_bps_1_percent_is_100() {
    let bps = percentage_to_bps(Decimal::new(1, 0));
    assert_eq!(bps, 100);
}

#[test]
fn percentage_to_bps_roundtrip() {
    for bps_val in [0u32, 1, 50, 100, 200, 500, 10000] {
        let pct = bps_to_percentage(bps_val);
        let back = percentage_to_bps(pct);
        assert_eq!(back, bps_val, "roundtrip failed for {bps_val}");
    }
}

// ── apply_percentage ──────────────────────────────────────────────────────────

#[test]
fn apply_percentage_zero_returns_zero() {
    let amount = U256::from(1_000_000u64);
    let pct = Decimal::ZERO;
    assert_eq!(apply_percentage(amount, pct), U256::ZERO);
}

#[test]
fn apply_percentage_1_0_returns_full_amount() {
    // 1.0 = 100 bps → value * 100 / 100 = value
    let amount = U256::from(1_000u64);
    let pct = Decimal::new(1, 0); // 1.0
    assert_eq!(apply_percentage(amount, pct), amount);
}

#[test]
fn apply_percentage_half_returns_half() {
    // 0.5 = 50 bps → value * 50 / 100 = value / 2
    let amount = U256::from(1_000u64);
    let pct = Decimal::new(5, 1); // 0.5
    assert_eq!(apply_percentage(amount, pct), U256::from(500u64));
}

#[test]
fn apply_percentage_hundredth_of_10000() {
    // 0.01 = 1 bps → 10000 * 1 / 100 = 100
    let amount = U256::from(10_000u64);
    let pct = Decimal::new(1, 2); // 0.01
    assert_eq!(apply_percentage(amount, pct), U256::from(100u64));
}

// ── suggest_slippage_from_fee ──────────────────────────────────────────────────
// suggest_slippage_from_fee(fee_amount: U256, multiply_factor_pct: u32) -> U256

#[test]
fn suggest_slippage_from_fee_50pct_of_1000() {
    // 50% of 1000 atoms = 500 atoms
    let result = suggest_slippage_from_fee(U256::from(1_000u64), 50);
    assert_eq!(result, U256::from(500u64));
}

#[test]
fn suggest_slippage_from_fee_zero_fee() {
    let result = suggest_slippage_from_fee(U256::ZERO, 50);
    assert_eq!(result, U256::ZERO);
}

#[test]
fn suggest_slippage_from_fee_100pct() {
    let fee = U256::from(1_000u64);
    let result = suggest_slippage_from_fee(fee, 100);
    assert_eq!(result, fee);
}

// ── suggest_slippage_from_volume ───────────────────────────────────────────────
// suggest_slippage_from_volume(sell_before: U256, sell_after: U256, is_sell: bool, volume_bps: u32)
// -> U256

#[test]
fn suggest_slippage_from_volume_50bps_of_10000() {
    // 0.5% of 10_000 = 50 atoms
    let result = suggest_slippage_from_volume(
        U256::from(10_000u64),
        U256::from(9_000u64),
        true, // is_sell
        50,   // 0.5%
    );
    // For sell orders, uses sell_after (9_000): 9_000 * 50 / 10_000 = 45
    assert_eq!(result, U256::from(45u64));
}

#[test]
fn suggest_slippage_from_volume_buy_order_uses_sell_before() {
    // For buy orders, uses sell_before
    let result = suggest_slippage_from_volume(
        U256::from(10_000u64),
        U256::from(9_000u64),
        false, // is_buy
        100,   // 1%
    );
    // 10_000 * 100 / 10_000 = 100
    assert_eq!(result, U256::from(100u64));
}

#[test]
fn suggest_slippage_from_volume_zero_base_returns_zero() {
    let result = suggest_slippage_from_volume(U256::ZERO, U256::ZERO, true, 50);
    assert_eq!(result, U256::ZERO);
}

// ── constants ─────────────────────────────────────────────────────────────────

#[test]
fn default_slippage_bps_is_reasonable() {
    // Default slippage should be between 1 and 500 bps
    assert!(DEFAULT_SLIPPAGE_BPS >= 1);
    assert!(DEFAULT_SLIPPAGE_BPS <= 500);
}

#[test]
fn max_slippage_bps_is_10000() {
    assert_eq!(MAX_SLIPPAGE_BPS, 10_000);
}

#[test]
fn default_slippage_bps_less_than_max() {
    assert!(DEFAULT_SLIPPAGE_BPS < MAX_SLIPPAGE_BPS);
}

#[test]
fn default_volume_slippage_bps_is_reasonable() {
    assert!(DEFAULT_VOLUME_SLIPPAGE_BPS <= MAX_SLIPPAGE_BPS);
}
