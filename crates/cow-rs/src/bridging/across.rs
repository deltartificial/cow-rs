//! Across Protocol bridge provider — types, constants, and utility functions.

use foldhash::HashMap;

use alloy_primitives::{Address, U256};

use crate::{
    OrderKind,
    bridging::{
        types::{
            AcrossChainConfig, AcrossDepositStatus, AcrossSuggestedFeesResponse, BridgeAmounts,
            BridgeCosts, BridgeError, BridgeFees, BridgeLimits, BridgeQuoteAmountsAndCosts,
            BridgeQuoteResult, BridgeStatus, BridgingFee,
        },
        utils::{apply_bps, apply_pct_fee, pct_to_bps},
    },
    config::SupportedChainId,
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
/// use cow_rs::bridging::across::across_spoke_pool_addresses;
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
/// use cow_rs::bridging::across::across_math_contract_addresses;
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
/// use cow_rs::bridging::across::across_token_mapping;
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
/// use cow_rs::bridging::across::get_chain_configs;
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
/// use cow_rs::bridging::across::{across_token_mapping, get_token_symbol};
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
/// use cow_rs::bridging::across::{across_token_mapping, get_token_address};
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
/// use cow_rs::bridging::across::{across_token_mapping, get_token_by_address_and_chain_id};
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
/// use cow_rs::bridging::{
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

use alloy_primitives::B256;

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

use crate::bridging::types::AcrossDepositEvent;
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
    // Resolve settlement contract address for this chain.
    let chain = crate::config::SupportedChainId::try_from_u64(chain_id);
    let default_settlement = chain.map(crate::config::settlement_contract);

    let topic0 = cow_trade_event_topic0();

    logs.iter()
        .filter(|log| {
            let addr_match = default_settlement.is_some_and(|a| a == log.address) ||
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

use crate::bridging::types::BridgingDepositParams;

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

use crate::bridging::types::QuoteBridgeRequest;

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
) -> Result<crate::config::EvmCall, BridgeError> {
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
    let output_amount =
        crate::bridging::utils::apply_pct_fee(params.request.sell_amount, total_fee_pct)
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

    Ok(crate::config::EvmCall { to: *spoke_pool, data: calldata, value: U256::ZERO })
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
