//! [`CowShedSdk`] — helpers for building `CowShed` proxy hooks.

use alloy_primitives::{Address, address, keccak256};

use super::types::CowShedHookParams;

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
    /// * `chain_id` — EIP-155 chain identifier (e.g. `1` for Ethereum mainnet,
    ///   `100` for Gnosis Chain).
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
    /// use cow_rs::cow_shed::CowShedSdk;
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
    /// * `params` — The [`CowShedHookParams`] containing the nonce, deadline,
    ///   and calls to encode.
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

    /// Build a [`CowHook`](crate::app_data::CowHook) that calls through this
    /// user's `CowShed` proxy.
    ///
    /// # Arguments
    ///
    /// * `_user` — The user's EOA address (reserved for future use).
    /// * `proxy` — The deployed `CowShed` proxy address that will be the hook
    ///   target.
    /// * `params` — The [`CowShedHookParams`] describing the calls, nonce, and
    ///   deadline.
    ///
    /// # Returns
    ///
    /// A [`CowHook`](crate::app_data::CowHook) with the proxy as the target,
    /// encoded calldata, and an estimated gas limit based on the number of
    /// calls.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`](crate::CowError) if encoding fails (currently infallible).
    pub fn build_hook(
        &self,
        _user: Address,
        proxy: Address,
        params: &CowShedHookParams,
    ) -> Result<crate::app_data::CowHook, crate::CowError> {
        let calldata = Self::encode_execute_hooks_calldata(params);
        let gas_limit = 100_000_u64 + 50_000_u64 * params.call_count() as u64;
        Ok(crate::app_data::CowHook {
            target: format!("{proxy:#x}"),
            call_data: alloy_primitives::hex::encode(&calldata),
            gas_limit: gas_limit.to_string(),
            dapp_id: None,
        })
    }
}
