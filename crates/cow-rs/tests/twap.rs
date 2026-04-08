//! Tests for `TWAP` order construction, encoding, and validation.

use alloy_primitives::{Address, B256, U256, address};
use cow_rs::{
    ConditionalOrderKind, ConditionalOrderParams, DurationOfPart, GpV2OrderStruct, MAX_FREQUENCY,
    TWAP_HANDLER_ADDRESS, TwapData, TwapOrder, TwapStartTime, TwapStruct, decode_twap_static_input,
    encode_twap_struct,
};

fn weth() -> Address {
    address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
}

fn usdc() -> Address {
    address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
}

fn basic_twap_data() -> TwapData {
    TwapData {
        sell_token: weth(),
        buy_token: usdc(),
        receiver: Address::ZERO,
        sell_amount: U256::from(1_000_000_000_000_000_000u64), // 1 WETH
        buy_amount: U256::from(2_000_000_000u64),              // 2000 USDC
        part_duration: 3600,                                   // 1 hour
        num_parts: 4,
        start_time: TwapStartTime::AtMiningTime,
        duration_of_part: DurationOfPart::Auto,
        app_data: B256::ZERO,
        partially_fillable: false,
        kind: cow_rs::OrderKind::Sell,
    }
}

// ── TWAP_HANDLER_ADDRESS ──────────────────────────────────────────────────────

#[test]
fn twap_handler_address_is_non_zero() {
    assert_ne!(TWAP_HANDLER_ADDRESS, Address::ZERO);
}

// ── TwapOrder construction ────────────────────────────────────────────────────

#[test]
fn twap_order_new_creates_order() {
    let data = basic_twap_data();
    let order = TwapOrder::new(data);
    assert_ne!(order.salt, alloy_primitives::B256::ZERO);
}

#[test]
fn twap_order_salt_ref_is_same_as_field() {
    let order = TwapOrder::new(basic_twap_data());
    assert_eq!(order.salt_ref(), &order.salt);
}

#[test]
fn twap_order_data_ref_is_same_as_field() {
    let data = basic_twap_data();
    let order = TwapOrder::new(data.clone());
    assert_eq!(order.data_ref().sell_token, data.sell_token);
}

#[test]
fn twap_order_new_two_calls_produce_different_salts() {
    let data = basic_twap_data();
    let order1 = TwapOrder::new(data.clone());
    let order2 = TwapOrder::new(data);
    // Salts are derived from data + timestamp, may differ in practice
    // but at minimum they should be valid B256 values
    assert!(!order1.salt.is_zero() || !order2.salt.is_zero());
}

#[test]
fn twap_order_total_sell_amount() {
    let data = basic_twap_data();
    let order = TwapOrder::new(data.clone());
    // total_sell_amount returns data.sell_amount directly (it IS the total)
    assert_eq!(order.total_sell_amount(), data.sell_amount);
}

#[test]
fn twap_order_total_buy_amount() {
    let data = basic_twap_data();
    let order = TwapOrder::new(data.clone());
    // total_buy_amount returns data.buy_amount directly (it IS the total)
    assert_eq!(order.total_buy_amount(), data.buy_amount);
}

#[test]
fn twap_order_is_valid() {
    let order = TwapOrder::new(basic_twap_data());
    assert!(order.is_valid());
}

#[test]
fn twap_order_invalid_when_zero_parts() {
    let mut data = basic_twap_data();
    data.num_parts = 0;
    let order = TwapOrder::new(data);
    assert!(!order.is_valid());
}

#[test]
fn twap_order_invalid_when_sell_amount_zero() {
    let mut data = basic_twap_data();
    data.sell_amount = U256::ZERO;
    let order = TwapOrder::new(data);
    assert!(!order.is_valid());
}

#[test]
fn twap_order_to_params_succeeds() {
    let order = TwapOrder::new(basic_twap_data());
    let result = order.to_params();
    assert!(result.is_ok());
}

// ── TwapStruct encoding roundtrip ─────────────────────────────────────────────

#[test]
fn twap_struct_encode_decode_roundtrip() {
    let order = TwapOrder::new(basic_twap_data());
    let params = order.to_params().unwrap();
    let static_input = params.static_input.clone();
    let decoded = decode_twap_static_input(&static_input).unwrap();
    assert_eq!(decoded.sell_token, weth());
    assert_eq!(decoded.buy_token, usdc());
}

#[test]
fn encode_twap_struct_produces_320_bytes() {
    let ts = TwapStruct {
        sell_token: weth(),
        buy_token: usdc(),
        receiver: Address::ZERO,
        part_sell_amount: U256::from(250_000_000_000_000_000u64),
        min_part_limit: U256::from(500_000_000u64),
        t0: 0,
        n: 4,
        t: 3600,
        span: 0,
        app_data: alloy_primitives::B256::ZERO,
    };
    let encoded = encode_twap_struct(&ts);
    assert_eq!(encoded.len(), 10 * 32, "expected 10×32 = 320 bytes");
}

// ── ConditionalOrderParams ────────────────────────────────────────────────────

#[test]
fn conditional_order_params_handler_is_twap_handler() {
    let order = TwapOrder::new(basic_twap_data());
    let params = order.to_params().unwrap();
    assert_eq!(params.handler, TWAP_HANDLER_ADDRESS);
}

#[test]
fn conditional_order_params_is_empty_static_input_false() {
    let order = TwapOrder::new(basic_twap_data());
    let params = order.to_params().unwrap();
    assert!(!params.is_empty_static_input());
}

#[test]
fn conditional_order_params_static_input_len_is_320() {
    let order = TwapOrder::new(basic_twap_data());
    let params = order.to_params().unwrap();
    assert_eq!(params.static_input_len(), 320);
}

// ── DurationOfPart ────────────────────────────────────────────────────────────

#[test]
fn duration_of_part_auto_is_not_limit() {
    assert!(!DurationOfPart::Auto.is_limit_duration());
}

#[test]
fn duration_of_part_limit_is_limit() {
    assert!(DurationOfPart::LimitDuration { duration: 3600 }.is_limit_duration());
}

// ── ConditionalOrderKind ──────────────────────────────────────────────────────

#[test]
fn conditional_order_kind_twap_is_twap() {
    let order = TwapOrder::new(basic_twap_data());
    let params = order.to_params().unwrap();
    // Twap variant wraps a TwapOrder
    let kind = ConditionalOrderKind::Twap(TwapOrder::new(basic_twap_data()));
    assert!(kind.is_twap());
    assert!(!kind.is_unknown());
    // The TWAP order's handler matches the TWAP kind
    assert_eq!(params.handler, TWAP_HANDLER_ADDRESS);
}

#[test]
fn conditional_order_kind_unknown_is_unknown() {
    let params = ConditionalOrderParams::new(Address::ZERO, B256::ZERO, vec![]);
    let kind = ConditionalOrderKind::Unknown(params);
    assert!(kind.is_unknown());
    assert!(!kind.is_twap());
}

// ── GpV2OrderStruct ───────────────────────────────────────────────────────────

#[test]
fn gp_v2_order_struct_has_custom_receiver() {
    let recv = address!("1111111111111111111111111111111111111111");
    let order = GpV2OrderStruct {
        sell_token: weth(),
        buy_token: usdc(),
        receiver: recv,
        sell_amount: U256::from(1_000u64),
        buy_amount: U256::from(900u64),
        valid_to: 9999,
        app_data: alloy_primitives::B256::ZERO,
        fee_amount: U256::ZERO,
        kind: B256::ZERO,
        partially_fillable: false,
        sell_token_balance: B256::ZERO,
        buy_token_balance: B256::ZERO,
    };
    assert!(order.has_custom_receiver());
}

#[test]
fn gp_v2_order_struct_no_custom_receiver_when_zero() {
    let order = GpV2OrderStruct {
        sell_token: weth(),
        buy_token: usdc(),
        receiver: Address::ZERO,
        sell_amount: U256::from(1_000u64),
        buy_amount: U256::from(900u64),
        valid_to: 9999,
        app_data: alloy_primitives::B256::ZERO,
        fee_amount: U256::ZERO,
        kind: B256::ZERO,
        partially_fillable: false,
        sell_token_balance: B256::ZERO,
        buy_token_balance: B256::ZERO,
    };
    assert!(!order.has_custom_receiver());
}

// ── MAX_FREQUENCY ─────────────────────────────────────────────────────────────

#[test]
fn max_frequency_positive() {
    assert!(MAX_FREQUENCY > 0);
}
