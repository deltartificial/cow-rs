//! Order refund helpers for reclaiming unfilled portions of `CoW` Protocol orders.
//!
//! Provides [`OrderRefund`] for representing refund claims, ABI-encoded calldata
//! builders for settlement and `EthFlow` refunds, and helpers for computing
//! refundable amounts.

use std::fmt;

use alloy_primitives::{U256, keccak256};
use cow_errors::CowError;

/// Refund claim for a `CoW` Protocol order.
///
/// Represents an order whose unfilled portion can be reclaimed by the owner.
/// The [`refund_type`](Self::refund_type) determines which contract to target.
///
/// # Example
///
/// ```
/// use cow_rs::settlement::refunds::{OrderRefund, RefundType};
///
/// let refund = OrderRefund::new("0xabcd".to_owned(), RefundType::Settlement);
/// assert_eq!(refund.refund_type, RefundType::Settlement);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderRefund {
    /// The hex-encoded order UID (with or without `0x` prefix).
    pub order_uid: String,
    /// Which contract path to use for the refund.
    pub refund_type: RefundType,
}

impl OrderRefund {
    /// Create a new order refund claim.
    ///
    /// # Arguments
    ///
    /// * `order_uid` - The hex-encoded order UID.
    /// * `refund_type` - The refund path ([`RefundType::Settlement`] or [`RefundType::EthFlow`]).
    ///
    /// # Returns
    ///
    /// A new [`OrderRefund`].
    #[must_use]
    pub const fn new(order_uid: String, refund_type: RefundType) -> Self {
        Self { order_uid, refund_type }
    }
}

impl fmt::Display for OrderRefund {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Refund({}, {:?})", self.order_uid, self.refund_type)
    }
}

/// The contract path to use for an order refund.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefundType {
    /// Standard order refund via the settlement contract.
    Settlement,
    /// `EthFlow` order refund via the `EthFlow` contract.
    EthFlow,
}

impl RefundType {
    /// Check whether this is a settlement refund.
    ///
    /// # Returns
    ///
    /// `true` if this is [`RefundType::Settlement`].
    #[must_use]
    pub const fn is_settlement(&self) -> bool {
        matches!(self, Self::Settlement)
    }

    /// Check whether this is an `EthFlow` refund.
    ///
    /// # Returns
    ///
    /// `true` if this is [`RefundType::EthFlow`].
    #[must_use]
    pub const fn is_eth_flow(&self) -> bool {
        matches!(self, Self::EthFlow)
    }
}

impl fmt::Display for RefundType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Settlement => write!(f, "Settlement"),
            Self::EthFlow => write!(f, "EthFlow"),
        }
    }
}

/// Build calldata for `GPv2Settlement.freeFilledAmountStorage(bytes orderUid)`.
///
/// This reclaims the storage slot used by the settlement contract to track
/// the filled amount for a fully cancelled or expired order, triggering a
/// gas refund.
///
/// # Arguments
///
/// * `order_uid` - The hex-encoded order UID (with or without `0x` prefix).
///
/// # Returns
///
/// The ABI-encoded calldata as a `Vec<u8>`.
///
/// # Errors
///
/// Returns [`CowError::Api`] if `order_uid` is not valid hex.
///
/// # Example
///
/// ```
/// use cow_rs::settlement::refunds::settlement_refund_calldata;
///
/// let uid = "0x".to_owned() + &"ab".repeat(56);
/// let calldata = settlement_refund_calldata(&uid).unwrap();
/// assert!(!calldata.is_empty());
/// ```
pub fn settlement_refund_calldata(order_uid: &str) -> Result<Vec<u8>, CowError> {
    let uid_bytes = decode_uid(order_uid)?;
    let sel = selector("freeFilledAmountStorage(bytes)");

    let padded_len = padded32(uid_bytes.len());
    let mut buf = Vec::with_capacity(4 + 32 + 32 + padded_len);
    buf.extend_from_slice(&sel);
    // Offset to dynamic bytes data (1 head slot = 32).
    buf.extend_from_slice(&u256_be(32));
    // Length of bytes data.
    buf.extend_from_slice(&u256_be(uid_bytes.len() as u64));
    // Actual data.
    buf.extend_from_slice(&uid_bytes);
    // Pad to 32-byte boundary.
    pad_to(&mut buf, uid_bytes.len());
    Ok(buf)
}

/// Build calldata for `CoWSwapEthFlow.invalidateOrder(bytes orderUid)`.
///
/// Invalidates an `EthFlow` order on the `EthFlow` contract, allowing the
/// user to reclaim their deposited ETH.
///
/// # Arguments
///
/// * `order_uid` - The hex-encoded order UID (with or without `0x` prefix).
///
/// # Returns
///
/// The ABI-encoded calldata as a `Vec<u8>`.
///
/// # Errors
///
/// Returns [`CowError::Api`] if `order_uid` is not valid hex.
///
/// # Example
///
/// ```
/// use cow_rs::settlement::refunds::ethflow_refund_calldata;
///
/// let uid = "0x".to_owned() + &"ab".repeat(56);
/// let calldata = ethflow_refund_calldata(&uid).unwrap();
/// assert!(!calldata.is_empty());
/// ```
pub fn ethflow_refund_calldata(order_uid: &str) -> Result<Vec<u8>, CowError> {
    let uid_bytes = decode_uid(order_uid)?;
    let sel = selector("invalidateOrder(bytes)");

    let padded_len = padded32(uid_bytes.len());
    let mut buf = Vec::with_capacity(4 + 32 + 32 + padded_len);
    buf.extend_from_slice(&sel);
    // Offset to dynamic bytes data (1 head slot = 32).
    buf.extend_from_slice(&u256_be(32));
    // Length of bytes data.
    buf.extend_from_slice(&u256_be(uid_bytes.len() as u64));
    // Actual data.
    buf.extend_from_slice(&uid_bytes);
    // Pad to 32-byte boundary.
    pad_to(&mut buf, uid_bytes.len());
    Ok(buf)
}

/// Check whether an order has an unfilled portion that can be refunded.
///
/// An order is refundable if it has not been fully filled (i.e.,
/// `filled_amount < total_amount`).
///
/// # Arguments
///
/// * `filled_amount` - The amount already filled.
/// * `total_amount` - The total order amount.
///
/// # Returns
///
/// `true` if the order has an unfilled portion (`filled_amount < total_amount`).
///
/// # Example
///
/// ```
/// use alloy_primitives::U256;
/// use cow_rs::settlement::refunds::is_refundable;
///
/// assert!(is_refundable(U256::ZERO, U256::from(1000)));
/// assert!(is_refundable(U256::from(500), U256::from(1000)));
/// assert!(!is_refundable(U256::from(1000), U256::from(1000)));
/// ```
#[must_use]
pub fn is_refundable(filled_amount: U256, total_amount: U256) -> bool {
    filled_amount < total_amount
}

/// Compute the refundable (unfilled) amount for an order.
///
/// Returns `total_amount - filled_amount`, saturating at zero if the
/// filled amount exceeds the total (which should not happen in practice).
///
/// # Arguments
///
/// * `filled_amount` - The amount already filled.
/// * `total_amount` - The total order amount.
///
/// # Returns
///
/// The refundable amount as [`U256`].
///
/// # Example
///
/// ```
/// use alloy_primitives::U256;
/// use cow_rs::settlement::refunds::refund_amount;
///
/// assert_eq!(refund_amount(U256::from(300), U256::from(1000)), U256::from(700));
/// assert_eq!(refund_amount(U256::from(1000), U256::from(1000)), U256::ZERO);
/// ```
#[must_use]
pub const fn refund_amount(filled_amount: U256, total_amount: U256) -> U256 {
    total_amount.saturating_sub(filled_amount)
}

// ── Private helpers ──────────────────────────────────────────────────────────

/// Compute the 4-byte selector from a Solidity function signature.
fn selector(sig: &str) -> [u8; 4] {
    let h = keccak256(sig.as_bytes());
    [h[0], h[1], h[2], h[3]]
}

/// Encode a `u64` as a 32-byte big-endian ABI word.
fn u256_be(v: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&v.to_be_bytes());
    out
}

/// Zero-pad `buf` to the next 32-byte boundary after `written` bytes.
fn pad_to(buf: &mut Vec<u8>, written: usize) {
    let rem = written % 32;
    if rem != 0 {
        buf.resize(buf.len() + (32 - rem), 0);
    }
}

/// Round `n` up to the next multiple of 32.
const fn padded32(n: usize) -> usize {
    if n.is_multiple_of(32) { n } else { n + (32 - n % 32) }
}

/// Decode a hex order UID (with or without `0x` prefix) into raw bytes.
fn decode_uid(uid: &str) -> Result<Vec<u8>, CowError> {
    let stripped = uid.trim_start_matches("0x");
    alloy_primitives::hex::decode(stripped)
        .map_err(|_e| CowError::Api { status: 0, body: format!("invalid orderUid: {uid}") })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_uid_56() -> String {
        "0x".to_owned() + &"ab".repeat(56)
    }

    // ── OrderRefund tests ────────────────────────────────────────────────

    #[test]
    fn order_refund_new() {
        let refund = OrderRefund::new("0xdead".to_owned(), RefundType::Settlement);
        assert_eq!(refund.order_uid, "0xdead");
        assert_eq!(refund.refund_type, RefundType::Settlement);
    }

    #[test]
    fn order_refund_display() {
        let refund = OrderRefund::new("0xbeef".to_owned(), RefundType::EthFlow);
        let s = format!("{refund}");
        assert!(s.contains("0xbeef"));
        assert!(s.contains("EthFlow"));
    }

    #[test]
    fn order_refund_clone_eq() {
        let a = OrderRefund::new("0xaa".to_owned(), RefundType::Settlement);
        let b = a.clone();
        assert_eq!(a, b);
    }

    // ── RefundType tests ─────────────────────────────────────────────────

    #[test]
    fn refund_type_is_settlement() {
        assert!(RefundType::Settlement.is_settlement());
        assert!(!RefundType::Settlement.is_eth_flow());
    }

    #[test]
    fn refund_type_is_eth_flow() {
        assert!(RefundType::EthFlow.is_eth_flow());
        assert!(!RefundType::EthFlow.is_settlement());
    }

    #[test]
    fn refund_type_display() {
        assert_eq!(format!("{}", RefundType::Settlement), "Settlement");
        assert_eq!(format!("{}", RefundType::EthFlow), "EthFlow");
    }

    #[test]
    fn refund_type_copy() {
        let a = RefundType::Settlement;
        let b = a;
        assert_eq!(a, b);
    }

    // ── settlement_refund_calldata tests ─────────────────────────────────

    #[test]
    fn settlement_refund_calldata_valid() {
        let uid = dummy_uid_56();
        let data = settlement_refund_calldata(&uid).unwrap();
        // 4 (selector) + 32 (offset) + 32 (length) + 64 (56 bytes padded to 64) = 132
        assert_eq!(data.len(), 132);
        assert_eq!(&data[..4], &selector("freeFilledAmountStorage(bytes)"));
    }

    #[test]
    fn settlement_refund_calldata_invalid_hex() {
        assert!(settlement_refund_calldata("0xZZZZ").is_err());
    }

    #[test]
    fn settlement_refund_calldata_without_prefix() {
        let uid = "ab".repeat(56);
        let data = settlement_refund_calldata(&uid).unwrap();
        assert_eq!(data.len(), 132);
    }

    #[test]
    fn settlement_refund_calldata_empty_uid() {
        let data = settlement_refund_calldata("0x").unwrap();
        // 4 (selector) + 32 (offset) + 32 (length=0) = 68
        assert_eq!(data.len(), 68);
    }

    // ── ethflow_refund_calldata tests ────────────────────────────────────

    #[test]
    fn ethflow_refund_calldata_valid() {
        let uid = dummy_uid_56();
        let data = ethflow_refund_calldata(&uid).unwrap();
        assert_eq!(data.len(), 132);
        assert_eq!(&data[..4], &selector("invalidateOrder(bytes)"));
    }

    #[test]
    fn ethflow_refund_calldata_invalid_hex() {
        assert!(ethflow_refund_calldata("not_hex_gg").is_err());
    }

    #[test]
    fn ethflow_refund_calldata_without_prefix() {
        let uid = "cd".repeat(56);
        let data = ethflow_refund_calldata(&uid).unwrap();
        assert_eq!(data.len(), 132);
    }

    // ── is_refundable tests ──────────────────────────────────────────────

    #[test]
    fn is_refundable_zero_filled() {
        assert!(is_refundable(U256::ZERO, U256::from(1000)));
    }

    #[test]
    fn is_refundable_partial_filled() {
        assert!(is_refundable(U256::from(500), U256::from(1000)));
    }

    #[test]
    fn is_refundable_fully_filled() {
        assert!(!is_refundable(U256::from(1000), U256::from(1000)));
    }

    #[test]
    fn is_refundable_zero_total() {
        assert!(!is_refundable(U256::ZERO, U256::ZERO));
    }

    // ── refund_amount tests ──────────────────────────────────────────────

    #[test]
    fn refund_amount_partial() {
        assert_eq!(refund_amount(U256::from(300), U256::from(1000)), U256::from(700));
    }

    #[test]
    fn refund_amount_fully_filled() {
        assert_eq!(refund_amount(U256::from(1000), U256::from(1000)), U256::ZERO);
    }

    #[test]
    fn refund_amount_zero_filled() {
        assert_eq!(refund_amount(U256::ZERO, U256::from(500)), U256::from(500));
    }

    #[test]
    fn refund_amount_overfilled_saturates() {
        // Saturating subtraction prevents underflow.
        assert_eq!(refund_amount(U256::from(2000), U256::from(1000)), U256::ZERO);
    }

    // ── Helper tests ────────────────────────────────────────────────────

    #[test]
    fn padded32_rounds_up() {
        assert_eq!(padded32(0), 0);
        assert_eq!(padded32(1), 32);
        assert_eq!(padded32(31), 32);
        assert_eq!(padded32(32), 32);
        assert_eq!(padded32(33), 64);
    }
}
