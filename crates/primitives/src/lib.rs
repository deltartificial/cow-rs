//! `cow-sdk-primitives` — Layer 0 foundational constants for the `CoW` Protocol SDK.
//!
//! This crate sits at the bottom of the workspace DAG and has **no internal
//! dependencies**. It exposes:
//!
//! - Numeric constants (`ZERO`, `ONE`, `MAX_UINT256`, `ONE_HUNDRED_BPS`, ...)
//! - The zero address and zero hash helpers
//! - Near Intents attestation constants
//!
//! Protocol enums (`OrderKind`, `SigningScheme`, `TokenBalance`, ...) live in
//! [`cow-sdk-types`](https://docs.rs/cow-sdk-types) (Layer 1) because their
//! `TryFrom<&str>` impls depend on [`cow-sdk-error`](https://docs.rs/cow-sdk-error).

#![deny(unsafe_code)]
#![warn(missing_docs)]

use alloy_primitives::{Address, U256};

// ── Common constants ────────────────────────────────────────────────────────

/// The zero address (`0x0000…0000`).
pub const ZERO_ADDRESS: Address = Address::ZERO;

/// The 32-byte zero hash.
pub const ZERO_HASH: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";

/// `U256` zero.
pub const ZERO: U256 = U256::ZERO;

/// `U256` one.
pub const ONE: U256 = U256::from_limbs([1, 0, 0, 0]);

/// Maximum `u32` value as a `U256` (2^32 - 1 = 4 294 967 295).
pub const MAX_UINT32: U256 = U256::from_limbs([u32::MAX as u64, 0, 0, 0]);

/// Maximum `U256` value (2^256 - 1).
pub const MAX_UINT256: U256 = U256::MAX;

/// Scale factor: 100 000.
pub const HUNDRED_THOUSANDS: u64 = 100_000;

/// One hundred basis points expressed as a `U256` (100 * 100 = 10 000).
pub const ONE_HUNDRED_BPS: u64 = 10_000;

/// Maximum concurrent requests to the `CoW` Protocol API.
pub const LIMIT_CONCURRENT_REQUESTS: u32 = 5;

/// Near Intents attestation prefix constant.
pub const ATTESTATION_PREFIX_CONST: &str = "0x0a773570";

/// Near Intents attestation version byte.
pub const ATTESTION_VERSION_BYTE: &str = "0x00";

/// Near Intents attestator address.
///
/// `0x0073DD100b51C555E41B2a452E5933ef76F42790`
pub const ATTESTATOR_ADDRESS: Address = Address::new([
    0x00, 0x73, 0xdd, 0x10, 0x0b, 0x51, 0xc5, 0x55, 0xe4, 0x1b, 0x2a, 0x45, 0x2e, 0x59, 0x33, 0xef,
    0x76, 0xf4, 0x27, 0x90,
]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_correct() {
        assert!(ZERO_ADDRESS.is_zero());
        assert_eq!(ZERO, U256::ZERO);
        assert_eq!(ONE, U256::from(1));
        assert_eq!(MAX_UINT32, U256::from(u32::MAX));
        assert_eq!(MAX_UINT256, U256::MAX);
        assert_eq!(ONE_HUNDRED_BPS, 10_000);
        assert_eq!(HUNDRED_THOUSANDS, 100_000);
        assert_eq!(LIMIT_CONCURRENT_REQUESTS, 5);
    }

    #[test]
    fn zero_hash_is_correct_length() {
        assert_eq!(ZERO_HASH.len(), 66); // "0x" + 64 hex chars
        assert!(ZERO_HASH.starts_with("0x"));
    }

    #[test]
    fn attestator_address_is_nonzero() {
        assert!(!ATTESTATOR_ADDRESS.is_zero());
    }
}
