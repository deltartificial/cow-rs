//! Cross-chain bridging types.

use foldhash::HashMap;

use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};

use cow_types::CowHook;

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
    /// Equivalent to the `TypeScript` `isHookBridgeProvider` type guard.
    #[must_use]
    pub const fn is_hook_bridge_provider(self) -> bool {
        matches!(self, Self::HookBridgeProvider)
    }

    /// Returns `true` if this is a
    /// [`ReceiverAccountBridgeProvider`](Self::ReceiverAccountBridgeProvider).
    ///
    /// Equivalent to the `TypeScript` `isReceiverAccountBridgeProvider` type guard.
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
    pub kind: cow_types::OrderKind,
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
    pub unsigned_bridge_call: cow_chains::EvmCall,
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
    Cow(#[from] cow_errors::CowError),
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
    /// use cow_bridging::types::BungeeBridge;
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── BridgeProviderType ──────────────────────────────────────────────

    #[test]
    fn hook_bridge_provider_is_hook() {
        assert!(BridgeProviderType::HookBridgeProvider.is_hook_bridge_provider());
        assert!(!BridgeProviderType::HookBridgeProvider.is_receiver_account_bridge_provider());
    }

    #[test]
    fn receiver_account_bridge_provider_is_receiver() {
        assert!(
            BridgeProviderType::ReceiverAccountBridgeProvider.is_receiver_account_bridge_provider()
        );
        assert!(!BridgeProviderType::ReceiverAccountBridgeProvider.is_hook_bridge_provider());
    }

    // ── BridgeProviderInfo delegation ───────────────────────────────────

    #[test]
    fn bridge_provider_info_delegates_hook() {
        let info = BridgeProviderInfo {
            name: "test".into(),
            logo_url: String::new(),
            dapp_id: String::new(),
            website: String::new(),
            provider_type: BridgeProviderType::HookBridgeProvider,
        };
        assert!(info.is_hook_bridge_provider());
        assert!(!info.is_receiver_account_bridge_provider());
    }

    #[test]
    fn bridge_provider_info_delegates_receiver() {
        let info = BridgeProviderInfo {
            name: "test".into(),
            logo_url: String::new(),
            dapp_id: String::new(),
            website: String::new(),
            provider_type: BridgeProviderType::ReceiverAccountBridgeProvider,
        };
        assert!(info.is_receiver_account_bridge_provider());
        assert!(!info.is_hook_bridge_provider());
    }

    // ── BridgeStatus ────────────────────────────────────────────────────

    #[test]
    fn bridge_status_variants_are_distinct() {
        let statuses = [
            BridgeStatus::InProgress,
            BridgeStatus::Executed,
            BridgeStatus::Expired,
            BridgeStatus::Refund,
            BridgeStatus::Unknown,
        ];
        for (i, a) in statuses.iter().enumerate() {
            for (j, b) in statuses.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    // ── BridgeStatusResult ──────────────────────────────────────────────

    #[test]
    fn bridge_status_result_new_sets_status_only() {
        let r = BridgeStatusResult::new(BridgeStatus::Executed);
        assert_eq!(r.status, BridgeStatus::Executed);
        assert!(r.fill_time_in_seconds.is_none());
        assert!(r.deposit_tx_hash.is_none());
        assert!(r.fill_tx_hash.is_none());
    }

    #[test]
    fn bridge_status_result_new_all_statuses() {
        for status in [
            BridgeStatus::InProgress,
            BridgeStatus::Executed,
            BridgeStatus::Expired,
            BridgeStatus::Refund,
            BridgeStatus::Unknown,
        ] {
            let r = BridgeStatusResult::new(status);
            assert_eq!(r.status, status);
        }
    }

    // ── QuoteBridgeResponse ─────────────────────────────────────────────

    fn make_quote(hook: Option<CowHook>, fee: U256) -> QuoteBridgeResponse {
        QuoteBridgeResponse {
            provider: "across".into(),
            sell_amount: U256::from(1000u64),
            buy_amount: U256::from(950u64),
            fee_amount: fee,
            estimated_secs: 60,
            bridge_hook: hook,
        }
    }

    #[test]
    fn has_bridge_hook_true_when_some() {
        let hook = CowHook {
            target: "0xdead".into(),
            call_data: "0x".into(),
            gas_limit: "100000".into(),
            dapp_id: None,
        };
        let q = make_quote(Some(hook), U256::ZERO);
        assert!(q.has_bridge_hook());
    }

    #[test]
    fn has_bridge_hook_false_when_none() {
        let q = make_quote(None, U256::ZERO);
        assert!(!q.has_bridge_hook());
    }

    #[test]
    fn provider_ref_returns_provider_name() {
        let q = make_quote(None, U256::ZERO);
        assert_eq!(q.provider_ref(), "across");
    }

    #[test]
    fn net_buy_amount_subtracts_fee() {
        let q = make_quote(None, U256::from(50u64));
        assert_eq!(q.net_buy_amount(), U256::from(900u64));
    }

    #[test]
    fn net_buy_amount_saturates_at_zero() {
        let q = make_quote(None, U256::from(2000u64));
        assert_eq!(q.net_buy_amount(), U256::ZERO);
    }

    #[test]
    fn net_buy_amount_zero_fee() {
        let q = make_quote(None, U256::ZERO);
        assert_eq!(q.net_buy_amount(), U256::from(950u64));
    }

    // ── BungeeBridge ────────────────────────────────────────────────────

    #[test]
    fn bungee_bridge_as_str() {
        assert_eq!(BungeeBridge::Across.as_str(), "across");
        assert_eq!(BungeeBridge::CircleCctp.as_str(), "cctp");
        assert_eq!(BungeeBridge::GnosisNative.as_str(), "gnosis-native-bridge");
    }

    #[test]
    fn bungee_bridge_display_name() {
        assert_eq!(BungeeBridge::Across.display_name(), "Across");
        assert_eq!(BungeeBridge::CircleCctp.display_name(), "Circle CCTP");
        assert_eq!(BungeeBridge::GnosisNative.display_name(), "Gnosis Native");
    }

    #[test]
    fn bungee_bridge_from_display_name_valid() {
        assert_eq!(BungeeBridge::from_display_name("Across"), Some(BungeeBridge::Across));
        assert_eq!(BungeeBridge::from_display_name("Circle CCTP"), Some(BungeeBridge::CircleCctp));
        assert_eq!(
            BungeeBridge::from_display_name("Gnosis Native"),
            Some(BungeeBridge::GnosisNative)
        );
    }

    #[test]
    fn bungee_bridge_from_display_name_invalid() {
        assert_eq!(BungeeBridge::from_display_name("across"), None);
        assert_eq!(BungeeBridge::from_display_name(""), None);
        assert_eq!(BungeeBridge::from_display_name("Unknown"), None);
    }

    #[test]
    fn bungee_bridge_roundtrip_display_name() {
        for bridge in [BungeeBridge::Across, BungeeBridge::CircleCctp, BungeeBridge::GnosisNative] {
            let name = bridge.display_name();
            assert_eq!(BungeeBridge::from_display_name(name), Some(bridge));
        }
    }

    // ── BridgeError priority ────────────────────────────────────────────

    #[test]
    fn sell_amount_too_small_has_highest_priority() {
        assert_eq!(bridge_error_priority(&BridgeError::SellAmountTooSmall), 10);
    }

    #[test]
    fn only_sell_order_supported_has_second_priority() {
        assert_eq!(bridge_error_priority(&BridgeError::OnlySellOrderSupported), 9);
    }

    #[test]
    fn other_errors_have_base_priority() {
        let base_errors: Vec<BridgeError> = vec![
            BridgeError::NoProviders,
            BridgeError::NoQuote,
            BridgeError::SameChain,
            BridgeError::NoIntermediateTokens,
            BridgeError::ApiError("test".into()),
            BridgeError::InvalidApiResponse("test".into()),
            BridgeError::TxBuildError("test".into()),
            BridgeError::QuoteError("test".into()),
            BridgeError::NoRoutes,
            BridgeError::InvalidBridge("test".into()),
            BridgeError::QuoteDoesNotMatchDepositAddress,
            BridgeError::ProviderNotFound { dapp_id: "test".into() },
            BridgeError::Timeout,
        ];
        for e in &base_errors {
            assert_eq!(bridge_error_priority(e), 1, "expected priority 1 for {e}");
        }
    }

    // ── BridgeError Display ─────────────────────────────────────────────

    #[test]
    fn bridge_error_display_messages() {
        assert_eq!(BridgeError::NoProviders.to_string(), "no providers available");
        assert_eq!(BridgeError::NoQuote.to_string(), "no quote available for this route");
        assert_eq!(
            BridgeError::SameChain.to_string(),
            "sell and buy chains must be different for cross-chain bridging"
        );
        assert_eq!(
            BridgeError::OnlySellOrderSupported.to_string(),
            "bridging only supports SELL orders"
        );
        assert_eq!(
            BridgeError::NoIntermediateTokens.to_string(),
            "no intermediate tokens available"
        );
        assert_eq!(BridgeError::ApiError("oops".into()).to_string(), "bridge API error: oops");
        assert_eq!(
            BridgeError::InvalidApiResponse("bad".into()).to_string(),
            "invalid API JSON response: bad"
        );
        assert_eq!(
            BridgeError::TxBuildError("fail".into()).to_string(),
            "transaction build error: fail"
        );
        assert_eq!(BridgeError::QuoteError("nope".into()).to_string(), "quote error: nope");
        assert_eq!(BridgeError::NoRoutes.to_string(), "no routes available");
        assert_eq!(BridgeError::InvalidBridge("x".into()).to_string(), "invalid bridge: x");
        assert_eq!(
            BridgeError::QuoteDoesNotMatchDepositAddress.to_string(),
            "quote does not match deposit address"
        );
        assert_eq!(BridgeError::SellAmountTooSmall.to_string(), "sell amount too small");
        assert_eq!(
            BridgeError::ProviderNotFound { dapp_id: "foo".into() }.to_string(),
            "provider not found: foo"
        );
        assert_eq!(BridgeError::Timeout.to_string(), "provider request timed out");
    }

    // ── Serde roundtrips ────────────────────────────────────────────────

    #[test]
    fn bridge_provider_type_serde_roundtrip() {
        for v in [
            BridgeProviderType::HookBridgeProvider,
            BridgeProviderType::ReceiverAccountBridgeProvider,
        ] {
            let json = serde_json::to_string(&v).unwrap();
            let back: BridgeProviderType = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back);
        }
    }

    #[test]
    fn bridge_status_serde_roundtrip() {
        for v in [
            BridgeStatus::InProgress,
            BridgeStatus::Executed,
            BridgeStatus::Expired,
            BridgeStatus::Refund,
            BridgeStatus::Unknown,
        ] {
            let json = serde_json::to_string(&v).unwrap();
            let back: BridgeStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back);
        }
    }

    #[test]
    fn bungee_bridge_serde_roundtrip() {
        for v in [BungeeBridge::Across, BungeeBridge::CircleCctp, BungeeBridge::GnosisNative] {
            let json = serde_json::to_string(&v).unwrap();
            let back: BungeeBridge = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back);
        }
    }

    #[test]
    fn across_deposit_status_serde_roundtrip() {
        for v in [
            AcrossDepositStatus::Filled,
            AcrossDepositStatus::SlowFillRequested,
            AcrossDepositStatus::Pending,
            AcrossDepositStatus::Expired,
            AcrossDepositStatus::Refunded,
        ] {
            let json = serde_json::to_string(&v).unwrap();
            let back: AcrossDepositStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back);
        }
    }

    #[test]
    fn across_deposit_status_camel_case_serialization() {
        assert_eq!(serde_json::to_string(&AcrossDepositStatus::Filled).unwrap(), "\"filled\"");
        assert_eq!(
            serde_json::to_string(&AcrossDepositStatus::SlowFillRequested).unwrap(),
            "\"slowFillRequested\""
        );
        assert_eq!(serde_json::to_string(&AcrossDepositStatus::Pending).unwrap(), "\"pending\"");
    }

    #[test]
    fn bungee_event_status_screaming_snake_case() {
        assert_eq!(serde_json::to_string(&BungeeEventStatus::Completed).unwrap(), "\"COMPLETED\"");
        assert_eq!(serde_json::to_string(&BungeeEventStatus::Pending).unwrap(), "\"PENDING\"");
    }

    #[test]
    fn bungee_bridge_name_lowercase_serialization() {
        assert_eq!(serde_json::to_string(&BungeeBridgeName::Across).unwrap(), "\"across\"");
        assert_eq!(serde_json::to_string(&BungeeBridgeName::Cctp).unwrap(), "\"cctp\"");
    }
}
