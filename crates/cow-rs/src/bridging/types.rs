//! Cross-chain bridging types.

use foldhash::HashMap;

use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};

use crate::app_data::CowHook;

// ── Provider type ─────────────────────────────────────────────────────────────

/// Type of bridge provider — either hook-based or receiver-account-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BridgeProviderType {
    /// Provider relies on a post-hook to initiate the bridge.
    HookBridgeProvider,
    /// Provider sends tokens to a specific deposit account.
    ReceiverAccountBridgeProvider,
}

impl BridgeProviderType {
    /// Returns `true` if this is a [`HookBridgeProvider`](Self::HookBridgeProvider).
    ///
    /// Equivalent to the TypeScript `isHookBridgeProvider` type guard.
    #[must_use]
    pub const fn is_hook_bridge_provider(self) -> bool {
        matches!(self, Self::HookBridgeProvider)
    }

    /// Returns `true` if this is a
    /// [`ReceiverAccountBridgeProvider`](Self::ReceiverAccountBridgeProvider).
    ///
    /// Equivalent to the TypeScript `isReceiverAccountBridgeProvider` type guard.
    #[must_use]
    pub const fn is_receiver_account_bridge_provider(self) -> bool {
        matches!(self, Self::ReceiverAccountBridgeProvider)
    }
}

/// Metadata about a bridge provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeProviderInfo {
    /// Provider display name.
    pub name: String,
    /// URL to the provider's logo.
    pub logo_url: String,
    /// Unique dApp identifier (e.g. `"cow-sdk://bridging/providers/across"`).
    pub dapp_id: String,
    /// Provider website URL.
    pub website: String,
    /// Type of bridge provider.
    pub provider_type: BridgeProviderType,
}

impl BridgeProviderInfo {
    /// Returns `true` if this provider uses hooks to initiate the bridge.
    ///
    /// Delegates to [`BridgeProviderType::is_hook_bridge_provider`] on the
    /// inner `provider_type` field.
    ///
    /// # Returns
    ///
    /// `true` when `provider_type` is [`BridgeProviderType::HookBridgeProvider`],
    /// `false` otherwise.
    #[must_use]
    pub const fn is_hook_bridge_provider(&self) -> bool {
        self.provider_type.is_hook_bridge_provider()
    }

    /// Returns `true` if this provider sends tokens to a deposit account.
    ///
    /// Delegates to [`BridgeProviderType::is_receiver_account_bridge_provider`]
    /// on the inner `provider_type` field.
    ///
    /// # Returns
    ///
    /// `true` when `provider_type` is
    /// [`BridgeProviderType::ReceiverAccountBridgeProvider`], `false` otherwise.
    #[must_use]
    pub const fn is_receiver_account_bridge_provider(&self) -> bool {
        self.provider_type.is_receiver_account_bridge_provider()
    }
}

// ── Bridge status ─────────────────────────────────────────────────────────────

/// Status of a cross-chain bridge transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BridgeStatus {
    /// The bridge transaction is still in progress.
    InProgress,
    /// The bridge transaction was successfully executed.
    Executed,
    /// The bridge transaction has expired.
    Expired,
    /// The bridge transaction was refunded.
    Refund,
    /// The bridge status is unknown.
    Unknown,
}

/// Result of querying a bridge transaction's status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStatusResult {
    /// Current status of the bridge.
    pub status: BridgeStatus,
    /// Time in seconds for the fill to complete, if available.
    pub fill_time_in_seconds: Option<u64>,
    /// Transaction hash of the deposit on the origin chain.
    pub deposit_tx_hash: Option<String>,
    /// Transaction hash of the fill on the destination chain.
    pub fill_tx_hash: Option<String>,
}

impl BridgeStatusResult {
    /// Create a status result with only a status.
    ///
    /// All optional fields (`fill_time_in_seconds`, `deposit_tx_hash`,
    /// `fill_tx_hash`) are set to `None`.
    ///
    /// # Arguments
    ///
    /// * `status` - The current [`BridgeStatus`] of the bridge transaction.
    ///
    /// # Returns
    ///
    /// A new [`BridgeStatusResult`] with only the status populated.
    #[must_use]
    pub const fn new(status: BridgeStatus) -> Self {
        Self { status, fill_time_in_seconds: None, deposit_tx_hash: None, fill_tx_hash: None }
    }
}

// ── Quote request / response ──────────────────────────────────────────────────

/// Request for a cross-chain bridge quote.
#[derive(Debug, Clone)]
pub struct QuoteBridgeRequest {
    /// Chain ID of the source chain.
    pub sell_chain_id: u64,
    /// Chain ID of the destination chain.
    pub buy_chain_id: u64,
    /// Token address on the source chain.
    pub sell_token: Address,
    /// Token decimals on the source chain.
    pub sell_token_decimals: u8,
    /// Token address on the destination chain.
    pub buy_token: Address,
    /// Token decimals on the destination chain.
    pub buy_token_decimals: u8,
    /// Amount of `sell_token` to bridge (in atoms).
    pub sell_amount: U256,
    /// Address of the user initiating the bridge.
    pub account: Address,
    /// Optional owner address.
    pub owner: Option<Address>,
    /// Optional receiver address on the destination chain.
    pub receiver: Option<String>,
    /// Optional bridge recipient (may be non-EVM, e.g. Solana/BTC).
    pub bridge_recipient: Option<String>,
    /// Slippage tolerance in basis points for the swap leg.
    pub slippage_bps: u32,
    /// Optional bridge-specific slippage tolerance in basis points.
    pub bridge_slippage_bps: Option<u32>,
    /// Whether this is a sell or buy order.
    pub kind: crate::OrderKind,
}

/// Amounts (sell and buy) at various stages of a bridge quote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeAmounts {
    /// Amount being sold (source chain atoms).
    pub sell_amount: U256,
    /// Amount being received (destination chain atoms).
    pub buy_amount: U256,
}

/// Costs associated with bridging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeCosts {
    /// Bridging fee information.
    pub bridging_fee: BridgingFee,
}

/// Fee breakdown for a bridge transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgingFee {
    /// Fee in basis points.
    pub fee_bps: u32,
    /// Fee amount denominated in the sell token.
    pub amount_in_sell_currency: U256,
    /// Fee amount denominated in the buy token.
    pub amount_in_buy_currency: U256,
}

/// Full amounts-and-costs breakdown for a bridge quote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeQuoteAmountsAndCosts {
    /// Costs of the bridging.
    pub costs: BridgeCosts,
    /// Amounts before fees.
    pub before_fee: BridgeAmounts,
    /// Amounts after fees.
    pub after_fee: BridgeAmounts,
    /// Amounts after slippage tolerance (minimum the user will receive).
    pub after_slippage: BridgeAmounts,
    /// Slippage tolerance in basis points.
    pub slippage_bps: u32,
}

/// Fee limits for a bridge deposit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeLimits {
    /// Minimum deposit amount in token atoms.
    pub min_deposit: U256,
    /// Maximum deposit amount in token atoms.
    pub max_deposit: U256,
}

/// Fee amounts charged by the bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeFees {
    /// Fee to cover relayer capital costs (in token atoms).
    pub bridge_fee: U256,
    /// Fee to cover destination chain gas costs (in token atoms).
    pub destination_gas_fee: U256,
}

/// A bridge quote from a single provider.
#[derive(Debug, Clone)]
pub struct QuoteBridgeResponse {
    /// Bridge provider identifier (e.g. `"bungee"`).
    pub provider: String,
    /// Input amount on the source chain.
    pub sell_amount: U256,
    /// Minimum output amount on the destination chain.
    pub buy_amount: U256,
    /// Fee charged by the bridge (in `buy_token` atoms).
    pub fee_amount: U256,
    /// Estimated seconds for the bridge to complete.
    pub estimated_secs: u64,
    /// Optional pre-interaction hook that triggers the bridge.
    pub bridge_hook: Option<CowHook>,
}

impl QuoteBridgeResponse {
    /// Returns `true` if a bridge hook is attached.
    ///
    /// Hook-based bridge providers attach a [`CowHook`] that is executed as a
    /// post-interaction to initiate the bridge transfer.
    ///
    /// # Returns
    ///
    /// `true` when `bridge_hook` is `Some`, `false` otherwise.
    #[must_use]
    pub const fn has_bridge_hook(&self) -> bool {
        self.bridge_hook.is_some()
    }

    /// Return a reference to the provider name.
    ///
    /// # Returns
    ///
    /// A string slice of the provider identifier (e.g. `"across"`, `"bungee"`).
    #[must_use]
    pub fn provider_ref(&self) -> &str {
        &self.provider
    }

    /// Net buy amount after subtracting the fee.
    ///
    /// Uses saturating subtraction: returns zero if `fee_amount > buy_amount`.
    #[must_use]
    pub const fn net_buy_amount(&self) -> U256 {
        self.buy_amount.saturating_sub(self.fee_amount)
    }
}

/// Result of a bridge quote with full cost details.
#[derive(Debug, Clone)]
pub struct BridgeQuoteResult {
    /// Unique ID of the quote.
    pub id: Option<String>,
    /// Provider quote signature (for `ReceiverAccountBridgeProvider`).
    pub signature: Option<String>,
    /// Attestation signature from the bridge provider.
    pub attestation_signature: Option<String>,
    /// Stringified JSON of the provider-specific quote body.
    pub quote_body: Option<String>,
    /// Whether this is a sell order.
    pub is_sell: bool,
    /// Full amounts and costs breakdown.
    pub amounts_and_costs: BridgeQuoteAmountsAndCosts,
    /// Estimated fill time in seconds.
    pub expected_fill_time_seconds: Option<u64>,
    /// Quote creation timestamp (UNIX seconds).
    pub quote_timestamp: u64,
    /// Bridge fees.
    pub fees: BridgeFees,
    /// Deposit limits.
    pub limits: BridgeLimits,
}

/// Extended bridge quote results with provider and trade context.
#[derive(Debug, Clone)]
pub struct BridgeQuoteResults {
    /// Bridge provider info.
    pub provider_info: BridgeProviderInfo,
    /// The bridge quote result.
    pub quote: BridgeQuoteResult,
    /// Bridge call details (for hook-based providers).
    pub bridge_call_details: Option<BridgeCallDetails>,
    /// Override receiver address (for receiver-account providers).
    pub bridge_receiver_override: Option<String>,
}

/// Details about a bridge hook call.
#[derive(Debug, Clone)]
pub struct BridgeCallDetails {
    /// Unsigned call to initiate the bridge.
    pub unsigned_bridge_call: crate::config::EvmCall,
    /// Pre-authorized bridging hook.
    pub pre_authorized_bridging_hook: BridgeHook,
}

/// A signed bridge hook ready for inclusion in a `CoW` Protocol order.
#[derive(Debug, Clone)]
pub struct BridgeHook {
    /// The post-hook to include in the order's app data.
    pub post_hook: CowHook,
    /// The recipient address for the bridged funds.
    pub recipient: String,
}

/// Parameters extracted from a bridging deposit event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgingDepositParams {
    /// Input token address.
    pub input_token_address: Address,
    /// Output token address.
    pub output_token_address: Address,
    /// Amount of input tokens deposited.
    pub input_amount: U256,
    /// Expected output amount (may be `None` if unknown).
    pub output_amount: Option<U256>,
    /// Address of the depositor.
    pub owner: Address,
    /// Quote timestamp used for fee computation.
    pub quote_timestamp: Option<u64>,
    /// Fill deadline as a UNIX timestamp.
    pub fill_deadline: Option<u64>,
    /// Recipient of bridged funds on the destination chain.
    pub recipient: Address,
    /// Source chain ID.
    pub source_chain_id: u64,
    /// Destination chain ID.
    pub destination_chain_id: u64,
    /// Provider-specific bridging identifier.
    pub bridging_id: String,
}

/// A resolved cross-chain order with bridging details.
#[derive(Debug, Clone)]
pub struct CrossChainOrder {
    /// Chain ID where the order was settled.
    pub chain_id: u64,
    /// Bridging status result.
    pub status_result: BridgeStatusResult,
    /// Bridging deposit parameters.
    pub bridging_params: BridgingDepositParams,
    /// Settlement transaction hash.
    pub trade_tx_hash: String,
    /// Bridge explorer URL for tracking.
    pub explorer_url: Option<String>,
}

/// Result from a single provider in a multi-quote request.
#[derive(Debug, Clone)]
pub struct MultiQuoteResult {
    /// The provider's dApp ID.
    pub provider_dapp_id: String,
    /// The bridge quote, if successful.
    pub quote: Option<BridgeQuoteAmountsAndCosts>,
    /// Error message, if the provider failed.
    pub error: Option<String>,
}

// ── Errors ────────────────────────────────────────────────────────────────────

/// Errors specific to bridging operations.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    /// No bridge providers are registered.
    #[error("no providers available")]
    NoProviders,
    /// None of the registered providers returned a quote for this route.
    #[error("no quote available for this route")]
    NoQuote,
    /// Attempted a cross-chain operation on same-chain tokens.
    #[error("sell and buy chains must be different for cross-chain bridging")]
    SameChain,
    /// Only sell orders are supported for bridging.
    #[error("bridging only supports SELL orders")]
    OnlySellOrderSupported,
    /// No intermediate tokens available for the requested route.
    #[error("no intermediate tokens available")]
    NoIntermediateTokens,
    /// The bridge API returned an error.
    #[error("bridge API error: {0}")]
    ApiError(String),
    /// Invalid API response format.
    #[error("invalid API JSON response: {0}")]
    InvalidApiResponse(String),
    /// Error building the bridge transaction.
    #[error("transaction build error: {0}")]
    TxBuildError(String),
    /// General quote error.
    #[error("quote error: {0}")]
    QuoteError(String),
    /// No routes found.
    #[error("no routes available")]
    NoRoutes,
    /// Invalid bridge configuration.
    #[error("invalid bridge: {0}")]
    InvalidBridge(String),
    /// Quote does not match expected deposit address.
    #[error("quote does not match deposit address")]
    QuoteDoesNotMatchDepositAddress,
    /// Sell amount is below the minimum threshold.
    #[error("sell amount too small")]
    SellAmountTooSmall,
    /// Provider with the given dApp ID was not found.
    #[error("provider not found: {dapp_id}")]
    ProviderNotFound {
        /// The requested dApp ID.
        dapp_id: String,
    },
    /// Provider request timed out.
    #[error("provider request timed out")]
    Timeout,
    /// A `CoW` Protocol API error.
    #[error(transparent)]
    Cow(#[from] crate::CowError),
}

/// Priority of bridge quote errors for selecting the best error to surface.
///
/// Higher values indicate errors that are more relevant to the user.
/// When multiple providers fail, the error with the highest priority is
/// shown so that the most actionable message reaches the caller.
///
/// # Arguments
///
/// * `error` - The [`BridgeError`] to evaluate.
///
/// # Returns
///
/// A numeric priority (`u32`). Currently `10` for
/// [`BridgeError::SellAmountTooSmall`], `9` for
/// [`BridgeError::OnlySellOrderSupported`], and `1` for all other variants.
#[must_use]
pub const fn bridge_error_priority(error: &BridgeError) -> u32 {
    match error {
        BridgeError::SellAmountTooSmall => 10,
        BridgeError::OnlySellOrderSupported => 9,
        BridgeError::NoProviders |
        BridgeError::NoQuote |
        BridgeError::SameChain |
        BridgeError::NoIntermediateTokens |
        BridgeError::ApiError(_) |
        BridgeError::InvalidApiResponse(_) |
        BridgeError::TxBuildError(_) |
        BridgeError::QuoteError(_) |
        BridgeError::NoRoutes |
        BridgeError::InvalidBridge(_) |
        BridgeError::QuoteDoesNotMatchDepositAddress |
        BridgeError::ProviderNotFound { .. } |
        BridgeError::Timeout |
        BridgeError::Cow(_) => 1,
    }
}

// ── Across-specific types ─────────────────────────────────────────────────────

/// A percentage fee as returned by the Across API.
///
/// `pct` is expressed in Across format: 1% = 1e16, 100% = 1e18.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcrossPctFee {
    /// Percentage as a string in contract format (1e18 = 100%).
    pub pct: String,
    /// Total fee amount as a string.
    pub total: String,
}

/// Deposit size limits from the Across API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcrossSuggestedFeesLimits {
    /// Minimum deposit size in token units.
    pub min_deposit: String,
    /// Maximum deposit size in token units.
    pub max_deposit: String,
    /// Maximum instant-fill deposit size.
    pub max_deposit_instant: String,
    /// Maximum short-delay deposit size.
    pub max_deposit_short_delay: String,
    /// Recommended instant deposit size.
    pub recommended_deposit_instant: String,
}

/// Full response from the Across suggested-fees endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcrossSuggestedFeesResponse {
    /// Total relay fee (inclusive of LP fee).
    pub total_relay_fee: AcrossPctFee,
    /// Relayer capital fee component.
    pub relayer_capital_fee: AcrossPctFee,
    /// Relayer gas fee component.
    pub relayer_gas_fee: AcrossPctFee,
    /// LP fee component.
    pub lp_fee: AcrossPctFee,
    /// Quote timestamp for LP fee computation.
    pub timestamp: String,
    /// Whether the amount is below the minimum.
    pub is_amount_too_low: bool,
    /// Block number associated with the quote.
    pub quote_block: String,
    /// Spoke pool contract address.
    pub spoke_pool_address: String,
    /// Suggested exclusive relayer address.
    pub exclusive_relayer: String,
    /// Exclusivity deadline in seconds.
    pub exclusivity_deadline: String,
    /// Estimated fill time in seconds.
    pub estimated_fill_time_sec: String,
    /// Recommended fill deadline as a UNIX timestamp.
    pub fill_deadline: String,
    /// Deposit size limits.
    pub limits: AcrossSuggestedFeesLimits,
}

/// Status of an Across deposit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AcrossDepositStatus {
    /// Deposit has been filled.
    Filled,
    /// Slow fill was requested.
    SlowFillRequested,
    /// Deposit is still pending.
    Pending,
    /// Deposit has expired.
    Expired,
    /// Deposit was refunded.
    Refunded,
}

/// Response from the Across deposit status endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcrossDepositStatusResponse {
    /// Current deposit status.
    pub status: AcrossDepositStatus,
    /// Origin chain ID.
    pub origin_chain_id: String,
    /// Unique deposit identifier.
    pub deposit_id: String,
    /// Deposit transaction hash on the origin chain.
    pub deposit_tx_hash: Option<String>,
    /// Fill transaction hash on the destination chain.
    pub fill_tx: Option<String>,
    /// Destination chain ID.
    pub destination_chain_id: Option<String>,
    /// Refund transaction hash.
    pub deposit_refund_tx_hash: Option<String>,
}

/// An Across deposit event parsed from transaction logs.
#[derive(Debug, Clone)]
pub struct AcrossDepositEvent {
    /// Input token address.
    pub input_token: Address,
    /// Output token address.
    pub output_token: Address,
    /// Amount of input tokens.
    pub input_amount: U256,
    /// Expected output amount.
    pub output_amount: U256,
    /// Destination chain ID.
    pub destination_chain_id: u64,
    /// Unique deposit identifier.
    pub deposit_id: U256,
    /// Quote timestamp for fee computation.
    pub quote_timestamp: u32,
    /// Fill deadline as a UNIX timestamp.
    pub fill_deadline: u32,
    /// Exclusivity deadline.
    pub exclusivity_deadline: u32,
    /// Depositor address.
    pub depositor: Address,
    /// Recipient address.
    pub recipient: Address,
    /// Exclusive relayer address.
    pub exclusive_relayer: Address,
}

/// Chain-specific token configuration for Across.
#[derive(Debug, Clone)]
pub struct AcrossChainConfig {
    /// Chain ID.
    pub chain_id: u64,
    /// Token symbol to address mapping.
    pub tokens: HashMap<String, Address>,
}

// ── Bungee-specific types ─────────────────────────────────────────────────────

/// Supported Bungee bridge variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BungeeBridge {
    /// Across bridge via Bungee.
    Across,
    /// Circle CCTP bridge via Bungee.
    CircleCctp,
    /// Gnosis native bridge.
    GnosisNative,
}

impl BungeeBridge {
    /// Return the API string identifier for this bridge.
    ///
    /// # Returns
    ///
    /// A static string used in Bungee API calls:
    /// - [`Across`](Self::Across) -> `"across"`
    /// - [`CircleCctp`](Self::CircleCctp) -> `"cctp"`
    /// - [`GnosisNative`](Self::GnosisNative) -> `"gnosis-native-bridge"`
    ///
    /// # Examples
    ///
    /// ```
    /// use cow_rs::bridging::types::BungeeBridge;
    ///
    /// assert_eq!(BungeeBridge::Across.as_str(), "across");
    /// assert_eq!(BungeeBridge::CircleCctp.as_str(), "cctp");
    /// ```
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Across => "across",
            Self::CircleCctp => "cctp",
            Self::GnosisNative => "gnosis-native-bridge",
        }
    }

    /// Try to parse a bridge from its display name.
    ///
    /// This is the inverse of [`display_name`](Self::display_name).
    ///
    /// # Arguments
    ///
    /// * `name` - A human-readable bridge name (e.g. `"Across"`, `"Circle CCTP"`, `"Gnosis
    ///   Native"`).
    ///
    /// # Returns
    ///
    /// `Some(BungeeBridge)` if the name matches a known variant, `None`
    /// otherwise.
    #[must_use]
    pub fn from_display_name(name: &str) -> Option<Self> {
        match name {
            "Across" => Some(Self::Across),
            "Circle CCTP" => Some(Self::CircleCctp),
            "Gnosis Native" => Some(Self::GnosisNative),
            _ => None,
        }
    }

    /// Return the human-readable display name.
    ///
    /// This is the inverse of [`from_display_name`](Self::from_display_name).
    ///
    /// # Returns
    ///
    /// A static, human-readable label for this bridge variant (e.g.
    /// `"Across"`, `"Circle CCTP"`, `"Gnosis Native"`).
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Across => "Across",
            Self::CircleCctp => "Circle CCTP",
            Self::GnosisNative => "Gnosis Native",
        }
    }
}

/// Bungee event status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BungeeEventStatus {
    /// Event is complete.
    Completed,
    /// Event is still pending.
    Pending,
}

/// Bridge name as used in Bungee events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BungeeBridgeName {
    /// Across bridge.
    Across,
    /// Circle CCTP bridge.
    Cctp,
}

/// A Bungee bridge event from the events API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BungeeEvent {
    /// Event identifier.
    pub identifier: String,
    /// Source transaction hash (None when pending).
    pub src_transaction_hash: Option<String>,
    /// Bridge name used.
    pub bridge_name: BungeeBridgeName,
    /// Origin chain ID.
    pub from_chain_id: u64,
    /// Whether this is a `CoW` Swap trade.
    pub is_cowswap_trade: bool,
    /// `CoW` Protocol order ID.
    pub order_id: String,
    /// Source transaction status.
    pub src_tx_status: BungeeEventStatus,
    /// Destination transaction status.
    pub dest_tx_status: BungeeEventStatus,
    /// Destination transaction hash (None when pending).
    pub dest_transaction_hash: Option<String>,
}

/// Byte offset indices for decoding Bungee transaction data.
#[derive(Debug, Clone, Copy)]
pub struct BungeeTxDataBytesIndex {
    /// Byte start offset in the raw calldata.
    pub bytes_start_index: usize,
    /// Byte length.
    pub bytes_length: usize,
    /// Character start offset in the hex string (including `0x` prefix).
    pub bytes_string_start_index: usize,
    /// Character length in the hex string.
    pub bytes_string_length: usize,
}

/// Decoded result from Bungee transaction data.
#[derive(Debug, Clone)]
pub struct DecodedBungeeTxData {
    /// Route ID (first 4 bytes).
    pub route_id: String,
    /// Encoded function data (after route ID).
    pub encoded_function_data: String,
    /// Function selector (first 4 bytes of function data).
    pub function_selector: String,
}

/// Decoded amounts from Bungee transaction data.
#[derive(Debug, Clone)]
pub struct DecodedBungeeAmounts {
    /// Raw input amount bytes as hex string.
    pub input_amount_bytes: String,
    /// Parsed input amount as U256.
    pub input_amount: U256,
}
