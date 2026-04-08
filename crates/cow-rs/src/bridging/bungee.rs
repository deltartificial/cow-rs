//! Bridge provider backed by the Bungee (Socket) aggregator API.
//!
//! Includes deposit call construction, transaction data decoding,
//! status tracking, and response validation.

use foldhash::HashMap;

use alloy_primitives::{Address, U256};

use crate::CowError;

use super::{
    provider::{BridgeProvider, QuoteFuture},
    types::{
        BridgeAmounts, BridgeCosts, BridgeError, BridgeFees, BridgeLimits,
        BridgeQuoteAmountsAndCosts, BridgeQuoteResult, BridgeStatus, BridgeStatusResult,
        BridgingFee, BungeeBridge, BungeeBridgeName, BungeeEvent, BungeeEventStatus,
        BungeeTxDataBytesIndex, DecodedBungeeAmounts, DecodedBungeeTxData, QuoteBridgeRequest,
        QuoteBridgeResponse,
    },
    utils::{apply_bps, calculate_fee_bps},
};

/// Bungee (Socket) bridge quote API base URL.
const BUNGEE_API_BASE: &str = "https://api.socket.tech/v2/quote";

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
    use crate::config::SupportedChainId;

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
/// use cow_rs::bridging::{bungee::get_bungee_bridge_from_display_name, types::BungeeBridge};
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
/// use cow_rs::bridging::{bungee::get_display_name_from_bungee_bridge, types::BungeeBridge};
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
        is_sell: request.kind == crate::OrderKind::Sell,
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
) -> Result<crate::config::EvmCall, BridgeError> {
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
    let native = crate::config::NATIVE_CURRENCY_ADDRESS;
    let value = if params.request.sell_token == native { params.input_amount } else { U256::ZERO };

    Ok(crate::config::EvmCall { to: *to, data: calldata, value })
}

// ── BungeeProvider (HTTP client) ──────────────────────────────────────────────

/// Bridge provider backed by the Bungee / Socket aggregator API.
///
/// Documentation: `https://docs.socket.tech/socket-liquidity-layer/use-socketll/quote`
#[derive(Debug)]
pub struct BungeeProvider {
    client: reqwest::Client,
    api_key: String,
}

impl BungeeProvider {
    /// Construct a new [`BungeeProvider`] with the given API key.
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
        Self { client: reqwest::Client::new(), api_key: api_key.into() }
    }
}

impl BridgeProvider for BungeeProvider {
    /// Returns the provider identifier string.
    ///
    /// Always returns `"bungee"`, used to tag quotes and logs originating
    /// from this provider.
    fn name(&self) -> &str {
        "bungee"
    }

    /// Check whether a cross-chain route is supported.
    ///
    /// Always returns `true` because the Bungee aggregator can route between
    /// any pair of chains it indexes; unsupported pairs are caught later when
    /// the quote API returns zero routes.
    fn supports_route(&self, _sell_chain: u64, _buy_chain: u64) -> bool {
        true
    }

    /// Fetch a bridge quote from the Bungee / Socket aggregator API.
    ///
    /// Delegates to `BungeeProvider::get_quote_inner` and returns the result
    /// as a pinned, boxed future suitable for the [`BridgeProvider`] trait.
    fn get_quote<'a>(&'a self, req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
        Box::pin(self.get_quote_inner(req))
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
            BUNGEE_API_BASE,
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
}
