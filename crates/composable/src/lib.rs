//! `cow-sdk-composable` — Layer 5 `CoW` Protocol composable (conditional) orders for the `CoW`
//! Protocol SDK.
//!
//! Composable orders are validated on-chain by handler contracts and managed
//! through the `ComposableCow` factory. This module provides:
//!
//! - [`TwapOrder`] — Time-Weighted Average Price orders with ABI encoding
//! - [`Multiplexer`] — manages multiple orders under a single Merkle root
//! - [`ConditionalOrderFactory`] — decode on-chain params into typed orders
//! - [`create_calldata`] / [`remove_calldata`] — `ComposableCow` calldata builders
//! - [`create_with_context_calldata`] — `createWithContext` for `AtMiningTime` TWAP orders
//!
//! # Example — single `TWAP` order
//!
//! ```rust,no_run
//! use alloy_primitives::{Address, B256, U256};
//! use cow_rs::{
//!     composable::{DurationOfPart, TwapData, TwapOrder, TwapStartTime},
//!     types::OrderKind,
//! };
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let data = TwapData {
//!     sell_token: Address::ZERO,
//!     buy_token: Address::ZERO,
//!     receiver: Address::ZERO,
//!     sell_amount: U256::from(24_000_000u64), // divisible by num_parts
//!     buy_amount: U256::from(21_600_000u64),
//!     start_time: TwapStartTime::AtMiningTime,
//!     part_duration: 3_600,
//!     num_parts: 24,
//!     app_data: B256::ZERO,
//!     partially_fillable: false,
//!     kind: OrderKind::Sell,
//!     duration_of_part: DurationOfPart::Auto,
//! };
//!
//! let order = TwapOrder::new(data);
//! assert!(order.is_valid());
//! let params = order.to_params()?;
//! let cd = cow_rs::composable::create_calldata(&params, &[], &[]);
//! println!("calldata: 0x{}", alloy_primitives::hex::encode(&cd));
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod calldata;
pub mod factory;
pub mod gat;
pub mod multiplexer;
pub mod stop_loss;
pub mod twap;
pub mod types;
pub mod utils;

pub use calldata::{
    create_calldata, create_with_context_calldata, remove_calldata, set_root_calldata,
    set_root_with_context_calldata,
};
pub use factory::{ConditionalOrderFactory, ConditionalOrderKind};
pub use gat::{GAT_HANDLER_ADDRESS, GatData, GatOrder, decode_gat_static_input, encode_gat_struct};
pub use multiplexer::{Multiplexer, OrderProof, ProofWithParams};
pub use stop_loss::{
    STOP_LOSS_HANDLER_ADDRESS, StopLossData, StopLossOrder, decode_stop_loss_static_input,
    encode_stop_loss_struct,
};
pub use twap::{
    TwapOrder, data_to_struct, decode_params, decode_twap_static_input, decode_twap_struct,
    encode_params, encode_twap_struct, format_epoch, order_id, struct_to_data,
};
pub use types::{
    BlockInfo, COMPOSABLE_COW_ADDRESS, CURRENT_BLOCK_TIMESTAMP_FACTORY_ADDRESS,
    ConditionalOrderParams, DEFAULT_TEST_HANDLER, DEFAULT_TEST_SALT, DurationOfPart,
    GpV2OrderStruct, IsNotValid, IsValid, IsValidResult, MAX_FREQUENCY, PollResult, ProofLocation,
    ProofStruct, TWAP_HANDLER_ADDRESS, TestConditionalOrderParams, TwapData, TwapStartTime,
    TwapStruct, create_test_conditional_order,
};
pub use utils::{
    balance_to_string, create_set_domain_verifier_tx, default_token_formatter,
    from_struct_to_order, get_block_info, get_domain_verifier, get_domain_verifier_calldata,
    get_is_valid_result, is_composable_cow, is_extensible_fallback_handler, is_valid_abi,
    kind_to_string, transform_data_to_struct, transform_struct_to_data,
};
