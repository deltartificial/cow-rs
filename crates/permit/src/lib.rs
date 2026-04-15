//! `cow-permit` — Layer 2 EIP-2612 permit utilities for `CoW` Protocol pre-interaction hooks.
//!
//! This crate ports the `permit-utils` package from the `CoW` Protocol
//! `TypeScript` SDK. It provides everything needed to sign an EIP-2612 permit
//! and encode it as a `CoW` Protocol pre-interaction hook that runs before
//! settlement, atomically granting the Vault Relayer a token allowance.
//!
//! # Submodules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`eip2612`] | EIP-712 domain separator, struct hash, signing, calldata encoding |
//! | [`types`] | [`PermitInfo`], [`Erc20PermitInfo`], [`PermitHookData`] |
//!
//! # Key functions
//!
//! | Function | Purpose |
//! |---|---|
//! | [`build_permit_hook`] | High-level: sign + encode → [`PermitHookData`] |
//! | [`sign_permit`] | Sign a permit → 65-byte `r\|s\|v` signature |
//! | [`build_permit_calldata`] | Encode signed permit → ABI calldata |
//! | [`permit_domain_separator`] | Compute the EIP-712 domain separator |
//! | [`permit_digest`] | Compute the EIP-712 signing digest |
//!
//! # Quick start
//!
//! ```rust,no_run
//! use alloy_primitives::{Address, U256, address};
//! use alloy_signer_local::PrivateKeySigner;
//! use cow_permit::{Erc20PermitInfo, PermitInfo, build_permit_hook};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let signer: PrivateKeySigner =
//!     "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".parse()?;
//!
//! let info = PermitInfo {
//!     token_address: address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
//!     owner: signer.address(),
//!     spender: address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110"),
//!     value: U256::from(1_000_000u64),
//!     nonce: U256::ZERO,
//!     deadline: 9_999_999_999u64,
//! };
//!
//! let erc20_info =
//!     Erc20PermitInfo { name: "USD Coin".to_string(), version: "2".to_string(), chain_id: 1 };
//!
//! let hook = build_permit_hook(&info, &erc20_info, &signer).await?;
//! println!("calldata len: {}", hook.calldata.len());
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod eip2612;
pub mod types;

pub use eip2612::{
    PERMIT_GAS_LIMIT, build_permit_calldata, build_permit_hook, permit_digest,
    permit_domain_separator, permit_type_hash, sign_permit,
};
pub use types::{Erc20PermitInfo, PermitHookData, PermitInfo};
