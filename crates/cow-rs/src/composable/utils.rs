//! Utility helpers for composable (conditional) orders.
//!
//! Mirrors `utils.ts` from the `@cowprotocol/composable` `TypeScript` SDK:
//! address checks, EIP-712 hash-to-string reversal, ABI validation, and
//! conversion from raw on-chain structs to typed orders.

use alloy_primitives::{Address, B256, U256, keccak256};

use super::types::{
    BlockInfo, COMPOSABLE_COW_ADDRESS, ConditionalOrderParams, GpV2OrderStruct, IsValidResult,
};
use crate::{
    config::contracts::EXTENSIBLE_FALLBACK_HANDLER,
    error::CowError,
    order_signing::types::UnsignedOrder,
    types::{OrderKind, TokenBalance},
};

/// Returns `true` if `address` is the canonical `ComposableCow` factory address.
///
/// Mirrors `isComposableCow` from the `TypeScript` SDK.
///
/// # Example
///
/// ```rust
/// use cow_rs::composable::{COMPOSABLE_COW_ADDRESS, is_composable_cow};
///
/// assert!(is_composable_cow(COMPOSABLE_COW_ADDRESS));
/// assert!(!is_composable_cow(alloy_primitives::Address::ZERO));
/// ```
#[must_use]
pub fn is_composable_cow(address: Address) -> bool {
    address == COMPOSABLE_COW_ADDRESS
}

/// Returns `true` if `address` is the canonical `ExtensibleFallbackHandler` contract.
///
/// Used to verify whether a `Safe` wallet has the correct fallback handler installed
/// to support `ComposableCow`-based conditional orders via EIP-712 domain verifiers.
///
/// # Example
///
/// ```rust
/// use cow_rs::{EXTENSIBLE_FALLBACK_HANDLER, composable::is_extensible_fallback_handler};
///
/// assert!(is_extensible_fallback_handler(EXTENSIBLE_FALLBACK_HANDLER));
/// assert!(!is_extensible_fallback_handler(alloy_primitives::Address::ZERO));
/// ```
#[must_use]
pub fn is_extensible_fallback_handler(address: Address) -> bool {
    address == EXTENSIBLE_FALLBACK_HANDLER
}

/// Reverse-map a `keccak256`-hashed token-balance string back to its name.
///
/// Returns `Some("erc20")`, `Some("external")`, or `Some("internal")` if `hash`
/// matches a known [`TokenBalance`] EIP-712 hash; `None` otherwise.
///
/// Mirrors `balanceToString` from the `TypeScript` SDK.
///
/// # Example
///
/// ```rust
/// use cow_rs::{TokenBalance, composable::balance_to_string};
///
/// let h = TokenBalance::Erc20.eip712_hash();
/// assert_eq!(balance_to_string(h), Some("erc20"));
/// assert_eq!(balance_to_string(alloy_primitives::B256::ZERO), None);
/// ```
#[must_use]
pub fn balance_to_string(hash: B256) -> Option<&'static str> {
    if hash == TokenBalance::Erc20.eip712_hash() {
        Some("erc20")
    } else if hash == TokenBalance::External.eip712_hash() {
        Some("external")
    } else if hash == TokenBalance::Internal.eip712_hash() {
        Some("internal")
    } else {
        None
    }
}

/// Reverse-map a `keccak256`-hashed order-kind string back to its name.
///
/// Returns `Some("sell")` or `Some("buy")` if `hash` matches a known
/// [`OrderKind`] EIP-712 hash; `None` otherwise.
///
/// Mirrors `kindToString` from the `TypeScript` SDK.
///
/// # Example
///
/// ```rust
/// use cow_rs::composable::kind_to_string;
///
/// let sell_hash = alloy_primitives::keccak256(b"sell");
/// assert_eq!(kind_to_string(sell_hash), Some("sell"));
///
/// let buy_hash = alloy_primitives::keccak256(b"buy");
/// assert_eq!(kind_to_string(buy_hash), Some("buy"));
///
/// assert_eq!(kind_to_string(alloy_primitives::B256::ZERO), None);
/// ```
#[must_use]
pub fn kind_to_string(hash: B256) -> Option<&'static str> {
    if hash == keccak256(b"sell" as &[u8]) {
        Some("sell")
    } else if hash == keccak256(b"buy" as &[u8]) {
        Some("buy")
    } else {
        None
    }
}

/// Decode a raw on-chain [`GpV2OrderStruct`] into a typed [`UnsignedOrder`].
///
/// The `kind`, `sell_token_balance`, and `buy_token_balance` fields in
/// [`GpV2OrderStruct`] are `keccak256` hashes; this function reverses them
/// via [`kind_to_string`] and [`balance_to_string`].
///
/// Mirrors `fromStructToOrder` from the `@cowprotocol/composable` SDK.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if any hash cannot be decoded to a known variant.
///
/// # Example
///
/// ```
/// use alloy_primitives::{Address, B256, U256, keccak256};
/// use cow_rs::{
///     OrderKind,
///     composable::{GpV2OrderStruct, from_struct_to_order},
/// };
///
/// let s = GpV2OrderStruct {
///     sell_token: Address::ZERO,
///     buy_token: Address::ZERO,
///     receiver: Address::ZERO,
///     sell_amount: U256::ZERO,
///     buy_amount: U256::ZERO,
///     valid_to: 0,
///     app_data: B256::ZERO,
///     fee_amount: U256::ZERO,
///     kind: keccak256(b"sell"),
///     partially_fillable: false,
///     sell_token_balance: keccak256(b"erc20"),
///     buy_token_balance: keccak256(b"erc20"),
/// };
/// let order = from_struct_to_order(&s).unwrap();
/// assert_eq!(order.kind, OrderKind::Sell);
/// ```
pub fn from_struct_to_order(s: &GpV2OrderStruct) -> Result<UnsignedOrder, CowError> {
    let kind_str = kind_to_string(s.kind)
        .ok_or_else(|| CowError::AppData(format!("unknown order kind hash: {}", s.kind)))?;
    let kind = if kind_str == "sell" { OrderKind::Sell } else { OrderKind::Buy };
    let sell_token_balance = decode_token_balance(s.sell_token_balance)?;
    let buy_token_balance = decode_token_balance(s.buy_token_balance)?;
    Ok(UnsignedOrder {
        sell_token: s.sell_token,
        buy_token: s.buy_token,
        receiver: s.receiver,
        sell_amount: s.sell_amount,
        buy_amount: s.buy_amount,
        valid_to: s.valid_to,
        app_data: s.app_data,
        fee_amount: s.fee_amount,
        kind,
        partially_fillable: s.partially_fillable,
        sell_token_balance,
        buy_token_balance,
    })
}

/// Decode a `keccak256`-hashed token-balance string to a [`TokenBalance`] variant.
fn decode_token_balance(hash: B256) -> Result<TokenBalance, CowError> {
    match balance_to_string(hash) {
        Some("erc20") => Ok(TokenBalance::Erc20),
        Some("external") => Ok(TokenBalance::External),
        Some("internal") => Ok(TokenBalance::Internal),
        _ => Err(CowError::AppData(format!("unknown token-balance hash: {hash}"))),
    }
}

/// Default token formatter: produces `"{amount}@{address}"`.
///
/// Mirrors `DEFAULT_TOKEN_FORMATTER` from the `TypeScript` SDK.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, U256};
/// use cow_rs::composable::default_token_formatter;
///
/// let s = default_token_formatter(Address::ZERO, U256::from(42u64));
/// assert_eq!(s, "42@0x0000000000000000000000000000000000000000");
/// ```
#[must_use]
pub fn default_token_formatter(address: Address, amount: U256) -> String {
    format!("{amount}@{address}")
}

/// Check whether an [`IsValidResult`] represents a valid state.
///
/// Returns `true` if the result is `Valid`, `false` if `Invalid`.
///
/// This is the Rust equivalent of `getIsValidResult` in the `TypeScript` SDK.
///
/// # Example
///
/// ```rust
/// use cow_rs::composable::{IsValidResult, get_is_valid_result};
///
/// let valid = IsValidResult::Valid;
/// assert!(get_is_valid_result(&valid));
///
/// let invalid = IsValidResult::Invalid { reason: "expired".to_owned() };
/// assert!(!get_is_valid_result(&invalid));
/// ```
#[must_use]
pub const fn get_is_valid_result(result: &IsValidResult) -> bool {
    matches!(result, IsValidResult::Valid)
}

/// Transform raw contract data bytes into a [`ConditionalOrderParams`] struct.
///
/// Decodes the ABI-encoded bytes (handler + salt + staticInput) into the
/// structured [`ConditionalOrderParams`] type.
///
/// This is the Rust equivalent of `transformDataToStruct` in the `TypeScript` SDK.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if the data is too short or malformed.
pub fn transform_data_to_struct(data: &[u8]) -> Result<ConditionalOrderParams, CowError> {
    // ABI layout: handler(32) + salt(32) + offset(32) + length(32) + data...
    if data.len() < 128 {
        return Err(CowError::AppData("data too short for ConditionalOrderParams".into()));
    }
    let handler = Address::from_slice(&data[12..32]);
    let salt = B256::from_slice(&data[32..64]);
    let data_offset = usize::try_from(U256::from_be_slice(&data[64..96]))
        .map_err(|e| CowError::AppData(format!("invalid offset: {e}")))?;
    let data_len = usize::try_from(U256::from_be_slice(&data[data_offset..data_offset + 32]))
        .map_err(|e| CowError::AppData(format!("invalid data length: {e}")))?;
    let static_input_start = data_offset + 32;
    let static_input = data[static_input_start..static_input_start + data_len].to_vec();
    Ok(ConditionalOrderParams { handler, salt, static_input })
}

/// Transform a [`ConditionalOrderParams`] struct back into ABI-encoded hex.
///
/// Produces the same `0x`-prefixed hex encoding that the `ComposableCow` contract expects.
///
/// This is the Rust equivalent of `transformStructToData` in the `TypeScript` SDK.
#[must_use]
pub fn transform_struct_to_data(params: &ConditionalOrderParams) -> String {
    super::twap::encode_params(params)
}

/// Encode a `setDomainVerifier(bytes32 domain, address verifier)` call.
///
/// Returns the ABI-encoded calldata for calling `setDomainVerifier` on the
/// `ExtensibleFallbackHandler` contract.
///
/// This is the Rust equivalent of `createSetDomainVerifierTx` in the `TypeScript` SDK.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, B256};
/// use cow_rs::composable::create_set_domain_verifier_tx;
///
/// let domain = B256::ZERO;
/// let verifier = Address::ZERO;
/// let calldata = create_set_domain_verifier_tx(domain, verifier);
/// assert!(!calldata.is_empty());
/// ```
#[must_use]
pub fn create_set_domain_verifier_tx(domain: B256, verifier: Address) -> Vec<u8> {
    // function setDomainVerifier(bytes32, address)
    // selector = keccak256("setDomainVerifier(bytes32,address)")[..4]
    let selector = &keccak256(b"setDomainVerifier(bytes32,address)" as &[u8])[..4];
    let mut calldata = Vec::with_capacity(4 + 64);
    calldata.extend_from_slice(selector);
    calldata.extend_from_slice(domain.as_slice());
    // Address is left-padded to 32 bytes
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(verifier.as_slice());
    calldata
}

/// Get block information (number and timestamp) for constructing conditional orders.
///
/// This is the Rust equivalent of `getBlockInfo` in the `TypeScript` SDK.
/// In the `TypeScript` SDK this makes an RPC call; here it is a simple constructor.
///
/// # Example
///
/// ```rust
/// use cow_rs::composable::get_block_info;
///
/// let info = get_block_info(12345, 1_700_000_000);
/// assert_eq!(info.block_number, 12345);
/// assert_eq!(info.block_timestamp, 1_700_000_000);
/// ```
#[must_use]
pub const fn get_block_info(block_number: u64, block_timestamp: u64) -> BlockInfo {
    BlockInfo { block_number, block_timestamp }
}

/// Get the domain verifier address for a Safe from the `ExtensibleFallbackHandler`.
///
/// Returns the ABI-encoded calldata for the `domainVerifiers(address,bytes32)` view call.
/// In the `TypeScript` SDK this makes an on-chain read; this Rust version returns
/// the calldata so callers can execute the call via their preferred provider.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, B256};
/// use cow_rs::composable::get_domain_verifier_calldata;
///
/// let safe = Address::ZERO;
/// let domain = B256::ZERO;
/// let calldata = get_domain_verifier_calldata(safe, domain);
/// assert_eq!(calldata.len(), 4 + 64);
/// ```
#[must_use]
pub fn get_domain_verifier_calldata(safe: Address, domain: B256) -> Vec<u8> {
    // function domainVerifiers(address, bytes32) view returns (address)
    let selector = &keccak256(b"domainVerifiers(address,bytes32)" as &[u8])[..4];
    let mut calldata = Vec::with_capacity(4 + 64);
    calldata.extend_from_slice(selector);
    // Address is left-padded to 32 bytes
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(safe.as_slice());
    calldata.extend_from_slice(domain.as_slice());
    calldata
}

/// Alias for [`get_domain_verifier_calldata`].
///
/// Matches the `getDomainVerifier` name from the `TypeScript` `composable` package.
/// Returns ABI-encoded calldata for the `domainVerifiers(address,bytes32)` view call
/// on the `ExtensibleFallbackHandler` contract.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, B256};
/// use cow_rs::composable::get_domain_verifier;
///
/// let calldata = get_domain_verifier(Address::ZERO, B256::ZERO);
/// assert_eq!(calldata.len(), 4 + 64);
/// ```
#[must_use]
pub fn get_domain_verifier(safe: Address, domain: B256) -> Vec<u8> {
    get_domain_verifier_calldata(safe, domain)
}

/// Returns `true` if `hex` is a plausibly valid ABI-encoded
/// [`ConditionalOrderParams`].
///
/// Checks that the hex decodes to at least 128 bytes (handler word + salt +
/// offset word + length word) and that the declared `static_input` length fits
/// within the buffer. This does **not** attempt to decode or validate the
/// handler address or static input contents.
///
/// Mirrors `isValidAbi` from the `TypeScript` SDK.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, B256};
/// use cow_rs::composable::{ConditionalOrderParams, encode_params, is_valid_abi};
///
/// let params =
///     ConditionalOrderParams { handler: Address::ZERO, salt: B256::ZERO, static_input: vec![] };
/// assert!(is_valid_abi(&encode_params(&params)));
/// assert!(!is_valid_abi("0xdeadbeef")); // too short
/// ```
#[must_use]
pub fn is_valid_abi(hex: &str) -> bool {
    let stripped = hex.trim_start_matches("0x");
    let Ok(bytes) = alloy_primitives::hex::decode(stripped) else {
        return false;
    };
    // Minimum: handler(32) + salt(32) + offset(32) + length(32) = 128 bytes
    if bytes.len() < 128 {
        return false;
    }
    let data_len = usize::try_from(U256::from_be_slice(&bytes[96..128]));
    let data_len_usize = data_len.map_or(usize::MAX, |v| v);
    let min_total = 128usize.saturating_add(data_len_usize);
    bytes.len() >= min_total
}
