//! Types for `CowShed` proxy contract interactions.

use alloy_primitives::{Address, B256, U256};

/// A single call to be executed by the `CowShed` proxy contract.
#[derive(Debug, Clone)]
pub struct CowShedCall {
    /// Target contract address.
    pub target: Address,
    /// ABI-encoded calldata for the call.
    pub calldata: Vec<u8>,
    /// ETH value to attach to the call.
    pub value: U256,
    /// If `true`, a revert from this call does not abort the bundle.
    pub allow_failure: bool,
}

impl CowShedCall {
    /// Construct a new [`CowShedCall`] with zero value and failure not allowed.
    ///
    /// # Arguments
    ///
    /// * `target` — The contract address to call.
    /// * `calldata` — ABI-encoded calldata for the call.
    ///
    /// # Returns
    ///
    /// A new [`CowShedCall`] with `value` set to zero and `allow_failure` set
    /// to `false`.
    #[must_use]
    pub const fn new(target: Address, calldata: Vec<u8>) -> Self {
        Self { target, calldata, value: U256::ZERO, allow_failure: false }
    }

    /// Attach an ETH value to this call.
    ///
    /// # Arguments
    ///
    /// * `value` — The amount of ETH (in wei) to send with the call.
    ///
    /// # Returns
    ///
    /// The modified [`CowShedCall`] with the given value attached (builder
    /// pattern).
    #[must_use]
    pub const fn with_value(mut self, value: U256) -> Self {
        self.value = value;
        self
    }

    /// Allow this call to revert without aborting the bundle.
    ///
    /// # Returns
    ///
    /// The modified [`CowShedCall`] with `allow_failure` set to `true`
    /// (builder pattern).
    #[must_use]
    pub const fn allowing_failure(mut self) -> Self {
        self.allow_failure = true;
        self
    }

    /// Return a reference to the target address.
    ///
    /// # Returns
    ///
    /// A reference to the target [`Address`] of this call.
    #[must_use]
    pub const fn target_ref(&self) -> &Address {
        &self.target
    }

    /// Return a reference to the calldata bytes.
    ///
    /// # Returns
    ///
    /// A byte slice of the ABI-encoded calldata.
    #[must_use]
    pub fn calldata_ref(&self) -> &[u8] {
        &self.calldata
    }

    /// Returns `true` if a non-zero ETH value is attached.
    ///
    /// # Returns
    ///
    /// `true` if `value` is greater than zero, `false` otherwise.
    #[must_use]
    pub fn has_value(&self) -> bool {
        !self.value.is_zero()
    }
}

/// Parameters for a `CowShed` execution bundle.
#[derive(Debug, Clone)]
pub struct CowShedHookParams {
    /// Ordered list of calls to execute.
    pub calls: Vec<CowShedCall>,
    /// Unique nonce preventing replay of this bundle.
    pub nonce: B256,
    /// Unix timestamp after which this bundle is invalid.
    pub deadline: U256,
}

impl CowShedHookParams {
    /// Construct a new [`CowShedHookParams`].
    ///
    /// # Arguments
    ///
    /// * `calls` — Ordered list of [`CowShedCall`]s to execute in the bundle.
    /// * `nonce` — Unique 32-byte value preventing replay of this bundle.
    /// * `deadline` — Unix timestamp after which the bundle is no longer valid.
    ///
    /// # Returns
    ///
    /// A new [`CowShedHookParams`] ready to be encoded or passed to
    /// [`CowShedSdk::build_hook`](super::CowShedSdk::build_hook).
    #[must_use]
    pub const fn new(calls: Vec<CowShedCall>, nonce: B256, deadline: U256) -> Self {
        Self { calls, nonce, deadline }
    }

    /// Number of calls in this bundle.
    ///
    /// # Returns
    ///
    /// The number of [`CowShedCall`]s contained in this bundle.
    #[must_use]
    pub const fn call_count(&self) -> usize {
        self.calls.len()
    }

    /// Returns `true` if the bundle contains no calls.
    ///
    /// # Returns
    ///
    /// `true` if the `calls` list is empty, `false` otherwise.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }

    /// Return a reference to the bundle nonce.
    ///
    /// # Returns
    ///
    /// A reference to the 32-byte nonce ([`B256`]) of this bundle.
    #[must_use]
    pub const fn nonce_ref(&self) -> &B256 {
        &self.nonce
    }
}
