//! `GoodAfterTime` (`GAT`) conditional order handler.
//!
//! A GAT order is not valid before `start_time` and must be executed before
//! `tx_deadline`.  It wraps a standard `GPv2Order` and adds time-gating.
//!
//! The GAT handler uses the same contract address as the `TWAP` handler:
//! `0x6cF1e9cA41f7611dEf408122793c358a3d11E5a5`.

use alloy_primitives::{Address, B256, U256, keccak256};

use cow_sdk_error::CowError;

use super::types::{ConditionalOrderParams, GpV2OrderStruct, TWAP_HANDLER_ADDRESS};

// ── Handler address ───────────────────────────────────────────────────────────

/// `GoodAfterTime` handler contract address.
///
/// This is the same address as the `TWAP` handler —
/// `0x6cF1e9cA41f7611dEf408122793c358a3d11E5a5`.
pub const GAT_HANDLER_ADDRESS: Address = TWAP_HANDLER_ADDRESS;

// ── GatData ───────────────────────────────────────────────────────────────────

/// Parameters for a `GoodAfterTime` (`GAT`) conditional order.
///
/// A `GAT` order wraps a regular `GPv2Order` and adds two time constraints:
/// - The order is **not** valid before `start_time`.
/// - The settling transaction must be submitted before `tx_deadline` to be accepted by the handler.
#[derive(Debug, Clone)]
pub struct GatData {
    /// The underlying `GPv2Order` that will be submitted when the time window
    /// is active.
    pub order: GpV2OrderStruct,
    /// Unix timestamp before which the order is not valid.
    pub start_time: u32,
    /// Absolute Unix timestamp by which the settlement transaction must be
    /// included (`block.timestamp ≤ tx_deadline`).
    pub tx_deadline: u32,
}

// ── GatOrder ──────────────────────────────────────────────────────────────────

/// A `GoodAfterTime` conditional order ready for submission to `ComposableCow`.
#[derive(Debug, Clone)]
pub struct GatOrder {
    /// `GAT` configuration.
    pub data: GatData,
    /// 32-byte salt uniquely identifying this order instance.
    pub salt: B256,
}

impl GatOrder {
    /// Create a new `GAT` order with a deterministic salt derived from the
    /// order parameters.
    ///
    /// # Returns
    ///
    /// A [`GatOrder`] whose salt is the `keccak256` hash of the key order
    /// fields (`sell_token`, `buy_token`, `sell_amount`, `start_time`,
    /// `tx_deadline`).
    #[must_use]
    pub fn new(data: GatData) -> Self {
        let salt = deterministic_salt(&data);
        Self { data, salt }
    }

    /// Create a `GAT` order with an explicit salt.
    ///
    /// # Arguments
    ///
    /// * `data` - The [`GatData`] containing the underlying `GPv2Order` and time-gating parameters.
    /// * `salt` - A caller-chosen 32-byte salt that uniquely identifies this order instance
    ///   on-chain.
    ///
    /// # Returns
    ///
    /// A [`GatOrder`] using the provided `salt` verbatim.
    #[must_use]
    pub const fn with_salt(data: GatData, salt: B256) -> Self {
        Self { data, salt }
    }

    /// Returns `true` if the order parameters are logically valid:
    ///
    /// - `sell_amount > 0`
    /// - `buy_amount > 0`
    /// - `sell_token != buy_token`
    /// - `start_time <= tx_deadline`
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let d = &self.data;
        let o = &d.order;
        !o.sell_amount.is_zero() &&
            !o.buy_amount.is_zero() &&
            o.sell_token != o.buy_token &&
            d.start_time <= d.tx_deadline
    }

    /// Build the on-chain [`ConditionalOrderParams`] for this order.
    ///
    /// # Returns
    ///
    /// A [`ConditionalOrderParams`] with `handler` set to
    /// [`GAT_HANDLER_ADDRESS`], the stored `salt`, and the ABI-encoded
    /// `static_input` bytes.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] if ABI encoding fails.
    pub fn to_params(&self) -> Result<ConditionalOrderParams, CowError> {
        Ok(ConditionalOrderParams {
            handler: GAT_HANDLER_ADDRESS,
            salt: self.salt,
            static_input: encode_gat_struct(&self.data),
        })
    }

    /// Returns a reference to the `GAT` data.
    ///
    /// # Returns
    ///
    /// A shared reference to the inner [`GatData`], giving access to the
    /// underlying `GPv2Order`, `start_time`, and `tx_deadline`.
    #[must_use]
    pub const fn data_ref(&self) -> &GatData {
        &self.data
    }

    /// Returns a reference to the 32-byte salt.
    ///
    /// # Returns
    ///
    /// A shared reference to the [`B256`] salt that uniquely identifies this
    /// order instance on-chain.
    #[must_use]
    pub const fn salt_ref(&self) -> &B256 {
        &self.salt
    }
}

// ── ABI encoding ──────────────────────────────────────────────────────────────

/// ABI-encode a [`GatData`] struct into the `staticInput` bytes expected by
/// the on-chain `GoodAfterTime` handler.
///
/// Encoding layout (each word is 32 bytes, big-endian):
/// - Words 0-11: `GpV2OrderStruct` fields (`sell_token`, `buy_token`, `receiver`, `sell_amount`,
///   `buy_amount`, `valid_to`, `app_data`, `fee_amount`, `kind`, `partially_fillable`,
///   `sell_token_balance`, `buy_token_balance`)
/// - Word 12: `start_time` (uint32)
/// - Word 13: `tx_deadline` (uint32)
///
/// Total: 14 × 32 = 448 bytes.
///
/// ```
/// use alloy_primitives::{Address, B256, U256};
/// use cow_rs::composable::{GatData, GpV2OrderStruct, encode_gat_struct};
///
/// let order = GpV2OrderStruct {
///     sell_token: Address::ZERO,
///     buy_token: Address::ZERO,
///     receiver: Address::ZERO,
///     sell_amount: U256::from(1_000u64),
///     buy_amount: U256::from(900u64),
///     valid_to: 9_999_999,
///     app_data: B256::ZERO,
///     fee_amount: U256::ZERO,
///     kind: B256::ZERO,
///     partially_fillable: false,
///     sell_token_balance: B256::ZERO,
///     buy_token_balance: B256::ZERO,
/// };
/// let data = GatData { order, start_time: 1_000_000, tx_deadline: 2_000_000 };
/// let encoded = encode_gat_struct(&data);
/// assert_eq!(encoded.len(), 448);
/// ```
#[must_use]
pub fn encode_gat_struct(d: &GatData) -> Vec<u8> {
    let o = &d.order;
    let mut buf = Vec::with_capacity(14 * 32);
    // GPv2Order fields
    buf.extend_from_slice(&pad_address(o.sell_token.as_slice()));
    buf.extend_from_slice(&pad_address(o.buy_token.as_slice()));
    buf.extend_from_slice(&pad_address(o.receiver.as_slice()));
    buf.extend_from_slice(&u256_bytes(o.sell_amount));
    buf.extend_from_slice(&u256_bytes(o.buy_amount));
    buf.extend_from_slice(&u256_be(u64::from(o.valid_to)));
    buf.extend_from_slice(o.app_data.as_slice());
    buf.extend_from_slice(&u256_bytes(o.fee_amount));
    buf.extend_from_slice(o.kind.as_slice());
    buf.extend_from_slice(&bool_word(o.partially_fillable));
    buf.extend_from_slice(o.sell_token_balance.as_slice());
    buf.extend_from_slice(o.buy_token_balance.as_slice());
    // GAT-specific fields
    buf.extend_from_slice(&u256_be(u64::from(d.start_time)));
    buf.extend_from_slice(&u256_be(u64::from(d.tx_deadline)));
    buf
}

/// ABI-decode a 448-byte `staticInput` buffer into a [`GatData`].
///
/// # Errors
///
/// Returns [`CowError::AppData`] if `bytes` is shorter than 448 bytes.
///
/// ```
/// use alloy_primitives::{Address, B256, U256};
/// use cow_rs::composable::{
///     GatData, GpV2OrderStruct, decode_gat_static_input, encode_gat_struct,
/// };
///
/// let order = GpV2OrderStruct {
///     sell_token: Address::repeat_byte(0x01),
///     buy_token: Address::repeat_byte(0x02),
///     receiver: Address::ZERO,
///     sell_amount: U256::from(500u64),
///     buy_amount: U256::from(400u64),
///     valid_to: 1_234_567,
///     app_data: B256::ZERO,
///     fee_amount: U256::ZERO,
///     kind: B256::ZERO,
///     partially_fillable: false,
///     sell_token_balance: B256::ZERO,
///     buy_token_balance: B256::ZERO,
/// };
/// let data = GatData { order, start_time: 1_000_000, tx_deadline: 2_000_000 };
/// let encoded = encode_gat_struct(&data);
/// let decoded = decode_gat_static_input(&encoded).unwrap();
/// assert_eq!(decoded.start_time, data.start_time);
/// assert_eq!(decoded.tx_deadline, data.tx_deadline);
/// assert_eq!(decoded.order.sell_amount, data.order.sell_amount);
/// ```
pub fn decode_gat_static_input(bytes: &[u8]) -> Result<GatData, CowError> {
    if bytes.len() < 14 * 32 {
        return Err(CowError::AppData(format!(
            "GAT static input too short: {} bytes (need 448)",
            bytes.len()
        )));
    }
    let addr = |off: usize| -> Address {
        let mut a = [0u8; 20];
        a.copy_from_slice(&bytes[off + 12..off + 32]);
        Address::new(a)
    };
    let u256 = |off: usize| -> U256 { U256::from_be_slice(&bytes[off..off + 32]) };
    let u32v = |off: usize| -> u32 {
        u32::from_be_bytes([bytes[off + 28], bytes[off + 29], bytes[off + 30], bytes[off + 31]])
    };
    let bool_v = |off: usize| -> bool { bytes[off + 31] != 0 };
    let b256 = |off: usize| -> B256 {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes[off..off + 32]);
        B256::new(arr)
    };

    let order = GpV2OrderStruct {
        sell_token: addr(0),
        buy_token: addr(32),
        receiver: addr(64),
        sell_amount: u256(96),
        buy_amount: u256(128),
        valid_to: u32v(160),
        app_data: b256(192),
        fee_amount: u256(224),
        kind: b256(256),
        partially_fillable: bool_v(288),
        sell_token_balance: b256(320),
        buy_token_balance: b256(352),
    };

    Ok(GatData { order, start_time: u32v(384), tx_deadline: u32v(416) })
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Left-pad an address (or shorter slice) to 32 bytes.
///
/// # Arguments
///
/// * `bytes` - A 20-byte (or shorter) slice to right-align within a 32-byte ABI word.
///
/// # Returns
///
/// A 32-byte array with the input placed in the last `bytes.len()` positions
/// and the leading bytes zeroed.
fn pad_address(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(bytes);
    out
}

/// Convert a `U256` to its 32-byte big-endian representation.
///
/// # Arguments
///
/// * `v` - The [`U256`] value to convert.
///
/// # Returns
///
/// A 32-byte array containing the big-endian encoding of `v`.
const fn u256_bytes(v: U256) -> [u8; 32] {
    v.to_be_bytes()
}

/// Encode a `u64` as a 32-byte big-endian ABI word.
///
/// # Arguments
///
/// * `v` - The `u64` value to encode.
///
/// # Returns
///
/// A 32-byte array with the big-endian `u64` right-aligned (bytes 24..32)
/// and the leading 24 bytes zeroed.
fn u256_be(v: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&v.to_be_bytes());
    out
}

/// Encode a `bool` as a 32-byte ABI word (0 or 1).
///
/// # Arguments
///
/// * `v` - The boolean value to encode.
///
/// # Returns
///
/// A 32-byte array where byte 31 is `1` when `v` is `true` and `0`
/// otherwise; all other bytes are zero.
const fn bool_word(v: bool) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[31] = if v { 1 } else { 0 };
    out
}

/// Derive a deterministic salt by hashing all GAT parameters.
///
/// # Arguments
///
/// * `d` - The [`GatData`] whose key fields (`sell_token`, `buy_token`, `sell_amount`,
///   `start_time`, `tx_deadline`) are concatenated and hashed.
///
/// # Returns
///
/// A [`B256`] salt computed as `keccak256(sell_token || buy_token ||
/// sell_amount || start_time || tx_deadline)`.
fn deterministic_salt(d: &GatData) -> B256 {
    let o = &d.order;
    let mut buf = Vec::with_capacity(20 + 20 + 32 + 4 + 4);
    buf.extend_from_slice(o.sell_token.as_slice());
    buf.extend_from_slice(o.buy_token.as_slice());
    buf.extend_from_slice(&u256_bytes(o.sell_amount));
    buf.extend_from_slice(&d.start_time.to_be_bytes());
    buf.extend_from_slice(&d.tx_deadline.to_be_bytes());
    keccak256(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_order() -> GpV2OrderStruct {
        GpV2OrderStruct {
            sell_token: Address::repeat_byte(0x01),
            buy_token: Address::repeat_byte(0x02),
            receiver: Address::ZERO,
            sell_amount: U256::from(1_000u64),
            buy_amount: U256::from(900u64),
            valid_to: 9_999_999,
            app_data: B256::ZERO,
            fee_amount: U256::ZERO,
            kind: B256::ZERO,
            partially_fillable: false,
            sell_token_balance: B256::ZERO,
            buy_token_balance: B256::ZERO,
        }
    }

    fn make_data() -> GatData {
        GatData { order: make_order(), start_time: 1_000_000, tx_deadline: 2_000_000 }
    }

    #[test]
    fn encode_is_448_bytes() {
        let data = make_data();
        let encoded = encode_gat_struct(&data);
        assert_eq!(encoded.len(), 448);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let data = make_data();
        let encoded = encode_gat_struct(&data);
        let decoded = decode_gat_static_input(&encoded).unwrap();
        assert_eq!(decoded.start_time, data.start_time);
        assert_eq!(decoded.tx_deadline, data.tx_deadline);
        assert_eq!(decoded.order.sell_token, data.order.sell_token);
        assert_eq!(decoded.order.buy_token, data.order.buy_token);
        assert_eq!(decoded.order.sell_amount, data.order.sell_amount);
        assert_eq!(decoded.order.buy_amount, data.order.buy_amount);
        assert_eq!(decoded.order.valid_to, data.order.valid_to);
        assert_eq!(decoded.order.partially_fillable, data.order.partially_fillable);
    }

    #[test]
    fn is_valid_returns_true_for_valid_order() {
        let order = GatOrder::new(make_data());
        assert!(order.is_valid());
    }

    #[test]
    fn is_valid_returns_false_for_same_tokens() {
        let mut data = make_data();
        data.order.buy_token = data.order.sell_token;
        let order = GatOrder::new(data);
        assert!(!order.is_valid());
    }

    #[test]
    fn is_valid_returns_false_when_deadline_before_start() {
        let mut data = make_data();
        data.tx_deadline = data.start_time - 1;
        let order = GatOrder::new(data);
        assert!(!order.is_valid());
    }

    #[test]
    fn to_params_sets_correct_handler() {
        let order = GatOrder::new(make_data());
        let params = order.to_params().unwrap();
        assert_eq!(params.handler, GAT_HANDLER_ADDRESS);
    }

    #[test]
    fn decode_too_short_returns_error() {
        let result = decode_gat_static_input(&[0u8; 100]);
        assert!(result.is_err());
    }
}
