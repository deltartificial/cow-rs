//! Slippage suggestion utilities for `CoW` Protocol trades.
//!
//! These functions mirror the `TypeScript` SDK's `suggestSlippageBps`,
//! `suggestSlippageFromFee`, `suggestSlippageFromVolume`, `percentageToBps`,
//! `bpsToPercentage`, and `applyPercentage` helpers.

use alloy_primitives::U256;
use rust_decimal::{Decimal, prelude::ToPrimitive as _};

use super::types::QuoteAmountsAndCosts;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Default fee-based slippage multiplier: 50 % of the quoted fee amount.
///
/// Mirrors `SUGGEST_SLIPPAGE_FEE_MULTIPLIER_PERCENT` from the `TypeScript` SDK.
pub const DEFAULT_FEE_SLIPPAGE_FACTOR_PCT: u32 = 50;

/// Default volume-based slippage in basis points (0.5 %).
///
/// Mirrors `DEFAULT_SLIPPAGE_BPS` for the volume component.
pub const DEFAULT_VOLUME_SLIPPAGE_BPS: u32 = 50;

/// Maximum allowed suggested slippage (100 %).
pub const MAX_SLIPPAGE_BPS: u32 = 10_000;

// ── Suggest slippage ──────────────────────────────────────────────────────────

/// Compute the fee-based slippage component.
///
/// Returns `fee_amount * multiply_factor_pct / 100` (in sell-token atoms).
///
/// # Arguments
///
/// * `fee_amount` — the network fee amount in sell-token atoms.
/// * `multiply_factor_pct` — percentage of the fee to use as slippage buffer (e.g. `50` for 50%).
///
/// # Returns
///
/// The fee-based slippage component as a `U256` amount in sell-token atoms.
///
/// # Example
///
/// ```
/// use alloy_primitives::U256;
/// use cow_rs::suggest_slippage_from_fee;
///
/// // 50 % of a 1_000 atom fee → 500 atoms
/// assert_eq!(suggest_slippage_from_fee(U256::from(1_000u32), 50), U256::from(500u32));
/// ```
#[must_use]
pub fn suggest_slippage_from_fee(fee_amount: U256, multiply_factor_pct: u32) -> U256 {
    fee_amount * U256::from(multiply_factor_pct) / U256::from(100u32)
}

/// Compute the volume-based slippage component.
///
/// # Arguments
///
/// * `sell_before` — sell amount before network costs (gross).
/// * `sell_after`  — sell amount after network costs (net).
/// * `is_sell`     — `true` for sell orders, `false` for buy orders.
/// * `volume_bps`  — fraction of sell amount to reserve as slippage buffer (50 = 0.5 %).
///
/// # Returns
///
/// The volume-based slippage component as a `U256` in sell-token atoms
/// (`base_sell_amount * volume_bps / 10_000`), or `U256::ZERO` when the base amount is zero.
#[must_use]
pub fn suggest_slippage_from_volume(
    sell_before: U256,
    sell_after: U256,
    is_sell: bool,
    volume_bps: u32,
) -> U256 {
    // Sell orders use the net amount (fees already deducted); buy orders use gross.
    let base = if is_sell { sell_after } else { sell_before };
    if base.is_zero() {
        return U256::ZERO;
    }
    base * U256::from(volume_bps) / U256::from(10_000u32)
}

/// Suggest a slippage tolerance for a trade, combining fee- and volume-based components.
///
/// The result is clamped to `[min_bps, MAX_SLIPPAGE_BPS]`.
///
/// Default parameters (matching the `TypeScript` SDK):
/// - `fee_factor_pct = 50` ([`DEFAULT_FEE_SLIPPAGE_FACTOR_PCT`])
/// - `volume_bps = 50`     ([`DEFAULT_VOLUME_SLIPPAGE_BPS`])
/// - `min_bps = 0`
///
/// # Arguments
///
/// * `costs` — the quote amounts and costs breakdown for the trade.
/// * `fee_factor_pct` — percentage of the network fee to include in the slippage buffer.
/// * `volume_bps` — fraction of sell amount to reserve as slippage buffer.
/// * `min_bps` — minimum slippage floor in basis points.
///
/// # Returns
///
/// The suggested slippage tolerance in basis points, clamped to `[min_bps, MAX_SLIPPAGE_BPS]`.
/// Returns `min_bps` when the sell amount before network costs is zero.
///
/// # Algorithm
///
/// 1. `fee_component  = fee_amount × fee_factor_pct / 100`
/// 2. `vol_component  = base_sell × volume_bps / 10_000`
/// 3. `total_atoms    = fee_component + vol_component`
/// 4. `suggested_bps  = total_atoms × 10_000 / sell_before_network_costs`
/// 5. Clamp to `[min_bps, MAX_SLIPPAGE_BPS]`
#[must_use]
pub fn suggest_slippage_bps(
    costs: &QuoteAmountsAndCosts,
    fee_factor_pct: u32,
    volume_bps: u32,
    min_bps: u32,
) -> u32 {
    let sell_before = costs.before_network_costs.sell_amount;
    if sell_before.is_zero() {
        return min_bps;
    }

    let fee = costs.network_fee.amount_in_sell_currency;
    let sell_after = costs.after_network_costs.sell_amount;

    let fee_component = suggest_slippage_from_fee(fee, fee_factor_pct);
    let vol_component =
        suggest_slippage_from_volume(sell_before, sell_after, costs.is_sell, volume_bps);

    let total = fee_component + vol_component;
    // Convert to bps: total_atoms * 10_000 / sell_before
    let suggested_u64: u64 = (total * U256::from(10_000u32) / sell_before)
        .try_into()
        .unwrap_or_else(|_| u64::from(MAX_SLIPPAGE_BPS));
    // Clamp to MAX_SLIPPAGE_BPS and truncate safely (≤ 10_000, fits u32)
    let suggested = suggested_u64.min(u64::from(MAX_SLIPPAGE_BPS)) as u32;
    suggested.max(min_bps)
}

// ── Percentage / BPS conversion ───────────────────────────────────────────────

/// Convert a percentage value to basis points (rounded).
///
/// `0.5` → `50`, `1.0` → `100`, `0.25` → `25`.
///
/// Mirrors `percentageToBps` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `percentage` — the percentage value to convert (e.g. `Decimal::new(5, 1)` for 0.5%).
///
/// # Returns
///
/// The equivalent value in basis points as a `u32`, or `0` if the conversion overflows.
///
/// # Example
///
/// ```
/// use cow_rs::trading::percentage_to_bps;
/// use rust_decimal::Decimal;
///
/// assert_eq!(percentage_to_bps(Decimal::new(5, 1)), 50); // 0.5 → 50 bps
/// assert_eq!(percentage_to_bps(Decimal::new(1, 0)), 100); // 1.0 → 100 bps
/// ```
#[must_use]
pub fn percentage_to_bps(percentage: Decimal) -> u32 {
    let bps = (percentage * Decimal::from(100)).round();
    bps.to_u32().map_or(0, |v| v)
}

/// Convert basis points to a percentage value.
///
/// `50` → `0.5`, `100` → `1.0`, `25` → `0.25`.
///
/// Mirrors `bpsToPercentage` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `bps` — basis points to convert (e.g. `50` for 0.5%).
///
/// # Returns
///
/// The equivalent percentage as a [`Decimal`] (e.g. `50` bps becomes `Decimal(0.5)`).
///
/// # Example
///
/// ```
/// use cow_rs::trading::bps_to_percentage;
/// use rust_decimal::Decimal;
///
/// assert_eq!(bps_to_percentage(50), Decimal::new(5, 1)); // 50 → 0.5
/// assert_eq!(bps_to_percentage(100), Decimal::new(1, 0)); // 100 → 1.0
/// ```
#[must_use]
pub fn bps_to_percentage(bps: u32) -> Decimal {
    Decimal::from(bps) / Decimal::from(100)
}

/// Apply a percentage value to `value`.
///
/// Converts `percentage` to integer basis points via [`percentage_to_bps`],
/// then returns `value × bps / 100`.  In the `TypeScript` SDK, `percentage`
/// lives in a `[0, 100]` scale — passing `0.5` means "0.5 on that scale",
/// equivalent to 50 bps, and yields `value × 50 / 100`.
///
/// Mirrors `applyPercentage` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `value` — the `U256` amount to scale.
/// * `percentage` — the percentage to apply (converted to bps internally).
///
/// # Returns
///
/// `value * bps / 100`, where `bps` is derived from `percentage` via [`percentage_to_bps`].
///
/// # Example
///
/// ```
/// use alloy_primitives::U256;
/// use cow_rs::trading::apply_percentage;
/// use rust_decimal::Decimal;
///
/// // percentage = 0.5 → 50 bps → 200 × 50 / 100 = 100
/// assert_eq!(apply_percentage(U256::from(200u32), Decimal::new(5, 1)), U256::from(100u32));
/// ```
#[must_use]
pub fn apply_percentage(value: U256, percentage: Decimal) -> U256 {
    let bps = percentage_to_bps(percentage);
    value * U256::from(bps) / U256::from(100u32)
}
