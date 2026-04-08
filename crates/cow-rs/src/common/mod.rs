//! Common utility functions ported from the `TypeScript` SDK `common` package.
//!
//! This module groups cross-cutting concerns shared by every other module in
//! the crate: address handling, provider abstraction, cryptography, logging,
//! contract helpers, and serialisation.
//!
//! # Submodules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`adapter`] | Global provider adapter singleton (`get/set_global_adapter`) |
//! | [`address`] | Address validation, normalisation, and comparison (EVM, BTC, Solana) |
//! | [`contracts`] | `EIP-712` typed-data signing, Balancer Vault role-granting calldata |
//! | [`crypto`] | Private-key validation and normalisation |
//! | [`logging`] | SDK-level logging toggle (`enable_logging`) |
//! | [`serialize`] | Serde helpers for `U256`-as-decimal-string (`json_with_bigint_replacer`) |

pub mod adapter;
pub mod address;
pub mod contracts;
pub mod crypto;
pub mod logging;
pub mod serialize;

pub use adapter::{ProviderAdapter, get_global_adapter, set_global_adapter};
pub use address::{is_co_w_settlement_contract, is_co_w_vault_relayer_contract};
pub use contracts::{
    TypedDataVersion, TypedDataVersionedSigner, ecdsa_sign_typed_data,
    get_int_chain_id_typed_data_v4_signer, get_typed_data_v3_signer,
    get_typed_data_versioned_signer, grant_required_roles,
};
pub use logging::{enable_logging, is_logging_enabled, sdk_log};
pub use serialize::{U256DecString, json_with_bigint_replacer, u256_to_dec_string};
