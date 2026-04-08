//! Tests for `is_*`, `has_*`, and `*_ref` predicates across all API types.

use alloy_primitives::{B256, address};
use cow_rs::{
    Bundle, Order, OrderClass, OrderKind, OrderStatus, OrderUid, SigningScheme, SolverExecution,
    SubgraphToken, TotalSurplus,
    order_book::{CompetitionOrderStatusKind, QuoteSide},
};

// ── OrderStatus predicates ────────────────────────────────────────────────────

#[test]
fn order_status_open_is_pending() {
    let s = OrderStatus::Open;
    assert!(s.is_pending()); // Open counts as pending
    assert!(!s.is_fulfilled());
    assert!(!s.is_cancelled());
}

#[test]
fn order_status_fulfilled_is_fulfilled() {
    let s = OrderStatus::Fulfilled;
    assert!(s.is_fulfilled());
    assert!(!s.is_pending());
}

#[test]
fn order_status_cancelled_is_cancelled_and_terminal() {
    let s = OrderStatus::Cancelled;
    assert!(s.is_cancelled());
    assert!(s.is_terminal()); // Cancelled is a terminal state
}

#[test]
fn order_status_expired_is_expired() {
    let s = OrderStatus::Expired;
    assert!(s.is_expired());
    assert!(!matches!(s, OrderStatus::PresignaturePending));
}

#[test]
fn order_status_presignature_pending_is_pending() {
    let s = OrderStatus::PresignaturePending;
    assert!(s.is_pending()); // PresignaturePending is also "pending"
    assert!(matches!(s, OrderStatus::PresignaturePending));
}

#[test]
fn order_status_terminal_variants() {
    assert!(OrderStatus::Fulfilled.is_terminal());
    assert!(OrderStatus::Cancelled.is_terminal());
    assert!(OrderStatus::Expired.is_terminal());
    assert!(!OrderStatus::Open.is_terminal());
    assert!(!OrderStatus::PresignaturePending.is_terminal());
}

// ── OrderUid ──────────────────────────────────────────────────────────────────

#[test]
fn order_uid_from_string_and_str() {
    let from_str = OrderUid::from("0xabc");
    let from_string = OrderUid::from("0xabc".to_owned());
    assert_eq!(from_str.0, from_string.0);
}

#[test]
fn order_uid_deserializes_from_json() {
    let json = "\"0xabc123\"";
    let uid: OrderUid = serde_json::from_str(json).unwrap();
    assert_eq!(uid.0, "0xabc123");
}

// ── Order predicates ──────────────────────────────────────────────────────────

fn sample_order() -> Order {
    Order {
        uid: "0x".to_owned() + &"ab".repeat(56),
        owner: address!("1111111111111111111111111111111111111111"),
        creation_date: "2024-01-01T00:00:00.000Z".to_owned(),
        status: OrderStatus::Open,
        class: Some(OrderClass::Limit),
        sell_token: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
        buy_token: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        receiver: None,
        sell_amount: "1000000".to_owned(),
        buy_amount: "1000000000000000".to_owned(),
        valid_to: 9_999_999u32,
        app_data: "0x0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        full_app_data: None,
        fee_amount: "0".to_owned(),
        kind: OrderKind::Sell,
        partially_fillable: false,
        executed_sell_amount: "0".to_owned(),
        executed_buy_amount: "0".to_owned(),
        executed_sell_amount_before_fees: "0".to_owned(),
        executed_fee_amount: "0".to_owned(),
        invalidated: false,
        is_liquidity_order: None,
        signing_scheme: SigningScheme::Eip712,
        signature: "0xabcd".into(),
        interactions: None,
        total_fee: None,
        full_fee_amount: None,
        available_balance: None,
        quote_id: None,
        executed_fee: None,
        ethflow_data: None,
        onchain_order_data: None,
        onchain_user: None,
    }
}

#[test]
fn order_has_receiver_false_when_none() {
    assert!(!sample_order().has_receiver());
}

#[test]
fn order_has_receiver_true_when_some() {
    let mut order = sample_order();
    order.receiver = Some(address!("2222222222222222222222222222222222222222"));
    assert!(order.has_receiver());
}

#[test]
fn order_is_partially_fillable_false() {
    assert!(!sample_order().is_partially_fillable());
}

#[test]
fn order_is_open_status() {
    let order = sample_order();
    assert!(order.status.is_pending());
    assert!(matches!(order.status, OrderStatus::Open));
}

#[test]
fn order_is_sell() {
    assert!(sample_order().is_sell());
}

#[test]
fn order_has_full_app_data_false_when_none() {
    assert!(!sample_order().has_full_app_data());
}

#[test]
fn order_has_ethflow_false_when_none() {
    assert!(!sample_order().has_ethflow_data());
}

#[test]
fn order_has_class_true_when_set() {
    assert!(sample_order().has_class());
}

#[test]
fn order_is_not_invalidated() {
    assert!(!sample_order().is_invalidated());
}

// ── TotalSurplus ──────────────────────────────────────────────────────────────

#[test]
fn total_surplus_deserializes_from_json() {
    let json = r#"{"totalSurplus":"12345678"}"#;
    let surplus: TotalSurplus = serde_json::from_str(json).unwrap();
    assert_eq!(surplus.total_surplus, "12345678");
}

#[test]
fn total_surplus_as_str() {
    let surplus = TotalSurplus::new("99999");
    assert_eq!(surplus.as_str(), "99999");
}

// ── SubgraphToken predicates ──────────────────────────────────────────────────

fn make_subgraph_token() -> SubgraphToken {
    SubgraphToken {
        id: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".into(),
        address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".into(),
        first_trade_timestamp: "1609459200".into(),
        name: "Wrapped Ether".into(),
        symbol: "WETH".into(),
        decimals: "18".into(),
        total_volume: "1000000000000000000".into(),
        price_eth: "1.0".into(),
        price_usd: "2500.00".into(),
        number_of_trades: "42".into(),
    }
}

#[test]
fn subgraph_token_symbol_ref() {
    let token = make_subgraph_token();
    assert_eq!(token.symbol_ref(), "WETH");
    assert_eq!(token.address_ref(), "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
}

// ── Bundle predicates ─────────────────────────────────────────────────────────

#[test]
fn bundle_eth_price_usd_ref() {
    let bundle = Bundle { id: "1".to_owned(), eth_price_usd: "2500.00".into() };
    assert_eq!(bundle.eth_price_usd_ref(), "2500.00");
}

// ── CompetitionOrderStatusKind ────────────────────────────────────────────────

#[test]
fn competition_status_open_is_open() {
    assert!(CompetitionOrderStatusKind::Open.is_open());
}

#[test]
fn competition_status_traded_is_traded() {
    assert!(CompetitionOrderStatusKind::Traded.is_traded());
}

// ── QuoteSide ─────────────────────────────────────────────────────────────────

#[test]
fn quote_side_sell_is_sell() {
    let side = QuoteSide::sell("1000");
    assert!(side.is_sell());
    assert!(!side.is_buy());
}

#[test]
fn quote_side_buy_is_buy() {
    let side = QuoteSide::buy("500");
    assert!(side.is_buy());
    assert!(!side.is_sell());
}

// ── SolverExecution ───────────────────────────────────────────────────────────

#[test]
fn solver_execution_has_no_sell_amount_when_none() {
    let exec = SolverExecution {
        solver: "0xsolver".into(),
        executed_sell_amount: None,
        executed_buy_amount: None,
    };
    assert!(!exec.has_executed_sell_amount());
}

#[test]
fn solver_execution_has_sell_amount_when_some() {
    let exec = SolverExecution {
        solver: "0xsolver".into(),
        executed_sell_amount: Some("1000".into()),
        executed_buy_amount: None,
    };
    assert!(exec.has_executed_sell_amount());
    assert!(!exec.has_executed_buy_amount());
}

#[test]
fn solver_execution_has_both_amounts() {
    let exec = SolverExecution {
        solver: "0xsolver".into(),
        executed_sell_amount: Some("1000".into()),
        executed_buy_amount: Some("900".into()),
    };
    assert!(exec.both_amounts_available());
}

// ── B256 zero check ───────────────────────────────────────────────────────────

#[test]
fn b256_zero_is_zero() {
    assert_eq!(B256::ZERO, B256::ZERO);
}
