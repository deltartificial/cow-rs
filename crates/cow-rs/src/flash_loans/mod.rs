//! Flash loan provider helpers and calldata builders.
//!
//! Supports building pre-interaction hooks that trigger flash loans from
//! Balancer, `MakerDAO`, or Aave V3 as part of a `CoW` Protocol order
//! settlement.
//!
//! # Submodules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`types`] | [`FlashLoanProvider`] enum and [`FlashLoanParams`] struct |
//! | `sdk` (private) | [`FlashLoanSdk`] calldata builders and constants |
//!
//! # Quick start
//!
//! ```rust
//! use alloy_primitives::{Address, U256};
//! use cow_rs::flash_loans::{FlashLoanParams, FlashLoanProvider, FlashLoanSdk};
//!
//! let params = FlashLoanParams::new(
//!     FlashLoanProvider::Balancer,
//!     Address::ZERO,        // token to borrow
//!     U256::from(1_000u64), // amount in token atoms
//!     1,                    // Ethereum mainnet
//! );
//!
//! let hook = FlashLoanSdk::build_flash_loan_hook(
//!     &params,
//!     Address::ZERO, // receiver
//!     &[],           // user data
//! );
//! assert!(hook.is_ok());
//! ```

mod sdk;
pub mod types;

pub use sdk::{
    AAVE_ADAPTER_FACTORY, AAVE_POOL_ADDRESS_MAINNET, ADAPTER_DOMAIN_NAME, ADAPTER_DOMAIN_VERSION,
    BASIS_POINTS_SCALE, DEFAULT_VALIDITY, FlashLoanSdk, GAS_ESTIMATION_ADDITION_PERCENT,
    HALF_BASIS_POINTS_SCALE, HASH_ZERO, PERCENT_SCALE,
};
pub use types::{FlashLoanParams, FlashLoanProvider};
