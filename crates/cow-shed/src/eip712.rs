//! EIP-712 hashing and signing for `CoWShed` hook bundles.
//!
//! Mirrors the `TypedData` contract used by `CoWShedHooks.signCalls` in the
//! `TypeScript` SDK (`packages/cow-shed/src/contracts/CoWShedHooks.ts`). The
//! digest is computed entirely with `alloy_primitives::keccak256` and
//! hand-rolled ABI encoding — `alloy-sol-types` is not a workspace
//! dependency and pulling it in just for this use case isn't worth the
//! footprint.
//!
//! # Layout
//!
//! | Item | Purpose |
//! |---|---|
//! | [`EIP712_DOMAIN_TYPE`] / [`domain_type_hash`] | Canonical EIP-712 domain type |
//! | [`CALL_TYPE`] / [`call_type_hash`] | Type string for a single [`CowShedCall`] |
//! | [`EXECUTE_HOOKS_TYPE`] / [`execute_hooks_type_hash`] | Type string for the full bundle |
//! | [`cow_shed_domain_separator`] | Build the `CoWShed` domain separator for `(chain_id, proxy, version)` |
//! | [`call_struct_hash`] | Hash a single call |
//! | [`execute_hooks_struct_hash`] | Hash the `ExecuteHooks(calls, nonce, deadline)` message |
//! | [`typed_data_digest`] | `keccak256(0x1901 ‖ domain_separator ‖ struct_hash)` — the digest to sign |

use alloy_primitives::{Address, B256, U256, keccak256};

use crate::types::{CowShedCall, CowShedHookParams};

// ── EIP-712 type strings ──────────────────────────────────────────────────────

/// Canonical `EIP712Domain` type string.
pub const EIP712_DOMAIN_TYPE: &[u8] =
    b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";

/// Type string of a single `CoWShed` [`CowShedCall`] as encoded in the
/// on-chain contract.
///
/// Mirrors the `Call` entry of `COW_SHED_712_TYPES` in the `TypeScript` SDK.
pub const CALL_TYPE: &[u8] =
    b"Call(address target,uint256 value,bytes callData,bool allowFailure,bool isDelegateCall)";

/// Type string of the full `ExecuteHooks` struct.
///
/// EIP-712 requires referenced structs to be appended in alphabetical
/// order — `Call` comes after `ExecuteHooks`. Mirrors the combined
/// `ExecuteHooks(...)Call(...)` string emitted by the `TypeScript` SDK.
pub const EXECUTE_HOOKS_TYPE: &[u8] = b"ExecuteHooks(Call[] calls,bytes32 nonce,uint256 deadline)Call(address target,uint256 value,bytes callData,bool allowFailure,bool isDelegateCall)";

// ── Type hashes ───────────────────────────────────────────────────────────────

/// Return the `keccak256` hash of [`EIP712_DOMAIN_TYPE`].
#[must_use]
pub fn domain_type_hash() -> B256 {
    keccak256(EIP712_DOMAIN_TYPE)
}

/// Return the `keccak256` hash of [`CALL_TYPE`].
#[must_use]
pub fn call_type_hash() -> B256 {
    keccak256(CALL_TYPE)
}

/// Return the `keccak256` hash of [`EXECUTE_HOOKS_TYPE`].
#[must_use]
pub fn execute_hooks_type_hash() -> B256 {
    keccak256(EXECUTE_HOOKS_TYPE)
}

// ── Domain separator ──────────────────────────────────────────────────────────

/// EIP-712 domain name for `CoWShed` — matches the `name` field on the
/// on-chain contract.
pub const COW_SHED_DOMAIN_NAME: &str = "COWShed";

/// Compute the EIP-712 domain separator for a `CoWShed` proxy.
///
/// Mirrors `CoWShedHooks.getDomain(proxy)` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `chain_id` — EIP-155 chain identifier of the target network.
/// * `proxy` — `CoWShed` proxy address acting as the `verifyingContract`.
/// * `version` — `CoWShed` version string; use [`crate::sdk::COW_SHED_LATEST_VERSION`] unless
///   pinning explicitly.
///
/// # Returns
///
/// The 32-byte domain separator for use in
/// [`typed_data_digest`].
#[must_use]
pub fn cow_shed_domain_separator(chain_id: u64, proxy: Address, version: &str) -> B256 {
    let name_hash = keccak256(COW_SHED_DOMAIN_NAME.as_bytes());
    let version_hash = keccak256(version.as_bytes());

    let mut buf = [0u8; 5 * 32];
    buf[0..32].copy_from_slice(domain_type_hash().as_slice());
    buf[32..64].copy_from_slice(name_hash.as_slice());
    buf[64..96].copy_from_slice(version_hash.as_slice());
    buf[96..128].copy_from_slice(&abi_u256(U256::from(chain_id)));
    buf[128..160].copy_from_slice(&abi_address(proxy));
    keccak256(buf)
}

// ── Struct hashes ─────────────────────────────────────────────────────────────

/// Compute the EIP-712 struct hash of a single [`CowShedCall`].
///
/// Matches the on-chain encoding:
///
/// ```text
/// keccak256(abi.encode(
///     CALL_TYPEHASH,
///     target,
///     value,
///     keccak256(callData),
///     allowFailure,
///     isDelegateCall,
/// ))
/// ```
#[must_use]
pub fn call_struct_hash(call: &CowShedCall) -> B256 {
    let calldata_hash = keccak256(&call.calldata);

    let mut buf = [0u8; 6 * 32];
    buf[0..32].copy_from_slice(call_type_hash().as_slice());
    buf[32..64].copy_from_slice(&abi_address(call.target));
    buf[64..96].copy_from_slice(&abi_u256(call.value));
    buf[96..128].copy_from_slice(calldata_hash.as_slice());
    buf[128..160].copy_from_slice(&abi_bool(call.allow_failure));
    buf[160..192].copy_from_slice(&abi_bool(call.is_delegate_call));
    keccak256(buf)
}

/// Compute the EIP-712 struct hash of an [`ExecuteHooks`](CowShedHookParams)
/// bundle.
///
/// Per EIP-712, a `Call[]` field is hashed as
/// `keccak256(concat(hash_of_each_call))`, not as an ABI-encoded dynamic
/// array.
#[must_use]
pub fn execute_hooks_struct_hash(params: &CowShedHookParams) -> B256 {
    let mut calls_buf = Vec::with_capacity(params.calls.len() * 32);
    for call in &params.calls {
        calls_buf.extend_from_slice(call_struct_hash(call).as_slice());
    }
    let calls_hash = keccak256(&calls_buf);

    let mut buf = [0u8; 4 * 32];
    buf[0..32].copy_from_slice(execute_hooks_type_hash().as_slice());
    buf[32..64].copy_from_slice(calls_hash.as_slice());
    buf[64..96].copy_from_slice(params.nonce.as_slice());
    buf[96..128].copy_from_slice(&abi_u256(params.deadline));
    keccak256(buf)
}

// ── Final digest ──────────────────────────────────────────────────────────────

/// Compute the EIP-712 digest of a `CoWShed` hook bundle — the 32-byte hash
/// the signer actually signs (`keccak256(0x1901 ‖ domain ‖ struct)`).
///
/// Mirrors `ecdsaSignTypedData(domain, types, message)` from the
/// `TypeScript` SDK when reduced to its raw digest form.
#[must_use]
pub fn typed_data_digest(
    chain_id: u64,
    proxy: Address,
    version: &str,
    params: &CowShedHookParams,
) -> B256 {
    let domain = cow_shed_domain_separator(chain_id, proxy, version);
    let struct_hash = execute_hooks_struct_hash(params);

    let mut buf = [0u8; 66];
    buf[0] = 0x19;
    buf[1] = 0x01;
    buf[2..34].copy_from_slice(domain.as_slice());
    buf[34..66].copy_from_slice(struct_hash.as_slice());
    keccak256(buf)
}

// ── ABI encoding helpers ──────────────────────────────────────────────────────

/// Left-pad a [`U256`] into a 32-byte big-endian word.
const fn abi_u256(value: U256) -> [u8; 32] {
    value.to_be_bytes::<32>()
}

/// Left-pad an [`Address`] into a 32-byte ABI-encoded word.
fn abi_address(addr: Address) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(addr.as_slice());
    out
}

/// ABI-encode a boolean into a 32-byte word (`0` or `1`).
fn abi_bool(v: bool) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[31] = u8::from(v);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_type_hash_matches_canonical() {
        // Well-known hash of the canonical EIP-712 domain type string.
        let hash = domain_type_hash();
        assert_eq!(
            format!("{hash:#x}"),
            "0x8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f"
        );
    }

    #[test]
    fn call_type_hash_matches_type_string() {
        let hash = call_type_hash();
        // Sanity: re-hashing the constant yields the same output.
        assert_eq!(hash, keccak256(CALL_TYPE));
    }

    #[test]
    fn execute_hooks_type_hash_includes_call_struct() {
        let hash = execute_hooks_type_hash();
        assert_eq!(hash, keccak256(EXECUTE_HOOKS_TYPE));
        assert_ne!(hash, keccak256(b"ExecuteHooks(Call[] calls,bytes32 nonce,uint256 deadline)"));
    }

    #[test]
    fn abi_address_left_pads_to_32_bytes() {
        let addr: Address = [0xaa; 20].into();
        let encoded = abi_address(addr);
        assert_eq!(&encoded[..12], &[0u8; 12]);
        assert_eq!(&encoded[12..], &[0xaa; 20]);
    }

    #[test]
    fn abi_bool_encodes_flag() {
        assert_eq!(abi_bool(false), [0u8; 32]);
        let mut expected = [0u8; 32];
        expected[31] = 1;
        assert_eq!(abi_bool(true), expected);
    }

    #[test]
    fn abi_u256_roundtrip() {
        let value = U256::from(12345u64);
        let encoded = abi_u256(value);
        assert_eq!(U256::from_be_bytes(encoded), value);
    }

    #[test]
    fn cow_shed_domain_separator_is_deterministic() {
        let proxy: Address = [0x11; 20].into();
        let a = cow_shed_domain_separator(1, proxy, "1.0.1");
        let b = cow_shed_domain_separator(1, proxy, "1.0.1");
        assert_eq!(a, b);
    }

    #[test]
    fn cow_shed_domain_separator_varies_by_chain() {
        let proxy: Address = [0x11; 20].into();
        let mainnet = cow_shed_domain_separator(1, proxy, "1.0.1");
        let gnosis = cow_shed_domain_separator(100, proxy, "1.0.1");
        assert_ne!(mainnet, gnosis);
    }

    #[test]
    fn cow_shed_domain_separator_varies_by_version() {
        let proxy: Address = [0x11; 20].into();
        let v100 = cow_shed_domain_separator(1, proxy, "1.0.0");
        let v101 = cow_shed_domain_separator(1, proxy, "1.0.1");
        assert_ne!(v100, v101);
    }

    #[test]
    fn cow_shed_domain_separator_varies_by_proxy() {
        let a = cow_shed_domain_separator(1, [0x11; 20].into(), "1.0.1");
        let b = cow_shed_domain_separator(1, [0x22; 20].into(), "1.0.1");
        assert_ne!(a, b);
    }

    fn sample_call() -> CowShedCall {
        CowShedCall::new([0xab; 20].into(), vec![0xde, 0xad, 0xbe, 0xef])
            .with_value(U256::from(100u64))
    }

    #[test]
    fn call_struct_hash_is_sensitive_to_every_field() {
        let base = sample_call();
        let base_hash = call_struct_hash(&base);

        let mut other = base.clone();
        other.target = [0xcd; 20].into();
        assert_ne!(base_hash, call_struct_hash(&other), "target must affect hash");

        let mut other = base.clone();
        other.value = U256::from(999u64);
        assert_ne!(base_hash, call_struct_hash(&other), "value must affect hash");

        let mut other = base.clone();
        other.calldata.push(0xff);
        assert_ne!(base_hash, call_struct_hash(&other), "calldata must affect hash");

        let mut other = base.clone();
        other.allow_failure = true;
        assert_ne!(base_hash, call_struct_hash(&other), "allow_failure must affect hash");

        let mut other = base;
        other.is_delegate_call = true;
        assert_ne!(base_hash, call_struct_hash(&other), "is_delegate_call must affect hash");
    }

    #[test]
    fn execute_hooks_struct_hash_empty_bundle() {
        let params = CowShedHookParams::new(vec![], B256::ZERO, U256::ZERO);
        // Empty bundle must hash successfully; the `calls` component is
        // the hash of an empty byte string.
        let hash = execute_hooks_struct_hash(&params);
        assert_ne!(hash, B256::ZERO);
    }

    #[test]
    fn execute_hooks_struct_hash_is_sensitive_to_nonce_and_deadline() {
        let base = CowShedHookParams::new(vec![sample_call()], B256::ZERO, U256::from(100u64));
        let base_hash = execute_hooks_struct_hash(&base);

        let other_nonce = CowShedHookParams::new(
            vec![sample_call()],
            B256::repeat_byte(0xaa),
            U256::from(100u64),
        );
        assert_ne!(base_hash, execute_hooks_struct_hash(&other_nonce));

        let other_deadline =
            CowShedHookParams::new(vec![sample_call()], B256::ZERO, U256::from(999u64));
        assert_ne!(base_hash, execute_hooks_struct_hash(&other_deadline));
    }

    #[test]
    fn execute_hooks_struct_hash_respects_call_order() {
        let a = sample_call();
        let b = CowShedCall::new([0xcd; 20].into(), vec![0x01, 0x02]);
        let ab = CowShedHookParams::new(vec![a.clone(), b.clone()], B256::ZERO, U256::ZERO);
        let ba = CowShedHookParams::new(vec![b, a], B256::ZERO, U256::ZERO);
        assert_ne!(execute_hooks_struct_hash(&ab), execute_hooks_struct_hash(&ba));
    }

    #[test]
    fn typed_data_digest_composes_domain_and_struct() {
        let proxy: Address = [0x11; 20].into();
        let params = CowShedHookParams::new(vec![sample_call()], B256::ZERO, U256::from(1u64));
        let digest = typed_data_digest(1, proxy, "1.0.1", &params);

        let domain = cow_shed_domain_separator(1, proxy, "1.0.1");
        let struct_hash = execute_hooks_struct_hash(&params);
        let mut buf = [0u8; 66];
        buf[0] = 0x19;
        buf[1] = 0x01;
        buf[2..34].copy_from_slice(domain.as_slice());
        buf[34..66].copy_from_slice(struct_hash.as_slice());
        assert_eq!(digest, keccak256(buf));
    }

    #[test]
    fn typed_data_digest_changes_when_any_input_changes() {
        let proxy: Address = [0x11; 20].into();
        let params = CowShedHookParams::new(vec![sample_call()], B256::ZERO, U256::from(1u64));
        let base = typed_data_digest(1, proxy, "1.0.1", &params);

        assert_ne!(base, typed_data_digest(100, proxy, "1.0.1", &params));
        assert_ne!(base, typed_data_digest(1, [0x22; 20].into(), "1.0.1", &params));
        assert_ne!(base, typed_data_digest(1, proxy, "1.0.0", &params));

        let tweaked = CowShedHookParams::new(
            vec![sample_call().as_delegate_call()],
            B256::ZERO,
            U256::from(1u64),
        );
        assert_ne!(base, typed_data_digest(1, proxy, "1.0.1", &tweaked));
    }
}
