//! EIP-2612 permit signing helpers.
//!
//! Implements the canonical EIP-712 domain separator and struct hash for the
//! `Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)`
//! type, plus ABI encoding for the on-chain `permit(...)` call selector.

use alloy_primitives::{Address, B256, U256, keccak256};
use alloy_signer::Signer as _;
use alloy_signer_local::PrivateKeySigner;

use crate::{
    error::CowError,
    permit::types::{Erc20PermitInfo, PermitHookData, PermitInfo},
};

// ── Gas budget for a single permit call (conservative estimate) ───────────────

/// Estimated gas required for a single `EIP-2612` `permit` call.
///
/// This is the value surfaced in [`PermitHookData::gas_limit`] and mirrors the
/// constant used by the `CoW` Protocol `TypeScript` SDK.
pub const PERMIT_GAS_LIMIT: u64 = 100_000;

// ── ABI encoding helpers (private) ───────────────────────────────────────────

/// Left-pad `a` to a 32-byte ABI word.
fn abi_address(a: Address) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[12..].copy_from_slice(a.as_slice());
    buf
}

/// Encode a [`U256`] as a 32-byte big-endian ABI word.
const fn abi_u256(v: U256) -> [u8; 32] {
    v.to_be_bytes()
}

/// Encode a `u64` as a 32-byte big-endian ABI word.
fn abi_u64(v: u64) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[24..].copy_from_slice(&v.to_be_bytes());
    buf
}

// ── Type hashes ───────────────────────────────────────────────────────────────

/// Compute `keccak256("EIP712Domain(string name,string version,uint256 chainId,address
/// verifyingContract)")`.
fn domain_type_hash() -> B256 {
    keccak256(b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)")
}

/// Compute `keccak256("Permit(address owner,address spender,uint256 value,uint256 nonce,uint256
/// deadline)")`.
///
/// This type hash is fixed for all `EIP-2612` compliant tokens.
///
/// # Returns
///
/// A 32-byte [`B256`] type hash constant.
#[must_use]
pub fn permit_type_hash() -> B256 {
    keccak256(b"Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)")
}

// ── Public helpers ────────────────────────────────────────────────────────────

/// Compute the EIP-712 domain separator for a token that implements
/// `EIP-2612`.
///
/// ```text
/// domain_separator = keccak256(abi.encode(
///     DOMAIN_TYPE_HASH,
///     keccak256(bytes(name)),
///     keccak256(bytes(version)),
///     chain_id,
///     token,
/// ))
/// ```
///
/// # Parameters
///
/// * `name` — the token's `name()` return value (e.g. `"USD Coin"`).
/// * `version` — the EIP-712 domain version (commonly `"1"` or `"2"`).
/// * `chain_id` — the EIP-155 chain ID.
/// * `token` — the token's contract [`Address`].
///
/// # Returns
///
/// A 32-byte [`B256`] domain separator hash.
#[must_use]
pub fn permit_domain_separator(name: &str, version: &str, chain_id: u64, token: Address) -> B256 {
    let name_hash = keccak256(name.as_bytes());
    let version_hash = keccak256(version.as_bytes());

    let mut buf = [0u8; 5 * 32];
    buf[0..32].copy_from_slice(domain_type_hash().as_slice());
    buf[32..64].copy_from_slice(name_hash.as_slice());
    buf[64..96].copy_from_slice(version_hash.as_slice());
    buf[96..128].copy_from_slice(&abi_u64(chain_id));
    buf[128..160].copy_from_slice(&abi_address(token));
    keccak256(buf)
}

/// Compute the EIP-712 signing digest for a `Permit` struct.
///
/// ```text
/// digest = keccak256("\x19\x01" || domain_separator || struct_hash)
/// struct_hash = keccak256(abi.encode(
///     PERMIT_TYPE_HASH, owner, spender, value, nonce, deadline
/// ))
/// ```
///
/// # Parameters
///
/// * `domain_sep` — the EIP-712 domain separator (from [`permit_domain_separator`]).
/// * `info` — the [`PermitInfo`] containing the permit fields.
///
/// # Returns
///
/// A 32-byte [`B256`] digest ready for ECDSA signing.
#[must_use]
pub fn permit_digest(domain_sep: B256, info: &PermitInfo) -> B256 {
    // --- struct hash ---
    let mut struct_buf = [0u8; 6 * 32];
    struct_buf[0..32].copy_from_slice(permit_type_hash().as_slice());
    struct_buf[32..64].copy_from_slice(&abi_address(info.owner));
    struct_buf[64..96].copy_from_slice(&abi_address(info.spender));
    struct_buf[96..128].copy_from_slice(&abi_u256(info.value));
    struct_buf[128..160].copy_from_slice(&abi_u256(info.nonce));
    struct_buf[160..192].copy_from_slice(&abi_u64(info.deadline));
    let struct_hash = keccak256(struct_buf);

    // --- final digest ---
    let mut digest_buf = [0u8; 66];
    digest_buf[0] = 0x19;
    digest_buf[1] = 0x01;
    digest_buf[2..34].copy_from_slice(domain_sep.as_slice());
    digest_buf[34..66].copy_from_slice(struct_hash.as_slice());
    keccak256(digest_buf)
}

/// Sign an `EIP-2612` permit and return the raw 65-byte `r ‖ s ‖ v`
/// signature.
///
/// Computes the domain separator from `erc20_info`, builds the EIP-712
/// digest from `info`, and signs it with `signer`.
///
/// # Parameters
///
/// * `info` — the [`PermitInfo`] containing owner, spender, value, nonce, and deadline.
/// * `erc20_info` — the [`Erc20PermitInfo`] containing the token's name, domain version, and chain
///   ID.
/// * `signer` — the [`PrivateKeySigner`] to sign with.
///
/// # Returns
///
/// A 65-byte array `[r(32) | s(32) | v(1)]`.
///
/// # Errors
///
/// Returns [`CowError::Signing`] if the underlying ECDSA operation fails.
pub async fn sign_permit(
    info: &PermitInfo,
    erc20_info: &Erc20PermitInfo,
    signer: &PrivateKeySigner,
) -> Result<[u8; 65], CowError> {
    let domain_sep = permit_domain_separator(
        &erc20_info.name,
        &erc20_info.version,
        erc20_info.chain_id,
        info.token_address,
    );
    let digest = permit_digest(domain_sep, info);
    let sig = signer.sign_hash(&digest).await.map_err(|e| CowError::Signing(e.to_string()))?;
    let mut out = [0u8; 65];
    out.copy_from_slice(&sig.as_bytes());
    Ok(out)
}

/// ABI-encode the `permit(address,address,uint256,uint256,uint256,uint8,bytes32,bytes32)` call.
///
/// The function selector is derived from:
/// `keccak256("permit(address,address,uint256,uint256,uint256,uint8,bytes32,bytes32)")[..4]`
///
/// Layout (4 + 8 × 32 = 260 bytes):
/// ```text
/// [selector][owner][spender][value][nonce][deadline][v][r][s]
/// ```
/// where `v` occupies a full 32-byte ABI word.
///
/// # Parameters
///
/// * `info` — the [`PermitInfo`] containing the permit fields.
/// * `signature` — the 65-byte `[r(32) | s(32) | v(1)]` ECDSA signature (from [`sign_permit`]).
///
/// # Returns
///
/// A 260-byte `Vec<u8>` containing the ABI-encoded `permit(...)` calldata.
#[must_use]
pub fn build_permit_calldata(info: &PermitInfo, signature: [u8; 65]) -> Vec<u8> {
    // EIP-2612 permit selector
    let selector: [u8; 4] = {
        let h = keccak256(b"permit(address,address,uint256,uint256,uint256,uint8,bytes32,bytes32)");
        [h[0], h[1], h[2], h[3]]
    };

    // Decompose signature: first 32 bytes = r, next 32 bytes = s, last byte = v.
    // The slices are exactly 32 bytes so the conversion is infallible; we use
    // a fixed-size copy rather than `try_into` to avoid a panic branch.
    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&signature[0..32]);
    s.copy_from_slice(&signature[32..64]);
    let v: u8 = signature[64];

    let mut calldata = Vec::with_capacity(4 + 8 * 32);
    calldata.extend_from_slice(&selector);
    calldata.extend_from_slice(&abi_address(info.owner));
    calldata.extend_from_slice(&abi_address(info.spender));
    calldata.extend_from_slice(&abi_u256(info.value));
    calldata.extend_from_slice(&abi_u256(info.nonce));
    calldata.extend_from_slice(&abi_u64(info.deadline));
    // v: uint8 padded to 32 bytes
    let mut v_word = [0u8; 32];
    v_word[31] = v;
    calldata.extend_from_slice(&v_word);
    // r and s: bytes32 verbatim
    calldata.extend_from_slice(&r);
    calldata.extend_from_slice(&s);
    calldata
}

/// Sign and build a [`PermitHookData`] ready for attachment to a `CoW`
/// Protocol order.
///
/// This is the high-level entry point: it signs the permit via
/// [`sign_permit`], encodes the calldata via [`build_permit_calldata`],
/// and wraps everything in a [`PermitHookData`] with
/// [`PERMIT_GAS_LIMIT`].
///
/// # Parameters
///
/// * `info` — the [`PermitInfo`] (token, owner, spender, value, nonce, deadline).
/// * `erc20_info` — the [`Erc20PermitInfo`] (token name, version, chain).
/// * `signer` — the [`PrivateKeySigner`] to sign with.
///
/// # Returns
///
/// A [`PermitHookData`] with the target token address, ABI-encoded
/// calldata, and gas limit. Use [`PermitHookData::into_cow_hook`] to
/// convert it to a [`CowHook`](crate::app_data::CowHook) for order
/// app-data.
///
/// # Errors
///
/// Returns [`CowError::Signing`] if the ECDSA signing step fails.
pub async fn build_permit_hook(
    info: &PermitInfo,
    erc20_info: &Erc20PermitInfo,
    signer: &PrivateKeySigner,
) -> Result<PermitHookData, CowError> {
    let signature = sign_permit(info, erc20_info, signer).await?;
    let calldata = build_permit_calldata(info, signature);
    Ok(PermitHookData { target: info.token_address, calldata, gas_limit: PERMIT_GAS_LIMIT })
}

#[cfg(test)]
mod tests {
    use alloy_primitives::address;

    use super::*;

    #[test]
    fn permit_type_hash_is_stable() {
        // Value is non-zero and deterministic across calls.
        let h = permit_type_hash();
        assert_ne!(h, B256::ZERO);
        assert_eq!(h, permit_type_hash());
    }

    #[test]
    fn domain_separator_is_deterministic() {
        let token = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
        let a = permit_domain_separator("USD Coin", "2", 1, token);
        let b = permit_domain_separator("USD Coin", "2", 1, token);
        assert_eq!(a, b);
        // Different chain → different separator
        let c = permit_domain_separator("USD Coin", "2", 5, token);
        assert_ne!(a, c);
    }

    #[test]
    fn calldata_has_correct_length() {
        let info = PermitInfo {
            token_address: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            owner: address!("1111111111111111111111111111111111111111"),
            spender: address!("2222222222222222222222222222222222222222"),
            value: U256::from(1_000_000u64),
            nonce: U256::ZERO,
            deadline: 9_999_999_999u64,
        };
        let sig = [0u8; 65];
        let cd = build_permit_calldata(&info, sig);
        // 4 (selector) + 8 × 32 (params) = 260 bytes
        assert_eq!(cd.len(), 4 + 8 * 32);
    }
}
