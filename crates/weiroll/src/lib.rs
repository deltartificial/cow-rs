//! `cow-sdk-weiroll` — Layer 2 Weiroll script builder for the `CoW` Protocol SDK.
//!
//! [Weiroll](https://github.com/weiroll/weiroll) is a minimal VM for
//! chaining arbitrary EVM calls into a single transaction. This crate
//! provides the Rust types for building Weiroll scripts and encoding them
//! as `execute(bytes32[],bytes[])` calldata targeting the canonical Weiroll
//! executor contract.
//!
//! # Submodules
//!
//! | Module | Purpose |
//! |---|---|
//! | `planner` (private) | [`WeirollPlanner`] builder for accumulating commands/state |
//! | [`types`] | [`WeirollCommand`], [`WeirollScript`], [`WeirollContractRef`], factory functions |
//!
//! # Key items
//!
//! | Item | Purpose |
//! |---|---|
//! | [`WeirollPlanner`] | Builder: `add_command` → `add_state_slot` → `plan()` |
//! | [`WeirollCommand`] | A single 32-byte packed instruction |
//! | [`WeirollScript`] | Finalised script (commands + state) |
//! | [`WeirollContractRef`] | Contract address + ABI + default call flags |
//! | [`create_weiroll_contract`] | Factory for `CALL`-mode contracts |
//! | [`create_weiroll_library`] | Factory for `DELEGATECALL`-mode libraries |
//! | [`create_weiroll_delegate_call`] | Build a complete `execute(...)` [`EvmCall`](cow_sdk_chains::chains::EvmCall) |

#![deny(unsafe_code)]
#![warn(missing_docs)]

mod planner;
pub mod types;

pub use planner::WeirollPlanner;
pub use types::{
    WEIROLL_ADDRESS, WeirollCommand, WeirollCommandFlags, WeirollContractRef, WeirollScript,
    create_weiroll_contract, create_weiroll_delegate_call, create_weiroll_library,
    define_read_only, get_static,
};
