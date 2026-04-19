//! `cow-shed` — Layer 2 `CowShed` proxy contract helpers for the `CoW` Protocol SDK.
//!
//! Exposes [`CowShedSdk`] for high-level hook construction plus the
//! [`eip712`] module containing type hashes, struct hashes, and the
//! [`eip712::typed_data_digest`] digest used by [`CowShedSdk::sign_hook`].

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod eip712;
mod sdk;
pub mod types;

pub use eip712::{
    CALL_TYPE, COW_SHED_DOMAIN_NAME, EIP712_DOMAIN_TYPE, EXECUTE_HOOKS_TYPE, call_struct_hash,
    call_type_hash, cow_shed_domain_separator, domain_type_hash, execute_hooks_struct_hash,
    execute_hooks_type_hash, typed_data_digest,
};
pub use sdk::{
    COW_SHED_1_0_0_VERSION, COW_SHED_1_0_1_VERSION, COW_SHED_FACTORY_GNOSIS,
    COW_SHED_FACTORY_MAINNET, COW_SHED_FACTORY_V1_0_0, COW_SHED_FACTORY_V1_0_1,
    COW_SHED_IMPLEMENTATION_V1_0_0, COW_SHED_IMPLEMENTATION_V1_0_1, COW_SHED_LATEST_VERSION,
    COW_SHED_PROXY_CREATION_GAS, CowShedSdk, SignedCowShedHook,
};
pub use types::{CowShedCall, CowShedHookParams};
