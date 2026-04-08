//! Private-key validation and normalisation utilities.
//!
//! Provides helpers to validate and normalise 32-byte hex private keys,
//! ported from the `TypeScript` SDK's `common/crypto` module.

use crate::error::CowError;

/// Normalise a private key to the `0x`-prefixed 64-character hex format.
///
/// Accepts keys with or without the `0x` prefix and validates that the
/// key is exactly 32 bytes (64 hex characters). The returned string is
/// always lowercase `0x`-prefixed.
///
/// # Parameters
///
/// * `private_key` — the private key string to normalise (with or without
///   `0x` prefix).
///
/// # Returns
///
/// A `0x`-prefixed, 66-character hex string (`"0x"` + 64 hex chars).
///
/// # Errors
///
/// Returns [`CowError::Parse`] if the key is empty, not valid hex, or
/// not exactly 64 hex characters (32 bytes).
///
/// # Examples
///
/// ```
/// use cow_rs::common::crypto::normalize_private_key;
///
/// let key = "0x".to_owned() + &"ab".repeat(32);
/// let normalized = normalize_private_key(&key).unwrap();
/// assert!(normalized.starts_with("0x"));
/// assert_eq!(normalized.len(), 66); // "0x" + 64 hex chars
/// ```
pub fn normalize_private_key(private_key: &str) -> Result<String, CowError> {
    if private_key.is_empty() {
        return Err(CowError::Parse {
            field: "private_key",
            reason: "private key must be a non-empty string".to_owned(),
        });
    }

    let clean = private_key.strip_prefix("0x").unwrap_or(private_key);

    if clean.len() != 64 || !clean.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(CowError::Parse {
            field: "private_key",
            reason: "invalid private key format: must be exactly 64 hexadecimal characters (with or without 0x prefix)".to_owned(),
        });
    }

    Ok(format!("0x{clean}"))
}

/// Returns `true` if `private_key` is a valid 32-byte hex private key.
///
/// Convenience wrapper around [`normalize_private_key`] — returns `true`
/// when normalisation succeeds.
///
/// # Parameters
///
/// * `private_key` — the key string to validate (with or without `0x`).
///
/// # Returns
///
/// `true` if the key is exactly 64 hex characters (32 bytes).
///
/// ```
/// use cow_rs::common::crypto::is_valid_private_key;
///
/// let valid = "0x".to_owned() + &"ab".repeat(32);
/// assert!(is_valid_private_key(&valid));
///
/// assert!(!is_valid_private_key("not_hex"));
/// assert!(!is_valid_private_key("0xabcd")); // too short
/// ```
#[must_use]
pub fn is_valid_private_key(private_key: &str) -> bool {
    normalize_private_key(private_key).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_with_prefix() {
        let key = format!("0x{}", "ab".repeat(32));
        let result = normalize_private_key(&key).unwrap();
        assert_eq!(result, key);
    }

    #[test]
    fn normalize_without_prefix() {
        let hex = "ab".repeat(32);
        let result = normalize_private_key(&hex).unwrap();
        assert_eq!(result, format!("0x{hex}"));
    }

    #[test]
    fn normalize_empty_fails() {
        assert!(normalize_private_key("").is_err());
    }

    #[test]
    fn normalize_too_short_fails() {
        assert!(normalize_private_key("0xabcd").is_err());
    }

    #[test]
    fn normalize_invalid_hex_fails() {
        let bad = format!("0x{}", "zz".repeat(32));
        assert!(normalize_private_key(&bad).is_err());
    }

    #[test]
    fn is_valid_true() {
        let key = format!("0x{}", "12".repeat(32));
        assert!(is_valid_private_key(&key));
    }

    #[test]
    fn is_valid_false() {
        assert!(!is_valid_private_key("invalid"));
    }
}
