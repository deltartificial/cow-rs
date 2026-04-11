//! On-chain settlement contract state reading via JSON-RPC.
//!
//! Provides [`SettlementReader`] for querying `GPv2Settlement` contract
//! state (filled amounts, pre-signatures, domain separator) and
//! [`AllowListReader`] for checking solver authorization status.

use alloy_primitives::{Address, B256, U256, keccak256};

use crate::{
    config::{chain::SupportedChainId, contracts::settlement_contract},
    error::CowError,
    onchain::OnchainReader,
};

/// Reads settlement contract state from an Ethereum node via JSON-RPC.
///
/// Wraps an [`OnchainReader`] targeting the `GPv2Settlement` contract on
/// a specific chain.
///
/// # Example
///
/// ```rust,no_run
/// use cow_rs::{SupportedChainId, settlement::reader::SettlementReader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let reader = SettlementReader::new("https://rpc.sepolia.org", SupportedChainId::Sepolia);
/// let domain_sep = reader.domain_separator().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SettlementReader {
    /// The settlement contract address.
    settlement: Address,
    /// Underlying JSON-RPC reader.
    reader: OnchainReader,
}

impl SettlementReader {
    /// Create a new settlement reader for the given chain.
    ///
    /// Uses the canonical `GPv2Settlement` contract address for `chain`.
    ///
    /// # Arguments
    ///
    /// * `rpc_url` - The JSON-RPC endpoint URL.
    /// * `chain` - The target [`SupportedChainId`].
    ///
    /// # Returns
    ///
    /// A new [`SettlementReader`] targeting the settlement contract on `chain`.
    #[must_use]
    pub fn new(rpc_url: impl Into<String>, chain: SupportedChainId) -> Self {
        Self { settlement: settlement_contract(chain), reader: OnchainReader::new(rpc_url) }
    }

    /// Create a settlement reader with a custom settlement contract address.
    ///
    /// Useful for staging/barn environments or non-standard deployments.
    ///
    /// # Arguments
    ///
    /// * `rpc_url` - The JSON-RPC endpoint URL.
    /// * `settlement` - The settlement contract [`Address`].
    ///
    /// # Returns
    ///
    /// A new [`SettlementReader`] targeting the specified settlement address.
    #[must_use]
    pub fn with_address(rpc_url: impl Into<String>, settlement: Address) -> Self {
        Self { settlement, reader: OnchainReader::new(rpc_url) }
    }

    /// Return the settlement contract address this reader targets.
    ///
    /// # Returns
    ///
    /// The settlement contract [`Address`].
    #[must_use]
    pub const fn settlement_address(&self) -> Address {
        self.settlement
    }

    /// Query the filled amount for an order by its UID.
    ///
    /// Calls `filledAmount(bytes)` on the settlement contract.
    ///
    /// # Arguments
    ///
    /// * `order_uid` - The hex-encoded order UID (with or without `0x` prefix).
    ///
    /// # Returns
    ///
    /// The filled amount as [`U256`].
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] on RPC failure, [`CowError::Parse`] if the
    /// order UID cannot be hex-decoded or the response cannot be decoded.
    pub async fn filled_amount(&self, order_uid: &str) -> Result<U256, CowError> {
        let uid_bytes = decode_hex_bytes(order_uid)?;
        let calldata = build_dynamic_bytes_call("filledAmount(bytes)", &uid_bytes);
        let ret = self.reader.eth_call(self.settlement, &calldata).await?;
        decode_u256(&ret)
    }

    /// Query the pre-signature status for an order.
    ///
    /// Calls `preSignature(bytes)` on the settlement contract.
    /// Returns `true` if the order has been pre-signed.
    ///
    /// # Arguments
    ///
    /// * `order_uid` - The hex-encoded order UID (with or without `0x` prefix).
    ///
    /// # Returns
    ///
    /// `true` if the order has a valid pre-signature, `false` otherwise.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] on RPC failure, [`CowError::Parse`] if the
    /// order UID cannot be hex-decoded or the response cannot be decoded.
    pub async fn pre_signature(&self, order_uid: &str) -> Result<bool, CowError> {
        let uid_bytes = decode_hex_bytes(order_uid)?;
        let calldata = build_dynamic_bytes_call("preSignature(bytes)", &uid_bytes);
        let ret = self.reader.eth_call(self.settlement, &calldata).await?;
        let value = decode_u256(&ret)?;
        Ok(!value.is_zero())
    }

    /// Query the EIP-712 domain separator from the settlement contract.
    ///
    /// Calls `domainSeparator()` on the settlement contract.
    ///
    /// # Returns
    ///
    /// The 32-byte domain separator as [`B256`].
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] on RPC failure or [`CowError::Parse`] if
    /// the response cannot be decoded.
    pub async fn domain_separator(&self) -> Result<B256, CowError> {
        let selector = &keccak256("domainSeparator()")[..4];
        let ret = self.reader.eth_call(self.settlement, selector).await?;
        if ret.len() < 32 {
            return Err(CowError::Parse {
                field: "domainSeparator",
                reason: format!("expected 32 bytes, got {}", ret.len()),
            });
        }
        Ok(B256::from_slice(&ret[..32]))
    }
}

/// Reads the `GPv2AllowListAuthentication` contract to check solver authorization.
///
/// # Example
///
/// ```rust,no_run
/// use alloy_primitives::address;
/// use cow_rs::settlement::reader::AllowListReader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let allow_list = address!("2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE");
/// let reader = AllowListReader::new("https://rpc.sepolia.org", allow_list);
/// let is_solver = reader.is_solver(address!("1111111111111111111111111111111111111111")).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct AllowListReader {
    /// The allow-list contract address.
    allow_list: Address,
    /// Underlying JSON-RPC reader.
    reader: OnchainReader,
}

impl AllowListReader {
    /// Create a new allow-list reader.
    ///
    /// # Arguments
    ///
    /// * `rpc_url` - The JSON-RPC endpoint URL.
    /// * `allow_list` - The [`Address`] of the `GPv2AllowListAuthentication` contract.
    ///
    /// # Returns
    ///
    /// A new [`AllowListReader`] targeting the specified contract.
    #[must_use]
    pub fn new(rpc_url: impl Into<String>, allow_list: Address) -> Self {
        Self { allow_list, reader: OnchainReader::new(rpc_url) }
    }

    /// Return the allow-list contract address.
    ///
    /// # Returns
    ///
    /// The allow-list contract [`Address`].
    #[must_use]
    pub const fn allow_list_address(&self) -> Address {
        self.allow_list
    }

    /// Check whether an address is an authorized solver.
    ///
    /// Calls `isSolver(address)` on the allow-list contract.
    ///
    /// # Arguments
    ///
    /// * `address` - The [`Address`] to check.
    ///
    /// # Returns
    ///
    /// `true` if the address is an authorized solver.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] on RPC failure or [`CowError::Parse`] if
    /// the response cannot be decoded.
    pub async fn is_solver(&self, address: Address) -> Result<bool, CowError> {
        let selector = &keccak256("isSolver(address)")[..4];
        let mut calldata = Vec::with_capacity(36);
        calldata.extend_from_slice(selector);
        calldata.extend_from_slice(&abi_address(address));
        let ret = self.reader.eth_call(self.allow_list, &calldata).await?;
        let value = decode_u256(&ret)?;
        Ok(!value.is_zero())
    }
}

// ── Private helpers ────────────────────────────────────────────────────────

/// Left-pad an [`Address`] to a 32-byte ABI word.
fn abi_address(a: Address) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[12..].copy_from_slice(a.as_slice());
    buf
}

/// Decode a hex string (with or without `0x` prefix) into bytes.
fn decode_hex_bytes(hex_str: &str) -> Result<Vec<u8>, CowError> {
    let clean = hex_str.trim_start_matches("0x");
    alloy_primitives::hex::decode(clean)
        .map_err(|e| CowError::Parse { field: "order_uid", reason: format!("invalid hex: {e}") })
}

/// Build calldata for a function that takes a single `bytes` parameter.
///
/// ABI layout: selector + offset (32) + length (32) + padded data.
fn build_dynamic_bytes_call(signature: &str, data: &[u8]) -> Vec<u8> {
    let selector = &keccak256(signature.as_bytes())[..4];
    let padding = (32 - (data.len() % 32)) % 32;
    let total = 4 + 32 + 32 + data.len() + padding;

    let mut buf = Vec::with_capacity(total);
    buf.extend_from_slice(selector);
    // Offset to dynamic data (always 32 for single dynamic param).
    buf.extend_from_slice(&abi_u256(U256::from(32)));
    // Length of bytes data.
    buf.extend_from_slice(&abi_u256(U256::from(data.len())));
    // Actual data.
    buf.extend_from_slice(data);
    // Pad to 32-byte boundary.
    buf.extend_from_slice(&vec![0u8; padding]);
    buf
}

/// Encode a [`U256`] as a 32-byte big-endian ABI word.
const fn abi_u256(v: U256) -> [u8; 32] {
    v.to_be_bytes()
}

/// Decode a big-endian `uint256` from the first 32 bytes.
fn decode_u256(bytes: &[u8]) -> Result<U256, CowError> {
    if bytes.len() < 32 {
        return Err(CowError::Parse {
            field: "uint256",
            reason: format!("expected >= 32 bytes, got {}", bytes.len()),
        });
    }
    let arr: [u8; 32] = bytes[..32]
        .try_into()
        .map_err(|_e| CowError::Parse { field: "uint256", reason: "slice conversion".into() })?;
    Ok(U256::from_be_bytes(arr))
}

#[cfg(test)]
mod tests {
    use alloy_primitives::address;

    use super::*;
    use crate::config::contracts::SETTLEMENT_CONTRACT;

    #[test]
    fn settlement_reader_new_uses_canonical_address() {
        let reader = SettlementReader::new("https://example.com", SupportedChainId::Mainnet);
        assert_eq!(reader.settlement_address(), SETTLEMENT_CONTRACT);
    }

    #[test]
    fn settlement_reader_with_address() {
        let custom = address!("1111111111111111111111111111111111111111");
        let reader = SettlementReader::with_address("https://example.com", custom);
        assert_eq!(reader.settlement_address(), custom);
    }

    #[test]
    fn allow_list_reader_new() {
        let allow_list = address!("2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE");
        let reader = AllowListReader::new("https://example.com", allow_list);
        assert_eq!(reader.allow_list_address(), allow_list);
    }

    #[test]
    fn decode_hex_bytes_with_prefix() {
        let result = decode_hex_bytes("0xdeadbeef").unwrap();
        assert_eq!(result, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn decode_hex_bytes_without_prefix() {
        let result = decode_hex_bytes("abcd").unwrap();
        assert_eq!(result, vec![0xab, 0xcd]);
    }

    #[test]
    fn decode_hex_bytes_invalid() {
        assert!(decode_hex_bytes("not_hex_gg").is_err());
    }

    #[test]
    fn build_dynamic_bytes_call_format() {
        let data = vec![0xde, 0xad, 0xbe, 0xef];
        let calldata = build_dynamic_bytes_call("filledAmount(bytes)", &data);

        // Selector (4) + offset (32) + length (32) + data (4) + padding (28) = 100
        assert_eq!(calldata.len(), 100);

        // Verify selector.
        let expected_sel = &keccak256(b"filledAmount(bytes)")[..4];
        assert_eq!(&calldata[..4], expected_sel);

        // Verify offset = 32.
        let offset_word = &calldata[4..36];
        assert_eq!(offset_word[31], 32);

        // Verify length = 4.
        let len_word = &calldata[36..68];
        assert_eq!(len_word[31], 4);

        // Verify data.
        assert_eq!(&calldata[68..72], &[0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn build_dynamic_bytes_call_empty_data() {
        let calldata = build_dynamic_bytes_call("filledAmount(bytes)", &[]);
        // Selector (4) + offset (32) + length (32) = 68
        assert_eq!(calldata.len(), 68);
    }

    #[test]
    fn build_dynamic_bytes_call_32_byte_data() {
        let data = vec![0xab; 32];
        let calldata = build_dynamic_bytes_call("filledAmount(bytes)", &data);
        // 4 + 32 + 32 + 32 = 100 (no padding needed)
        assert_eq!(calldata.len(), 100);
    }

    #[test]
    fn decode_u256_valid() {
        let mut buf = [0u8; 32];
        buf[31] = 42;
        let v = decode_u256(&buf).unwrap();
        assert_eq!(v, U256::from(42));
    }

    #[test]
    fn decode_u256_too_short() {
        assert!(decode_u256(&[0u8; 16]).is_err());
    }

    #[test]
    fn decode_u256_max() {
        let buf = [0xFFu8; 32];
        let v = decode_u256(&buf).unwrap();
        assert_eq!(v, U256::MAX);
    }

    #[test]
    fn settlement_reader_clone() {
        let reader = SettlementReader::new("https://example.com", SupportedChainId::Mainnet);
        let cloned = reader.clone();
        assert_eq!(cloned.settlement_address(), reader.settlement_address());
    }

    #[test]
    fn allow_list_reader_clone() {
        let allow_list = address!("2c4c28DDBdAc9C5E7055b4C863b72eA0149D8aFE");
        let reader = AllowListReader::new("https://example.com", allow_list);
        let cloned = reader.clone();
        assert_eq!(cloned.allow_list_address(), reader.allow_list_address());
    }
}
