//! `cow-onchain` — Layer 4 JSON-RPC `eth_call` reader for the `CoW` Protocol SDK.
//!
//! Reads on-chain state (balances, allowances, decimals, permit nonces)
//! via raw JSON-RPC. Uses a `reqwest` client directly — no alloy-provider
//! dependency is required, keeping the dep tree clean.
//!
//! # Example
//!
//! ```rust,no_run
//! use alloy_primitives::{Address, U256, address};
//! use cow_onchain::OnchainReader;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let reader = OnchainReader::new("https://rpc.sepolia.org");
//! let token = address!("fFf9976782d46CC05630D1f6eBAb18b2324d6B14");
//! let owner = address!("1111111111111111111111111111111111111111");
//! let bal: U256 = reader.erc20_balance(token, owner).await?;
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod erc20;
pub mod permit;

use alloy_primitives::{Address, B256};
use cow_chains::contracts::{IMPLEMENTATION_STORAGE_SLOT, OWNER_STORAGE_SLOT};
use cow_errors::CowError;
use serde::Deserialize;

/// Reads on-chain state from an Ethereum node via JSON-RPC `eth_call`.
///
/// Constructed with [`OnchainReader::new`].  All methods are `async` and
/// make a single `POST` to the configured RPC endpoint.
#[derive(Debug, Clone)]
pub struct OnchainReader {
    client: reqwest::Client,
    rpc_url: String,
}

impl OnchainReader {
    /// Build a `reqwest::Client` with platform-appropriate settings.
    ///
    /// # Returns
    ///
    /// A configured [`reqwest::Client`] with a 30-second timeout on native targets,
    /// or a default client on WASM targets. Falls back to [`reqwest::Client::default`]
    /// if the builder fails.
    #[allow(clippy::shadow_reuse, reason = "builder pattern chains naturally shadow")]
    fn build_client() -> reqwest::Client {
        let builder = reqwest::Client::builder();
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.timeout(std::time::Duration::from_secs(30));
        builder.build().unwrap_or_default()
    }

    /// Create a new reader targeting the given JSON-RPC endpoint URL.
    ///
    /// The reader uses a shared `reqwest::Client` with a 30-second timeout
    /// (on non-WASM targets) for all subsequent `eth_call` requests.
    ///
    /// # Arguments
    ///
    /// * `rpc_url` - The JSON-RPC endpoint URL (e.g. `"https://rpc.sepolia.org"`). Accepts any type
    ///   that implements `Into<String>`.
    ///
    /// # Returns
    ///
    /// A new [`OnchainReader`] instance configured to query the given endpoint.
    ///
    /// # Example
    ///
    /// ```rust
    /// use cow_onchain::OnchainReader;
    /// let reader = OnchainReader::new("https://rpc.sepolia.org");
    /// ```
    #[must_use]
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self { client: Self::build_client(), rpc_url: rpc_url.into() }
    }

    /// Low-level `eth_call`: send ABI-encoded `data` to contract `to` at block `"latest"`.
    ///
    /// Returns the decoded return bytes. Callers are responsible for ABI-decoding
    /// the result (e.g. via the crate's private `decode_u256` or
    /// `decode_string` helpers).
    ///
    /// # Arguments
    ///
    /// * `to` - The contract [`Address`] to call.
    /// * `data` - ABI-encoded calldata (selector + arguments).
    ///
    /// # Returns
    ///
    /// The raw bytes returned by the contract, hex-decoded from the RPC response.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] if the HTTP request fails, the RPC node returns
    /// an error object, or the hex-encoded result cannot be decoded.
    pub async fn eth_call(&self, to: Address, data: &[u8]) -> Result<Vec<u8>, CowError> {
        let to_hex = format!("{to:#x}");
        let data_hex = format!("0x{}", alloy_primitives::hex::encode(data));

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method":  "eth_call",
            "params":  [{"to": to_hex, "data": data_hex}, "latest"],
            "id":      1u32
        });

        let resp = self.client.post(&self.rpc_url).json(&body).send().await?;

        if !resp.status().is_success() {
            let code = i64::from(resp.status().as_u16());
            let msg = resp.text().await.unwrap_or_else(|_e| String::new());
            return Err(CowError::Rpc { code, message: msg });
        }

        let rpc: RpcResponse = resp.json().await?;

        if let Some(err) = rpc.error {
            return Err(CowError::Rpc { code: err.code, message: err.message });
        }

        let hex_str = rpc
            .result
            .ok_or_else(|| CowError::Rpc { code: -1, message: "missing result field".into() })?;

        let hex_clean = hex_str.as_str().trim_start_matches("0x");

        alloy_primitives::hex::decode(hex_clean)
            .map_err(|e| CowError::Rpc { code: -1, message: format!("hex decode: {e}") })
    }

    /// Low-level `eth_getStorageAt`: read a single storage slot at block `"latest"`.
    ///
    /// Returns the raw 32-byte slot value.
    ///
    /// # Arguments
    ///
    /// * `address` - The contract [`Address`] whose storage to read.
    /// * `slot` - The hex-encoded storage slot position (e.g. an EIP-1967 slot).
    ///
    /// # Returns
    ///
    /// The 32-byte storage value as [`B256`].
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] if the HTTP request fails, the RPC node returns
    /// an error object, or the hex-encoded result cannot be decoded.
    pub async fn eth_get_storage_at(&self, address: Address, slot: &str) -> Result<B256, CowError> {
        let addr_hex = format!("{address:#x}");

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method":  "eth_getStorageAt",
            "params":  [addr_hex, slot, "latest"],
            "id":      1u32
        });

        let resp = self.client.post(&self.rpc_url).json(&body).send().await?;

        if !resp.status().is_success() {
            let code = i64::from(resp.status().as_u16());
            let msg = resp.text().await.unwrap_or_else(|_e| String::new());
            return Err(CowError::Rpc { code, message: msg });
        }

        let rpc: RpcResponse = resp.json().await?;

        if let Some(err) = rpc.error {
            return Err(CowError::Rpc { code: err.code, message: err.message });
        }

        let hex_str = rpc
            .result
            .ok_or_else(|| CowError::Rpc { code: -1, message: "missing result field".into() })?;

        let hex_clean = hex_str.as_str().trim_start_matches("0x");
        let bytes = alloy_primitives::hex::decode(hex_clean)
            .map_err(|e| CowError::Rpc { code: -1, message: format!("hex decode: {e}") })?;

        if bytes.len() < 32 {
            return Err(CowError::Rpc {
                code: -1,
                message: format!("expected 32 bytes, got {}", bytes.len()),
            });
        }

        Ok(B256::from_slice(&bytes[..32]))
    }

    /// Read the EIP-1967 implementation address of a proxy contract.
    ///
    /// Mirrors `implementationAddress` from the `TypeScript` `contracts-ts` package.
    /// Makes an `eth_getStorageAt` JSON-RPC call to read the implementation slot
    /// and decodes the result as an [`Address`].
    ///
    /// # Arguments
    ///
    /// * `proxy` - The [`Address`] of the EIP-1967 proxy contract.
    ///
    /// # Returns
    ///
    /// The implementation contract [`Address`] stored in the proxy's
    /// EIP-1967 implementation slot.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] if the RPC request fails.
    pub async fn implementation_address(&self, proxy: Address) -> Result<Address, CowError> {
        let slot_value = self.eth_get_storage_at(proxy, IMPLEMENTATION_STORAGE_SLOT).await?;
        Ok(Address::from_slice(&slot_value[12..]))
    }

    /// Read the EIP-1967 admin/owner address of a proxy contract.
    ///
    /// Mirrors `ownerAddress` from the `TypeScript` `contracts-ts` package.
    /// Makes an `eth_getStorageAt` JSON-RPC call to read the admin slot
    /// and decodes the result as an [`Address`].
    ///
    /// # Arguments
    ///
    /// * `proxy` - The [`Address`] of the EIP-1967 proxy contract.
    ///
    /// # Returns
    ///
    /// The admin/owner [`Address`] stored in the proxy's EIP-1967 admin slot.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] if the RPC request fails.
    pub async fn owner_address(&self, proxy: Address) -> Result<Address, CowError> {
        let slot_value = self.eth_get_storage_at(proxy, OWNER_STORAGE_SLOT).await?;
        Ok(Address::from_slice(&slot_value[12..]))
    }
}

// ── JSON-RPC response types (private) ────────────────────────────────────────

#[derive(Deserialize)]
struct RpcResponse {
    result: Option<String>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

// ── ABI decode helpers (pub(crate) for child modules + tests) ─────────────────

/// Decode a big-endian `uint256` from the first 32 bytes of `bytes`.
///
/// # Arguments
///
/// * `bytes` - The raw ABI-encoded response bytes (must be at least 32 bytes).
///
/// # Returns
///
/// The decoded [`U256`](alloy_primitives::U256) value from the first 32-byte word.
pub(crate) fn decode_u256(bytes: &[u8]) -> Result<alloy_primitives::U256, CowError> {
    if bytes.len() < 32 {
        return Err(CowError::Parse {
            field: "uint256",
            reason: format!("expected ≥ 32 bytes, got {}", bytes.len()),
        });
    }
    let arr: [u8; 32] = bytes[..32]
        .try_into()
        .map_err(|_e| CowError::Parse { field: "uint256", reason: "slice conversion".into() })?;
    Ok(alloy_primitives::U256::from_be_bytes(arr))
}

/// Decode a `uint8` from the ABI-padded 32-byte word (last byte).
///
/// # Arguments
///
/// * `bytes` - The raw ABI-encoded response bytes (must be at least 32 bytes).
///
/// # Returns
///
/// The `u8` value extracted from the last byte of the first 32-byte word.
pub(crate) fn decode_u8(bytes: &[u8]) -> Result<u8, CowError> {
    if bytes.len() < 32 {
        return Err(CowError::Parse {
            field: "uint8",
            reason: format!("expected ≥ 32 bytes, got {}", bytes.len()),
        });
    }
    Ok(bytes[31])
}

/// Decode an ABI-encoded dynamic `string` return value.
///
/// ABI layout:
/// ```text
/// [0x00..0x1f]  offset  (= 0x20)
/// [0x20..0x3f]  length  N
/// [0x40..0x40+N] UTF-8 bytes
/// ```
///
/// # Arguments
///
/// * `bytes` - The raw ABI-encoded response bytes (must be at least 64 bytes, plus the string
///   length indicated in the length word).
///
/// # Returns
///
/// The decoded UTF-8 [`String`] extracted from the ABI-encoded payload.
pub(crate) fn decode_string(bytes: &[u8]) -> Result<String, CowError> {
    if bytes.len() < 64 {
        return Err(CowError::Parse {
            field: "string",
            reason: format!("expected ≥ 64 bytes, got {}", bytes.len()),
        });
    }
    let len_arr: [u8; 32] = bytes[32..64]
        .try_into()
        .map_err(|_e| CowError::Parse { field: "string", reason: "length slice".into() })?;
    let len_u256 = alloy_primitives::U256::from_be_bytes(len_arr);
    let len = usize::try_from(len_u256).map_err(|_e| CowError::Parse {
        field: "string",
        reason: "length overflows usize".into(),
    })?;
    if bytes.len() < 64 + len {
        return Err(CowError::Parse {
            field: "string",
            reason: format!("truncated: need {} + 64 bytes, got {}", len, bytes.len()),
        });
    }
    String::from_utf8(bytes[64..64 + len].to_vec())
        .map_err(|e| CowError::Parse { field: "string", reason: e.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_u256_roundtrip() {
        let mut buf = [0u8; 32];
        buf[31] = 42;
        let v = decode_u256(&buf).unwrap();
        assert_eq!(v, alloy_primitives::U256::from(42u64));
    }

    #[test]
    fn decode_u256_too_short() {
        let result = decode_u256(&[0u8; 16]);
        assert!(result.is_err());
    }

    #[test]
    fn decode_u8_roundtrip() {
        let mut buf = [0u8; 32];
        buf[31] = 18;
        assert_eq!(decode_u8(&buf).unwrap(), 18u8);
    }

    #[test]
    fn decode_u8_too_short() {
        assert!(decode_u8(&[0u8; 10]).is_err());
    }

    #[test]
    fn decode_string_roundtrip() {
        // Build ABI-encoded string "WETH"
        let mut buf = vec![0u8; 96];
        // offset = 32
        buf[31] = 32;
        // length = 4
        buf[63] = 4;
        // data
        buf[64..68].copy_from_slice(b"WETH");
        assert_eq!(decode_string(&buf).unwrap(), "WETH");
    }

    #[test]
    fn decode_string_too_short() {
        assert!(decode_string(&[0u8; 32]).is_err());
    }

    #[test]
    fn decode_string_truncated() {
        let mut buf = vec![0u8; 64];
        buf[31] = 32;
        buf[63] = 100; // length = 100 but no data
        assert!(decode_string(&buf).is_err());
    }

    #[test]
    fn onchain_reader_new() {
        let reader = OnchainReader::new("https://example.com");
        assert_eq!(reader.rpc_url, "https://example.com");
    }

    #[test]
    fn decode_string_invalid_utf8() {
        let mut buf = vec![0u8; 96];
        buf[31] = 32; // offset
        buf[63] = 2; // length = 2
        buf[64] = 0xFF;
        buf[65] = 0xFE;
        assert!(decode_string(&buf).is_err());
    }

    #[test]
    fn decode_u256_large_value() {
        let buf = [0xFFu8; 32];
        let v = decode_u256(&buf).unwrap();
        assert_eq!(v, alloy_primitives::U256::MAX);
    }

    #[test]
    fn decode_u256_extra_bytes_ignored() {
        let mut buf = vec![0u8; 64];
        buf[31] = 7;
        buf[63] = 99;
        let v = decode_u256(&buf).unwrap();
        assert_eq!(v, alloy_primitives::U256::from(7u64));
    }

    #[test]
    fn decode_u8_zero() {
        let buf = [0u8; 32];
        assert_eq!(decode_u8(&buf).unwrap(), 0u8);
    }

    #[test]
    fn decode_string_empty_string() {
        let mut buf = vec![0u8; 96];
        buf[31] = 32; // offset = 32
        buf[63] = 0; // length = 0
        assert_eq!(decode_string(&buf).unwrap(), "");
    }

    #[test]
    fn onchain_reader_clone() {
        let reader = OnchainReader::new("https://example.com");
        let cloned = reader.clone();
        assert_eq!(cloned.rpc_url, reader.rpc_url);
    }

    #[test]
    fn onchain_reader_debug() {
        let reader = OnchainReader::new("https://example.com");
        let s = format!("{reader:?}");
        assert!(s.contains("OnchainReader"));
    }

    #[test]
    fn decode_string_length_overflows_usize() {
        // Place a non-zero byte in the high half of the length word so the
        // decoded `U256` is larger than `usize::MAX` on a 64-bit host. This
        // forces the `usize::try_from` branch to return `Err`, exercising
        // the overflow `map_err` arm in `decode_string`.
        let mut buf = vec![0u8; 64];
        buf[31] = 32; // offset = 32
        // Length word: bytes[32..64]. Set byte 32 (top byte of the 256-bit
        // big-endian length) so the value is way beyond usize::MAX.
        buf[32] = 0x01;
        let result = decode_string(&buf);
        let err = result.expect_err("length > usize::MAX should fail");
        assert!(
            matches!(&err, CowError::Parse { field, reason } if *field == "string" && reason.contains("overflows usize")),
            "got {err:?}"
        );
    }
}
