//! `cow-ethflow` — Layer 2 `EthFlow` contract helpers for the `CoW` Protocol SDK.
//!
//! Encode `createOrder` calldata for native-currency orders.
//!
//! When a user wants to sell ETH (or another chain's native currency)
//! instead of an ERC-20, the order is submitted through the `EthFlow`
//! contract rather than the standard `GPv2Settlement` flow. This module
//! provides the types and encoding functions for that path.
//!
//! # Key items
//!
//! | Item | Purpose |
//! |---|---|
//! | [`EthFlowOrderData`] | Parameters for a native-currency sell order |
//! | [`EthFlowTransaction`] | Ready-to-send transaction (to, data, value) |
//! | [`encode_eth_flow_create_order`] | ABI-encode `createOrder(...)` calldata |
//! | [`build_eth_flow_transaction`] | Build the complete transaction |
//! | [`is_eth_flow_order_data`] | Check if on-chain data indicates an EthFlow order |

#![deny(unsafe_code)]
#![warn(missing_docs)]

use alloy_primitives::{Address, B256, U256, keccak256};
use cow_types::OnchainOrderData;

/// Parameters for a native-currency sell order submitted through the
/// `EthFlow` contract.
///
/// Maps to the `EthFlowOrder` struct in the Solidity contract. The
/// sell token is implicitly the chain's native currency (ETH, xDAI, …).
///
/// # Example
///
/// ```
/// use alloy_primitives::{Address, B256, U256};
/// use cow_ethflow::{EthFlowOrderData, encode_eth_flow_create_order};
///
/// let order = EthFlowOrderData {
///     buy_token: Address::ZERO,
///     receiver: Address::ZERO,
///     sell_amount: U256::from(1_000_000u64),
///     buy_amount: U256::from(500_000u64),
///     app_data: B256::ZERO,
///     fee_amount: U256::ZERO,
///     valid_to: 9_999_999,
///     partially_fillable: false,
///     quote_id: 42,
/// };
/// let cd = encode_eth_flow_create_order(&order);
/// assert_eq!(cd.len(), 292);
/// ```
#[derive(Debug, Clone)]
pub struct EthFlowOrderData {
    /// Token to buy (native currency is the sell token).
    pub buy_token: Address,
    /// Address that receives the bought tokens.
    pub receiver: Address,
    /// Amount of native currency to sell (in wei).
    pub sell_amount: U256,
    /// Minimum amount of `buy_token` to receive.
    pub buy_amount: U256,
    /// `bytes32` app-data hash.
    pub app_data: B256,
    /// Protocol fee (in wei), typically zero for `EthFlow`.
    pub fee_amount: U256,
    /// Order expiry as Unix timestamp.
    pub valid_to: u32,
    /// Whether the order may be partially filled.
    pub partially_fillable: bool,
    /// Quote identifier from the orderbook.
    pub quote_id: i64,
}

/// Ready-to-send transaction for submitting a native-currency order.
///
/// Produced by [`build_eth_flow_transaction`]. Send this transaction with
/// the indicated [`value`](Self::value) of native currency attached.
#[derive(Debug, Clone)]
pub struct EthFlowTransaction {
    /// `EthFlow` contract address to call.
    pub to: Address,
    /// ABI-encoded `createOrder(EthFlowOrder)` calldata.
    pub data: Vec<u8>,
    /// ETH value to attach (= `sell_amount`).
    pub value: U256,
}

/// Compute the 4-byte selector from a Solidity function signature.
fn selector(sig: &[u8]) -> [u8; 4] {
    let h = keccak256(sig);
    [h[0], h[1], h[2], h[3]]
}

/// Left-pad an [`Address`] to a 32-byte ABI word.
fn abi_address(a: Address) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[12..].copy_from_slice(a.as_slice());
    buf
}

/// Encode a `u32` as a 32-byte big-endian ABI word.
fn abi_u32(v: u32) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[28..].copy_from_slice(&v.to_be_bytes());
    buf
}

/// Encode a `bool` as a 32-byte ABI word.
fn abi_bool(v: bool) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[31] = u8::from(v);
    buf
}

/// Encode an `i64` as a 32-byte two's-complement big-endian ABI word.
fn abi_i64(v: i64) -> [u8; 32] {
    // Sign-extend i64 to 32 bytes (two's complement big-endian).
    let fill: u8 = if v < 0 { 0xff } else { 0x00 };
    let mut buf = [fill; 32];
    buf[24..].copy_from_slice(&v.to_be_bytes());
    buf
}

/// Encode the `EthFlow.createOrder(order, quoteId)` calldata.
///
/// Function signature:
/// `createOrder((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64))`
///
/// Total payload: selector (4) + 9 × 32-byte words = 292 bytes.
///
/// # Parameters
///
/// * `order` — the [`EthFlowOrderData`] to encode.
///
/// # Returns
///
/// A 292-byte `Vec<u8>` containing the ABI-encoded calldata.
///
/// # Example
///
/// ```
/// use alloy_primitives::{Address, B256, U256};
/// use cow_ethflow::{EthFlowOrderData, encode_eth_flow_create_order};
///
/// let order = EthFlowOrderData {
///     buy_token: Address::ZERO,
///     receiver: Address::ZERO,
///     sell_amount: U256::from(1_000_000_u64),
///     buy_amount: U256::from(500_000_u64),
///     app_data: B256::ZERO,
///     fee_amount: U256::ZERO,
///     valid_to: 9_999_999_u32,
///     partially_fillable: false,
///     quote_id: 42,
/// };
/// let cd = encode_eth_flow_create_order(&order);
/// assert_eq!(cd.len(), 292);
/// ```
#[must_use]
pub fn encode_eth_flow_create_order(order: &EthFlowOrderData) -> Vec<u8> {
    // The EthFlow contract declares `createOrder` with a single `EthFlowOrder`
    // struct argument; `quoteId` is the last field of the struct, not a
    // separate function parameter. The byte layout of the trailing 9 × 32
    // static words is identical either way — only the selector differs.
    let sig = b"createOrder((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64))";
    let sel = selector(sig);

    let mut buf = Vec::with_capacity(292);
    buf.extend_from_slice(&sel);
    buf.extend_from_slice(&abi_address(order.buy_token));
    buf.extend_from_slice(&abi_address(order.receiver));
    buf.extend_from_slice(&order.sell_amount.to_be_bytes::<32>());
    buf.extend_from_slice(&order.buy_amount.to_be_bytes::<32>());
    buf.extend_from_slice(order.app_data.as_slice());
    buf.extend_from_slice(&order.fee_amount.to_be_bytes::<32>());
    buf.extend_from_slice(&abi_u32(order.valid_to));
    buf.extend_from_slice(&abi_bool(order.partially_fillable));
    buf.extend_from_slice(&abi_i64(order.quote_id));
    buf
}

/// Build a complete [`EthFlowTransaction`] for `contract_address`.
///
/// The caller is responsible for sending the returned transaction with the
/// ETH value indicated by [`EthFlowTransaction::value`].
///
/// # Parameters
///
/// * `contract` — the [`Address`] of the `EthFlow` contract on the target chain (use
///   [`eth_flow_for_env`](cow_chains::eth_flow_for_env) to look it up).
/// * `order` — the [`EthFlowOrderData`] to encode.
///
/// # Returns
///
/// An [`EthFlowTransaction`] with `to`, `data`, and `value` fields set.
///
/// # Example
///
/// ```
/// use alloy_primitives::{Address, B256, U256};
/// use cow_ethflow::{EthFlowOrderData, build_eth_flow_transaction};
///
/// let order = EthFlowOrderData {
///     buy_token: Address::ZERO,
///     receiver: Address::ZERO,
///     sell_amount: U256::from(1_000_u64),
///     buy_amount: U256::from(500_u64),
///     app_data: B256::ZERO,
///     fee_amount: U256::ZERO,
///     valid_to: 0,
///     partially_fillable: false,
///     quote_id: 1,
/// };
/// let tx = build_eth_flow_transaction(Address::ZERO, &order);
/// assert_eq!(tx.value, order.sell_amount);
/// ```
#[must_use]
pub fn build_eth_flow_transaction(
    contract: Address,
    order: &EthFlowOrderData,
) -> EthFlowTransaction {
    EthFlowTransaction {
        to: contract,
        data: encode_eth_flow_create_order(order),
        value: order.sell_amount,
    }
}

/// Check whether on-chain data is present, indicating an `EthFlow` order.
///
/// Returns `true` if `onchain_data` is `Some`, which signals that the order
/// was submitted via the `EthFlow` contract rather than the standard
/// `GPv2Settlement` flow.
///
/// # Parameters
///
/// * `onchain_data` — optional reference to [`OnchainOrderData`].
///
/// # Returns
///
/// `true` if `onchain_data` is `Some`.
///
/// # Example
///
/// ```
/// use cow_ethflow::is_eth_flow_order_data;
///
/// assert!(!is_eth_flow_order_data(None));
/// ```
#[must_use]
pub const fn is_eth_flow_order_data(onchain_data: Option<&OnchainOrderData>) -> bool {
    onchain_data.is_some()
}
