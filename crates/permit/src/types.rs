//! EIP-2612 permit types used across the `permit` module.
//!
//! Defines [`PermitInfo`] (the five fields hashed during signing),
//! [`Erc20PermitInfo`] (EIP-712 domain metadata for the token), and
//! [`PermitHookData`] (the signed calldata ready to attach to an order).

use std::fmt;

use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};

use cow_types::CowHook;

/// Core parameters for an EIP-2612 permit operation.
///
/// These fields map directly to the `Permit` struct hashed during EIP-712
/// signing. Construct via [`new`](Self::new) and customise with
/// [`with_nonce`](Self::with_nonce) / [`with_deadline`](Self::with_deadline).
///
/// # Example
///
/// ```
/// use alloy_primitives::{Address, U256};
/// use cow_permit::PermitInfo;
///
/// let info = PermitInfo::new(Address::ZERO, Address::ZERO, Address::ZERO, U256::MAX)
///     .with_nonce(U256::from(5u64))
///     .with_deadline(1_700_000_000);
/// assert!(info.is_unlimited_allowance());
/// assert!(!info.is_expired(1_699_999_999));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermitInfo {
    /// The ERC-20 token address that implements `EIP-2612`.
    pub token_address: Address,
    /// The address whose allowance is being set (`msg.sender` when signed).
    pub owner: Address,
    /// The address receiving the spending allowance (typically `CoW` Vault Relayer).
    pub spender: Address,
    /// The allowance value to set.
    pub value: U256,
    /// The token's current permit nonce for `owner`.
    pub nonce: U256,
    /// Unix timestamp (seconds) after which the signature is invalid.
    pub deadline: u64,
}

impl PermitInfo {
    /// Construct a [`PermitInfo`] with `nonce = 0` and `deadline = 0`.
    ///
    /// Use [`with_nonce`](Self::with_nonce) and
    /// [`with_deadline`](Self::with_deadline) to set the remaining fields.
    ///
    /// # Parameters
    ///
    /// * `token_address` — the ERC-20 contract [`Address`].
    /// * `owner` — the address granting the allowance.
    /// * `spender` — the address receiving the allowance (typically the `CoW` Vault Relayer).
    /// * `value` — the allowance amount (`U256::MAX` for unlimited).
    ///
    /// # Returns
    ///
    /// A new [`PermitInfo`] with nonce `0` and deadline `0`.
    #[must_use]
    pub const fn new(
        token_address: Address,
        owner: Address,
        spender: Address,
        value: U256,
    ) -> Self {
        Self { token_address, owner, spender, value, nonce: U256::ZERO, deadline: 0 }
    }

    /// Set the token's current permit nonce for `owner`.
    ///
    /// # Parameters
    ///
    /// * `nonce` — the token's current permit nonce for `owner`.
    ///
    /// # Returns
    ///
    /// The modified [`PermitInfo`] (builder pattern).
    #[must_use]
    pub const fn with_nonce(mut self, nonce: U256) -> Self {
        self.nonce = nonce;
        self
    }

    /// Set the permit deadline as a Unix timestamp.
    ///
    /// # Parameters
    ///
    /// * `deadline` — Unix timestamp (seconds) after which the permit is invalid.
    ///
    /// # Returns
    ///
    /// The modified [`PermitInfo`] (builder pattern).
    #[must_use]
    pub const fn with_deadline(mut self, deadline: u64) -> Self {
        self.deadline = deadline;
        self
    }

    /// Returns `true` if the permit has expired at the given Unix timestamp.
    ///
    /// A permit is expired when `timestamp > deadline`.
    ///
    /// # Parameters
    ///
    /// * `timestamp` — the current Unix timestamp to check against.
    ///
    /// # Returns
    ///
    /// `true` if `timestamp > self.deadline`.
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_permit::PermitInfo;
    ///
    /// let info = PermitInfo::new(Address::ZERO, Address::ZERO, Address::ZERO, U256::ZERO)
    ///     .with_deadline(1_000_000);
    /// assert!(!info.is_expired(1_000_000)); // deadline is inclusive
    /// assert!(info.is_expired(1_000_001));
    /// ```
    #[must_use]
    pub const fn is_expired(&self, timestamp: u64) -> bool {
        timestamp > self.deadline
    }

    /// Returns `true` if the permit allowance is zero (revocation permit).
    ///
    /// # Returns
    ///
    /// `true` if `self.value` is zero.
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_permit::PermitInfo;
    ///
    /// let revoke = PermitInfo::new(Address::ZERO, Address::ZERO, Address::ZERO, U256::ZERO);
    /// assert!(revoke.is_zero_allowance());
    /// ```
    #[must_use]
    pub fn is_zero_allowance(&self) -> bool {
        self.value.is_zero()
    }

    /// Returns `true` if the permit allowance is `U256::MAX` (unlimited approval).
    ///
    /// # Returns
    ///
    /// `true` if `self.value == U256::MAX`.
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_permit::PermitInfo;
    ///
    /// let unlimited = PermitInfo::new(Address::ZERO, Address::ZERO, Address::ZERO, U256::MAX);
    /// assert!(unlimited.is_unlimited_allowance());
    /// ```
    #[must_use]
    pub fn is_unlimited_allowance(&self) -> bool {
        self.value == U256::MAX
    }
}

/// Formats as `permit(token=0x…, owner=0x…, spender=0x…)`.
impl fmt::Display for PermitInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "permit(token={:#x}, owner={:#x}, spender={:#x})",
            self.token_address, self.owner, self.spender
        )
    }
}

/// EIP-712 domain metadata for an ERC-20 token that implements `EIP-2612`.
///
/// These values feed into [`permit_domain_separator`](super::eip2612::permit_domain_separator)
/// when computing the signing digest.
///
/// # Example
///
/// ```
/// use cow_permit::Erc20PermitInfo;
///
/// let info = Erc20PermitInfo::new("USD Coin", "2", 1);
/// assert_eq!(info.name, "USD Coin");
/// assert_eq!(info.chain_id, 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Erc20PermitInfo {
    /// The token's `name()` return value, used in the EIP-712 domain separator.
    pub name: String,
    /// The signing domain version (commonly `"1"`).
    pub version: String,
    /// The chain ID the permit is valid on.
    pub chain_id: u64,
}

impl Erc20PermitInfo {
    /// Construct [`Erc20PermitInfo`] from its three fields.
    ///
    /// # Parameters
    ///
    /// * `name` — the token's `name()` return value.
    /// * `version` — the EIP-712 domain version (commonly `"1"` or `"2"`).
    /// * `chain_id` — the EIP-155 chain ID.
    ///
    /// # Returns
    ///
    /// A new [`Erc20PermitInfo`].
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>, chain_id: u64) -> Self {
        Self { name: name.into(), version: version.into(), chain_id }
    }
}

/// Formats as `erc20-permit(name, vversion, chain=chain_id)`.
impl fmt::Display for Erc20PermitInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "erc20-permit({}, v{}, chain={})", self.name, self.version, self.chain_id)
    }
}

/// Pre-interaction hook data that `CoW` Protocol appends to an order to
/// trigger an `EIP-2612` permit call before settlement.
///
/// Produced by [`build_permit_hook`](super::eip2612::build_permit_hook).
/// Call [`into_cow_hook`](Self::into_cow_hook) to convert to a
/// [`CowHook`] for embedding in order app-data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermitHookData {
    /// The token contract that will receive the `permit(...)` call.
    pub target: Address,
    /// ABI-encoded `permit(owner,spender,value,nonce,deadline,v,r,s)` calldata.
    pub calldata: Vec<u8>,
    /// Upper-bound gas cost for the hook (used by the solver for gas estimation).
    pub gas_limit: u64,
}

impl PermitHookData {
    /// Construct a [`PermitHookData`] from its constituent fields.
    ///
    /// # Parameters
    ///
    /// * `target` — the token contract [`Address`] to call `permit()` on.
    /// * `calldata` — ABI-encoded `permit(...)` calldata (260 bytes).
    /// * `gas_limit` — upper-bound gas for the hook (typically
    ///   [`PERMIT_GAS_LIMIT`](super::eip2612::PERMIT_GAS_LIMIT)).
    ///
    /// # Returns
    ///
    /// A new [`PermitHookData`].
    #[must_use]
    pub const fn new(target: Address, calldata: Vec<u8>, gas_limit: u64) -> Self {
        Self { target, calldata, gas_limit }
    }

    /// Returns `true` if the calldata is non-empty.
    ///
    /// # Returns
    ///
    /// `true` if the ABI-encoded calldata has a non-zero length.
    #[must_use]
    pub const fn has_calldata(&self) -> bool {
        !self.calldata.is_empty()
    }

    /// Returns the length of the ABI-encoded calldata in bytes.
    ///
    /// # Returns
    ///
    /// The byte length of the calldata (typically 260 for a standard permit).
    #[must_use]
    pub const fn calldata_len(&self) -> usize {
        self.calldata.len()
    }

    /// Convert this hook into a [`CowHook`] for embedding in order app-data.
    ///
    /// The resulting hook can be placed in
    /// `OrderInteractionHooks::pre`
    /// so that the solver executes the permit call before settling the order.
    ///
    /// # Returns
    ///
    /// A [`CowHook`] with the target address, hex-encoded calldata, and gas
    /// limit as a string.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use alloy_primitives::{U256, address};
    /// use alloy_signer_local::PrivateKeySigner;
    /// use cow_permit::{Erc20PermitInfo, PermitInfo, build_permit_hook};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let signer: PrivateKeySigner =
    ///     "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".parse()?;
    /// let info = PermitInfo {
    ///     token_address: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
    ///     owner: signer.address(),
    ///     spender: address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110"),
    ///     value: U256::from(1_000_000u64),
    ///     nonce: U256::ZERO,
    ///     deadline: 9_999_999_999u64,
    /// };
    /// let erc20_info = Erc20PermitInfo { name: "USD Coin".into(), version: "2".into(), chain_id: 1 };
    /// let hook = build_permit_hook(&info, &erc20_info, &signer).await?.into_cow_hook();
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn into_cow_hook(self) -> CowHook {
        CowHook {
            target: format!("{:#x}", self.target),
            call_data: format!("0x{}", alloy_primitives::hex::encode(&self.calldata)),
            gas_limit: self.gas_limit.to_string(),
            dapp_id: None,
        }
    }
}

/// Formats as `permit-hook(token=0x…, gas=gas_limit, calldata_len=len)`.
impl fmt::Display for PermitHookData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "permit-hook(token={:#x}, gas={}, calldata_len={})",
            self.target,
            self.gas_limit,
            self.calldata.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permit_info_new_defaults() {
        let info = PermitInfo::new(Address::ZERO, Address::ZERO, Address::ZERO, U256::ZERO);
        assert!(info.nonce.is_zero());
        assert_eq!(info.deadline, 0);
        assert!(info.is_zero_allowance());
        assert!(!info.is_unlimited_allowance());
    }

    #[test]
    fn permit_info_builders() {
        let info = PermitInfo::new(Address::ZERO, Address::ZERO, Address::ZERO, U256::MAX)
            .with_nonce(U256::from(5u64))
            .with_deadline(1_000_000);
        assert_eq!(info.nonce, U256::from(5u64));
        assert_eq!(info.deadline, 1_000_000);
        assert!(info.is_unlimited_allowance());
        assert!(!info.is_zero_allowance());
    }

    #[test]
    fn permit_info_is_expired_boundary() {
        let info = PermitInfo::new(Address::ZERO, Address::ZERO, Address::ZERO, U256::ZERO)
            .with_deadline(1000);
        assert!(!info.is_expired(999));
        assert!(!info.is_expired(1000));
        assert!(info.is_expired(1001));
    }

    #[test]
    fn permit_info_display() {
        let info = PermitInfo::new(Address::ZERO, Address::ZERO, Address::ZERO, U256::ZERO);
        let s = format!("{info}");
        assert!(s.starts_with("permit("));
    }

    #[test]
    fn erc20_permit_info_new() {
        let info = Erc20PermitInfo::new("USD Coin", "2", 1);
        assert_eq!(info.name, "USD Coin");
        assert_eq!(info.version, "2");
        assert_eq!(info.chain_id, 1);
    }

    #[test]
    fn erc20_permit_info_display() {
        let info = Erc20PermitInfo::new("USDC", "1", 1);
        let s = format!("{info}");
        assert!(s.contains("USDC"));
        assert!(s.contains("chain=1"));
    }

    #[test]
    fn permit_hook_data_new() {
        let data = PermitHookData::new(Address::ZERO, vec![1, 2, 3], 50_000);
        assert!(data.has_calldata());
        assert_eq!(data.calldata_len(), 3);
        assert_eq!(data.gas_limit, 50_000);
    }

    #[test]
    fn permit_hook_data_empty_calldata() {
        let data = PermitHookData::new(Address::ZERO, vec![], 0);
        assert!(!data.has_calldata());
        assert_eq!(data.calldata_len(), 0);
    }

    #[test]
    fn permit_hook_data_into_cow_hook() {
        let data = PermitHookData::new(Address::ZERO, vec![0xab, 0xcd], 100_000);
        let hook = data.into_cow_hook();
        assert!(hook.call_data.starts_with("0x"));
        assert!(hook.call_data.contains("abcd"));
        assert_eq!(hook.gas_limit, "100000");
    }

    #[test]
    fn permit_hook_data_display() {
        let data = PermitHookData::new(Address::ZERO, vec![0; 260], 50_000);
        let s = format!("{data}");
        assert!(s.contains("gas=50000"));
        assert!(s.contains("calldata_len=260"));
    }
}
