//! Contract-level helpers ported from the `contracts-ts` package.
//!
//! Provides `EIP-712` typed-data signing utilities, signer wrappers, and
//! Balancer Vault role-granting calldata generation.
//!
//! # Key items
//!
//! | Item | Purpose |
//! |---|---|
//! | [`ecdsa_sign_typed_data`] | Sign `EIP-712` digest with ECDSA (supports `Eip712` and `EthSign` schemes) |
//! | [`TypedDataVersion`] | `V3` / `V4` enum for `eth_signTypedData` semantics |
//! | [`TypedDataVersionedSigner`] | Wraps a [`PrivateKeySigner`] with a version |
//! | [`grant_required_roles`] | Generate `grantRole` calldata for Balancer Vault authorizer |

use alloy_primitives::{Address, B256, keccak256};
use alloy_signer::Signer as _;
use alloy_signer_local::PrivateKeySigner;

use crate::{error::CowError, types::EcdsaSigningScheme};

// ── EIP-712 signing ─────────────────────────────────────────────────────────

/// Sign `EIP-712` typed data using the given signing scheme.
///
/// Mirrors `ecdsaSignTypedData` from the `TypeScript` `contracts-ts/sign.ts`.
///
/// For [`EcdsaSigningScheme::Eip712`], the signer produces a typed-data
/// signature over `keccak256("\x19\x01" || domain_sep || struct_hash)`.
/// For [`EcdsaSigningScheme::EthSign`], the `EIP-712` hash is first
/// computed, then wrapped in the `EIP-191` personal-sign envelope
/// (`"\x19Ethereum Signed Message:\n32"` prefix) before signing.
///
/// The returned signature is a `0x`-prefixed 65-byte `r | s | v` hex
/// string with the `v` byte normalised to 27 or 28.
///
/// # Parameters
///
/// * `scheme` — the ECDSA signing scheme ([`Eip712`](EcdsaSigningScheme::Eip712)
///   or [`EthSign`](EcdsaSigningScheme::EthSign)).
/// * `domain_sep` — the 32-byte `EIP-712` domain separator hash.
/// * `struct_hash` — the 32-byte `EIP-712` struct hash.
/// * `signer` — the private key signer to use.
///
/// # Returns
///
/// A `0x`-prefixed hex string of the 65-byte ECDSA signature.
///
/// # Errors
///
/// Returns [`CowError::Signing`] on any signing failure.
pub async fn ecdsa_sign_typed_data(
    scheme: EcdsaSigningScheme,
    domain_sep: B256,
    struct_hash: B256,
    signer: &PrivateKeySigner,
) -> Result<String, CowError> {
    let digest = match scheme {
        EcdsaSigningScheme::Eip712 => {
            // EIP-712: "\x19\x01" || domainSeparator || structHash
            let mut buf = [0u8; 66];
            buf[0] = 0x19;
            buf[1] = 0x01;
            buf[2..34].copy_from_slice(domain_sep.as_slice());
            buf[34..66].copy_from_slice(struct_hash.as_slice());
            keccak256(buf)
        }
        EcdsaSigningScheme::EthSign => {
            // First compute the EIP-712 digest, then wrap in EIP-191
            let mut typed_buf = [0u8; 66];
            typed_buf[0] = 0x19;
            typed_buf[1] = 0x01;
            typed_buf[2..34].copy_from_slice(domain_sep.as_slice());
            typed_buf[34..66].copy_from_slice(struct_hash.as_slice());
            let typed_hash = keccak256(typed_buf);

            // EIP-191 personal sign prefix
            let prefix = "\x19Ethereum Signed Message:\n32";
            let mut msg = Vec::with_capacity(prefix.len() + 32);
            msg.extend_from_slice(prefix.as_bytes());
            msg.extend_from_slice(typed_hash.as_slice());
            keccak256(&msg)
        }
    };

    let sig = signer.sign_hash(&digest).await.map_err(|e| CowError::Signing(e.to_string()))?;

    let sig_bytes = sig.as_bytes();
    Ok(format!("0x{}", alloy_primitives::hex::encode(sig_bytes)))
}

// ── EIP-712 signing strategies ──────────────────────────────────────────────

/// `EIP-712` signing version, mirroring the `TypeScript` `v3 | v4` parameter.
///
/// In practice, most wallets use `V4` semantics. The distinction matters
/// primarily for browser wallets (MetaMask) that expose both RPC methods.
/// In Rust with `alloy`, the canonical `EIP-712` algorithm is always used
/// regardless of version — this enum records the **intent** so callers can
/// route to the correct RPC method if needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedDataVersion {
    /// Use `eth_signTypedData_v3` semantics.
    V3,
    /// Use `eth_signTypedData_v4` semantics (default for most wallets).
    V4,
}

impl TypedDataVersion {
    /// Return the version string (`"v3"` or `"v4"`).
    ///
    /// # Returns
    ///
    /// `"v3"` for [`V3`](Self::V3) or `"v4"` for [`V4`](Self::V4).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V3 => "v3",
            Self::V4 => "v4",
        }
    }
}

/// Typed-data signer wrapping a [`PrivateKeySigner`] with a specific
/// [`TypedDataVersion`].
///
/// Mirrors `getTypedDataVersionedSigner` from the `TypeScript` SDK.
/// In practice, `alloy` always uses the canonical `EIP-712` algorithm and
/// version differences only matter for browser wallets. This struct records
/// the intent so callers can route to the correct RPC method if needed.
///
/// Construct via [`get_typed_data_versioned_signer`],
/// [`get_typed_data_v3_signer`], or
/// [`get_int_chain_id_typed_data_v4_signer`].
#[derive(Debug, Clone)]
pub struct TypedDataVersionedSigner {
    /// The underlying signer.
    pub signer: PrivateKeySigner,
    /// The `EIP-712` signing version.
    pub version: TypedDataVersion,
}

/// Create a versioned typed-data signer.
///
/// Mirrors `getTypedDataVersionedSigner(signer, version)` from the
/// `TypeScript` SDK.
///
/// # Parameters
///
/// * `signer` — the [`PrivateKeySigner`] to wrap.
/// * `version` — the [`TypedDataVersion`] to associate.
///
/// # Returns
///
/// A [`TypedDataVersionedSigner`] bundling both.
#[must_use]
pub const fn get_typed_data_versioned_signer(
    signer: PrivateKeySigner,
    version: TypedDataVersion,
) -> TypedDataVersionedSigner {
    TypedDataVersionedSigner { signer, version }
}

/// Create a v3 typed-data signer.
///
/// Convenience wrapper around [`get_typed_data_versioned_signer`] with
/// [`TypedDataVersion::V3`].
///
/// Mirrors `getTypedDataV3Signer(signer)` from the `TypeScript` SDK.
///
/// # Parameters
///
/// * `signer` — the [`PrivateKeySigner`] to wrap.
///
/// # Returns
///
/// A [`TypedDataVersionedSigner`] with version `V3`.
#[must_use]
pub const fn get_typed_data_v3_signer(signer: PrivateKeySigner) -> TypedDataVersionedSigner {
    get_typed_data_versioned_signer(signer, TypedDataVersion::V3)
}

/// Create a v4 typed-data signer with integer chain ID.
///
/// Mirrors `getIntChainIdTypedDataV4Signer(signer)` from the `TypeScript`
/// SDK. The "int chain ID" distinction is a `MetaMask` workaround; in
/// Rust, chain IDs are always numeric so this is equivalent to a standard
/// v4 signer.
///
/// # Parameters
///
/// * `signer` — the [`PrivateKeySigner`] to wrap.
///
/// # Returns
///
/// A [`TypedDataVersionedSigner`] with version `V4`.
#[must_use]
pub const fn get_int_chain_id_typed_data_v4_signer(
    signer: PrivateKeySigner,
) -> TypedDataVersionedSigner {
    get_typed_data_versioned_signer(signer, TypedDataVersion::V4)
}

// ── Role granting ───────────────────────────────────────────────────────────

/// Balancer Vault action IDs used for granting required roles.
///
/// These are the `actionId` values for `manageUserBalance` and related methods.
const VAULT_ACTION_IDS: [&str; 4] = [
    // manageUserBalance
    "0xeba777d811cd36c06d540d7ff2ed18ed042fd67bbf7c9afcf88c818c7ee6b498",
    // batchSwap (given in)
    "0x1282ab709b2b70070f829c46bc36f76b32ad4989fecb2fcb09a1b3ce00bbfc30",
    // batchSwap (given out)
    "0x78ad1b68d148c070372f8643c4648efbb63c6a8a338f3c24714868e791367653",
    // swap
    "0x7b8a1d293670124924a0f532213753b89db10bde737249d4540e9a03657d1aff",
];

/// Generate ABI-encoded calldata for granting the required Balancer Vault
/// roles to the `CoW` Protocol vault relayer.
///
/// The `CoW` Protocol settlement contract interacts with the Balancer
/// Vault for token management. This function generates the `grantRole`
/// calldata for the four required action IDs (`manageUserBalance`,
/// `batchSwap` given-in, `batchSwap` given-out, `swap`).
///
/// Mirrors `grantRequiredRoles` from the `TypeScript` `contracts-ts/vault.ts`.
///
/// # Parameters
///
/// * `authorizer_address` — [`Address`] of the Vault's authorizer contract.
///   This is the `target` of each generated transaction.
/// * `vault_relayer_address` — [`Address`] of the `GPv2` vault relayer
///   that will receive the granted roles.
///
/// # Returns
///
/// A `Vec` of `(target_address, calldata)` pairs — one per role (4 total).
/// Each `calldata` is a 68-byte ABI-encoded `grantRole(bytes32,address)`
/// call.
#[must_use]
pub fn grant_required_roles(
    authorizer_address: Address,
    vault_relayer_address: Address,
) -> Vec<(Address, Vec<u8>)> {
    // Selector for `grantRole(bytes32,address)`
    let selector = &keccak256("grantRole(bytes32,address)")[..4];

    VAULT_ACTION_IDS
        .iter()
        .filter_map(|action_id_hex| {
            let action_id =
                alloy_primitives::hex::decode(action_id_hex.trim_start_matches("0x")).ok()?;
            if action_id.len() != 32 {
                return None;
            }

            let mut calldata = Vec::with_capacity(4 + 64);
            calldata.extend_from_slice(selector);
            calldata.extend_from_slice(&action_id);
            // Left-pad address to 32 bytes
            let mut addr_word = [0u8; 32];
            addr_word[12..32].copy_from_slice(vault_relayer_address.as_slice());
            calldata.extend_from_slice(&addr_word);

            Some((authorizer_address, calldata))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_required_roles_produces_correct_count() {
        let authorizer: Address = "0x1111111111111111111111111111111111111111".parse().unwrap();
        let relayer: Address = "0x2222222222222222222222222222222222222222".parse().unwrap();
        let calls = grant_required_roles(authorizer, relayer);
        assert_eq!(calls.len(), 4);
        for (target, data) in &calls {
            assert_eq!(*target, authorizer);
            // 4 byte selector + 32 byte actionId + 32 byte address = 68
            assert_eq!(data.len(), 68);
        }
    }

    #[test]
    fn typed_data_version_str() {
        assert_eq!(TypedDataVersion::V3.as_str(), "v3");
        assert_eq!(TypedDataVersion::V4.as_str(), "v4");
    }
}
