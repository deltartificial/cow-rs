//! The [`CowSigner`] trait and its default `PrivateKeySigner` implementation.
//!
//! Abstracts ECDSA signing for dependency injection (tests, custom backends).

use alloy_primitives::{Address, B256, keccak256};
use cow_sdk_error::CowError;

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
            .map_err(|e| CowError::Signing(e.to_string()))?;
        Ok(sig.as_bytes().to_vec())
    }

    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, CowError> {
        let sig = alloy_signer::Signer::sign_message(self, message)
            .await
            .map_err(|e| CowError::Signing(e.to_string()))?;
        Ok(sig.as_bytes().to_vec())
    }
}
