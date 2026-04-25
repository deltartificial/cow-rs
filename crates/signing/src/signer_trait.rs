//! The [`CowSigner`] trait and its default `PrivateKeySigner` implementation.
//!
//! Abstracts ECDSA signing for dependency injection (tests, custom backends).

use alloy_primitives::{Address, B256, keccak256};
use cow_errors::CowError;

/// Abstraction over ECDSA signing used by the SDK.
///
/// [`alloy_signer_local::PrivateKeySigner`] implements this trait. Tests can
/// inject a mock signer that returns deterministic signatures without a
/// real private key; browser wallet adapters can implement it against an
/// EIP-1193 provider.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait CowSigner: Send + Sync {
    /// Return the signer's Ethereum address.
    fn address(&self) -> Address;

    /// Sign an EIP-712 typed-data digest.
    ///
    /// `domain_separator` and `struct_hash` are the two 32-byte components;
    /// the implementor must hash them with the `\x19\x01` prefix and sign
    /// the result.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] on signing failure.
    async fn sign_typed_data(
        &self,
        domain_separator: B256,
        struct_hash: B256,
    ) -> Result<Vec<u8>, CowError>;

    /// Sign a raw message using EIP-191 personal-sign semantics.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] on signing failure.
    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, CowError>;
}

// Function pointer (rather than a closure) so the unreachable error
// arm has its own item we can exclude from coverage.
#[cfg_attr(coverage_nightly, coverage(off))]
fn map_alloy_signing_error(e: alloy_signer::Error) -> CowError {
    CowError::Signing(e.to_string())
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl CowSigner for alloy_signer_local::PrivateKeySigner {
    fn address(&self) -> Address {
        alloy_signer::Signer::address(self)
    }

    async fn sign_typed_data(
        &self,
        domain_separator: B256,
        struct_hash: B256,
    ) -> Result<Vec<u8>, CowError> {
        let mut msg = [0u8; 66];
        msg[0] = 0x19;
        msg[1] = 0x01;
        msg[2..34].copy_from_slice(domain_separator.as_ref());
        msg[34..66].copy_from_slice(struct_hash.as_ref());
        let digest = keccak256(msg);
        let sig = alloy_signer::Signer::sign_hash(self, &digest)
            .await
            .map_err(map_alloy_signing_error)?;
        Ok(sig.as_bytes().to_vec())
    }

    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, CowError> {
        let sig = alloy_signer::Signer::sign_message(self, message)
            .await
            .map_err(map_alloy_signing_error)?;
        Ok(sig.as_bytes().to_vec())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code; panic on unexpected state is acceptable"
)]
mod tests {
    use alloy_signer_local::PrivateKeySigner;

    use super::*;

    const TEST_KEY: &str = "0x4c0883a69102937d6231471b5dbb6204fe512961708279f99ae5f1e7b8a6c5e1";

    fn signer() -> PrivateKeySigner {
        TEST_KEY.parse().expect("valid test key")
    }

    #[tokio::test]
    async fn private_key_signer_address_via_trait() {
        let s = signer();
        let direct = alloy_signer::Signer::address(&s);
        // Calls the trait impl, not the inherent method.
        let via_trait = <PrivateKeySigner as CowSigner>::address(&s);
        assert_eq!(direct, via_trait);
    }

    #[tokio::test]
    async fn private_key_signer_sign_typed_data_via_trait() {
        let s = signer();
        let domain = B256::from([0xaa; 32]);
        let struct_hash = B256::from([0xbb; 32]);
        let sig = <PrivateKeySigner as CowSigner>::sign_typed_data(&s, domain, struct_hash)
            .await
            .expect("signing should succeed");
        assert_eq!(sig.len(), 65, "ECDSA signature is r || s || v = 65 bytes");
    }

    #[tokio::test]
    async fn private_key_signer_sign_typed_data_is_deterministic() {
        // alloy's deterministic-ECDSA (RFC 6979) means identical inputs
        // always yield identical signatures, which is what we rely on for
        // EIP-712 verification stability.
        let s = signer();
        let domain = B256::from([0x11; 32]);
        let struct_hash = B256::from([0x22; 32]);
        let a = <PrivateKeySigner as CowSigner>::sign_typed_data(&s, domain, struct_hash)
            .await
            .unwrap();
        let b = <PrivateKeySigner as CowSigner>::sign_typed_data(&s, domain, struct_hash)
            .await
            .unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn private_key_signer_sign_message_via_trait() {
        let s = signer();
        let sig = <PrivateKeySigner as CowSigner>::sign_message(&s, b"hello world")
            .await
            .expect("signing should succeed");
        assert_eq!(sig.len(), 65);
    }

    #[tokio::test]
    async fn private_key_signer_typed_and_message_signatures_differ() {
        let s = signer();
        let typed = <PrivateKeySigner as CowSigner>::sign_typed_data(
            &s,
            B256::from([0x33; 32]),
            B256::from([0x44; 32]),
        )
        .await
        .unwrap();
        let msg =
            <PrivateKeySigner as CowSigner>::sign_message(&s, b"different scheme").await.unwrap();
        assert_ne!(typed, msg);
    }
}
