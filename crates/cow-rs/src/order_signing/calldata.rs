//! ABI-encoded calldata builders for the `GPv2Settlement` contract.
//!
//! These functions build the raw transaction data needed for on-chain order
//! management:
//!
//! - [`set_pre_signature_calldata`] — mark an order as pre-signed (or unmark it)
//! - [`invalidate_order_calldata`] — permanently cancel an order on-chain
//!
//! Pair these with [`crate::config::contracts::SETTLEMENT_CONTRACT`] as the
//! transaction target.

use alloy_primitives::keccak256;

use crate::error::CowError;

// ── Function selectors ────────────────────────────────────────────────────────

/// Compute the 4-byte selector from a Solidity function signature.
///
/// # Arguments
///
/// * `sig` — the Solidity function signature string (e.g. `"transfer(address,uint256)"`).
///
/// # Returns
///
/// The first 4 bytes of the Keccak-256 hash of `sig`.
fn selector(sig: &str) -> [u8; 4] {
    let h = keccak256(sig.as_bytes());
    [h[0], h[1], h[2], h[3]]
}

// ── ABI helpers ───────────────────────────────────────────────────────────────

/// Encode a `u64` as a 32-byte big-endian ABI word.
///
/// # Arguments
///
/// * `v` — the value to encode.
///
/// # Returns
///
/// A 32-byte array with `v` right-aligned in big-endian format.
fn u256_be(v: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&v.to_be_bytes());
    out
}

/// Zero-pad `buf` to the next 32-byte boundary after `written` bytes.
///
/// # Arguments
///
/// * `buf` — the buffer to pad in place.
/// * `written` — the number of unpadded content bytes just appended.
fn pad_to(buf: &mut Vec<u8>, written: usize) {
    let rem = written % 32;
    if rem != 0 {
        buf.resize(buf.len() + (32 - rem), 0);
    }
}

// ── setPreSignature ───────────────────────────────────────────────────────────

/// Build calldata for `GPv2Settlement.setPreSignature(bytes orderUid, bool signed)`.
///
/// Pass `signed = true` to authenticate a pre-sign order and `signed = false`
/// to revoke the pre-signature.  `order_uid` must be the `0x`-prefixed 56-byte
/// hex string returned when the order was created.
///
/// The returned bytes should be sent in a transaction to
/// [`SETTLEMENT_CONTRACT`](crate::config::contracts::SETTLEMENT_CONTRACT).
///
/// Mirrors `getPreSignTransaction` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `order_uid` — the `0x`-prefixed hex-encoded order UID (56 bytes).
/// * `signed` — `true` to authenticate, `false` to revoke.
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
/// ```rust
/// use cow_rs::{SETTLEMENT_CONTRACT, order_signing::set_pre_signature_calldata};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let uid = "0x".to_owned() + &"ab".repeat(56); // 56-byte dummy UID
/// let calldata = set_pre_signature_calldata(&uid, true)?;
/// assert_eq!(calldata.len(), 164); // 4 + 32 + 32 + 32 + 64
///
/// # Ok(())
/// # }
/// ```
pub fn set_pre_signature_calldata(order_uid: &str, signed: bool) -> Result<Vec<u8>, CowError> {
    let uid_bytes = decode_uid(order_uid)?;
    // ABI encode (bytes, bool):
    //   head:  offset_to_bytes(32) = 64, bool(32)
    //   tail:  length(32) + uid_bytes(padded)
    let padded_len = padded32(uid_bytes.len());
    let mut buf = Vec::with_capacity(4 + 2 * 32 + 32 + padded_len);
    buf.extend_from_slice(&selector("setPreSignature(bytes,bool)"));
    buf.extend_from_slice(&u256_be(64)); // offset to bytes data = 2 head slots
    buf.extend_from_slice(&u256_be(u64::from(signed)));
    buf.extend_from_slice(&u256_be(uid_bytes.len() as u64));
    buf.extend_from_slice(&uid_bytes);
    pad_to(&mut buf, uid_bytes.len());
    Ok(buf)
}

// ── invalidateOrder ───────────────────────────────────────────────────────────

/// Build calldata for `GPv2Settlement.invalidateOrder(bytes orderUid)`.
///
/// Permanently cancels an order on-chain.  Once invalidated, the order can
/// never be executed even if it was previously signed or pre-signed.
///
/// The returned bytes should be sent in a transaction to
/// [`SETTLEMENT_CONTRACT`](crate::config::contracts::SETTLEMENT_CONTRACT).
///
/// Mirrors `getSettlementCancellation` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `order_uid` — the `0x`-prefixed hex-encoded order UID to invalidate.
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
/// ```rust
/// use cow_rs::order_signing::invalidate_order_calldata;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let uid = "0x".to_owned() + &"ab".repeat(56);
/// let calldata = invalidate_order_calldata(&uid)?;
/// assert_eq!(calldata.len(), 132); // 4 + 32 + 32 + 64
///
/// # Ok(())
/// # }
/// ```
pub fn invalidate_order_calldata(order_uid: &str) -> Result<Vec<u8>, CowError> {
    let uid_bytes = decode_uid(order_uid)?;
    // ABI encode (bytes):
    //   head:  offset_to_bytes(32) = 32 (1 head slot)
    //   tail:  length(32) + uid_bytes(padded)
    let padded_len = padded32(uid_bytes.len());
    let mut buf = Vec::with_capacity(4 + 32 + 32 + padded_len);
    buf.extend_from_slice(&selector("invalidateOrder(bytes)"));
    buf.extend_from_slice(&u256_be(32)); // offset to bytes data = 1 head slot
    buf.extend_from_slice(&u256_be(uid_bytes.len() as u64));
    buf.extend_from_slice(&uid_bytes);
    pad_to(&mut buf, uid_bytes.len());
    Ok(buf)
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Decode a `0x`-prefixed hex order UID into raw bytes.
///
/// # Arguments
///
/// * `uid` — the `0x`-prefixed hex string to decode.
///
/// # Returns
///
/// The decoded byte vector.
fn decode_uid(uid: &str) -> Result<Vec<u8>, CowError> {
    let stripped = uid.trim_start_matches("0x");
    alloy_primitives::hex::decode(stripped)
        .map_err(|_e| CowError::Api { status: 0, body: format!("invalid orderUid: {uid}") })
}

/// Round `n` up to the next multiple of 32.
///
/// # Arguments
///
/// * `n` — the byte count to round up.
///
/// # Returns
///
/// The smallest multiple of 32 that is >= `n`.
const fn padded32(n: usize) -> usize {
    if n.is_multiple_of(32) { n } else { n + (32 - n % 32) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_uid_56() -> String {
        "0x".to_owned() + &"ab".repeat(56)
    }

    #[test]
    fn set_pre_signature_calldata_valid() {
        let uid = dummy_uid_56();
        let data = set_pre_signature_calldata(&uid, true).unwrap_or_default();
        // 4 (selector) + 32 (offset) + 32 (bool) + 32 (length) + 64 (56 bytes padded to 64)
        assert_eq!(data.len(), 164);
        // First 4 bytes are the selector for setPreSignature(bytes,bool)
        assert_eq!(&data[..4], &selector("setPreSignature(bytes,bool)"));
    }

    #[test]
    fn set_pre_signature_calldata_false() {
        let uid = dummy_uid_56();
        let data = set_pre_signature_calldata(&uid, false).unwrap_or_default();
        assert_eq!(data.len(), 164);
        // bool = false → last byte of second word is 0
        assert_eq!(data[4 + 32 + 31], 0);
    }

    #[test]
    fn set_pre_signature_calldata_true() {
        let uid = dummy_uid_56();
        let data = set_pre_signature_calldata(&uid, true).unwrap_or_default();
        // bool = true → last byte of second word is 1
        assert_eq!(data[4 + 32 + 31], 1);
    }

    #[test]
    fn invalidate_order_calldata_valid() {
        let uid = dummy_uid_56();
        let data = invalidate_order_calldata(&uid).unwrap_or_default();
        // 4 (selector) + 32 (offset) + 32 (length) + 64 (56 bytes padded to 64)
        assert_eq!(data.len(), 132);
        assert_eq!(&data[..4], &selector("invalidateOrder(bytes)"));
    }

    #[test]
    fn calldata_rejects_invalid_hex() {
        assert!(set_pre_signature_calldata("not_hex", true).is_err());
        assert!(invalidate_order_calldata("0xZZZZ").is_err());
    }

    #[test]
    fn calldata_works_without_0x_prefix() {
        let uid = "ab".repeat(56);
        assert!(set_pre_signature_calldata(&uid, true).is_ok());
        assert!(invalidate_order_calldata(&uid).is_ok());
    }

    #[test]
    fn calldata_empty_uid() {
        let data = invalidate_order_calldata("0x").unwrap_or_default();
        // 4 (selector) + 32 (offset) + 32 (length=0) = 68
        assert_eq!(data.len(), 68);
    }

    #[test]
    fn padded32_rounds_up() {
        assert_eq!(padded32(0), 0);
        assert_eq!(padded32(1), 32);
        assert_eq!(padded32(31), 32);
        assert_eq!(padded32(32), 32);
        assert_eq!(padded32(33), 64);
        assert_eq!(padded32(64), 64);
    }
}
