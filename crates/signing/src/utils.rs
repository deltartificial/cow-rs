//! ECDSA signing helpers for `CoW` Protocol orders.

use alloy_primitives::{Address, B256};
use alloy_signer::Signer as _;
use alloy_signer_local::PrivateKeySigner;
use cow_errors::CowError;
use cow_types::{EcdsaSigningScheme, SigningScheme};

use crate::{
    eip712::{cancellations_hash, domain_separator, order_hash, signing_digest},
    types::{SigningResult, UnsignedOrder},
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
/// use cow_signing::presign_result;
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
/// use cow_signing::eip1271_result;
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

/// Return the EIP-712 [`OrderDomain`](crate::types::OrderDomain)
/// for the `CoW` Protocol on `chain_id`.
///
/// This is a convenience wrapper matching the `getDomain` name from the
/// `TypeScript` `order-signing` package. It returns a fully populated
/// [`OrderDomain`](crate::types::OrderDomain) struct.
///
/// # Arguments
///
/// * `chain_id` — numeric chain ID (e.g. `1` for Mainnet, `100` for Gnosis).
///
/// # Returns
///
/// An [`OrderDomain`](crate::types::OrderDomain) configured for
/// the given chain.
///
/// # Example
///
/// ```rust
/// use cow_signing::get_domain;
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

#[cfg(test)]
mod tests {
    use alloy_primitives::U256;

    use super::*;

    fn default_order() -> UnsignedOrder {
        UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::ZERO, U256::ZERO)
    }

    // ── presign_result ──────────────────────────────────────────────────

    #[test]
    fn presign_result_has_presign_scheme() {
        let r = presign_result(Address::ZERO);
        assert_eq!(r.signing_scheme, SigningScheme::PreSign);
    }

    #[test]
    fn presign_result_signature_is_hex_address() {
        let r = presign_result(Address::ZERO);
        assert_eq!(r.signature, format!("0x{}", "00".repeat(20)));
        assert_eq!(r.signature.len(), 2 + 40); // "0x" + 20 bytes hex
    }

    #[test]
    fn presign_result_nonzero_address() {
        let addr = "0000000000000000000000000000000000000001".parse::<Address>().unwrap();
        let r = presign_result(addr);
        assert!(r.signature.ends_with("01"));
        assert!(r.is_presign());
    }

    // ── eip1271_result ──────────────────────────────────────────────────

    #[test]
    fn eip1271_result_has_eip1271_scheme() {
        let r = eip1271_result(&[0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(r.signing_scheme, SigningScheme::Eip1271);
    }

    #[test]
    fn eip1271_result_encodes_bytes() {
        let r = eip1271_result(&[0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(r.signature, "0xdeadbeef");
    }

    #[test]
    fn eip1271_result_empty_bytes() {
        let r = eip1271_result(&[]);
        assert_eq!(r.signature, "0x");
        assert!(r.is_eip1271());
    }

    // ── compute_order_uid ───────────────────────────────────────────────

    #[test]
    fn compute_order_uid_length() {
        let uid = compute_order_uid(1, &default_order(), Address::ZERO);
        assert!(uid.starts_with("0x"));
        assert_eq!(uid.len(), 2 + 112); // "0x" + 56 bytes hex
    }

    #[test]
    fn compute_order_uid_deterministic() {
        let order = default_order().with_valid_to(1_800_000_000);
        let uid1 = compute_order_uid(1, &order, Address::ZERO);
        let uid2 = compute_order_uid(1, &order, Address::ZERO);
        assert_eq!(uid1, uid2);
    }

    #[test]
    fn compute_order_uid_different_chains_differ() {
        let order = default_order();
        let uid1 = compute_order_uid(1, &order, Address::ZERO);
        let uid2 = compute_order_uid(100, &order, Address::ZERO);
        assert_ne!(uid1, uid2);
    }

    #[test]
    fn compute_order_uid_different_owners_differ() {
        let order = default_order();
        let owner1 = Address::ZERO;
        let owner2 = "0000000000000000000000000000000000000001".parse::<Address>().unwrap();
        let uid1 = compute_order_uid(1, &order, owner1);
        let uid2 = compute_order_uid(1, &order, owner2);
        assert_ne!(uid1, uid2);
    }

    #[test]
    fn compute_order_uid_different_valid_to_differ() {
        let o1 = default_order().with_valid_to(100);
        let o2 = default_order().with_valid_to(200);
        let uid1 = compute_order_uid(1, &o1, Address::ZERO);
        let uid2 = compute_order_uid(1, &o2, Address::ZERO);
        assert_ne!(uid1, uid2);
    }

    // ── generate_order_id ───────────────────────────────────────────────

    #[test]
    fn generate_order_id_matches_compute_order_uid() {
        let order = default_order().with_valid_to(12345);
        let uid = compute_order_uid(1, &order, Address::ZERO);
        let id = generate_order_id(1, &order, Address::ZERO);
        assert_eq!(uid, id);
    }

    // ── get_domain ──────────────────────────────────────────────────────

    #[test]
    fn get_domain_returns_correct_chain() {
        let d = get_domain(42);
        assert_eq!(d.chain_id, 42);
        assert_eq!(d.name, "Gnosis Protocol v2");
        assert_eq!(d.version, "v2");
    }

    // ── sign_order (async) ──────────────────────────────────────────────

    #[tokio::test]
    async fn sign_order_eip712_produces_valid_signature() {
        let signer: PrivateKeySigner =
            "0x4c0883a69102937d6231471b5dbb6204fe512961708279f99ae5f1e7b8a6c5e1".parse().unwrap();
        let order = default_order().with_valid_to(1_000_000);
        let result = sign_order(&order, 1, &signer, EcdsaSigningScheme::Eip712).await.unwrap();
        assert!(result.signature.starts_with("0x"));
        // 65 bytes = 130 hex chars + "0x" prefix
        assert_eq!(result.signature.len(), 132);
        assert!(result.is_eip712());
    }

    #[tokio::test]
    async fn sign_order_eth_sign_produces_valid_signature() {
        let signer: PrivateKeySigner =
            "0x4c0883a69102937d6231471b5dbb6204fe512961708279f99ae5f1e7b8a6c5e1".parse().unwrap();
        let order = default_order().with_valid_to(1_000_000);
        let result = sign_order(&order, 1, &signer, EcdsaSigningScheme::EthSign).await.unwrap();
        assert!(result.signature.starts_with("0x"));
        assert_eq!(result.signature.len(), 132);
        assert!(result.is_eth_sign());
    }

    #[tokio::test]
    async fn sign_order_different_schemes_produce_different_signatures() {
        let signer: PrivateKeySigner =
            "0x4c0883a69102937d6231471b5dbb6204fe512961708279f99ae5f1e7b8a6c5e1".parse().unwrap();
        let order = default_order().with_valid_to(1_000_000);
        let r1 = sign_order(&order, 1, &signer, EcdsaSigningScheme::Eip712).await.unwrap();
        let r2 = sign_order(&order, 1, &signer, EcdsaSigningScheme::EthSign).await.unwrap();
        assert_ne!(r1.signature, r2.signature);
    }

    // ── sign_order_cancellation ─────────────────────────────────────────

    #[tokio::test]
    async fn sign_order_cancellation_produces_valid_signature() {
        let signer: PrivateKeySigner =
            "0x4c0883a69102937d6231471b5dbb6204fe512961708279f99ae5f1e7b8a6c5e1".parse().unwrap();
        let order = default_order().with_valid_to(1_000_000);
        let uid = compute_order_uid(1, &order, signer.address());
        let result =
            sign_order_cancellation(&uid, 1, &signer, EcdsaSigningScheme::Eip712).await.unwrap();
        assert!(result.signature.starts_with("0x"));
        assert_eq!(result.signature.len(), 132);
        assert!(result.is_eip712());
    }

    // ── sign_order_cancellations ────────────────────────────────────────

    #[tokio::test]
    async fn sign_order_cancellations_multiple_uids() {
        let signer: PrivateKeySigner =
            "0x4c0883a69102937d6231471b5dbb6204fe512961708279f99ae5f1e7b8a6c5e1".parse().unwrap();
        let o1 = default_order().with_valid_to(100);
        let o2 = default_order().with_valid_to(200);
        let uid1 = compute_order_uid(1, &o1, signer.address());
        let uid2 = compute_order_uid(1, &o2, signer.address());
        let result = sign_order_cancellations(
            &[uid1.as_str(), uid2.as_str()],
            1,
            &signer,
            EcdsaSigningScheme::Eip712,
        )
        .await
        .unwrap();
        assert!(result.signature.starts_with("0x"));
        assert_eq!(result.signature.len(), 132);
    }

    // ── eth_sign_digest ─────────────────────────────────────────────────

    #[test]
    fn eth_sign_digest_is_deterministic() {
        let hash = B256::from([0xab; 32]);
        let d1 = eth_sign_digest(hash);
        let d2 = eth_sign_digest(hash);
        assert_eq!(d1, d2);
        assert_ne!(d1, B256::ZERO);
    }

    #[test]
    fn eth_sign_digest_differs_from_input() {
        let hash = B256::from([0xab; 32]);
        let d = eth_sign_digest(hash);
        assert_ne!(d, hash);
    }

    #[test]
    fn eth_sign_digest_different_inputs_differ() {
        let d1 = eth_sign_digest(B256::from([0x01; 32]));
        let d2 = eth_sign_digest(B256::from([0x02; 32]));
        assert_ne!(d1, d2);
    }
}
