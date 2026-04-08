//! Per-chain `CoW` Protocol contract addresses.
//!
//! All contracts use the same address across EVM chains thanks to
//! deterministic `CREATE2` deployment. This module defines the canonical
//! addresses as constants and provides per-chain/per-env lookup functions.
//!
//! # Key constants
//!
//! | Constant | Address |
//! |---|---|
//! | [`SETTLEMENT_CONTRACT`] | `0x9008D19f…0560ab41` |
//! | [`VAULT_RELAYER`] | `0xC92E8bdf…BFE0110` |
//! | [`COMPOSABLE_COW`] | `0xfdaFc9d1…013b74` |
//! | [`EXTENSIBLE_FALLBACK_HANDLER`] | `0x2f55e8b2…605bF5` |
//! | [`BUY_ETH_ADDRESS`] | `0xEeee…EeEe` (buy native currency sentinel) |

use alloy_primitives::{Address, B256, keccak256};

use super::chain::SupportedChainId;

/// The `CoW` Protocol `GPv2Settlement` contract address.
///
/// Identical on all supported chains.
/// `0x9008D19f58AAbD9eD0D60971565AA8510560ab41`
pub const SETTLEMENT_CONTRACT: Address = Address::new([
    0x90, 0x08, 0xd1, 0x9f, 0x58, 0xaa, 0xbd, 0x9e, 0xd0, 0xd6, 0x09, 0x71, 0x56, 0x5a, 0xa8, 0x51,
    0x05, 0x60, 0xab, 0x41,
]);

/// The `CoW` Protocol Vault Relayer contract address.
///
/// Identical on all supported chains.
/// `0xC92E8bdf79f0507f65a392b0ab4667716BFE0110`
pub const VAULT_RELAYER: Address = Address::new([
    0xc9, 0x2e, 0x8b, 0xdf, 0x79, 0xf0, 0x50, 0x7f, 0x65, 0xa3, 0x92, 0xb0, 0xab, 0x46, 0x67, 0x71,
    0x6b, 0xfe, 0x01, 0x10,
]);

/// The production `EthFlow` contract address.
///
/// Identical on all supported chains except Lens (not included here).
/// `0xba3cb449bd2b4adddbc894d8697f5170800eadec`
pub const ETH_FLOW_PROD: Address = Address::new([
    0xba, 0x3c, 0xb4, 0x49, 0xbd, 0x2b, 0x4a, 0xdd, 0xdb, 0xc8, 0x94, 0xd8, 0x69, 0x7f, 0x51, 0x70,
    0x80, 0x0e, 0xad, 0xec,
]);

/// The staging (barn) `EthFlow` contract address.
///
/// `0x04501b9b1d52e67f6862d157e00d13419d2d6e95`
pub const ETH_FLOW_STAGING: Address = Address::new([
    0x04, 0x50, 0x1b, 0x9b, 0x1d, 0x52, 0xe6, 0x7f, 0x68, 0x62, 0xd1, 0x57, 0xe0, 0x0d, 0x13, 0x41,
    0x9d, 0x2d, 0x6e, 0x95,
]);

/// The `ExtensibleFallbackHandler` contract address.
///
/// Used by `Safe` wallets to enable custom EIP-712 domain verifiers
/// (such as `ComposableCow`-based conditional orders).
/// Identical on all supported chains.
/// `0x2f55e8b20D0B9FEFA187AA7d00B6Cbe563605bF5`
pub const EXTENSIBLE_FALLBACK_HANDLER: Address = Address::new([
    0x2f, 0x55, 0xe8, 0xb2, 0x0d, 0x0b, 0x9f, 0xef, 0xa1, 0x87, 0xaa, 0x7d, 0x00, 0xb6, 0xcb, 0xe5,
    0x63, 0x60, 0x5b, 0xf5,
]);

/// The staging `CoW` Protocol `GPv2Settlement` contract address.
///
/// Used on the barn (staging) environment.
/// `0xf553d092b50bdcbddeD1A99aF2cA29FBE5E2CB13`
pub const SETTLEMENT_CONTRACT_STAGING: Address = Address::new([
    0xf5, 0x53, 0xd0, 0x92, 0xb5, 0x0b, 0xdc, 0xbd, 0xde, 0xd1, 0xa9, 0x9a, 0xf2, 0xca, 0x29, 0xfb,
    0xe5, 0xe2, 0xcb, 0x13,
]);

/// The staging `CoW` Protocol Vault Relayer contract address.
///
/// Used on the barn (staging) environment.
/// `0xC7242d167563352E2BCA4d71C043fbe542DB8FB2`
pub const VAULT_RELAYER_STAGING: Address = Address::new([
    0xc7, 0x24, 0x2d, 0x16, 0x75, 0x63, 0x35, 0x2e, 0x2b, 0xca, 0x4d, 0x71, 0xc0, 0x43, 0xfb, 0xe5,
    0x42, 0xdb, 0x8f, 0xb2,
]);

/// The staging (barn) `EthFlow` contract address.
///
/// `0xb37aDD6AC288BD3825a901Cba6ec65A89f31B8CC`
pub const BARN_ETH_FLOW: Address = Address::new([
    0xb3, 0x7a, 0xdd, 0x6a, 0xc2, 0x88, 0xbd, 0x38, 0x25, 0xa9, 0x01, 0xcb, 0xa6, 0xec, 0x65, 0xa8,
    0x9f, 0x31, 0xb8, 0xcc,
]);

/// The `ComposableCow` factory contract address.
///
/// Same address on all supported chains.
/// `0xfdaFc9d1902f4e0b84f65F49f244b32b31013b74`
pub const COMPOSABLE_COW: Address = Address::new([
    0xfd, 0xaf, 0xc9, 0xd1, 0x90, 0x2f, 0x4e, 0x0b, 0x84, 0xf6, 0x5f, 0x49, 0xf2, 0x44, 0xb3, 0x2b,
    0x31, 0x01, 0x3b, 0x74,
]);

/// Marker address to indicate that an order is buying Ether (native currency).
///
/// This address only has special meaning in the `buyToken` field.
/// `0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE`
pub const BUY_ETH_ADDRESS: Address = Address::new([0xee; 20]);

/// The `CREATE2` deployer contract address.
///
/// Used by the hardhat-deploy library for deterministic deployments.
/// Same on all EVM chains.
/// `0x4e59b44847b379578588920ca78fbf26c0b4956c`
pub const DEPLOYER_CONTRACT: Address = Address::new([
    0x4e, 0x59, 0xb4, 0x48, 0x47, 0xb3, 0x79, 0x57, 0x85, 0x88, 0x92, 0x0c, 0xa7, 0x8f, 0xbf, 0x26,
    0xc0, 0xb4, 0x95, 0x6c,
]);

/// The deterministic deployment salt.
///
/// `ethers.utils.formatBytes32String("Mattresses in Berlin!")`
pub const SALT: &str = "0x4d61747472657373657320696e204265726c696e210000000000000000000000";

/// Maximum valid-to epoch value (`u32::MAX`).
///
/// Used as the `validTo` timestamp for `EthFlow` orders where the actual
/// deadline is controlled by the contract rather than the order expiry.
pub const MAX_VALID_TO_EPOCH: u32 = u32::MAX;

/// Return the production settlement contract address for `chain`.
///
/// Currently the same for all supported chains (deterministic deployment).
///
/// # Parameters
///
/// * `_chain` — the target chain (unused — address is identical across chains).
///
/// # Returns
///
/// [`SETTLEMENT_CONTRACT`] address.
#[must_use]
pub const fn settlement_contract(_chain: SupportedChainId) -> Address {
    SETTLEMENT_CONTRACT
}

/// Return the vault relayer contract address for `chain`.
///
/// Currently the same for all supported chains.
///
/// # Arguments
///
/// * `_chain` — the target chain (unused — address is identical across chains).
///
/// # Returns
///
/// [`VAULT_RELAYER`] address.
#[must_use]
pub const fn vault_relayer(_chain: SupportedChainId) -> Address {
    VAULT_RELAYER
}

/// Return the settlement contract address for `chain` in a given
/// environment.
///
/// Returns [`SETTLEMENT_CONTRACT`] for [`Env::Prod`](super::chain::Env::Prod)
/// and [`SETTLEMENT_CONTRACT_STAGING`] for
/// [`Env::Staging`](super::chain::Env::Staging).
///
/// # Parameters
///
/// * `_chain` — the target chain (unused).
/// * `env` — the orderbook environment.
///
/// # Returns
///
/// The settlement contract [`Address`] for the given environment.
#[must_use]
pub const fn settlement_contract_for_env(
    _chain: SupportedChainId,
    env: super::chain::Env,
) -> Address {
    match env {
        super::chain::Env::Prod => SETTLEMENT_CONTRACT,
        super::chain::Env::Staging => SETTLEMENT_CONTRACT_STAGING,
    }
}

/// Return the vault relayer contract address for `chain` in a given
/// environment.
///
/// # Arguments
///
/// * `_chain` — the target chain (unused).
/// * `env` — the orderbook environment.
///
/// # Returns
///
/// The vault relayer [`Address`] for the given environment.
#[must_use]
pub const fn vault_relayer_for_env(_chain: SupportedChainId, env: super::chain::Env) -> Address {
    match env {
        super::chain::Env::Prod => VAULT_RELAYER,
        super::chain::Env::Staging => VAULT_RELAYER_STAGING,
    }
}

/// Return the `EthFlow` contract address for `chain` in a given environment.
///
/// # Arguments
///
/// * `_chain` — the target chain (unused).
/// * `env` — the orderbook environment.
///
/// # Returns
///
/// The `EthFlow` contract [`Address`] for the given environment.
#[must_use]
pub const fn eth_flow_for_env(_chain: SupportedChainId, env: super::chain::Env) -> Address {
    match env {
        super::chain::Env::Prod => ETH_FLOW_PROD,
        super::chain::Env::Staging => BARN_ETH_FLOW,
    }
}

/// Return the `ComposableCow` contract address for `chain`.
///
/// Currently the same for all supported chains.
///
/// # Arguments
///
/// * `_chain` — the target chain (unused — address is identical across chains).
///
/// # Returns
///
/// [`COMPOSABLE_COW`] address.
#[must_use]
pub const fn composable_cow(_chain: SupportedChainId) -> Address {
    COMPOSABLE_COW
}

/// Return the `ExtensibleFallbackHandler` contract address for `chain`.
///
/// Currently the same for all supported chains.
///
/// # Arguments
///
/// * `_chain` — the target chain (unused — address is identical across chains).
///
/// # Returns
///
/// [`EXTENSIBLE_FALLBACK_HANDLER`] address.
#[must_use]
pub const fn extensible_fallback_handler(_chain: SupportedChainId) -> Address {
    EXTENSIBLE_FALLBACK_HANDLER
}

// ── Per-chain address map accessors ───────────��─────────────────────────────
//
// The TypeScript SDK exposes `Record<SupportedChainId, string>` maps. In Rust,
// since all addresses are identical across chains, we provide lookup functions
// that mirror the TS `COMPOSABLE_COW_CONTRACT_ADDRESS[chainId]` pattern.

/// Return the `ComposableCow` contract address for a given chain.
///
/// Mirrors `COMPOSABLE_COW_CONTRACT_ADDRESS[chainId]` from the `TypeScript` SDK.
/// Currently returns the same address for all supported chains.
///
/// # Arguments
///
/// * `_chain` — the target chain (unused — address is identical across chains).
///
/// # Returns
///
/// [`COMPOSABLE_COW`] address.
#[must_use]
pub const fn composable_cow_contract_address(_chain: SupportedChainId) -> Address {
    COMPOSABLE_COW
}

/// Return the `CoW` Protocol settlement contract address for a given chain.
///
/// Mirrors `COW_PROTOCOL_SETTLEMENT_CONTRACT_ADDRESS[chainId]` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `_chain` — the target chain (unused — address is identical across chains).
///
/// # Returns
///
/// [`SETTLEMENT_CONTRACT`] address.
#[must_use]
pub const fn cow_protocol_settlement_contract_address(_chain: SupportedChainId) -> Address {
    SETTLEMENT_CONTRACT
}

/// Return the `CoW` Protocol Vault Relayer address for a given chain.
///
/// Mirrors `COW_PROTOCOL_VAULT_RELAYER_ADDRESS[chainId]` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `_chain` — the target chain (unused — address is identical across chains).
///
/// # Returns
///
/// [`VAULT_RELAYER`] address.
#[must_use]
pub const fn cow_protocol_vault_relayer_address(_chain: SupportedChainId) -> Address {
    VAULT_RELAYER
}

/// Return the staging `CoW` Protocol Vault Relayer address for a given chain.
///
/// Mirrors `COW_PROTOCOL_VAULT_RELAYER_ADDRESS_STAGING[chainId]` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `_chain` — the target chain (unused — address is identical across chains).
///
/// # Returns
///
/// [`VAULT_RELAYER_STAGING`] address.
#[must_use]
pub const fn cow_protocol_vault_relayer_address_staging(_chain: SupportedChainId) -> Address {
    VAULT_RELAYER_STAGING
}

/// Return the `ExtensibleFallbackHandler` contract address for a given chain.
///
/// Mirrors `EXTENSIBLE_FALLBACK_HANDLER_CONTRACT_ADDRESS[chainId]` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `_chain` — the target chain (unused — address is identical across chains).
///
/// # Returns
///
/// [`EXTENSIBLE_FALLBACK_HANDLER`] address.
#[must_use]
pub const fn extensible_fallback_handler_contract_address(_chain: SupportedChainId) -> Address {
    EXTENSIBLE_FALLBACK_HANDLER
}

// ── CREATE2 deterministic deployment ────────────────────────────────────────

/// Compute the deterministic `CREATE2` deployment address for a contract.
///
/// Uses the canonical [`DEPLOYER_CONTRACT`] and [`SALT`] values from the
/// `CoW` Protocol deployment tooling.
///
/// Mirrors `deterministicDeploymentAddress` from the `TypeScript` `contracts-ts` package.
///
/// # Arguments
///
/// * `bytecode` - The contract creation bytecode (init code).
/// * `constructor_args` - ABI-encoded constructor arguments to append to the bytecode.
///
/// # Example
///
/// ```
/// use cow_rs::config::contracts::deterministic_deployment_address;
///
/// let addr = deterministic_deployment_address(&[0xfe], &[]);
/// assert!(!addr.is_zero());
/// ```
#[must_use]
pub fn deterministic_deployment_address(bytecode: &[u8], constructor_args: &[u8]) -> Address {
    let salt_bytes = B256::from_slice(
        &alloy_primitives::hex::decode(SALT.trim_start_matches("0x"))
            .expect("SALT is always valid hex"),
    );

    // init_code = bytecode ++ constructor_args
    let mut init_code = Vec::with_capacity(bytecode.len() + constructor_args.len());
    init_code.extend_from_slice(bytecode);
    init_code.extend_from_slice(constructor_args);
    let init_code_hash = keccak256(&init_code);

    // CREATE2: keccak256(0xff ++ deployer ++ salt ++ keccak256(init_code))[12..]
    let mut buf = [0u8; 1 + 20 + 32 + 32];
    buf[0] = 0xff;
    buf[1..21].copy_from_slice(DEPLOYER_CONTRACT.as_slice());
    buf[21..53].copy_from_slice(salt_bytes.as_slice());
    buf[53..85].copy_from_slice(init_code_hash.as_slice());
    let hash = keccak256(buf);
    Address::from_slice(&hash[12..])
}

// ── EIP-1967 proxy storage slots ────────────────────────────────────────────

/// EIP-1967 storage slot for the implementation address.
///
/// `bytes32(uint256(keccak256("eip1967.proxy.implementation")) - 1)`
pub const IMPLEMENTATION_STORAGE_SLOT: &str =
    "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc";

/// EIP-1967 storage slot for the proxy admin/owner address.
///
/// `bytes32(uint256(keccak256("eip1967.proxy.admin")) - 1)`
pub const OWNER_STORAGE_SLOT: &str =
    "0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103";

/// Build `eth_getStorageAt` calldata to read the EIP-1967 implementation address.
///
/// Returns `(proxy_address, storage_slot)` — pass these to an `eth_getStorageAt`
/// JSON-RPC call to retrieve the implementation address of an EIP-1967 proxy.
///
/// Mirrors `implementationAddress` from the `TypeScript` `contracts-ts` package.
/// The `TypeScript` version makes a live RPC call; this Rust version returns the
/// parameters so callers can use their preferred provider.
///
/// # Example
///
/// ```
/// use alloy_primitives::Address;
/// use cow_rs::config::contracts::implementation_address_slot;
///
/// let proxy = Address::ZERO;
/// let (addr, slot) = implementation_address_slot(proxy);
/// assert_eq!(addr, proxy);
/// assert!(slot.starts_with("0x360894a1"));
/// ```
#[must_use]
pub const fn implementation_address_slot(proxy: Address) -> (Address, &'static str) {
    (proxy, IMPLEMENTATION_STORAGE_SLOT)
}

/// Build `eth_getStorageAt` calldata to read the EIP-1967 admin/owner address.
///
/// Returns `(proxy_address, storage_slot)` — pass these to an `eth_getStorageAt`
/// JSON-RPC call to retrieve the owner address of an EIP-1967 proxy.
///
/// Mirrors `ownerAddress` from the `TypeScript` `contracts-ts` package.
///
/// # Example
///
/// ```
/// use alloy_primitives::Address;
/// use cow_rs::config::contracts::owner_address_slot;
///
/// let proxy = Address::ZERO;
/// let (addr, slot) = owner_address_slot(proxy);
/// assert_eq!(addr, proxy);
/// assert!(slot.starts_with("0xb53127684a"));
/// ```
#[must_use]
pub const fn owner_address_slot(proxy: Address) -> (Address, &'static str) {
    (proxy, OWNER_STORAGE_SLOT)
}
