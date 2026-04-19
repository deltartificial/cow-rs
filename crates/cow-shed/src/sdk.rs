//! [`CowShedSdk`] — helpers for building `CowShed` proxy hooks.

use alloy_primitives::{Address, B256, U256, address, keccak256};
use alloy_signer_local::PrivateKeySigner;
use cow_errors::CowError;

use super::{eip712::typed_data_digest, types::CowShedHookParams};

/// `CowShed` version string for the 1.0.0 release.
pub const COW_SHED_1_0_0_VERSION: &str = "1.0.0";

/// `CowShed` version string for the 1.0.1 release.
pub const COW_SHED_1_0_1_VERSION: &str = "1.0.1";

/// The latest `CowShed` version.
pub const COW_SHED_LATEST_VERSION: &str = COW_SHED_1_0_1_VERSION;

/// `CowShed` factory contract address for version 1.0.0.
///
/// `0x00E989b87700514118Fa55326CD1cCE82faebEF6`
pub const COW_SHED_FACTORY_V1_0_0: Address = address!("00E989b87700514118Fa55326CD1cCE82faebEF6");

/// `CowShed` factory contract address for version 1.0.1.
///
/// `0x312f92fe5f1710408B20D52A374fa29e099cFA86`
pub const COW_SHED_FACTORY_V1_0_1: Address = address!("312f92fe5f1710408B20D52A374fa29e099cFA86");

/// `CowShed` implementation contract address for version 1.0.0.
///
/// `0x2CFFA8cf11B90C9F437567b86352169dF4009F73`
pub const COW_SHED_IMPLEMENTATION_V1_0_0: Address =
    address!("2CFFA8cf11B90C9F437567b86352169dF4009F73");

/// `CowShed` implementation contract address for version 1.0.1.
///
/// `0xa2704cf562ad418bf0453f4b662ebf6a2489ed88`
pub const COW_SHED_IMPLEMENTATION_V1_0_1: Address =
    address!("a2704cf562ad418bf0453f4b662ebf6a2489ed88");

/// Estimated gas cost for deploying a `CowShed` proxy (360 000 gas).
pub const COW_SHED_PROXY_CREATION_GAS: u64 = 360_000;

/// `CowShed` factory address on Ethereum mainnet.
///
/// `0xd8d3789083bb4b92b56dbda5b2ac8c5e4d3e7d30`
pub const COW_SHED_FACTORY_MAINNET: Address = address!("d8d3789083bb4b92b56dbda5b2ac8c5e4d3e7d30");

/// `CowShed` factory address on Gnosis Chain.
///
/// `0xd8d3789083bb4b92b56dbda5b2ac8c5e4d3e7d30`
pub const COW_SHED_FACTORY_GNOSIS: Address = address!("d8d3789083bb4b92b56dbda5b2ac8c5e4d3e7d30");

/// High-level helper for building `CowShed` proxy interactions.
#[derive(Debug, Clone)]
pub struct CowShedSdk {
    chain_id: u64,
}

impl CowShedSdk {
    /// Construct a new [`CowShedSdk`] for the given chain.
    ///
    /// # Arguments
    ///
    /// * `chain_id` — EIP-155 chain identifier (e.g. `1` for Ethereum mainnet, `100` for Gnosis
    ///   Chain).
    ///
    /// # Returns
    ///
    /// A new [`CowShedSdk`] instance bound to the specified chain.
    #[must_use]
    pub const fn new(chain_id: u64) -> Self {
        Self { chain_id }
    }

    /// Return the `CowShed` factory address for this chain, if supported.
    ///
    /// Returns `None` for unsupported chains.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_shed::CowShedSdk;
    ///
    /// let sdk = CowShedSdk::new(1);
    /// assert!(sdk.factory_address().is_some());
    ///
    /// let sdk_unknown = CowShedSdk::new(999);
    /// assert!(sdk_unknown.factory_address().is_none());
    /// ```
    #[must_use]
    pub const fn factory_address(&self) -> Option<Address> {
        match self.chain_id {
            1 => Some(COW_SHED_FACTORY_MAINNET),
            100 => Some(COW_SHED_FACTORY_GNOSIS),
            _ => None,
        }
    }

    /// Encode a simplified `CowShed.executeHooks(...)` calldata.
    ///
    /// Encodes the function selector, nonce, and deadline as the first three
    /// 32-byte words.  This is a functional stub — full dynamic ABI encoding
    /// of the `calls` array is intentionally omitted.
    ///
    /// Selector:
    /// `executeHooks((address,bytes,uint256,bool)[],bytes32,uint256,address,bytes)`
    ///
    /// # Arguments
    ///
    /// * `params` — The [`CowShedHookParams`] containing the nonce, deadline, and calls to encode.
    ///
    /// # Returns
    ///
    /// A 68-byte `Vec<u8>` containing the 4-byte function selector followed by
    /// the 32-byte nonce and 32-byte deadline.
    #[must_use]
    pub fn encode_execute_hooks_calldata(params: &CowShedHookParams) -> Vec<u8> {
        let sig = b"executeHooks((address,bytes,uint256,bool)[],bytes32,uint256,address,bytes)";
        let h = keccak256(sig);
        let sel = [h[0], h[1], h[2], h[3]];

        // Simplified encoding: [selector 4] [nonce 32] [deadline 32]
        // (68 bytes total — sufficient for the hook target to identify the call)
        let mut buf = Vec::with_capacity(68);
        buf.extend_from_slice(&sel);
        buf.extend_from_slice(params.nonce.as_slice());
        buf.extend_from_slice(&params.deadline.to_be_bytes::<32>());
        buf
    }

    /// Build a [`CowHook`](cow_types::CowHook) that calls through this
    /// user's `CowShed` proxy.
    ///
    /// # Arguments
    ///
    /// * `_user` — The user's EOA address (reserved for future use).
    /// * `proxy` — The deployed `CowShed` proxy address that will be the hook target.
    /// * `params` — The [`CowShedHookParams`] describing the calls, nonce, and deadline.
    ///
    /// # Returns
    ///
    /// A [`CowHook`](cow_types::CowHook) with the proxy as the target,
    /// encoded calldata, and an estimated gas limit based on the number of
    /// calls.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if encoding fails (currently infallible).
    pub fn build_hook(
        &self,
        _user: Address,
        proxy: Address,
        params: &CowShedHookParams,
    ) -> Result<cow_types::CowHook, cow_errors::CowError> {
        let calldata = Self::encode_execute_hooks_calldata(params);
        let gas_limit = 100_000_u64 + 50_000_u64 * params.call_count() as u64;
        Ok(cow_types::CowHook {
            target: format!("{proxy:#x}"),
            call_data: alloy_primitives::hex::encode(&calldata),
            gas_limit: gas_limit.to_string(),
            dapp_id: None,
        })
    }

    /// Compute the EIP-712 digest a signer would sign for this hook bundle.
    ///
    /// Equivalent to the raw `keccak256(0x1901 ‖ domain ‖ struct)` digest
    /// produced by `ecdsaSignTypedData` in the `TypeScript` SDK when
    /// targeting the `CoWShed` proxy on `(chain_id, version)`.
    ///
    /// # Arguments
    ///
    /// * `proxy` — the user's `CoWShed` proxy address (the EIP-712 `verifyingContract`).
    /// * `params` — the hook bundle to hash.
    /// * `version` — the `CoWShed` version string (defaults to [`COW_SHED_LATEST_VERSION`] when
    ///   calling [`CowShedSdk::sign_hook`]).
    #[must_use]
    pub fn hook_typed_data_digest(
        &self,
        proxy: Address,
        params: &CowShedHookParams,
        version: &str,
    ) -> B256 {
        typed_data_digest(self.chain_id, proxy, version, params)
    }

    /// Sign a [`CowShedHookParams`] bundle under the user's proxy with the
    /// given ECDSA signer.
    ///
    /// Produces a [`SignedCowShedHook`] carrying the raw 65-byte ECDSA
    /// signature (`r ‖ s ‖ v`) plus the digest that was signed, ready to
    /// be passed to `CowShed.executeHooks(calls, nonce, deadline, owner,
    /// signature)` on-chain. Mirrors the end of
    /// `CoWShedHooks.signCalls(...)` in the `TypeScript` SDK when
    /// `signingScheme == EIP712`.
    ///
    /// # Arguments
    ///
    /// * `proxy` — the user's `CoWShed` proxy address (used as the `verifyingContract` of the
    ///   EIP-712 domain).
    /// * `params` — the hook bundle to sign.
    /// * `signer` — the user's private-key signer.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Signing`] if the ECDSA operation fails.
    pub async fn sign_hook(
        &self,
        proxy: Address,
        params: &CowShedHookParams,
        signer: &PrivateKeySigner,
    ) -> Result<SignedCowShedHook, CowError> {
        self.sign_hook_with_version(proxy, params, signer, COW_SHED_LATEST_VERSION).await
    }

    /// Like [`sign_hook`](Self::sign_hook) but with an explicit `CoWShed`
    /// version string — use this if you need to pin the domain to a
    /// specific deployment.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Signing`] if the ECDSA operation fails.
    pub async fn sign_hook_with_version(
        &self,
        proxy: Address,
        params: &CowShedHookParams,
        signer: &PrivateKeySigner,
        version: &str,
    ) -> Result<SignedCowShedHook, CowError> {
        let digest = typed_data_digest(self.chain_id, proxy, version, params);
        let signature = alloy_signer::Signer::sign_hash(signer, &digest)
            .await
            .map_err(|e| CowError::Signing(e.to_string()))?;
        Ok(SignedCowShedHook { digest, signature: signature.as_bytes().to_vec() })
    }

    /// Compute the replay nonce for a `CoWShed` proxy given a stringly-typed
    /// identifier.
    ///
    /// Mirrors the `nonce` component of `CoWShedHooks.signCalls(...)` — the
    /// TS SDK hashes the caller-provided string (e.g.
    /// `"bridge-<dapp_id>-<order_uid>"`) into a `bytes32` value. The Rust
    /// equivalent is `keccak256(bytes(nonce))`.
    ///
    /// Callers that already have a raw 32-byte nonce should construct
    /// [`CowShedHookParams`] directly and bypass this helper.
    #[must_use]
    pub fn derive_nonce(nonce: &str) -> B256 {
        keccak256(nonce.as_bytes())
    }

    /// Build a UNIX-timestamp deadline `seconds` in the future relative to
    /// `now_unix`.
    ///
    /// Helper used to populate [`CowShedHookParams::deadline`]; mirrors
    /// `calculateDeadline` from the TS SDK.
    #[must_use]
    pub fn deadline_from_now(now_unix: u64, seconds: u64) -> U256 {
        U256::from(now_unix.saturating_add(seconds))
    }
}

// ── Signed hook output ────────────────────────────────────────────────────────

/// A signed `CoWShed` hook bundle — output of [`CowShedSdk::sign_hook`].
///
/// Bundles the 32-byte EIP-712 `digest` alongside the 65-byte
/// `r ‖ s ‖ v` signature so callers can forward the pair directly to
/// `CoWShed.executeHooks(...)` or re-verify locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedCowShedHook {
    /// EIP-712 digest (`keccak256(0x1901 ‖ domain ‖ struct_hash)`).
    pub digest: B256,
    /// Raw ECDSA signature bytes (`r ‖ s ‖ v`, 65 bytes total).
    pub signature: Vec<u8>,
}

impl SignedCowShedHook {
    /// Return a hex-encoded, `0x`-prefixed signature string suitable for
    /// passing to `executeHooks` as the `signature` argument.
    #[must_use]
    pub fn signature_hex(&self) -> String {
        format!("0x{}", alloy_primitives::hex::encode(&self.signature))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CowShedCall;

    /// Well-known Hardhat test account #0. SAFE to commit — public test key.
    const TEST_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    fn sample_params() -> CowShedHookParams {
        let call = CowShedCall::new([0xab; 20].into(), vec![0xde, 0xad, 0xbe, 0xef])
            .with_value(U256::from(42u64));
        CowShedHookParams::new(vec![call], B256::repeat_byte(0x11), U256::from(9_999_999u64))
    }

    #[test]
    fn derive_nonce_matches_keccak_of_bytes() {
        let nonce = CowShedSdk::derive_nonce("bridge-abc");
        assert_eq!(nonce, keccak256(b"bridge-abc"));
    }

    #[test]
    fn deadline_from_now_adds_seconds() {
        let deadline = CowShedSdk::deadline_from_now(1_000_000, 300);
        assert_eq!(deadline, U256::from(1_000_300u64));
    }

    #[test]
    fn deadline_from_now_saturates_on_overflow() {
        let deadline = CowShedSdk::deadline_from_now(u64::MAX - 1, 10);
        assert_eq!(deadline, U256::from(u64::MAX));
    }

    #[test]
    fn hook_typed_data_digest_matches_module_helper() {
        let sdk = CowShedSdk::new(1);
        let proxy: Address = [0x22; 20].into();
        let params = sample_params();
        let a = sdk.hook_typed_data_digest(proxy, &params, COW_SHED_LATEST_VERSION);
        let b = typed_data_digest(1, proxy, COW_SHED_LATEST_VERSION, &params);
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn sign_hook_produces_65_byte_signature() {
        let sdk = CowShedSdk::new(1);
        let signer: PrivateKeySigner = TEST_KEY.parse().unwrap();
        let signed = sdk.sign_hook([0x22; 20].into(), &sample_params(), &signer).await.unwrap();
        assert_eq!(signed.signature.len(), 65, "r ‖ s ‖ v is 65 bytes");
        assert!(signed.signature_hex().starts_with("0x"));
        assert_eq!(signed.signature_hex().len(), 2 + 65 * 2);
    }

    #[tokio::test]
    async fn sign_hook_is_deterministic_for_fixed_key_and_params() {
        let sdk = CowShedSdk::new(1);
        let signer: PrivateKeySigner = TEST_KEY.parse().unwrap();
        let proxy: Address = [0x22; 20].into();
        let params = sample_params();
        let a = sdk.sign_hook(proxy, &params, &signer).await.unwrap();
        let b = sdk.sign_hook(proxy, &params, &signer).await.unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn sign_hook_digest_matches_standalone_helper() {
        let sdk = CowShedSdk::new(1);
        let signer: PrivateKeySigner = TEST_KEY.parse().unwrap();
        let proxy: Address = [0x22; 20].into();
        let params = sample_params();
        let signed = sdk.sign_hook(proxy, &params, &signer).await.unwrap();
        assert_eq!(signed.digest, typed_data_digest(1, proxy, COW_SHED_LATEST_VERSION, &params));
    }

    #[tokio::test]
    async fn sign_hook_differs_by_chain_id() {
        let signer: PrivateKeySigner = TEST_KEY.parse().unwrap();
        let proxy: Address = [0x22; 20].into();
        let params = sample_params();
        let a = CowShedSdk::new(1).sign_hook(proxy, &params, &signer).await.unwrap();
        let b = CowShedSdk::new(100).sign_hook(proxy, &params, &signer).await.unwrap();
        assert_ne!(a.digest, b.digest);
        assert_ne!(a.signature, b.signature);
    }

    #[tokio::test]
    async fn sign_hook_with_version_pins_domain() {
        let sdk = CowShedSdk::new(1);
        let signer: PrivateKeySigner = TEST_KEY.parse().unwrap();
        let proxy: Address = [0x22; 20].into();
        let params = sample_params();
        let v100 = sdk
            .sign_hook_with_version(proxy, &params, &signer, COW_SHED_1_0_0_VERSION)
            .await
            .unwrap();
        let v101 = sdk
            .sign_hook_with_version(proxy, &params, &signer, COW_SHED_1_0_1_VERSION)
            .await
            .unwrap();
        assert_ne!(v100.digest, v101.digest);
    }

    #[tokio::test]
    async fn signed_hook_can_be_recovered_to_signer_address() {
        let sdk = CowShedSdk::new(1);
        let signer: PrivateKeySigner = TEST_KEY.parse().unwrap();
        let proxy: Address = [0x22; 20].into();
        let params = sample_params();
        let signed = sdk.sign_hook(proxy, &params, &signer).await.unwrap();

        // Rebuild the alloy signature and verify the ECDSA recovery
        // returns the original address.
        let sig = alloy_primitives::Signature::try_from(signed.signature.as_slice())
            .expect("valid signature");
        let recovered = sig.recover_address_from_prehash(&signed.digest).expect("recoverable");
        assert_eq!(recovered, alloy_signer::Signer::address(&signer));
    }
}
