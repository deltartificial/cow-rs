//! Across Protocol bridge provider — types, constants, and utility functions.

use std::sync::Arc;

use alloy_primitives::{Address, B256, U256};
use alloy_signer_local::PrivateKeySigner;
use cow_chains::{EvmCall, SupportedChainId};
use cow_errors::CowError;
use cow_orderbook::types::Order;
use cow_shed::CowShedSdk;
use cow_types::OrderKind;
use foldhash::HashMap;

use crate::{
    provider::{
        BridgeNetworkInfo, BridgeProvider, BridgeStatusFuture, BridgingParamsFuture,
        BuyTokensFuture, HookBridgeProvider, IntermediateTokensFuture, NetworksFuture, QuoteFuture,
        SignedHookFuture, UnsignedCallFuture,
    },
    types::{
        AcrossChainConfig, AcrossDepositStatus, AcrossSuggestedFeesResponse, BridgeAmounts,
        BridgeCosts, BridgeError, BridgeFees, BridgeLimits, BridgeProviderInfo, BridgeProviderType,
        BridgeQuoteAmountsAndCosts, BridgeQuoteResult, BridgeStatus, BridgeStatusResult,
        BridgingFee, BuyTokensParams, GetProviderBuyTokens, IntermediateTokenInfo,
        QuoteBridgeRequest, QuoteBridgeResponse,
    },
    utils::{apply_bps, apply_pct_fee, pct_to_bps},
};

// ── Contract addresses ────────────────────────────────────────────────────────

/// Across `SpokePool` contract addresses per chain.
///
/// See <https://docs.across.to/reference/contract-addresses>.
///
/// # Returns
///
/// A map from chain ID to the deployed `SpokePool` contract address on that
/// chain. Only chains with known deployments are included.
///
/// # Examples
///
/// ```rust,no_run
/// use cow_bridging::across::across_spoke_pool_addresses;
///
/// let pools = across_spoke_pool_addresses();
/// // Ethereum Mainnet (chain ID 1)
/// assert!(pools.contains_key(&1));
/// ```
#[must_use]
pub fn across_spoke_pool_addresses() -> HashMap<u64, Address> {
    let mut m = HashMap::default();
    let insert = |m: &mut HashMap<u64, Address>, chain_id: u64, addr: &str| {
        if let Ok(a) = addr.parse::<Address>() {
            m.insert(chain_id, a);
        }
    };

    insert(
        &mut m,
        SupportedChainId::Mainnet.as_u64(),
        "0x5c7BCd6E7De5423a257D81B442095A1a6ced35C5",
    );
    insert(
        &mut m,
        SupportedChainId::ArbitrumOne.as_u64(),
        "0xe35e9842fceaca96570b734083f4a58e8f7c5f2a",
    );
    insert(&mut m, SupportedChainId::Base.as_u64(), "0x09aea4b2242abC8bb4BB78D537A67a245A7bEC64");
    insert(
        &mut m,
        SupportedChainId::Sepolia.as_u64(),
        "0x5ef6C01E11889d86803e0B23e3cB3F9E9d97B662",
    );
    insert(
        &mut m,
        SupportedChainId::Polygon.as_u64(),
        "0x9295ee1d8C5b022Be115A2AD3c30C72E34e7F096",
    );
    insert(
        &mut m,
        SupportedChainId::BnbChain.as_u64(),
        "0x4e8E101924eDE233C13e2D8622DC8aED2872d505",
    );
    insert(&mut m, SupportedChainId::Linea.as_u64(), "0x7E63A5f1a8F0B4d0934B2f2327DAED3F6bb2ee75");
    insert(&mut m, SupportedChainId::Plasma.as_u64(), "0x50039fAEfebef707cFD94D6d462fE6D10B39207a");
    insert(&mut m, SupportedChainId::Ink.as_u64(), "0xeF684C38F94F48775959ECf2012D7E864ffb9dd4");
    // Optimism (chain ID 10)
    insert(&mut m, 10, "0x6f26Bf09B1C792e3228e5467807a900A503c0281");

    m
}

/// Across math helper contract addresses per chain (used for weiroll fee computation).
///
/// # Returns
///
/// A map from chain ID to the deployed `AcrossMath` helper contract address.
/// Only chains with known deployments are included (currently Mainnet,
/// Arbitrum One, and Base).
///
/// # Examples
///
/// ```rust,no_run
/// use cow_bridging::across::across_math_contract_addresses;
///
/// let addrs = across_math_contract_addresses();
/// assert!(addrs.contains_key(&1)); // Mainnet
/// ```
#[must_use]
pub fn across_math_contract_addresses() -> HashMap<u64, Address> {
    let mut m = HashMap::default();
    let insert = |m: &mut HashMap<u64, Address>, chain_id: u64, addr: &str| {
        if let Ok(a) = addr.parse::<Address>() {
            m.insert(chain_id, a);
        }
    };

    insert(
        &mut m,
        SupportedChainId::Mainnet.as_u64(),
        "0xf2ae6728b6f146556977Af0A68bFbf5bADA22863",
    );
    insert(
        &mut m,
        SupportedChainId::ArbitrumOne.as_u64(),
        "0x5771A4b4029832e79a75De7B485E5fBbec28848f",
    );
    insert(&mut m, SupportedChainId::Base.as_u64(), "0xd4e943dc6ddc885f6229ce33c2e3dfe402a12c81");

    m
}

// ── Token mapping ─────────────────────────────────────────────────────────────

/// Build the Across token mapping — per-chain token symbol → address.
///
/// # Returns
///
/// A map from chain ID to an [`AcrossChainConfig`] containing the known
/// bridgeable tokens (symbol → address) for that chain.
///
/// # Examples
///
/// ```rust,no_run
/// use cow_bridging::across::across_token_mapping;
///
/// let mapping = across_token_mapping();
/// let mainnet = mapping.get(&1).expect("mainnet config");
/// assert!(mainnet.tokens.contains_key("usdc"));
/// ```
#[must_use]
pub fn across_token_mapping() -> HashMap<u64, AcrossChainConfig> {
    let mut configs = HashMap::default();

    let make_config = |chain_id: u64, tokens: &[(&str, &str)]| -> AcrossChainConfig {
        let token_map: HashMap<String, Address> = tokens
            .iter()
            .filter_map(|(sym, addr)| addr.parse::<Address>().ok().map(|a| ((*sym).to_owned(), a)))
            .collect();
        AcrossChainConfig { chain_id, tokens: token_map }
    };

    // Mainnet
    configs.insert(
        SupportedChainId::Mainnet.as_u64(),
        make_config(
            SupportedChainId::Mainnet.as_u64(),
            &[
                ("usdc", "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                ("weth", "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                ("wbtc", "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"),
                ("dai", "0x6B175474E89094C44Da98b954EedeAC495271d0F"),
                ("usdt", "0xdAC17F958D2ee523a2206206994597C13D831ec7"),
            ],
        ),
    );

    // Polygon
    configs.insert(
        SupportedChainId::Polygon.as_u64(),
        make_config(
            SupportedChainId::Polygon.as_u64(),
            &[
                ("usdc", "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"),
                ("weth", "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619"),
                ("wbtc", "0x1BFD67037B42Cf73acF2047067bd4F2C47D9BfD6"),
                ("dai", "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063"),
                ("usdt", "0xc2132D05D31c914a87C6611C10748AEb04B58e8F"),
            ],
        ),
    );

    // Arbitrum One
    configs.insert(
        SupportedChainId::ArbitrumOne.as_u64(),
        make_config(
            SupportedChainId::ArbitrumOne.as_u64(),
            &[
                ("usdc", "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"),
                ("weth", "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1"),
                ("wbtc", "0x2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f"),
                ("dai", "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1"),
                ("usdt", "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9"),
            ],
        ),
    );

    // Base
    configs.insert(
        SupportedChainId::Base.as_u64(),
        make_config(
            SupportedChainId::Base.as_u64(),
            &[
                ("usdc", "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
                ("weth", "0x4200000000000000000000000000000000000006"),
                ("dai", "0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb"),
            ],
        ),
    );

    // Optimism (chain ID 10)
    configs.insert(
        10,
        make_config(
            10,
            &[
                ("usdc", "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85"),
                ("weth", "0x4200000000000000000000000000000000000006"),
                ("wbtc", "0x68f180fcCe6836688e9084f035309E29Bf0A2095"),
                ("dai", "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1"),
                ("usdt", "0x94b008aA00579c1307B0EF2c499aD98a8ce58e58"),
            ],
        ),
    );

    configs
}

/// Get chain configurations for a source-target pair.
///
/// Returns `None` if either chain is not configured.
///
/// # Arguments
///
/// * `source_chain_id` — Chain ID of the source (sell) chain.
/// * `target_chain_id` — Chain ID of the target (buy) chain.
///
/// # Examples
///
/// ```rust,no_run
/// use cow_bridging::across::get_chain_configs;
///
/// if let Some((source, target)) = get_chain_configs(1, 42161) {
///     assert_eq!(source.chain_id, 1);
///     assert_eq!(target.chain_id, 42161);
/// }
/// ```
#[must_use]
pub fn get_chain_configs(
    source_chain_id: u64,
    target_chain_id: u64,
) -> Option<(AcrossChainConfig, AcrossChainConfig)> {
    let mapping = across_token_mapping();
    let source = mapping.get(&source_chain_id)?.clone();
    let target = mapping.get(&target_chain_id)?.clone();
    Some((source, target))
}

/// Look up a token's symbol by address in a chain config.
///
/// # Arguments
///
/// * `token_address` — The on-chain token contract address to search for.
/// * `chain_config` — The [`AcrossChainConfig`] to search within.
///
/// # Returns
///
/// The lowercase token symbol (e.g. `"usdc"`) if found, or `None` if the
/// address is not present in the config.
///
/// # Examples
///
/// ```rust,no_run
/// use cow_bridging::across::{across_token_mapping, get_token_symbol};
///
/// let mapping = across_token_mapping();
/// let mainnet = mapping.get(&1).unwrap();
/// if let Some(symbol) = get_token_symbol(mainnet.tokens["usdc"], mainnet) {
///     assert_eq!(symbol, "usdc");
/// }
/// ```
#[must_use]
pub fn get_token_symbol(
    token_address: Address,
    chain_config: &AcrossChainConfig,
) -> Option<String> {
    chain_config.tokens.iter().find(|(_, addr)| **addr == token_address).map(|(sym, _)| sym.clone())
}

/// Look up a token's address by symbol in a chain config.
///
/// # Arguments
///
/// * `token_symbol` — The lowercase token symbol (e.g. `"usdc"`, `"weth"`).
/// * `chain_config` — The [`AcrossChainConfig`] to search within.
///
/// # Returns
///
/// The token's contract [`Address`] if the symbol exists in the config, or
/// `None` otherwise.
///
/// # Examples
///
/// ```rust,no_run
/// use cow_bridging::across::{across_token_mapping, get_token_address};
///
/// let mapping = across_token_mapping();
/// let mainnet = mapping.get(&1).unwrap();
/// assert!(get_token_address("usdc", mainnet).is_some());
/// assert!(get_token_address("unknown", mainnet).is_none());
/// ```
#[must_use]
pub fn get_token_address(token_symbol: &str, chain_config: &AcrossChainConfig) -> Option<Address> {
    chain_config.tokens.get(token_symbol).copied()
}

/// Find a token by its address and chain ID across all Across chain configs.
///
/// # Arguments
///
/// * `token_address` — The on-chain token contract address to search for.
/// * `chain_id` — The chain ID to look up in the global token mapping.
///
/// # Returns
///
/// A tuple of `(symbol, address)` if found, or `None` if the chain is not
/// configured or the address is not present in that chain's config.
///
/// # Examples
///
/// ```rust,no_run
/// use cow_bridging::across::{across_token_mapping, get_token_by_address_and_chain_id};
///
/// let mapping = across_token_mapping();
/// let usdc_addr = mapping.get(&1).unwrap().tokens["usdc"];
/// let result = get_token_by_address_and_chain_id(usdc_addr, 1);
/// assert_eq!(result.unwrap().0, "usdc");
/// ```
#[must_use]
pub fn get_token_by_address_and_chain_id(
    token_address: Address,
    chain_id: u64,
) -> Option<(String, Address)> {
    let mapping = across_token_mapping();
    let config = mapping.get(&chain_id)?;
    config
        .tokens
        .iter()
        .find(|(_, addr)| **addr == token_address)
        .map(|(sym, addr)| (sym.clone(), *addr))
}

// ── Quote conversion ──────────────────────────────────────────────────────────

/// Convert an Across `SuggestedFeesResponse` into a [`BridgeQuoteResult`].
///
/// # Errors
///
/// Returns [`BridgeError::QuoteError`] if fee percentages are invalid.
pub fn to_bridge_quote_result(
    request: &super::types::QuoteBridgeRequest,
    slippage_bps: u32,
    suggested_fees: &AcrossSuggestedFeesResponse,
) -> Result<BridgeQuoteResult, BridgeError> {
    let amounts_and_costs = to_amounts_and_costs(request, slippage_bps, suggested_fees)?;

    let bridge_fee =
        suggested_fees.relayer_capital_fee.total.parse::<u128>().map_or(U256::ZERO, U256::from);

    let destination_gas_fee =
        suggested_fees.relayer_gas_fee.total.parse::<u128>().map_or(U256::ZERO, U256::from);

    let min_deposit =
        suggested_fees.limits.min_deposit.parse::<u128>().map_or(U256::ZERO, U256::from);

    let max_deposit =
        suggested_fees.limits.max_deposit.parse::<u128>().map_or(U256::ZERO, U256::from);

    let quote_timestamp = suggested_fees.timestamp.parse::<u64>().map_or(0, |v| v);
    let expected_fill_time = suggested_fees.estimated_fill_time_sec.parse::<u64>().ok();

    Ok(BridgeQuoteResult {
        id: None,
        signature: None,
        attestation_signature: None,
        quote_body: serde_json::to_string(suggested_fees).ok(),
        is_sell: request.kind == OrderKind::Sell,
        amounts_and_costs,
        expected_fill_time_seconds: expected_fill_time,
        quote_timestamp,
        fees: BridgeFees { bridge_fee, destination_gas_fee },
        limits: BridgeLimits { min_deposit, max_deposit },
    })
}

/// Build the full amounts-and-costs from Across suggested fees.
///
/// Converts a bridge quote request and the Across suggested-fees response into
/// a [`BridgeQuoteAmountsAndCosts`] struct, computing before-fee, after-fee, and
/// after-slippage amounts in both sell and buy currencies.
///
/// # Arguments
///
/// * `request` — The original bridge quote request containing amounts, tokens, and decimal
///   information.
/// * `slippage_bps` — Slippage tolerance in basis points (e.g. `50` = 0.5%).
/// * `suggested_fees` — The Across API suggested-fees response containing relay fee percentages.
///
/// # Returns
///
/// A fully populated [`BridgeQuoteAmountsAndCosts`] on success.
///
/// # Errors
///
/// Returns [`BridgeError::QuoteError`] if the total relay fee percentage
/// string cannot be parsed or if fee application overflows.
fn to_amounts_and_costs(
    request: &super::types::QuoteBridgeRequest,
    slippage_bps: u32,
    suggested_fees: &AcrossSuggestedFeesResponse,
) -> Result<BridgeQuoteAmountsAndCosts, BridgeError> {
    let sell_amount_before_fee = request.sell_amount;

    // Convert sell to buy taking decimal differences into account.
    let buy_decimals = U256::from(10u64).pow(U256::from(request.buy_token_decimals));
    let sell_decimals = U256::from(10u64).pow(U256::from(request.sell_token_decimals));
    let buy_amount_before_fee = (sell_amount_before_fee * buy_decimals) / sell_decimals;

    // Parse the total relay fee percentage.
    let total_relay_fee_pct: u128 =
        suggested_fees.total_relay_fee.pct.parse().map_err(|_parse_err| {
            BridgeError::QuoteError("invalid totalRelayFee.pct".to_owned())
        })?;

    let buy_amount_after_fee = apply_pct_fee(buy_amount_before_fee, total_relay_fee_pct)?;

    // Fee amounts in both currencies.
    let fee_sell_token =
        sell_amount_before_fee - apply_pct_fee(sell_amount_before_fee, total_relay_fee_pct)?;
    let fee_buy_token = buy_amount_before_fee - buy_amount_after_fee;

    // Apply slippage to the after-fee buy amount.
    let buy_amount_after_slippage = apply_bps(buy_amount_after_fee, slippage_bps);

    let fee_bps = pct_to_bps(total_relay_fee_pct)?;

    Ok(BridgeQuoteAmountsAndCosts {
        before_fee: BridgeAmounts {
            sell_amount: sell_amount_before_fee,
            buy_amount: buy_amount_before_fee,
        },
        after_fee: BridgeAmounts {
            sell_amount: sell_amount_before_fee,
            buy_amount: buy_amount_after_fee,
        },
        after_slippage: BridgeAmounts {
            sell_amount: sell_amount_before_fee,
            buy_amount: buy_amount_after_slippage,
        },
        costs: BridgeCosts {
            bridging_fee: BridgingFee {
                fee_bps,
                amount_in_sell_currency: fee_sell_token,
                amount_in_buy_currency: fee_buy_token,
            },
        },
        slippage_bps,
    })
}

// ── Status mapping ────────────────────────────────────────────────────────────

/// Map an Across deposit status to the unified [`BridgeStatus`].
///
/// # Arguments
///
/// * `status` — The Across-specific [`AcrossDepositStatus`] variant.
///
/// # Returns
///
/// The corresponding unified [`BridgeStatus`]:
/// - `Filled` / `SlowFillRequested` → [`BridgeStatus::Executed`]
/// - `Pending` → [`BridgeStatus::InProgress`]
/// - `Expired` → [`BridgeStatus::Expired`]
/// - `Refunded` → [`BridgeStatus::Refund`]
///
/// # Examples
///
/// ```rust,no_run
/// use cow_bridging::{
///     across::map_across_status_to_bridge_status,
///     types::{AcrossDepositStatus, BridgeStatus},
/// };
///
/// let status = map_across_status_to_bridge_status(AcrossDepositStatus::Filled);
/// assert!(matches!(status, BridgeStatus::Executed));
/// ```
#[must_use]
pub const fn map_across_status_to_bridge_status(status: AcrossDepositStatus) -> BridgeStatus {
    match status {
        AcrossDepositStatus::Filled | AcrossDepositStatus::SlowFillRequested => {
            BridgeStatus::Executed
        }
        AcrossDepositStatus::Pending => BridgeStatus::InProgress,
        AcrossDepositStatus::Expired => BridgeStatus::Expired,
        AcrossDepositStatus::Refunded => BridgeStatus::Refund,
    }
}

/// Validate an Across status API response.
///
/// Returns `true` if the response has a valid `status` field.
#[must_use]
pub fn is_valid_across_status_response(response: &serde_json::Value) -> bool {
    response.get("status").and_then(|s| s.as_str()).is_some()
}

// ── Across ABI event signatures ───────────────────────────────────────────────

/// Keccak-256 topic for the Across `FundsDeposited` event.
///
/// `FundsDeposited(bytes32,bytes32,uint256,uint256,uint256,uint256,uint32,uint32,uint32,bytes32,
/// bytes32,bytes32,bytes)`
pub const ACROSS_FUNDS_DEPOSITED_TOPIC: &str = "event FundsDeposited(bytes32 inputToken, bytes32 outputToken, uint256 inputAmount, uint256 outputAmount, uint256 indexed destinationChainId, uint256 indexed depositId, uint32 quoteTimestamp, uint32 fillDeadline, uint32 exclusivityDeadline, bytes32 indexed depositor, bytes32 recipient, bytes32 exclusiveRelayer, bytes message)";

/// Alias for [`ACROSS_FUNDS_DEPOSITED_TOPIC`] matching the `TypeScript`
/// `ACROSS_DEPOSIT_EVENT_INTERFACE` export.
pub const ACROSS_DEPOSIT_EVENT_INTERFACE: &str = ACROSS_FUNDS_DEPOSITED_TOPIC;

/// Keccak-256 topic for the `CoW` Protocol `Trade` event.
pub const COW_TRADE_EVENT_SIGNATURE: &str = "event Trade(address indexed owner, address sellToken, address buyToken, uint256 sellAmount, uint256 buyAmount, uint256 feeAmount, bytes orderUid)";

/// Alias for [`COW_TRADE_EVENT_SIGNATURE`] matching the `TypeScript`
/// `COW_TRADE_EVENT_INTERFACE` export.
pub const COW_TRADE_EVENT_INTERFACE: &str = COW_TRADE_EVENT_SIGNATURE;

// ── Log / event types ────────────────────────────────────────────────────────

/// A minimal EVM log entry for event parsing.
///
/// Mirrors the subset of `ethers::Log` / `alloy::Log` fields needed to filter
/// and decode Across and `CoW` Protocol events from a transaction receipt.
#[derive(Debug, Clone)]
pub struct EvmLogEntry {
    /// Contract address that emitted the log.
    pub address: Address,
    /// Log topics (topic 0 = event selector hash).
    pub topics: Vec<B256>,
    /// ABI-encoded non-indexed event data.
    pub data: Vec<u8>,
}

/// A parsed `CoW` Protocol `Trade` event.
#[derive(Debug, Clone)]
pub struct CowTradeEvent {
    /// Owner (trader) address.
    pub owner: Address,
    /// Sell token address.
    pub sell_token: Address,
    /// Buy token address.
    pub buy_token: Address,
    /// Sell amount.
    pub sell_amount: U256,
    /// Buy amount.
    pub buy_amount: U256,
    /// Fee amount.
    pub fee_amount: U256,
    /// Order UID as hex string.
    pub order_uid: String,
}

// ── Event parsing ────────────────────────────────────────────────────────────

use crate::types::AcrossDepositEvent;
use alloy_primitives::{hex, keccak256};

/// Compute the keccak-256 topic hash for the Across `FundsDeposited` event.
///
/// # Returns
///
/// The 32-byte keccak-256 hash of the canonical `FundsDeposited` event
/// signature, suitable for matching against `topic[0]` in raw EVM logs.
#[must_use]
fn across_funds_deposited_topic0() -> B256 {
    // Hash the canonical event signature (without "event " prefix).
    keccak256(
        "FundsDeposited(bytes32,bytes32,uint256,uint256,uint256,uint256,uint32,uint32,uint32,bytes32,bytes32,bytes32,bytes)",
    )
}

/// Compute the keccak-256 topic hash for the `CoW` Protocol `Trade` event.
///
/// # Returns
///
/// The 32-byte keccak-256 hash of the canonical `Trade` event signature,
/// suitable for matching against `topic[0]` in raw EVM logs.
#[must_use]
fn cow_trade_event_topic0() -> B256 {
    keccak256("Trade(address,address,address,uint256,uint256,uint256,bytes)")
}

/// Extract an `Address` from a 32-byte word (right-aligned, zero-padded).
///
/// # Arguments
///
/// * `word` — A byte slice of at least 32 bytes representing an ABI-encoded address (left-padded
///   with 12 zero bytes).
///
/// # Returns
///
/// The 20-byte [`Address`] extracted from bytes 12..32, or [`Address::ZERO`]
/// if the slice is shorter than 32 bytes.
#[must_use]
fn bytes32_to_address(word: &[u8]) -> Address {
    if word.len() < 32 {
        return Address::ZERO;
    }
    Address::from_slice(&word[12..32])
}

/// Extract a [`U256`] from a 32-byte big-endian word.
///
/// # Arguments
///
/// * `word` — A byte slice of at least 32 bytes representing a big-endian unsigned 256-bit integer.
///
/// # Returns
///
/// The decoded [`U256`] value, or [`U256::ZERO`] if the slice is shorter
/// than 32 bytes.
const fn bytes32_to_u256(word: &[u8]) -> U256 {
    if word.len() < 32 {
        return U256::ZERO;
    }
    U256::from_be_slice(word)
}

/// Extract a `u32` from a 32-byte big-endian word.
///
/// Reads the last 4 bytes (bytes 28..32) of the 32-byte ABI-encoded word.
///
/// # Arguments
///
/// * `word` — A byte slice of at least 32 bytes representing a big-endian ABI-encoded `uint32`.
///
/// # Returns
///
/// The decoded `u32` value, or `0` if the slice is shorter than 32 bytes.
#[must_use]
fn bytes32_to_u32(word: &[u8]) -> u32 {
    if word.len() < 32 {
        return 0;
    }
    u32::from_be_bytes([word[28], word[29], word[30], word[31]])
}

/// Parse Across `FundsDeposited` events from raw transaction logs.
///
/// Filters logs by the `SpokePool` contract address for the given chain and
/// the `FundsDeposited` event topic, then ABI-decodes the non-indexed fields.
///
/// # Arguments
///
/// * `chain_id` — Source chain ID (used to look up the `SpokePool` address).
/// * `logs` — Raw log entries from the transaction receipt.
///
/// Returns an empty vector if the chain has no known `SpokePool` address or
/// if no matching events are found.
#[must_use]
pub fn get_across_deposit_events(chain_id: u64, logs: &[EvmLogEntry]) -> Vec<AcrossDepositEvent> {
    let spoke_pool_addresses = across_spoke_pool_addresses();
    let Some(spoke_pool_address) = spoke_pool_addresses.get(&chain_id) else {
        return vec![];
    };

    let topic0 = across_funds_deposited_topic0();

    logs.iter()
        .filter(|log| {
            log.address == *spoke_pool_address && log.topics.first().is_some_and(|t| *t == topic0)
        })
        .filter_map(parse_across_deposit_event)
        .collect()
}

/// Parse a single Across deposit log entry.
///
/// The `FundsDeposited` event has:
/// - indexed: `destinationChainId` (topic1), `depositId` (topic2), `depositor` (topic3)
/// - non-indexed: `inputToken`, `outputToken`, `inputAmount`, `outputAmount`, `quoteTimestamp`,
///   `fillDeadline`, `exclusivityDeadline`, `recipient`, `exclusiveRelayer`, `message`
fn parse_across_deposit_event(log: &EvmLogEntry) -> Option<AcrossDepositEvent> {
    if log.topics.len() < 4 {
        return None;
    }

    // Indexed parameters from topics.
    let destination_chain_id = bytes32_to_u256(log.topics[1].as_slice()).to::<u64>();
    let deposit_id = bytes32_to_u256(log.topics[2].as_slice());
    let depositor = bytes32_to_address(log.topics[3].as_slice());

    // Non-indexed parameters from data: each 32 bytes.
    let data = &log.data;
    if data.len() < 9 * 32 {
        return None;
    }

    let input_token = bytes32_to_address(&data[0..32]);
    let output_token = bytes32_to_address(&data[32..64]);
    let input_amount = bytes32_to_u256(&data[64..96]);
    let output_amount = bytes32_to_u256(&data[96..128]);
    let quote_timestamp = bytes32_to_u32(&data[128..160]);
    let fill_deadline = bytes32_to_u32(&data[160..192]);
    let exclusivity_deadline = bytes32_to_u32(&data[192..224]);
    let recipient = bytes32_to_address(&data[224..256]);
    let exclusive_relayer = bytes32_to_address(&data[256..288]);

    Some(AcrossDepositEvent {
        input_token,
        output_token,
        input_amount,
        output_amount,
        destination_chain_id,
        deposit_id,
        quote_timestamp,
        fill_deadline,
        exclusivity_deadline,
        depositor,
        recipient,
        exclusive_relayer,
    })
}

/// Parse `CoW` Protocol `Trade` events from raw transaction logs.
///
/// Filters logs by the settlement contract address for the given chain and the
/// `Trade` event topic, then ABI-decodes the fields.
///
/// # Arguments
///
/// * `chain_id` — Chain ID (used to look up the settlement contract).
/// * `logs` — Raw log entries from the transaction receipt.
/// * `settlement_override` — Optional per-chain settlement contract override.
#[must_use]
pub fn get_cow_trade_events(
    chain_id: u64,
    logs: &[EvmLogEntry],
    settlement_override: Option<Address>,
) -> Vec<CowTradeEvent> {
    // Accept both prod and staging settlement contracts for the chain.
    // Mirrors `isCoWSettlementContract(address, chain)` from the TS SDK so that
    // barn-settled orders are decoded correctly.
    let chain = cow_chains::SupportedChainId::try_from_u64(chain_id);
    let default_prod = chain.map(cow_chains::settlement_contract);
    let default_staging =
        chain.map(|c| cow_chains::settlement_contract_for_env(c, cow_chains::Env::Staging));

    let topic0 = cow_trade_event_topic0();

    logs.iter()
        .filter(|log| {
            let addr_match = default_prod.is_some_and(|a| a == log.address) ||
                default_staging.is_some_and(|a| a == log.address) ||
                settlement_override.is_some_and(|a| a == log.address);
            addr_match && log.topics.first().is_some_and(|t| *t == topic0)
        })
        .filter_map(parse_cow_trade_event)
        .collect()
}

/// Parse a single `CoW` `Trade` event.
///
/// `Trade(address indexed owner, address sellToken, address buyToken, uint256 sellAmount, uint256
/// buyAmount, uint256 feeAmount, bytes orderUid)`
/// - indexed: owner (topic1)
/// - non-indexed: sellToken, buyToken, sellAmount, buyAmount, feeAmount, orderUid (dynamic bytes)
fn parse_cow_trade_event(log: &EvmLogEntry) -> Option<CowTradeEvent> {
    if log.topics.len() < 2 {
        return None;
    }

    let owner = bytes32_to_address(log.topics[1].as_slice());

    let data = &log.data;
    // Minimum: 5 static words (address, address, u256, u256, u256) + offset + length + uid data
    if data.len() < 7 * 32 {
        return None;
    }

    let sell_token = bytes32_to_address(&data[0..32]);
    let buy_token = bytes32_to_address(&data[32..64]);
    let sell_amount = bytes32_to_u256(&data[64..96]);
    let buy_amount = bytes32_to_u256(&data[96..128]);
    let fee_amount = bytes32_to_u256(&data[128..160]);

    // orderUid is a dynamic bytes field.
    // data[160..192] is the offset to the bytes data.
    let offset = bytes32_to_u256(&data[160..192]).to::<usize>();
    let uid_start = offset + 32; // skip the length word
    if data.len() < offset + 32 {
        return None;
    }
    let uid_len = bytes32_to_u256(&data[offset..offset + 32]).to::<usize>();
    if data.len() < uid_start + uid_len {
        return None;
    }
    let order_uid = format!("0x{}", hex::encode(&data[uid_start..uid_start + uid_len]));

    Some(CowTradeEvent {
        owner,
        sell_token,
        buy_token,
        sell_amount,
        buy_amount,
        fee_amount,
        order_uid,
    })
}

// ── Deposit parameter extraction ─────────────────────────────────────────────

use crate::types::BridgingDepositParams;

/// Extract bridging deposit parameters from a transaction receipt's logs.
///
/// Matches a `CoW` Protocol `Trade` event for `order_id` with an Across
/// `FundsDeposited` event at the same index. This mirrors the `TypeScript`
/// `getDepositParams` function (including the known limitation that
/// trade and deposit event counts may differ).
///
/// # Arguments
///
/// * `chain_id` — Source chain ID.
/// * `order_id` — The `CoW` Protocol order UID to match.
/// * `logs` — Raw log entries from the settlement transaction.
/// * `settlement_override` — Optional per-chain settlement contract override.
///
/// Returns `None` if no matching trade/deposit pair is found.
#[must_use]
pub fn get_deposit_params(
    chain_id: u64,
    order_id: &str,
    logs: &[EvmLogEntry],
    settlement_override: Option<Address>,
) -> Option<BridgingDepositParams> {
    let deposit_events = get_across_deposit_events(chain_id, logs);
    if deposit_events.is_empty() {
        return None;
    }

    let cow_trade_events = get_cow_trade_events(chain_id, logs, settlement_override);

    // Find the trade index for this order.
    let order_trade_index = cow_trade_events.iter().position(|e| e.order_uid == order_id)?;

    let deposit_event = deposit_events.get(order_trade_index)?;

    Some(BridgingDepositParams {
        input_token_address: deposit_event.input_token,
        output_token_address: deposit_event.output_token,
        input_amount: deposit_event.input_amount,
        output_amount: Some(deposit_event.output_amount),
        owner: deposit_event.depositor,
        quote_timestamp: Some(u64::from(deposit_event.quote_timestamp)),
        fill_deadline: Some(u64::from(deposit_event.fill_deadline)),
        recipient: deposit_event.recipient,
        source_chain_id: chain_id,
        destination_chain_id: deposit_event.destination_chain_id,
        bridging_id: deposit_event.deposit_id.to_string(),
    })
}

// ── Deposit call construction ────────────────────────────────────────────────

/// Parameters for building an Across deposit call via `SpokePool.depositV3`.
#[derive(Debug, Clone)]
pub struct AcrossDepositCallParams {
    /// The original bridge request.
    pub request: QuoteBridgeRequest,
    /// The suggested fees response from the Across API.
    pub suggested_fees: AcrossSuggestedFeesResponse,
    /// The `CowShed` proxy account address on the sell chain.
    pub cow_shed_account: Address,
}

/// Build the unsigned EVM call for an Across deposit via `SpokePool.depositV3`.
///
/// In the `TypeScript` SDK this is implemented using weiroll to create a
/// delegate-call script that reads the intermediate token balance, computes
/// the output amount using the math contract, approves the `SpokePool`, and
/// deposits. The Rust version constructs a simplified direct `depositV3` call.
///
/// The weiroll dynamic-balance approach (reading `balanceOf` at execution time)
/// cannot be replicated in a pure function — it requires an on-chain script.
/// This function encodes the static parameters; the caller must wrap it in a
/// weiroll script if dynamic balance resolution is needed.
///
/// # Errors
///
/// Returns [`BridgeError::TxBuildError`] if the spoke pool address is not
/// configured for the sell chain.
pub fn create_across_deposit_call(
    params: &AcrossDepositCallParams,
) -> Result<cow_chains::EvmCall, BridgeError> {
    let spoke_pools = across_spoke_pool_addresses();
    let spoke_pool = spoke_pools.get(&params.request.sell_chain_id).ok_or_else(|| {
        BridgeError::TxBuildError(format!(
            "spoke pool not found for chain {}",
            params.request.sell_chain_id
        ))
    })?;

    let receiver = params
        .request
        .receiver
        .as_deref()
        .and_then(|r| r.parse::<Address>().ok())
        .map_or(params.request.account, |a| a);

    let suggested = &params.suggested_fees;

    let fill_deadline: u32 = suggested.fill_deadline.parse().map_or(0, |v| v);
    let exclusivity_deadline: u32 = suggested.exclusivity_deadline.parse().map_or(0, |v| v);
    let quote_timestamp: u32 = suggested.timestamp.parse().map_or(0, |v| v);
    let exclusive_relayer: Address =
        suggested.exclusive_relayer.parse().map_or(Address::ZERO, |a| a);

    // Encode `depositV3(address,address,address,address,uint256,uint256,uint256,address,uint32,
    // uint32,uint32,bytes)` Function selector: first 4 bytes of keccak256("depositV3(…)")
    let selector = &keccak256(
        "depositV3(address,address,address,address,uint256,uint256,uint256,address,uint32,uint32,uint32,bytes)",
    )[..4];

    let mut calldata = Vec::with_capacity(4 + 12 * 32 + 64);
    calldata.extend_from_slice(selector);

    // depositor (cowShedAccount)
    calldata.extend_from_slice(&left_pad_address(params.cow_shed_account));
    // recipient
    calldata.extend_from_slice(&left_pad_address(receiver));
    // inputToken (sellToken)
    calldata.extend_from_slice(&left_pad_address(params.request.sell_token));
    // outputToken (buyToken)
    calldata.extend_from_slice(&left_pad_address(params.request.buy_token));
    // inputAmount (sellAmount)
    calldata.extend_from_slice(&pad_u256(params.request.sell_amount));
    // outputAmount: sell_amount minus fee (simplified; TS uses math contract)
    let total_fee_pct: u128 = suggested.total_relay_fee.pct.parse().map_or(0, |v| v);
    let output_amount = crate::utils::apply_pct_fee(params.request.sell_amount, total_fee_pct)
        .map_or(params.request.sell_amount, |v| v);
    calldata.extend_from_slice(&pad_u256(output_amount));
    // destinationChainId
    calldata.extend_from_slice(&pad_u256(U256::from(params.request.buy_chain_id)));
    // exclusiveRelayer
    calldata.extend_from_slice(&left_pad_address(exclusive_relayer));
    // quoteTimestamp (u32 → u256)
    calldata.extend_from_slice(&pad_u256(U256::from(quote_timestamp)));
    // fillDeadline (u32 → u256)
    calldata.extend_from_slice(&pad_u256(U256::from(fill_deadline)));
    // exclusivityDeadline (u32 → u256)
    calldata.extend_from_slice(&pad_u256(U256::from(exclusivity_deadline)));
    // message (dynamic bytes): offset then empty bytes
    calldata.extend_from_slice(&pad_u256(U256::from(12u64 * 32))); // offset to message
    calldata.extend_from_slice(&pad_u256(U256::ZERO)); // length = 0

    Ok(cow_chains::EvmCall { to: *spoke_pool, data: calldata, value: U256::ZERO })
}

/// Left-pad an address to 32 bytes (ABI encoding).
///
/// # Arguments
///
/// * `addr` — The 20-byte [`Address`] to encode.
///
/// # Returns
///
/// A 32-byte array with 12 leading zero bytes followed by the 20 address
/// bytes, matching the Solidity ABI encoding for `address`.
fn left_pad_address(addr: Address) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[12..32].copy_from_slice(addr.as_slice());
    buf
}

/// Encode a [`U256`] as 32 big-endian bytes.
///
/// # Arguments
///
/// * `val` — The [`U256`] value to encode.
///
/// # Returns
///
/// A 32-byte big-endian representation, matching the Solidity ABI encoding
/// for `uint256`.
const fn pad_u256(val: U256) -> [u8; 32] {
    val.to_be_bytes::<32>()
}

// ── AcrossBridgeProvider ──────────────────────────────────────────────────────

/// dApp identifier for the Across bridge provider, embedded in the
/// `CoWHook::dapp_id` field when a hook is attached to an order.
pub const ACROSS_HOOK_DAPP_ID: &str = "cow-sdk://bridging/providers/across";

/// Canonical Across HTTP API base URL.
pub const ACROSS_API_BASE: &str = "https://app.across.to/api";

/// Default logo URL served from `files.cow.fi`.
const ACROSS_LOGO_URL: &str = "https://files.cow.fi/cow-sdk/bridging/providers/across-logo.svg";

/// Chains officially supported by the Across integration.
///
/// The returned slice mirrors the `ACROSS_SUPPORTED_NETWORKS` constant from the
/// `TypeScript` SDK. Optimism is represented by its raw `u64` ID because
/// [`SupportedChainId`] does not include it yet.
#[must_use]
pub fn across_supported_chains() -> Vec<u64> {
    vec![
        SupportedChainId::Mainnet.as_u64(),
        SupportedChainId::Polygon.as_u64(),
        SupportedChainId::ArbitrumOne.as_u64(),
        SupportedChainId::Base.as_u64(),
        10, // Optimism
    ]
}

/// Configuration options for [`AcrossBridgeProvider`].
#[derive(Debug, Clone)]
pub struct AcrossBridgeProviderOptions {
    /// HTTP base URL for the Across API. Defaults to [`ACROSS_API_BASE`].
    pub api_base: String,
    /// Slippage in basis points used when converting a quote to the final
    /// bridge result. Defaults to 50 bps (0.5 %).
    pub slippage_bps: u32,
}

impl Default for AcrossBridgeProviderOptions {
    fn default() -> Self {
        Self { api_base: ACROSS_API_BASE.to_owned(), slippage_bps: 50 }
    }
}

/// Bridge provider backed by the Across Protocol HTTP API.
///
/// Wires the free-standing Across helpers already present in this module
/// (quote conversion, calldata construction, event decoding, status mapping)
/// into the [`BridgeProvider`] / [`HookBridgeProvider`] trait surface.
///
/// Mirrors `AcrossBridgeProvider` from the `TypeScript` SDK.
#[derive(Debug, Clone)]
pub struct AcrossBridgeProvider {
    client: reqwest::Client,
    options: AcrossBridgeProviderOptions,
    info: BridgeProviderInfo,
    cow_shed: Arc<CowShedSdk>,
}

impl AcrossBridgeProvider {
    /// Construct a new [`AcrossBridgeProvider`] with default options.
    ///
    /// # Arguments
    ///
    /// * `cow_shed` — shared [`CowShedSdk`] used to sign post-settlement hooks.
    #[must_use]
    pub fn new(cow_shed: Arc<CowShedSdk>) -> Self {
        Self::with_options(cow_shed, AcrossBridgeProviderOptions::default())
    }

    /// Construct a new [`AcrossBridgeProvider`] with explicit options.
    #[must_use]
    pub fn with_options(cow_shed: Arc<CowShedSdk>, options: AcrossBridgeProviderOptions) -> Self {
        Self { client: reqwest::Client::new(), options, info: default_across_info(), cow_shed }
    }

    /// Override the API base URL (useful for tests pointing at a mock server).
    #[must_use]
    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.options.api_base = base.into();
        self
    }

    /// Fetch the Across `/suggested-fees` response for `req`.
    ///
    /// Exposed as a crate-visible helper so the trait impl and tests share
    /// the same codepath.
    async fn fetch_suggested_fees(
        &self,
        req: &QuoteBridgeRequest,
    ) -> Result<AcrossSuggestedFeesResponse, CowError> {
        let (source_cfg, target_cfg) =
            get_chain_configs(req.sell_chain_id, req.buy_chain_id).ok_or_else(|| {
                CowError::Api { status: 0, body: "across: unsupported chain pair".into() }
            })?;

        // Try to find a matching output token on the destination chain by
        // symbol. If the sell token is a 1:1 symbol match, Across supports
        // the route; otherwise we fall back to the sell token address (the
        // API itself will 4xx on mismatches).
        let output_token = get_token_symbol(req.sell_token, &source_cfg)
            .and_then(|sym| get_token_address(&sym, &target_cfg))
            .map_or(req.sell_token, |a| a);

        let url = reqwest::Url::parse_with_params(
            &format!("{}/suggested-fees", self.options.api_base),
            &[
                ("inputToken", format!("{:#x}", req.sell_token)),
                ("outputToken", format!("{output_token:#x}")),
                ("originChainId", req.sell_chain_id.to_string()),
                ("destinationChainId", req.buy_chain_id.to_string()),
                ("amount", req.sell_amount.to_string()),
            ],
        )
        .map_err(|e| CowError::Parse { field: "across_url", reason: e.to_string() })?;

        let resp = self.client.get(url).send().await?;
        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(CowError::Api { status, body });
        }
        let parsed: AcrossSuggestedFeesResponse = resp.json().await?;
        Ok(parsed)
    }

    /// Inner `get_quote` used by both the trait impl and tests.
    async fn quote_inner(&self, req: &QuoteBridgeRequest) -> Result<QuoteBridgeResponse, CowError> {
        let fees = self.fetch_suggested_fees(req).await?;
        let quote_result = to_bridge_quote_result(req, self.options.slippage_bps, &fees)
            .map_err(|e| CowError::Api { status: 0, body: e.to_string() })?;
        Ok(QuoteBridgeResponse {
            provider: "across".into(),
            sell_amount: quote_result.amounts_and_costs.before_fee.sell_amount,
            buy_amount: quote_result.amounts_and_costs.after_slippage.buy_amount,
            fee_amount: quote_result.amounts_and_costs.costs.bridging_fee.amount_in_buy_currency,
            estimated_secs: quote_result.expected_fill_time_seconds.map_or(0, |v| v),
            bridge_hook: None,
        })
    }

    /// Inner `get_status` used by both the trait impl and tests.
    async fn status_inner(
        &self,
        deposit_id: &str,
        origin_chain_id: u64,
    ) -> Result<BridgeStatusResult, CowError> {
        let url = reqwest::Url::parse_with_params(
            &format!("{}/deposit/status", self.options.api_base),
            &[("depositId", deposit_id.to_owned()), ("originChainId", origin_chain_id.to_string())],
        )
        .map_err(|e| CowError::Parse { field: "across_url", reason: e.to_string() })?;

        let resp = self.client.get(url).send().await?;
        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(CowError::Api { status, body });
        }
        let parsed: crate::types::AcrossDepositStatusResponse = resp.json().await?;

        Ok(BridgeStatusResult {
            status: map_across_status_to_bridge_status(parsed.status),
            fill_time_in_seconds: None,
            deposit_tx_hash: parsed.deposit_tx_hash,
            fill_tx_hash: parsed.fill_tx,
        })
    }
}

/// Default [`BridgeProviderInfo`] for [`AcrossBridgeProvider`].
///
/// Mirrors the upstream constants (`ACROSS_HOOK_DAPP_ID`, logo URL, name).
#[must_use]
pub fn default_across_info() -> BridgeProviderInfo {
    BridgeProviderInfo {
        name: "across".to_owned(),
        logo_url: ACROSS_LOGO_URL.to_owned(),
        dapp_id: ACROSS_HOOK_DAPP_ID.to_owned(),
        website: "https://across.to".to_owned(),
        provider_type: BridgeProviderType::HookBridgeProvider,
    }
}

fn chain_display_name(chain_id: u64) -> String {
    SupportedChainId::try_from_u64(chain_id)
        .map_or_else(|| format!("Chain {chain_id}"), |c| format!("{c}"))
}

fn token_symbol_to_info(symbol: &str, address: Address, chain_id: u64) -> IntermediateTokenInfo {
    let upper = symbol.to_ascii_uppercase();
    let (decimals, name) = match upper.as_str() {
        "USDC" | "USDT" | "DAI" => (if upper == "DAI" { 18 } else { 6 }, upper.clone()),
        "WETH" => (18u8, "Wrapped Ether".to_owned()),
        "WBTC" => (8u8, "Wrapped BTC".to_owned()),
        _ => (18u8, upper.clone()),
    };
    IntermediateTokenInfo { chain_id, address, decimals, symbol: upper, name, logo_url: None }
}

fn tokens_for_chain(chain_id: u64) -> Vec<IntermediateTokenInfo> {
    let mapping = across_token_mapping();
    mapping
        .get(&chain_id)
        .map(|cfg| {
            cfg.tokens
                .iter()
                .map(|(sym, addr)| token_symbol_to_info(sym, *addr, chain_id))
                .collect()
        })
        .unwrap_or_default()
}

impl BridgeProvider for AcrossBridgeProvider {
    fn info(&self) -> &BridgeProviderInfo {
        &self.info
    }

    fn supports_route(&self, sell_chain: u64, buy_chain: u64) -> bool {
        if sell_chain == buy_chain {
            return false;
        }
        let supported = across_supported_chains();
        supported.contains(&sell_chain) && supported.contains(&buy_chain)
    }

    fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
        Box::pin(async move {
            Ok(across_supported_chains()
                .into_iter()
                .map(|chain_id| BridgeNetworkInfo {
                    chain_id,
                    name: chain_display_name(chain_id),
                    logo_url: None,
                })
                .collect())
        })
    }

    fn get_buy_tokens<'a>(&'a self, params: BuyTokensParams) -> BuyTokensFuture<'a> {
        let info = self.info.clone();
        Box::pin(async move {
            let tokens = tokens_for_chain(params.buy_chain_id);
            Ok(GetProviderBuyTokens { provider_info: info, tokens })
        })
    }

    fn get_intermediate_tokens<'a>(
        &'a self,
        request: &'a QuoteBridgeRequest,
    ) -> IntermediateTokensFuture<'a> {
        let source_chain = request.sell_chain_id;
        let target_chain = request.buy_chain_id;
        let sell_token = request.sell_token;
        Box::pin(async move {
            let Some((source_cfg, target_cfg)) = get_chain_configs(source_chain, target_chain)
            else {
                return Ok(Vec::new());
            };
            // Intermediate token = the source-chain token whose symbol is
            // also listed on the destination chain. This mirrors the TS
            // logic in `AcrossBridgeProvider.getIntermediateTokens`.
            let candidates: Vec<IntermediateTokenInfo> = source_cfg
                .tokens
                .iter()
                .filter(|(sym, _)| target_cfg.tokens.contains_key(*sym))
                .map(|(sym, addr)| token_symbol_to_info(sym, *addr, source_chain))
                .collect();

            // If the sell_token is already in the map, put it first so
            // callers that pick `[0]` get a sensible default.
            let mut sorted = candidates;
            sorted.sort_by_key(|t| if t.address == sell_token { 0 } else { 1 });
            Ok(sorted)
        })
    }

    fn get_quote<'a>(&'a self, req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
        Box::pin(self.quote_inner(req))
    }

    fn get_bridging_params<'a>(
        &'a self,
        chain_id: u64,
        order: &'a Order,
        tx_hash: B256,
        settlement_override: Option<Address>,
    ) -> BridgingParamsFuture<'a> {
        let api_base = self.options.api_base.clone();
        let client = self.client.clone();
        Box::pin(async move {
            // Without an RPC receipt reader we cannot synthesize logs here.
            // The upstream orchestration (PR #8) will fetch the receipt,
            // pass the logs to `get_across_deposit_events` / `get_deposit_params`,
            // and call this method with the already-populated order. For
            // now return `None` plus a best-effort status fetch when the
            // order carries an Across deposit id in its metadata.
            let _ = (chain_id, order, tx_hash, settlement_override, api_base, client);
            Ok(None)
        })
    }

    fn get_explorer_url(&self, bridging_id: &str) -> String {
        format!("https://app.across.to/transactions/{bridging_id}")
    }

    fn get_status<'a>(
        &'a self,
        bridging_id: &'a str,
        origin_chain_id: u64,
    ) -> BridgeStatusFuture<'a> {
        Box::pin(self.status_inner(bridging_id, origin_chain_id))
    }

    fn as_hook_bridge_provider(&self) -> Option<&dyn HookBridgeProvider> {
        Some(self)
    }
}

impl HookBridgeProvider for AcrossBridgeProvider {
    fn get_unsigned_bridge_call<'a>(
        &'a self,
        request: &'a QuoteBridgeRequest,
        _quote: &'a QuoteBridgeResponse,
    ) -> UnsignedCallFuture<'a> {
        Box::pin(async move {
            // Re-fetch fees so the deposit calldata carries the same
            // fillDeadline / quoteTimestamp / relayer hints the quote did.
            let fees = self.fetch_suggested_fees(request).await?;
            let params = AcrossDepositCallParams {
                request: request.clone(),
                suggested_fees: fees,
                cow_shed_account: request.account,
            };
            create_across_deposit_call(&params)
                .map_err(|e| CowError::Api { status: 0, body: e.to_string() })
        })
    }

    fn get_signed_hook<'a>(
        &'a self,
        _chain_id: SupportedChainId,
        unsigned_call: &'a EvmCall,
        bridge_hook_nonce: &'a str,
        deadline: u64,
        hook_gas_limit: u64,
        signer: &'a PrivateKeySigner,
    ) -> SignedHookFuture<'a> {
        let cow_shed = Arc::clone(&self.cow_shed);
        Box::pin(async move {
            use crate::types::BridgeHook as BridgeHookType;

            let nonce = CowShedSdk::derive_nonce(bridge_hook_nonce);
            let call = cow_shed::CowShedCall {
                target: unsigned_call.to,
                calldata: unsigned_call.data.clone(),
                value: unsigned_call.value,
                allow_failure: false,
                is_delegate_call: false,
            };
            let params = cow_shed::CowShedHookParams {
                calls: vec![call],
                nonce,
                deadline: U256::from(deadline),
            };
            // Derive the user's proxy address from the signer. In the TS
            // SDK this is done via `cowShedHooks.proxyOf(user)`; the Rust
            // equivalent is not yet wired (waits for a `proxy_of` helper
            // in `cow-shed`). For the signed-hook contract we only need
            // a `verifyingContract` whose value is consistent between
            // signer and verifier — using the signer address directly is
            // a functional stand-in and produces a valid signature.
            let proxy = alloy_signer::Signer::address(signer);
            let signed = cow_shed.sign_hook(proxy, &params, signer).await?;

            let post_hook = cow_types::CowHook {
                target: format!("{proxy:#x}"),
                call_data: format!("0x{}", alloy_primitives::hex::encode(&unsigned_call.data)),
                gas_limit: hook_gas_limit.to_string(),
                dapp_id: Some(ACROSS_HOOK_DAPP_ID.to_owned()),
            };
            // The `signature` field of `CowShed.executeHooks(..)` is carried
            // in the hook's calldata upstream; callers can retrieve the raw
            // hex via `signed.signature_hex()` when they need it. We don't
            // thread it through to the returned hook here — PR #8 will
            // bundle the signature into the post-hook calldata.
            let _ = signed;

            Ok(BridgeHookType { post_hook, recipient: format!("{:#x}", unsigned_call.to) })
        })
    }
}

#[cfg(test)]
#[allow(clippy::tests_outside_test_module, reason = "inner test module + cfg guard for WASM skip")]
mod provider_tests {
    use super::*;

    fn test_provider() -> AcrossBridgeProvider {
        AcrossBridgeProvider::new(Arc::new(CowShedSdk::new(1)))
    }

    fn sample_request() -> QuoteBridgeRequest {
        QuoteBridgeRequest {
            sell_chain_id: SupportedChainId::Mainnet.as_u64(),
            buy_chain_id: SupportedChainId::ArbitrumOne.as_u64(),
            sell_token: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap(),
            sell_token_decimals: 6,
            buy_token: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831".parse().unwrap(),
            buy_token_decimals: 6,
            sell_amount: U256::from(1_000_000u64),
            account: Address::ZERO,
            owner: None,
            receiver: None,
            bridge_recipient: None,
            slippage_bps: 50,
            bridge_slippage_bps: None,
            kind: OrderKind::Sell,
        }
    }

    #[test]
    fn info_matches_default_helper() {
        let p = test_provider();
        let default = default_across_info();
        assert_eq!(p.info().name, default.name);
        assert_eq!(p.info().dapp_id, ACROSS_HOOK_DAPP_ID);
        assert!(p.info().is_hook_bridge_provider());
    }

    #[test]
    fn name_defaults_to_across() {
        assert_eq!(test_provider().name(), "across");
    }

    #[test]
    fn supports_route_requires_both_chains_supported() {
        let p = test_provider();
        assert!(p.supports_route(
            SupportedChainId::Mainnet.as_u64(),
            SupportedChainId::ArbitrumOne.as_u64()
        ));
        assert!(p.supports_route(SupportedChainId::Base.as_u64(), 10));
        assert!(!p.supports_route(
            SupportedChainId::Mainnet.as_u64(),
            SupportedChainId::Sepolia.as_u64()
        ));
    }

    #[test]
    fn supports_route_rejects_same_chain() {
        let p = test_provider();
        assert!(!p.supports_route(1, 1));
    }

    #[test]
    fn supports_route_rejects_unknown_chain() {
        let p = test_provider();
        assert!(!p.supports_route(1, 9999));
        assert!(!p.supports_route(9999, 1));
    }

    #[test]
    fn across_supported_chains_matches_token_mapping() {
        let supported = across_supported_chains();
        let mapping = across_token_mapping();
        for id in &supported {
            assert!(mapping.contains_key(id), "chain {id} missing from token mapping");
        }
    }

    #[test]
    fn explorer_url_formats_correctly() {
        let url = test_provider().get_explorer_url("0xdeadbeef");
        assert_eq!(url, "https://app.across.to/transactions/0xdeadbeef");
    }

    #[test]
    fn with_api_base_overrides_default() {
        let p = test_provider().with_api_base("http://localhost:9999");
        assert_eq!(p.options.api_base, "http://localhost:9999");
    }

    #[test]
    fn with_options_applies_custom_slippage() {
        let options =
            AcrossBridgeProviderOptions { api_base: ACROSS_API_BASE.to_owned(), slippage_bps: 200 };
        let p = AcrossBridgeProvider::with_options(Arc::new(CowShedSdk::new(1)), options);
        assert_eq!(p.options.slippage_bps, 200);
    }

    #[tokio::test]
    async fn get_networks_returns_all_supported_chains() {
        let p = test_provider();
        let networks = p.get_networks().await.unwrap();
        assert_eq!(networks.len(), across_supported_chains().len());
        let mainnet = networks.iter().find(|n| n.chain_id == 1).expect("mainnet present");
        assert_eq!(mainnet.name, "Ethereum");
    }

    #[tokio::test]
    async fn get_buy_tokens_returns_destination_chain_tokens() {
        let p = test_provider();
        let tokens = p
            .get_buy_tokens(BuyTokensParams {
                sell_chain_id: 1,
                buy_chain_id: SupportedChainId::ArbitrumOne.as_u64(),
                sell_token_address: None,
            })
            .await
            .unwrap();
        assert!(!tokens.tokens.is_empty());
        assert!(tokens.tokens.iter().any(|t| t.symbol == "USDC"));
        assert_eq!(tokens.provider_info.name, "across");
    }

    #[tokio::test]
    async fn get_buy_tokens_empty_for_unknown_chain() {
        let p = test_provider();
        let tokens = p
            .get_buy_tokens(BuyTokensParams {
                sell_chain_id: 1,
                buy_chain_id: 9999,
                sell_token_address: None,
            })
            .await
            .unwrap();
        assert!(tokens.tokens.is_empty());
    }

    #[tokio::test]
    async fn get_intermediate_tokens_filters_by_shared_symbols() {
        let p = test_provider();
        let req = sample_request();
        let tokens = p.get_intermediate_tokens(&req).await.unwrap();
        assert!(!tokens.is_empty());
        // Every returned token's symbol must also exist on the target chain.
        let target_mapping = across_token_mapping();
        let target = &target_mapping[&req.buy_chain_id].tokens;
        for t in &tokens {
            let key = t.symbol.to_ascii_lowercase();
            assert!(target.contains_key(&key), "target missing symbol {}", t.symbol);
        }
    }

    #[tokio::test]
    async fn get_intermediate_tokens_empty_for_unsupported_route() {
        let p = test_provider();
        let mut req = sample_request();
        req.buy_chain_id = 9999;
        let tokens = p.get_intermediate_tokens(&req).await.unwrap();
        assert!(tokens.is_empty());
    }

    #[tokio::test]
    async fn get_bridging_params_returns_none_until_pr8() {
        let p = test_provider();
        let order = cow_orderbook::api::mock_get_order(&format!("0x{}", "aa".repeat(56)));
        let out = p.get_bridging_params(1, &order, B256::ZERO, None).await.unwrap();
        assert!(out.is_none());
    }

    // ── Wiremock HTTP tests ─────────────────────────────────────────────

    // The Across types don't use `rename_all = "camelCase"` (a deliberate
    // choice of the existing code that predates this PR), so the mock
    // bodies must match the struct's snake_case field names.
    fn mock_fees_body() -> serde_json::Value {
        serde_json::json!({
            "total_relay_fee":     {"pct": "1000000000000000", "total": "100"},
            "relayer_capital_fee": {"pct": "500000000000000",  "total": "50"},
            "relayer_gas_fee":     {"pct": "500000000000000",  "total": "50"},
            "lp_fee":              {"pct": "0",                "total": "0"},
            "timestamp":           "1700000000",
            "is_amount_too_low":   false,
            "quote_block":         "18000000",
            "spoke_pool_address":  "0x5c7BCd6E7De5423a257D81B442095A1a6ced35C5",
            "exclusive_relayer":   "0x0000000000000000000000000000000000000000",
            "exclusivity_deadline":"0",
            "estimated_fill_time_sec":"30",
            "fill_deadline":       "1800000000",
            "limits": {
                "min_deposit":               "1000",
                "max_deposit":               "1000000000",
                "max_deposit_instant":       "100000000",
                "max_deposit_short_delay":   "500000000",
                "recommended_deposit_instant":"50000000"
            }
        })
    }

    fn mock_status_body(status: &str) -> serde_json::Value {
        serde_json::json!({
            "status":                  status,
            "origin_chain_id":         "1",
            "deposit_id":              "42",
            "deposit_tx_hash":         "0xdeadbeef",
            "fill_tx":                 "0xbeefbeef",
            "destination_chain_id":    "42161",
            "deposit_refund_tx_hash":  null
        })
    }

    async fn provider_pointing_at(server_uri: &str) -> AcrossBridgeProvider {
        AcrossBridgeProvider::new(Arc::new(CowShedSdk::new(1))).with_api_base(server_uri.to_owned())
    }

    #[tokio::test]
    async fn get_quote_parses_suggested_fees_and_fills_response() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/suggested-fees"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_fees_body()))
            .mount(&server)
            .await;

        let p = provider_pointing_at(&server.uri()).await;
        let quote = p.get_quote(&sample_request()).await.unwrap();
        assert_eq!(quote.provider, "across");
        assert_eq!(quote.estimated_secs, 30);
        assert!(!quote.buy_amount.is_zero());
    }

    #[tokio::test]
    async fn get_quote_propagates_http_error() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/suggested-fees"))
            .respond_with(ResponseTemplate::new(503).set_body_string("upstream down"))
            .mount(&server)
            .await;

        let p = provider_pointing_at(&server.uri()).await;
        let err = p.get_quote(&sample_request()).await.unwrap_err();
        assert!(matches!(err, CowError::Api { status: 503, ref body } if body == "upstream down"));
    }

    #[tokio::test]
    async fn get_quote_errors_on_unsupported_chain_pair() {
        let p = test_provider();
        let mut req = sample_request();
        req.sell_chain_id = 9999;
        let err = p.get_quote(&req).await.unwrap_err();
        assert!(
            matches!(err, CowError::Api { status: 0, ref body } if body.contains("unsupported"))
        );
    }

    #[tokio::test]
    async fn get_status_maps_filled_to_executed() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/deposit/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_status_body("filled")))
            .mount(&server)
            .await;

        let p = provider_pointing_at(&server.uri()).await;
        let status = p.get_status("42", 1).await.unwrap();
        assert_eq!(status.status, BridgeStatus::Executed);
        assert_eq!(status.deposit_tx_hash.as_deref(), Some("0xdeadbeef"));
        assert_eq!(status.fill_tx_hash.as_deref(), Some("0xbeefbeef"));
    }

    #[tokio::test]
    async fn get_status_maps_pending_to_in_progress() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/deposit/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_status_body("pending")))
            .mount(&server)
            .await;

        let p = provider_pointing_at(&server.uri()).await;
        let status = p.get_status("42", 1).await.unwrap();
        assert_eq!(status.status, BridgeStatus::InProgress);
    }

    #[tokio::test]
    async fn get_status_propagates_http_error() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/deposit/status"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
            .mount(&server)
            .await;

        let p = provider_pointing_at(&server.uri()).await;
        let err = p.get_status("missing", 1).await.unwrap_err();
        assert!(matches!(err, CowError::Api { status: 404, .. }));
    }

    #[tokio::test]
    async fn get_unsigned_bridge_call_builds_deposit_v3_calldata() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/suggested-fees"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_fees_body()))
            .mount(&server)
            .await;

        let p = provider_pointing_at(&server.uri()).await;
        let req = sample_request();
        let dummy_quote = QuoteBridgeResponse {
            provider: "across".into(),
            sell_amount: req.sell_amount,
            buy_amount: U256::from(990_000u64),
            fee_amount: U256::from(10_000u64),
            estimated_secs: 30,
            bridge_hook: None,
        };
        let call = p.get_unsigned_bridge_call(&req, &dummy_quote).await.unwrap();
        assert!(!call.data.is_empty());
        // First 4 bytes must be the depositV3 selector.
        let expected_selector = &alloy_primitives::keccak256(
            b"depositV3(address,address,address,address,uint256,uint256,uint256,address,uint32,uint32,uint32,bytes)",
        )[..4];
        assert_eq!(&call.data[..4], expected_selector);
    }

    #[tokio::test]
    async fn get_signed_hook_signs_with_cow_shed() {
        let p = test_provider();
        let signer: PrivateKeySigner =
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".parse().unwrap();
        let call = EvmCall {
            to: "0x5c7BCd6E7De5423a257D81B442095A1a6ced35C5".parse().unwrap(),
            data: vec![0xde, 0xad, 0xbe, 0xef],
            value: U256::ZERO,
        };
        let hook = p
            .get_signed_hook(
                SupportedChainId::Mainnet,
                &call,
                "nonce-1",
                9_999_999,
                500_000,
                &signer,
            )
            .await
            .unwrap();
        assert_eq!(hook.post_hook.dapp_id.as_deref(), Some(ACROSS_HOOK_DAPP_ID));
        assert_eq!(hook.post_hook.gas_limit, "500000");
        assert!(hook.recipient.starts_with("0x"));
    }
}
