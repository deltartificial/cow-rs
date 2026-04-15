//! `cow-sdk-contracts-abi` — Layer 1 ABI bindings for `CoW` Protocol contracts.
//!
//! **Placeholder**: the current SDK hand-encodes calldata in each domain crate
//! (`erc20`, `ethflow`, `weiroll`, ...). This crate will host `alloy::sol!`
//! bindings for the settlement, vault, and composable contracts once the
//! hand-rolled builders are migrated.
//!
//! Keeping it as an empty crate now reserves the name in the workspace and
//! makes the forthcoming migration a drop-in addition rather than a new
//! dependency line.

#![deny(unsafe_code)]
#![warn(missing_docs)]
