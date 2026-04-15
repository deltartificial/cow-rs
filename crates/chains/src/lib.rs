//! `cow-chains` — Layer 0 chain configuration for the `CoW` Protocol SDK.
//!
//! Centralises all deployment-specific knowledge: which chains are supported,
//! where the protocol contracts live, what the native/wrapped tokens are, and
//! how to reach the orderbook API.
//!
//! This crate sits at Layer 0 of the workspace DAG and has no dependencies on
//! any other internal crate.
//!
//! # Submodules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`chain`] | [`SupportedChainId`], [`Env`], API base URLs, explorer links |
//! | [`chains`] | Extended chain enums ([`EvmChains`], [`NonEvmChains`]), rich [`ChainInfo`] metadata, classification helpers |
//! | [`contracts`] | Protocol contract addresses (`SETTLEMENT_CONTRACT`, `VAULT_RELAYER`, …), `CREATE2` helpers, EIP-1967 proxy slots |
//! | [`tokens`] | Native/wrapped currency constants and per-chain [`TokenInfo`] |
//! | [`params`] | [`CowSwapConfig`] executor configuration and [`TokenRegistry`] |

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod chain;
pub mod chains;
pub mod contracts;
pub mod params;
pub mod tokens;

pub use chain::{Env, SupportedChainId, api_base_url, api_url, order_explorer_link};
pub use chains::{
    AdditionalTargetChainId, AddressPerChain, ApiBaseUrls, ApiContext, ChainContract,
    ChainContracts, ChainInfo, ChainRpcUrls, ChainTokenInfo, EvmCall, EvmChainInfo, EvmChains,
    IpfsConfig, NonEvmChainInfo, NonEvmChains, ProtocolOptions, RAW_CHAINS_FILES_PATH,
    RAW_FILES_PATH, TOKEN_LIST_IMAGES_PATH, TargetChainId, ThemedImage, WebUrl,
    additional_target_chain_info, all_additional_target_chain_ids, all_additional_target_chains,
    all_chain_ids, all_chains, all_supported_chain_ids, all_supported_chains, get_chain_info,
    is_additional_target_chain, is_btc_chain, is_chain_deprecated, is_chain_under_development,
    is_evm_chain, is_evm_chain_info, is_non_evm_chain, is_non_evm_chain_info, is_supported_chain,
    is_target_chain_id, is_zk_sync_chain, map_address_to_supported_networks, map_all_networks,
    map_supported_networks, supported_chain_info, tradable_supported_chain_ids,
    tradable_supported_chains,
};
pub use contracts::{
    BARN_ETH_FLOW, BUY_ETH_ADDRESS, COMPOSABLE_COW, DEPLOYER_CONTRACT, ETH_FLOW_PROD,
    ETH_FLOW_STAGING, EXTENSIBLE_FALLBACK_HANDLER, IMPLEMENTATION_STORAGE_SLOT, MAX_VALID_TO_EPOCH,
    OWNER_STORAGE_SLOT, SALT, SETTLEMENT_CONTRACT, SETTLEMENT_CONTRACT_STAGING, VAULT_RELAYER,
    VAULT_RELAYER_STAGING, composable_cow, composable_cow_contract_address,
    cow_protocol_settlement_contract_address, cow_protocol_vault_relayer_address,
    cow_protocol_vault_relayer_address_staging, deterministic_deployment_address, eth_flow_for_env,
    extensible_fallback_handler, extensible_fallback_handler_contract_address,
    implementation_address_slot, owner_address_slot, settlement_contract,
    settlement_contract_for_env, vault_relayer, vault_relayer_for_env,
};
pub use params::{CowSwapConfig, TokenRegistry};
pub use tokens::{
    BTC_CURRENCY_ADDRESS, EVM_NATIVE_CURRENCY_ADDRESS, NATIVE_CURRENCY_ADDRESS,
    SOL_NATIVE_CURRENCY_ADDRESS, TokenInfo, get_wrapped_token_for_chain, wrapped_native_currency,
};
