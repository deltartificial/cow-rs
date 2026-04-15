//! `cow-sdk-cow-shed` — Layer 2 `CowShed` proxy contract helpers for the `CoW` Protocol SDK.

#![deny(unsafe_code)]
#![warn(missing_docs)]

mod sdk;
pub mod types;

pub use sdk::{
    COW_SHED_1_0_0_VERSION, COW_SHED_1_0_1_VERSION, COW_SHED_FACTORY_GNOSIS,
    COW_SHED_FACTORY_MAINNET, COW_SHED_FACTORY_V1_0_0, COW_SHED_FACTORY_V1_0_1,
    COW_SHED_IMPLEMENTATION_V1_0_0, COW_SHED_IMPLEMENTATION_V1_0_1, COW_SHED_LATEST_VERSION,
    COW_SHED_PROXY_CREATION_GAS, CowShedSdk,
};
pub use types::{CowShedCall, CowShedHookParams};
