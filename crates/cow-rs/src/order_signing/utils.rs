//! ECDSA signing helpers for `CoW` Protocol orders.

use alloy_primitives::{Address, B256};
use alloy_signer::Signer as _;
use alloy_signer_local::PrivateKeySigner;

use crate::{
    error::CowError,
    order_signing::{
        eip712::{cancellations_hash, domain_separator, order_hash, signing_digest},
        types::{SigningResult, UnsignedOrder},
    },
    types::{EcdsaSigningScheme, SigningScheme},
};

/// Sign `order` using EIP-712 typed data and return the encoded [`SigningResult`].
///
/// The signature is the standard 65-byte `r | s | v` encoding expected by the
/// `CoW` Protocol API with `signingScheme = "eip712"`.
///
/// # Arguments
///
/// * `order` — the unsigned order to sign.
/// * `chain_id` — numeric chain ID (e.g. `1` for Mainnet).
/// * `signer` — the ECDSA private key used to produce the signature.
/// * `scheme` — [`EcdsaSigningScheme::Eip712`] or [`EcdsaSigningScheme::EthSign`].
///
/// # Returns
///
/// A [`SigningResult`] containing the hex-encoded signature and the chosen
/// signing scheme.
///
/// # Errors
///
/// Returns [`CowError::Signing`] if the underlying ECDSA signing operation fails.
///
/// # Example
///
/// ```rust,no_run
/// use alloy_primitives::{Address, U256};
/// use cow_rs::{EcdsaSigningScheme, UnsignedOrder, order_signing::sign_order};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let order =
///     UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::from(1u64), U256::from(1u64));
/// let signer = "0x4c0883a69102937d6231471b5dbb6204fe512961708279f99ae5f1e7b8a6c5e1"
///     .parse::<alloy_signer_local::PrivateKeySigner>()?;
/// let result = sign_order(&order, 1, &signer, EcdsaSigningScheme::Eip712).await?;
/// assert!(result.signature.starts_with("0x"));
/// # Ok(())
/// # }
/// ```
pub async fn sign_order(
    order: &UnsignedOrder,
    chain_id: u64,
    signer: &PrivateKeySigner,
    scheme: EcdsaSigningScheme,
) -> Result<SigningResult, CowError> {
    let domain_sep = domain_separator(chain_id);
    let o_hash = order_hash(order);
    let digest = match scheme {
        EcdsaSigningScheme::Eip712 => signing_digest(domain_sep, o_hash),
        // EIP-191: prefix the raw order hash with "\x19Ethereum Signed Message:\n32".
        EcdsaSigningScheme::EthSign => eth_sign_digest(o_hash),
    };

    let sig_bytes = sign_digest(digest, signer).await?;
    let signature = format!("0x{}", alloy_primitives::hex::encode(sig_bytes));
    Ok(SigningResult { signature, signing_scheme: scheme.into_signing_scheme() })
}

/// Build a [`SigningResult`] for a **pre-sign** order.
///
/// Pre-sign orders are authenticated on-chain: the owner calls
/// `GPv2Settlement.setPreSignature(orderUid, true)` before the order can be
/// executed. The `signature` field must contain the owner's 20-byte address
/// as required by the `CoW` Protocol API.
///
/// # Arguments
///
/// * `owner` — the address of the order owner.
///
/// # Returns
///
/// A [`SigningResult`] with the hex-encoded owner address as the signature and
/// [`SigningScheme::PreSign`] as the scheme.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::Address;
/// use cow_rs::order_signing::presign_result;
///
/// let owner = Address::ZERO;
/// let result = presign_result(owner);
/// assert_eq!(result.signature, format!("0x{}", "00".repeat(20)));
/// ```
#[must_use]
pub fn presign_result(owner: Address) -> SigningResult {
    let signature = format!("0x{}", alloy_primitives::hex::encode(owner.as_slice()));
    SigningResult { signature, signing_scheme: SigningScheme::PreSign }
}

/// Build a [`SigningResult`] for an **EIP-1271** smart-contract-wallet order.
///
/// `signature_bytes` should be the bytes returned by the smart contract's
/// `isValidSignature(bytes32, bytes)` verifier, or whatever the contract's
/// off-chain signing flow produces.  The caller is responsible for obtaining
/// these bytes through the contract's signing mechanism.
///
/// # Arguments
///
/// * `signature_bytes` — the raw signature bytes from the smart contract.
///
/// # Returns
///
/// A [`SigningResult`] with the hex-encoded signature and
/// [`SigningScheme::Eip1271`] as the scheme.
///
/// # Example
///
/// ```rust
/// use cow_rs::order_signing::eip1271_result;
///
/// // Hypothetical smart-contract signature (arbitrary bytes)
/// let sig = vec![0xde, 0xad, 0xbe, 0xef];
/// let result = eip1271_result(&sig);
/// assert_eq!(result.signature, "0xdeadbeef");
/// ```
#[must_use]
pub fn eip1271_result(signature_bytes: &[u8]) -> SigningResult {
    let signature = format!("0x{}", alloy_primitives::hex::encode(signature_bytes));
    SigningResult { signature, signing_scheme: SigningScheme::Eip1271 }
}

/// Produce the raw 65-byte ECDSA signature for `digest`.
///
/// # Arguments
///
/// * `digest` — the 32-byte hash to sign.
/// * `signer` — the ECDSA private key signer.
///
/// # Returns
///
/// A 65-byte array containing the `r | s | v` ECDSA signature.
pub(crate) async fn sign_digest(
    digest: B256,
    signer: &PrivateKeySigner,
) -> Result<[u8; 65], CowError> {
    let sig = signer.sign_hash(&digest).await.map_err(|e| CowError::Signing(e.to_string()))?;
    let mut out = [0u8; 65];
    out.copy_from_slice(&sig.as_bytes());
    Ok(out)
}

/// Compute the `CoW` Protocol order UID (56-byte hex string).
///
/// The order UID is the concatenation of the EIP-712 signing digest (32 bytes),
/// the owner address (20 bytes), and the order expiry as a big-endian `uint32`
/// (4 bytes):
///
/// ```text
/// uid = signing_digest(domain_sep(chain_id), order_hash(order))
///       ‖ owner (20 bytes)
///       ‖ valid_to.to_be_bytes() (4 bytes)
/// ```
///
/// Mirrors `packOrderUidParams` / `computeOrderUid` from the `TypeScript` SDK.
/// Use this to predict the order UID before submission.
///
/// # Arguments
///
/// * `chain_id` — numeric chain ID (e.g. `1` for Mainnet).
/// * `order` — the unsigned order whose UID to compute.
/// * `owner` — the address that owns the order.
///
/// # Returns
///
/// A `0x`-prefixed 112-character hex string (56 bytes).
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, B256, U256};
/// use cow_rs::{OrderKind, TokenBalance, UnsignedOrder, order_signing::compute_order_uid};
///
/// let order = UnsignedOrder {
///     sell_token: Address::ZERO,
///     buy_token: Address::ZERO,
///     receiver: Address::ZERO,
///     sell_amount: U256::ZERO,
///     buy_amount: U256::ZERO,
///     valid_to: 1_800_000_000u32,
///     app_data: B256::ZERO,
///     fee_amount: U256::ZERO,
///     kind: OrderKind::Sell,
///     partially_fillable: false,
///     sell_token_balance: TokenBalance::Erc20,
///     buy_token_balance: TokenBalance::Erc20,
/// };
/// let uid = compute_order_uid(11_155_111, &order, Address::ZERO);
/// assert!(uid.starts_with("0x"));
/// assert_eq!(uid.len(), 2 + 112); // "0x" + 56 bytes hex
/// ```
#[must_use]
pub fn compute_order_uid(chain_id: u64, order: &UnsignedOrder, owner: Address) -> String {
    let domain_sep = domain_separator(chain_id);
    let digest = signing_digest(domain_sep, order_hash(order));
    let mut uid = Vec::with_capacity(56);
    uid.extend_from_slice(digest.as_slice()); // 32 bytes
    uid.extend_from_slice(owner.as_slice()); // 20 bytes
    uid.extend_from_slice(&order.valid_to.to_be_bytes()); // 4 bytes
    format!("0x{}", alloy_primitives::hex::encode(&uid))
}

/// Generate the order ID (UID) for a given order, chain, and owner.
///
/// This is an alias for [`compute_order_uid`] matching the `generateOrderId`
/// name from the `TypeScript` `order-signing` package.
///
/// The order ID is a 56-byte hex string composed of the signing digest
/// (32 bytes), the owner address (20 bytes), and the order expiry (4 bytes).
///
/// # Arguments
///
/// * `chain_id` — numeric chain ID (e.g. `1` for Mainnet).
/// * `order` — the unsigned order whose ID to generate.
/// * `owner` — the address that owns the order.
///
/// # Returns
///
/// A `0x`-prefixed 112-character hex string (56 bytes).
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, U256};
/// use cow_rs::{UnsignedOrder, order_signing::generate_order_id};
///
/// let order = UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::ZERO, U256::ZERO);
/// let uid = generate_order_id(1, &order, Address::ZERO);
/// assert!(uid.starts_with("0x"));
/// assert_eq!(uid.len(), 2 + 112);
/// ```
#[must_use]
pub fn generate_order_id(chain_id: u64, order: &UnsignedOrder, owner: Address) -> String {
    compute_order_uid(chain_id, order, owner)
}

/// Return the EIP-712 [`OrderDomain`](crate::order_signing::types::OrderDomain)
/// for the `CoW` Protocol on `chain_id`.
///
/// This is a convenience wrapper matching the `getDomain` name from the
/// `TypeScript` `order-signing` package. It returns a fully populated
/// [`OrderDomain`](crate::order_signing::types::OrderDomain) struct.
///
/// # Arguments
///
/// * `chain_id` — numeric chain ID (e.g. `1` for Mainnet, `100` for Gnosis).
///
/// # Returns
///
/// An [`OrderDomain`](crate::order_signing::types::OrderDomain) configured for
/// the given chain.
///
/// # Example
///
/// ```rust
/// use cow_rs::order_signing::get_domain;
///
/// let domain = get_domain(1);
/// assert_eq!(domain.chain_id, 1);
/// assert_eq!(domain.name, "Gnosis Protocol v2");
/// ```
#[must_use]
pub const fn get_domain(chain_id: u64) -> super::types::OrderDomain {
    super::types::OrderDomain::for_chain(chain_id)
}

/// Sign one or more order cancellations using EIP-712 typed data.
///
/// Mirrors `signOrderCancellations` from the `TypeScript` SDK. All order UIDs
/// are signed together in a single `OrderCancellations { orderUids: bytes[] }`
/// struct, using the same `GPv2` settlement domain as regular orders.
///
/// Each `order_uid` must be a `0x`-prefixed 56-byte hex string as returned by
/// the orderbook API when an order is created.
///
/// # Arguments
///
/// * `order_uids` — slice of `0x`-prefixed hex order UIDs to cancel.
/// * `chain_id` — numeric chain ID (e.g. `1` for Mainnet).
/// * `signer` — the ECDSA private key used to sign the cancellation.
/// * `scheme` — [`EcdsaSigningScheme::Eip712`] or [`EcdsaSigningScheme::EthSign`].
///
/// # Returns
///
/// A [`SigningResult`] containing the hex-encoded cancellation signature.
///
/// # Errors
///
/// Returns [`CowError`] if any UID is invalid hex or the signing operation
/// fails.
pub async fn sign_order_cancellations(
    order_uids: &[&str],
    chain_id: u64,
    signer: &PrivateKeySigner,
    scheme: EcdsaSigningScheme,
) -> Result<SigningResult, CowError> {
    let domain_sep = domain_separator(chain_id);
    let c_hash = cancellations_hash(order_uids)?;
    let digest = match scheme {
        EcdsaSigningScheme::Eip712 => signing_digest(domain_sep, c_hash),
        EcdsaSigningScheme::EthSign => eth_sign_digest(c_hash),
    };
    let sig_bytes = sign_digest(digest, signer).await?;
    let signature = format!("0x{}", alloy_primitives::hex::encode(sig_bytes));
    Ok(SigningResult { signature, signing_scheme: scheme.into_signing_scheme() })
}

/// Sign a single order cancellation using EIP-712 typed data.
///
/// Convenience wrapper around [`sign_order_cancellations`] for the common
/// single-order case. Mirrors `signOrderCancellation` from the `TypeScript`
/// SDK.
///
/// # Arguments
///
/// * `order_uid` — the `0x`-prefixed hex order UID to cancel.
/// * `chain_id` — numeric chain ID (e.g. `1` for Mainnet).
/// * `signer` — the ECDSA private key used to sign the cancellation.
/// * `scheme` — [`EcdsaSigningScheme::Eip712`] or [`EcdsaSigningScheme::EthSign`].
///
/// # Returns
///
/// A [`SigningResult`] containing the hex-encoded cancellation signature.
///
/// # Errors
///
/// Returns [`CowError`] if the UID is invalid hex or signing fails.
pub async fn sign_order_cancellation(
    order_uid: &str,
    chain_id: u64,
    signer: &PrivateKeySigner,
    scheme: EcdsaSigningScheme,
) -> Result<SigningResult, CowError> {
    sign_order_cancellations(&[order_uid], chain_id, signer, scheme).await
}

/// Prefix `hash` with `"\x19Ethereum Signed Message:\n32"` per EIP-191.
///
/// # Arguments
///
/// * `hash` — the 32-byte hash to wrap with the EIP-191 prefix.
///
/// # Returns
///
/// The Keccak-256 hash of the prefixed message.
fn eth_sign_digest(hash: B256) -> B256 {
    let mut buf = [0u8; 60];
    buf[..28].copy_from_slice(b"\x19Ethereum Signed Message:\n32");
    buf[28..].copy_from_slice(hash.as_slice());
    alloy_primitives::keccak256(buf)
}
