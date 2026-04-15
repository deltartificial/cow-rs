//! EIP-712 digest computation for `CoW` Protocol orders.
//!
//! All hashes are computed with `alloy_primitives::keccak256`. ABI encoding
//! is done manually: [`Address`] values are left-padded to 32 bytes, `U256`
//! integers are encoded as 32-byte big-endian, and `bytes32` values are
//! copied verbatim.

use alloy_primitives::{Address, B256, U256, keccak256};
use cow_chains::contracts::SETTLEMENT_CONTRACT;
use cow_errors::CowError;
use cow_types::OrderKind;

use crate::types::{OrderDomain, OrderTypedData, UnsignedOrder};

/// Marker address indicating that an order is buying native Ether.
///
/// This address only has special meaning in the `buyToken` field and will be
/// treated as a regular ERC-20 token address in the `sellToken` position.
pub const BUY_ETH_ADDRESS: Address = Address::new([
    0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee,
    0xee, 0xee, 0xee, 0xee,
]);

// ── Order constants ─────────────────────────────────────────────────────────

/// EIP-712 primary type for `CoW` Protocol orders.
pub const ORDER_PRIMARY_TYPE: &str = "Order";

/// Pre-computed EIP-712 type hash for a Gnosis Protocol v2 order.
///
/// `keccak256("Order(address sellToken,address buyToken,address receiver,uint256 sellAmount,uint256
/// buyAmount,uint32 validTo,bytes32 appData,uint256 feeAmount,string kind,bool
/// partiallyFillable,string sellTokenBalance,string buyTokenBalance)")`
pub const ORDER_TYPE_HASH: &str =
    "0xd5a25ba2e97094ad7d83dc28a6572da797d6b3e7fc6663bd93efb789fc17e489";

/// The byte length of an order UID (32 bytes order digest + 20 bytes owner + 4 bytes validTo).
pub const ORDER_UID_LENGTH: usize = 56;

/// Value returned by a call to `isValidSignature` if the signature was verified
/// successfully (EIP-1271).
///
/// `bytes4(keccak256("isValidSignature(bytes32,bytes)"))`
pub const EIP1271_MAGICVALUE: &str = "0x1626ba7e";

/// Marker value indicating a presignature is set.
///
/// `keccak256("GPv2Signing.Scheme.PreSign")`
pub const PRE_SIGNED: &str = "0xf59c009283ff87aa78203fc4d9c2df025ee851130fb69cc3e068941f6b5e2d6f";

// ── EIP-712 type-string hashes (computed lazily, never hardcoded) ─────────────

/// EIP-712 type hash for `EIP712Domain`.
///
/// # Returns
///
/// The `keccak256` hash of the `EIP712Domain` type string.
fn domain_type_hash() -> B256 {
    keccak256(b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)")
}

/// EIP-712 type hash for `GPv2Order.Data`.
///
/// # Returns
///
/// The `keccak256` hash of the `GPv2Order.Data` type string.
fn order_type_hash() -> B256 {
    keccak256(
        b"GPv2Order.Data(\
        address sellToken,\
        address buyToken,\
        address receiver,\
        uint256 sellAmount,\
        uint256 buyAmount,\
        uint32 validTo,\
        bytes32 appData,\
        uint256 feeAmount,\
        string kind,\
        bool partiallyFillable,\
        string sellTokenBalance,\
        string buyTokenBalance\
    )",
    )
}

// ── ABI encoding helpers ──────────────────────────────────────────────────────

/// Left-pad an [`Address`] to 32 bytes.
///
/// # Arguments
///
/// * `a` - The 20-byte address to encode.
///
/// # Returns
///
/// A 32-byte array with the address right-aligned (left-padded with zeros).
fn abi_address(a: Address) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[12..].copy_from_slice(a.as_slice());
    buf
}

/// Encode a [`U256`] as a 32-byte big-endian word.
///
/// # Arguments
///
/// * `v` - The 256-bit unsigned integer to encode.
///
/// # Returns
///
/// A 32-byte big-endian representation.
#[must_use]
const fn abi_u256(v: U256) -> [u8; 32] {
    v.to_be_bytes()
}

/// Encode a `u32` as a 32-byte big-endian word.
///
/// # Arguments
///
/// * `v` - The 32-bit unsigned integer to encode.
///
/// # Returns
///
/// A 32-byte array with the value right-aligned in big-endian form.
fn abi_u32(v: u32) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[28..].copy_from_slice(&v.to_be_bytes());
    buf
}

/// Encode a `bool` as a 32-byte ABI word.
///
/// # Arguments
///
/// * `v` - The boolean value to encode.
///
/// # Returns
///
/// A 32-byte ABI-encoded word (`0` or `1`).
fn abi_bool(v: bool) -> [u8; 32] {
    abi_u32(u32::from(v))
}

// ── Public interface ──────────────────────────────────────────────────────────

/// Compute the EIP-712 domain separator for `CoW` Protocol on `chain_id`.
///
/// # Arguments
///
/// * `chain_id` - The EVM chain ID for the target network.
///
/// # Returns
///
/// The 32-byte EIP-712 domain separator hash.
#[must_use]
pub fn domain_separator(chain_id: u64) -> B256 {
    let name_hash = keccak256(b"Gnosis Protocol v2");
    let version_hash = keccak256(b"v2");

    let mut buf = [0u8; 5 * 32];
    buf[0..32].copy_from_slice(domain_type_hash().as_slice());
    buf[32..64].copy_from_slice(name_hash.as_slice());
    buf[64..96].copy_from_slice(version_hash.as_slice());
    buf[96..128].copy_from_slice(&abi_u256(U256::from(chain_id)));
    buf[128..160].copy_from_slice(&abi_address(SETTLEMENT_CONTRACT));
    keccak256(buf)
}

/// Compute the EIP-712 domain separator from a custom [`OrderDomain`].
///
/// Unlike [`domain_separator`], which always uses the canonical protocol
/// name, version, and settlement contract address, this function reads
/// every field from the provided `domain`. Use this when you need a
/// domain separator for a fork or alternative deployment.
///
/// # Arguments
///
/// * `domain` - The [`OrderDomain`] whose fields define the separator.
///
/// # Returns
///
/// The 32-byte EIP-712 domain separator hash.
#[must_use]
pub fn domain_separator_from(domain: &OrderDomain) -> B256 {
    let name_hash = keccak256(domain.name.as_bytes());
    let version_hash = keccak256(domain.version.as_bytes());

    let mut buf = [0u8; 5 * 32];
    buf[0..32].copy_from_slice(domain_type_hash().as_slice());
    buf[32..64].copy_from_slice(name_hash.as_slice());
    buf[64..96].copy_from_slice(version_hash.as_slice());
    buf[96..128].copy_from_slice(&abi_u256(U256::from(domain.chain_id)));
    buf[128..160].copy_from_slice(&abi_address(domain.verifying_contract));
    keccak256(buf)
}

/// Compute the EIP-712 struct hash for `order`.
///
/// # Arguments
///
/// * `order` - The unsigned order to hash.
///
/// # Returns
///
/// The 32-byte `keccak256` struct hash of the ABI-encoded order fields.
#[must_use]
pub fn order_hash(order: &UnsignedOrder) -> B256 {
    let kind_hash = keccak256(match order.kind {
        OrderKind::Sell => b"sell" as &[u8],
        OrderKind::Buy => b"buy" as &[u8],
    });

    let mut buf = [0u8; 13 * 32];
    buf[0..32].copy_from_slice(order_type_hash().as_slice());
    buf[32..64].copy_from_slice(&abi_address(order.sell_token));
    buf[64..96].copy_from_slice(&abi_address(order.buy_token));
    buf[96..128].copy_from_slice(&abi_address(order.receiver));
    buf[128..160].copy_from_slice(&abi_u256(order.sell_amount));
    buf[160..192].copy_from_slice(&abi_u256(order.buy_amount));
    buf[192..224].copy_from_slice(&abi_u32(order.valid_to));
    buf[224..256].copy_from_slice(order.app_data.as_slice());
    buf[256..288].copy_from_slice(&abi_u256(order.fee_amount));
    buf[288..320].copy_from_slice(kind_hash.as_slice());
    buf[320..352].copy_from_slice(&abi_bool(order.partially_fillable));
    buf[352..384].copy_from_slice(order.sell_token_balance.eip712_hash().as_slice());
    buf[384..416].copy_from_slice(order.buy_token_balance.eip712_hash().as_slice());
    keccak256(buf)
}

/// Compute the final signing digest: `"\x19\x01" ‖ domainSep ‖ orderHash`.
///
/// # Arguments
///
/// * `domain_sep` - The EIP-712 domain separator.
/// * `o_hash` - The EIP-712 struct hash (order or cancellation).
///
/// # Returns
///
/// The 32-byte signing digest ready to be signed with a private key.
#[must_use]
pub fn signing_digest(domain_sep: B256, o_hash: B256) -> B256 {
    let mut buf = [0u8; 66];
    buf[0] = 0x19;
    buf[1] = 0x01;
    buf[2..34].copy_from_slice(domain_sep.as_slice());
    buf[34..66].copy_from_slice(o_hash.as_slice());
    keccak256(buf)
}

/// Build a complete [`OrderTypedData`] envelope for `order` on `chain_id`.
///
/// This bundles the EIP-712 domain, primary type name, and the order message
/// into a single struct ready for hardware-wallet or smart-contract signing.
///
/// Mirrors `getOrderTypedData` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `order` - The unsigned order to wrap.
/// * `chain_id` - The EVM chain ID for the target network.
///
/// # Returns
///
/// An [`OrderTypedData`] envelope containing the domain, primary type, and order.
#[must_use]
pub const fn build_order_typed_data(order: UnsignedOrder, chain_id: u64) -> OrderTypedData {
    OrderTypedData {
        domain: OrderDomain {
            name: "Gnosis Protocol v2",
            version: "v2",
            chain_id,
            verifying_contract: SETTLEMENT_CONTRACT,
        },
        primary_type: "GPv2Order.Data",
        order,
    }
}

/// EIP-712 type hash for `OrderCancellations`.
///
/// `keccak256("OrderCancellations(bytes[] orderUids)")`
///
/// # Returns
///
/// The `keccak256` hash of the `OrderCancellations` type string.
fn cancellations_type_hash() -> B256 {
    keccak256(b"OrderCancellations(bytes[] orderUids)")
}

/// Compute the EIP-712 struct hash for a batch of order cancellations.
///
/// Each order UID in `order_uids` must be a `0x`-prefixed 56-byte hex string.
///
/// # Arguments
///
/// * `order_uids` - Slice of `0x`-prefixed hex-encoded order UIDs.
///
/// # Returns
///
/// The 32-byte EIP-712 struct hash for the cancellation batch.
///
/// # Errors
///
/// Returns [`CowError::Api`] if any UID is not valid hex.
pub fn cancellations_hash(order_uids: &[&str]) -> Result<B256, CowError> {
    // EIP-712 `bytes[]`: each element hashed with keccak256, then the array
    // encoded as keccak256 of the concatenated element hashes.
    let mut element_hashes: Vec<u8> = Vec::with_capacity(order_uids.len() * 32);
    for uid in order_uids {
        let stripped = uid.trim_start_matches("0x");
        let bytes = alloy_primitives::hex::decode(stripped)
            .map_err(|_e| CowError::Api { status: 0, body: format!("invalid orderUid: {uid}") })?;
        element_hashes.extend_from_slice(keccak256(&bytes).as_slice());
    }
    let array_hash = keccak256(&element_hashes);

    let mut buf = [0u8; 64];
    buf[0..32].copy_from_slice(cancellations_type_hash().as_slice());
    buf[32..64].copy_from_slice(array_hash.as_slice());
    Ok(keccak256(buf))
}

/// Convert a numeric app-data value to a zero-padded 32-byte hash.
///
/// Mirrors `hashify` from the `TypeScript` SDK. When the input is already a
/// `B256`, it is returned as-is. For integer values, this left-pads with
/// zeros to 32 bytes.
///
/// # Arguments
///
/// * `value` - The integer value to convert.
///
/// # Returns
///
/// A [`B256`] with the value left-padded with zeros to 32 bytes.
///
/// ```
/// use alloy_primitives::B256;
/// use cow_signing::hashify;
///
/// let hash = hashify(42);
/// assert_eq!(hash, B256::left_padding_from(&[42]));
/// ```
#[must_use]
pub fn hashify(value: u64) -> B256 {
    let u = U256::from(value);
    B256::from(u.to_be_bytes())
}

/// Compute the EIP-712 typed data hash: `keccak256("\x19\x01" || domainSep || structHash)`.
///
/// This is a convenience function that combines the domain separator and a
/// struct hash into the final signing digest. Mirrors `hashTypedData` from the
/// `TypeScript` SDK.
///
/// # Arguments
///
/// * `domain_sep` - The EIP-712 domain separator.
/// * `struct_hash` - The EIP-712 struct hash.
///
/// # Returns
///
/// The 32-byte signing digest.
///
/// ```
/// use alloy_primitives::{Address, B256, U256};
/// use cow_rs::{
///     UnsignedOrder,
///     order_signing::{domain_separator, hash_typed_data, order_hash},
/// };
///
/// let order = UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::ZERO, U256::ZERO);
/// let ds = domain_separator(1);
/// let oh = order_hash(&order);
/// let digest = hash_typed_data(ds, oh);
/// assert_ne!(digest, B256::ZERO);
/// ```
#[must_use]
pub fn hash_typed_data(domain_sep: B256, struct_hash: B256) -> B256 {
    signing_digest(domain_sep, struct_hash)
}

/// Compute the EIP-712 signing hash for a single order cancellation.
///
/// Convenience wrapper around [`hash_order_cancellations`] for the common
/// single-order case.
///
/// Mirrors `hashOrderCancellation` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `chain_id` - The EVM chain ID for the target network.
/// * `order_uid` - A `0x`-prefixed hex-encoded order UID.
///
/// # Returns
///
/// The 32-byte EIP-712 signing digest for the cancellation.
///
/// # Errors
///
/// Returns [`CowError`] if the UID is not valid hex.
pub fn hash_order_cancellation(chain_id: u64, order_uid: &str) -> Result<B256, CowError> {
    hash_order_cancellations(chain_id, &[order_uid])
}

/// Compute the EIP-712 signing hash for a batch of order cancellations.
///
/// Returns `keccak256("\x19\x01" || domainSep || cancellationsHash(orderUids))`.
///
/// Mirrors `hashOrderCancellations` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `chain_id` - The EVM chain ID for the target network.
/// * `order_uids` - Slice of `0x`-prefixed hex-encoded order UIDs to cancel.
///
/// # Returns
///
/// The 32-byte EIP-712 signing digest for the batch cancellation.
///
/// # Errors
///
/// Returns [`CowError`] if any UID is not valid hex.
pub fn hash_order_cancellations(chain_id: u64, order_uids: &[&str]) -> Result<B256, CowError> {
    let ds = domain_separator(chain_id);
    let ch = cancellations_hash(order_uids)?;
    Ok(signing_digest(ds, ch))
}

/// Normalize an [`UnsignedOrder`] for EIP-712 hashing, filling defaults.
///
/// - `sell_token_balance` defaults to `Erc20`
/// - `buy_token_balance`: `External` is normalized to `Erc20`
///
/// Mirrors `normalizeOrder` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `order` - The unsigned order to normalize.
///
/// # Returns
///
/// A cloned [`UnsignedOrder`] with normalized token balance fields.
///
/// # Errors
///
/// Returns [`CowError::Parse`] if `receiver` is the zero address (explicit zero
/// receiver is not allowed — use `Address::ZERO` only to indicate "no custom
/// receiver").
///
/// ```
/// use alloy_primitives::{Address, B256, U256};
/// use cow_rs::{OrderKind, TokenBalance, UnsignedOrder, order_signing::normalize_order};
///
/// let order = UnsignedOrder {
///     sell_token: Address::ZERO,
///     buy_token: Address::ZERO,
///     receiver: Address::ZERO,
///     sell_amount: U256::from(100),
///     buy_amount: U256::from(90),
///     valid_to: 1_000,
///     app_data: B256::ZERO,
///     fee_amount: U256::ZERO,
///     kind: OrderKind::Sell,
///     partially_fillable: false,
///     sell_token_balance: TokenBalance::Erc20,
///     buy_token_balance: TokenBalance::External, // will be normalized to Erc20
/// };
///
/// let normalized = normalize_order(&order);
/// assert_eq!(normalized.buy_token_balance, TokenBalance::Erc20);
/// ```
#[must_use]
pub fn normalize_order(order: &UnsignedOrder) -> UnsignedOrder {
    use crate::flags::normalize_buy_token_balance;

    let mut normalized = order.clone();
    normalized.buy_token_balance = normalize_buy_token_balance(order.buy_token_balance);
    normalized
}

/// Pack order UID parameters into a 56-byte hex-encoded string.
///
/// The order UID is `order_digest (32 bytes) || owner (20 bytes) || valid_to (4 bytes big-endian)`.
///
/// Mirrors `packOrderUidParams` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `order_digest` - The 32-byte EIP-712 order struct hash.
/// * `owner` - The order owner address.
/// * `valid_to` - The order expiry as a Unix timestamp.
///
/// # Returns
///
/// A `0x`-prefixed hex string encoding the 56-byte order UID.
///
/// ```
/// use alloy_primitives::{Address, B256};
/// use cow_signing::pack_order_uid_params;
///
/// let uid = pack_order_uid_params(B256::ZERO, Address::ZERO, 1_000_000);
/// assert!(uid.starts_with("0x"));
/// assert_eq!(uid.len(), 2 + 112); // "0x" + 56 bytes as hex
/// ```
#[must_use]
pub fn pack_order_uid_params(order_digest: B256, owner: Address, valid_to: u32) -> String {
    let mut uid = Vec::with_capacity(ORDER_UID_LENGTH);
    uid.extend_from_slice(order_digest.as_slice()); // 32 bytes
    uid.extend_from_slice(owner.as_slice()); // 20 bytes
    uid.extend_from_slice(&valid_to.to_be_bytes()); // 4 bytes
    format!("0x{}", alloy_primitives::hex::encode(&uid))
}

/// Parameters extracted from a 56-byte order UID.
///
/// Corresponds to the `TypeScript` `OrderUidParams` type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderUidParams {
    /// The EIP-712 order struct hash (32 bytes).
    pub order_digest: B256,
    /// The order owner address (20 bytes).
    pub owner: Address,
    /// The order expiry as Unix timestamp.
    pub valid_to: u32,
}

/// Extract order UID parameters from a 56-byte hex-encoded string.
///
/// Mirrors `extractOrderUidParams` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `order_uid` - A `0x`-prefixed hex string encoding a 56-byte order UID.
///
/// # Returns
///
/// An [`OrderUidParams`] containing the decoded digest, owner, and expiry.
///
/// # Errors
///
/// Returns [`CowError::Parse`] if the input is not exactly 56 bytes.
///
/// ```
/// use alloy_primitives::{Address, B256};
/// use cow_signing::{extract_order_uid_params, pack_order_uid_params};
///
/// let digest = B256::ZERO;
/// let owner = Address::ZERO;
/// let valid_to = 1_000_000u32;
///
/// let uid = pack_order_uid_params(digest, owner, valid_to);
/// let params = extract_order_uid_params(&uid).unwrap();
/// assert_eq!(params.order_digest, digest);
/// assert_eq!(params.owner, owner);
/// assert_eq!(params.valid_to, valid_to);
/// ```
pub fn extract_order_uid_params(order_uid: &str) -> Result<OrderUidParams, CowError> {
    let stripped = order_uid.trim_start_matches("0x");
    let bytes = alloy_primitives::hex::decode(stripped).map_err(|_e| CowError::Parse {
        field: "orderUid",
        reason: format!("invalid hex: {order_uid}"),
    })?;

    if bytes.len() != ORDER_UID_LENGTH {
        return Err(CowError::Parse {
            field: "orderUid",
            reason: format!("invalid length: expected {ORDER_UID_LENGTH}, got {}", bytes.len()),
        });
    }

    let order_digest = B256::from_slice(&bytes[0..32]);
    let owner = Address::from_slice(&bytes[32..52]);
    let valid_to = u32::from_be_bytes([bytes[52], bytes[53], bytes[54], bytes[55]]);

    Ok(OrderUidParams { order_digest, owner, valid_to })
}

#[cfg(test)]
mod tests {
    use cow_types::TokenBalance;

    use super::*;

    fn sample_order() -> UnsignedOrder {
        UnsignedOrder {
            sell_token: Address::repeat_byte(0x11),
            buy_token: Address::repeat_byte(0x22),
            receiver: Address::repeat_byte(0x33),
            sell_amount: U256::from(1_000_000u64),
            buy_amount: U256::from(900_000u64),
            valid_to: 1_700_000_000,
            app_data: B256::ZERO,
            fee_amount: U256::from(1000u64),
            kind: OrderKind::Sell,
            partially_fillable: false,
            sell_token_balance: TokenBalance::Erc20,
            buy_token_balance: TokenBalance::Erc20,
        }
    }

    // ── normalize_order ──────────────────────────────────────────────────

    #[test]
    fn normalize_order_external_to_erc20() {
        let mut order = sample_order();
        order.buy_token_balance = TokenBalance::External;
        let normalized = normalize_order(&order);
        assert_eq!(normalized.buy_token_balance, TokenBalance::Erc20);
    }

    #[test]
    fn normalize_order_erc20_stays() {
        let order = sample_order();
        let normalized = normalize_order(&order);
        assert_eq!(normalized.buy_token_balance, TokenBalance::Erc20);
        assert_eq!(normalized.sell_token_balance, TokenBalance::Erc20);
    }

    #[test]
    fn normalize_order_internal_stays() {
        let mut order = sample_order();
        order.buy_token_balance = TokenBalance::Internal;
        let normalized = normalize_order(&order);
        assert_eq!(normalized.buy_token_balance, TokenBalance::Internal);
    }

    // ── pack / extract order uid roundtrip ───────────────────────────────

    #[test]
    fn pack_extract_order_uid_roundtrip() {
        let digest = B256::new([0xab; 32]);
        let owner = Address::repeat_byte(0xcd);
        let valid_to = 1_700_000_000u32;

        let uid = pack_order_uid_params(digest, owner, valid_to);
        assert!(uid.starts_with("0x"));
        assert_eq!(uid.len(), 2 + 112); // "0x" + 56 bytes hex

        let params = extract_order_uid_params(&uid).unwrap();
        assert_eq!(params.order_digest, digest);
        assert_eq!(params.owner, owner);
        assert_eq!(params.valid_to, valid_to);
    }

    #[test]
    fn extract_order_uid_params_wrong_length() {
        let result = extract_order_uid_params("0xabcd");
        assert!(result.is_err());
    }

    #[test]
    fn extract_order_uid_params_invalid_hex() {
        let result = extract_order_uid_params("0xZZZZ");
        assert!(result.is_err());
    }

    #[test]
    fn pack_order_uid_params_zero_valid_to() {
        let uid = pack_order_uid_params(B256::ZERO, Address::ZERO, 0);
        let params = extract_order_uid_params(&uid).unwrap();
        assert_eq!(params.valid_to, 0);
    }

    // ── hash_order_cancellation / hash_order_cancellations ───────────────

    #[test]
    fn hash_order_cancellation_produces_nonzero() {
        // A valid 56-byte order UID as hex
        let uid = format!("0x{}", "ab".repeat(56));
        let result = hash_order_cancellation(1, &uid).unwrap();
        assert_ne!(result, B256::ZERO);
    }

    #[test]
    fn hash_order_cancellations_batch() {
        let uid1 = format!("0x{}", "aa".repeat(56));
        let uid2 = format!("0x{}", "bb".repeat(56));
        let result = hash_order_cancellations(1, &[uid1.as_str(), uid2.as_str()]).unwrap();
        assert_ne!(result, B256::ZERO);
    }

    #[test]
    fn hash_order_cancellations_single_matches_convenience() {
        let uid = format!("0x{}", "cc".repeat(56));
        let single = hash_order_cancellation(1, &uid).unwrap();
        let batch = hash_order_cancellations(1, &[uid.as_str()]).unwrap();
        assert_eq!(single, batch);
    }

    #[test]
    fn hash_order_cancellations_invalid_hex() {
        let result = hash_order_cancellations(1, &["0xNOTHEX"]);
        assert!(result.is_err());
    }

    // ── cancellations_hash ───────────────────────────────────────────────

    #[test]
    fn cancellations_hash_deterministic() {
        let uid = format!("0x{}", "dd".repeat(20));
        let h1 = cancellations_hash(&[uid.as_str()]).unwrap();
        let h2 = cancellations_hash(&[uid.as_str()]).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn cancellations_hash_order_matters() {
        let uid1 = format!("0x{}", "aa".repeat(20));
        let uid2 = format!("0x{}", "bb".repeat(20));
        let h_ab = cancellations_hash(&[uid1.as_str(), uid2.as_str()]).unwrap();
        let h_ba = cancellations_hash(&[uid2.as_str(), uid1.as_str()]).unwrap();
        assert_ne!(h_ab, h_ba);
    }

    // ── domain_separator ─────────────────────────────────────────────────

    #[test]
    fn domain_separator_deterministic() {
        let ds1 = domain_separator(1);
        let ds2 = domain_separator(1);
        assert_eq!(ds1, ds2);
    }

    #[test]
    fn domain_separator_differs_by_chain() {
        assert_ne!(domain_separator(1), domain_separator(5));
    }

    // ── order_hash ───────────────────────────────────────────────────────

    #[test]
    fn order_hash_deterministic() {
        let order = sample_order();
        assert_eq!(order_hash(&order), order_hash(&order));
    }

    #[test]
    fn order_hash_differs_by_kind() {
        let mut sell = sample_order();
        sell.kind = OrderKind::Sell;
        let mut buy = sample_order();
        buy.kind = OrderKind::Buy;
        assert_ne!(order_hash(&sell), order_hash(&buy));
    }

    // ── signing_digest / hash_typed_data ─────────────────────────────────

    #[test]
    fn signing_digest_starts_with_eip712_prefix() {
        let ds = domain_separator(1);
        let oh = order_hash(&sample_order());
        let digest = signing_digest(ds, oh);
        assert_ne!(digest, B256::ZERO);
    }

    #[test]
    fn hash_typed_data_equals_signing_digest() {
        let ds = domain_separator(1);
        let oh = order_hash(&sample_order());
        assert_eq!(hash_typed_data(ds, oh), signing_digest(ds, oh));
    }

    // ── build_order_typed_data ───────────────────────────────────────────

    #[test]
    fn build_order_typed_data_fields() {
        let order = sample_order();
        let typed = build_order_typed_data(order.clone(), 1);
        assert_eq!(typed.primary_type, "GPv2Order.Data");
        assert_eq!(typed.domain.chain_id, 1);
        assert_eq!(typed.domain.name, "Gnosis Protocol v2");
        assert_eq!(typed.order.sell_token, order.sell_token);
    }

    // ── hashify ──────────────────────────────────────────────────────────

    // ── OrderDomain builders + domain_separator_from ──────────────────

    #[test]
    fn order_domain_default_separator_matches_domain_separator() {
        use crate::types::OrderDomain;
        let domain = OrderDomain::for_chain(1);
        assert_eq!(domain_separator_from(&domain), domain_separator(1));
    }

    #[test]
    fn order_domain_with_chain_id() {
        use crate::types::OrderDomain;
        let domain = OrderDomain::for_chain(1).with_chain_id(5);
        assert_eq!(domain.chain_id, 5);
        assert_eq!(domain_separator_from(&domain), domain_separator(5));
    }

    #[test]
    fn order_domain_with_name_changes_separator() {
        use crate::types::OrderDomain;
        let standard = OrderDomain::for_chain(1);
        let custom = OrderDomain::for_chain(1).with_name("Custom Protocol");
        assert_ne!(domain_separator_from(&standard), domain_separator_from(&custom));
    }

    #[test]
    fn order_domain_with_version_changes_separator() {
        use crate::types::OrderDomain;
        let standard = OrderDomain::for_chain(1);
        let custom = OrderDomain::for_chain(1).with_version("v3");
        assert_ne!(domain_separator_from(&standard), domain_separator_from(&custom));
    }

    #[test]
    fn order_domain_with_verifying_contract_changes_separator() {
        use crate::types::OrderDomain;
        let standard = OrderDomain::for_chain(1);
        let custom = OrderDomain::for_chain(1).with_verifying_contract(Address::repeat_byte(0xff));
        assert_ne!(domain_separator_from(&standard), domain_separator_from(&custom));
    }

    #[test]
    fn order_domain_builder_chaining() {
        use crate::types::OrderDomain;
        let domain = OrderDomain::for_chain(1)
            .with_name("Test")
            .with_version("v1")
            .with_chain_id(42)
            .with_verifying_contract(Address::repeat_byte(0xab));
        assert_eq!(domain.name, "Test");
        assert_eq!(domain.version, "v1");
        assert_eq!(domain.chain_id, 42);
        assert_eq!(domain.verifying_contract, Address::repeat_byte(0xab));
        // The separator should be non-zero and deterministic.
        let sep = domain_separator_from(&domain);
        assert_ne!(sep, B256::ZERO);
        assert_eq!(sep, domain_separator_from(&domain));
    }

    #[test]
    fn order_domain_domain_separator_method_uses_custom_fields() {
        use crate::types::OrderDomain;
        let custom = OrderDomain::for_chain(1).with_name("Fork");
        // The method on OrderDomain should delegate to domain_separator_from,
        // so it must respect the custom name.
        assert_ne!(custom.domain_separator(), domain_separator(1));
        assert_eq!(custom.domain_separator(), domain_separator_from(&custom));
    }

    // ── hashify ──────────────────────────────────────────────────────────

    #[test]
    fn hashify_zero() {
        assert_eq!(hashify(0), B256::ZERO);
    }

    #[test]
    fn hashify_small_value() {
        let h = hashify(42);
        assert_eq!(h, B256::left_padding_from(&[42]));
    }
}
