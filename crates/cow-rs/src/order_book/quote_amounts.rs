//! Quote amount calculations: fee breakdown, slippage, and cost stages.
//!
//! Ports the `quoteAmountsAndCosts` package from the `TypeScript` SDK.
//! All arithmetic uses [`U256`] to avoid overflow on token amounts.
//!
//! # Fee stages
//!
//! The `/quote` API returns amounts where protocol fee and network costs are
//! already baked in. This module reconstructs every intermediate stage:
//!
//! 1. **Before all fees** — spot-price amounts.
//! 2. **After protocol fees** (= before network costs).
//! 3. **After network costs**.
//! 4. **After partner fees**.
//! 5. **After slippage** — the minimum the user signs for.
//!
//! # Key functions
//!
//! | Function | Purpose |
//! |---|---|
//! | [`get_quote_amounts_and_costs`] | Full breakdown from `/quote` response |
//! | [`get_protocol_fee_amount`] | Reverse-engineer the protocol fee |
//! | [`get_quote_amounts_after_partner_fee`] | Apply partner fee to amounts |
//! | [`get_quote_amounts_after_slippage`] | Apply slippage tolerance |
//! | [`transform_order`] | Enrich an `Order` with `total_fee` and EthFlow fields |

use alloy_primitives::U256;

use crate::types::{HUNDRED_THOUSANDS, ONE_HUNDRED_BPS, OrderKind};

// ── Core types ───────────────────────────────────────────────────────────────

/// Sell and buy amounts at a particular fee stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuoteAmounts {
    /// Amount of the sell token (in atoms).
    pub sell_amount: U256,
    /// Amount of the buy token (in atoms).
    pub buy_amount: U256,
}

/// Network fee expressed in both sell and buy currency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuoteNetworkFee {
    /// Network fee denominated in the sell token.
    pub amount_in_sell_currency: U256,
    /// Network fee denominated in the buy token.
    pub amount_in_buy_currency: U256,
}

/// A single fee component: absolute amount and its BPS rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuoteFeeComponent {
    /// Absolute fee amount (in the surplus token's atoms).
    pub amount: U256,
    /// Fee rate in basis points.
    pub bps: f64,
}

/// All cost components of a quote.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuoteCosts {
    /// Gas / network fee covering on-chain settlement.
    pub network_fee: QuoteNetworkFee,
    /// Partner fee (revenue share).
    pub partner_fee: QuoteFeeComponent,
    /// Protocol fee collected by `CoW` Protocol.
    pub protocol_fee: QuoteFeeComponent,
}

/// Complete breakdown of quote amounts at every fee stage.
///
/// See the `TypeScript` SDK's `QuoteAmountsAndCosts` for the canonical
/// description of each stage and how fees are layered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuoteAmountsAndCostsResult {
    /// Whether the underlying order is a sell order.
    pub is_sell: bool,
    /// Cost breakdown.
    pub costs: QuoteCosts,
    /// Spot-price amounts before any fees.
    pub before_all_fees: QuoteAmounts,
    /// Amounts before network costs (same as `after_protocol_fees`).
    pub before_network_costs: QuoteAmounts,
    /// Amounts after protocol fees have been applied.
    pub after_protocol_fees: QuoteAmounts,
    /// Amounts after network costs have been applied.
    pub after_network_costs: QuoteAmounts,
    /// Amounts after partner fees have been applied.
    pub after_partner_fees: QuoteAmounts,
    /// Amounts after slippage tolerance has been applied.
    pub after_slippage: QuoteAmounts,
    /// Final amounts to sign in the order.
    pub amounts_to_sign: QuoteAmounts,
}

// ── Input types ──────────────────────────────────────────────────────────────

/// Order parameters needed by the quote-amount calculations.
///
/// These map to the fields returned by `POST /api/v1/quote`.
#[derive(Debug, Clone)]
pub struct QuoteOrderParams {
    /// Sell or buy.
    pub kind: OrderKind,
    /// Sell amount (after network costs for sell orders).
    pub sell_amount: U256,
    /// Buy amount (minimum for sell orders, exact for buy orders).
    pub buy_amount: U256,
    /// Network fee / gas cost amount.
    pub fee_amount: U256,
}

/// Parameters for [`get_quote_amounts_and_costs`].
#[derive(Debug, Clone)]
pub struct QuoteAmountsAndCostsParams {
    /// The order parameters from the `/quote` response.
    pub order_params: QuoteOrderParams,
    /// Protocol fee in BPS (may be fractional, e.g. `0.003`). `None` or `0` means no protocol fee.
    pub protocol_fee_bps: Option<f64>,
    /// Partner fee in BPS. `None` or `0` means no partner fee.
    pub partner_fee_bps: Option<u32>,
    /// Slippage tolerance in BPS.
    pub slippage_percent_bps: u32,
}

/// Parameters for [`get_protocol_fee_amount`].
#[derive(Debug, Clone)]
pub struct ProtocolFeeAmountParams {
    /// The order parameters from the `/quote` response.
    pub order_params: QuoteOrderParams,
    /// Protocol fee in BPS (may be fractional).
    pub protocol_fee_bps: f64,
}

// ── Protocol fee ─────────────────────────────────────────────────────────────

/// Derive the absolute protocol-fee amount from the quote response.
///
/// The `/quote` API returns amounts where the protocol fee is already
/// baked in. This function reverses the fee to recover the absolute amount.
///
/// # Parameters
///
/// * `params` — a [`ProtocolFeeAmountParams`] containing the order parameters and the protocol fee
///   rate in BPS.
///
/// For **sell orders** the fee was deducted from `buyAmount`:
/// ```text
/// protocolFee = buyAmount * bps / (ONE_HUNDRED_BPS * scale - bps_scaled)
/// ```
///
/// For **buy orders** the fee was added to `sellAmount`:
/// ```text
/// protocolFee = (sellAmount + feeAmount) * bps / (ONE_HUNDRED_BPS * scale + bps_scaled)
/// ```
#[must_use]
pub fn get_protocol_fee_amount(params: &ProtocolFeeAmountParams) -> U256 {
    let ProtocolFeeAmountParams { order_params, protocol_fee_bps } = params;

    if *protocol_fee_bps <= 0.0 {
        return U256::ZERO;
    }

    let is_sell = order_params.kind.is_sell();
    let sell_amount = order_params.sell_amount;
    let buy_amount = order_params.buy_amount;
    let fee_amount = order_params.fee_amount;

    let protocol_fee_scale = U256::from(HUNDRED_THOUSANDS);
    // Keep 5 decimal places of BPS precision while staying in integer domain.
    let protocol_fee_bps_scaled = (*protocol_fee_bps * HUNDRED_THOUSANDS as f64).round() as u64;
    let protocol_fee_bps_big = U256::from(protocol_fee_bps_scaled);

    if protocol_fee_bps_big.is_zero() {
        return U256::ZERO;
    }

    let one_hundred_bps = U256::from(ONE_HUNDRED_BPS);

    if is_sell {
        // SELL: protocolFee = buyAmount * bps / (ONE_HUNDRED_BPS * scale - bps)
        let denominator = one_hundred_bps * protocol_fee_scale - protocol_fee_bps_big;
        buy_amount * protocol_fee_bps_big / denominator
    } else {
        // BUY: protocolFee = (sellAmount + feeAmount) * bps / (ONE_HUNDRED_BPS * scale + bps)
        let denominator = one_hundred_bps * protocol_fee_scale + protocol_fee_bps_big;
        (sell_amount + fee_amount) * protocol_fee_bps_big / denominator
    }
}

// ── Partner fee ──────────────────────────────────────────────────────────────

/// Result of partner-fee calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartnerFeeResult {
    /// Absolute partner-fee amount.
    pub partner_fee_amount: U256,
    /// Amounts after the partner fee has been applied.
    pub after_partner_fees: QuoteAmounts,
}

/// Calculate the partner-fee amount and adjust quote amounts accordingly.
///
/// The partner fee is computed relative to spot-price amounts
/// (`before_all_fees`) so it reflects the full trade volume, not volume
/// already reduced by protocol fee.
///
/// - **Sell orders**: partner fee is subtracted from `buyAmount`.
/// - **Buy orders**: partner fee is added to `sellAmount`.
///
/// # Parameters
///
/// * `after_network_costs` — amounts after network costs have been applied.
/// * `before_all_fees` — spot-price amounts (used as the fee base).
/// * `is_sell` — `true` for sell orders, `false` for buy orders.
/// * `partner_fee_bps` — partner fee rate in basis points.
///
/// # Returns
///
/// A [`PartnerFeeResult`] with the absolute fee amount and adjusted
/// amounts.
#[must_use]
pub fn get_quote_amounts_after_partner_fee(
    after_network_costs: &QuoteAmounts,
    before_all_fees: &QuoteAmounts,
    is_sell: bool,
    partner_fee_bps: u32,
) -> PartnerFeeResult {
    let one_hundred_bps = U256::from(ONE_HUNDRED_BPS);

    // Partner fee is based on spot-price amounts (before all fees).
    let surplus_amount =
        if is_sell { before_all_fees.buy_amount } else { before_all_fees.sell_amount };
    let partner_fee_amount = if partner_fee_bps > 0 {
        surplus_amount * U256::from(partner_fee_bps) / one_hundred_bps
    } else {
        U256::ZERO
    };

    let after_partner_fees = if is_sell {
        QuoteAmounts {
            sell_amount: after_network_costs.sell_amount,
            buy_amount: after_network_costs.buy_amount - partner_fee_amount,
        }
    } else {
        QuoteAmounts {
            sell_amount: after_network_costs.sell_amount + partner_fee_amount,
            buy_amount: after_network_costs.buy_amount,
        }
    };

    PartnerFeeResult { partner_fee_amount, after_partner_fees }
}

// ── Slippage ─────────────────────────────────────────────────────────────────

/// Apply slippage tolerance to amounts after all fees.
///
/// Slippage protects the user from price movements between quoting and
/// execution. It is always applied to the surplus (non-fixed) side:
///
/// - **Sell orders**: slippage reduces `buyAmount` (user accepts less).
/// - **Buy orders**: slippage increases `sellAmount` (user accepts more).
///
/// # Parameters
///
/// * `after_partner_fees` — amounts after partner fees have been applied.
/// * `is_sell` — `true` for sell orders, `false` for buy orders.
/// * `slippage_bps` — slippage tolerance in basis points.
///
/// # Returns
///
/// A [`QuoteAmounts`] with slippage applied to the surplus side.
#[must_use]
pub fn get_quote_amounts_after_slippage(
    after_partner_fees: &QuoteAmounts,
    is_sell: bool,
    slippage_bps: u32,
) -> QuoteAmounts {
    let one_hundred_bps = U256::from(ONE_HUNDRED_BPS);
    let slippage_bps_big = U256::from(slippage_bps);

    let slippage = |amount: U256| -> U256 { amount * slippage_bps_big / one_hundred_bps };

    if is_sell {
        QuoteAmounts {
            sell_amount: after_partner_fees.sell_amount,
            buy_amount: after_partner_fees.buy_amount - slippage(after_partner_fees.buy_amount),
        }
    } else {
        QuoteAmounts {
            sell_amount: after_partner_fees.sell_amount + slippage(after_partner_fees.sell_amount),
            buy_amount: after_partner_fees.buy_amount,
        }
    }
}

// ── Main entry point ─────────────────────────────────────────────────────────

/// Calculate all quote-amount stages and costs from a `/quote` API
/// response.
///
/// Takes the raw order parameters (where protocol fee and network costs
/// are already baked in) and reconstructs every intermediate amount stage:
///
/// 1. **Before all fees** — spot-price amounts.
/// 2. **After protocol fees** (= before network costs).
/// 3. **After network costs**.
/// 4. **After partner fees**.
/// 5. **After slippage** — the minimum the user signs for.
///
/// The returned [`QuoteAmountsAndCostsResult`] includes `amounts_to_sign`,
/// the final sell/buy amounts that should be placed in the signed order.
///
/// # Parameters
///
/// * `params` — a [`QuoteAmountsAndCostsParams`] containing the order parameters from the `/quote`
///   response, protocol/partner fee rates, and slippage tolerance.
///
/// # Returns
///
/// A [`QuoteAmountsAndCostsResult`] with amounts at every fee stage and
/// the complete cost breakdown.
#[must_use]
pub fn get_quote_amounts_and_costs(
    params: &QuoteAmountsAndCostsParams,
) -> QuoteAmountsAndCostsResult {
    let QuoteAmountsAndCostsParams {
        order_params,
        protocol_fee_bps,
        partner_fee_bps,
        slippage_percent_bps,
    } = params;

    let partner_fee = partner_fee_bps.map_or(0, |v| v);
    let protocol_fee = protocol_fee_bps.map_or(0.0, |v| v);
    let is_sell = order_params.kind.is_sell();

    let sell_amount = order_params.sell_amount;
    let buy_amount = order_params.buy_amount;
    let network_cost_amount = order_params.fee_amount;

    // Avoid division by zero when sell_amount is 0.
    let network_cost_in_buy = if sell_amount.is_zero() {
        U256::ZERO
    } else {
        buy_amount * network_cost_amount / sell_amount
    };

    // Reconstruct the protocol fee amount from the baked-in quote.
    let protocol_fee_amount = get_protocol_fee_amount(&ProtocolFeeAmountParams {
        order_params: order_params.clone(),
        protocol_fee_bps: protocol_fee,
    });

    // Stage 0: before all fees (spot price).
    let before_all_fees = if is_sell {
        QuoteAmounts {
            sell_amount: sell_amount + network_cost_amount,
            buy_amount: buy_amount + network_cost_in_buy + protocol_fee_amount,
        }
    } else {
        QuoteAmounts { sell_amount: sell_amount - protocol_fee_amount, buy_amount }
    };

    // Stage 1: after protocol fees (= before network costs).
    let after_protocol_fees = if is_sell {
        QuoteAmounts {
            sell_amount: before_all_fees.sell_amount,
            buy_amount: before_all_fees.buy_amount - protocol_fee_amount,
        }
    } else {
        QuoteAmounts { sell_amount, buy_amount: before_all_fees.buy_amount }
    };

    // Stage 2: after network costs.
    let after_network_costs = if is_sell {
        QuoteAmounts { sell_amount, buy_amount }
    } else {
        QuoteAmounts {
            sell_amount: sell_amount + network_cost_amount,
            buy_amount: after_protocol_fees.buy_amount,
        }
    };

    // Stage 3: after partner fees.
    let PartnerFeeResult { partner_fee_amount, after_partner_fees } =
        get_quote_amounts_after_partner_fee(
            &after_network_costs,
            &before_all_fees,
            is_sell,
            partner_fee,
        );

    // Stage 4: after slippage.
    let after_slippage =
        get_quote_amounts_after_slippage(&after_partner_fees, is_sell, *slippage_percent_bps);

    // Final amounts to sign.
    let amounts_to_sign = if is_sell {
        QuoteAmounts {
            sell_amount: before_all_fees.sell_amount,
            buy_amount: after_slippage.buy_amount,
        }
    } else {
        QuoteAmounts {
            sell_amount: after_slippage.sell_amount,
            buy_amount: before_all_fees.buy_amount,
        }
    };

    QuoteAmountsAndCostsResult {
        is_sell,
        costs: QuoteCosts {
            network_fee: QuoteNetworkFee {
                amount_in_sell_currency: network_cost_amount,
                amount_in_buy_currency: network_cost_in_buy,
            },
            partner_fee: QuoteFeeComponent { amount: partner_fee_amount, bps: partner_fee as f64 },
            protocol_fee: QuoteFeeComponent { amount: protocol_fee_amount, bps: protocol_fee },
        },
        before_all_fees,
        before_network_costs: after_protocol_fees,
        after_protocol_fees,
        after_network_costs,
        after_partner_fees,
        after_slippage,
        amounts_to_sign,
    }
}

// ── Transform order ──────────────────────────────────────────────────────────

/// Enrich an [`Order`](super::Order) by computing its `total_fee` field.
///
/// The total fee is the sum of `executed_fee_amount` and `executed_fee`
/// (both may be zero or non-zero independently). After computing the total
/// fee, `EthFlow` transformations are applied if applicable:
///
/// - `valid_to` is overwritten with the user's original validity.
/// - `owner` is overwritten with the on-chain user address.
/// - `sell_token` is replaced with
///   [`NATIVE_CURRENCY_ADDRESS`](crate::config::NATIVE_CURRENCY_ADDRESS).
///
/// Mirrors `transformOrder` from the `TypeScript` SDK.
///
/// # Parameters
///
/// * `order` — the [`Order`](super::Order) to enrich (consumed and returned).
///
/// # Returns
///
/// The enriched [`Order`](super::Order) with `total_fee` set.
#[must_use]
pub fn transform_order(mut order: super::Order) -> super::Order {
    // Compute total fee.
    let executed_fee_amount: U256 = order.executed_fee_amount.parse().map_or(U256::ZERO, |v| v);
    let executed_fee: U256 =
        order.executed_fee.as_deref().and_then(|s| s.parse().ok()).map_or(U256::ZERO, |v| v);
    order.total_fee = Some((executed_fee_amount + executed_fee).to_string());

    // Apply EthFlow transformations (no-op for regular orders).
    if let Some(ref ethflow_data) = order.ethflow_data {
        order.valid_to = ethflow_data.user_valid_to;
        if let Some(user) = order.onchain_user {
            order.owner = user;
        }
        order.sell_token = crate::config::NATIVE_CURRENCY_ADDRESS;
    }

    order
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Test fixtures from the TypeScript SDK test suite.
    fn sell_order() -> QuoteOrderParams {
        QuoteOrderParams {
            kind: OrderKind::Sell,
            sell_amount: U256::from_str_radix("156144455961718918", 10).unwrap(),
            fee_amount: U256::from_str_radix("3855544038281082", 10).unwrap(),
            buy_amount: U256::from_str_radix("18632013982", 10).unwrap(),
        }
    }

    fn buy_order() -> QuoteOrderParams {
        QuoteOrderParams {
            kind: OrderKind::Buy,
            sell_amount: U256::from_str_radix("168970833896526983", 10).unwrap(),
            fee_amount: U256::from_str_radix("2947344072902629", 10).unwrap(),
            buy_amount: U256::from_str_radix("2000000000", 10).unwrap(),
        }
    }

    // ── Network costs ────────────────────────────────────────────────────────

    #[test]
    fn sell_after_network_costs_equals_api_sell_amount() {
        let order_params = sell_order();
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: 0,
            partner_fee_bps: None,
            protocol_fee_bps: None,
        });
        assert_eq!(result.after_network_costs.sell_amount, order_params.sell_amount);
    }

    #[test]
    fn sell_before_network_costs_is_sell_plus_fee() {
        let order_params = sell_order();
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: 0,
            partner_fee_bps: None,
            protocol_fee_bps: None,
        });
        assert_eq!(
            result.before_network_costs.sell_amount,
            order_params.sell_amount + order_params.fee_amount
        );
    }

    #[test]
    fn buy_after_network_costs_is_sell_plus_fee() {
        let order_params = buy_order();
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: 0,
            partner_fee_bps: None,
            protocol_fee_bps: None,
        });
        assert_eq!(
            result.after_network_costs.sell_amount,
            order_params.sell_amount + order_params.fee_amount
        );
    }

    #[test]
    fn buy_before_network_costs_is_raw_sell() {
        let order_params = buy_order();
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: 0,
            partner_fee_bps: None,
            protocol_fee_bps: None,
        });
        assert_eq!(result.before_network_costs.sell_amount, order_params.sell_amount);
    }

    #[test]
    fn sell_buy_amount_includes_network_cost_in_buy_currency() {
        let order_params = sell_order();
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: 0,
            partner_fee_bps: None,
            protocol_fee_bps: None,
        });
        let network_cost_in_buy =
            order_params.buy_amount * order_params.fee_amount / order_params.sell_amount;
        assert_eq!(
            result.before_network_costs.buy_amount,
            order_params.buy_amount + network_cost_in_buy
        );
        assert_eq!(result.after_network_costs.buy_amount, order_params.buy_amount);
    }

    #[test]
    fn buy_buy_amount_unchanged_by_network_costs() {
        let order_params = buy_order();
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params,
            slippage_percent_bps: 0,
            partner_fee_bps: None,
            protocol_fee_bps: None,
        });
        assert_eq!(result.after_network_costs.buy_amount, result.before_network_costs.buy_amount);
    }

    // ── Partner fee ──────────────────────────────────────────────────────────

    #[test]
    fn sell_partner_fee_subtracted_from_buy() {
        let order_params = sell_order();
        let partner_fee_bps = 100u32;
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: 0,
            partner_fee_bps: Some(partner_fee_bps),
            protocol_fee_bps: None,
        });
        let network_cost_in_buy =
            order_params.buy_amount * order_params.fee_amount / order_params.sell_amount;
        let buy_before_all_fees = order_params.buy_amount + network_cost_in_buy;
        let expected =
            buy_before_all_fees * U256::from(partner_fee_bps) / U256::from(ONE_HUNDRED_BPS);
        assert_eq!(result.costs.partner_fee.amount, expected);
    }

    #[test]
    fn buy_partner_fee_added_to_sell() {
        let order_params = buy_order();
        let partner_fee_bps = 100u32;
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: 0,
            partner_fee_bps: Some(partner_fee_bps),
            protocol_fee_bps: None,
        });
        let expected =
            order_params.sell_amount * U256::from(partner_fee_bps) / U256::from(ONE_HUNDRED_BPS);
        assert_eq!(result.costs.partner_fee.amount, expected);
    }

    // ── Slippage ─────────────────────────────────────────────────────────────

    #[test]
    fn sell_slippage_subtracted_from_buy() {
        let order_params = sell_order();
        let slippage_bps = 200u32;
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: slippage_bps,
            partner_fee_bps: None,
            protocol_fee_bps: None,
        });
        let buy_after_network = order_params.buy_amount;
        let slippage = buy_after_network * U256::from(slippage_bps) / U256::from(ONE_HUNDRED_BPS);
        assert_eq!(result.after_slippage.buy_amount, buy_after_network - slippage);
    }

    #[test]
    fn buy_slippage_added_to_sell() {
        let order_params = buy_order();
        let slippage_bps = 200u32;
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: slippage_bps,
            partner_fee_bps: None,
            protocol_fee_bps: None,
        });
        let sell_after_network = order_params.sell_amount + order_params.fee_amount;
        let slippage = sell_after_network * U256::from(slippage_bps) / U256::from(ONE_HUNDRED_BPS);
        assert_eq!(result.after_slippage.sell_amount, sell_after_network + slippage);
    }

    // ── Protocol fee ─────────────────────────────────────────────────────────

    #[test]
    fn sell_protocol_fee_calculated_correctly() {
        let order_params = sell_order();
        let protocol_fee_bps = 20.0;
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: 0,
            partner_fee_bps: None,
            protocol_fee_bps: Some(protocol_fee_bps),
        });
        let bps = U256::from(protocol_fee_bps as u64);
        let denominator = U256::from(ONE_HUNDRED_BPS) - bps;
        let expected = order_params.buy_amount * bps / denominator;
        assert_eq!(result.costs.protocol_fee.amount, expected);
    }

    #[test]
    fn buy_protocol_fee_calculated_correctly() {
        let order_params = buy_order();
        let protocol_fee_bps = 20.0;
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: 0,
            partner_fee_bps: None,
            protocol_fee_bps: Some(protocol_fee_bps),
        });
        let sell_after_network = order_params.sell_amount + order_params.fee_amount;
        let bps = U256::from(protocol_fee_bps as u64);
        let denominator = U256::from(ONE_HUNDRED_BPS) + bps;
        let expected = sell_after_network * bps / denominator;
        assert_eq!(result.costs.protocol_fee.amount, expected);
    }

    #[test]
    fn sell_before_all_fees_includes_protocol_fee_once() {
        let order_params = sell_order();
        let protocol_fee_bps = 20.0;
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params,
            slippage_percent_bps: 0,
            partner_fee_bps: None,
            protocol_fee_bps: Some(protocol_fee_bps),
        });
        assert_eq!(
            result.before_all_fees.buy_amount,
            result.before_network_costs.buy_amount + result.costs.protocol_fee.amount
        );
    }

    #[test]
    fn buy_before_all_fees_includes_protocol_fee_once() {
        let order_params = buy_order();
        let protocol_fee_bps = 20.0;
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params,
            slippage_percent_bps: 0,
            partner_fee_bps: None,
            protocol_fee_bps: Some(protocol_fee_bps),
        });
        assert_eq!(
            result.before_all_fees.sell_amount,
            result.before_network_costs.sell_amount - result.costs.protocol_fee.amount
        );
    }

    #[test]
    fn sell_fractional_protocol_fee_bps() {
        let order_params = sell_order();
        let protocol_fee_bps: f64 = 0.003;
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: 0,
            partner_fee_bps: None,
            protocol_fee_bps: Some(protocol_fee_bps),
        });
        // Match the TypeScript test: bps = BigInt(0.003 * 100_000) = 300
        let bps = U256::from((protocol_fee_bps * HUNDRED_THOUSANDS as f64) as u64);
        let denominator = U256::from(ONE_HUNDRED_BPS) * U256::from(HUNDRED_THOUSANDS) - bps;
        let expected = order_params.buy_amount * bps / denominator;
        assert_eq!(result.costs.protocol_fee.amount, expected);
        // The TS test asserts amount == 5589
        assert_eq!(result.costs.protocol_fee.amount, U256::from(5589u64));
    }

    #[test]
    fn buy_fractional_protocol_fee_bps() {
        let order_params = buy_order();
        let protocol_fee_bps: f64 = 0.00071;
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params,
            slippage_percent_bps: 0,
            partner_fee_bps: None,
            protocol_fee_bps: Some(protocol_fee_bps),
        });
        // The TS test asserts amount == 12206189769
        assert_eq!(result.costs.protocol_fee.amount, U256::from(12_206_189_769u64));
    }

    #[test]
    fn sell_partner_fee_with_protocol_fee() {
        let order_params = sell_order();
        let protocol_fee_bps = 20.0;
        let partner_fee_bps = 100u32;
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: 0,
            partner_fee_bps: Some(partner_fee_bps),
            protocol_fee_bps: Some(protocol_fee_bps),
        });

        let buy_after = order_params.buy_amount;
        let protocol_bps = U256::from(protocol_fee_bps as u64);
        let protocol_denom = U256::from(ONE_HUNDRED_BPS) - protocol_bps;
        let protocol_fee = buy_after * protocol_bps / protocol_denom;

        let network_cost_in_buy = buy_after * order_params.fee_amount / order_params.sell_amount;
        let buy_before_all_fees = buy_after + network_cost_in_buy + protocol_fee;
        let expected_partner =
            buy_before_all_fees * U256::from(partner_fee_bps) / U256::from(ONE_HUNDRED_BPS);
        assert_eq!(result.costs.partner_fee.amount, expected_partner);
        assert_eq!(result.after_partner_fees.buy_amount, buy_after - expected_partner);
    }

    #[test]
    fn buy_partner_fee_with_protocol_fee() {
        let order_params = buy_order();
        let protocol_fee_bps = 20.0;
        let partner_fee_bps = 100u32;
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params: order_params.clone(),
            slippage_percent_bps: 0,
            partner_fee_bps: Some(partner_fee_bps),
            protocol_fee_bps: Some(protocol_fee_bps),
        });

        let sell_amount = order_params.sell_amount;
        let fee_amount = order_params.fee_amount;
        let sell_after_network = sell_amount + fee_amount;

        let protocol_bps = U256::from(protocol_fee_bps as u64);
        let protocol_denom = U256::from(ONE_HUNDRED_BPS) + protocol_bps;
        let protocol_fee = sell_after_network * protocol_bps / protocol_denom;

        let sell_before_all_fees = sell_amount - protocol_fee;
        let expected_partner =
            sell_before_all_fees * U256::from(partner_fee_bps) / U256::from(ONE_HUNDRED_BPS);
        assert_eq!(result.costs.partner_fee.amount, expected_partner);
        assert_eq!(result.after_partner_fees.sell_amount, sell_after_network + expected_partner);
    }

    // ── Zero protocol fee ───────────────────────────────────────────────────

    #[test]
    fn sell_protocol_fee_zero_bps() {
        let order_params = sell_order();
        let amount = get_protocol_fee_amount(&ProtocolFeeAmountParams {
            order_params,
            protocol_fee_bps: 0.0,
        });
        assert_eq!(amount, U256::ZERO);
    }

    #[test]
    fn sell_protocol_fee_negative_bps() {
        let order_params = sell_order();
        let amount = get_protocol_fee_amount(&ProtocolFeeAmountParams {
            order_params,
            protocol_fee_bps: -1.0,
        });
        assert_eq!(amount, U256::ZERO);
    }

    // ── Zero sell amount (division by zero guard) ───────────────────────────

    #[test]
    fn sell_order_zero_sell_amount() {
        let order_params = QuoteOrderParams {
            kind: OrderKind::Sell,
            sell_amount: U256::ZERO,
            buy_amount: U256::from(1000u64),
            fee_amount: U256::from(100u64),
        };
        let result = get_quote_amounts_and_costs(&QuoteAmountsAndCostsParams {
            order_params,
            slippage_percent_bps: 50,
            partner_fee_bps: None,
            protocol_fee_bps: None,
        });
        assert_eq!(result.costs.network_fee.amount_in_buy_currency, U256::ZERO);
    }

    // ── Partner fee zero bps ────────────────────────────────────────────────

    #[test]
    fn partner_fee_zero_bps() {
        let amounts =
            QuoteAmounts { sell_amount: U256::from(1000u64), buy_amount: U256::from(500u64) };
        let result = get_quote_amounts_after_partner_fee(&amounts, &amounts, true, 0);
        assert_eq!(result.partner_fee_amount, U256::ZERO);
        assert_eq!(result.after_partner_fees.buy_amount, U256::from(500u64));
    }

    // ── Slippage ────────────────────────────────────────────────────────────

    #[test]
    fn sell_slippage_reduces_buy_amount() {
        let amounts =
            QuoteAmounts { sell_amount: U256::from(1000u64), buy_amount: U256::from(10_000u64) };
        let result = get_quote_amounts_after_slippage(&amounts, true, 100); // 1% slippage
        assert_eq!(result.sell_amount, U256::from(1000u64));
        assert!(result.buy_amount < U256::from(10_000u64));
    }

    #[test]
    fn buy_slippage_increases_sell_amount() {
        let amounts =
            QuoteAmounts { sell_amount: U256::from(10_000u64), buy_amount: U256::from(1000u64) };
        let result = get_quote_amounts_after_slippage(&amounts, false, 100); // 1% slippage
        assert!(result.sell_amount > U256::from(10_000u64));
        assert_eq!(result.buy_amount, U256::from(1000u64));
    }
}
