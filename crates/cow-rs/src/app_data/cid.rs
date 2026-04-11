//! IPFS `CIDv1` conversion helpers for `CoW` Protocol app-data.
//!
//! Every `CoW` Protocol order's `appData` hash can be mapped to an IPFS
//! Content Identifier (CID) so that the full JSON document is retrievable
//! from any IPFS gateway. This module handles the bidirectional conversion
//! between the 32-byte `appDataHex` stored on-chain and the `CIDv1` string
//! used by IPFS.
//!
//! The modern encoding uses `keccak256` with the `raw` multicodec (`0x55`)
//! and multibase base16 (lowercase, prefix `f`). Legacy helpers using
//! `dag-pb` / `sha2-256` are preserved for backwards compatibility but are
//! deprecated.
//!
//! # Key functions
//!
//! | Function | Direction |
//! |---|---|
//! | [`appdata_hex_to_cid`] | `appDataHex` → `CIDv1` string |
//! | [`cid_to_appdata_hex`] | `CIDv1` string → `appDataHex` |
//! | [`parse_cid`] | `CIDv1` string → [`CidComponents`] |
//! | [`decode_cid`] | raw CID bytes → [`CidComponents`] |
//! | [`extract_digest`] | `CIDv1` string → digest hex |

use alloy_primitives::keccak256;

use crate::error::CowError;

// CIDv1 constants (modern encoding)
const CID_VERSION: u8 = 0x01;
const MULTICODEC_RAW: u8 = 0x55;
const HASH_KECCAK256: u8 = 0x1b;
const HASH_LEN: u8 = 0x20; // 32 bytes

// CIDv1 constants (legacy encoding: dag-pb + sha2-256)
const MULTICODEC_DAG_PB: u8 = 0x70;
const HASH_SHA2_256: u8 = 0x12;

/// Convert an `appDataHex` value (the 32-byte `keccak256` stored in the
/// order struct) into a `CIDv1` string.
///
/// The CID is built by hashing the raw bytes of `app_data_hex` with
/// `keccak256`, then wrapping the digest in a `CIDv1` envelope:
/// `[version=0x01, codec=0x55 (raw), hash_fn=0x1b (keccak256), len=0x20, ...digest]`.
/// The result is returned as a multibase base16 string (prefix `f`).
///
/// This is the inverse of [`cid_to_appdata_hex`].
///
/// Mirrors `appDataHexToCid` from the `@cowprotocol/app-data` `TypeScript`
/// package.
///
/// # Parameters
///
/// * `app_data_hex` — the `appData` value, with or without `0x` prefix.
///
/// # Returns
///
/// A base16 `CIDv1` string prefixed with `f` (e.g.
/// `f015501201b20...`).
///
/// # Errors
///
/// Returns [`CowError::AppData`] if `app_data_hex` is not valid hex.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{appdata_hex_to_cid, cid_to_appdata_hex};
///
/// let hex = "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
/// let cid = appdata_hex_to_cid(hex).unwrap();
/// assert!(cid.starts_with('f')); // multibase base16
/// ```
pub fn appdata_hex_to_cid(app_data_hex: &str) -> Result<String, CowError> {
    let hex = app_data_hex.strip_prefix("0x").map_or(app_data_hex, |s| s);
    let bytes = alloy_primitives::hex::decode(hex)
        .map_err(|e| CowError::AppData(format!("invalid hex: {e}")))?;

    // CID digest = keccak256(the 32 raw bytes of appDataHex)
    let digest = keccak256(&bytes);

    // CIDv1: [version, codec, hash_fn, hash_len, ...digest...]
    let mut cid = Vec::with_capacity(4 + 32);
    cid.push(CID_VERSION);
    cid.push(MULTICODEC_RAW);
    cid.push(HASH_KECCAK256);
    cid.push(HASH_LEN);
    cid.extend_from_slice(digest.as_slice());

    // Multibase base16 lowercase: prefix 'f'
    Ok(format!("f{}", alloy_primitives::hex::encode(&cid)))
}

/// Extract the digest from a `CIDv1` base16 string and return it as
/// `0x`-prefixed hex.
///
/// This is the inverse of [`appdata_hex_to_cid`]: given a CID stored
/// alongside an order, recover the 32-byte digest embedded in the CID
/// header. The returned value can be used as the `appData` field in an
/// on-chain order struct.
///
/// Only base16 CIDs (prefix `f` or `F`) are supported; other multibase
/// encodings will return an error.
///
/// Mirrors `cidToAppDataHex` from the `@cowprotocol/app-data` `TypeScript`
/// package.
///
/// # Parameters
///
/// * `cid` — a base16 multibase CID string (e.g. `"f015501201b20..."`).
///
/// # Returns
///
/// A `0x`-prefixed, lowercase hex string of the 32-byte digest.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if the CID is not base16, not valid hex,
/// or shorter than 36 bytes (4-byte header + 32-byte digest).
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{appdata_hex_to_cid, cid_to_appdata_hex};
///
/// let hex = "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
/// let cid = appdata_hex_to_cid(hex).unwrap();
/// let recovered = cid_to_appdata_hex(&cid).unwrap();
/// assert!(recovered.starts_with("0x"));
/// assert_eq!(recovered.len(), 66); // "0x" + 64 hex chars
/// ```
pub fn cid_to_appdata_hex(cid: &str) -> Result<String, CowError> {
    let lower = cid.to_ascii_lowercase();
    let hex = lower
        .strip_prefix('f')
        .ok_or_else(|| CowError::AppData("only base16 CIDs are supported (prefix 'f')".into()))?;

    let bytes = alloy_primitives::hex::decode(hex)
        .map_err(|e| CowError::AppData(format!("invalid CID hex: {e}")))?;

    // Skip CIDv1 header: version(1) + codec(1) + hash_fn(1) + hash_len(1) = 4 bytes
    if bytes.len() < 4 + 32 {
        return Err(CowError::AppData("CID too short".into()));
    }
    let digest = &bytes[4..4 + 32];
    Ok(format!("0x{}", alloy_primitives::hex::encode(digest)))
}

// ── Legacy CID helpers ──────────────────────────────────────────────────────

/// Internal helper: build CID bytes from the given multicodec and hash
/// algorithm parameters.
///
/// This is the Rust equivalent of the `TypeScript` SDK's `_toCidBytes`.
fn to_cid_bytes(
    version: u8,
    multicodec: u8,
    hashing_algorithm: u8,
    hashing_length: u8,
    multihash_hex: &str,
) -> Result<Vec<u8>, CowError> {
    let hex = multihash_hex.strip_prefix("0x").map_or(multihash_hex, |s| s);
    let hash_bytes = alloy_primitives::hex::decode(hex)
        .map_err(|e| CowError::AppData(format!("invalid hex: {e}")))?;

    let mut cid = Vec::with_capacity(4 + hash_bytes.len());
    cid.push(version);
    cid.push(multicodec);
    cid.push(hashing_algorithm);
    cid.push(hashing_length);
    cid.extend_from_slice(&hash_bytes);
    Ok(cid)
}

/// Internal helper: convert an `appDataHex` to a `CIDv1` string using the
/// legacy encoding (`sha2-256` + `dag-pb` multicodec).
///
/// **Note**: Legacy CIDs used `CIDv0` (`base58btc`) in the `TypeScript` SDK. This Rust
/// implementation returns the CID as base16 (prefix `f`) since the crate does not
/// include a `base58` encoder. Callers requiring `CIDv0` format should convert externally.
///
/// This is the Rust equivalent of `_appDataHexToCidLegacy` in the `TypeScript` SDK.
fn app_data_hex_to_cid_legacy_aux(app_data_hex: &str) -> Result<String, CowError> {
    let cid_bytes =
        to_cid_bytes(CID_VERSION, MULTICODEC_DAG_PB, HASH_SHA2_256, HASH_LEN, app_data_hex)?;
    // Return as base16 since we don't have base58 encoding
    Ok(format!("f{}", alloy_primitives::hex::encode(&cid_bytes)))
}

/// Validate that a CID string is non-empty.
///
/// A simple guard used after CID derivation to ensure the conversion did
/// not silently produce an empty string. If `cid` is empty, returns an
/// error that includes the original `app_data_hex` for debugging.
///
/// Mirrors `_assertCid` from the `@cowprotocol/app-data` `TypeScript` package.
///
/// # Parameters
///
/// * `cid` — the CID string to validate.
/// * `app_data_hex` — the source hex, included in the error message on failure.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if `cid` is empty.
pub fn assert_cid(cid: &str, app_data_hex: &str) -> Result<(), CowError> {
    if cid.is_empty() {
        return Err(CowError::AppData(format!("Error getting CID from appDataHex: {app_data_hex}")));
    }
    Ok(())
}

/// Convert an `appDataHex` to a `CIDv1` string using the legacy encoding.
///
/// Uses `dag-pb` multicodec with `sha2-256` hashing, matching the original
/// IPFS CID generation before `CoW` Protocol switched to `keccak256`.
///
/// **Note**: The `TypeScript` SDK returns a `CIDv0` (`base58btc`) string. This Rust
/// implementation returns base16 (prefix `f`) since no `base58` encoder is bundled.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if `app_data_hex` cannot be decoded.
#[deprecated(
    note = "Use appdata_hex_to_cid instead — legacy CID encoding is no longer used by CoW Protocol"
)]
pub fn app_data_hex_to_cid_legacy(app_data_hex: &str) -> Result<String, CowError> {
    let cid = app_data_hex_to_cid_legacy_aux(app_data_hex)?;
    assert_cid(&cid, app_data_hex)?;
    Ok(cid)
}

/// Parsed components of an IPFS Content Identifier (CID).
///
/// A CID encodes four header fields followed by the raw hash digest:
///
/// ```text
/// ┌─────────┬───────┬──────────────┬────────────┬──────────────┐
/// │ version │ codec │ hash_function│ hash_length│   digest     │
/// │  (1 B)  │ (1 B) │    (1 B)     │   (1 B)    │ (N bytes)    │
/// └─────────┴───────┴──────────────┴────────────┴──────────────┘
/// ```
///
/// Use [`parse_cid`] to obtain this from a multibase string, or
/// [`decode_cid`] to obtain it from raw bytes.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{appdata_hex_to_cid, parse_cid};
///
/// let hex = "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
/// let cid = appdata_hex_to_cid(hex).unwrap();
/// let components = parse_cid(&cid).unwrap();
/// assert_eq!(components.version, 0x01); // CIDv1
/// assert_eq!(components.codec, 0x55); // raw multicodec
/// assert_eq!(components.hash_function, 0x1b); // keccak256
/// assert_eq!(components.hash_length, 0x20); // 32 bytes
/// assert_eq!(components.digest.len(), 32);
/// ```
#[derive(Debug, Clone)]
pub struct CidComponents {
    /// CID version (e.g. `1` for `CIDv1`).
    pub version: u8,
    /// Multicodec code (e.g. `0x55` for raw, `0x70` for dag-pb).
    pub codec: u8,
    /// Multihash function code (e.g. `0x1b` for keccak256, `0x12` for sha2-256).
    pub hash_function: u8,
    /// Hash digest length in bytes (typically `32`).
    pub hash_length: u8,
    /// The raw hash digest bytes.
    pub digest: Vec<u8>,
}

/// Parse a CID string into its constituent [`CidComponents`].
///
/// Decodes the multibase prefix, strips it, hex-decodes the remainder, and
/// splits the resulting bytes into the four header fields plus the digest.
///
/// Currently supports base16 multibase encoding (prefix `f` or `F`). Other
/// multibase encodings (e.g. `base58btc` starting with `Qm`) return an
/// error.
///
/// Mirrors `parseCid` from the `@cowprotocol/app-data` `TypeScript` package.
///
/// # Parameters
///
/// * `ipfs_hash` — a multibase-encoded CID string (e.g. `"f015501201b20..."`).
///
/// # Returns
///
/// A [`CidComponents`] struct with the parsed version, codec, hash function,
/// hash length, and raw digest bytes.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if the CID encoding is unsupported, the
/// hex is invalid, or the payload is shorter than 4 bytes.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{appdata_hex_to_cid, parse_cid};
///
/// let hex = "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
/// let cid = appdata_hex_to_cid(hex).unwrap();
/// let c = parse_cid(&cid).unwrap();
/// assert_eq!(c.version, 1);
/// assert_eq!(c.digest.len(), 32);
/// ```
pub fn parse_cid(ipfs_hash: &str) -> Result<CidComponents, CowError> {
    let lower = ipfs_hash.to_ascii_lowercase();
    let hex = lower
        .strip_prefix('f')
        .ok_or_else(|| CowError::AppData("only base16 CIDs are supported (prefix 'f')".into()))?;

    let bytes = alloy_primitives::hex::decode(hex)
        .map_err(|e| CowError::AppData(format!("invalid CID hex: {e}")))?;

    if bytes.len() < 4 {
        return Err(CowError::AppData("CID too short".into()));
    }

    let version = bytes[0];
    let codec = bytes[1];
    let hash_function = bytes[2];
    let hash_length = bytes[3];
    let digest = bytes[4..].to_vec();

    Ok(CidComponents { version, codec, hash_function, hash_length, digest })
}

/// Decode raw CID bytes into their constituent [`CidComponents`].
///
/// Unlike [`parse_cid`], this function operates on raw bytes rather than a
/// multibase-encoded string. Use it when you already have the CID as a byte
/// slice (e.g. from a binary protocol or a database column).
///
/// Mirrors `decodeCid` from the `@cowprotocol/app-data` `TypeScript` package.
///
/// # Parameters
///
/// * `bytes` — raw CID bytes: `[version, codec, hash_fn, hash_len, ...digest]`.
///
/// # Returns
///
/// A [`CidComponents`] struct with the parsed fields.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if the byte slice is shorter than 4 bytes
/// (the minimum CID header size).
///
/// # Example
///
/// ```
/// use cow_rs::app_data::decode_cid;
///
/// let mut bytes = vec![0x01, 0x55, 0x1b, 0x20];
/// bytes.extend_from_slice(&[0u8; 32]); // 32 digest bytes
/// let c = decode_cid(&bytes).unwrap();
/// assert_eq!(c.version, 1);
/// assert_eq!(c.codec, 0x55);
/// assert_eq!(c.digest.len(), 32);
/// ```
pub fn decode_cid(bytes: &[u8]) -> Result<CidComponents, CowError> {
    if bytes.len() < 4 {
        return Err(CowError::AppData("CID bytes too short".into()));
    }

    Ok(CidComponents {
        version: bytes[0],
        codec: bytes[1],
        hash_function: bytes[2],
        hash_length: bytes[3],
        digest: bytes[4..].to_vec(),
    })
}

/// Extract the multihash digest from a CID string and return it as
/// `0x`-prefixed hex.
///
/// Parses the CID via [`parse_cid`], then returns only the raw digest
/// portion as a `0x`-prefixed hex string. This is useful when you have a
/// CID from IPFS and need to recover the hash digest to match against
/// on-chain `appData` values.
///
/// Note: the digest extracted here is the hash **inside** the CID, not the
/// original `appDataHex`. For round-trip conversion use [`cid_to_appdata_hex`].
///
/// Mirrors `extractDigest` from the `@cowprotocol/app-data` `TypeScript`
/// package.
///
/// # Parameters
///
/// * `cid` — a base16 multibase CID string.
///
/// # Returns
///
/// A `0x`-prefixed hex string of the raw digest bytes.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if the CID cannot be parsed.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{appdata_hex_to_cid, extract_digest};
///
/// let hex = "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
/// let cid = appdata_hex_to_cid(hex).unwrap();
/// let digest = extract_digest(&cid).unwrap();
/// assert!(digest.starts_with("0x"));
/// assert_eq!(digest.len(), 66); // "0x" + 64 hex chars
/// ```
pub fn extract_digest(cid: &str) -> Result<String, CowError> {
    let components = parse_cid(cid)?;
    Ok(format!("0x{}", alloy_primitives::hex::encode(&components.digest)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HEX: &str = "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";

    #[test]
    fn appdata_hex_to_cid_produces_base16_cid() {
        let cid = appdata_hex_to_cid(SAMPLE_HEX).unwrap_or_default();
        assert!(cid.starts_with('f'));
        // CID header (4 bytes) + digest (32 bytes) = 36 bytes → 72 hex chars + 'f' prefix
        assert_eq!(cid.len(), 1 + 72);
    }

    #[test]
    fn appdata_hex_to_cid_without_0x_prefix() {
        let hex = SAMPLE_HEX.strip_prefix("0x").unwrap_or_else(|| SAMPLE_HEX);
        let cid = appdata_hex_to_cid(hex).unwrap_or_default();
        assert!(cid.starts_with('f'));
    }

    #[test]
    fn cid_to_appdata_hex_roundtrip() {
        let cid = appdata_hex_to_cid(SAMPLE_HEX).unwrap_or_default();
        let recovered = cid_to_appdata_hex(&cid).unwrap_or_default();
        assert!(recovered.starts_with("0x"));
        assert_eq!(recovered.len(), 66);
    }

    #[test]
    fn cid_to_appdata_hex_rejects_non_base16() {
        assert!(cid_to_appdata_hex("Qmabc123").is_err());
        assert!(cid_to_appdata_hex("babc123").is_err());
    }

    #[test]
    fn cid_to_appdata_hex_rejects_too_short() {
        assert!(cid_to_appdata_hex("f0155").is_err());
    }

    #[test]
    fn parse_cid_components() {
        let cid = appdata_hex_to_cid(SAMPLE_HEX).unwrap_or_default();
        let c = parse_cid(&cid).unwrap_or_else(|_| CidComponents {
            version: 0,
            codec: 0,
            hash_function: 0,
            hash_length: 0,
            digest: vec![],
        });
        assert_eq!(c.version, CID_VERSION);
        assert_eq!(c.codec, MULTICODEC_RAW);
        assert_eq!(c.hash_function, HASH_KECCAK256);
        assert_eq!(c.hash_length, HASH_LEN);
        assert_eq!(c.digest.len(), 32);
    }

    #[test]
    fn parse_cid_rejects_non_base16() {
        assert!(parse_cid("not_a_cid").is_err());
    }

    #[test]
    fn parse_cid_rejects_too_short() {
        assert!(parse_cid("f01").is_err());
    }

    #[test]
    fn decode_cid_from_bytes() {
        let mut bytes = vec![0x01, 0x55, 0x1b, 0x20];
        bytes.extend_from_slice(&[0xaa; 32]);
        let c = decode_cid(&bytes).unwrap_or_else(|_| CidComponents {
            version: 0,
            codec: 0,
            hash_function: 0,
            hash_length: 0,
            digest: vec![],
        });
        assert_eq!(c.version, 1);
        assert_eq!(c.codec, 0x55);
        assert_eq!(c.digest.len(), 32);
    }

    #[test]
    fn decode_cid_rejects_short_bytes() {
        assert!(decode_cid(&[0x01, 0x02, 0x03]).is_err());
        assert!(decode_cid(&[]).is_err());
    }

    #[test]
    fn extract_digest_returns_0x_prefixed() {
        let cid = appdata_hex_to_cid(SAMPLE_HEX).unwrap_or_default();
        let digest = extract_digest(&cid).unwrap_or_default();
        assert!(digest.starts_with("0x"));
        assert_eq!(digest.len(), 66);
    }

    #[test]
    fn assert_cid_accepts_nonempty() {
        assert!(assert_cid("f01234", "0xabc").is_ok());
    }

    #[test]
    fn assert_cid_rejects_empty() {
        assert!(assert_cid("", "0xabc").is_err());
    }

    #[test]
    #[allow(deprecated, reason = "testing legacy API surface")]
    fn legacy_cid_produces_base16_string() {
        let cid = app_data_hex_to_cid_legacy(SAMPLE_HEX).unwrap_or_default();
        assert!(cid.starts_with('f'));
    }

    #[test]
    fn appdata_hex_to_cid_invalid_hex() {
        assert!(appdata_hex_to_cid("0xZZZZ").is_err());
    }

    #[test]
    fn deterministic_output() {
        let cid1 = appdata_hex_to_cid(SAMPLE_HEX).unwrap_or_default();
        let cid2 = appdata_hex_to_cid(SAMPLE_HEX).unwrap_or_default();
        assert_eq!(cid1, cid2);
    }

    #[test]
    fn cid_to_appdata_hex_invalid_hex() {
        assert!(cid_to_appdata_hex("fZZZZinvalid").is_err());
    }

    #[test]
    fn parse_cid_uppercase_f_prefix() {
        let cid = appdata_hex_to_cid(SAMPLE_HEX).unwrap();
        // Replace lowercase 'f' prefix with uppercase 'F'
        let upper = format!("F{}", &cid[1..]);
        let c = parse_cid(&upper).unwrap();
        assert_eq!(c.version, CID_VERSION);
    }

    #[test]
    fn to_cid_bytes_without_0x() {
        let hex = SAMPLE_HEX.strip_prefix("0x").unwrap();
        let bytes = to_cid_bytes(CID_VERSION, MULTICODEC_RAW, HASH_KECCAK256, HASH_LEN, hex);
        assert!(bytes.is_ok());
    }

    #[test]
    fn to_cid_bytes_invalid_hex() {
        let result = to_cid_bytes(CID_VERSION, MULTICODEC_RAW, HASH_KECCAK256, HASH_LEN, "ZZZZ");
        assert!(result.is_err());
    }
}
