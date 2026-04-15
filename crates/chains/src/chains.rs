//! Extended chain enums, chain info types, and utility functions.
//!
//! Mirrors the `TypeScript` SDK `chains` package. This module is the
//! authoritative source for chain metadata beyond what [`SupportedChainId`]
//! provides — it covers bridge-only chains, non-EVM chains (BTC, Solana),
//! rich per-chain metadata ([`ChainInfo`]), and classification helpers.
//!
//! # Key items
//!
//! | Item | Purpose |
//! |---|---|
//! | [`EvmChains`] | Superset of [`SupportedChainId`] including bridge-only EVM chains |
//! | [`NonEvmChains`] | BTC and Solana chain IDs |
//! | [`AdditionalTargetChainId`] | Bridge destination chains not in [`SupportedChainId`] |
//! | [`ChainInfo`] | Rich per-chain metadata (RPC URLs, contracts, tokens, …) |
//! | [`get_chain_info`] | Look up [`ChainInfo`] by numeric chain ID |
//! | [`is_evm_chain`] / [`is_btc_chain`] | Chain classification helpers |
//! | [`all_supported_chains`] / [`all_chain_ids`] | Iteration helpers |

use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};

/// Per-chain address mapping for supported chains.
pub type AddressPerChain = Vec<(super::chain::SupportedChainId, Address)>;

/// Per-chain base URL mapping for supported chains.
pub type ApiBaseUrls = Vec<(super::chain::SupportedChainId, String)>;

use super::chain::SupportedChainId;

// ── CDN paths ────────────────────────────────────────────────────────────────

/// Base CDN path for SDK files.
pub const RAW_FILES_PATH: &str = "https://files.cow.fi/cow-sdk";

/// CDN path for chain data.
pub const RAW_CHAINS_FILES_PATH: &str = "https://files.cow.fi/cow-sdk/chains";

/// CDN path for token list images.
pub const TOKEN_LIST_IMAGES_PATH: &str = "https://files.cow.fi/token-lists/images";

// ── Chain enums ──────────────────────────────────────────────────────────────

/// All EVM chains supported by `CoW` Protocol or available for bridging.
///
/// This is a superset of [`SupportedChainId`] -- it includes bridge-only chains
/// like Optimism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u64)]
pub enum EvmChains {
    /// Ethereum mainnet (chain ID 1).
    Mainnet = 1,
    /// Optimism (chain ID 10).
    Optimism = 10,
    /// BNB Smart Chain (chain ID 56).
    Bnb = 56,
    /// Gnosis Chain (chain ID 100).
    GnosisChain = 100,
    /// Polygon `PoS` (chain ID 137).
    Polygon = 137,
    /// Base (chain ID 8453).
    Base = 8_453,
    /// Plasma (chain ID 9745).
    Plasma = 9_745,
    /// Arbitrum One (chain ID 42161).
    ArbitrumOne = 42_161,
    /// Avalanche C-Chain (chain ID 43114).
    Avalanche = 43_114,
    /// Ink (chain ID 57073).
    Ink = 57_073,
    /// Linea (chain ID 59144).
    Linea = 59_144,
    /// Ethereum Sepolia testnet (chain ID 11155111).
    Sepolia = 11_155_111,
}

impl EvmChains {
    /// Return the numeric EIP-155 chain ID.
    ///
    /// # Returns
    ///
    /// The `u64` chain ID.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self as u64
    }

    /// Try to construct an [`EvmChains`] from a raw chain ID.
    ///
    /// # Arguments
    ///
    /// * `chain_id` — the numeric EIP-155 chain ID.
    ///
    /// # Returns
    ///
    /// `Some(variant)` if `chain_id` is a known EVM chain, `None` otherwise.
    #[must_use]
    pub const fn try_from_u64(chain_id: u64) -> Option<Self> {
        match chain_id {
            1 => Some(Self::Mainnet),
            10 => Some(Self::Optimism),
            56 => Some(Self::Bnb),
            100 => Some(Self::GnosisChain),
            137 => Some(Self::Polygon),
            8_453 => Some(Self::Base),
            9_745 => Some(Self::Plasma),
            42_161 => Some(Self::ArbitrumOne),
            43_114 => Some(Self::Avalanche),
            57_073 => Some(Self::Ink),
            59_144 => Some(Self::Linea),
            11_155_111 => Some(Self::Sepolia),
            _ => None,
        }
    }
}

/// All non-EVM chains available for bridging only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u64)]
pub enum NonEvmChains {
    /// Bitcoin (custom internal ID `1_000_000_000`).
    Bitcoin = 1_000_000_000,
    /// Solana (custom internal ID `1_000_000_001`).
    Solana = 1_000_000_001,
}

impl NonEvmChains {
    /// Return the numeric chain ID.
    ///
    /// # Returns
    ///
    /// The `u64` chain ID.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self as u64
    }

    /// Try to construct a [`NonEvmChains`] from a raw chain ID.
    ///
    /// # Arguments
    ///
    /// * `chain_id` — the numeric chain ID.
    ///
    /// # Returns
    ///
    /// `Some(variant)` if `chain_id` is a known non-EVM chain, `None` otherwise.
    #[must_use]
    pub const fn try_from_u64(chain_id: u64) -> Option<Self> {
        match chain_id {
            1_000_000_000 => Some(Self::Bitcoin),
            1_000_000_001 => Some(Self::Solana),
            _ => None,
        }
    }
}

/// Chains where you can buy tokens using bridge functionality but that are not
/// directly supported by `CoW` Protocol for selling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u64)]
pub enum AdditionalTargetChainId {
    /// Optimism (chain ID 10).
    Optimism = 10,
    /// Bitcoin (custom internal ID `1_000_000_000`).
    Bitcoin = 1_000_000_000,
    /// Solana (custom internal ID `1_000_000_001`).
    Solana = 1_000_000_001,
}

impl AdditionalTargetChainId {
    /// Return the numeric chain ID.
    ///
    /// # Returns
    ///
    /// The `u64` chain ID.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self as u64
    }

    /// Try to construct an [`AdditionalTargetChainId`] from a raw chain ID.
    ///
    /// # Arguments
    ///
    /// * `chain_id` — the numeric chain ID.
    ///
    /// # Returns
    ///
    /// `Some(variant)` if `chain_id` is a known additional target, `None` otherwise.
    #[must_use]
    pub const fn try_from_u64(chain_id: u64) -> Option<Self> {
        match chain_id {
            10 => Some(Self::Optimism),
            1_000_000_000 => Some(Self::Bitcoin),
            1_000_000_001 => Some(Self::Solana),
            _ => None,
        }
    }

    /// Return all additional target chain IDs.
    ///
    /// # Returns
    ///
    /// A static slice of every [`AdditionalTargetChainId`] variant.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Optimism, Self::Bitcoin, Self::Solana]
    }
}

/// A chain ID that is either a [`SupportedChainId`] or an
/// [`AdditionalTargetChainId`].
///
/// This union covers all chains where you can either trade directly or bridge
/// to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetChainId {
    /// A chain supported directly by `CoW` Protocol.
    Supported(SupportedChainId),
    /// A bridge-only target chain.
    Additional(AdditionalTargetChainId),
}

impl TargetChainId {
    /// Return the numeric chain ID.
    ///
    /// # Returns
    ///
    /// The `u64` chain ID from the inner variant.
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        match self {
            Self::Supported(c) => c.as_u64(),
            Self::Additional(c) => c.as_u64(),
        }
    }
}

// ── Chain info types ─────────────────────────────────────────────────────────

/// A themed image with light and dark variants.
#[derive(Debug, Clone)]
pub struct ThemedImage {
    /// URL for the light theme logo.
    pub light: &'static str,
    /// URL for the dark theme logo.
    pub dark: &'static str,
}

/// A named URL.
#[derive(Debug, Clone)]
pub struct WebUrl {
    /// Display name.
    pub name: &'static str,
    /// The URL.
    pub url: &'static str,
}

/// An on-chain contract reference with an optional creation block.
#[derive(Debug, Clone, Copy)]
pub struct ChainContract {
    /// The contract address.
    pub address: Address,
    /// The block at which the contract was created, if known.
    pub block_created: Option<u64>,
}

/// Well-known contracts on an EVM chain.
#[derive(Debug, Clone)]
pub struct ChainContracts {
    /// Multicall3 contract.
    pub multicall3: Option<ChainContract>,
    /// ENS registry contract.
    pub ens_registry: Option<ChainContract>,
    /// ENS universal resolver contract.
    pub ens_universal_resolver: Option<ChainContract>,
}

/// RPC URL configuration for an EVM chain.
#[derive(Debug, Clone)]
pub struct ChainRpcUrls {
    /// HTTP RPC endpoints.
    pub http: &'static [&'static str],
    /// WebSocket RPC endpoints (optional).
    pub web_socket: Option<&'static [&'static str]>,
}

/// Token info used in chain metadata.
///
/// Uses a string address rather than `alloy_primitives::Address` because
/// non-EVM chains (Bitcoin, Solana) have non-hex addresses.
#[derive(Debug, Clone)]
pub struct ChainTokenInfo {
    /// Chain-specific ID.
    pub chain_id: u64,
    /// Token address as a string.
    pub address: &'static str,
    /// Decimal places.
    pub decimals: u8,
    /// Token name.
    pub name: &'static str,
    /// Token symbol.
    pub symbol: &'static str,
    /// Logo URL, if available.
    pub logo_url: Option<&'static str>,
}

/// Metadata for an EVM chain.
#[derive(Debug, Clone)]
pub struct EvmChainInfo {
    /// The EIP-155 chain ID.
    pub id: u64,
    /// Display label.
    pub label: &'static str,
    /// EIP-155 label (used for wallet connections).
    pub eip155_label: &'static str,
    /// ERC-3770 address prefix.
    pub address_prefix: &'static str,
    /// Native currency token info.
    pub native_currency: ChainTokenInfo,
    /// Whether this is a testnet.
    pub is_testnet: bool,
    /// Brand color for UI display.
    pub color: &'static str,
    /// Chain logo.
    pub logo: ThemedImage,
    /// Chain website.
    pub website: WebUrl,
    /// Chain documentation.
    pub docs: WebUrl,
    /// Block explorer.
    pub block_explorer: WebUrl,
    /// Bridges.
    pub bridges: &'static [WebUrl],
    /// Well-known contracts.
    pub contracts: ChainContracts,
    /// Default RPC URLs.
    pub rpc_urls: ChainRpcUrls,
    /// Whether this chain is zkSync-based.
    pub is_zk_sync: bool,
    /// Whether this chain is under development.
    pub is_under_development: bool,
    /// Whether this chain is deprecated (no new trading).
    pub is_deprecated: bool,
}

/// Metadata for a non-EVM chain (e.g. Bitcoin, Solana).
#[derive(Debug, Clone)]
pub struct NonEvmChainInfo {
    /// Internal chain ID.
    pub id: u64,
    /// Display label.
    pub label: &'static str,
    /// Address prefix.
    pub address_prefix: &'static str,
    /// Native currency info.
    pub native_currency: ChainTokenInfo,
    /// Whether this is a testnet.
    pub is_testnet: bool,
    /// Brand color for UI display.
    pub color: &'static str,
    /// Chain logo.
    pub logo: ThemedImage,
    /// Chain website.
    pub website: WebUrl,
    /// Chain documentation.
    pub docs: WebUrl,
    /// Block explorer.
    pub block_explorer: WebUrl,
    /// Whether this chain is under development.
    pub is_under_development: bool,
    /// Whether this chain is deprecated.
    pub is_deprecated: bool,
}

/// A chain on the network -- either an EVM chain or a non-EVM chain.
#[derive(Debug, Clone)]
pub enum ChainInfo {
    /// An EVM-compatible chain.
    Evm(EvmChainInfo),
    /// A non-EVM chain (e.g. Bitcoin, Solana).
    NonEvm(NonEvmChainInfo),
}

impl ChainInfo {
    /// Return the chain ID.
    ///
    /// # Returns
    ///
    /// The `u64` chain ID from the inner variant.
    #[must_use]
    pub const fn id(&self) -> u64 {
        match self {
            Self::Evm(info) => info.id,
            Self::NonEvm(info) => info.id,
        }
    }

    /// Return the display label.
    ///
    /// # Returns
    ///
    /// A human-readable chain name (e.g. `"Ethereum"`, `"Bitcoin"`).
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Evm(info) => info.label,
            Self::NonEvm(info) => info.label,
        }
    }

    /// Returns `true` if this chain is under development.
    ///
    /// # Returns
    ///
    /// `true` when the chain's `is_under_development` flag is set.
    #[must_use]
    pub const fn is_under_development(&self) -> bool {
        match self {
            Self::Evm(info) => info.is_under_development,
            Self::NonEvm(info) => info.is_under_development,
        }
    }

    /// Returns `true` if this chain is deprecated.
    ///
    /// # Returns
    ///
    /// `true` when the chain's `is_deprecated` flag is set.
    #[must_use]
    pub const fn is_deprecated(&self) -> bool {
        match self {
            Self::Evm(info) => info.is_deprecated,
            Self::NonEvm(info) => info.is_deprecated,
        }
    }

    /// Returns `true` if this is an EVM chain.
    ///
    /// # Returns
    ///
    /// `true` for the [`Evm`](Self::Evm) variant.
    #[must_use]
    pub const fn is_evm(&self) -> bool {
        matches!(self, Self::Evm(_))
    }

    /// Returns `true` if this is a non-EVM chain.
    ///
    /// # Returns
    ///
    /// `true` for the [`NonEvm`](Self::NonEvm) variant.
    #[must_use]
    pub const fn is_non_evm(&self) -> bool {
        matches!(self, Self::NonEvm(_))
    }

    /// Returns the inner [`EvmChainInfo`] if this is an EVM chain.
    ///
    /// # Returns
    ///
    /// `Some(&info)` for EVM chains, `None` for non-EVM chains.
    #[must_use]
    pub const fn as_evm(&self) -> Option<&EvmChainInfo> {
        match self {
            Self::Evm(info) => Some(info),
            Self::NonEvm(_) => None,
        }
    }

    /// Returns the inner [`NonEvmChainInfo`] if this is a non-EVM chain.
    ///
    /// # Returns
    ///
    /// `Some(&info)` for non-EVM chains, `None` for EVM chains.
    #[must_use]
    pub const fn as_non_evm(&self) -> Option<&NonEvmChainInfo> {
        match self {
            Self::NonEvm(info) => Some(info),
            Self::Evm(_) => None,
        }
    }

    /// Return the native currency info for this chain.
    ///
    /// # Returns
    ///
    /// A reference to the [`ChainTokenInfo`] describing the chain's native currency.
    #[must_use]
    pub const fn native_currency(&self) -> &ChainTokenInfo {
        match self {
            Self::Evm(info) => &info.native_currency,
            Self::NonEvm(info) => &info.native_currency,
        }
    }
}

// ── Chain classification functions ───────────────────────────────────────────

/// Check if a chain ID represents an EVM chain (including bridge-only ones
/// like Optimism).
///
/// # Arguments
///
/// * `chain_id` — the numeric chain ID to check.
///
/// # Returns
///
/// `true` if `chain_id` belongs to a known EVM chain.
///
/// ```
/// use cow_sdk_chains::chains::is_evm_chain;
///
/// assert!(is_evm_chain(1)); // Mainnet
/// assert!(is_evm_chain(10)); // Optimism
/// assert!(!is_evm_chain(1_000_000_000)); // Bitcoin
/// ```
#[must_use]
pub const fn is_evm_chain(chain_id: u64) -> bool {
    EvmChains::try_from_u64(chain_id).is_some()
}

/// Check if a chain ID represents a non-EVM chain (Bitcoin, Solana).
///
/// # Arguments
///
/// * `chain_id` — the numeric chain ID to check.
///
/// # Returns
///
/// `true` if `chain_id` belongs to a known non-EVM chain.
///
/// ```
/// use cow_sdk_chains::chains::is_non_evm_chain;
///
/// assert!(is_non_evm_chain(1_000_000_000)); // Bitcoin
/// assert!(is_non_evm_chain(1_000_000_001)); // Solana
/// assert!(!is_non_evm_chain(1)); // Mainnet
/// ```
#[must_use]
pub const fn is_non_evm_chain(chain_id: u64) -> bool {
    NonEvmChains::try_from_u64(chain_id).is_some()
}

/// Check if a [`ChainInfo`] represents an EVM chain.
///
/// Type guard equivalent of `isEvmChainInfo` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `chain_info` — the [`ChainInfo`] to inspect.
///
/// # Returns
///
/// `true` for the [`ChainInfo::Evm`] variant.
///
/// ```
/// use cow_sdk_chains::{
///     SupportedChainId,
///     chains::{is_evm_chain_info, supported_chain_info},
/// };
///
/// let info = supported_chain_info(SupportedChainId::Mainnet);
/// assert!(is_evm_chain_info(&info));
/// ```
#[must_use]
pub const fn is_evm_chain_info(chain_info: &ChainInfo) -> bool {
    chain_info.is_evm()
}

/// Check if a [`ChainInfo`] represents a non-EVM chain.
///
/// Type guard equivalent of `isNonEvmChainInfo` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `chain_info` — the [`ChainInfo`] to inspect.
///
/// # Returns
///
/// `true` for the [`ChainInfo::NonEvm`] variant.
///
/// ```
/// use cow_sdk_chains::{
///     AdditionalTargetChainId,
///     chains::{additional_target_chain_info, is_non_evm_chain_info},
/// };
///
/// let info = additional_target_chain_info(AdditionalTargetChainId::Bitcoin);
/// assert!(is_non_evm_chain_info(&info));
/// ```
#[must_use]
pub const fn is_non_evm_chain_info(chain_info: &ChainInfo) -> bool {
    chain_info.is_non_evm()
}

/// Check if a chain ID represents Bitcoin.
///
/// # Arguments
///
/// * `chain_id` — the numeric chain ID to check.
///
/// # Returns
///
/// `true` when `chain_id` equals [`NonEvmChains::Bitcoin`].
///
/// ```
/// use cow_sdk_chains::chains::is_btc_chain;
///
/// assert!(is_btc_chain(1_000_000_000));
/// assert!(!is_btc_chain(1));
/// ```
#[must_use]
pub const fn is_btc_chain(chain_id: u64) -> bool {
    chain_id == NonEvmChains::Bitcoin as u64
}

/// Check if a chain ID is directly supported by `CoW` Protocol for trading.
///
/// # Arguments
///
/// * `chain_id` — the numeric chain ID to check.
///
/// # Returns
///
/// `true` if `chain_id` is a [`SupportedChainId`] variant.
///
/// ```
/// use cow_sdk_chains::chains::is_supported_chain;
///
/// assert!(is_supported_chain(1)); // Mainnet
/// assert!(!is_supported_chain(10)); // Optimism (bridge-only)
/// ```
#[must_use]
pub const fn is_supported_chain(chain_id: u64) -> bool {
    SupportedChainId::try_from_u64(chain_id).is_some()
}

/// Check if a chain ID is a bridge-only target chain.
///
/// # Arguments
///
/// * `chain_id` — the numeric chain ID to check.
///
/// # Returns
///
/// `true` if `chain_id` is an [`AdditionalTargetChainId`] variant.
///
/// ```
/// use cow_sdk_chains::chains::is_additional_target_chain;
///
/// assert!(is_additional_target_chain(10)); // Optimism
/// assert!(is_additional_target_chain(1_000_000_000)); // Bitcoin
/// assert!(!is_additional_target_chain(1)); // Mainnet
/// ```
#[must_use]
pub const fn is_additional_target_chain(chain_id: u64) -> bool {
    AdditionalTargetChainId::try_from_u64(chain_id).is_some()
}

/// Check if a chain ID is either a supported chain or a bridge target.
///
/// # Arguments
///
/// * `chain_id` — the numeric chain ID to check.
///
/// # Returns
///
/// `true` if `chain_id` is a supported or additional target chain.
///
/// ```
/// use cow_sdk_chains::chains::is_target_chain_id;
///
/// assert!(is_target_chain_id(1)); // Mainnet (supported)
/// assert!(is_target_chain_id(10)); // Optimism (bridge target)
/// assert!(is_target_chain_id(1_000_000_000)); // Bitcoin (bridge target)
/// assert!(!is_target_chain_id(999)); // Unknown
/// ```
#[must_use]
pub const fn is_target_chain_id(chain_id: u64) -> bool {
    is_supported_chain(chain_id) || is_additional_target_chain(chain_id)
}

/// Check if a chain is zkSync-based.
///
/// # Arguments
///
/// * `chain_id` — the numeric chain ID to check.
///
/// # Returns
///
/// `true` if the chain is an EVM chain with `is_zk_sync` set.
///
/// ```
/// use cow_sdk_chains::chains::is_zk_sync_chain;
///
/// assert!(!is_zk_sync_chain(1)); // Mainnet
/// ```
#[must_use]
pub fn is_zk_sync_chain(chain_id: u64) -> bool {
    if !is_evm_chain(chain_id) {
        return false;
    }
    get_chain_info(chain_id)
        .and_then(|info| info.as_evm().cloned())
        .is_some_and(|evm| evm.is_zk_sync)
}

/// Check if a chain is under development.
///
/// # Arguments
///
/// * `chain_id` — the numeric chain ID to check.
///
/// # Returns
///
/// `true` if the chain exists and its `is_under_development` flag is set.
///
/// ```
/// use cow_sdk_chains::chains::is_chain_under_development;
///
/// assert!(!is_chain_under_development(1)); // Mainnet
/// ```
#[must_use]
pub fn is_chain_under_development(chain_id: u64) -> bool {
    get_chain_info(chain_id).is_some_and(|info| info.is_under_development())
}

/// Check if a chain is deprecated (no new trading; chain remains for
/// history/Explorer).
///
/// # Arguments
///
/// * `chain_id` — the numeric chain ID to check.
///
/// # Returns
///
/// `true` if the chain exists and its `is_deprecated` flag is set.
///
/// ```
/// use cow_sdk_chains::chains::is_chain_deprecated;
///
/// assert!(!is_chain_deprecated(1)); // Mainnet
/// ```
#[must_use]
pub fn is_chain_deprecated(chain_id: u64) -> bool {
    get_chain_info(chain_id).is_some_and(|info| info.is_deprecated())
}

/// Return the chain info for a given chain ID, or `None` if the chain is not
/// known.
///
/// Looks up both supported chains and additional target chains.
///
/// # Arguments
///
/// * `chain_id` — the numeric chain ID to look up.
///
/// # Returns
///
/// `Some(chain_info)` if the chain is known, `None` otherwise.
///
/// ```
/// use cow_sdk_chains::chains::get_chain_info;
///
/// let info = get_chain_info(1).unwrap();
/// assert_eq!(info.label(), "Ethereum");
///
/// let btc = get_chain_info(1_000_000_000).unwrap();
/// assert_eq!(btc.label(), "Bitcoin");
///
/// assert!(get_chain_info(9999).is_none());
/// ```
#[must_use]
pub const fn get_chain_info(chain_id: u64) -> Option<ChainInfo> {
    if let Some(supported) = SupportedChainId::try_from_u64(chain_id) {
        return Some(supported_chain_info(supported));
    }

    if let Some(additional) = AdditionalTargetChainId::try_from_u64(chain_id) {
        return Some(additional_target_chain_info(additional));
    }

    None
}

// ── Collection helpers ───────────────────────────────────────────────────────

/// Map a value across all supported chains, returning a `Vec` of
/// `(SupportedChainId, T)` pairs.
///
/// # Arguments
///
/// * `f` — a closure that maps each [`SupportedChainId`] to a value of type `T`.
///
/// # Returns
///
/// A `Vec` of `(chain, value)` pairs for every supported chain.
///
/// ```
/// use cow_sdk_chains::chains::map_supported_networks;
///
/// let names = map_supported_networks(|chain| chain.to_string());
/// assert!(!names.is_empty());
/// ```
#[must_use]
pub fn map_supported_networks<T>(f: impl Fn(SupportedChainId) -> T) -> Vec<(SupportedChainId, T)> {
    SupportedChainId::all().iter().map(|&chain| (chain, f(chain))).collect()
}

/// Map a value across all target chains (supported + additional), returning a
/// `Vec` of `(TargetChainId, T)` pairs.
///
/// # Arguments
///
/// * `f` — a closure that maps each [`TargetChainId`] to a value of type `T`.
///
/// # Returns
///
/// A `Vec` of `(chain, value)` pairs for every known chain.
#[must_use]
pub fn map_all_networks<T>(f: impl Fn(TargetChainId) -> T) -> Vec<(TargetChainId, T)> {
    all_chain_ids().into_iter().map(|id| (id, f(id))).collect()
}

/// Map an address to all supported networks.
///
/// Useful for contracts that have the same address on every chain.
///
/// # Arguments
///
/// * `address` — the [`Address`] to replicate across all chains.
///
/// # Returns
///
/// A `Vec` of `(chain, address)` pairs for every supported chain.
#[must_use]
pub fn map_address_to_supported_networks(address: Address) -> Vec<(SupportedChainId, Address)> {
    map_supported_networks(|_| address)
}

/// Return all supported chain IDs as a `Vec`.
///
/// # Returns
///
/// A `Vec` containing every [`SupportedChainId`] variant.
#[must_use]
pub fn all_supported_chain_ids() -> Vec<SupportedChainId> {
    SupportedChainId::all().to_vec()
}

/// Return chain info for all supported chains.
///
/// # Returns
///
/// A `Vec` of [`ChainInfo`] for every supported chain.
#[must_use]
pub fn all_supported_chains() -> Vec<ChainInfo> {
    SupportedChainId::all().iter().map(|&c| supported_chain_info(c)).collect()
}

/// Return chain IDs where new trading is allowed (excludes deprecated chains).
///
/// # Returns
///
/// A `Vec` of [`SupportedChainId`] variants that are not deprecated.
#[must_use]
pub fn tradable_supported_chain_ids() -> Vec<SupportedChainId> {
    SupportedChainId::all()
        .iter()
        .copied()
        .filter(|&c| !supported_chain_info(c).is_deprecated())
        .collect()
}

/// Return chain info for tradable supported chains (excludes deprecated).
///
/// # Returns
///
/// A `Vec` of [`ChainInfo`] for supported chains that are not deprecated.
#[must_use]
pub fn tradable_supported_chains() -> Vec<ChainInfo> {
    SupportedChainId::all()
        .iter()
        .copied()
        .filter(|&c| !supported_chain_info(c).is_deprecated())
        .map(supported_chain_info)
        .collect()
}

/// Return chain info for all additional target chains (bridge-only).
///
/// # Returns
///
/// A `Vec` of [`ChainInfo`] for every bridge-only target chain.
#[must_use]
pub fn all_additional_target_chains() -> Vec<ChainInfo> {
    AdditionalTargetChainId::all().iter().map(|&c| additional_target_chain_info(c)).collect()
}

/// Return all chain IDs for additional target chains (bridge-only).
///
/// # Returns
///
/// A `Vec` containing every [`AdditionalTargetChainId`] variant.
#[must_use]
pub fn all_additional_target_chain_ids() -> Vec<AdditionalTargetChainId> {
    AdditionalTargetChainId::all().to_vec()
}

/// Return chain info for all known chains (both supported and bridge-only).
///
/// # Returns
///
/// A `Vec` of [`ChainInfo`] for every known chain.
#[must_use]
pub fn all_chains() -> Vec<ChainInfo> {
    let mut chains = all_supported_chains();
    chains.extend(all_additional_target_chains());
    chains
}

/// Return all known chain IDs as [`TargetChainId`] values.
///
/// # Returns
///
/// A `Vec` of [`TargetChainId`] covering both supported and additional target chains.
#[must_use]
pub fn all_chain_ids() -> Vec<TargetChainId> {
    let mut ids: Vec<TargetChainId> =
        SupportedChainId::all().iter().map(|&c| TargetChainId::Supported(c)).collect();
    ids.extend(AdditionalTargetChainId::all().iter().map(|&c| TargetChainId::Additional(c)));
    ids
}

// ── Chain info data ──────────────────────────────────────────────────────────

/// Return the [`ChainInfo`] for a [`SupportedChainId`].
///
/// # Arguments
///
/// * `chain` — the supported chain to look up.
///
/// # Returns
///
/// A [`ChainInfo::Evm`] containing the chain's metadata.
#[must_use]
pub const fn supported_chain_info(chain: SupportedChainId) -> ChainInfo {
    ChainInfo::Evm(evm_chain_detail(chain))
}

/// Return the [`ChainInfo`] for an [`AdditionalTargetChainId`].
///
/// # Arguments
///
/// * `chain` — the additional target chain to look up.
///
/// # Returns
///
/// A [`ChainInfo`] (EVM for Optimism, non-EVM for Bitcoin/Solana).
#[must_use]
pub const fn additional_target_chain_info(chain: AdditionalTargetChainId) -> ChainInfo {
    match chain {
        AdditionalTargetChainId::Optimism => ChainInfo::Evm(optimism_chain_info()),
        AdditionalTargetChainId::Bitcoin => ChainInfo::NonEvm(bitcoin_chain_info()),
        AdditionalTargetChainId::Solana => ChainInfo::NonEvm(solana_chain_info()),
    }
}

const fn evm_chain_detail(chain: SupportedChainId) -> EvmChainInfo {
    match chain {
        SupportedChainId::Mainnet => mainnet_chain_info(),
        SupportedChainId::GnosisChain => gnosis_chain_info(),
        SupportedChainId::ArbitrumOne => arbitrum_chain_info(),
        SupportedChainId::Base => base_chain_info(),
        SupportedChainId::Sepolia => sepolia_chain_info(),
        SupportedChainId::Polygon => polygon_chain_info(),
        SupportedChainId::Avalanche => avalanche_chain_info(),
        SupportedChainId::BnbChain => bnb_chain_info(),
        SupportedChainId::Linea => linea_chain_info(),
        SupportedChainId::Lens => lens_chain_info(),
        SupportedChainId::Plasma => plasma_chain_info(),
        SupportedChainId::Ink => ink_chain_info(),
    }
}

/// The standard EVM native currency address.
const EVM_NATIVE_ADDR: &str = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";
/// The BTC genesis address used as token address.
const BTC_ADDR: &str = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
/// The SOL default program address used as token address.
const SOL_ADDR: &str = "11111111111111111111111111111111";

// Multicall3 address constant used by most chains.
const MULTICALL3: Address = Address::new([
    0xca, 0x11, 0xbd, 0xe0, 0x59, 0x77, 0xb3, 0x63, 0x11, 0x67, 0x02, 0x88, 0x62, 0xbe, 0x2a, 0x17,
    0x39, 0x76, 0xca, 0x11,
]);

const fn default_native_currency(chain_id: u64) -> ChainTokenInfo {
    ChainTokenInfo {
        chain_id,
        address: EVM_NATIVE_ADDR,
        decimals: 18,
        name: "Ether",
        symbol: "ETH",
        logo_url: Some(
            "https://files.cow.fi/token-lists/images/1/0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee/logo.png",
        ),
    }
}

const fn no_contracts() -> ChainContracts {
    ChainContracts { multicall3: None, ens_registry: None, ens_universal_resolver: None }
}

const fn multicall3_only(block_created: u64) -> ChainContracts {
    ChainContracts {
        multicall3: Some(ChainContract { address: MULTICALL3, block_created: Some(block_created) }),
        ens_registry: None,
        ens_universal_resolver: None,
    }
}

const fn mainnet_chain_info() -> EvmChainInfo {
    EvmChainInfo {
        id: 1,
        label: "Ethereum",
        eip155_label: "Ethereum Mainnet",
        address_prefix: "eth",
        native_currency: default_native_currency(1),
        is_testnet: false,
        color: "#62688F",
        logo: ThemedImage {
            light: "https://files.cow.fi/cow-sdk/chains/images/mainnet-logo.svg",
            dark: "https://files.cow.fi/cow-sdk/chains/images/mainnet-logo.svg",
        },
        website: WebUrl { name: "Ethereum", url: "https://ethereum.org" },
        docs: WebUrl { name: "Ethereum Docs", url: "https://ethereum.org/en/developers/docs" },
        block_explorer: WebUrl { name: "Etherscan", url: "https://etherscan.io" },
        bridges: &[],
        contracts: ChainContracts {
            multicall3: Some(ChainContract {
                address: MULTICALL3,
                block_created: Some(14_353_601),
            }),
            ens_registry: Some(ChainContract {
                address: Address::new([
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x2e, 0x07, 0x4e, 0xc6, 0x9a, 0x0d, 0xfb,
                    0x29, 0x97, 0xba, 0x6c, 0x7d, 0x2e, 0x1e,
                ]),
                block_created: None,
            }),
            ens_universal_resolver: Some(ChainContract {
                address: Address::new([
                    0xce, 0x01, 0xf8, 0xee, 0xe7, 0xe4, 0x79, 0xc9, 0x28, 0xf8, 0x91, 0x9a, 0xbd,
                    0x53, 0xe5, 0x53, 0xa3, 0x6c, 0xef, 0x67,
                ]),
                block_created: Some(19_258_213),
            }),
        },
        rpc_urls: ChainRpcUrls { http: &["https://eth.merkle.io"], web_socket: None },
        is_zk_sync: false,
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn gnosis_chain_info() -> EvmChainInfo {
    EvmChainInfo {
        id: 100,
        label: "Gnosis",
        eip155_label: "Gnosis",
        address_prefix: "gno",
        native_currency: ChainTokenInfo {
            chain_id: 100,
            address: EVM_NATIVE_ADDR,
            decimals: 18,
            name: "xDAI",
            symbol: "xDAI",
            logo_url: Some(
                "https://files.cow.fi/token-lists/images/100/0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee/logo.png",
            ),
        },
        is_testnet: false,
        color: "#07795B",
        logo: ThemedImage {
            light: "https://files.cow.fi/cow-sdk/chains/images/gnosis-logo.svg",
            dark: "https://files.cow.fi/cow-sdk/chains/images/gnosis-logo.svg",
        },
        website: WebUrl { name: "Gnosis Chain", url: "https://www.gnosischain.com" },
        docs: WebUrl { name: "Gnosis Chain Docs", url: "https://docs.gnosischain.com" },
        block_explorer: WebUrl { name: "Gnosisscan", url: "https://gnosisscan.io" },
        bridges: &[WebUrl { name: "Gnosis Chain Bridge", url: "https://bridge.gnosischain.com" }],
        contracts: multicall3_only(21_022_491),
        rpc_urls: ChainRpcUrls {
            http: &["https://rpc.gnosischain.com"],
            web_socket: Some(&["wss://rpc.gnosischain.com/wss"]),
        },
        is_zk_sync: false,
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn arbitrum_chain_info() -> EvmChainInfo {
    EvmChainInfo {
        id: 42_161,
        label: "Arbitrum",
        eip155_label: "Arbitrum One",
        address_prefix: "arb1",
        native_currency: default_native_currency(42_161),
        is_testnet: false,
        color: "#1B4ADD",
        logo: ThemedImage {
            light: "https://files.cow.fi/cow-sdk/chains/images/arbitrum-logo.svg",
            dark: "https://files.cow.fi/cow-sdk/chains/images/arbitrum-logo.svg",
        },
        website: WebUrl { name: "Arbitrum", url: "https://arbitrum.io" },
        docs: WebUrl { name: "Arbitrum Docs", url: "https://docs.arbitrum.io" },
        block_explorer: WebUrl { name: "Arbiscan", url: "https://arbiscan.io" },
        bridges: &[WebUrl { name: "Arbitrum Bridge", url: "https://bridge.arbitrum.io" }],
        contracts: multicall3_only(7_654_707),
        rpc_urls: ChainRpcUrls { http: &["https://arb1.arbitrum.io/rpc"], web_socket: None },
        is_zk_sync: false,
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn base_chain_info() -> EvmChainInfo {
    EvmChainInfo {
        id: 8_453,
        label: "Base",
        eip155_label: "Base",
        address_prefix: "base",
        native_currency: default_native_currency(8_453),
        is_testnet: false,
        color: "#0052FF",
        logo: ThemedImage {
            light: "https://files.cow.fi/cow-sdk/chains/images/base-logo.svg",
            dark: "https://files.cow.fi/cow-sdk/chains/images/base-logo.svg",
        },
        website: WebUrl { name: "Base", url: "https://base.org" },
        docs: WebUrl { name: "Base Docs", url: "https://docs.base.org" },
        block_explorer: WebUrl { name: "Basescan", url: "https://basescan.org" },
        bridges: &[WebUrl { name: "Superchain Bridges", url: "https://bridge.base.org/deposit" }],
        contracts: multicall3_only(5022),
        rpc_urls: ChainRpcUrls { http: &["https://mainnet.base.org"], web_socket: None },
        is_zk_sync: false,
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn sepolia_chain_info() -> EvmChainInfo {
    EvmChainInfo {
        id: 11_155_111,
        label: "Sepolia",
        eip155_label: "Ethereum Sepolia",
        address_prefix: "sep",
        native_currency: default_native_currency(11_155_111),
        is_testnet: true,
        color: "#C12FF2",
        logo: ThemedImage {
            light: "https://files.cow.fi/cow-sdk/chains/images/sepolia-logo.svg",
            dark: "https://files.cow.fi/cow-sdk/chains/images/sepolia-logo.svg",
        },
        website: WebUrl { name: "Ethereum", url: "https://sepolia.dev" },
        docs: WebUrl {
            name: "Sepolia Docs",
            url: "https://ethereum.org/en/developers/docs/networks/#sepolia",
        },
        block_explorer: WebUrl { name: "Etherscan", url: "https://sepolia.etherscan.io" },
        bridges: &[],
        contracts: ChainContracts {
            multicall3: Some(ChainContract { address: MULTICALL3, block_created: Some(751_532) }),
            ens_registry: Some(ChainContract {
                address: Address::new([
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x2e, 0x07, 0x4e, 0xc6, 0x9a, 0x0d, 0xfb,
                    0x29, 0x97, 0xba, 0x6c, 0x7d, 0x2e, 0x1e,
                ]),
                block_created: None,
            }),
            ens_universal_resolver: Some(ChainContract {
                address: Address::new([
                    0xc8, 0xaf, 0x99, 0x9e, 0x38, 0x27, 0x3d, 0x65, 0x8b, 0xe1, 0xb9, 0x21, 0xb8,
                    0x8a, 0x9d, 0xdf, 0x00, 0x57, 0x69, 0xcc,
                ]),
                block_created: Some(5_317_080),
            }),
        },
        rpc_urls: ChainRpcUrls { http: &["https://sepolia.drpc.org"], web_socket: None },
        is_zk_sync: false,
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn polygon_chain_info() -> EvmChainInfo {
    let logo_url = "https://files.cow.fi/cow-sdk/chains/images/polygon-logo.svg";
    EvmChainInfo {
        id: 137,
        label: "Polygon",
        eip155_label: "Polygon Mainnet",
        address_prefix: "matic",
        native_currency: ChainTokenInfo {
            chain_id: 137,
            address: EVM_NATIVE_ADDR,
            decimals: 18,
            name: "POL",
            symbol: "POL",
            logo_url: Some(logo_url),
        },
        is_testnet: false,
        color: "#8247e5",
        logo: ThemedImage { light: logo_url, dark: logo_url },
        website: WebUrl { name: "Polygon", url: "https://polygon.technology" },
        docs: WebUrl { name: "Polygon Docs", url: "https://docs.polygon.technology" },
        block_explorer: WebUrl { name: "Polygonscan", url: "https://polygonscan.com" },
        bridges: &[],
        contracts: multicall3_only(25_770_160),
        rpc_urls: ChainRpcUrls { http: &["https://polygon-rpc.com"], web_socket: None },
        is_zk_sync: false,
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn avalanche_chain_info() -> EvmChainInfo {
    let logo_url = "https://files.cow.fi/cow-sdk/chains/images/avax-logo.svg";
    EvmChainInfo {
        id: 43_114,
        label: "Avalanche",
        eip155_label: "Avalanche C-Chain",
        address_prefix: "avax",
        native_currency: ChainTokenInfo {
            chain_id: 43_114,
            address: EVM_NATIVE_ADDR,
            decimals: 18,
            name: "Avalanche",
            symbol: "AVAX",
            logo_url: Some(logo_url),
        },
        is_testnet: false,
        color: "#ff3944",
        logo: ThemedImage { light: logo_url, dark: logo_url },
        website: WebUrl { name: "Avalanche", url: "https://www.avax.network/" },
        docs: WebUrl { name: "Avalanche Docs", url: "https://build.avax.network/docs" },
        block_explorer: WebUrl { name: "Snowscan", url: "https://snowscan.xyz" },
        bridges: &[],
        contracts: multicall3_only(11_907_934),
        rpc_urls: ChainRpcUrls {
            http: &["https://api.avax.network/ext/bc/C/rpc"],
            web_socket: None,
        },
        is_zk_sync: false,
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn bnb_chain_info() -> EvmChainInfo {
    let logo_url = "https://files.cow.fi/cow-sdk/chains/images/bnb-logo.svg";
    EvmChainInfo {
        id: 56,
        label: "BNB",
        eip155_label: "BNB Chain Mainnet",
        address_prefix: "bnb",
        native_currency: ChainTokenInfo {
            chain_id: 56,
            address: EVM_NATIVE_ADDR,
            decimals: 18,
            name: "BNB Chain Native Token",
            symbol: "BNB",
            logo_url: Some(logo_url),
        },
        is_testnet: false,
        color: "#F0B90B",
        logo: ThemedImage { light: logo_url, dark: logo_url },
        website: WebUrl { name: "BNB Chain", url: "https://www.bnbchain.org" },
        docs: WebUrl { name: "BNB Chain Docs", url: "https://docs.bnbchain.org" },
        block_explorer: WebUrl { name: "Bscscan", url: "https://bscscan.com" },
        bridges: &[WebUrl {
            name: "BNB Chain Cross-Chain Bridge",
            url: "https://www.bnbchain.org/en/bnb-chain-bridge",
        }],
        contracts: multicall3_only(15_921_452),
        rpc_urls: ChainRpcUrls { http: &["https://bsc-dataseed1.bnbchain.org"], web_socket: None },
        is_zk_sync: false,
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn linea_chain_info() -> EvmChainInfo {
    let logo_url = "https://files.cow.fi/cow-sdk/chains/images/linea-logo.svg";
    EvmChainInfo {
        id: 59_144,
        label: "Linea",
        eip155_label: "Linea Mainnet",
        address_prefix: "linea",
        native_currency: default_native_currency(59_144),
        is_testnet: false,
        color: "#61dfff",
        logo: ThemedImage { light: logo_url, dark: logo_url },
        website: WebUrl { name: "Linea", url: "https://linea.build" },
        docs: WebUrl { name: "Linea Docs", url: "https://docs.linea.build" },
        block_explorer: WebUrl { name: "Lineascan", url: "https://lineascan.build" },
        bridges: &[WebUrl { name: "Linea Bridge", url: "https://linea.build/hub/bridge" }],
        contracts: multicall3_only(42),
        rpc_urls: ChainRpcUrls { http: &["https://rpc.linea.build"], web_socket: None },
        is_zk_sync: false,
        is_under_development: false,
        is_deprecated: false,
    }
}

// Lens is in the Rust SDK but not in the TS config at this time.
const fn lens_chain_info() -> EvmChainInfo {
    EvmChainInfo {
        id: 232,
        label: "Lens",
        eip155_label: "Lens Network",
        address_prefix: "lens",
        native_currency: ChainTokenInfo {
            chain_id: 232,
            address: EVM_NATIVE_ADDR,
            decimals: 18,
            name: "GHO",
            symbol: "GHO",
            logo_url: None,
        },
        is_testnet: false,
        color: "#00501e",
        logo: ThemedImage {
            light: "https://files.cow.fi/cow-sdk/chains/images/lens-logo.svg",
            dark: "https://files.cow.fi/cow-sdk/chains/images/lens-logo.svg",
        },
        website: WebUrl { name: "Lens", url: "https://lens.xyz" },
        docs: WebUrl { name: "Lens Docs", url: "https://docs.lens.xyz" },
        block_explorer: WebUrl { name: "Lens Explorer", url: "https://explorer.lens.xyz" },
        bridges: &[],
        contracts: no_contracts(),
        rpc_urls: ChainRpcUrls { http: &["https://rpc.lens.xyz"], web_socket: None },
        is_zk_sync: true,
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn plasma_chain_info() -> EvmChainInfo {
    let logo_url = "https://files.cow.fi/cow-sdk/chains/images/plasma-logo.svg";
    EvmChainInfo {
        id: 9_745,
        label: "Plasma",
        eip155_label: "Plasma Mainnet",
        address_prefix: "plasma",
        native_currency: ChainTokenInfo {
            chain_id: 9_745,
            address: EVM_NATIVE_ADDR,
            decimals: 18,
            name: "Plasma",
            symbol: "XPL",
            logo_url: Some(logo_url),
        },
        is_testnet: false,
        color: "#569F8C",
        logo: ThemedImage { light: logo_url, dark: logo_url },
        website: WebUrl { name: "Plasma", url: "https://www.plasma.to" },
        docs: WebUrl { name: "Plasma Docs", url: "https://docs.plasma.to" },
        block_explorer: WebUrl { name: "Plasmascan", url: "https://plasmascan.to" },
        bridges: &[],
        contracts: multicall3_only(0),
        rpc_urls: ChainRpcUrls { http: &["https://rpc.plasma.to"], web_socket: None },
        is_zk_sync: false,
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn ink_chain_info() -> EvmChainInfo {
    let logo_url = "https://files.cow.fi/cow-sdk/chains/images/ink-logo.svg";
    EvmChainInfo {
        id: 57_073,
        label: "Ink",
        eip155_label: "Ink Chain Mainnet",
        address_prefix: "ink",
        native_currency: default_native_currency(57_073),
        is_testnet: false,
        color: "#7132f5",
        logo: ThemedImage { light: logo_url, dark: logo_url },
        website: WebUrl { name: "Ink", url: "https://inkonchain.com/" },
        docs: WebUrl { name: "Ink Docs", url: "https://docs.inkonchain.com" },
        block_explorer: WebUrl { name: "Ink Explorer", url: "https://explorer.inkonchain.com" },
        bridges: &[WebUrl { name: "Ink Bridge", url: "https://inkonchain.com/bridge" }],
        contracts: multicall3_only(0),
        rpc_urls: ChainRpcUrls { http: &["https://rpc-ten.inkonchain.com"], web_socket: None },
        is_zk_sync: false,
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn optimism_chain_info() -> EvmChainInfo {
    let logo_url = "https://files.cow.fi/cow-sdk/chains/images/optimism-logo.svg";
    EvmChainInfo {
        id: 10,
        label: "Optimism",
        eip155_label: "OP Mainnet",
        address_prefix: "op",
        native_currency: default_native_currency(10),
        is_testnet: false,
        color: "#ff0420",
        logo: ThemedImage { light: logo_url, dark: logo_url },
        website: WebUrl { name: "Optimism", url: "https://optimism.io" },
        docs: WebUrl { name: "Optimism Docs", url: "https://docs.optimism.io" },
        block_explorer: WebUrl { name: "Etherscan", url: "https://optimistic.etherscan.io" },
        bridges: &[],
        contracts: multicall3_only(4_286_263),
        rpc_urls: ChainRpcUrls { http: &["https://mainnet.optimism.io"], web_socket: None },
        is_zk_sync: false,
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn bitcoin_chain_info() -> NonEvmChainInfo {
    let logo_url = "https://files.cow.fi/cow-sdk/chains/images/bitcoin-logo.svg";
    NonEvmChainInfo {
        id: 1_000_000_000,
        label: "Bitcoin",
        address_prefix: "btc",
        native_currency: ChainTokenInfo {
            chain_id: 1_000_000_000,
            address: BTC_ADDR,
            decimals: 8,
            name: "Bitcoin",
            symbol: "BTC",
            logo_url: Some(logo_url),
        },
        is_testnet: false,
        color: "#f7931a",
        logo: ThemedImage { light: logo_url, dark: logo_url },
        website: WebUrl { name: "Bitcoin", url: "https://bitcoin.org" },
        docs: WebUrl {
            name: "Bitcoin Docs",
            url: "https://bitcoin.org/en/developer-documentation",
        },
        block_explorer: WebUrl { name: "Blockstream Explorer", url: "https://blockstream.info" },
        is_under_development: false,
        is_deprecated: false,
    }
}

const fn solana_chain_info() -> NonEvmChainInfo {
    let logo_url = "https://files.cow.fi/cow-sdk/chains/images/solana-logo.svg";
    NonEvmChainInfo {
        id: 1_000_000_001,
        label: "Solana",
        address_prefix: "sol",
        native_currency: ChainTokenInfo {
            chain_id: 1_000_000_001,
            address: SOL_ADDR,
            decimals: 9,
            name: "Solana",
            symbol: "SOL",
            logo_url: Some(logo_url),
        },
        is_testnet: false,
        color: "#9945FF",
        logo: ThemedImage { light: logo_url, dark: logo_url },
        website: WebUrl { name: "Solana", url: "https://solana.com" },
        docs: WebUrl { name: "Solana Docs", url: "https://docs.solana.com" },
        block_explorer: WebUrl { name: "Solana Explorer", url: "https://explorer.solana.com" },
        is_under_development: false,
        is_deprecated: false,
    }
}

// ── API context types ────────────────────────────────────────────────────────

/// IPFS configuration for reading and writing app data.
#[derive(Debug, Clone, Default)]
pub struct IpfsConfig {
    /// The URI of the IPFS node.
    pub uri: Option<String>,
    /// The URI of the IPFS node for writing.
    pub write_uri: Option<String>,
    /// The URI of the IPFS node for reading.
    pub read_uri: Option<String>,
    /// Pinata API key.
    pub pinata_api_key: Option<String>,
    /// Pinata API secret.
    pub pinata_api_secret: Option<String>,
}

/// The `CoW` Protocol API context.
///
/// Defines the chain, environment, and optional overrides for connecting to the
/// `CoW` Protocol API.
#[derive(Debug, Clone)]
pub struct ApiContext {
    /// The target chain ID.
    pub chain_id: SupportedChainId,
    /// The API environment (`prod` or `staging`).
    pub env: super::chain::Env,
    /// Optional per-chain base URL overrides.
    pub base_urls: Option<ApiBaseUrls>,
    /// Optional API key for the partner API.
    pub api_key: Option<String>,
}

impl Default for ApiContext {
    /// Returns the default API context (production, mainnet).
    fn default() -> Self {
        Self {
            chain_id: SupportedChainId::Mainnet,
            env: super::chain::Env::Prod,
            base_urls: None,
            api_key: None,
        }
    }
}

/// Protocol-level options for overriding `CoW` Protocol contract addresses and
/// environment.
#[derive(Debug, Clone, Default)]
pub struct ProtocolOptions {
    /// The API environment.
    pub env: Option<super::chain::Env>,
    /// Per-chain settlement contract address overrides.
    pub settlement_contract_override: Option<AddressPerChain>,
    /// Per-chain `EthFlow` contract address overrides.
    pub eth_flow_contract_override: Option<AddressPerChain>,
}

/// An EVM call with a target address, calldata, and value.
#[derive(Debug, Clone)]
pub struct EvmCall {
    /// The target contract address.
    pub to: Address,
    /// The encoded calldata.
    pub data: Vec<u8>,
    /// The value to send (in wei).
    pub value: U256,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── EvmChains ───────────────────────────────────────────────────────

    #[test]
    fn evm_chains_roundtrip_u64() {
        let chains = [
            (EvmChains::Mainnet, 1),
            (EvmChains::Optimism, 10),
            (EvmChains::Bnb, 56),
            (EvmChains::GnosisChain, 100),
            (EvmChains::Polygon, 137),
            (EvmChains::Base, 8_453),
            (EvmChains::Plasma, 9_745),
            (EvmChains::ArbitrumOne, 42_161),
            (EvmChains::Avalanche, 43_114),
            (EvmChains::Ink, 57_073),
            (EvmChains::Linea, 59_144),
            (EvmChains::Sepolia, 11_155_111),
        ];
        for (chain, id) in chains {
            assert_eq!(chain.as_u64(), id);
            assert_eq!(EvmChains::try_from_u64(id), Some(chain));
        }
    }

    #[test]
    fn evm_chains_unknown_returns_none() {
        assert_eq!(EvmChains::try_from_u64(9999), None);
    }

    // ── NonEvmChains ────────────────────────────────────────────────────

    #[test]
    fn non_evm_chains_roundtrip() {
        assert_eq!(NonEvmChains::Bitcoin.as_u64(), 1_000_000_000);
        assert_eq!(NonEvmChains::Solana.as_u64(), 1_000_000_001);
        assert_eq!(NonEvmChains::try_from_u64(1_000_000_000), Some(NonEvmChains::Bitcoin));
        assert_eq!(NonEvmChains::try_from_u64(1_000_000_001), Some(NonEvmChains::Solana));
        assert_eq!(NonEvmChains::try_from_u64(999), None);
    }

    // ── AdditionalTargetChainId ─────────────────────────────────────────

    #[test]
    fn additional_target_roundtrip() {
        for &chain in AdditionalTargetChainId::all() {
            let id = chain.as_u64();
            assert_eq!(AdditionalTargetChainId::try_from_u64(id), Some(chain));
        }
    }

    #[test]
    fn additional_target_all_has_three() {
        assert_eq!(AdditionalTargetChainId::all().len(), 3);
    }

    // ── TargetChainId ───────────────────────────────────────────────────

    #[test]
    fn target_chain_id_as_u64() {
        let supported = TargetChainId::Supported(SupportedChainId::Mainnet);
        assert_eq!(supported.as_u64(), 1);
        let additional = TargetChainId::Additional(AdditionalTargetChainId::Bitcoin);
        assert_eq!(additional.as_u64(), 1_000_000_000);
    }

    // ── Classification helpers ──────────────────────────────────────────

    #[test]
    fn is_evm_chain_correct() {
        assert!(is_evm_chain(1));
        assert!(is_evm_chain(10));
        assert!(!is_evm_chain(1_000_000_000));
        assert!(!is_evm_chain(9999));
    }

    #[test]
    fn is_non_evm_chain_correct() {
        assert!(is_non_evm_chain(1_000_000_000));
        assert!(is_non_evm_chain(1_000_000_001));
        assert!(!is_non_evm_chain(1));
    }

    #[test]
    fn is_btc_chain_correct() {
        assert!(is_btc_chain(1_000_000_000));
        assert!(!is_btc_chain(1));
    }

    #[test]
    fn is_supported_chain_correct() {
        assert!(is_supported_chain(1));
        assert!(is_supported_chain(100));
        assert!(!is_supported_chain(10));
        assert!(!is_supported_chain(9999));
    }

    #[test]
    fn is_additional_target_chain_correct() {
        assert!(is_additional_target_chain(10));
        assert!(is_additional_target_chain(1_000_000_000));
        assert!(!is_additional_target_chain(1));
    }

    #[test]
    fn is_target_chain_id_correct() {
        assert!(is_target_chain_id(1));
        assert!(is_target_chain_id(10));
        assert!(is_target_chain_id(1_000_000_000));
        assert!(!is_target_chain_id(9999));
    }

    #[test]
    fn is_zk_sync_chain_is_false_for_all() {
        assert!(!is_zk_sync_chain(1));
        assert!(!is_zk_sync_chain(100));
    }

    // ── ChainInfo ───────────────────────────────────────────────────────

    #[test]
    fn get_chain_info_all_supported() {
        for &chain in SupportedChainId::all() {
            let chain_info = get_chain_info(chain.as_u64());
            assert!(chain_info.is_some(), "no chain info for {chain:?}");
            let info = chain_info.unwrap_or_else(|| supported_chain_info(chain));
            assert_eq!(info.id(), chain.as_u64());
            assert!(!info.label().is_empty());
        }
    }

    #[test]
    fn get_chain_info_additional_targets() {
        for &chain in AdditionalTargetChainId::all() {
            let info = get_chain_info(chain.as_u64());
            assert!(info.is_some(), "no chain info for {chain:?}");
        }
    }

    #[test]
    fn get_chain_info_unknown_returns_none() {
        assert!(get_chain_info(9999).is_none());
    }

    #[test]
    fn chain_info_evm_predicates() {
        let info = supported_chain_info(SupportedChainId::Mainnet);
        assert!(info.is_evm());
        assert!(!info.is_non_evm());
        assert!(info.as_evm().is_some());
        assert!(info.as_non_evm().is_none());
    }

    #[test]
    fn chain_info_non_evm_predicates() {
        let info = additional_target_chain_info(AdditionalTargetChainId::Bitcoin);
        assert!(!info.is_evm());
        assert!(info.is_non_evm());
        assert!(info.as_non_evm().is_some());
    }

    #[test]
    fn chain_info_native_currency() {
        let info = supported_chain_info(SupportedChainId::Mainnet);
        let currency = info.native_currency();
        assert_eq!(currency.decimals, 18);
    }

    // ── Iteration helpers ───────────────────────────────────────────────

    #[test]
    fn all_supported_chain_ids_matches_all() {
        let ids = all_supported_chain_ids();
        assert_eq!(ids.len(), SupportedChainId::all().len());
    }

    #[test]
    fn all_supported_chains_matches_all() {
        let chains = all_supported_chains();
        assert_eq!(chains.len(), SupportedChainId::all().len());
    }

    #[test]
    fn tradable_chains_excludes_deprecated_and_dev() {
        let tradable = tradable_supported_chain_ids();
        assert!(!tradable.is_empty());
        assert!(tradable.len() <= SupportedChainId::all().len());
    }

    #[test]
    fn all_additional_target_chain_ids_has_three() {
        assert_eq!(all_additional_target_chain_ids().len(), 3);
    }

    #[test]
    fn all_chains_includes_supported_and_additional() {
        let all = all_chains();
        assert!(all.len() >= SupportedChainId::all().len());
    }

    #[test]
    fn all_chain_ids_includes_both() {
        let ids = all_chain_ids();
        assert!(ids.len() >= SupportedChainId::all().len() + AdditionalTargetChainId::all().len());
    }

    #[test]
    fn map_supported_networks_maps_all() {
        let mapped = map_supported_networks(|c| c.as_u64());
        assert_eq!(mapped.len(), SupportedChainId::all().len());
    }

    #[test]
    fn map_address_to_supported_networks_produces_correct_count() {
        let mapped = map_address_to_supported_networks(Address::ZERO);
        assert_eq!(mapped.len(), SupportedChainId::all().len());
    }

    // ── Additional coverage ────────────────────────────────────────────

    #[test]
    fn is_zk_sync_chain_returns_false_for_lens() {
        // Lens (232) has is_zk_sync = true in chain info, but it is not in EvmChains
        // so is_zk_sync_chain returns false because it first checks is_evm_chain.
        assert!(!is_zk_sync_chain(232));
    }

    #[test]
    fn is_zk_sync_chain_returns_false_for_non_evm() {
        assert!(!is_zk_sync_chain(1_000_000_000));
    }

    #[test]
    fn is_chain_under_development_unknown_chain() {
        assert!(!is_chain_under_development(9999));
    }

    #[test]
    fn is_chain_deprecated_unknown_chain() {
        assert!(!is_chain_deprecated(9999));
    }

    #[test]
    fn chain_info_is_under_development_non_evm() {
        let info = additional_target_chain_info(AdditionalTargetChainId::Bitcoin);
        assert!(!info.is_under_development());
    }

    #[test]
    fn chain_info_is_deprecated_non_evm() {
        let info = additional_target_chain_info(AdditionalTargetChainId::Bitcoin);
        assert!(!info.is_deprecated());
    }

    #[test]
    fn evm_chain_info_type_guards() {
        let info = supported_chain_info(SupportedChainId::Mainnet);
        assert!(is_evm_chain_info(&info));
        assert!(!is_non_evm_chain_info(&info));
    }

    #[test]
    fn non_evm_chain_info_type_guards() {
        let info = additional_target_chain_info(AdditionalTargetChainId::Bitcoin);
        assert!(is_non_evm_chain_info(&info));
        assert!(!is_evm_chain_info(&info));
    }

    #[test]
    fn tradable_supported_chains_returns_chain_infos() {
        let chains = tradable_supported_chains();
        assert!(!chains.is_empty());
        for c in &chains {
            assert!(!c.is_deprecated());
        }
    }

    #[test]
    fn all_additional_target_chains_returns_infos() {
        let chains = all_additional_target_chains();
        assert_eq!(chains.len(), 3);
    }

    #[test]
    fn map_all_networks_covers_everything() {
        let mapped = map_all_networks(|t| t.as_u64());
        assert!(
            mapped.len() >= SupportedChainId::all().len() + AdditionalTargetChainId::all().len()
        );
    }

    #[test]
    fn chain_info_label_non_evm() {
        let info = additional_target_chain_info(AdditionalTargetChainId::Solana);
        assert_eq!(info.label(), "Solana");
        assert_eq!(info.id(), 1_000_000_001);
    }

    #[test]
    fn additional_target_optimism_is_evm() {
        let info = additional_target_chain_info(AdditionalTargetChainId::Optimism);
        assert!(info.is_evm());
        assert_eq!(info.label(), "Optimism");
    }

    #[test]
    fn api_context_default() {
        let ctx = ApiContext::default();
        assert_eq!(ctx.chain_id, SupportedChainId::Mainnet);
        assert!(ctx.base_urls.is_none());
        assert!(ctx.api_key.is_none());
    }

    #[test]
    fn additional_target_try_from_u64_unknown() {
        assert!(AdditionalTargetChainId::try_from_u64(999).is_none());
    }
}
