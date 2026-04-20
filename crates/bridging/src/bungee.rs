//! Bridge provider backed by the Bungee (Socket) aggregator API.
//!
//! Includes deposit call construction, transaction data decoding,
//! status tracking, and response validation.

use foldhash::HashMap;

use std::sync::Arc;

use alloy_primitives::{Address, B256, U256};

use cow_errors::CowError;
use cow_orderbook::types::Order;

use alloy_signer_local::PrivateKeySigner;
use cow_chains::{EvmCall, SupportedChainId};
use cow_shed::CowShedSdk;

use super::{
    provider::{
        BridgeNetworkInfo, BridgeProvider, BridgeStatusFuture, BridgingParamsFuture,
        BuyTokensFuture, HookBridgeProvider, IntermediateTokensFuture, NetworksFuture, QuoteFuture,
        SignedHookFuture, UnsignedCallFuture,
    },
    types::{
        BridgeAmounts, BridgeCosts, BridgeError, BridgeFees, BridgeLimits, BridgeProviderInfo,
        BridgeProviderType, BridgeQuoteAmountsAndCosts, BridgeQuoteResult, BridgeStatus,
        BridgeStatusResult, BridgingFee, BungeeBridge, BungeeBridgeName, BungeeEvent,
        BungeeEventStatus, BungeeTxDataBytesIndex, BuyTokensParams, DecodedBungeeAmounts,
        DecodedBungeeTxData, GetProviderBuyTokens, IntermediateTokenInfo, QuoteBridgeRequest,
        QuoteBridgeResponse,
    },
    utils::{apply_bps, calculate_fee_bps},
};

// ── Contract addresses ────────────────────────────────────────────────────────

/// Bungee `ApproveAndBridge` V1 contract address (same on all supported chains).
pub const BUNGEE_APPROVE_AND_BRIDGE_V1_ADDRESS: &str = "0xD06a673fe1fa27B1b9E5BA0be980AB15Dbce85cc";

/// Bungee `CoW` Swap library address (same on most supported chains).
pub const BUNGEE_COWSWAP_LIB_ADDRESS: &str = "0x75b6ba5fcab20848ca00f132d253638fea82e598";

/// Socket verifier address.
pub const SOCKET_VERIFIER_ADDRESS: &str = "0xa27A3f5A96DF7D8Be26EE2790999860C00eb688D";

/// Per-chain `ApproveAndBridge` V1 contract addresses.
///
/// Builds a mapping from chain ID to the canonical `ApproveAndBridge` V1
/// contract address for every chain supported by the Bungee integration.
///
/// # Returns
///
/// A [`HashMap`] keyed by chain ID (`u64`) whose values are the parsed
/// [`Address`] of the `ApproveAndBridge` V1 contract.  Returns an empty
/// map if the hard-coded address constant fails to parse (should never
/// happen in practice).
#[must_use]
pub fn bungee_approve_and_bridge_v1_addresses() -> HashMap<u64, Address> {
    // Safety: the address literal is a valid hex address.
    let Ok(addr) = BUNGEE_APPROVE_AND_BRIDGE_V1_ADDRESS.parse::<Address>() else {
        return HashMap::default();
    };
    let mut m = HashMap::default();
    for chain in &[
        SupportedChainId::Mainnet,
        SupportedChainId::GnosisChain,
        SupportedChainId::ArbitrumOne,
        SupportedChainId::Base,
        SupportedChainId::Avalanche,
        SupportedChainId::Polygon,
    ] {
        m.insert(chain.as_u64(), addr);
    }
    // Optimism (chain ID 10)
    m.insert(10, addr);
    m
}

// ── Tx data byte indices ──────────────────────────────────────────────────────

/// Return the calldata byte-offset index for a given bridge + function selector.
///
/// These indices describe where the input amount lives in the raw `SocketGateway`
/// calldata (after `0x` + routeId + function selector).
///
/// # Returns
///
/// `Some(`[`BungeeTxDataBytesIndex`]`)` containing the byte and hex-string
/// offsets for the input amount field, or `None` if the bridge / selector
/// combination is not recognised.
#[must_use]
pub fn bungee_tx_data_bytes_index(
    bridge: BungeeBridge,
    function_selector: &str,
) -> Option<BungeeTxDataBytesIndex> {
    let selector = function_selector.to_lowercase();
    match bridge {
        BungeeBridge::Across => match selector.as_str() {
            "0xcc54d224" | "0xa3b8bfba" => Some(BungeeTxDataBytesIndex {
                bytes_start_index: 8,
                bytes_length: 32,
                bytes_string_start_index: 2 + 8 * 2,
                bytes_string_length: 32 * 2,
            }),
            _ => None,
        },
        BungeeBridge::CircleCctp => match selector.as_str() {
            "0xb7dfe9d0" => Some(BungeeTxDataBytesIndex {
                bytes_start_index: 8,
                bytes_length: 32,
                bytes_string_start_index: 2 + 8 * 2,
                bytes_string_length: 32 * 2,
            }),
            _ => None,
        },
        BungeeBridge::GnosisNative => match selector.as_str() {
            "0x3bf5c228" => Some(BungeeTxDataBytesIndex {
                bytes_start_index: 136,
                bytes_length: 32,
                bytes_string_start_index: 2 + 8 * 2,
                bytes_string_length: 32 * 2,
            }),
            "0xfcb23eb0" => Some(BungeeTxDataBytesIndex {
                bytes_start_index: 104,
                bytes_length: 32,
                bytes_string_start_index: 2 + 8 * 2,
                bytes_string_length: 32 * 2,
            }),
            _ => None,
        },
    }
}

// ── Tx data decoding ──────────────────────────────────────────────────────────

/// Decode the route ID and function data from Bungee `SocketGateway` calldata.
///
/// The calldata format is: `0x` + routeId (4 bytes) + functionSelector (4 bytes) + params.
///
/// # Errors
///
/// Returns [`BridgeError::TxBuildError`] if the data is too short or malformed.
pub fn decode_bungee_bridge_tx_data(tx_data: &str) -> Result<DecodedBungeeTxData, BridgeError> {
    if tx_data.len() < 10 {
        return Err(BridgeError::TxBuildError("txData too short".to_owned()));
    }
    if !tx_data.starts_with("0x") {
        return Err(BridgeError::TxBuildError("txData must start with 0x".to_owned()));
    }

    let without_prefix = &tx_data[2..];
    if without_prefix.len() < 8 {
        return Err(BridgeError::TxBuildError("insufficient data for routeId".to_owned()));
    }

    let route_id = format!("0x{}", &without_prefix[..8]);
    let encoded_function_data = format!("0x{}", &without_prefix[8..]);

    if encoded_function_data.len() < 10 {
        return Err(BridgeError::TxBuildError("insufficient data for function selector".to_owned()));
    }

    let function_selector = encoded_function_data[..10].to_owned();

    Ok(DecodedBungeeTxData { route_id, encoded_function_data, function_selector })
}

/// Decode the input amount from Bungee transaction data for a specific bridge.
///
/// # Errors
///
/// Returns [`BridgeError::TxBuildError`] if the bridge type or function selector
/// is unsupported, or the data is malformed.
pub fn decode_amounts_bungee_tx_data(
    tx_data: &str,
    bridge: BungeeBridge,
) -> Result<DecodedBungeeAmounts, BridgeError> {
    if tx_data.is_empty() || !tx_data.starts_with("0x") {
        return Err(BridgeError::TxBuildError("invalid txData format".to_owned()));
    }

    let decoded = decode_bungee_bridge_tx_data(tx_data)?;
    let indices =
        bungee_tx_data_bytes_index(bridge, &decoded.function_selector).ok_or_else(|| {
            BridgeError::TxBuildError(format!(
                "unsupported bridge type {:?} with selector {}",
                bridge, decoded.function_selector
            ))
        })?;

    let start = indices.bytes_string_start_index;
    let len = indices.bytes_string_length;
    if tx_data.len() < start + len {
        return Err(BridgeError::TxBuildError("txData too short for amount field".to_owned()));
    }

    let input_amount_hex = format!("0x{}", &tx_data[start..start + len]);
    let input_amount = U256::from_str_radix(&input_amount_hex[2..], 16)
        .map_err(|e| BridgeError::TxBuildError(format!("cannot parse amount: {e}")))?;

    Ok(DecodedBungeeAmounts { input_amount_bytes: input_amount_hex, input_amount })
}

// ── Display name mapping ──────────────────────────────────────────────────────

/// Get the [`BungeeBridge`] enum from a display name string.
///
/// # Example
///
/// ```
/// use cow_bridging::{bungee::get_bungee_bridge_from_display_name, types::BungeeBridge};
///
/// assert_eq!(get_bungee_bridge_from_display_name("Across"), Some(BungeeBridge::Across));
/// assert_eq!(get_bungee_bridge_from_display_name("Unknown"), None);
/// ```
#[must_use]
pub fn get_bungee_bridge_from_display_name(display_name: &str) -> Option<BungeeBridge> {
    BungeeBridge::from_display_name(display_name)
}

/// Get the display name string for a [`BungeeBridge`] variant.
///
/// # Arguments
///
/// * `bridge` — The [`BungeeBridge`] variant to convert.
///
/// # Returns
///
/// A static string slice containing the human-readable bridge name
/// (e.g. `"Across"`, `"Circle CCTP"`, `"Gnosis Native"`).
///
/// # Example
///
/// ```
/// use cow_bridging::{bungee::get_display_name_from_bungee_bridge, types::BungeeBridge};
///
/// assert_eq!(get_display_name_from_bungee_bridge(BungeeBridge::Across), "Across");
/// ```
#[must_use]
pub const fn get_display_name_from_bungee_bridge(bridge: BungeeBridge) -> &'static str {
    bridge.display_name()
}

// ── Quote result conversion ───────────────────────────────────────────────────

/// Convert a Bungee quote into a [`BridgeQuoteResult`].
///
/// Computes amounts-and-costs from the raw Bungee API data.
///
/// # Arguments
///
/// * `request` — The original bridge request
/// * `slippage_bps` — Slippage tolerance in basis points
/// * `buy_amount` — Output amount from the Bungee route
/// * `route_fee_amount` — Fee amount from the route details
/// * `quote_timestamp` — Timestamp from the Bungee quote
/// * `estimated_time` — Estimated fill time in seconds
/// * `quote_id` — Bungee quote ID
/// * `quote_body` — Serialized quote body for caching
///
/// # Errors
///
/// Returns [`BridgeError::QuoteError`] if fee calculation fails.
#[allow(clippy::too_many_arguments, reason = "mirrors the multi-field Bungee API response")]
pub fn bungee_to_bridge_quote_result(
    request: &QuoteBridgeRequest,
    slippage_bps: u32,
    buy_amount: U256,
    route_fee_amount: U256,
    quote_timestamp: u64,
    estimated_time: u64,
    quote_id: Option<String>,
    quote_body: Option<String>,
) -> Result<BridgeQuoteResult, BridgeError> {
    let sell_amount_before_fee = request.sell_amount;
    let buy_amount_before_fee = buy_amount;
    // Route fee is taken on the source chain, so buy amount is unchanged.
    let buy_amount_after_fee = buy_amount_before_fee;

    // Fee in buy token based on price ratio.
    let fee_buy_token = if sell_amount_before_fee.is_zero() {
        U256::ZERO
    } else {
        (route_fee_amount * buy_amount_before_fee) / sell_amount_before_fee
    };

    let buy_amount_after_slippage = apply_bps(buy_amount_after_fee, slippage_bps);

    let bridge_fee_bps = calculate_fee_bps(route_fee_amount, request.sell_amount).map_or(0, |v| v);

    let amounts_and_costs = BridgeQuoteAmountsAndCosts {
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
                fee_bps: bridge_fee_bps,
                amount_in_sell_currency: route_fee_amount,
                amount_in_buy_currency: fee_buy_token,
            },
        },
        slippage_bps,
    };

    Ok(BridgeQuoteResult {
        id: quote_id,
        signature: None,
        attestation_signature: None,
        quote_body,
        is_sell: request.kind == cow_types::OrderKind::Sell,
        amounts_and_costs,
        expected_fill_time_seconds: Some(estimated_time),
        quote_timestamp,
        fees: BridgeFees { bridge_fee: route_fee_amount, destination_gas_fee: U256::ZERO },
        limits: BridgeLimits { min_deposit: U256::ZERO, max_deposit: U256::ZERO },
    })
}

// ── Status from events ────────────────────────────────────────────────────────

/// Determine the bridge status from Bungee events and an Across status callback.
///
/// Follows the logic:
/// 1. No events → `Unknown`
/// 2. Source pending → `InProgress`
/// 3. Source complete + dest pending → check Across API for expiry/refund, else `InProgress`
/// 4. Both complete → `Executed`
///
/// # Errors
///
/// Returns [`BridgeError::QuoteError`] if the event combination is unrecognized.
pub async fn get_bridging_status_from_events<F, Fut>(
    events: Option<&[BungeeEvent]>,
    get_across_status: F,
) -> Result<BridgeStatusResult, BridgeError>
where
    F: Fn(&str) -> Fut,
    Fut: std::future::Future<Output = Result<String, BridgeError>>,
{
    let active_events = match events {
        Some(e) if !e.is_empty() => e,
        _ => return Ok(BridgeStatusResult::new(BridgeStatus::Unknown)),
    };

    let event = &active_events[0];

    // Source still pending.
    if event.src_tx_status == BungeeEventStatus::Pending {
        return Ok(BridgeStatusResult::new(BridgeStatus::InProgress));
    }

    // Source complete, destination pending.
    if event.src_tx_status == BungeeEventStatus::Completed &&
        event.dest_tx_status == BungeeEventStatus::Pending
    {
        if event.bridge_name == BungeeBridgeName::Across &&
            let Ok(across_status) = get_across_status(&event.order_id).await
        {
            match across_status.as_str() {
                "expired" => {
                    return Ok(BridgeStatusResult {
                        status: BridgeStatus::Expired,
                        deposit_tx_hash: event.src_transaction_hash.clone(),
                        ..BridgeStatusResult::new(BridgeStatus::Expired)
                    });
                }
                "refunded" => {
                    return Ok(BridgeStatusResult {
                        status: BridgeStatus::Refund,
                        deposit_tx_hash: event.src_transaction_hash.clone(),
                        ..BridgeStatusResult::new(BridgeStatus::Refund)
                    });
                }
                _ => {}
            }
        }

        return Ok(BridgeStatusResult {
            status: BridgeStatus::InProgress,
            deposit_tx_hash: event.src_transaction_hash.clone(),
            ..BridgeStatusResult::new(BridgeStatus::InProgress)
        });
    }

    // Both complete.
    if event.src_tx_status == BungeeEventStatus::Completed &&
        event.dest_tx_status == BungeeEventStatus::Completed
    {
        return Ok(BridgeStatusResult {
            status: BridgeStatus::Executed,
            deposit_tx_hash: event.src_transaction_hash.clone(),
            fill_tx_hash: event.dest_transaction_hash.clone(),
            ..BridgeStatusResult::new(BridgeStatus::Executed)
        });
    }

    Err(BridgeError::QuoteError("unknown Bungee event status combination".to_owned()))
}

// ── Response validation ───────────────────────────────────────────────────────

/// Validate a Bungee quote API response.
///
/// Returns `true` if the response has the expected structure with
/// `success`, `statusCode`, `result.manualRoutes[]`.
#[must_use]
pub fn is_valid_quote_response(response: &serde_json::Value) -> bool {
    let Some(success) = response.get("success").and_then(|v| v.as_bool()) else {
        return false;
    };
    if !success {
        return false;
    }

    let Some(result) = response.get("result") else {
        return false;
    };

    let Some(routes) = result.get("manualRoutes").and_then(|r| r.as_array()) else {
        return false;
    };

    routes.iter().all(|route| {
        route.get("quoteId").is_some() &&
            route.get("output").is_some() &&
            route.get("estimatedTime").is_some() &&
            route
                .get("routeDetails")
                .and_then(|rd| rd.get("routeFee"))
                .and_then(|rf| rf.get("amount"))
                .is_some()
    })
}

/// Validate a Bungee events API response.
///
/// Returns `true` if the response has `success: true` and `result` is an array
/// with the required fields in each event.
#[must_use]
pub fn is_valid_bungee_events_response(response: &serde_json::Value) -> bool {
    let Some(success) = response.get("success").and_then(|v| v.as_bool()) else {
        return false;
    };
    if !success {
        return false;
    }

    let Some(result) = response.get("result").and_then(|r| r.as_array()) else {
        return false;
    };

    result.iter().all(|event| {
        event.get("identifier").is_some() &&
            event.get("bridgeName").is_some() &&
            event.get("fromChainId").is_some() &&
            event.get("orderId").is_some() &&
            event.get("srcTxStatus").is_some() &&
            event.get("destTxStatus").is_some()
    })
}

// ── API URL resolution ────────────────────────────────────────────────────────

/// Bungee API URL options.
#[derive(Debug, Clone)]
pub struct BungeeApiUrlOptions {
    /// Base URL for Bungee API.
    pub api_base_url: String,
    /// Base URL for manual API.
    pub manual_api_base_url: String,
    /// Base URL for events API.
    pub events_api_base_url: String,
    /// Base URL for Across API.
    pub across_api_base_url: String,
}

impl Default for BungeeApiUrlOptions {
    fn default() -> Self {
        Self {
            api_base_url: super::sdk::BUNGEE_API_URL.to_owned(),
            manual_api_base_url: super::sdk::BUNGEE_MANUAL_API_URL.to_owned(),
            events_api_base_url: super::sdk::BUNGEE_EVENTS_API_URL.to_owned(),
            across_api_base_url: super::sdk::ACROSS_API_URL.to_owned(),
        }
    }
}

/// Resolve an API endpoint URL, with fallback logic.
///
/// When `use_fallback` is `true`, always returns the default URL.
/// Otherwise, uses the custom URL if provided, then the options value,
/// and finally the default.
#[must_use]
pub fn resolve_api_endpoint_from_options(
    key: &str,
    options: &BungeeApiUrlOptions,
    use_fallback: bool,
    custom_url: Option<&str>,
) -> String {
    let defaults = BungeeApiUrlOptions::default();
    let default_val = match key {
        "manual_api_base_url" => &defaults.manual_api_base_url,
        "events_api_base_url" => &defaults.events_api_base_url,
        "across_api_base_url" => &defaults.across_api_base_url,
        // "api_base_url" and any unrecognized key fall back to the base URL.
        _ => &defaults.api_base_url,
    };

    if use_fallback {
        return default_val.clone();
    }

    if let Some(url) = custom_url {
        return url.to_owned();
    }

    let opt_val = match key {
        "manual_api_base_url" => &options.manual_api_base_url,
        "events_api_base_url" => &options.events_api_base_url,
        "across_api_base_url" => &options.across_api_base_url,
        _ => &options.api_base_url,
    };

    if opt_val.is_empty() { default_val.clone() } else { opt_val.clone() }
}

// ── Deposit call construction ────────────────────────────────────────────────

use alloy_primitives::{hex, keccak256};

/// Parameters for building a Bungee deposit call.
#[derive(Debug, Clone)]
pub struct BungeeDepositCallParams {
    /// The original bridge request.
    pub request: QuoteBridgeRequest,
    /// Raw transaction data from the Bungee `buildTx` API response.
    pub build_tx_data: String,
    /// Input amount from the Bungee quote.
    pub input_amount: U256,
    /// The bridge name (Across, CCTP, etc.) returned by the Bungee quote.
    pub bridge: BungeeBridge,
}

/// Build the unsigned EVM call for a Bungee deposit via `ApproveAndBridge`.
///
/// Decodes the raw transaction data from the Bungee API, appends the
/// `modifyCalldataParams` trailer (input amount offset, no output amount
/// modification), and encodes the final `approveAndBridge` call.
///
/// # Errors
///
/// Returns [`BridgeError::TxBuildError`] if:
/// - The bridge type / function selector has no known byte indices
/// - The `ApproveAndBridge` contract address is not configured for the chain
pub fn create_bungee_deposit_call(
    params: &BungeeDepositCallParams,
) -> Result<cow_chains::EvmCall, BridgeError> {
    let decoded_tx = decode_bungee_bridge_tx_data(&params.build_tx_data)?;
    let function_selector = decoded_tx.function_selector.to_lowercase();

    let function_params = bungee_tx_data_bytes_index(params.bridge, &function_selector)
        .ok_or_else(|| {
            BridgeError::TxBuildError(format!("no params for function [{function_selector}]"))
        })?;

    let input_amount_start_index = function_params.bytes_start_index;

    // Encode modifyCalldataParams: (uint256, bool, uint256)
    let mut modify_params = Vec::with_capacity(3 * 32);
    modify_params.extend_from_slice(&U256::from(input_amount_start_index).to_be_bytes::<32>());
    modify_params.extend_from_slice(&U256::ZERO.to_be_bytes::<32>()); // modifyOutputAmount = false
    modify_params.extend_from_slice(&U256::ZERO.to_be_bytes::<32>()); // outputAmountStartIndex = 0

    // Concatenate original tx data + modifyCalldataParams
    let raw_data =
        params.build_tx_data.strip_prefix("0x").map_or(params.build_tx_data.as_str(), |s| s);
    let modify_hex = hex::encode(&modify_params);
    let full_data_hex = format!("{raw_data}{modify_hex}");
    let full_data_bytes = hex::decode(&full_data_hex)
        .map_err(|e| BridgeError::TxBuildError(format!("hex decode error: {e}")))?;

    // Encode `approveAndBridge(address token, uint256 minAmount, uint256 nativeTokenExtraFee, bytes
    // data)`
    let selector = &keccak256("approveAndBridge(address,uint256,uint256,bytes)")[..4];

    let mut calldata = Vec::with_capacity(4 + 5 * 32 + full_data_bytes.len() + 32);
    calldata.extend_from_slice(selector);

    // token (sell token)
    let mut addr_buf = [0u8; 32];
    addr_buf[12..32].copy_from_slice(params.request.sell_token.as_slice());
    calldata.extend_from_slice(&addr_buf);
    // minAmount
    calldata.extend_from_slice(&params.input_amount.to_be_bytes::<32>());
    // nativeTokenExtraFee
    calldata.extend_from_slice(&U256::ZERO.to_be_bytes::<32>());
    // offset to `data` bytes (4th param, so offset = 4 * 32)
    calldata.extend_from_slice(&U256::from(4u64 * 32).to_be_bytes::<32>());
    // bytes length
    calldata.extend_from_slice(&U256::from(full_data_bytes.len()).to_be_bytes::<32>());
    // bytes data (padded to 32-byte boundary)
    calldata.extend_from_slice(&full_data_bytes);
    let padding = (32 - (full_data_bytes.len() % 32)) % 32;
    calldata.extend(std::iter::repeat_n(0u8, padding));

    // Resolve the ApproveAndBridge contract address.
    let addresses = bungee_approve_and_bridge_v1_addresses();
    let to = addresses.get(&params.request.sell_chain_id).ok_or_else(|| {
        BridgeError::TxBuildError("BungeeApproveAndBridgeV1 not found".to_owned())
    })?;

    // Determine value (native token sends).
    let native = cow_chains::NATIVE_CURRENCY_ADDRESS;
    let value = if params.request.sell_token == native { params.input_amount } else { U256::ZERO };

    Ok(cow_chains::EvmCall { to: *to, data: calldata, value })
}

// ── BungeeProvider (HTTP client) ──────────────────────────────────────────────

/// Bridge provider backed by the Bungee / Socket aggregator API.
///
/// Documentation: `https://docs.socket.tech/socket-liquidity-layer/use-socketll/quote`
#[derive(Debug, Clone)]
pub struct BungeeProvider {
    client: reqwest::Client,
    api_key: String,
    info: BridgeProviderInfo,
    cow_shed: Option<Arc<CowShedSdk>>,
    api_base: String,
    events_api_base: String,
}

impl BungeeProvider {
    /// Construct a new [`BungeeProvider`] with the given API key.
    ///
    /// The provider is built with default endpoints
    /// ([`crate::sdk::BUNGEE_API_URL`], [`crate::sdk::BUNGEE_EVENTS_API_URL`])
    /// and no [`CowShedSdk`] — call [`Self::with_cow_shed`] if you need
    /// [`HookBridgeProvider::get_signed_hook`] to succeed.
    ///
    /// # Arguments
    ///
    /// * `api_key` — A Bungee / Socket API key used to authenticate requests.
    ///
    /// # Returns
    ///
    /// A ready-to-use [`BungeeProvider`] backed by a default `reqwest::Client`.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            info: default_bungee_info(),
            cow_shed: None,
            api_base: crate::sdk::BUNGEE_API_URL.to_owned(),
            events_api_base: crate::sdk::BUNGEE_EVENTS_API_URL.to_owned(),
        }
    }

    /// Attach a shared [`CowShedSdk`] so hook signing works end-to-end.
    ///
    /// Required to make [`HookBridgeProvider::get_signed_hook`] succeed;
    /// without it the method returns [`CowError::Signing`].
    #[must_use]
    pub fn with_cow_shed(mut self, cow_shed: Arc<CowShedSdk>) -> Self {
        self.cow_shed = Some(cow_shed);
        self
    }

    /// Override the quote API base URL (useful for tests pointing at a mock server).
    #[must_use]
    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self
    }

    /// Override the events API base URL (useful for tests pointing at a mock server).
    #[must_use]
    pub fn with_events_api_base(mut self, base: impl Into<String>) -> Self {
        self.events_api_base = base.into();
        self
    }
}

/// Chains supported by the Bungee integration.
///
/// Mirrors the `BUNGEE_SUPPORTED_NETWORKS` constant from the `TypeScript` SDK.
#[must_use]
pub fn bungee_supported_chains() -> Vec<u64> {
    vec![
        SupportedChainId::Mainnet.as_u64(),
        SupportedChainId::Polygon.as_u64(),
        SupportedChainId::ArbitrumOne.as_u64(),
        SupportedChainId::Base.as_u64(),
        SupportedChainId::Avalanche.as_u64(),
        SupportedChainId::GnosisChain.as_u64(),
        10, // Optimism
    ]
}

fn bungee_chain_name(chain_id: u64) -> String {
    SupportedChainId::try_from_u64(chain_id)
        .map_or_else(|| format!("Chain {chain_id}"), |c| format!("{c}"))
}

/// Minimal catalog of popular ERC-20 tokens per supported chain — used as
/// the seed for [`BridgeProvider::get_buy_tokens`] and
/// [`BridgeProvider::get_intermediate_tokens`]. Mirrors the
/// `ACROSS_TOKEN_MAPPING`-style constants on the TS side; a future PR can
/// wire this to a live `/tokens` endpoint.
fn bungee_popular_tokens(chain_id: u64) -> Vec<IntermediateTokenInfo> {
    let mainnet = SupportedChainId::Mainnet.as_u64();
    let polygon = SupportedChainId::Polygon.as_u64();
    let arbitrum = SupportedChainId::ArbitrumOne.as_u64();
    let base = SupportedChainId::Base.as_u64();
    let gnosis = SupportedChainId::GnosisChain.as_u64();

    let make = |symbol: &str, name: &str, addr: &str, decimals: u8| IntermediateTokenInfo {
        chain_id,
        address: addr.parse().map_or(Address::ZERO, |a| a),
        decimals,
        symbol: symbol.into(),
        name: name.into(),
        logo_url: None,
    };

    if chain_id == mainnet {
        vec![
            make("USDC", "USD Coin", "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", 6),
            make("USDT", "Tether USD", "0xdAC17F958D2ee523a2206206994597C13D831ec7", 6),
            make("DAI", "Dai Stablecoin", "0x6B175474E89094C44Da98b954EedeAC495271d0F", 18),
            make("WETH", "Wrapped Ether", "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", 18),
            make("WBTC", "Wrapped BTC", "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599", 8),
        ]
    } else if chain_id == polygon {
        vec![
            make("USDC", "USD Coin", "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359", 6),
            make("USDT", "Tether USD", "0xc2132D05D31c914a87C6611C10748AEb04B58e8F", 6),
            make("DAI", "Dai Stablecoin", "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063", 18),
            make("WETH", "Wrapped Ether", "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619", 18),
        ]
    } else if chain_id == arbitrum {
        vec![
            make("USDC", "USD Coin", "0xaf88d065e77c8cC2239327C5EDb3A432268e5831", 6),
            make("USDT", "Tether USD", "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9", 6),
            make("WETH", "Wrapped Ether", "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1", 18),
        ]
    } else if chain_id == base {
        vec![
            make("USDC", "USD Coin", "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 6),
            make("WETH", "Wrapped Ether", "0x4200000000000000000000000000000000000006", 18),
        ]
    } else if chain_id == 10 {
        vec![
            make("USDC", "USD Coin", "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85", 6),
            make("WETH", "Wrapped Ether", "0x4200000000000000000000000000000000000006", 18),
        ]
    } else if chain_id == SupportedChainId::Avalanche.as_u64() {
        vec![make("USDC", "USD Coin", "0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E", 6)]
    } else if chain_id == gnosis {
        vec![make("USDC", "USD Coin", "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83", 6)]
    } else {
        Vec::new()
    }
}

/// Default [`BridgeProviderInfo`] for [`BungeeProvider`].
///
/// Mirrors the constants exposed by the `TypeScript` SDK
/// (`BUNGEE_HOOK_DAPP_ID`, the Bungee logo, etc.).
#[must_use]
pub fn default_bungee_info() -> BridgeProviderInfo {
    BridgeProviderInfo {
        name: "bungee".to_owned(),
        logo_url: "https://files.cow.fi/cow-sdk/bridging/providers/bungee-logo.svg".to_owned(),
        dapp_id: crate::sdk::BUNGEE_HOOK_DAPP_ID.to_owned(),
        website: "https://bungee.exchange".to_owned(),
        provider_type: BridgeProviderType::HookBridgeProvider,
    }
}

impl BridgeProvider for BungeeProvider {
    /// Metadata about the Bungee provider.
    fn info(&self) -> &BridgeProviderInfo {
        &self.info
    }

    /// Check whether a cross-chain route is supported.
    ///
    /// Returns `true` only when both chains are in
    /// [`bungee_supported_chains`] and distinct. The Bungee aggregator
    /// can still route between any pair it indexes, but gating at the
    /// trait level avoids wasting a round-trip for obviously unsupported
    /// pairs.
    fn supports_route(&self, sell_chain: u64, buy_chain: u64) -> bool {
        if sell_chain == buy_chain {
            return false;
        }
        let supported = bungee_supported_chains();
        supported.contains(&sell_chain) && supported.contains(&buy_chain)
    }

    /// List the networks supported by Bungee.
    fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
        Box::pin(async move {
            Ok(bungee_supported_chains()
                .into_iter()
                .map(|chain_id| BridgeNetworkInfo {
                    chain_id,
                    name: bungee_chain_name(chain_id),
                    logo_url: None,
                })
                .collect())
        })
    }

    /// List buyable tokens on a destination chain.
    ///
    /// Returns a curated set of popular ERC-20 tokens for the target
    /// chain. When Bungee adds a public `/tokens` endpoint we'll wire
    /// it here and fall back to this static list on errors.
    fn get_buy_tokens<'a>(&'a self, params: BuyTokensParams) -> BuyTokensFuture<'a> {
        let info = self.info.clone();
        Box::pin(async move {
            let tokens = bungee_popular_tokens(params.buy_chain_id);
            Ok(GetProviderBuyTokens { provider_info: info, tokens })
        })
    }

    /// List candidate intermediate tokens for a bridging request.
    ///
    /// Returns the source-chain tokens whose symbol is also present on
    /// the destination chain, with the sell-token match (if any) first
    /// — same heuristic as the Across provider.
    fn get_intermediate_tokens<'a>(
        &'a self,
        request: &'a QuoteBridgeRequest,
    ) -> IntermediateTokensFuture<'a> {
        let source_chain = request.sell_chain_id;
        let target_chain = request.buy_chain_id;
        let sell_token = request.sell_token;
        Box::pin(async move {
            let target_symbols: foldhash::HashSet<String> = bungee_popular_tokens(target_chain)
                .into_iter()
                .map(|t| t.symbol.to_ascii_uppercase())
                .collect();
            let mut candidates: Vec<IntermediateTokenInfo> = bungee_popular_tokens(source_chain)
                .into_iter()
                .filter(|t| target_symbols.contains(&t.symbol.to_ascii_uppercase()))
                .collect();
            candidates.sort_by_key(|t| if t.address == sell_token { 0 } else { 1 });
            Ok(candidates)
        })
    }

    /// Fetch a bridge quote from the Bungee / Socket aggregator API.
    ///
    /// Delegates to `BungeeProvider::get_quote_inner` and returns the result
    /// as a pinned, boxed future suitable for the [`BridgeProvider`] trait.
    fn get_quote<'a>(&'a self, req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
        Box::pin(self.get_quote_inner(req))
    }

    /// Reconstruct bridging deposit parameters from a settlement transaction.
    ///
    /// Returns `Ok(None)` for now. Threading a real receipt through here
    /// (so we can call [`get_bridging_status_from_events`]) is a PR #8
    /// deliverable that lands alongside the orchestration rewrite.
    fn get_bridging_params<'a>(
        &'a self,
        _chain_id: u64,
        _order: &'a Order,
        _tx_hash: B256,
        _settlement_override: Option<Address>,
    ) -> BridgingParamsFuture<'a> {
        Box::pin(async { Ok(None) })
    }

    /// Return the provider's explorer URL for a bridging ID.
    fn get_explorer_url(&self, bridging_id: &str) -> String {
        format!("https://bungee.exchange/tx/{bridging_id}")
    }

    /// Fetch the current bridge status for a bridging ID.
    ///
    /// Hits the Bungee events microservice (`GET /api/v1/status?srcTxHash=…`)
    /// and maps the response through [`get_bridging_status_from_events`].
    fn get_status<'a>(
        &'a self,
        bridging_id: &'a str,
        _origin_chain_id: u64,
    ) -> BridgeStatusFuture<'a> {
        Box::pin(self.get_status_inner(bridging_id))
    }
}

impl HookBridgeProvider for BungeeProvider {
    /// Build the unsigned EVM call that initiates the Bungee bridge.
    ///
    /// Returns a clear error until PR #7 threads the Bungee `/build-tx`
    /// response into the quote so this method can delegate to
    /// [`create_bungee_deposit_call`]. The problem is that
    /// `create_bungee_deposit_call` needs the raw `build_tx_data` blob
    /// which is *not* included in the upstream quote call — it's a
    /// separate API round-trip handled by the orchestration layer.
    fn get_unsigned_bridge_call<'a>(
        &'a self,
        _request: &'a QuoteBridgeRequest,
        _quote: &'a QuoteBridgeResponse,
    ) -> UnsignedCallFuture<'a> {
        Box::pin(async {
            Err(CowError::Api {
                status: 0,
                body: "BungeeProvider::get_unsigned_bridge_call needs a build-tx response; \
                       will be wired by PR #7 orchestration"
                    .into(),
            })
        })
    }

    /// Sign a post-hook via `CowShedSdk::sign_hook`.
    ///
    /// Fails with [`CowError::Signing`] if the provider was constructed
    /// without a [`CowShedSdk`] via [`BungeeProvider::with_cow_shed`].
    fn get_signed_hook<'a>(
        &'a self,
        _chain_id: SupportedChainId,
        unsigned_call: &'a EvmCall,
        bridge_hook_nonce: &'a str,
        deadline: u64,
        hook_gas_limit: u64,
        signer: &'a PrivateKeySigner,
    ) -> SignedHookFuture<'a> {
        let cow_shed_opt = self.cow_shed.clone();
        Box::pin(async move {
            use crate::types::BridgeHook as BridgeHookType;
            let cow_shed = cow_shed_opt.ok_or_else(|| {
                CowError::Signing(
                    "BungeeProvider built without CowShedSdk — call with_cow_shed(...)".to_owned(),
                )
            })?;
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
            let proxy = alloy_signer::Signer::address(signer);
            let signed = cow_shed.sign_hook(proxy, &params, signer).await?;
            let _ = signed; // PR #8 will bundle the signature into the hook calldata.

            let post_hook = cow_types::CowHook {
                target: format!("{proxy:#x}"),
                call_data: format!("0x{}", alloy_primitives::hex::encode(&unsigned_call.data)),
                gas_limit: hook_gas_limit.to_string(),
                dapp_id: Some(crate::sdk::BUNGEE_HOOK_DAPP_ID.to_owned()),
            };
            Ok(BridgeHookType { post_hook, recipient: format!("{:#x}", unsigned_call.to) })
        })
    }
}

impl BungeeProvider {
    /// Perform the actual HTTP request to the Bungee quote API and parse the
    /// best route from the response.
    ///
    /// # Arguments
    ///
    /// * `req` — The [`QuoteBridgeRequest`] containing chain IDs, token addresses, amounts, and
    ///   slippage tolerance.
    ///
    /// # Returns
    ///
    /// A [`QuoteBridgeResponse`] with the output amount, estimated time, and
    /// fee data from the best available Bungee route.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Parse`] if the Bungee URL cannot be constructed or
    /// the response JSON is missing expected fields.
    /// Returns [`CowError::Api`] if the HTTP response status is non-2xx.
    /// Returns [`CowError`] network variants on transport failures.
    async fn get_quote_inner(
        &self,
        req: &QuoteBridgeRequest,
    ) -> Result<QuoteBridgeResponse, CowError> {
        // slippage_bps → percentage string (50 bps = "0.5")
        let slippage_pct = req.slippage_bps as f64 / 100.0;
        let slippage_str = format!("{slippage_pct:.1}");

        let url = reqwest::Url::parse_with_params(
            &format!("{}/quote", self.api_base),
            &[
                ("fromChainId", req.sell_chain_id.to_string()),
                ("toChainId", req.buy_chain_id.to_string()),
                ("fromTokenAddress", format!("{:#x}", req.sell_token)),
                ("toTokenAddress", format!("{:#x}", req.buy_token)),
                ("fromAmount", req.sell_amount.to_string()),
                ("userAddress", format!("{:#x}", req.account)),
                ("slippageTolerance", slippage_str),
                ("isContractCall", "false".to_owned()),
            ],
        )
        .map_err(|e| CowError::Parse { field: "bungee_url", reason: e.to_string() })?;

        let resp = self.client.get(url).header("API-KEY", &self.api_key).send().await?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.map_or(String::new(), |b| b);
            return Err(CowError::Api { status, body });
        }

        let json: serde_json::Value = resp.json().await?;

        // Expect { "success": true, "result": { "routes": [...] } }
        let route = json
            .get("result")
            .and_then(|r| r.get("routes"))
            .and_then(|r| r.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| CowError::Parse {
                field: "bungee_routes",
                reason: "no routes in response".to_owned(),
            })?;

        let output_amount_str =
            route.get("outputAmount").and_then(|v| v.as_str()).map_or("0", |s| s);
        let buy_amount = output_amount_str
            .parse::<U256>()
            .map_err(|e| CowError::Parse { field: "outputAmount", reason: e.to_string() })?;

        let estimated_secs =
            route.get("estimatedTimeInSeconds").and_then(|v| v.as_u64()).map_or(0, |v| v);

        Ok(QuoteBridgeResponse {
            provider: "bungee".to_owned(),
            sell_amount: req.sell_amount,
            buy_amount,
            fee_amount: U256::ZERO,
            estimated_secs,
            bridge_hook: None,
        })
    }

    /// Inner `get_status` used by the trait impl and tests.
    ///
    /// Queries the Bungee events microservice
    /// (`GET <events_api_base>/api/v1/status?srcTxHash=…`) and returns a
    /// [`BridgeStatusResult`]. The response shape is the `data.status`
    /// string documented at
    /// `docs.socket.tech/socket-liquidity-layer/transaction-status-api`.
    async fn get_status_inner(&self, bridging_id: &str) -> Result<BridgeStatusResult, CowError> {
        let url = reqwest::Url::parse_with_params(
            &format!("{}/api/v1/status", self.events_api_base),
            &[("srcTxHash", bridging_id.to_owned())],
        )
        .map_err(|e| CowError::Parse { field: "bungee_status_url", reason: e.to_string() })?;

        let resp = self.client.get(url).header("API-KEY", &self.api_key).send().await?;
        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(CowError::Api { status, body });
        }

        let json: serde_json::Value = resp.json().await?;
        // Shape: { "success": bool, "result": { "sourceTxStatus": "COMPLETED" | ...,
        // "destinationTxStatus": "...", "srcTransactionHash": "0x…", "destinationTransactionHash":
        // "0x…" } }
        let result = json.get("result").ok_or_else(|| CowError::Parse {
            field: "bungee_status",
            reason: "missing result field".to_owned(),
        })?;

        let src_tx_status = result
            .get("sourceTxStatus")
            .and_then(|v| v.as_str())
            .map_or_else(String::new, |s| s.to_ascii_uppercase());
        let dst_tx_status = result
            .get("destinationTxStatus")
            .and_then(|v| v.as_str())
            .map_or_else(String::new, |s| s.to_ascii_uppercase());

        let mapped = match (src_tx_status.as_str(), dst_tx_status.as_str()) {
            (_, "COMPLETED") => BridgeStatus::Executed,
            (_, "PENDING") | ("PENDING", _) | ("COMPLETED", "") => BridgeStatus::InProgress,
            (_, "FAILED") | ("FAILED", _) => BridgeStatus::Refund,
            _ => BridgeStatus::Unknown,
        };

        Ok(BridgeStatusResult {
            status: mapped,
            fill_time_in_seconds: None,
            deposit_tx_hash: result
                .get("srcTransactionHash")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
            fill_tx_hash: result
                .get("destinationTransactionHash")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
        })
    }
}

#[cfg(test)]
#[allow(
    clippy::tests_outside_test_module,
    reason = "inner module pattern — enforced cfg guard keeps tests compile-gated"
)]
mod bungee_provider_trait_tests {
    use super::*;

    fn test_provider() -> BungeeProvider {
        BungeeProvider::new("test-key")
    }

    fn sample_request() -> QuoteBridgeRequest {
        QuoteBridgeRequest {
            sell_chain_id: 1,
            buy_chain_id: 10,
            sell_token: Address::ZERO,
            sell_token_decimals: 18,
            buy_token: Address::ZERO,
            buy_token_decimals: 18,
            sell_amount: U256::from(100u64),
            account: Address::ZERO,
            owner: None,
            receiver: None,
            bridge_recipient: None,
            slippage_bps: 50,
            bridge_slippage_bps: None,
            kind: cow_types::OrderKind::Sell,
        }
    }

    #[test]
    fn info_exposes_default_metadata() {
        let provider = test_provider();
        let info = provider.info();
        assert_eq!(info.name, "bungee");
        assert_eq!(info.dapp_id, crate::sdk::BUNGEE_HOOK_DAPP_ID);
        assert!(info.is_hook_bridge_provider());
    }

    #[test]
    fn name_defaults_to_info_name() {
        assert_eq!(test_provider().name(), "bungee");
    }

    fn hook_request() -> QuoteBridgeRequest {
        let mut r = sample_request();
        r.sell_chain_id = SupportedChainId::Mainnet.as_u64();
        r.buy_chain_id = SupportedChainId::ArbitrumOne.as_u64();
        r.sell_token = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
        r
    }

    #[test]
    fn supports_route_requires_both_supported_and_distinct() {
        let p = test_provider();
        assert!(p.supports_route(
            SupportedChainId::Mainnet.as_u64(),
            SupportedChainId::ArbitrumOne.as_u64(),
        ));
        assert!(p.supports_route(SupportedChainId::Base.as_u64(), 10));
        assert!(!p.supports_route(1, 1));
        assert!(!p.supports_route(1, 9999));
    }

    #[test]
    fn explorer_url_uses_bungee_domain() {
        let url = test_provider().get_explorer_url("abc");
        assert!(url.starts_with("https://bungee.exchange/tx/"));
        assert!(url.ends_with("/abc"));
    }

    #[test]
    fn bungee_supported_chains_list_is_non_empty() {
        let supported = bungee_supported_chains();
        assert!(supported.contains(&SupportedChainId::Mainnet.as_u64()));
        assert!(supported.contains(&10)); // Optimism
        assert!(supported.contains(&SupportedChainId::GnosisChain.as_u64()));
    }

    #[tokio::test]
    async fn get_networks_returns_all_supported_chains() {
        let p = test_provider();
        let networks = p.get_networks().await.unwrap();
        assert_eq!(networks.len(), bungee_supported_chains().len());
        assert!(networks.iter().any(|n| n.chain_id == SupportedChainId::Mainnet.as_u64()));
    }

    #[tokio::test]
    async fn get_buy_tokens_returns_mainnet_stablecoins() {
        let p = test_provider();
        let tokens = p
            .get_buy_tokens(BuyTokensParams {
                sell_chain_id: 1,
                buy_chain_id: 1,
                sell_token_address: None,
            })
            .await
            .unwrap();
        assert!(!tokens.tokens.is_empty());
        assert!(tokens.tokens.iter().any(|t| t.symbol == "USDC"));
        assert_eq!(tokens.provider_info.name, "bungee");
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
        let req = hook_request();
        let tokens = p.get_intermediate_tokens(&req).await.unwrap();
        assert!(!tokens.is_empty());
        // Everything returned must share symbol with the target set.
        let target_symbols: foldhash::HashSet<String> =
            bungee_popular_tokens(req.buy_chain_id).into_iter().map(|t| t.symbol).collect();
        for t in &tokens {
            assert!(target_symbols.contains(&t.symbol), "target missing symbol {}", t.symbol);
        }
    }

    #[tokio::test]
    async fn get_intermediate_tokens_empty_for_unsupported_target() {
        let p = test_provider();
        let mut req = hook_request();
        req.buy_chain_id = 9999;
        let tokens = p.get_intermediate_tokens(&req).await.unwrap();
        assert!(tokens.is_empty());
    }

    #[tokio::test]
    async fn get_bridging_params_returns_none_until_pr8() {
        let p = test_provider();
        let order = cow_orderbook::api::mock_get_order(&format!("0x{}", "aa".repeat(56)));
        assert!(p.get_bridging_params(1, &order, B256::ZERO, None).await.unwrap().is_none());
    }

    #[test]
    fn default_bungee_info_matches_provider_info() {
        let provider = test_provider();
        let default = default_bungee_info();
        assert_eq!(provider.info().name, default.name);
        assert_eq!(provider.info().dapp_id, default.dapp_id);
        assert_eq!(provider.info().provider_type, default.provider_type);
    }

    // ── HookBridgeProvider ──────────────────────────────────────────────

    #[tokio::test]
    async fn get_unsigned_bridge_call_errors_until_pr7_buildtx() {
        let provider = test_provider();
        let req = hook_request();
        let quote = QuoteBridgeResponse {
            provider: "bungee".into(),
            sell_amount: U256::from(100u64),
            buy_amount: U256::from(99u64),
            fee_amount: U256::ZERO,
            estimated_secs: 0,
            bridge_hook: None,
        };
        let err = provider.get_unsigned_bridge_call(&req, &quote).await.unwrap_err();
        assert!(matches!(err, CowError::Api { status: 0, ref body } if body.contains("build-tx")));
    }

    #[tokio::test]
    async fn get_signed_hook_without_cow_shed_returns_signing_error() {
        let provider = test_provider();
        let signer: PrivateKeySigner =
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".parse().unwrap();
        let call = cow_chains::EvmCall { to: Address::ZERO, data: vec![], value: U256::ZERO };
        let err = provider
            .get_signed_hook(SupportedChainId::Mainnet, &call, "nonce", 0, 0, &signer)
            .await
            .unwrap_err();
        assert!(matches!(err, CowError::Signing(ref msg) if msg.contains("with_cow_shed")));
    }

    #[tokio::test]
    async fn get_signed_hook_with_cow_shed_produces_hook() {
        let provider = BungeeProvider::new("test").with_cow_shed(Arc::new(CowShedSdk::new(1)));
        let signer: PrivateKeySigner =
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".parse().unwrap();
        let call = cow_chains::EvmCall {
            to: "0xD06a673fe1fa27B1b9E5BA0be980AB15Dbce85cc".parse().unwrap(),
            data: vec![0xde, 0xad, 0xbe, 0xef],
            value: U256::ZERO,
        };
        let hook = provider
            .get_signed_hook(
                SupportedChainId::Mainnet,
                &call,
                "nonce-1",
                9_999_999,
                400_000,
                &signer,
            )
            .await
            .unwrap();
        assert_eq!(hook.post_hook.dapp_id.as_deref(), Some(crate::sdk::BUNGEE_HOOK_DAPP_ID));
        assert_eq!(hook.post_hook.gas_limit, "400000");
    }

    #[tokio::test]
    async fn default_gas_limit_estimation_matches_helper() {
        let provider = test_provider();
        let gas = provider.get_gas_limit_estimation_for_hook(true, Some(1000), None).await.unwrap();
        assert_eq!(gas, crate::utils::get_gas_limit_estimation_for_hook(true, Some(1000), None));
    }

    // ── Wiremock HTTP tests ─────────────────────────────────────────────

    fn mock_quote_body() -> serde_json::Value {
        serde_json::json!({
            "success": true,
            "result": {
                "routes": [
                    {
                        "outputAmount": "990000",
                        "estimatedTimeInSeconds": 120
                    }
                ]
            }
        })
    }

    fn mock_status_body(src: &str, dst: &str) -> serde_json::Value {
        serde_json::json!({
            "success": true,
            "result": {
                "sourceTxStatus": src,
                "destinationTxStatus": dst,
                "srcTransactionHash": "0xabc",
                "destinationTransactionHash": "0xdef"
            }
        })
    }

    #[tokio::test]
    async fn get_quote_uses_configured_api_base() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/quote"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_quote_body()))
            .mount(&server)
            .await;

        let p = BungeeProvider::new("test").with_api_base(server.uri());
        let quote = p.get_quote(&hook_request()).await.unwrap();
        assert_eq!(quote.provider, "bungee");
        assert_eq!(quote.estimated_secs, 120);
        assert_eq!(quote.buy_amount, U256::from(990_000u64));
    }

    #[tokio::test]
    async fn get_status_maps_completed_to_executed() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/status"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(mock_status_body("COMPLETED", "COMPLETED")),
            )
            .mount(&server)
            .await;

        let p = BungeeProvider::new("test").with_events_api_base(server.uri());
        let status = p.get_status("0xdeadbeef", 1).await.unwrap();
        assert_eq!(status.status, BridgeStatus::Executed);
        assert_eq!(status.deposit_tx_hash.as_deref(), Some("0xabc"));
        assert_eq!(status.fill_tx_hash.as_deref(), Some("0xdef"));
    }

    #[tokio::test]
    async fn get_status_maps_pending_to_in_progress() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/status"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(mock_status_body("PENDING", "PENDING")),
            )
            .mount(&server)
            .await;

        let p = BungeeProvider::new("test").with_events_api_base(server.uri());
        let status = p.get_status("0xdeadbeef", 1).await.unwrap();
        assert_eq!(status.status, BridgeStatus::InProgress);
    }

    #[tokio::test]
    async fn get_status_maps_failed_to_refund() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/status"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(mock_status_body("COMPLETED", "FAILED")),
            )
            .mount(&server)
            .await;

        let p = BungeeProvider::new("test").with_events_api_base(server.uri());
        let status = p.get_status("0xdeadbeef", 1).await.unwrap();
        assert_eq!(status.status, BridgeStatus::Refund);
    }

    #[tokio::test]
    async fn get_status_propagates_http_error() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/status"))
            .respond_with(ResponseTemplate::new(503).set_body_string("down"))
            .mount(&server)
            .await;

        let p = BungeeProvider::new("test").with_events_api_base(server.uri());
        let err = p.get_status("0xdeadbeef", 1).await.unwrap_err();
        assert!(matches!(err, CowError::Api { status: 503, .. }));
    }
}
