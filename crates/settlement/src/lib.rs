//! `cow-sdk-settlement` — Layer 2 settlement encoding and on-chain helpers for the `CoW` Protocol
//! SDK.
//!
//! This crate provides tools for building `GPv2Settlement.settle()` calldata,
//! managing Balancer Vault roles, and refund handling for partially filled or
//! expired orders.
//!
//! # Sub-modules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`encoder`] | Build complete `settle()` calldata from orders and interactions |
//! | [`vault`] | Balancer Vault role management (`grantRole` / `revokeRole`) |
//! | [`refunds`] | Order refund calldata builders and amount helpers |
//! | [`simulator`] | Trade simulation for gas estimation and revert detection |
//!
//! The previous `reader` sub-module (on-chain state queries) remains in the
//! legacy `cow-rs` crate until the `onchain` transport crate is extracted,
//! at which point it will move to a dedicated client crate.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod encoder;
pub mod refunds;
pub mod simulator;
pub mod vault;
