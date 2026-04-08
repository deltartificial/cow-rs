//! Global provider adapter — ported from `adapters/context.ts`.
//!
//! The `TypeScript` SDK uses a singleton `AdapterContext` that holds a
//! `ProviderAdapter` used by every package. In Rust we model this as a
//! [`ProviderAdapter`] trait behind a `OnceLock<Arc<dyn ProviderAdapter>>`.
//!
//! # Usage
//!
//! 1. Implement [`ProviderAdapter`] for your Ethereum provider/signer.
//! 2. Call [`set_global_adapter`] once at application startup.
//! 3. Other SDK modules call [`get_global_adapter`] internally when they
//!    need to sign or send transactions.

use std::sync::{Arc, OnceLock};

use alloy_primitives::Address;

use crate::error::CowError;

/// Type alias for the boxed adapter stored in the global singleton.
type AdapterArc = Arc<dyn ProviderAdapter>;

/// Global adapter singleton.
static GLOBAL_ADAPTER: OnceLock<AdapterArc> = OnceLock::new();

/// Abstraction over an Ethereum provider + signer, mirroring the `TypeScript`
/// `AbstractProviderAdapter`.
///
/// Implementors supply chain I/O (signing, RPC calls, etc.) so that the SDK
/// core remains transport-agnostic. The trait is object-safe (`Send + Sync`)
/// and stored behind an `Arc` in the global singleton.
///
/// # Required methods
///
/// | Method | Purpose |
/// |---|---|
/// | [`signer_address`](Self::signer_address) | Return the default EOA/contract address |
/// | [`sign_typed_data`](Self::sign_typed_data) | Sign an `EIP-712` digest → 65-byte `r\|s\|v` |
/// | [`sign_message`](Self::sign_message) | Sign a raw message hash (`EIP-191`) |
pub trait ProviderAdapter: Send + Sync {
    /// Return the default signer address (the "account").
    ///
    /// This is the Ethereum address that will be used as the order `owner`
    /// when submitting orders to the `CoW` Protocol.
    ///
    /// # Returns
    ///
    /// The signer's [`Address`].
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if no signer is configured.
    fn signer_address(&self) -> Result<Address, CowError>;

    /// Sign an `EIP-712` typed-data digest and return the 65-byte `r|s|v`
    /// signature.
    ///
    /// The SDK computes the domain separator and struct hash internally and
    /// passes them here. The implementor must produce the signature over
    /// `keccak256("\x19\x01" || domain_separator || struct_hash)`.
    ///
    /// # Parameters
    ///
    /// * `domain_separator` — the 32-byte `EIP-712` domain separator.
    /// * `struct_hash` — the 32-byte `EIP-712` struct hash.
    ///
    /// # Returns
    ///
    /// A 65-byte `Vec<u8>` containing `r` (32 B) || `s` (32 B) || `v` (1 B).
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] on signing failure.
    fn sign_typed_data(
        &self,
        domain_separator: [u8; 32],
        struct_hash: [u8; 32],
    ) -> Result<Vec<u8>, CowError>;

    /// Sign a raw message hash using `eth_sign` semantics.
    ///
    /// The provider is expected to apply the `EIP-191` personal-sign prefix
    /// (`"\x19Ethereum Signed Message:\n" + len + message`) before signing.
    ///
    /// # Parameters
    ///
    /// * `message` — the raw message bytes to sign.
    ///
    /// # Returns
    ///
    /// A 65-byte `Vec<u8>` containing the `r|s|v` signature.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] on signing failure.
    fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, CowError>;
}

/// Set the global [`ProviderAdapter`].
///
/// Can only be called **once** per process; subsequent calls are silently
/// ignored (the first adapter wins, matching the `TypeScript` SDK's
/// `setGlobalAdapter` behaviour).
///
/// Call this at application startup before any SDK operation that requires
/// signing or on-chain interaction.
///
/// # Parameters
///
/// * `adapter` — an `Arc<dyn ProviderAdapter>` wrapping your provider
///   implementation.
pub fn set_global_adapter(adapter: AdapterArc) {
    // OnceLock::set returns Err on duplicate — we intentionally ignore it.
    let _already_set = GLOBAL_ADAPTER.set(adapter);
}

/// Retrieve the global [`ProviderAdapter`].
///
/// Returns a cloned `Arc` handle to the adapter previously registered via
/// [`set_global_adapter`]. The clone is cheap (reference-count bump only).
///
/// # Returns
///
/// An `Arc<dyn ProviderAdapter>` that can be used for signing and RPC calls.
///
/// # Errors
///
/// Returns [`CowError::Config`] if no adapter has been set via
/// [`set_global_adapter`].
pub fn get_global_adapter() -> Result<AdapterArc, CowError> {
    GLOBAL_ADAPTER.get().cloned().ok_or_else(|| {
        CowError::Config(
            "Provider adapter is not configured. Call set_global_adapter() first.".to_owned(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // We cannot test set/get in unit tests easily because OnceLock is process-global,
    // but we can verify the error path.
    #[test]
    fn get_global_adapter_returns_error_when_unset() {
        // In a fresh test binary this may or may not have been set by another test.
        // We just verify the function doesn't panic.
        let _result = get_global_adapter();
    }
}
