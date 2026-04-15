//! Balancer Vault role management for `CoW` Protocol settlements.
//!
//! The `CoW` Protocol settlement contract interacts with the Balancer V2
//! Vault for token management. This module provides utilities for computing
//! vault role hashes and generating `grantRole`/`revokeRole` calldata for
//! the Vault's authorizer contract.

use alloy_primitives::{Address, B256, keccak256};

/// The Balancer V2 Vault function names used by `GPv2Settlement`.
pub const VAULT_ACTIONS: &[&str] = &["manageUserBalance", "batchSwap"];

/// Function selectors for the Vault actions that require authorization.
///
/// - `manageUserBalance((uint8,address,uint256,address,address)[])` : `0x0e8e3e84`
/// - `batchSwap(uint8,(bytes32,uint256,uint256,uint256,bytes)[],address[],(address,bool,address,
///   bool),int256[],uint256)` : `0x945bcec9`
const MANAGE_USER_BALANCE_SELECTOR: [u8; 4] = [0x0e, 0x8e, 0x3e, 0x84];
const BATCH_SWAP_SELECTOR: [u8; 4] = [0x94, 0x5b, 0xce, 0xc9];

/// Compute the Balancer Vault action-ID hash for a given vault and function selector.
///
/// The Balancer authorizer uses `keccak256(abi.encodePacked(bytes32(actionId_disambiguator),
/// bytes4(selector)))` where the disambiguator is derived from the vault address. For simplicity,
/// this function computes `keccak256(vault_address ++ selector)` as a role
/// identifier suitable for use with `grantRole`.
///
/// # Arguments
///
/// * `vault` - The Balancer Vault contract [`Address`].
/// * `selector` - The 4-byte function selector.
///
/// # Returns
///
/// The 32-byte role hash as [`B256`].
///
/// ```
/// use alloy_primitives::{Address, B256};
/// use cow_settlement::vault::vault_role_hash;
///
/// let hash = vault_role_hash(Address::ZERO, [0x0e, 0x8e, 0x3e, 0x84]);
/// assert_ne!(hash, B256::ZERO);
/// ```
#[must_use]
pub fn vault_role_hash(vault: Address, selector: [u8; 4]) -> B256 {
    let mut buf = Vec::with_capacity(20 + 4);
    buf.extend_from_slice(vault.as_slice());
    buf.extend_from_slice(&selector);
    keccak256(&buf)
}

/// Return the function selectors for the Vault methods that require authorization.
///
/// Currently returns selectors for:
/// - `manageUserBalance` (`0x0e8e3e84`)
/// - `batchSwap` (`0x945bcec9`)
///
/// # Returns
///
/// A `Vec` of 4-byte function selectors.
///
/// ```
/// use cow_settlement::vault::required_vault_role_selectors;
///
/// let selectors = required_vault_role_selectors();
/// assert_eq!(selectors.len(), 2);
/// assert_eq!(selectors[0], [0x0e, 0x8e, 0x3e, 0x84]);
/// assert_eq!(selectors[1], [0x94, 0x5b, 0xce, 0xc9]);
/// ```
#[must_use]
pub fn required_vault_role_selectors() -> Vec<[u8; 4]> {
    vec![MANAGE_USER_BALANCE_SELECTOR, BATCH_SWAP_SELECTOR]
}

/// ABI-encode `grantRole(bytes32 role, address account)` calldata.
///
/// # Arguments
///
/// * `role` - The 32-byte role identifier.
/// * `account` - The [`Address`] to grant the role to.
///
/// # Returns
///
/// A 68-byte `Vec<u8>`: 4-byte selector + 32-byte role + 32-byte address.
///
/// ```
/// use alloy_primitives::{Address, B256};
/// use cow_settlement::vault::grant_role_calldata;
///
/// let calldata = grant_role_calldata(B256::ZERO, Address::ZERO);
/// assert_eq!(calldata.len(), 68);
/// assert_eq!(&calldata[..4], &alloy_primitives::keccak256(b"grantRole(bytes32,address)")[..4]);
/// ```
#[must_use]
pub fn grant_role_calldata(role: B256, account: Address) -> Vec<u8> {
    let selector = &keccak256("grantRole(bytes32,address)")[..4];
    let mut buf = Vec::with_capacity(68);
    buf.extend_from_slice(selector);
    buf.extend_from_slice(role.as_slice());
    buf.extend_from_slice(&abi_address(account));
    buf
}

/// ABI-encode `revokeRole(bytes32 role, address account)` calldata.
///
/// # Arguments
///
/// * `role` - The 32-byte role identifier.
/// * `account` - The [`Address`] to revoke the role from.
///
/// # Returns
///
/// A 68-byte `Vec<u8>`: 4-byte selector + 32-byte role + 32-byte address.
///
/// ```
/// use alloy_primitives::{Address, B256};
/// use cow_settlement::vault::revoke_role_calldata;
///
/// let calldata = revoke_role_calldata(B256::ZERO, Address::ZERO);
/// assert_eq!(calldata.len(), 68);
/// assert_eq!(&calldata[..4], &alloy_primitives::keccak256(b"revokeRole(bytes32,address)")[..4]);
/// ```
#[must_use]
pub fn revoke_role_calldata(role: B256, account: Address) -> Vec<u8> {
    let selector = &keccak256("revokeRole(bytes32,address)")[..4];
    let mut buf = Vec::with_capacity(68);
    buf.extend_from_slice(selector);
    buf.extend_from_slice(role.as_slice());
    buf.extend_from_slice(&abi_address(account));
    buf
}

/// Generate all `grantRole` calls needed to authorize an account on the Balancer Vault.
///
/// Produces one `(authorizer_address, calldata)` pair for each required
/// Vault role selector, using the role hash computed from the vault address
/// and function selector.
///
/// # Arguments
///
/// * `vault` - The Balancer Vault contract [`Address`].
/// * `authorizer` - The Vault's authorizer contract [`Address`] (call target).
/// * `account` - The [`Address`] to grant roles to (e.g. the settlement contract).
///
/// # Returns
///
/// A `Vec` of `(target_address, calldata)` pairs. Each `calldata` is a 68-byte
/// `grantRole(bytes32,address)` call targeting the authorizer.
///
/// ```
/// use alloy_primitives::{Address, address};
/// use cow_settlement::vault::required_vault_role_calls;
///
/// let vault = address!("BA12222222228d8Ba445958a75a0704d566BF2C8");
/// let authorizer = address!("1111111111111111111111111111111111111111");
/// let account = address!("2222222222222222222222222222222222222222");
///
/// let calls = required_vault_role_calls(vault, authorizer, account);
/// assert_eq!(calls.len(), 2);
/// for (target, data) in &calls {
///     assert_eq!(*target, authorizer);
///     assert_eq!(data.len(), 68);
/// }
/// ```
#[must_use]
#[allow(clippy::type_complexity, reason = "return type matches domain contract encoding")]
pub fn required_vault_role_calls(
    vault: Address,
    authorizer: Address,
    account: Address,
) -> Vec<(Address, Vec<u8>)> {
    required_vault_role_selectors()
        .into_iter()
        .map(|selector| {
            let role = vault_role_hash(vault, selector);
            let calldata = grant_role_calldata(role, account);
            (authorizer, calldata)
        })
        .collect()
}

/// Left-pad an [`Address`] to a 32-byte ABI word.
fn abi_address(a: Address) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[12..].copy_from_slice(a.as_slice());
    buf
}

#[cfg(test)]
mod tests {
    use alloy_primitives::address;

    use super::*;

    #[test]
    fn vault_actions_list() {
        assert_eq!(VAULT_ACTIONS.len(), 2);
        assert_eq!(VAULT_ACTIONS[0], "manageUserBalance");
        assert_eq!(VAULT_ACTIONS[1], "batchSwap");
    }

    #[test]
    fn required_selectors_count() {
        let selectors = required_vault_role_selectors();
        assert_eq!(selectors.len(), 2);
    }

    #[test]
    fn vault_role_hash_deterministic() {
        let vault = address!("BA12222222228d8Ba445958a75a0704d566BF2C8");
        let h1 = vault_role_hash(vault, MANAGE_USER_BALANCE_SELECTOR);
        let h2 = vault_role_hash(vault, MANAGE_USER_BALANCE_SELECTOR);
        assert_eq!(h1, h2);
        assert_ne!(h1, B256::ZERO);
    }

    #[test]
    fn vault_role_hash_differs_by_selector() {
        let vault = address!("BA12222222228d8Ba445958a75a0704d566BF2C8");
        let h1 = vault_role_hash(vault, MANAGE_USER_BALANCE_SELECTOR);
        let h2 = vault_role_hash(vault, BATCH_SWAP_SELECTOR);
        assert_ne!(h1, h2);
    }

    #[test]
    fn vault_role_hash_differs_by_vault() {
        let v1 = address!("1111111111111111111111111111111111111111");
        let v2 = address!("2222222222222222222222222222222222222222");
        let h1 = vault_role_hash(v1, MANAGE_USER_BALANCE_SELECTOR);
        let h2 = vault_role_hash(v2, MANAGE_USER_BALANCE_SELECTOR);
        assert_ne!(h1, h2);
    }

    #[test]
    fn grant_role_calldata_format() {
        let role = B256::ZERO;
        let account = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let cd = grant_role_calldata(role, account);

        assert_eq!(cd.len(), 68);
        let expected_sel = &keccak256("grantRole(bytes32,address)")[..4];
        assert_eq!(&cd[..4], expected_sel);
        assert_eq!(&cd[4..36], role.as_slice());
        assert_eq!(&cd[48..68], account.as_slice());
    }

    #[test]
    fn revoke_role_calldata_format() {
        let role = B256::ZERO;
        let account = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let cd = revoke_role_calldata(role, account);

        assert_eq!(cd.len(), 68);
        let expected_sel = &keccak256("revokeRole(bytes32,address)")[..4];
        assert_eq!(&cd[..4], expected_sel);
        assert_eq!(&cd[4..36], role.as_slice());
        assert_eq!(&cd[48..68], account.as_slice());
    }

    #[test]
    fn grant_and_revoke_differ() {
        let role = B256::ZERO;
        let account = Address::ZERO;
        let grant = grant_role_calldata(role, account);
        let revoke = revoke_role_calldata(role, account);
        // Only selectors differ.
        assert_ne!(&grant[..4], &revoke[..4]);
        assert_eq!(&grant[4..], &revoke[4..]);
    }

    #[test]
    fn required_vault_role_calls_produces_correct_count() {
        let vault = address!("BA12222222228d8Ba445958a75a0704d566BF2C8");
        let authorizer = address!("1111111111111111111111111111111111111111");
        let account = address!("2222222222222222222222222222222222222222");

        let calls = required_vault_role_calls(vault, authorizer, account);
        assert_eq!(calls.len(), 2);
        for (target, data) in &calls {
            assert_eq!(*target, authorizer);
            assert_eq!(data.len(), 68);
        }
    }

    #[test]
    fn required_vault_role_calls_embeds_account() {
        let vault = address!("BA12222222228d8Ba445958a75a0704d566BF2C8");
        let authorizer = address!("1111111111111111111111111111111111111111");
        let account = address!("2222222222222222222222222222222222222222");

        let calls = required_vault_role_calls(vault, authorizer, account);
        for (_, data) in &calls {
            // Last 20 bytes of 68-byte calldata should be the account address.
            assert_eq!(&data[48..68], account.as_slice());
        }
    }

    #[test]
    fn required_vault_role_calls_uses_grant_selector() {
        let vault = Address::ZERO;
        let authorizer = Address::ZERO;
        let account = Address::ZERO;

        let calls = required_vault_role_calls(vault, authorizer, account);
        let expected_sel = &keccak256("grantRole(bytes32,address)")[..4];
        for (_, data) in &calls {
            assert_eq!(&data[..4], expected_sel);
        }
    }
}
