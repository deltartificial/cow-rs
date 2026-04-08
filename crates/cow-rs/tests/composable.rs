//! Integration tests for composable (conditional) orders:
//!
//! - [`Multiplexer`] add / remove / root / proof
//! - [`ConditionalOrderFactory`] decode round-trips for `TWAP`, `StopLoss`, `GAT`
//! - [`StopLossOrder`] encode / decode / validation
//! - [`GatOrder`] encode / decode / validation

use alloy_primitives::{Address, B256, U256, address};
use cow_rs::{
    ConditionalOrderFactory, ConditionalOrderKind, ConditionalOrderParams, DurationOfPart,
    GAT_HANDLER_ADDRESS, GatData, GatOrder, GpV2OrderStruct, Multiplexer, OrderKind, ProofLocation,
    STOP_LOSS_HANDLER_ADDRESS, StopLossData, StopLossOrder, TWAP_HANDLER_ADDRESS, TwapData,
    TwapOrder, TwapStartTime,
    composable::{
        decode_gat_static_input, decode_stop_loss_static_input, encode_gat_struct,
        encode_stop_loss_struct,
    },
};

// ── Fixture helpers ───────────────────────────────────────────────────────────

const fn weth() -> Address {
    address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
}

const fn usdc() -> Address {
    address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
}

const fn oracle_a() -> Address {
    address!("5f4eC3Df9cbd43714FE2740f5E3616155c5b8419")
}

const fn oracle_b() -> Address {
    address!("986b5E1e1755e3C2440e960477f25201B0a8bbD4")
}

fn make_twap_data() -> TwapData {
    TwapData {
        sell_token: weth(),
        buy_token: usdc(),
        receiver: Address::ZERO,
        sell_amount: U256::from(1_000_000_000_000_000_000u64),
        buy_amount: U256::from(2_000_000_000u64),
        part_duration: 3_600,
        num_parts: 4,
        start_time: TwapStartTime::AtMiningTime,
        duration_of_part: DurationOfPart::Auto,
        app_data: B256::ZERO,
        partially_fillable: false,
        kind: OrderKind::Sell,
    }
}

fn make_stop_loss_data() -> StopLossData {
    StopLossData {
        sell_token: weth(),
        buy_token: usdc(),
        sell_amount: U256::from(1_000_000_000_000_000_000u64),
        buy_amount: U256::from(1_800_000_000u64),
        app_data: B256::ZERO,
        receiver: Address::ZERO,
        is_sell_order: true,
        is_partially_fillable: false,
        valid_to: 9_999_999,
        strike_price: U256::from(1_800_000_000_000_000_000_000u128), // $1 800 / ETH
        sell_token_price_oracle: oracle_a(),
        buy_token_price_oracle: oracle_b(),
        token_amount_in_eth: false,
    }
}

fn make_gat_order_struct() -> GpV2OrderStruct {
    GpV2OrderStruct {
        sell_token: weth(),
        buy_token: usdc(),
        receiver: Address::ZERO,
        sell_amount: U256::from(500_000_000_000_000_000u64),
        buy_amount: U256::from(900_000_000u64),
        valid_to: 9_999_999,
        app_data: B256::ZERO,
        fee_amount: U256::ZERO,
        kind: B256::ZERO,
        partially_fillable: false,
        sell_token_balance: B256::ZERO,
        buy_token_balance: B256::ZERO,
    }
}

fn make_gat_data() -> GatData {
    GatData {
        order: make_gat_order_struct(),
        start_time: 1_700_000_000,
        tx_deadline: 1_700_003_600,
    }
}

const fn make_params(handler: Address, static_input: Vec<u8>) -> ConditionalOrderParams {
    ConditionalOrderParams { handler, salt: B256::ZERO, static_input }
}

// ══════════════════════════════════════════════════════════════════════════════
// StopLossOrder
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn stop_loss_encode_produces_416_bytes() {
    let data = make_stop_loss_data();
    let enc = encode_stop_loss_struct(&data);
    assert_eq!(enc.len(), 13 * 32, "expected 416 bytes");
}

#[test]
fn stop_loss_encode_decode_roundtrip() {
    let data = make_stop_loss_data();
    let enc = encode_stop_loss_struct(&data);
    let dec = decode_stop_loss_static_input(&enc).unwrap();

    assert_eq!(dec.sell_token, data.sell_token);
    assert_eq!(dec.buy_token, data.buy_token);
    assert_eq!(dec.sell_amount, data.sell_amount);
    assert_eq!(dec.buy_amount, data.buy_amount);
    assert_eq!(dec.app_data, data.app_data);
    assert_eq!(dec.receiver, data.receiver);
    assert_eq!(dec.is_sell_order, data.is_sell_order);
    assert_eq!(dec.is_partially_fillable, data.is_partially_fillable);
    assert_eq!(dec.valid_to, data.valid_to);
    assert_eq!(dec.strike_price, data.strike_price);
    assert_eq!(dec.sell_token_price_oracle, data.sell_token_price_oracle);
    assert_eq!(dec.buy_token_price_oracle, data.buy_token_price_oracle);
    assert_eq!(dec.token_amount_in_eth, data.token_amount_in_eth);
}

#[test]
fn stop_loss_decode_too_short_is_error() {
    let result = decode_stop_loss_static_input(&[0u8; 100]);
    assert!(result.is_err());
}

#[test]
fn stop_loss_order_new_generates_salt() {
    let order = StopLossOrder::new(make_stop_loss_data());
    // deterministic_salt hashes sell_token + buy_token + sell_amount + strike_price
    assert_ne!(order.salt, B256::ZERO);
}

#[test]
fn stop_loss_order_with_explicit_salt() {
    let salt = B256::repeat_byte(0xaa);
    let order = StopLossOrder::with_salt(make_stop_loss_data(), salt);
    assert_eq!(order.salt, salt);
}

#[test]
fn stop_loss_order_salt_ref_matches_field() {
    let order = StopLossOrder::new(make_stop_loss_data());
    assert_eq!(order.salt_ref(), &order.salt);
}

#[test]
fn stop_loss_order_data_ref_matches_field() {
    let data = make_stop_loss_data();
    let order = StopLossOrder::new(data.clone());
    assert_eq!(order.data_ref().sell_token, data.sell_token);
    assert_eq!(order.data_ref().strike_price, data.strike_price);
}

#[test]
fn stop_loss_order_is_valid_true_for_good_data() {
    let order = StopLossOrder::new(make_stop_loss_data());
    assert!(order.is_valid());
}

#[test]
fn stop_loss_order_invalid_when_same_token() {
    let mut data = make_stop_loss_data();
    data.buy_token = data.sell_token;
    let order = StopLossOrder::new(data);
    assert!(!order.is_valid());
}

#[test]
fn stop_loss_order_invalid_when_zero_sell_amount() {
    let mut data = make_stop_loss_data();
    data.sell_amount = U256::ZERO;
    let order = StopLossOrder::new(data);
    assert!(!order.is_valid());
}

#[test]
fn stop_loss_order_invalid_when_zero_buy_amount() {
    let mut data = make_stop_loss_data();
    data.buy_amount = U256::ZERO;
    let order = StopLossOrder::new(data);
    assert!(!order.is_valid());
}

#[test]
fn stop_loss_to_params_sets_handler_address() {
    let order = StopLossOrder::new(make_stop_loss_data());
    let params = order.to_params().unwrap();
    assert_eq!(params.handler, STOP_LOSS_HANDLER_ADDRESS);
    assert_eq!(params.static_input.len(), 416);
}

#[test]
fn stop_loss_handler_address_non_zero() {
    assert_ne!(STOP_LOSS_HANDLER_ADDRESS, Address::ZERO);
}

// ══════════════════════════════════════════════════════════════════════════════
// GatOrder
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn gat_encode_produces_448_bytes() {
    let data = make_gat_data();
    let enc = encode_gat_struct(&data);
    assert_eq!(enc.len(), 14 * 32, "expected 448 bytes");
}

#[test]
fn gat_encode_decode_roundtrip() {
    let data = make_gat_data();
    let enc = encode_gat_struct(&data);
    let dec = decode_gat_static_input(&enc).unwrap();

    assert_eq!(dec.start_time, data.start_time);
    assert_eq!(dec.tx_deadline, data.tx_deadline);
    assert_eq!(dec.order.sell_token, data.order.sell_token);
    assert_eq!(dec.order.buy_token, data.order.buy_token);
    assert_eq!(dec.order.sell_amount, data.order.sell_amount);
    assert_eq!(dec.order.buy_amount, data.order.buy_amount);
    assert_eq!(dec.order.valid_to, data.order.valid_to);
    assert_eq!(dec.order.partially_fillable, data.order.partially_fillable);
}

#[test]
fn gat_decode_too_short_is_error() {
    let result = decode_gat_static_input(&[0u8; 100]);
    assert!(result.is_err());
}

#[test]
fn gat_order_new_generates_salt() {
    let order = GatOrder::new(make_gat_data());
    assert_ne!(order.salt, B256::ZERO);
}

#[test]
fn gat_order_with_explicit_salt() {
    let salt = B256::repeat_byte(0xbb);
    let order = GatOrder::with_salt(make_gat_data(), salt);
    assert_eq!(order.salt, salt);
}

#[test]
fn gat_order_salt_ref_matches_field() {
    let order = GatOrder::new(make_gat_data());
    assert_eq!(order.salt_ref(), &order.salt);
}

#[test]
fn gat_order_data_ref_matches_field() {
    let data = make_gat_data();
    let order = GatOrder::new(data.clone());
    assert_eq!(order.data_ref().start_time, data.start_time);
    assert_eq!(order.data_ref().tx_deadline, data.tx_deadline);
}

#[test]
fn gat_order_is_valid_true_for_good_data() {
    let order = GatOrder::new(make_gat_data());
    assert!(order.is_valid());
}

#[test]
fn gat_order_invalid_when_same_tokens() {
    let mut data = make_gat_data();
    data.order.buy_token = data.order.sell_token;
    let order = GatOrder::new(data);
    assert!(!order.is_valid());
}

#[test]
fn gat_order_invalid_when_deadline_before_start() {
    let mut data = make_gat_data();
    data.tx_deadline = data.start_time - 1;
    let order = GatOrder::new(data);
    assert!(!order.is_valid());
}

#[test]
fn gat_order_valid_when_deadline_equals_start() {
    let mut data = make_gat_data();
    data.tx_deadline = data.start_time;
    let order = GatOrder::new(data);
    assert!(order.is_valid());
}

#[test]
fn gat_order_invalid_when_zero_sell_amount() {
    let mut data = make_gat_data();
    data.order.sell_amount = U256::ZERO;
    let order = GatOrder::new(data);
    assert!(!order.is_valid());
}

#[test]
fn gat_to_params_sets_handler_address() {
    let order = GatOrder::new(make_gat_data());
    let params = order.to_params().unwrap();
    assert_eq!(params.handler, GAT_HANDLER_ADDRESS);
    assert_eq!(params.static_input.len(), 448);
}

#[test]
fn gat_handler_address_equals_twap_handler() {
    // GAT reuses the TWAP handler address
    assert_eq!(GAT_HANDLER_ADDRESS, TWAP_HANDLER_ADDRESS);
}

// ══════════════════════════════════════════════════════════════════════════════
// ConditionalOrderFactory
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn factory_decodes_twap_from_320_byte_input() {
    let order = TwapOrder::new(make_twap_data());
    let params = order.to_params().unwrap();
    assert_eq!(params.static_input.len(), 320);

    let factory = ConditionalOrderFactory::new();
    let kind = factory.from_params(params).unwrap();
    assert!(kind.is_twap());
    assert!(!kind.is_unknown());
    assert!(!kind.is_stop_loss());
    assert!(!kind.is_good_after_time());
}

#[test]
fn factory_decodes_gat_from_448_byte_input() {
    let order = GatOrder::new(make_gat_data());
    let params = order.to_params().unwrap();
    assert_eq!(params.static_input.len(), 448);

    let factory = ConditionalOrderFactory::new();
    let kind = factory.from_params(params).unwrap();
    assert!(kind.is_good_after_time());
    assert!(!kind.is_twap());
    assert!(!kind.is_unknown());
}

#[test]
fn factory_decodes_stop_loss() {
    let order = StopLossOrder::new(make_stop_loss_data());
    let params = order.to_params().unwrap();

    let factory = ConditionalOrderFactory::new();
    let kind = factory.from_params(params).unwrap();
    assert!(kind.is_stop_loss());
    assert!(!kind.is_twap());
    assert!(!kind.is_unknown());
}

#[test]
fn factory_returns_unknown_for_unrecognised_handler() {
    let params = make_params(Address::ZERO, vec![0u8; 128]);
    let factory = ConditionalOrderFactory::new();
    let kind = factory.from_params(params).unwrap();
    assert!(kind.is_unknown());
    assert!(!kind.is_twap());
    assert!(!kind.is_stop_loss());
    assert!(!kind.is_good_after_time());
}

#[test]
fn factory_as_str_labels() {
    let twap_kind = ConditionalOrderKind::Twap(TwapOrder::new(make_twap_data()));
    assert_eq!(twap_kind.as_str(), "twap");

    let sl_kind = ConditionalOrderKind::StopLoss(StopLossOrder::new(make_stop_loss_data()));
    assert_eq!(sl_kind.as_str(), "stop-loss");

    let gat_kind = ConditionalOrderKind::GoodAfterTime(GatOrder::new(make_gat_data()));
    assert_eq!(gat_kind.as_str(), "good-after-time");

    let unknown_kind = ConditionalOrderKind::Unknown(make_params(Address::ZERO, vec![]));
    assert_eq!(unknown_kind.as_str(), "unknown");
}

#[test]
fn factory_stop_loss_decode_roundtrip_preserves_data() {
    let original = make_stop_loss_data();
    let order = StopLossOrder::new(original.clone());
    let params = order.to_params().unwrap();

    let factory = ConditionalOrderFactory::new();
    let kind = factory.from_params(params).unwrap();

    if let ConditionalOrderKind::StopLoss(decoded) = kind {
        assert_eq!(decoded.data.sell_token, original.sell_token);
        assert_eq!(decoded.data.buy_token, original.buy_token);
        assert_eq!(decoded.data.strike_price, original.strike_price);
        assert_eq!(decoded.data.valid_to, original.valid_to);
    } else {
        panic!("expected StopLoss variant");
    }
}

#[test]
fn factory_gat_decode_roundtrip_preserves_data() {
    let original = make_gat_data();
    let order = GatOrder::new(original.clone());
    let params = order.to_params().unwrap();

    let factory = ConditionalOrderFactory::new();
    let kind = factory.from_params(params).unwrap();

    if let ConditionalOrderKind::GoodAfterTime(decoded) = kind {
        assert_eq!(decoded.data.start_time, original.start_time);
        assert_eq!(decoded.data.tx_deadline, original.tx_deadline);
        assert_eq!(decoded.data.order.sell_token, original.order.sell_token);
    } else {
        panic!("expected GoodAfterTime variant");
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Multiplexer
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn multiplexer_starts_empty() {
    let mux = Multiplexer::new(ProofLocation::Emitted);
    assert!(mux.is_empty());
    assert_eq!(mux.len(), 0);
}

#[test]
fn multiplexer_root_none_when_empty() {
    let mux = Multiplexer::new(ProofLocation::Emitted);
    assert!(mux.root().unwrap().is_none());
}

#[test]
fn multiplexer_add_one_order_root_is_some() {
    let mut mux = Multiplexer::new(ProofLocation::Emitted);
    let params = TwapOrder::new(make_twap_data()).to_params().unwrap();
    mux.add(params);
    assert_eq!(mux.len(), 1);
    assert!(!mux.is_empty());
    let root = mux.root().unwrap();
    assert!(root.is_some());
}

#[test]
fn multiplexer_two_orders_have_different_root_than_one() {
    let mut mux1 = Multiplexer::new(ProofLocation::Emitted);
    let params1 = TwapOrder::new(make_twap_data()).to_params().unwrap();
    mux1.add(params1.clone());
    let root1 = mux1.root().unwrap().unwrap();

    let mut mux2 = Multiplexer::new(ProofLocation::Emitted);
    let params2 = StopLossOrder::new(make_stop_loss_data()).to_params().unwrap();
    mux2.add(params1);
    mux2.add(params2);
    let root2 = mux2.root().unwrap().unwrap();

    assert_ne!(root1, root2);
}

#[test]
fn multiplexer_remove_by_id_reduces_length() {
    let mut mux = Multiplexer::new(ProofLocation::Emitted);
    let params = TwapOrder::new(make_twap_data()).to_params().unwrap();
    let id = cow_rs::composable::order_id(&params);
    mux.add(params);
    assert_eq!(mux.len(), 1);
    mux.remove(id);
    assert_eq!(mux.len(), 0);
}

#[test]
fn multiplexer_get_by_index_returns_params() {
    let mut mux = Multiplexer::new(ProofLocation::Emitted);
    let params = TwapOrder::new(make_twap_data()).to_params().unwrap();
    let handler = params.handler;
    mux.add(params);
    let got = mux.get_by_index(0).unwrap();
    assert_eq!(got.handler, handler);
}

#[test]
fn multiplexer_get_by_index_out_of_range_returns_none() {
    let mux = Multiplexer::new(ProofLocation::Emitted);
    assert!(mux.get_by_index(99).is_none());
}

#[test]
fn multiplexer_proof_for_single_order_has_zero_siblings() {
    let mut mux = Multiplexer::new(ProofLocation::Emitted);
    let params = TwapOrder::new(make_twap_data()).to_params().unwrap();
    mux.add(params);
    let proof = mux.proof(0).unwrap();
    assert_eq!(proof.proof_len(), 0);
}

#[test]
fn multiplexer_proof_for_two_orders_has_one_sibling_each() {
    let mut mux = Multiplexer::new(ProofLocation::Emitted);
    mux.add(TwapOrder::new(make_twap_data()).to_params().unwrap());
    mux.add(StopLossOrder::new(make_stop_loss_data()).to_params().unwrap());
    let proof0 = mux.proof(0).unwrap();
    let proof1 = mux.proof(1).unwrap();
    assert_eq!(proof0.proof_len(), 1);
    assert_eq!(proof1.proof_len(), 1);
}

#[test]
fn multiplexer_proof_out_of_range_returns_error() {
    let mux = Multiplexer::new(ProofLocation::Emitted);
    assert!(mux.proof(0).is_err());
}

#[test]
fn multiplexer_update_replaces_params() {
    let mut mux = Multiplexer::new(ProofLocation::Emitted);
    let params1 = TwapOrder::new(make_twap_data()).to_params().unwrap();
    let params2 = StopLossOrder::new(make_stop_loss_data()).to_params().unwrap();
    let handler2 = params2.handler;
    mux.add(params1);
    mux.update(0, params2).unwrap();
    assert_eq!(mux.get_by_index(0).unwrap().handler, handler2);
}

#[test]
fn multiplexer_update_out_of_range_returns_error() {
    let mut mux = Multiplexer::new(ProofLocation::Emitted);
    let params = TwapOrder::new(make_twap_data()).to_params().unwrap();
    assert!(mux.update(5, params).is_err());
}

#[test]
fn multiplexer_same_orders_same_root() {
    let make_mux = || {
        let mut mux = Multiplexer::new(ProofLocation::Emitted);
        mux.add(TwapOrder::new(make_twap_data()).to_params().unwrap());
        mux.add(StopLossOrder::new(make_stop_loss_data()).to_params().unwrap());
        mux
    };
    let root_a = make_mux().root().unwrap().unwrap();
    let root_b = make_mux().root().unwrap().unwrap();
    assert_eq!(root_a, root_b);
}
