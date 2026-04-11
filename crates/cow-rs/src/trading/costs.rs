//! Fee and amounts breakdown calculation for quoted trades.

use alloy_primitives::U256;

use crate::{
    error::CowError,
    order_book::types::OrderQuoteResponse,
    trading::types::{Amounts, NetworkFee, PartnerFeeCost, ProtocolFeeCost, QuoteAmountsAndCosts},
    types::OrderKind,
};

/// Parse a decimal integer string into [`U256`].
///
/// # Arguments
///
/// * `s` — the decimal integer string to parse.
/// * `field` — a label used in the error message if parsing fails.
///
/// # Returns
///
/// The parsed [`U256`] value, or a [`CowError::Api`] if `s` is not a valid integer.
fn parse_u256(s: &str, field: &'static str) -> Result<U256, CowError> {
    s.parse::<U256>().map_err(|_e| CowError::Api { status: 0, body: format!("invalid {field}") })
}

/// Compute the slippage-adjusted buy amount for a sell order.
///
/// `buy_amount * (10_000 - slippage_bps) / 10_000`
///
/// # Arguments
///
/// * `buy_amount` — the original buy amount in atoms.
/// * `slippage_bps` — slippage tolerance in basis points.
///
/// # Returns
///
/// The reduced buy amount after applying slippage.
fn apply_slippage_sell(buy_amount: U256, slippage_bps: u32) -> U256 {
    buy_amount * U256::from(10_000u32 - slippage_bps) / U256::from(10_000u32)
}

/// Compute the slippage-inflated sell amount for a buy order.
///
/// `sell_amount * (10_000 + slippage_bps) / 10_000`
///
/// # Arguments
///
/// * `sell_amount` — the original sell amount in atoms.
/// * `slippage_bps` — slippage tolerance in basis points.
///
/// # Returns
///
/// The inflated sell amount after applying slippage.
fn apply_slippage_buy(sell_amount: U256, slippage_bps: u32) -> U256 {
    sell_amount * U256::from(10_000u32 + slippage_bps) / U256::from(10_000u32)
}

/// Derive [`QuoteAmountsAndCosts`] from a raw quote response.
///
/// # Arguments
///
/// * `quote` — the raw [`OrderQuoteResponse`] from the orderbook API.
/// * `slippage_bps` — the slippage tolerance to apply (default 50 = 0.5 %).
///
/// # Returns
///
/// A fully populated [`QuoteAmountsAndCosts`] with before/after amounts and fee breakdown,
/// or a [`CowError`] if any amount field in the quote cannot be parsed.
pub(crate) fn compute_quote_amounts_and_costs(
    quote: &OrderQuoteResponse,
    slippage_bps: u32,
) -> Result<QuoteAmountsAndCosts, CowError> {
    let is_sell = matches!(quote.quote.kind, OrderKind::Sell);
    let sell_amount = parse_u256(&quote.quote.sell_amount, "sellAmount")?;
    let buy_amount = parse_u256(&quote.quote.buy_amount, "buyAmount")?;
    let fee_amount = parse_u256(&quote.quote.fee_amount, "feeAmount")?;

    // Before network costs: the gross amounts (excluding fee).
    // For a sell order: gross_sell = sell_amount + fee_amount.
    let (gross_sell, gross_buy) =
        if is_sell { (sell_amount + fee_amount, buy_amount) } else { (sell_amount, buy_amount) };

    let before_network_costs = Amounts { sell_amount: gross_sell, buy_amount: gross_buy };
    let after_network_costs = Amounts { sell_amount, buy_amount };

    // Slippage is applied to the output side.
    let after_slippage = if is_sell {
        Amounts { sell_amount, buy_amount: apply_slippage_sell(buy_amount, slippage_bps) }
    } else {
        Amounts { sell_amount: apply_slippage_buy(sell_amount, slippage_bps), buy_amount }
    };

    // Estimated buy-currency fee (rough approximation via ratio).
    let fee_in_buy =
        if gross_sell.is_zero() { U256::ZERO } else { fee_amount * buy_amount / gross_sell };

    Ok(QuoteAmountsAndCosts {
        is_sell,
        // When no protocol fee is provided, before_all_fees equals before_network_costs.
        before_all_fees: before_network_costs,
        before_network_costs,
        after_network_costs,
        // When no partner fee is configured, after_partner_fees equals after_network_costs.
        after_partner_fees: after_network_costs,
        after_slippage,
        network_fee: NetworkFee {
            amount_in_sell_currency: fee_amount,
            amount_in_buy_currency: fee_in_buy,
        },
        partner_fee: PartnerFeeCost::default(),
        protocol_fee: ProtocolFeeCost::default(),
    })
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;

    use crate::{order_book::types::QuoteData, types::TokenBalance};

    use super::*;

    fn make_sell_quote(sell: &str, buy: &str, fee: &str) -> OrderQuoteResponse {
        OrderQuoteResponse {
            quote: QuoteData {
                sell_token: Address::repeat_byte(0x11),
                buy_token: Address::repeat_byte(0x22),
                receiver: None,
                sell_amount: sell.to_owned(),
                buy_amount: buy.to_owned(),
                valid_to: 1_700_000_000,
                app_data: "0x00".to_owned(),
                fee_amount: fee.to_owned(),
                kind: OrderKind::Sell,
                partially_fillable: false,
                sell_token_balance: TokenBalance::Erc20,
                buy_token_balance: TokenBalance::Erc20,
            },
            from: Address::ZERO,
            expiration: "2025-01-01T00:00:00Z".to_owned(),
            id: Some(1),
            verified: true,
            protocol_fee_bps: None,
        }
    }

    fn make_buy_quote(sell: &str, buy: &str, fee: &str) -> OrderQuoteResponse {
        let mut q = make_sell_quote(sell, buy, fee);
        q.quote.kind = OrderKind::Buy;
        q
    }

    #[test]
    fn sell_order_basic() {
        let quote = make_sell_quote("1000", "2000", "100");
        let result = compute_quote_amounts_and_costs(&quote, 50).unwrap();

        assert!(result.is_sell);
        // before network costs: gross_sell = 1000 + 100 = 1100, gross_buy = 2000
        assert_eq!(result.before_network_costs.sell_amount, U256::from(1100u64));
        assert_eq!(result.before_network_costs.buy_amount, U256::from(2000u64));
        // after network costs: sell=1000, buy=2000
        assert_eq!(result.after_network_costs.sell_amount, U256::from(1000u64));
        assert_eq!(result.after_network_costs.buy_amount, U256::from(2000u64));
        // slippage on buy side: 2000 * (10000 - 50) / 10000 = 2000 * 9950 / 10000 = 1990
        assert_eq!(result.after_slippage.sell_amount, U256::from(1000u64));
        assert_eq!(result.after_slippage.buy_amount, U256::from(1990u64));
        // network fee
        assert_eq!(result.network_fee.amount_in_sell_currency, U256::from(100u64));
    }

    #[test]
    fn buy_order_basic() {
        let quote = make_buy_quote("1000", "2000", "100");
        let result = compute_quote_amounts_and_costs(&quote, 50).unwrap();

        assert!(!result.is_sell);
        // For buy order: gross_sell = sell_amount (1000), gross_buy = buy_amount (2000)
        assert_eq!(result.before_network_costs.sell_amount, U256::from(1000u64));
        assert_eq!(result.before_network_costs.buy_amount, U256::from(2000u64));
        // slippage on sell side: 1000 * (10000 + 50) / 10000 = 1000 * 10050 / 10000 = 1005
        assert_eq!(result.after_slippage.sell_amount, U256::from(1005u64));
        assert_eq!(result.after_slippage.buy_amount, U256::from(2000u64));
    }

    #[test]
    fn zero_fee() {
        let quote = make_sell_quote("1000", "2000", "0");
        let result = compute_quote_amounts_and_costs(&quote, 0).unwrap();

        assert_eq!(result.network_fee.amount_in_sell_currency, U256::ZERO);
        assert_eq!(result.network_fee.amount_in_buy_currency, U256::ZERO);
        // No slippage either
        assert_eq!(result.after_slippage.buy_amount, U256::from(2000u64));
    }

    #[test]
    fn zero_gross_sell_does_not_panic() {
        // Edge case: sell_amount=0, fee=0 => gross_sell=0 => fee_in_buy should be 0
        let quote = make_sell_quote("0", "1000", "0");
        let result = compute_quote_amounts_and_costs(&quote, 50).unwrap();
        assert_eq!(result.network_fee.amount_in_buy_currency, U256::ZERO);
    }

    #[test]
    fn invalid_amount_returns_error() {
        let mut quote = make_sell_quote("1000", "2000", "100");
        quote.quote.sell_amount = "not_a_number".to_owned();
        assert!(compute_quote_amounts_and_costs(&quote, 50).is_err());
    }

    #[test]
    fn partner_and_protocol_fees_default_to_zero() {
        let quote = make_sell_quote("1000", "2000", "100");
        let result = compute_quote_amounts_and_costs(&quote, 50).unwrap();
        // partner_fee and protocol_fee should be default (zero) so these amounts match
        assert_eq!(result.after_partner_fees.sell_amount, result.after_network_costs.sell_amount);
        assert_eq!(result.after_partner_fees.buy_amount, result.after_network_costs.buy_amount);
        assert_eq!(result.before_all_fees.sell_amount, result.before_network_costs.sell_amount);
        assert_eq!(result.before_all_fees.buy_amount, result.before_network_costs.buy_amount);
    }
}
