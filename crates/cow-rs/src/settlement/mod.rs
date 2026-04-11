//! Settlement encoding and contract interaction layer for `CoW` Protocol.
//!
//! This module provides tools for building `GPv2Settlement.settle()` calldata,
//! managing Balancer Vault roles, and reading settlement contract state.
//!
//! # Sub-modules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`encoder`] | Build complete `settle()` calldata from orders and interactions |
//! | [`vault`] | Balancer Vault role management (`grantRole` / `revokeRole`) |
//! | [`reader`] | On-chain settlement contract state reading via JSON-RPC |
//! | [`refunds`] | Order refund calldata builders and amount helpers |
//! | [`simulator`] | Trade simulation for gas estimation and revert detection |

pub mod encoder;
pub mod reader;
pub mod refunds;
pub mod simulator;
pub mod vault;
