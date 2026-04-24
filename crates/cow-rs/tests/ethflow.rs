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
//! Tests for the `EthFlow` calldata encoder.

use alloy_primitives::{Address, B256, U256, address, keccak256};
use cow_rs::{
    OnchainOrderData,
    ethflow::{
        EthFlowOrderData, build_eth_flow_transaction, encode_eth_flow_create_order,
        is_eth_flow_order_data,
    },
};

fn sample_order() -> EthFlowOrderData {
    EthFlowOrderData {
        buy_token: address!("1111111111111111111111111111111111111111"),
        receiver: address!("2222222222222222222222222222222222222222"),
        sell_amount: U256::from(1_000_000_u64),
        buy_amount: U256::from(500_000_u64),
        app_data: B256::ZERO,
        fee_amount: U256::ZERO,
        valid_to: 9_999_999_u32,
        partially_fillable: false,
        quote_id: 42_i64,
    }
}

fn expected_selector() -> [u8; 4] {
    // `quoteId` is the last field of the `EthFlowOrder` struct, not a separate
    // function parameter — the on-chain contract declares
    // `createOrder(EthFlowOrder order)`.
    let h = keccak256(
        b"createOrder((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64))",
    );
    [h[0], h[1], h[2], h[3]]
}

#[test]
fn encode_eth_flow_create_order_selector_correct() {
    let cd = encode_eth_flow_create_order(&sample_order());
    assert_eq!(&cd[..4], &expected_selector());
}

#[test]
fn encode_eth_flow_create_order_length_is_292() {
    let cd = encode_eth_flow_create_order(&sample_order());
    assert_eq!(cd.len(), 292);
}

#[test]
fn encode_eth_flow_buy_token_correctly_padded() {
    let order = sample_order();
    let cd = encode_eth_flow_create_order(&order);
    // buy_token is in word 0 (bytes 4..36): address in last 20 bytes of the word
    assert_eq!(&cd[16..36], order.buy_token.as_slice());
}

#[test]
fn encode_eth_flow_sell_amount_in_value() {
    let order = sample_order();
    let tx = build_eth_flow_transaction(Address::ZERO, &order);
    assert_eq!(tx.value, order.sell_amount);
}

#[test]
fn is_eth_flow_order_data_none() {
    assert!(!is_eth_flow_order_data(None));
}

#[test]
fn is_eth_flow_order_data_some() {
    let data = OnchainOrderData::new(Address::ZERO);
    assert!(is_eth_flow_order_data(Some(&data)));
}

#[test]
fn encode_eth_flow_positive_quote_id() {
    let mut order = sample_order();
    order.quote_id = 12345_i64;
    let cd = encode_eth_flow_create_order(&order);
    // last word = bytes 260..292, last 8 bytes = quote_id big-endian
    let last_word = &cd[260..292];
    let got = i64::from_be_bytes(last_word[24..32].try_into().unwrap());
    assert_eq!(got, 12345_i64);
}

#[test]
fn encode_eth_flow_negative_quote_id_sign_extended() {
    let mut order = sample_order();
    order.quote_id = -1_i64;
    let cd = encode_eth_flow_create_order(&order);
    // For -1 every byte of the 32-byte word should be 0xFF.
    let last_word = &cd[260..292];
    assert!(last_word.iter().all(|&b| b == 0xff), "expected all 0xFF, got {last_word:?}");
}

#[test]
fn encode_eth_flow_large_negative_quote_id() {
    let mut order = sample_order();
    order.quote_id = i64::MIN;
    let cd = encode_eth_flow_create_order(&order);
    let last_word = &cd[260..292];
    // i64::MIN = 0x80_00_00_00_00_00_00_00 — sign-extended to 32 bytes:
    // bytes 0..24 are all 0xFF (sign extension), bytes 24..32 are i64::MIN big-endian.
    assert!(last_word[..24].iter().all(|&b| b == 0xff), "expected sign extension 0xFF");
    // Last 8 bytes = 0x80_00_00_00_00_00_00_00
    assert_eq!(last_word[24], 0x80);
    assert!(last_word[25..32].iter().all(|&b| b == 0x00));
}

#[test]
fn build_eth_flow_transaction_target_is_contract() {
    let contract = address!("ba3cb449bd2b4adddbc894d8697f5170800eadec");
    let order = sample_order();
    let tx = build_eth_flow_transaction(contract, &order);
    assert_eq!(tx.to, contract);
}

#[test]
fn build_eth_flow_transaction_data_length_is_292() {
    let order = sample_order();
    let tx = build_eth_flow_transaction(Address::ZERO, &order);
    assert_eq!(tx.data.len(), 292);
}
