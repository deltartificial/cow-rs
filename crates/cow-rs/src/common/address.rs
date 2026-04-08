//! Address validation, normalisation, and comparison utilities.
//!
//! Supports EVM, Bitcoin, and Solana address formats. Ported from the
//! `TypeScript` SDK's `common/address` module.
//!
//! # Key functions
//!
//! | Function | Purpose |
//! |---|---|
//! | [`is_evm_address`] | Validate `0x` + 40 hex chars |
//! | [`is_btc_address`] | Validate legacy P2PKH/P2SH or Bech32 |
//! | [`is_solana_address`] | Validate Base58 32-44 chars |
//! | [`get_address_key`] | Normalise any address to an [`AddressKey`] |
//! | [`are_addresses_equal`] | Format-aware equality check |
//! | [`get_token_id`] | Build `{chain_id}:{address}` composite key |
//! | [`is_native_token`] | Check if address is the chain's native currency |
//! | [`is_wrapped_native_token`] | Check if address is the wrapped native (e.g. WETH) |
//! | [`is_cow_settlement_contract`] | Match against `CoW` settlement addresses |
//! | [`is_cow_vault_relayer_contract`] | Match against `CoW` vault relayer addresses |

use crate::config::{
    SupportedChainId,
    chains::get_chain_info,
    contracts::{
        SETTLEMENT_CONTRACT, SETTLEMENT_CONTRACT_STAGING, VAULT_RELAYER, VAULT_RELAYER_STAGING,
    },
    wrapped_native_currency,
};

// ── Address key types ────────────────────────────────────────────────────────

/// A normalised address key suitable for use as a map key or comparison
/// token.
///
/// For EVM addresses, this is the checksumless lowercase hex form
/// (`0xabcdef…`). For BTC and Solana addresses, it is the original string
/// (they are case-sensitive by design).
///
/// Obtain an instance via [`get_address_key`].
///
/// # Example
///
/// ```
/// use cow_rs::common::address::{get_address_key, AddressKey};
///
/// let key = get_address_key("0xABCDEF1234567890abcdef1234567890ABCDEF12");
/// assert!(matches!(key, AddressKey::Evm(_)));
/// assert_eq!(key.as_str(), "0xabcdef1234567890abcdef1234567890abcdef12");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AddressKey {
    /// An EVM address, normalized to lowercase hex with `0x` prefix.
    Evm(String),
    /// A Bitcoin address (case-sensitive, returned as-is).
    Btc(String),
    /// A Solana address (case-sensitive, returned as-is).
    Sol(String),
}

impl AddressKey {
    /// Return the inner string slice regardless of variant.
    ///
    /// # Returns
    ///
    /// A `&str` referencing the normalised address string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Evm(s) | Self::Btc(s) | Self::Sol(s) => s,
        }
    }
}

impl std::fmt::Display for AddressKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A composite identifier for a token: `{chain_id}:{address_key}`.
///
/// Encodes both the chain and the normalised address in a single string
/// so that tokens can be compared and used as map keys across chains.
///
/// Obtain an instance via [`get_token_id`].
///
/// # Example
///
/// ```
/// use cow_rs::common::address::get_token_id;
///
/// let id = get_token_id(1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
/// assert_eq!(id.as_str(), "1:0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenId(String);

impl TokenId {
    /// Return the string representation.
    ///
    /// # Returns
    ///
    /// A `&str` referencing the `{chain_id}:{address_key}` string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TokenId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── Validation ───────────────────────────────────────────────────────────────

/// Returns `true` if `address` is a valid EVM address.
///
/// A valid EVM address starts with `0x` (or `0X`) followed by exactly 40
/// hexadecimal characters (case-insensitive). Does **not** validate
/// `EIP-55` mixed-case checksum.
///
/// # Parameters
///
/// * `address` — the string to validate.
///
/// # Returns
///
/// `true` if the string matches the `0x[0-9a-fA-F]{40}` pattern.
///
/// ```
/// use cow_rs::common::address::is_evm_address;
///
/// assert!(is_evm_address("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
/// assert!(is_evm_address("0x0000000000000000000000000000000000000000"));
/// assert!(!is_evm_address("0xZZZZ"));
/// assert!(!is_evm_address("not_an_address"));
/// ```
#[must_use]
pub fn is_evm_address(address: &str) -> bool {
    if address.len() != 42 {
        return false;
    }
    let Some(hex) = address.strip_prefix("0x").or_else(|| address.strip_prefix("0X")) else {
        return false;
    };
    hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Returns `true` if `address` is a valid Bitcoin address.
///
/// Recognises legacy P2PKH/P2SH addresses (starting with `1` or `3`,
/// 25-34 Base58 characters) and Bech32 mainnet addresses (starting with
/// `bc1` / `BC1`, 42-62 alphanumeric characters).
///
/// # Parameters
///
/// * `address` — the string to validate.
///
/// # Returns
///
/// `true` if the string matches a known Bitcoin address format.
///
/// ```
/// use cow_rs::common::address::is_btc_address;
///
/// // Legacy P2PKH
/// assert!(is_btc_address("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"));
/// // Bech32
/// assert!(is_btc_address("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"));
/// assert!(!is_btc_address("not_a_btc_address"));
/// ```
#[must_use]
pub fn is_btc_address(address: &str) -> bool {
    let len = address.len();
    if len < 25 || len > 62 {
        return false;
    }
    is_btc_legacy(address) || is_btc_bech32_mainnet(address)
}

/// Returns `true` if `address` is a valid Solana address.
///
/// Solana addresses are Base58-encoded Ed25519 public keys, 32-44
/// characters long, using the Base58 alphabet (no `0`, `O`, `I`, `l`).
///
/// # Parameters
///
/// * `address` — the string to validate.
///
/// # Returns
///
/// `true` if the string is 32-44 Base58 characters.
///
/// ```
/// use cow_rs::common::address::is_solana_address;
///
/// assert!(is_solana_address("11111111111111111111111111111111"));
/// assert!(!is_solana_address("short"));
/// ```
#[must_use]
pub fn is_solana_address(address: &str) -> bool {
    let len = address.len();
    if len < 32 || len > 44 {
        return false;
    }
    address.bytes().all(is_base58_char)
}

/// Returns `true` if `address` is any supported address format (EVM, BTC,
/// or Solana).
///
/// Delegates to [`is_evm_address`], [`is_btc_address`], and
/// [`is_solana_address`] in order.
///
/// # Parameters
///
/// * `address` — the string to validate.
///
/// # Returns
///
/// `true` if the string matches any supported format.
///
/// ```
/// use cow_rs::common::address::is_supported_address;
///
/// assert!(is_supported_address("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
/// assert!(is_supported_address("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"));
/// assert!(!is_supported_address("xyz"));
/// ```
#[must_use]
pub fn is_supported_address(address: &str) -> bool {
    is_evm_address(address) || is_btc_address(address) || is_solana_address(address)
}

// ── Normalization / key extraction ───────────────────────────────────────────

/// Normalise an EVM address to a lowercase hex key.
///
/// # Parameters
///
/// * `address` — the EVM address string (any case).
///
/// # Returns
///
/// A lowercase copy of the input string.
///
/// ```
/// use cow_rs::common::address::get_evm_address_key;
///
/// assert_eq!(
///     get_evm_address_key("0xABCDEF1234567890abcdef1234567890ABCDEF12"),
///     "0xabcdef1234567890abcdef1234567890abcdef12"
/// );
/// ```
#[must_use]
pub fn get_evm_address_key(address: &str) -> String {
    address.to_ascii_lowercase()
}

/// Return a Bitcoin address key (identity -- BTC addresses are case-sensitive).
///
/// # Parameters
///
/// * `address` — the BTC address string (returned as-is).
///
/// # Returns
///
/// An owned copy of the input string.
///
/// ```
/// use cow_rs::common::address::get_btc_address_key;
///
/// let addr = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
/// assert_eq!(get_btc_address_key(addr), addr);
/// ```
#[must_use]
pub fn get_btc_address_key(address: &str) -> String {
    address.to_owned()
}

/// Return a Solana address key (identity -- Solana addresses are case-sensitive).
///
/// # Parameters
///
/// * `address` — the Solana address string (returned as-is).
///
/// # Returns
///
/// An owned copy of the input string.
///
/// ```
/// use cow_rs::common::address::get_sol_address_key;
///
/// let addr = "11111111111111111111111111111111";
/// assert_eq!(get_sol_address_key(addr), addr);
/// ```
#[must_use]
pub fn get_sol_address_key(address: &str) -> String {
    address.to_owned()
}

/// Normalise any supported address to its canonical [`AddressKey`].
///
/// EVM addresses are lowercased; BTC and Solana addresses are returned
/// as-is (case-sensitive). Unknown formats default to the
/// [`AddressKey::Sol`] variant (identity key).
///
/// # Parameters
///
/// * `address` — the address string to normalise.
///
/// # Returns
///
/// An [`AddressKey`] variant matching the detected format.
///
/// ```
/// use cow_rs::common::address::{AddressKey, get_address_key};
///
/// let key = get_address_key("0xABCDEF1234567890abcdef1234567890ABCDEF12");
/// assert_eq!(key.as_str(), "0xabcdef1234567890abcdef1234567890abcdef12");
/// ```
#[must_use]
pub fn get_address_key(address: &str) -> AddressKey {
    if is_evm_address(address) {
        AddressKey::Evm(get_evm_address_key(address))
    } else if is_btc_address(address) {
        AddressKey::Btc(address.to_owned())
    } else {
        // Default to Sol-style key (identity).
        AddressKey::Sol(address.to_owned())
    }
}

// ── Comparison ───────────────────────────────────────────────────────────────

/// Compare two addresses for equality in a format-aware manner.
///
/// EVM addresses are compared case-insensitively (both lowercased before
/// comparison). BTC and Solana addresses are compared as exact strings.
/// Returns `false` if either argument is `None`.
///
/// # Parameters
///
/// * `a` — first address (optional).
/// * `b` — second address (optional).
///
/// # Returns
///
/// `true` if both are `Some` and represent the same address.
///
/// ```
/// use cow_rs::common::address::are_addresses_equal;
///
/// assert!(are_addresses_equal(
///     Some("0xABCDEF1234567890abcdef1234567890ABCDEF12"),
///     Some("0xabcdef1234567890abcdef1234567890abcdef12"),
/// ));
/// assert!(!are_addresses_equal(None, Some("0x1234")));
/// ```
#[must_use]
pub fn are_addresses_equal(a: Option<&str>, b: Option<&str>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };

    let a_is_evm = is_evm_address(a);
    let b_is_evm = is_evm_address(b);

    if a_is_evm && b_is_evm {
        return get_evm_address_key(a) == get_evm_address_key(b);
    }

    // BTC and Solana addresses are case-sensitive.
    a == b
}

/// A minimal token-like type for address comparison.
///
/// Implement this trait on your token types so they can be compared via
/// [`are_tokens_equal`] and identified via [`get_token_id`].
pub trait TokenLike {
    /// The chain ID this token lives on (e.g. `1` for Ethereum mainnet).
    fn chain_id(&self) -> u64;
    /// The token's on-chain address as a string (any supported format).
    fn address(&self) -> &str;
}

/// Compare two tokens for equality by chain ID and normalised address.
///
/// Two tokens are equal if and only if they share the same chain ID and
/// their addresses match (using format-aware normalisation). Returns
/// `false` if either argument is `None`.
///
/// # Parameters
///
/// * `a` — first token (optional reference).
/// * `b` — second token (optional reference).
///
/// # Returns
///
/// `true` if both are `Some` and have identical [`TokenId`] values.
///
/// ```
/// use cow_rs::common::address::{TokenLike, are_tokens_equal};
///
/// struct Tok {
///     chain: u64,
///     addr: String,
/// }
/// impl TokenLike for Tok {
///     fn chain_id(&self) -> u64 {
///         self.chain
///     }
///     fn address(&self) -> &str {
///         &self.addr
///     }
/// }
///
/// let a = Tok { chain: 1, addr: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".into() };
/// let b = Tok { chain: 1, addr: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".into() };
/// assert!(are_tokens_equal(Some(&a), Some(&b)));
/// ```
#[must_use]
pub fn are_tokens_equal<T: TokenLike>(a: Option<&T>, b: Option<&T>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    get_token_id(a.chain_id(), a.address()) == get_token_id(b.chain_id(), b.address())
}

/// Build a composite token identifier: `{chain_id}:{normalised_address}`.
///
/// The address is normalised via [`get_address_key`] before concatenation,
/// ensuring case-insensitive uniqueness for EVM tokens.
///
/// # Parameters
///
/// * `chain_id` — the chain ID (e.g. `1` for mainnet).
/// * `address` — the token's on-chain address.
///
/// # Returns
///
/// A [`TokenId`] string like `"1:0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"`.
///
/// ```
/// use cow_rs::common::address::get_token_id;
///
/// let id = get_token_id(1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
/// assert_eq!(id.as_str(), "1:0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
/// ```
#[must_use]
pub fn get_token_id(chain_id: u64, address: &str) -> TokenId {
    let key = get_address_key(address);
    TokenId(format!("{chain_id}:{key}"))
}

// ── Token classification ─────────────────────────────────────────────────────

/// Returns `true` if the given address is the native currency for
/// `chain_id`.
///
/// Compares `address` against the native currency address defined in the
/// chain's configuration (e.g. `0xEeee…` for ETH on Ethereum mainnet).
///
/// # Parameters
///
/// * `chain_id` — the chain ID to look up.
/// * `address` — the token address to check.
///
/// # Returns
///
/// `true` if `address` matches the chain's native currency address,
/// `false` otherwise (including when `chain_id` is unknown).
///
/// ```
/// use cow_rs::common::address::is_native_token;
///
/// assert!(is_native_token(1, "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"));
/// assert!(!is_native_token(1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
/// ```
#[must_use]
pub fn is_native_token(chain_id: u64, address: &str) -> bool {
    let Some(chain_info) = get_chain_info(chain_id) else {
        return false;
    };
    let native_addr = chain_info.native_currency().address;
    are_addresses_equal(Some(native_addr), Some(address))
}

/// Returns `true` if the given address is the wrapped native currency for
/// `chain_id` (e.g. WETH on Ethereum mainnet, WXDAI on Gnosis).
///
/// Only returns `true` for chains in [`SupportedChainId`]. Unknown chain
/// IDs return `false`.
///
/// # Parameters
///
/// * `chain_id` — the chain ID to look up.
/// * `address` — the token address to check.
///
/// # Returns
///
/// `true` if `address` matches the chain's wrapped native token.
///
/// ```
/// use cow_rs::common::address::is_wrapped_native_token;
///
/// // WETH on mainnet
/// assert!(is_wrapped_native_token(1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
/// assert!(!is_wrapped_native_token(1, "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"));
/// ```
#[must_use]
pub fn is_wrapped_native_token(chain_id: u64, address: &str) -> bool {
    let Some(supported) = SupportedChainId::try_from_u64(chain_id) else {
        return false;
    };
    let wrapped = wrapped_native_currency(supported);
    are_addresses_equal(Some(&format!("{:#x}", wrapped.address)), Some(address))
}

// ── Protocol contract helpers ────────────────────────────────────────────────

/// Returns `true` if `address` matches a `CoW` Protocol settlement
/// contract (production or staging) on the given chain.
///
/// Checks against both the production [`SETTLEMENT_CONTRACT`] and the
/// staging [`SETTLEMENT_CONTRACT_STAGING`] addresses.
///
/// # Parameters
///
/// * `address` — the contract address to check.
/// * `_chain_id` — the [`SupportedChainId`] (currently unused since
///   settlement addresses are the same across chains).
///
/// # Returns
///
/// `true` if `address` matches either settlement contract address.
///
/// ```
/// use cow_rs::{SupportedChainId, common::address::is_cow_settlement_contract};
///
/// assert!(is_cow_settlement_contract(
///     "0x9008D19f58AAbD9eD0D60971565AA8510560ab41",
///     SupportedChainId::Mainnet,
/// ));
/// ```
#[must_use]
pub fn is_cow_settlement_contract(address: &str, _chain_id: SupportedChainId) -> bool {
    let key = get_address_key(address);
    let prod = format!("{:#x}", SETTLEMENT_CONTRACT);
    let staging = format!("{:#x}", SETTLEMENT_CONTRACT_STAGING);
    are_addresses_equal(Some(key.as_str()), Some(&prod)) ||
        are_addresses_equal(Some(key.as_str()), Some(&staging))
}

/// Returns `true` if `address` matches a `CoW` Protocol vault relayer
/// contract (production or staging) on the given chain.
///
/// Checks against both the production [`VAULT_RELAYER`] and the staging
/// [`VAULT_RELAYER_STAGING`] addresses.
///
/// # Parameters
///
/// * `address` — the contract address to check.
/// * `_chain_id` — the [`SupportedChainId`] (currently unused since
///   vault relayer addresses are the same across chains).
///
/// # Returns
///
/// `true` if `address` matches either vault relayer address.
///
/// ```
/// use cow_rs::{SupportedChainId, common::address::is_cow_vault_relayer_contract};
///
/// assert!(is_cow_vault_relayer_contract(
///     "0xC92E8bdf79f0507f65a392b0ab4667716BFE0110",
///     SupportedChainId::Mainnet,
/// ));
/// ```
#[must_use]
pub fn is_cow_vault_relayer_contract(address: &str, _chain_id: SupportedChainId) -> bool {
    let key = get_address_key(address);
    let prod = format!("{:#x}", VAULT_RELAYER);
    let staging = format!("{:#x}", VAULT_RELAYER_STAGING);
    are_addresses_equal(Some(key.as_str()), Some(&prod)) ||
        are_addresses_equal(Some(key.as_str()), Some(&staging))
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Check if a byte is in the Base58 alphabet (excludes `0`, `O`, `I`, `l`).
const fn is_base58_char(b: u8) -> bool {
    matches!(b,
        b'1'..=b'9'
        | b'A'..=b'H'
        | b'J'..=b'N'
        | b'P'..=b'Z'
        | b'a'..=b'k'
        | b'm'..=b'z'
    )
}

/// Check if a BTC address is a legacy P2PKH or P2SH address.
fn is_btc_legacy(address: &str) -> bool {
    let bytes = address.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    // Must start with '1' or '3'.
    if bytes[0] != b'1' && bytes[0] != b'3' {
        return false;
    }
    // Remaining characters (indices 1..) must be base58, total length 25-34.
    let len = bytes.len();
    if len < 25 || len > 34 {
        return false;
    }
    bytes[1..].iter().all(|&b| is_base58_char(b))
}

/// Check if a BTC address is a Bech32 mainnet address.
fn is_btc_bech32_mainnet(address: &str) -> bool {
    let len = address.len();
    // bc1 + 39..=59 chars -> total 42..=62
    if len < 42 || len > 62 {
        return false;
    }
    if let Some(rest) = address.strip_prefix("bc1") {
        // All lowercase alphanumeric.
        rest.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    } else if let Some(rest) = address.strip_prefix("BC1") {
        // All uppercase alphanumeric.
        rest.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    } else {
        false
    }
}

// ── Analyzer-friendly aliases ────────────────────────────────────────────────
//
// The TypeScript names `isCoWSettlementContract` and `isCoWVaultRelayerContract`
// are naively snake-cased to `is_co_w_settlement_contract` / `is_co_w_vault_relayer_contract`
// by some analyzers. Provide these aliases so such tools can locate the items.

/// Alias for [`is_cow_settlement_contract`] — provided for analyzer compatibility.
///
/// Some automated tools convert `CoW` to `co_w` when snake-casing TypeScript names.
pub use is_cow_settlement_contract as is_co_w_settlement_contract;

/// Alias for [`is_cow_vault_relayer_contract`] — provided for analyzer compatibility.
///
/// Some automated tools convert `CoW` to `co_w` when snake-casing TypeScript names.
pub use is_cow_vault_relayer_contract as is_co_w_vault_relayer_contract;

#[cfg(test)]
mod tests {
    use super::*;

    // ── EVM ──────────────────────────────────────────────────────────────

    #[test]
    fn evm_valid() {
        assert!(is_evm_address("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
        assert!(is_evm_address("0x0000000000000000000000000000000000000000"));
    }

    #[test]
    fn evm_invalid() {
        assert!(!is_evm_address(""));
        assert!(!is_evm_address("0x"));
        assert!(!is_evm_address("0xZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"));
        assert!(!is_evm_address("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
        // Too short / too long.
        assert!(!is_evm_address("0xabcd"));
        assert!(!is_evm_address("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2a"));
    }

    // ── BTC ──────────────────────────────────────────────────────────────

    #[test]
    fn btc_legacy_valid() {
        assert!(is_btc_address("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"));
        assert!(is_btc_address("3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy"));
    }

    #[test]
    fn btc_bech32_valid() {
        assert!(is_btc_address("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"));
    }

    #[test]
    fn btc_invalid() {
        assert!(!is_btc_address(""));
        assert!(!is_btc_address("not_a_btc_address"));
    }

    // ── Solana ───────────────────────────────────────────────────────────

    #[test]
    fn sol_valid() {
        assert!(is_solana_address("11111111111111111111111111111111"));
        assert!(is_solana_address("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"));
    }

    #[test]
    fn sol_invalid() {
        assert!(!is_solana_address("short"));
        // Contains 'O' which is not in base58.
        assert!(!is_solana_address("OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO"));
    }

    // ── Address comparison ───────────────────────────────────────────────

    #[test]
    fn evm_case_insensitive_equal() {
        assert!(are_addresses_equal(
            Some("0xABCDEF1234567890abcdef1234567890ABCDEF12"),
            Some("0xabcdef1234567890abcdef1234567890abcdef12"),
        ));
    }

    #[test]
    fn none_never_equal() {
        assert!(!are_addresses_equal(None, Some("0x1234")));
        assert!(!are_addresses_equal(Some("0x1234"), None));
        assert!(!are_addresses_equal(None, None));
    }

    // ── Token ID ─────────────────────────────────────────────────────────

    #[test]
    fn token_id_normalizes_evm() {
        let id = get_token_id(1, "0xABCDEF1234567890abcdef1234567890ABCDEF12");
        assert_eq!(id.as_str(), "1:0xabcdef1234567890abcdef1234567890abcdef12");
    }

    // ── Native / wrapped ─────────────────────────────────────────────────

    #[test]
    fn native_token_detected() {
        assert!(is_native_token(1, "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"));
    }

    #[test]
    fn wrapped_native_token_detected() {
        // WETH on mainnet.
        assert!(is_wrapped_native_token(1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
    }

    // ── Settlement / vault relayer ───────────────────────────────────────

    #[test]
    fn settlement_contract_detected() {
        assert!(is_cow_settlement_contract(
            "0x9008D19f58AAbD9eD0D60971565AA8510560ab41",
            SupportedChainId::Mainnet,
        ));
        // Staging address.
        assert!(is_cow_settlement_contract(
            "0xf553d092b50bdcbddeD1A99aF2cA29FBE5E2CB13",
            SupportedChainId::Mainnet,
        ));
    }

    #[test]
    fn vault_relayer_detected() {
        assert!(is_cow_vault_relayer_contract(
            "0xC92E8bdf79f0507f65a392b0ab4667716BFE0110",
            SupportedChainId::Mainnet,
        ));
    }
}
