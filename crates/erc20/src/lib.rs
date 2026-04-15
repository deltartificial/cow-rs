//! `cow-erc20` — Layer 2 `ERC-20` calldata builders for the `CoW` Protocol SDK.
//!
//! Provides ABI-encoded calldata for common `ERC-20` and `EIP-2612`
//! functions needed when preparing `CoW` Protocol orders. All functions
//! return raw `Vec<u8>` calldata ready to be sent via `eth_call` or
//! included in a transaction.
//!
//! # Key functions
//!
//! | Function | Solidity signature | Size |
//! |---|---|---|
//! | [`build_erc20_approve_calldata`] | `approve(address,uint256)` | 68 B |
//! | [`build_erc20_balance_of_calldata`] | `balanceOf(address)` | 36 B |
//! | [`build_erc20_allowance_calldata`] | `allowance(address,address)` | 68 B |
//! | [`build_erc20_transfer_calldata`] | `transfer(address,uint256)` | 68 B |
//! | [`build_erc20_transfer_from_calldata`] | `transferFrom(address,address,uint256)` | 100 B |
//! | [`build_erc20_decimals_calldata`] | `decimals()` | 4 B |
//! | [`build_erc20_name_calldata`] | `name()` | 4 B |
//! | [`build_eip2612_nonces_calldata`] | `nonces(address)` | 36 B |
//! | [`build_eip2612_version_calldata`] | `version()` | 4 B |
//!
//! The spender for swap orders is typically the `VAULT_RELAYER` contract
//! address exposed by [`cow-chains`](https://docs.rs/cow-chains).

#![deny(unsafe_code)]
#![warn(missing_docs)]

use alloy_primitives::{Address, U256, keccak256};

/// Compute the 4-byte selector from a Solidity function signature.
fn selector(sig: &str) -> [u8; 4] {
    let h = keccak256(sig.as_bytes());
    [h[0], h[1], h[2], h[3]]
}

/// Left-pad an [`Address`] to a 32-byte ABI word.
fn abi_address(a: Address) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[12..].copy_from_slice(a.as_slice());
    buf
}

/// Encode a [`U256`] as a 32-byte big-endian ABI word.
const fn abi_u256(v: U256) -> [u8; 32] {
    v.to_be_bytes()
}

/// Build calldata for `ERC20.approve(address spender, uint256 amount)`.
///
/// Call this on the sell token to grant the
/// [`VAULT_RELAYER`](cow_chains::contracts::VAULT_RELAYER) (or any other
/// spender) the allowance it needs to transfer tokens on your behalf.
///
/// # Parameters
///
/// * `spender` — the [`Address`] to approve (typically `VAULT_RELAYER`).
/// * `amount` — the allowance to set (`U256::MAX` for unlimited).
///
/// # Returns
///
/// A 68-byte `Vec<u8>`: 4-byte selector + 32-byte address + 32-byte amount.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, U256};
/// use cow_chains::contracts::VAULT_RELAYER;
/// use cow_erc20::build_erc20_approve_calldata;
///
/// let calldata = build_erc20_approve_calldata(VAULT_RELAYER, U256::MAX);
/// assert_eq!(calldata.len(), 68); // 4 (selector) + 32 (address) + 32 (amount)
/// // First 4 bytes are the `approve(address,uint256)` function selector.
/// assert_eq!(&calldata[..4], &alloy_primitives::keccak256(b"approve(address,uint256)")[..4]);
/// ```
#[must_use]
pub fn build_erc20_approve_calldata(spender: Address, amount: U256) -> Vec<u8> {
    let mut buf = Vec::with_capacity(68);
    buf.extend_from_slice(&selector("approve(address,uint256)"));
    buf.extend_from_slice(&abi_address(spender));
    buf.extend_from_slice(&abi_u256(amount));
    buf
}

/// Build calldata for `ERC20.balanceOf(address account)`.
///
/// Useful for encoding a `balanceOf` static call to check a token balance
/// before placing an order.
///
/// # Parameters
///
/// * `account` — the [`Address`] whose balance to query.
///
/// # Returns
///
/// A 36-byte `Vec<u8>`: 4-byte selector + 32-byte address.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::Address;
/// use cow_erc20::build_erc20_balance_of_calldata;
///
/// let calldata = build_erc20_balance_of_calldata(Address::ZERO);
/// assert_eq!(calldata.len(), 36); // 4 (selector) + 32 (address)
/// assert_eq!(&calldata[..4], &alloy_primitives::keccak256(b"balanceOf(address)")[..4]);
/// ```
#[must_use]
pub fn build_erc20_balance_of_calldata(account: Address) -> Vec<u8> {
    let mut buf = Vec::with_capacity(36);
    buf.extend_from_slice(&selector("balanceOf(address)"));
    buf.extend_from_slice(&abi_address(account));
    buf
}

/// Build calldata for `ERC20.allowance(address owner, address spender)`.
///
/// Useful for encoding an `allowance` static call to check existing
/// approval before deciding whether to call `approve`.
///
/// # Parameters
///
/// * `owner` — the token holder's [`Address`].
/// * `spender` — the approved spender's [`Address`].
///
/// # Returns
///
/// A 68-byte `Vec<u8>`: 4-byte selector + 32-byte owner + 32-byte spender.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::Address;
/// use cow_chains::contracts::VAULT_RELAYER;
/// use cow_erc20::build_erc20_allowance_calldata;
///
/// let calldata = build_erc20_allowance_calldata(Address::ZERO, VAULT_RELAYER);
/// assert_eq!(calldata.len(), 68); // 4 (selector) + 32 (owner) + 32 (spender)
/// assert_eq!(&calldata[..4], &alloy_primitives::keccak256(b"allowance(address,address)")[..4]);
/// ```
#[must_use]
pub fn build_erc20_allowance_calldata(owner: Address, spender: Address) -> Vec<u8> {
    let mut buf = Vec::with_capacity(68);
    buf.extend_from_slice(&selector("allowance(address,address)"));
    buf.extend_from_slice(&abi_address(owner));
    buf.extend_from_slice(&abi_address(spender));
    buf
}

/// Build calldata for `ERC20.transfer(address to, uint256 amount)`.
///
/// Encodes a direct `transfer` call to move tokens from the caller to
/// `to`. Unlike [`build_erc20_transfer_from_calldata`], this does not
/// require a prior `approve` because the caller is always `msg.sender`.
///
/// # Parameters
///
/// * `to` — the recipient [`Address`].
/// * `amount` — the token amount to transfer.
///
/// # Returns
///
/// A 68-byte `Vec<u8>`: 4-byte selector + 32-byte address + 32-byte amount.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, U256};
/// use cow_erc20::build_erc20_transfer_calldata;
///
/// let cd = build_erc20_transfer_calldata(Address::ZERO, U256::from(100u64));
/// assert_eq!(cd.len(), 68); // 4 (selector) + 32 (address) + 32 (amount)
/// assert_eq!(&cd[..4], &alloy_primitives::keccak256(b"transfer(address,uint256)")[..4]);
/// ```
#[must_use]
pub fn build_erc20_transfer_calldata(to: Address, amount: U256) -> Vec<u8> {
    let mut buf = Vec::with_capacity(68);
    buf.extend_from_slice(&selector("transfer(address,uint256)"));
    buf.extend_from_slice(&abi_address(to));
    buf.extend_from_slice(&abi_u256(amount));
    buf
}

/// Build calldata for `ERC20.transferFrom(address from, address to, uint256 amount)`.
///
/// Encodes a `transferFrom` call, for use in pre/post-settlement hooks
/// that need to move tokens atomically within the settlement transaction.
///
/// # Parameters
///
/// * `from` — the token holder's [`Address`].
/// * `to` — the recipient [`Address`].
/// * `amount` — the token amount to transfer.
///
/// # Returns
///
/// A 100-byte `Vec<u8>`: 4-byte selector + 3 × 32-byte arguments.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, U256};
/// use cow_erc20::build_erc20_transfer_from_calldata;
///
/// let cd = build_erc20_transfer_from_calldata(Address::ZERO, Address::ZERO, U256::from(100u64));
/// assert_eq!(cd.len(), 100); // 4 + 32 + 32 + 32
/// ```
#[must_use]
pub fn build_erc20_transfer_from_calldata(from: Address, to: Address, amount: U256) -> Vec<u8> {
    let mut buf = Vec::with_capacity(100);
    buf.extend_from_slice(&selector("transferFrom(address,address,uint256)"));
    buf.extend_from_slice(&abi_address(from));
    buf.extend_from_slice(&abi_address(to));
    buf.extend_from_slice(&abi_u256(amount));
    buf
}

/// Build calldata for `ERC20.decimals()` — a 4-byte selector-only call.
///
/// # Returns
///
/// A 4-byte `Vec<u8>` containing the function selector.
///
/// # Example
///
/// ```rust
/// use cow_erc20::build_erc20_decimals_calldata;
///
/// let calldata = build_erc20_decimals_calldata();
/// assert_eq!(calldata.len(), 4);
/// assert_eq!(&calldata, &alloy_primitives::keccak256(b"decimals()")[..4]);
/// ```
#[must_use]
pub fn build_erc20_decimals_calldata() -> Vec<u8> {
    selector("decimals()").to_vec()
}

/// Build calldata for `ERC20.name()` — a 4-byte selector-only call.
///
/// # Returns
///
/// A 4-byte `Vec<u8>` containing the function selector.
///
/// # Example
///
/// ```rust
/// use cow_erc20::build_erc20_name_calldata;
///
/// let calldata = build_erc20_name_calldata();
/// assert_eq!(calldata.len(), 4);
/// assert_eq!(&calldata, &alloy_primitives::keccak256(b"name()")[..4]);
/// ```
#[must_use]
pub fn build_erc20_name_calldata() -> Vec<u8> {
    selector("name()").to_vec()
}

/// Build calldata for `EIP-2612.nonces(address owner)`.
///
/// # Parameters
///
/// * `owner` — the [`Address`] whose permit nonce to query.
///
/// # Returns
///
/// A 36-byte `Vec<u8>`: 4-byte selector + 32-byte address.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::Address;
/// use cow_erc20::build_eip2612_nonces_calldata;
///
/// let calldata = build_eip2612_nonces_calldata(Address::ZERO);
/// assert_eq!(calldata.len(), 36); // 4 (selector) + 32 (address)
/// assert_eq!(&calldata[..4], &alloy_primitives::keccak256(b"nonces(address)")[..4]);
/// ```
#[must_use]
pub fn build_eip2612_nonces_calldata(owner: Address) -> Vec<u8> {
    let mut buf = Vec::with_capacity(36);
    buf.extend_from_slice(&selector("nonces(address)"));
    buf.extend_from_slice(&abi_address(owner));
    buf
}

/// Build calldata for `EIP-2612.version()` — a 4-byte selector-only call.
///
/// # Returns
///
/// A 4-byte `Vec<u8>` containing the function selector.
///
/// # Example
///
/// ```rust
/// use cow_erc20::build_eip2612_version_calldata;
///
/// let calldata = build_eip2612_version_calldata();
/// assert_eq!(calldata.len(), 4);
/// assert_eq!(&calldata, &alloy_primitives::keccak256(b"version()")[..4]);
/// ```
#[must_use]
pub fn build_eip2612_version_calldata() -> Vec<u8> {
    selector("version()").to_vec()
}
