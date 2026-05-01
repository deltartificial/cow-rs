//! High-level trading types: parameters, fee breakdown, and quote results.

use std::fmt;

use alloy_primitives::{Address, U256};
use cow_orderbook::types::OrderQuoteResponse;
use cow_signing::types::{OrderTypedData, UnsignedOrder};
use cow_types::OrderKind;

/// Amounts at a specific stage of the fee pipeline.
#[derive(Debug, Clone, Copy, Default)]
pub struct Amounts {
    /// Sell-token amount at this stage (in atoms).
    pub sell_amount: U256,
    /// Buy-token amount at this stage (in atoms).
    pub buy_amount: U256,
}

impl Amounts {
    /// Construct an [`Amounts`] from sell and buy amounts.
    ///
    /// # Arguments
    ///
    /// * `sell_amount` — sell-token amount in atoms.
    /// * `buy_amount` — buy-token amount in atoms.
    ///
    /// # Returns
    ///
    /// A new [`Amounts`] instance.
    #[must_use]
    pub const fn new(sell_amount: U256, buy_amount: U256) -> Self {
        Self { sell_amount, buy_amount }
    }

    /// Returns `true` if both sell and buy amounts are zero.
    ///
    /// # Returns
    ///
    /// `true` when `sell_amount` and `buy_amount` are both `U256::ZERO`.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.sell_amount.is_zero() && self.buy_amount.is_zero()
    }

    /// Total token amount: `sell_amount + buy_amount` (saturating).
    ///
    /// ```
    /// use alloy_primitives::U256;
    /// use cow_trading::Amounts;
    ///
    /// let a = Amounts::new(U256::from(100u32), U256::from(90u32));
    /// assert_eq!(a.total(), U256::from(190u32));
    /// ```
    #[must_use]
    pub const fn total(&self) -> U256 {
        self.sell_amount.saturating_add(self.buy_amount)
    }
}

impl fmt::Display for Amounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sell {} → buy {}", self.sell_amount, self.buy_amount)
    }
}

/// Network fee expressed in both currencies.
#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkFee {
    /// Fee denominated in `sell_token` atoms.
    pub amount_in_sell_currency: U256,
    /// Fee denominated in `buy_token` atoms (estimated).
    pub amount_in_buy_currency: U256,
}

impl NetworkFee {
    /// Construct a [`NetworkFee`] from sell-currency and buy-currency amounts.
    ///
    /// # Arguments
    ///
    /// * `amount_in_sell_currency` — fee denominated in sell-token atoms.
    /// * `amount_in_buy_currency` — fee denominated in buy-token atoms (estimated).
    ///
    /// # Returns
    ///
    /// A new [`NetworkFee`] instance.
    #[must_use]
    pub const fn new(amount_in_sell_currency: U256, amount_in_buy_currency: U256) -> Self {
        Self { amount_in_sell_currency, amount_in_buy_currency }
    }

    /// Returns `true` if both fee components are zero.
    ///
    /// # Returns
    ///
    /// `true` when both `amount_in_sell_currency` and `amount_in_buy_currency` are `U256::ZERO`.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.amount_in_sell_currency.is_zero() && self.amount_in_buy_currency.is_zero()
    }

    /// Total fee: `sell_currency + buy_currency` (saturating).
    ///
    /// # Returns
    ///
    /// The saturating sum of both fee components.
    #[must_use]
    pub const fn total_atoms(&self) -> U256 {
        self.amount_in_sell_currency.saturating_add(self.amount_in_buy_currency)
    }
}

impl fmt::Display for NetworkFee {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "network-fee sell={} buy={}",
            self.amount_in_sell_currency, self.amount_in_buy_currency,
        )
    }
}

/// Partner fee cost component.
///
/// Mirrors the `costs.partnerFee` field in the `TypeScript` SDK's `QuoteAmountsAndCosts`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PartnerFeeCost {
    /// Fee amount deducted from the output token (in atoms).
    pub amount: U256,
    /// Fee rate in basis points.
    pub bps: u32,
}

impl PartnerFeeCost {
    /// Construct a [`PartnerFeeCost`] from fee amount and basis points.
    ///
    /// # Arguments
    ///
    /// * `amount` — fee amount deducted from the output token (in atoms).
    /// * `bps` — fee rate in basis points.
    ///
    /// # Returns
    ///
    /// A new [`PartnerFeeCost`] instance.
    #[must_use]
    pub const fn new(amount: U256, bps: u32) -> Self {
        Self { amount, bps }
    }

    /// Returns `true` if the fee amount is zero and the rate is 0 bps.
    ///
    /// # Returns
    ///
    /// `true` when both `amount` is `U256::ZERO` and `bps` is `0`.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.amount.is_zero() && self.bps == 0
    }

    /// Returns `true` if the fee rate is non-zero.
    ///
    /// # Returns
    ///
    /// `true` when `bps > 0`.
    #[must_use]
    pub const fn has_bps(&self) -> bool {
        self.bps > 0
    }
}

impl fmt::Display for PartnerFeeCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "partner-fee {}bps {}", self.bps, self.amount)
    }
}

/// Protocol fee cost component.
///
/// Mirrors the `costs.protocolFee` field in the `TypeScript` SDK's `QuoteAmountsAndCosts`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProtocolFeeCost {
    /// Fee amount (in atoms).
    pub amount: U256,
    /// Fee rate in basis points.
    pub bps: u32,
}

impl ProtocolFeeCost {
    /// Construct a [`ProtocolFeeCost`] from fee amount and basis points.
    ///
    /// # Arguments
    ///
    /// * `amount` — fee amount in atoms.
    /// * `bps` — fee rate in basis points.
    ///
    /// # Returns
    ///
    /// A new [`ProtocolFeeCost`] instance.
    #[must_use]
    pub const fn new(amount: U256, bps: u32) -> Self {
        Self { amount, bps }
    }

    /// Returns `true` if the fee amount is zero and the rate is 0 bps.
    ///
    /// # Returns
    ///
    /// `true` when both `amount` is `U256::ZERO` and `bps` is `0`.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.amount.is_zero() && self.bps == 0
    }

    /// Returns `true` if the fee rate is non-zero.
    ///
    /// # Returns
    ///
    /// `true` when `bps > 0`.
    #[must_use]
    pub const fn has_bps(&self) -> bool {
        self.bps > 0
    }
}

impl fmt::Display for ProtocolFeeCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "protocol-fee {}bps {}", self.bps, self.amount)
    }
}

/// Full fee and amount breakdown for a quoted trade, mirroring the `CoW` SDK's
/// `QuoteAmountsAndCosts` structure.
#[derive(Debug, Clone, Copy)]
pub struct QuoteAmountsAndCosts {
    /// `true` for sell orders, `false` for buy orders.
    pub is_sell: bool,
    /// Gross amounts before **all** fees (network + partner + protocol).
    pub before_all_fees: Amounts,
    /// Gross amounts before the network fee (may reflect protocol fee adjustments).
    pub before_network_costs: Amounts,
    /// Amounts after deducting the network (protocol) fee.
    pub after_network_costs: Amounts,
    /// Amounts after deducting the partner fee.
    pub after_partner_fees: Amounts,
    /// Amounts after applying the requested slippage tolerance.
    pub after_slippage: Amounts,
    /// The network fee component.
    pub network_fee: NetworkFee,
    /// The partner fee component (zero when no partner fee is configured).
    pub partner_fee: PartnerFeeCost,
    /// The protocol fee component (zero when not applicable).
    pub protocol_fee: ProtocolFeeCost,
}

/// App-data document and its `keccak256` hash.
///
/// Mirrors `TradingAppDataInfo` from the `TypeScript` SDK.
#[derive(Debug, Clone)]
pub struct TradingAppDataInfo {
    /// The canonical JSON app-data document (full UTF-8 string).
    pub full_app_data: String,
    /// `keccak256(full_app_data)` as a `0x`-prefixed 32-byte hex string.
    pub app_data_keccak256: String,
}

impl TradingAppDataInfo {
    /// Construct a [`TradingAppDataInfo`] from raw content and its hash.
    ///
    /// # Arguments
    ///
    /// * `full_app_data` — the canonical JSON app-data document.
    /// * `app_data_keccak256` — `keccak256(full_app_data)` as a `0x`-prefixed hex string.
    ///
    /// # Returns
    ///
    /// A new [`TradingAppDataInfo`] instance.
    #[must_use]
    pub fn new(full_app_data: impl Into<String>, app_data_keccak256: impl Into<String>) -> Self {
        Self { full_app_data: full_app_data.into(), app_data_keccak256: app_data_keccak256.into() }
    }

    /// Returns `true` if the full app-data document is non-empty.
    ///
    /// # Returns
    ///
    /// `true` when `full_app_data` is a non-empty string.
    #[must_use]
    pub const fn has_full_app_data(&self) -> bool {
        !self.full_app_data.is_empty()
    }

    /// Returns the full app-data JSON document as a string slice.
    ///
    /// # Returns
    ///
    /// A `&str` reference to the full app-data JSON content.
    ///
    /// ```
    /// use cow_trading::TradingAppDataInfo;
    ///
    /// let info = TradingAppDataInfo::new("{}", "0xabc");
    /// assert_eq!(info.full_app_data_ref(), "{}");
    /// ```
    #[must_use]
    pub fn full_app_data_ref(&self) -> &str {
        &self.full_app_data
    }

    /// Returns the `keccak256` hash of the app-data as a `0x`-prefixed hex string slice.
    ///
    /// # Returns
    ///
    /// A `&str` reference to the `0x`-prefixed 32-byte hex hash.
    ///
    /// ```
    /// use cow_trading::TradingAppDataInfo;
    ///
    /// let info = TradingAppDataInfo::new("{}", "0xdeadbeef");
    /// assert_eq!(info.keccak256_ref(), "0xdeadbeef");
    /// ```
    #[must_use]
    pub fn keccak256_ref(&self) -> &str {
        &self.app_data_keccak256
    }
}

impl fmt::Display for TradingAppDataInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "app-data({})", self.app_data_keccak256)
    }
}

/// A raw Ethereum transaction object produced by the `CoW` SDK on-chain helpers.
///
/// Mirrors `TradingTransactionParams` from the `TypeScript` SDK.
/// Used by calldata-returning functions such as `getPreSignTransaction` and
/// `getEthFlowTransaction`.
#[derive(Debug, Clone)]
pub struct TradingTransactionParams {
    /// ABI-encoded calldata (`0x`-prefixed hex string).
    pub data: Vec<u8>,
    /// Target contract address.
    pub to: Address,
    /// Gas limit for the transaction.
    pub gas_limit: u64,
    /// ETH value to send in wei (usually zero for token swaps).
    pub value: U256,
}

impl TradingTransactionParams {
    /// Construct a [`TradingTransactionParams`] from its four core fields.
    ///
    /// # Arguments
    ///
    /// * `data` — ABI-encoded calldata bytes.
    /// * `to` — target contract address.
    /// * `gas_limit` — gas limit for the transaction.
    /// * `value` — ETH value to send in wei.
    ///
    /// # Returns
    ///
    /// A new [`TradingTransactionParams`] instance.
    #[must_use]
    pub const fn new(data: Vec<u8>, to: Address, gas_limit: u64, value: U256) -> Self {
        Self { data, to, gas_limit, value }
    }

    /// Override the ABI-encoded calldata.
    ///
    /// # Arguments
    ///
    /// * `data` — replacement calldata bytes.
    ///
    /// # Returns
    ///
    /// The modified [`TradingTransactionParams`] with the new calldata.
    #[must_use]
    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    /// Override the target contract address.
    ///
    /// # Arguments
    ///
    /// * `to` — replacement target contract address.
    ///
    /// # Returns
    ///
    /// The modified [`TradingTransactionParams`] with the new target.
    #[must_use]
    pub const fn with_to(mut self, to: Address) -> Self {
        self.to = to;
        self
    }

    /// Override the gas limit.
    ///
    /// # Arguments
    ///
    /// * `gas_limit` — replacement gas limit.
    ///
    /// # Returns
    ///
    /// The modified [`TradingTransactionParams`] with the new gas limit.
    #[must_use]
    pub const fn with_gas_limit(mut self, gas_limit: u64) -> Self {
        self.gas_limit = gas_limit;
        self
    }

    /// Override the ETH value to send (in wei).
    ///
    /// # Arguments
    ///
    /// * `value` — replacement ETH value in wei.
    ///
    /// # Returns
    ///
    /// The modified [`TradingTransactionParams`] with the new value.
    #[must_use]
    pub const fn with_value(mut self, value: U256) -> Self {
        self.value = value;
        self
    }

    /// Returns the length of the calldata in bytes.
    ///
    /// # Returns
    ///
    /// The byte length of `data`.
    #[must_use]
    pub const fn data_len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the ETH value to send is non-zero.
    ///
    /// # Returns
    ///
    /// `true` when `value` is not `U256::ZERO`.
    #[must_use]
    pub fn has_value(&self) -> bool {
        !self.value.is_zero()
    }
}

impl fmt::Display for TradingTransactionParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tx to={:#x} gas={}", self.to, self.gas_limit)
    }
}

/// Additional parameters for posting orders.
///
/// All fields are optional; the SDK applies sensible defaults when absent.
/// Mirrors `PostTradeAdditionalParams` from the `TypeScript` SDK.
#[derive(Debug, Clone, Default)]
pub struct PostTradeAdditionalParams {
    /// Override for the order signing scheme.
    ///
    /// Defaults to [`EcdsaSigningScheme::Eip712`](cow_types::EcdsaSigningScheme::Eip712)
    /// for EOA wallets.  Use
    /// [`SigningScheme::PreSign`](cow_types::SigningScheme::PreSign) for
    /// smart-contract wallets.
    pub signing_scheme: Option<cow_types::SigningScheme>,
    /// Network gas cost in wei, expressed as a decimal string.
    ///
    /// Used when computing adjusted quote amounts.  Set to `None` to use the
    /// cost embedded in the quote response.
    pub network_costs_amount: Option<String>,
    /// When `Some(false)`, the SDK posts raw caller-supplied amounts without
    /// adjusting for network costs, slippage, or partner fees.
    ///
    /// Defaults to `None` (amounts are adjusted, as for swap orders).
    pub apply_costs_slippage_and_fees: Option<bool>,
    /// Optional protocol-fee rate in basis points sourced from the `/quote`
    /// response (`OrderQuoteResponse::protocol_fee_bps`).
    ///
    /// Required for orders that have BOTH a protocol fee AND a partner fee:
    /// without it, the partner-fee base is computed against the wrong
    /// `before_all_fees` and the final `buy_amount` is overstated.
    /// Mirrors `protocolFeeBps` added to `PostTradeAdditionalParams` in
    /// `cow-sdk` PR #867.
    pub protocol_fee_bps: Option<f64>,
}

impl PostTradeAdditionalParams {
    /// Set the signing scheme override.
    ///
    /// # Arguments
    ///
    /// * `scheme` — the [`SigningScheme`](cow_types::SigningScheme) to use for this order.
    ///
    /// # Returns
    ///
    /// The modified [`PostTradeAdditionalParams`] with the signing scheme set.
    #[must_use]
    pub const fn with_signing_scheme(mut self, scheme: cow_types::SigningScheme) -> Self {
        self.signing_scheme = Some(scheme);
        self
    }

    /// Set the network cost amount override (decimal atom string).
    ///
    /// # Arguments
    ///
    /// * `amount` — network gas cost in wei as a decimal string.
    ///
    /// # Returns
    ///
    /// The modified [`PostTradeAdditionalParams`] with the network cost set.
    #[must_use]
    pub fn with_network_costs_amount(mut self, amount: impl Into<String>) -> Self {
        self.network_costs_amount = Some(amount.into());
        self
    }

    /// Override whether the SDK adjusts amounts for costs, slippage, and fees.
    ///
    /// # Arguments
    ///
    /// * `apply` — `true` to let the SDK adjust amounts; `false` to post raw amounts.
    ///
    /// # Returns
    ///
    /// The modified [`PostTradeAdditionalParams`] with the flag set.
    #[must_use]
    pub const fn with_apply_costs_slippage_and_fees(mut self, apply: bool) -> Self {
        self.apply_costs_slippage_and_fees = Some(apply);
        self
    }

    /// Returns `true` if a signing scheme override is set.
    ///
    /// # Returns
    ///
    /// `true` when `signing_scheme` is `Some`.
    #[must_use]
    pub const fn has_signing_scheme(&self) -> bool {
        self.signing_scheme.is_some()
    }

    /// Returns `true` if a network costs amount override is set.
    ///
    /// # Returns
    ///
    /// `true` when `network_costs_amount` is `Some`.
    #[must_use]
    pub const fn has_network_costs(&self) -> bool {
        self.network_costs_amount.is_some()
    }

    /// Returns `true` if `apply_costs_slippage_and_fees` is explicitly set to `true`.
    ///
    /// # Returns
    ///
    /// `true` only when the inner value is `Some(true)`.
    #[must_use]
    pub const fn should_apply_costs(&self) -> bool {
        matches!(self.apply_costs_slippage_and_fees, Some(true))
    }

    /// Override the protocol-fee rate sourced from the `/quote` response.
    ///
    /// # Arguments
    ///
    /// * `bps` — protocol-fee rate in basis points (may be fractional, e.g. `0.3`).
    ///
    /// # Returns
    ///
    /// The modified [`PostTradeAdditionalParams`] with the protocol-fee rate set.
    #[must_use]
    pub const fn with_protocol_fee_bps(mut self, bps: f64) -> Self {
        self.protocol_fee_bps = Some(bps);
        self
    }

    /// Returns `true` if a protocol-fee rate is set.
    #[must_use]
    pub const fn has_protocol_fee_bps(&self) -> bool {
        self.protocol_fee_bps.is_some()
    }
}

impl fmt::Display for PostTradeAdditionalParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("post-trade-params")
    }
}

/// Advanced overrides for swap and quote operations.
///
/// Mirrors `SwapAdvancedSettings` from the `TypeScript` SDK.
#[derive(Debug, Clone, Default)]
pub struct SwapAdvancedSettings {
    /// Custom app-data fields to merge into the auto-generated document.
    pub app_data: Option<serde_json::Value>,
    /// Override for the slippage tolerance in basis points.
    ///
    /// Takes precedence over [`TradeParameters::slippage_bps`] and the
    /// SDK-level [`crate::TradingSdkConfig::slippage_bps`].
    pub slippage_bps: Option<u32>,
    /// Override for the partner fee.
    ///
    /// Takes precedence over [`TradeParameters::partner_fee`] and the
    /// SDK-level [`crate::TradingSdkConfig::partner_fee`].
    pub partner_fee: Option<cow_app_data::types::PartnerFee>,
}

impl SwapAdvancedSettings {
    /// Set custom app-data fields to merge into the auto-generated document.
    ///
    /// # Arguments
    ///
    /// * `app_data` — JSON value containing custom app-data fields.
    ///
    /// # Returns
    ///
    /// The modified [`SwapAdvancedSettings`] with the app-data set.
    #[must_use]
    pub fn with_app_data(mut self, app_data: serde_json::Value) -> Self {
        self.app_data = Some(app_data);
        self
    }

    /// Override the slippage tolerance in basis points for this swap.
    ///
    /// # Arguments
    ///
    /// * `bps` — slippage tolerance in basis points (e.g. `50` for 0.5%).
    ///
    /// # Returns
    ///
    /// The modified [`SwapAdvancedSettings`] with the slippage override set.
    #[must_use]
    pub const fn with_slippage_bps(mut self, bps: u32) -> Self {
        self.slippage_bps = Some(bps);
        self
    }

    /// Override the partner fee for this swap.
    ///
    /// # Arguments
    ///
    /// * `fee` — the [`PartnerFee`](cow_app_data::types::PartnerFee) to apply.
    ///
    /// # Returns
    ///
    /// The modified [`SwapAdvancedSettings`] with the partner fee set.
    #[must_use]
    pub fn with_partner_fee(mut self, fee: cow_app_data::types::PartnerFee) -> Self {
        self.partner_fee = Some(fee);
        self
    }

    /// Returns `true` if custom app-data fields are set.
    ///
    /// # Returns
    ///
    /// `true` when `app_data` is `Some`.
    #[must_use]
    pub const fn has_app_data(&self) -> bool {
        self.app_data.is_some()
    }

    /// Returns `true` if a slippage tolerance override is set.
    ///
    /// # Returns
    ///
    /// `true` when `slippage_bps` is `Some`.
    #[must_use]
    pub const fn has_slippage_bps(&self) -> bool {
        self.slippage_bps.is_some()
    }

    /// Returns `true` if a partner fee override is set.
    ///
    /// # Returns
    ///
    /// `true` when `partner_fee` is `Some`.
    #[must_use]
    pub const fn has_partner_fee(&self) -> bool {
        self.partner_fee.is_some()
    }
}

impl fmt::Display for SwapAdvancedSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("swap-settings")
    }
}

/// Advanced overrides for limit order submission.
///
/// Applied on top of [`LimitTradeParameters`] via
/// [`apply_settings_to_limit_trade_parameters`].
///
/// Mirrors `LimitOrderAdvancedSettings` from the `TypeScript` SDK.
#[derive(Debug, Clone, Default)]
pub struct LimitOrderAdvancedSettings {
    /// Override for the order receiver.
    pub receiver: Option<Address>,
    /// Absolute order expiry timestamp.  Overrides `valid_for` in the params.
    pub valid_to: Option<u32>,
    /// Partner fee override (replaces any fee set at the config level).
    pub partner_fee: Option<cow_app_data::types::PartnerFee>,
    /// Whether the order may be partially filled.
    pub partially_fillable: Option<bool>,
    /// Pre-computed app-data hash override (`0x`-prefixed `bytes32`).
    pub app_data: Option<String>,
}

impl LimitOrderAdvancedSettings {
    /// Override the order receiver address.
    ///
    /// # Arguments
    ///
    /// * `receiver` — the address that will receive the bought tokens.
    ///
    /// # Returns
    ///
    /// The modified [`LimitOrderAdvancedSettings`] with the receiver set.
    #[must_use]
    pub const fn with_receiver(mut self, receiver: Address) -> Self {
        self.receiver = Some(receiver);
        self
    }

    /// Set an absolute order expiry Unix timestamp.
    ///
    /// # Arguments
    ///
    /// * `valid_to` — Unix timestamp after which the order expires.
    ///
    /// # Returns
    ///
    /// The modified [`LimitOrderAdvancedSettings`] with the expiry set.
    #[must_use]
    pub const fn with_valid_to(mut self, valid_to: u32) -> Self {
        self.valid_to = Some(valid_to);
        self
    }

    /// Override the partner fee for this limit order.
    ///
    /// # Arguments
    ///
    /// * `fee` — the [`PartnerFee`](cow_app_data::types::PartnerFee) to apply.
    ///
    /// # Returns
    ///
    /// The modified [`LimitOrderAdvancedSettings`] with the partner fee set.
    #[must_use]
    pub fn with_partner_fee(mut self, fee: cow_app_data::types::PartnerFee) -> Self {
        self.partner_fee = Some(fee);
        self
    }

    /// Override whether the order may be partially filled.
    ///
    /// # Arguments
    ///
    /// * `partially_fillable` — `true` to allow partial fills.
    ///
    /// # Returns
    ///
    /// The modified [`LimitOrderAdvancedSettings`] with the flag set.
    #[must_use]
    pub const fn with_partially_fillable(mut self, partially_fillable: bool) -> Self {
        self.partially_fillable = Some(partially_fillable);
        self
    }

    /// Override the pre-computed app-data hash (`0x`-prefixed `bytes32`).
    ///
    /// # Arguments
    ///
    /// * `app_data` — `0x`-prefixed 32-byte hex string of the app-data hash.
    ///
    /// # Returns
    ///
    /// The modified [`LimitOrderAdvancedSettings`] with the app-data hash set.
    #[must_use]
    pub fn with_app_data(mut self, app_data: impl Into<String>) -> Self {
        self.app_data = Some(app_data.into());
        self
    }

    /// Returns `true` if a receiver override is set.
    ///
    /// # Returns
    ///
    /// `true` when `receiver` is `Some`.
    #[must_use]
    pub const fn has_receiver(&self) -> bool {
        self.receiver.is_some()
    }

    /// Returns `true` if an absolute expiry timestamp override is set.
    ///
    /// # Returns
    ///
    /// `true` when `valid_to` is `Some`.
    #[must_use]
    pub const fn has_valid_to(&self) -> bool {
        self.valid_to.is_some()
    }

    /// Returns `true` if a partner fee override is set.
    ///
    /// # Returns
    ///
    /// `true` when `partner_fee` is `Some`.
    #[must_use]
    pub const fn has_partner_fee(&self) -> bool {
        self.partner_fee.is_some()
    }

    /// Returns `true` if a partially-fillable override is set.
    ///
    /// # Returns
    ///
    /// `true` when `partially_fillable` is `Some`.
    #[must_use]
    pub const fn has_partially_fillable(&self) -> bool {
        self.partially_fillable.is_some()
    }

    /// Returns `true` if a pre-computed app-data override is set.
    ///
    /// # Returns
    ///
    /// `true` when `app_data` is `Some`.
    #[must_use]
    pub const fn has_app_data(&self) -> bool {
        self.app_data.is_some()
    }
}

impl fmt::Display for LimitOrderAdvancedSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("limit-settings")
    }
}

/// Apply [`LimitOrderAdvancedSettings`] overrides to limit order parameters.
///
/// When `settings` is `None`, `params` is returned unchanged.  Only fields
/// that are `Some` in `settings` replace the corresponding fields in `params`.
///
/// Mirrors `applySettingsToLimitTradeParameters` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `params` — the base limit trade parameters to modify.
/// * `settings` — optional overrides; when `None`, `params` is returned as-is.
///
/// # Returns
///
/// The updated [`LimitTradeParameters`] with any overrides applied.
///
/// # Example
///
/// ```no_run
/// use alloy_primitives::{Address, U256};
/// use cow_trading::{
///     LimitOrderAdvancedSettings, LimitTradeParameters, apply_settings_to_limit_trade_parameters,
/// };
/// use cow_types::OrderKind;
///
/// let params = LimitTradeParameters {
///     kind: OrderKind::Sell,
///     sell_token: Address::ZERO,
///     buy_token: Address::ZERO,
///     sell_amount: U256::from(1_000u32),
///     buy_amount: U256::from(900u32),
///     receiver: None,
///     valid_for: None,
///     valid_to: None,
///     partially_fillable: false,
///     app_data: None,
///     partner_fee: None,
/// };
///
/// let settings = LimitOrderAdvancedSettings {
///     receiver: Some(Address::ZERO),
///     partially_fillable: Some(true),
///     ..LimitOrderAdvancedSettings::default()
/// };
///
/// let updated = apply_settings_to_limit_trade_parameters(params, Some(&settings));
/// assert_eq!(updated.receiver, Some(Address::ZERO));
/// assert!(updated.partially_fillable);
/// ```
#[must_use]
pub fn apply_settings_to_limit_trade_parameters(
    mut params: LimitTradeParameters,
    settings: Option<&LimitOrderAdvancedSettings>,
) -> LimitTradeParameters {
    let Some(s) = settings else {
        return params;
    };
    if s.receiver.is_some() {
        params.receiver = s.receiver;
    }
    if s.valid_to.is_some() {
        params.valid_to = s.valid_to;
    }
    if s.partner_fee.is_some() {
        params.partner_fee = s.partner_fee.clone();
    }
    if let Some(pf) = s.partially_fillable {
        params.partially_fillable = pf;
    }
    if s.app_data.is_some() {
        params.app_data = s.app_data.clone();
    }
    params
}

/// Simplified limit order parameters derived directly from a quote response.
///
/// Mirrors `LimitTradeParametersFromQuote` from the `TypeScript` SDK.
/// Use [`TradingSdk::post_limit_order`](crate::TradingSdk::post_limit_order) with the
/// full [`LimitTradeParameters`] when you need to set receiver, validity, or other options.
#[derive(Debug, Clone)]
pub struct LimitTradeParametersFromQuote {
    /// Token to sell.
    pub sell_token: Address,
    /// Token to buy.
    pub buy_token: Address,
    /// Amount to sell (from quote, in atoms).
    pub sell_amount: U256,
    /// Amount to buy (from quote, in atoms).
    pub buy_amount: U256,
    /// Quote ID returned by the orderbook (for analytics).
    pub quote_id: Option<i64>,
}

impl LimitTradeParametersFromQuote {
    /// Construct from the essential quote fields.
    ///
    /// # Arguments
    ///
    /// * `sell_token` — address of the token to sell.
    /// * `buy_token` — address of the token to buy.
    /// * `sell_amount` — sell amount from the quote (in atoms).
    /// * `buy_amount` — buy amount from the quote (in atoms).
    ///
    /// # Returns
    ///
    /// A new [`LimitTradeParametersFromQuote`] with `quote_id` set to `None`.
    #[must_use]
    pub const fn new(
        sell_token: Address,
        buy_token: Address,
        sell_amount: U256,
        buy_amount: U256,
    ) -> Self {
        Self { sell_token, buy_token, sell_amount, buy_amount, quote_id: None }
    }

    /// Attach a quote ID for analytics.
    ///
    /// # Arguments
    ///
    /// * `quote_id` — the quote identifier returned by the orderbook.
    ///
    /// # Returns
    ///
    /// The modified [`LimitTradeParametersFromQuote`] with the quote ID set.
    #[must_use]
    pub const fn with_quote_id(mut self, quote_id: i64) -> Self {
        self.quote_id = Some(quote_id);
        self
    }

    /// Returns `true` if a quote ID is attached.
    ///
    /// # Returns
    ///
    /// `true` when `quote_id` is `Some`.
    #[must_use]
    pub const fn has_quote_id(&self) -> bool {
        self.quote_id.is_some()
    }
}

impl fmt::Display for LimitTradeParametersFromQuote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "limit-from-quote {:#x} sell={} buy\u{2265}{}",
            self.sell_token, self.sell_amount, self.buy_amount
        )
    }
}

/// Parameters for requesting a swap quote.
#[derive(Debug, Clone)]
pub struct TradeParameters {
    /// Sell or buy direction.
    pub kind: OrderKind,
    /// Token to sell.
    pub sell_token: Address,
    /// Decimal places of `sell_token`.
    pub sell_token_decimals: u8,
    /// Token to buy.
    pub buy_token: Address,
    /// Decimal places of `buy_token`.
    pub buy_token_decimals: u8,
    /// Amount in atoms: sell amount for `kind = Sell`, buy amount for `kind = Buy`.
    pub amount: U256,
    /// Slippage tolerance in basis points.  Defaults to 50 (0.5 %).
    pub slippage_bps: Option<u32>,
    /// Override for the order receiver (defaults to the signer's address).
    pub receiver: Option<Address>,
    /// Relative order TTL in seconds.  Defaults to 1800 (30 min).
    ///
    /// Mutually exclusive with `valid_to`. When both are `Some`, `valid_to` takes precedence.
    pub valid_for: Option<u32>,
    /// Absolute order expiry as a Unix timestamp.
    ///
    /// When set, overrides `valid_for`. Mirrors `TradeOptionalParameters.validTo` from the TS SDK.
    pub valid_to: Option<u32>,
    /// Whether the order may be partially filled.
    ///
    /// Defaults to `false` (fill-or-kill). Mirrors `TradeOptionalParameters.partiallyFillable`.
    pub partially_fillable: Option<bool>,
    /// Per-trade partner fee override.
    ///
    /// When set, this fee policy is embedded in the order's app-data for this trade only,
    /// overriding any partner fee configured at the [`crate::TradingSdkConfig`] level.
    pub partner_fee: Option<cow_app_data::types::PartnerFee>,
}

impl TradeParameters {
    /// Construct a **sell** quote request: sell exactly `amount` of `sell_token`.
    ///
    /// # Arguments
    ///
    /// * `sell_token` — address of the token to sell.
    /// * `sell_token_decimals` — decimal places of `sell_token`.
    /// * `buy_token` — address of the token to buy.
    /// * `buy_token_decimals` — decimal places of `buy_token`.
    /// * `amount` — exact sell amount in atoms.
    ///
    /// # Returns
    ///
    /// A new [`TradeParameters`] configured for a sell order with no optional overrides.
    #[must_use]
    pub const fn sell(
        sell_token: Address,
        sell_token_decimals: u8,
        buy_token: Address,
        buy_token_decimals: u8,
        amount: U256,
    ) -> Self {
        Self {
            kind: OrderKind::Sell,
            sell_token,
            sell_token_decimals,
            buy_token,
            buy_token_decimals,
            amount,
            slippage_bps: None,
            receiver: None,
            valid_for: None,
            valid_to: None,
            partially_fillable: None,
            partner_fee: None,
        }
    }

    /// Construct a **buy** quote request: receive exactly `amount` of `buy_token`.
    ///
    /// # Arguments
    ///
    /// * `sell_token` — address of the token to sell.
    /// * `sell_token_decimals` — decimal places of `sell_token`.
    /// * `buy_token` — address of the token to buy.
    /// * `buy_token_decimals` — decimal places of `buy_token`.
    /// * `amount` — exact buy amount in atoms.
    ///
    /// # Returns
    ///
    /// A new [`TradeParameters`] configured for a buy order with no optional overrides.
    #[must_use]
    pub const fn buy(
        sell_token: Address,
        sell_token_decimals: u8,
        buy_token: Address,
        buy_token_decimals: u8,
        amount: U256,
    ) -> Self {
        Self {
            kind: OrderKind::Buy,
            sell_token,
            sell_token_decimals,
            buy_token,
            buy_token_decimals,
            amount,
            slippage_bps: None,
            receiver: None,
            valid_for: None,
            valid_to: None,
            partially_fillable: None,
            partner_fee: None,
        }
    }

    /// Override the slippage tolerance in basis points.
    ///
    /// # Arguments
    ///
    /// * `bps` — slippage tolerance in basis points (e.g. `50` for 0.5%).
    ///
    /// # Returns
    ///
    /// The modified [`TradeParameters`] with the slippage override set.
    #[must_use]
    pub const fn with_slippage_bps(mut self, bps: u32) -> Self {
        self.slippage_bps = Some(bps);
        self
    }

    /// Override the order receiver.
    ///
    /// # Arguments
    ///
    /// * `receiver` — the address that will receive the bought tokens.
    ///
    /// # Returns
    ///
    /// The modified [`TradeParameters`] with the receiver set.
    #[must_use]
    pub const fn with_receiver(mut self, receiver: Address) -> Self {
        self.receiver = Some(receiver);
        self
    }

    /// Set a relative validity window in seconds.
    ///
    /// # Arguments
    ///
    /// * `secs` — order time-to-live in seconds.
    ///
    /// # Returns
    ///
    /// The modified [`TradeParameters`] with the validity window set.
    #[must_use]
    pub const fn with_valid_for(mut self, secs: u32) -> Self {
        self.valid_for = Some(secs);
        self
    }

    /// Set an absolute expiry Unix timestamp.
    ///
    /// # Arguments
    ///
    /// * `ts` — Unix timestamp after which the order expires.
    ///
    /// # Returns
    ///
    /// The modified [`TradeParameters`] with the expiry set.
    #[must_use]
    pub const fn with_valid_to(mut self, ts: u32) -> Self {
        self.valid_to = Some(ts);
        self
    }

    /// Allow partial fills.
    ///
    /// # Returns
    ///
    /// The modified [`TradeParameters`] with `partially_fillable` set to `true`.
    #[must_use]
    pub const fn with_partially_fillable(mut self) -> Self {
        self.partially_fillable = Some(true);
        self
    }

    /// Returns `true` if this is a sell-direction trade.
    ///
    /// # Returns
    ///
    /// `true` when `kind` is [`OrderKind::Sell`].
    #[must_use]
    pub const fn is_sell(&self) -> bool {
        self.kind.is_sell()
    }

    /// Returns `true` if this is a buy-direction trade.
    ///
    /// # Returns
    ///
    /// `true` when `kind` is [`OrderKind::Buy`].
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        self.kind.is_buy()
    }

    /// Returns `true` if a slippage tolerance override is set.
    ///
    /// # Returns
    ///
    /// `true` when `slippage_bps` is `Some`.
    #[must_use]
    pub const fn has_slippage_bps(&self) -> bool {
        self.slippage_bps.is_some()
    }

    /// Returns `true` if a receiver override is set.
    ///
    /// # Returns
    ///
    /// `true` when `receiver` is `Some`.
    #[must_use]
    pub const fn has_receiver(&self) -> bool {
        self.receiver.is_some()
    }

    /// Returns `true` if a partner fee override is set.
    ///
    /// # Returns
    ///
    /// `true` when `partner_fee` is `Some`.
    #[must_use]
    pub const fn has_partner_fee(&self) -> bool {
        self.partner_fee.is_some()
    }
}

impl fmt::Display for TradeParameters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {:#x} \u{2192} {:#x} amt={}",
            self.kind, self.sell_token, self.buy_token, self.amount
        )
    }
}

impl LimitTradeParameters {
    /// Construct a limit **sell** order: sell exactly `sell_amount`, receive at least `buy_amount`.
    ///
    /// # Arguments
    ///
    /// * `sell_token` — address of the token to sell.
    /// * `buy_token` — address of the token to buy.
    /// * `sell_amount` — exact sell amount in atoms.
    /// * `buy_amount` — minimum buy amount in atoms.
    ///
    /// # Returns
    ///
    /// A new [`LimitTradeParameters`] configured for a sell limit order.
    #[must_use]
    pub const fn sell(
        sell_token: Address,
        buy_token: Address,
        sell_amount: U256,
        buy_amount: U256,
    ) -> Self {
        Self {
            kind: cow_types::OrderKind::Sell,
            sell_token,
            buy_token,
            sell_amount,
            buy_amount,
            receiver: None,
            valid_for: None,
            valid_to: None,
            partially_fillable: false,
            app_data: None,
            partner_fee: None,
        }
    }

    /// Construct a limit **buy** order: receive exactly `buy_amount`, spend at most `sell_amount`.
    ///
    /// # Arguments
    ///
    /// * `sell_token` — address of the token to sell.
    /// * `buy_token` — address of the token to buy.
    /// * `sell_amount` — maximum sell amount in atoms.
    /// * `buy_amount` — exact buy amount in atoms.
    ///
    /// # Returns
    ///
    /// A new [`LimitTradeParameters`] configured for a buy limit order.
    #[must_use]
    pub const fn buy(
        sell_token: Address,
        buy_token: Address,
        sell_amount: U256,
        buy_amount: U256,
    ) -> Self {
        Self {
            kind: cow_types::OrderKind::Buy,
            sell_token,
            buy_token,
            sell_amount,
            buy_amount,
            receiver: None,
            valid_for: None,
            valid_to: None,
            partially_fillable: false,
            app_data: None,
            partner_fee: None,
        }
    }

    /// Override the order receiver.
    ///
    /// # Arguments
    ///
    /// * `receiver` — the address that will receive the bought tokens.
    ///
    /// # Returns
    ///
    /// The modified [`LimitTradeParameters`] with the receiver set.
    #[must_use]
    pub const fn with_receiver(mut self, receiver: Address) -> Self {
        self.receiver = Some(receiver);
        self
    }

    /// Set a relative validity window in seconds.
    ///
    /// # Arguments
    ///
    /// * `secs` — order time-to-live in seconds.
    ///
    /// # Returns
    ///
    /// The modified [`LimitTradeParameters`] with the validity window set.
    #[must_use]
    pub const fn with_valid_for(mut self, secs: u32) -> Self {
        self.valid_for = Some(secs);
        self
    }

    /// Set an absolute expiry Unix timestamp.
    ///
    /// # Arguments
    ///
    /// * `ts` — Unix timestamp after which the order expires.
    ///
    /// # Returns
    ///
    /// The modified [`LimitTradeParameters`] with the expiry set.
    #[must_use]
    pub const fn with_valid_to(mut self, ts: u32) -> Self {
        self.valid_to = Some(ts);
        self
    }

    /// Allow partial fills.
    ///
    /// # Returns
    ///
    /// The modified [`LimitTradeParameters`] with `partially_fillable` set to `true`.
    #[must_use]
    pub const fn with_partially_fillable(mut self) -> Self {
        self.partially_fillable = true;
        self
    }

    /// Returns `true` if this is a sell-direction limit order.
    ///
    /// # Returns
    ///
    /// `true` when `kind` is [`OrderKind::Sell`].
    #[must_use]
    pub const fn is_sell(&self) -> bool {
        self.kind.is_sell()
    }

    /// Returns `true` if this is a buy-direction limit order.
    ///
    /// # Returns
    ///
    /// `true` when `kind` is [`OrderKind::Buy`].
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        self.kind.is_buy()
    }

    /// Returns `true` if a receiver override is set.
    ///
    /// # Returns
    ///
    /// `true` when `receiver` is `Some`.
    #[must_use]
    pub const fn has_receiver(&self) -> bool {
        self.receiver.is_some()
    }

    /// Returns `true` if an absolute expiry timestamp override is set.
    ///
    /// # Returns
    ///
    /// `true` when `valid_to` is `Some`.
    #[must_use]
    pub const fn has_valid_to(&self) -> bool {
        self.valid_to.is_some()
    }

    /// Returns `true` if a relative validity window is set.
    ///
    /// # Returns
    ///
    /// `true` when `valid_for` is `Some`.
    #[must_use]
    pub const fn has_valid_for(&self) -> bool {
        self.valid_for.is_some()
    }

    /// Returns `true` if a pre-computed app-data override is set.
    ///
    /// # Returns
    ///
    /// `true` when `app_data` is `Some`.
    #[must_use]
    pub const fn has_app_data(&self) -> bool {
        self.app_data.is_some()
    }

    /// Returns `true` if a partner fee override is set.
    ///
    /// # Returns
    ///
    /// `true` when `partner_fee` is `Some`.
    #[must_use]
    pub const fn has_partner_fee(&self) -> bool {
        self.partner_fee.is_some()
    }
}

impl fmt::Display for LimitTradeParameters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "limit {} {:#x} sell={} buy\u{2265}{}",
            self.kind, self.sell_token, self.sell_amount, self.buy_amount
        )
    }
}

impl QuoteAmountsAndCosts {
    /// Returns `true` for buy orders.
    ///
    /// This is the logical complement of the `is_sell` field.
    ///
    /// # Returns
    ///
    /// `true` when `is_sell` is `false`.
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        !self.is_sell
    }

    /// Slippage tolerance in buy-token atoms.
    ///
    /// Computed as the difference between the buy amount before slippage is
    /// applied and the final after-slippage buy amount. Returns `U256::ZERO`
    /// when `after_partner_fees.buy_amount < after_slippage.buy_amount`
    /// (saturating subtract prevents underflow).
    ///
    /// # Returns
    ///
    /// The maximum slippage as a `U256` amount in buy-token atoms.
    #[must_use]
    pub const fn max_slippage_atoms(&self) -> U256 {
        self.after_partner_fees.buy_amount.saturating_sub(self.after_slippage.buy_amount)
    }

    /// Total fees in sell-token atoms (network + partner + protocol).
    ///
    /// # Returns
    ///
    /// The saturating sum of network, partner, and protocol fee amounts.
    #[must_use]
    pub const fn total_fees_atoms(&self) -> U256 {
        self.network_fee
            .amount_in_sell_currency
            .saturating_add(self.partner_fee.amount)
            .saturating_add(self.protocol_fee.amount)
    }

    /// Returns `true` if both network fee components are non-zero.
    ///
    /// # Returns
    ///
    /// `true` when at least one component of `network_fee` is non-zero.
    #[must_use]
    pub fn has_network_fee(&self) -> bool {
        !self.network_fee.is_zero()
    }

    /// Returns `true` if the partner fee is non-zero.
    ///
    /// # Returns
    ///
    /// `true` when the partner fee amount or bps is non-zero.
    #[must_use]
    pub fn has_partner_fee(&self) -> bool {
        !self.partner_fee.is_zero()
    }

    /// Returns `true` if the protocol fee is non-zero.
    ///
    /// # Returns
    ///
    /// `true` when the protocol fee amount or bps is non-zero.
    #[must_use]
    pub fn has_protocol_fee(&self) -> bool {
        !self.protocol_fee.is_zero()
    }
}

impl fmt::Display for QuoteAmountsAndCosts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dir = if self.is_sell { "sell" } else { "buy" };
        write!(
            f,
            "{dir} gross={} after-slippage={} [{} / {} / {}]",
            self.before_all_fees,
            self.after_slippage,
            self.network_fee,
            self.partner_fee,
            self.protocol_fee,
        )
    }
}

/// Apply a mapping function to every amount in a [`QuoteAmountsAndCosts`].
///
/// Useful for converting between token denominators or scaling amounts.
/// Mirrors `mapQuoteAmountsAndCosts` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `costs` — the original quote amounts and costs to transform.
/// * `f` — a closure applied to each `U256` amount field; fee `bps` values are preserved.
///
/// # Returns
///
/// A new [`QuoteAmountsAndCosts`] with every amount field mapped through `f`.
///
/// # Example
///
/// ```
/// use alloy_primitives::U256;
/// use cow_trading::{
///     Amounts, NetworkFee, PartnerFeeCost, ProtocolFeeCost, QuoteAmountsAndCosts,
///     map_quote_amounts_and_costs,
/// };
///
/// let costs = QuoteAmountsAndCosts {
///     is_sell: true,
///     before_all_fees: Amounts {
///         sell_amount: U256::from(200u32),
///         buy_amount: U256::from(110u32),
///     },
///     before_network_costs: Amounts {
///         sell_amount: U256::from(200u32),
///         buy_amount: U256::from(100u32),
///     },
///     after_network_costs: Amounts {
///         sell_amount: U256::from(190u32),
///         buy_amount: U256::from(100u32),
///     },
///     after_partner_fees: Amounts {
///         sell_amount: U256::from(190u32),
///         buy_amount: U256::from(100u32),
///     },
///     after_slippage: Amounts { sell_amount: U256::from(190u32), buy_amount: U256::from(95u32) },
///     network_fee: NetworkFee {
///         amount_in_sell_currency: U256::from(10u32),
///         amount_in_buy_currency: U256::ZERO,
///     },
///     partner_fee: PartnerFeeCost { amount: U256::ZERO, bps: 0 },
///     protocol_fee: ProtocolFeeCost { amount: U256::ZERO, bps: 0 },
/// };
///
/// // Double all amounts
/// let doubled = map_quote_amounts_and_costs(&costs, |a| a * U256::from(2u32));
/// assert_eq!(doubled.before_network_costs.sell_amount, U256::from(400u32));
/// ```
#[must_use]
pub fn map_quote_amounts_and_costs<F>(
    costs: &QuoteAmountsAndCosts,
    mut f: F,
) -> QuoteAmountsAndCosts
where
    F: FnMut(U256) -> U256,
{
    QuoteAmountsAndCosts {
        is_sell: costs.is_sell,
        before_all_fees: Amounts {
            sell_amount: f(costs.before_all_fees.sell_amount),
            buy_amount: f(costs.before_all_fees.buy_amount),
        },
        before_network_costs: Amounts {
            sell_amount: f(costs.before_network_costs.sell_amount),
            buy_amount: f(costs.before_network_costs.buy_amount),
        },
        after_network_costs: Amounts {
            sell_amount: f(costs.after_network_costs.sell_amount),
            buy_amount: f(costs.after_network_costs.buy_amount),
        },
        after_partner_fees: Amounts {
            sell_amount: f(costs.after_partner_fees.sell_amount),
            buy_amount: f(costs.after_partner_fees.buy_amount),
        },
        after_slippage: Amounts {
            sell_amount: f(costs.after_slippage.sell_amount),
            buy_amount: f(costs.after_slippage.buy_amount),
        },
        network_fee: NetworkFee {
            amount_in_sell_currency: f(costs.network_fee.amount_in_sell_currency),
            amount_in_buy_currency: f(costs.network_fee.amount_in_buy_currency),
        },
        partner_fee: PartnerFeeCost {
            amount: f(costs.partner_fee.amount),
            bps: costs.partner_fee.bps,
        },
        protocol_fee: ProtocolFeeCost {
            amount: f(costs.protocol_fee.amount),
            bps: costs.protocol_fee.bps,
        },
    }
}

/// Parameters for a limit order (fixed price, no slippage).
///
/// Unlike [`TradeParameters`], the amounts here are exact — the order is
/// submitted as-is without slippage adjustment. Use `TradingSdk::post_limit_order`
/// to sign and submit.
#[derive(Debug, Clone)]
pub struct LimitTradeParameters {
    /// Sell or buy direction.
    pub kind: cow_types::OrderKind,
    /// Token to sell.
    pub sell_token: Address,
    /// Token to buy.
    pub buy_token: Address,
    /// Amount to sell (exact, in atoms, for `kind = Sell`).
    pub sell_amount: U256,
    /// Amount to buy (minimum, in atoms, for `kind = Sell`; exact for `kind = Buy`).
    pub buy_amount: U256,
    /// Override for the order receiver (defaults to the signer's address).
    pub receiver: Option<Address>,
    /// Order TTL in seconds. Defaults to [`super::sdk::DEFAULT_QUOTE_VALIDITY`].
    ///
    /// Ignored when `valid_to` is `Some`.
    pub valid_for: Option<u32>,
    /// Absolute order expiry as a Unix timestamp.
    ///
    /// When set, overrides `valid_for`.
    pub valid_to: Option<u32>,
    /// Whether the order may be partially filled.
    pub partially_fillable: bool,
    /// Pre-computed app-data hash (hex, `0x`-prefixed `bytes32`).
    /// Uses `0x000…0` when `None`.
    pub app_data: Option<String>,
    /// Per-trade partner fee override.
    ///
    /// When set, replaces any partner fee configured at the [`crate::TradingSdkConfig`] level.
    pub partner_fee: Option<cow_app_data::types::PartnerFee>,
}

/// The result of a successful order submission.
///
/// Mirrors `OrderPostingResult` from the `TypeScript` SDK — bundles the order
/// UID with the signing details used to place it.
#[derive(Debug, Clone)]
pub struct OrderPostingResult {
    /// The unique order identifier returned by `POST /api/v1/orders`.
    pub order_id: String,
    /// The signing scheme used.
    pub signing_scheme: cow_types::SigningScheme,
    /// Hex-encoded signature (format depends on `signing_scheme`).
    pub signature: String,
    /// The order struct that was signed.
    pub order_to_sign: UnsignedOrder,
}

impl OrderPostingResult {
    /// Construct an [`OrderPostingResult`] from the four fields returned by order submission.
    ///
    /// # Arguments
    ///
    /// * `order_id` — the unique order identifier from `POST /api/v1/orders`.
    /// * `signing_scheme` — the [`SigningScheme`](cow_types::SigningScheme) used.
    /// * `signature` — hex-encoded signature string.
    /// * `order_to_sign` — the order struct that was signed.
    ///
    /// # Returns
    ///
    /// A new [`OrderPostingResult`] instance.
    #[must_use]
    pub fn new(
        order_id: impl Into<String>,
        signing_scheme: cow_types::SigningScheme,
        signature: impl Into<String>,
        order_to_sign: UnsignedOrder,
    ) -> Self {
        Self {
            order_id: order_id.into(),
            signing_scheme,
            signature: signature.into(),
            order_to_sign,
        }
    }

    /// Returns `true` if the order was signed with `EIP-712`.
    ///
    /// # Returns
    ///
    /// `true` when `signing_scheme` is
    /// [`SigningScheme::Eip712`](cow_types::SigningScheme::Eip712).
    #[must_use]
    pub const fn is_eip712(&self) -> bool {
        matches!(self.signing_scheme, cow_types::SigningScheme::Eip712)
    }

    /// Returns `true` if the order was signed with `eth_sign` (`EIP-191`).
    ///
    /// # Returns
    ///
    /// `true` when `signing_scheme` is
    /// [`SigningScheme::EthSign`](cow_types::SigningScheme::EthSign).
    #[must_use]
    pub const fn is_eth_sign(&self) -> bool {
        matches!(self.signing_scheme, cow_types::SigningScheme::EthSign)
    }

    /// Returns `true` if the order was signed with `EIP-1271` (smart-contract signature).
    ///
    /// # Returns
    ///
    /// `true` when `signing_scheme` is
    /// [`SigningScheme::Eip1271`](cow_types::SigningScheme::Eip1271).
    #[must_use]
    pub const fn is_eip1271(&self) -> bool {
        matches!(self.signing_scheme, cow_types::SigningScheme::Eip1271)
    }

    /// Returns `true` if the order uses a pre-signature (on-chain sign-later flow).
    ///
    /// # Returns
    ///
    /// `true` when `signing_scheme` is
    /// [`SigningScheme::PreSign`](cow_types::SigningScheme::PreSign).
    #[must_use]
    pub const fn is_presign(&self) -> bool {
        matches!(self.signing_scheme, cow_types::SigningScheme::PreSign)
    }

    /// Returns the order UID string.
    ///
    /// # Returns
    ///
    /// A `&str` reference to the order identifier.
    #[must_use]
    pub fn order_id_ref(&self) -> &str {
        &self.order_id
    }

    /// Returns the hex-encoded signature string.
    ///
    /// # Returns
    ///
    /// A `&str` reference to the hex-encoded signature.
    #[must_use]
    pub fn signature_ref(&self) -> &str {
        &self.signature
    }
}

impl fmt::Display for OrderPostingResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "order({})", self.order_id)
    }
}

impl fmt::Display for QuoteResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "quote slippage={}bps {}", self.suggested_slippage_bps, self.amounts_and_costs)
    }
}

/// The result of a successful quote, bundled with posting capability.
#[derive(Debug, Clone)]
pub struct QuoteResults {
    /// The order struct ready to be signed and submitted.
    pub order_to_sign: UnsignedOrder,
    /// Full EIP-712 typed-data envelope for the order.
    ///
    /// Pass this to any EIP-712-aware signer (hardware wallet, `eth_signTypedData_v4`)
    /// that needs the structured domain and types alongside the message.
    /// Mirrors `QuoteResults.orderTypedData` from the `TypeScript` SDK.
    pub order_typed_data: OrderTypedData,
    /// Raw quote response from the orderbook.
    pub quote_response: OrderQuoteResponse,
    /// Detailed fee and amount breakdown.
    pub amounts_and_costs: QuoteAmountsAndCosts,
    /// Suggested slippage (may differ from the requested value).
    pub suggested_slippage_bps: u32,
    /// Full app-data document and its `keccak256` hash.
    pub app_data_info: TradingAppDataInfo,
}

impl QuoteResults {
    /// Returns a reference to the order ready for signing.
    ///
    /// # Returns
    ///
    /// A `&UnsignedOrder` reference to the order struct.
    #[must_use]
    pub const fn order_ref(&self) -> &UnsignedOrder {
        &self.order_to_sign
    }

    /// Returns a reference to the raw quote response from the orderbook.
    ///
    /// # Returns
    ///
    /// A `&OrderQuoteResponse` reference to the raw API response.
    #[must_use]
    pub const fn quote_ref(&self) -> &OrderQuoteResponse {
        &self.quote_response
    }
}

// ── BuildAppDataParams ───────────────────────────────────────────────────────

/// Parameters for building the app-data document for a trade.
///
/// Mirrors `BuildAppDataParams` from the `TypeScript` SDK.
#[derive(Debug, Clone)]
pub struct BuildAppDataParams {
    /// Application code identifying the dApp (e.g. `"CoW Swap"`).
    pub app_code: String,
    /// Slippage tolerance in basis points.
    pub slippage_bps: u32,
    /// Order class classification.
    pub order_class: cow_app_data::types::OrderClassKind,
    /// Optional partner fee to embed in the app-data.
    pub partner_fee: Option<cow_app_data::types::PartnerFee>,
}

impl BuildAppDataParams {
    /// Construct a [`BuildAppDataParams`] with the required fields.
    ///
    /// # Arguments
    ///
    /// * `app_code` — application code identifying the dApp (e.g. `"CoW Swap"`).
    /// * `slippage_bps` — slippage tolerance in basis points.
    /// * `order_class` — order class classification for the app-data document.
    ///
    /// # Returns
    ///
    /// A new [`BuildAppDataParams`] with `partner_fee` set to `None`.
    #[must_use]
    pub fn new(
        app_code: impl Into<String>,
        slippage_bps: u32,
        order_class: cow_app_data::types::OrderClassKind,
    ) -> Self {
        Self { app_code: app_code.into(), slippage_bps, order_class, partner_fee: None }
    }

    /// Attach a partner fee to embed in the app-data.
    ///
    /// # Arguments
    ///
    /// * `fee` — the [`PartnerFee`](cow_app_data::types::PartnerFee) to embed.
    ///
    /// # Returns
    ///
    /// The modified [`BuildAppDataParams`] with the partner fee set.
    #[must_use]
    pub fn with_partner_fee(mut self, fee: cow_app_data::types::PartnerFee) -> Self {
        self.partner_fee = Some(fee);
        self
    }

    /// Returns `true` if a partner fee is set.
    ///
    /// # Returns
    ///
    /// `true` when `partner_fee` is `Some`.
    #[must_use]
    pub const fn has_partner_fee(&self) -> bool {
        self.partner_fee.is_some()
    }
}

impl fmt::Display for BuildAppDataParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "build-app-data({}, {}bps, {})",
            self.app_code, self.slippage_bps, self.order_class
        )
    }
}

// ── SlippageTolerance request/response ───────────────────────────────────────

/// Request parameters for a slippage tolerance suggestion.
///
/// Mirrors `SlippageToleranceRequest` from the `TypeScript` SDK.
#[derive(Debug, Clone)]
pub struct SlippageToleranceRequest {
    /// Chain ID on which the trade will execute.
    pub chain_id: u64,
    /// Token to sell.
    pub sell_token: Address,
    /// Token to buy.
    pub buy_token: Address,
    /// Sell amount in atoms (optional; improves suggestion accuracy).
    pub sell_amount: Option<U256>,
    /// Buy amount in atoms (optional; improves suggestion accuracy).
    pub buy_amount: Option<U256>,
}

impl SlippageToleranceRequest {
    /// Construct a [`SlippageToleranceRequest`] with the required fields.
    ///
    /// # Arguments
    ///
    /// * `chain_id` — chain ID on which the trade will execute.
    /// * `sell_token` — address of the token to sell.
    /// * `buy_token` — address of the token to buy.
    ///
    /// # Returns
    ///
    /// A new [`SlippageToleranceRequest`] with optional amounts set to `None`.
    #[must_use]
    pub const fn new(chain_id: u64, sell_token: Address, buy_token: Address) -> Self {
        Self { chain_id, sell_token, buy_token, sell_amount: None, buy_amount: None }
    }

    /// Attach a sell amount to improve the suggestion.
    ///
    /// # Arguments
    ///
    /// * `amount` — sell amount in atoms.
    ///
    /// # Returns
    ///
    /// The modified [`SlippageToleranceRequest`] with the sell amount set.
    #[must_use]
    pub const fn with_sell_amount(mut self, amount: U256) -> Self {
        self.sell_amount = Some(amount);
        self
    }

    /// Attach a buy amount to improve the suggestion.
    ///
    /// # Arguments
    ///
    /// * `amount` — buy amount in atoms.
    ///
    /// # Returns
    ///
    /// The modified [`SlippageToleranceRequest`] with the buy amount set.
    #[must_use]
    pub const fn with_buy_amount(mut self, amount: U256) -> Self {
        self.buy_amount = Some(amount);
        self
    }
}

impl fmt::Display for SlippageToleranceRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "slippage-req(chain={}, {:#x} -> {:#x})",
            self.chain_id, self.sell_token, self.buy_token
        )
    }
}

/// Response from a slippage tolerance suggestion.
///
/// Mirrors `SlippageToleranceResponse` from the `TypeScript` SDK.
#[derive(Debug, Clone)]
pub struct SlippageToleranceResponse {
    /// Suggested slippage in basis points, or `None` if no suggestion is available.
    pub slippage_bps: Option<u32>,
}

impl SlippageToleranceResponse {
    /// Construct a [`SlippageToleranceResponse`] with a suggested slippage value.
    ///
    /// # Arguments
    ///
    /// * `slippage_bps` — suggested slippage in basis points, or `None` if unavailable.
    ///
    /// # Returns
    ///
    /// A new [`SlippageToleranceResponse`] instance.
    #[must_use]
    pub const fn new(slippage_bps: Option<u32>) -> Self {
        Self { slippage_bps }
    }

    /// Returns `true` if a slippage suggestion is available.
    ///
    /// # Returns
    ///
    /// `true` when `slippage_bps` is `Some`.
    #[must_use]
    pub const fn has_suggestion(&self) -> bool {
        self.slippage_bps.is_some()
    }
}

impl fmt::Display for SlippageToleranceResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.slippage_bps {
            Some(bps) => write!(f, "slippage-resp({bps}bps)"),
            None => f.write_str("slippage-resp(none)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloy_primitives::B256;

    use cow_app_data::types::{OrderClassKind, PartnerFee, PartnerFeeEntry};
    use cow_types::{SigningScheme, TokenBalance};

    // ── Amounts ─────────────────────────────────────────────────────────────

    #[test]
    fn amounts_new_stores_fields() {
        let a = Amounts::new(U256::from(100u32), U256::from(200u32));
        assert_eq!(a.sell_amount, U256::from(100u32));
        assert_eq!(a.buy_amount, U256::from(200u32));
    }

    #[test]
    fn amounts_is_zero() {
        assert!(Amounts::default().is_zero());
        assert!(Amounts::new(U256::ZERO, U256::ZERO).is_zero());
        assert!(!Amounts::new(U256::from(1u32), U256::ZERO).is_zero());
        assert!(!Amounts::new(U256::ZERO, U256::from(1u32)).is_zero());
    }

    #[test]
    fn amounts_total() {
        let a = Amounts::new(U256::from(100u32), U256::from(90u32));
        assert_eq!(a.total(), U256::from(190u32));
    }

    #[test]
    fn amounts_total_saturates() {
        let a = Amounts::new(U256::MAX, U256::from(1u32));
        assert_eq!(a.total(), U256::MAX);
    }

    #[test]
    fn amounts_display() {
        let a = Amounts::new(U256::from(42u32), U256::from(7u32));
        let s = format!("{a}");
        assert!(s.contains("sell"));
        assert!(s.contains("buy"));
        assert!(s.contains("42"));
        assert!(s.contains('7'));
    }

    #[test]
    fn amounts_default() {
        let a = Amounts::default();
        assert_eq!(a.sell_amount, U256::ZERO);
        assert_eq!(a.buy_amount, U256::ZERO);
    }

    // ── NetworkFee ──────────────────────────────────────────────────────────

    #[test]
    fn network_fee_new_stores_fields() {
        let nf = NetworkFee::new(U256::from(10u32), U256::from(20u32));
        assert_eq!(nf.amount_in_sell_currency, U256::from(10u32));
        assert_eq!(nf.amount_in_buy_currency, U256::from(20u32));
    }

    #[test]
    fn network_fee_is_zero() {
        assert!(NetworkFee::default().is_zero());
        assert!(!NetworkFee::new(U256::from(1u32), U256::ZERO).is_zero());
        assert!(!NetworkFee::new(U256::ZERO, U256::from(1u32)).is_zero());
    }

    #[test]
    fn network_fee_total_atoms() {
        let nf = NetworkFee::new(U256::from(5u32), U256::from(3u32));
        assert_eq!(nf.total_atoms(), U256::from(8u32));
    }

    #[test]
    fn network_fee_total_atoms_saturates() {
        let nf = NetworkFee::new(U256::MAX, U256::from(1u32));
        assert_eq!(nf.total_atoms(), U256::MAX);
    }

    #[test]
    fn network_fee_display() {
        let nf = NetworkFee::new(U256::from(10u32), U256::from(20u32));
        let s = format!("{nf}");
        assert!(s.contains("network-fee"));
        assert!(s.contains("10"));
        assert!(s.contains("20"));
    }

    #[test]
    fn network_fee_default() {
        let nf = NetworkFee::default();
        assert_eq!(nf.amount_in_sell_currency, U256::ZERO);
        assert_eq!(nf.amount_in_buy_currency, U256::ZERO);
    }

    // ── PartnerFeeCost ──────────────────────────────────────────────────────

    #[test]
    fn partner_fee_cost_new() {
        let pf = PartnerFeeCost::new(U256::from(500u32), 50);
        assert_eq!(pf.amount, U256::from(500u32));
        assert_eq!(pf.bps, 50);
    }

    #[test]
    fn partner_fee_cost_is_zero() {
        assert!(PartnerFeeCost::default().is_zero());
        assert!(!PartnerFeeCost::new(U256::from(1u32), 0).is_zero());
        assert!(!PartnerFeeCost::new(U256::ZERO, 1).is_zero());
    }

    #[test]
    fn partner_fee_cost_has_bps() {
        assert!(!PartnerFeeCost::default().has_bps());
        assert!(PartnerFeeCost::new(U256::ZERO, 10).has_bps());
    }

    #[test]
    fn partner_fee_cost_display() {
        let pf = PartnerFeeCost::new(U256::from(99u32), 25);
        let s = format!("{pf}");
        assert!(s.contains("partner-fee"));
        assert!(s.contains("25bps"));
    }

    #[test]
    fn partner_fee_cost_default() {
        let pf = PartnerFeeCost::default();
        assert_eq!(pf.amount, U256::ZERO);
        assert_eq!(pf.bps, 0);
    }

    // ── ProtocolFeeCost ─────────────────────────────────────────────────────

    #[test]
    fn protocol_fee_cost_new() {
        let pf = ProtocolFeeCost::new(U256::from(300u32), 15);
        assert_eq!(pf.amount, U256::from(300u32));
        assert_eq!(pf.bps, 15);
    }

    #[test]
    fn protocol_fee_cost_is_zero() {
        assert!(ProtocolFeeCost::default().is_zero());
        assert!(!ProtocolFeeCost::new(U256::from(1u32), 0).is_zero());
        assert!(!ProtocolFeeCost::new(U256::ZERO, 1).is_zero());
    }

    #[test]
    fn protocol_fee_cost_has_bps() {
        assert!(!ProtocolFeeCost::default().has_bps());
        assert!(ProtocolFeeCost::new(U256::ZERO, 5).has_bps());
    }

    #[test]
    fn protocol_fee_cost_display() {
        let pf = ProtocolFeeCost::new(U256::from(77u32), 10);
        let s = format!("{pf}");
        assert!(s.contains("protocol-fee"));
        assert!(s.contains("10bps"));
    }

    #[test]
    fn protocol_fee_cost_default() {
        let pf = ProtocolFeeCost::default();
        assert_eq!(pf.amount, U256::ZERO);
        assert_eq!(pf.bps, 0);
    }

    // ── TradingAppDataInfo ──────────────────────────────────────────────────

    #[test]
    fn trading_app_data_info_new() {
        let info = TradingAppDataInfo::new("{\"v\":1}", "0xabc");
        assert_eq!(info.full_app_data, "{\"v\":1}");
        assert_eq!(info.app_data_keccak256, "0xabc");
    }

    #[test]
    fn trading_app_data_info_has_full_app_data() {
        let with = TradingAppDataInfo::new("{}", "0x1");
        assert!(with.has_full_app_data());

        let without = TradingAppDataInfo::new("", "0x1");
        assert!(!without.has_full_app_data());
    }

    #[test]
    fn trading_app_data_info_refs() {
        let info = TradingAppDataInfo::new("doc", "0xhash");
        assert_eq!(info.full_app_data_ref(), "doc");
        assert_eq!(info.keccak256_ref(), "0xhash");
    }

    #[test]
    fn trading_app_data_info_display() {
        let info = TradingAppDataInfo::new("{}", "0xdeadbeef");
        let s = format!("{info}");
        assert!(s.contains("app-data"));
        assert!(s.contains("0xdeadbeef"));
    }

    // ── TradingTransactionParams ────────────────────────────────────────────

    #[test]
    fn trading_tx_params_new() {
        let data = vec![0xAA, 0xBB];
        let to = Address::ZERO;
        let tx = TradingTransactionParams::new(data.clone(), to, 21_000, U256::from(1u32));
        assert_eq!(tx.data, data);
        assert_eq!(tx.to, to);
        assert_eq!(tx.gas_limit, 21_000);
        assert_eq!(tx.value, U256::from(1u32));
    }

    #[test]
    fn trading_tx_params_builders() {
        let tx = TradingTransactionParams::new(vec![], Address::ZERO, 0, U256::ZERO)
            .with_data(vec![1, 2, 3])
            .with_to(Address::with_last_byte(0x01))
            .with_gas_limit(50_000)
            .with_value(U256::from(999u32));

        assert_eq!(tx.data, vec![1, 2, 3]);
        assert_eq!(tx.to, Address::with_last_byte(0x01));
        assert_eq!(tx.gas_limit, 50_000);
        assert_eq!(tx.value, U256::from(999u32));
    }

    #[test]
    fn trading_tx_params_data_len() {
        let tx = TradingTransactionParams::new(vec![0; 64], Address::ZERO, 0, U256::ZERO);
        assert_eq!(tx.data_len(), 64);
    }

    #[test]
    fn trading_tx_params_has_value() {
        let no_val = TradingTransactionParams::new(vec![], Address::ZERO, 0, U256::ZERO);
        assert!(!no_val.has_value());

        let with_val = no_val.with_value(U256::from(1u32));
        assert!(with_val.has_value());
    }

    #[test]
    fn trading_tx_params_display() {
        let tx = TradingTransactionParams::new(vec![], Address::ZERO, 21_000, U256::ZERO);
        let s = format!("{tx}");
        assert!(s.contains("tx"));
        assert!(s.contains("21000"));
    }

    // ── PostTradeAdditionalParams ───────────────────────────────────────────

    #[test]
    fn post_trade_default() {
        let p = PostTradeAdditionalParams::default();
        assert!(!p.has_signing_scheme());
        assert!(!p.has_network_costs());
        assert!(!p.should_apply_costs());
    }

    #[test]
    fn post_trade_with_signing_scheme() {
        let p = PostTradeAdditionalParams::default().with_signing_scheme(SigningScheme::PreSign);
        assert!(p.has_signing_scheme());
        assert!(matches!(p.signing_scheme, Some(SigningScheme::PreSign)));
    }

    #[test]
    fn post_trade_with_network_costs_amount() {
        let p = PostTradeAdditionalParams::default().with_network_costs_amount("12345");
        assert!(p.has_network_costs());
        assert_eq!(p.network_costs_amount.as_deref(), Some("12345"));
    }

    #[test]
    fn post_trade_with_apply_costs() {
        let p = PostTradeAdditionalParams::default().with_apply_costs_slippage_and_fees(true);
        assert!(p.should_apply_costs());

        let p2 = PostTradeAdditionalParams::default().with_apply_costs_slippage_and_fees(false);
        assert!(!p2.should_apply_costs());
    }

    #[test]
    fn post_trade_display() {
        let p = PostTradeAdditionalParams::default();
        assert_eq!(format!("{p}"), "post-trade-params");
    }

    // ── SwapAdvancedSettings ────────────────────────────────────────────────

    #[test]
    fn swap_settings_default() {
        let s = SwapAdvancedSettings::default();
        assert!(!s.has_app_data());
        assert!(!s.has_slippage_bps());
        assert!(!s.has_partner_fee());
    }

    #[test]
    fn swap_settings_with_app_data() {
        let s = SwapAdvancedSettings::default().with_app_data(serde_json::json!({"k": "v"}));
        assert!(s.has_app_data());
    }

    #[test]
    fn swap_settings_with_slippage_bps() {
        let s = SwapAdvancedSettings::default().with_slippage_bps(100);
        assert!(s.has_slippage_bps());
        assert_eq!(s.slippage_bps, Some(100));
    }

    #[test]
    fn swap_settings_with_partner_fee() {
        let fee = PartnerFee::single(PartnerFeeEntry::volume(50, "0xRecipient"));
        let s = SwapAdvancedSettings::default().with_partner_fee(fee);
        assert!(s.has_partner_fee());
    }

    #[test]
    fn swap_settings_display() {
        let s = SwapAdvancedSettings::default();
        assert_eq!(format!("{s}"), "swap-settings");
    }

    // ── LimitOrderAdvancedSettings ──────────────────────────────────────────

    #[test]
    fn limit_settings_default() {
        let s = LimitOrderAdvancedSettings::default();
        assert!(!s.has_receiver());
        assert!(!s.has_valid_to());
        assert!(!s.has_partner_fee());
        assert!(!s.has_partially_fillable());
        assert!(!s.has_app_data());
    }

    #[test]
    fn limit_settings_with_receiver() {
        let addr = Address::with_last_byte(0x42);
        let s = LimitOrderAdvancedSettings::default().with_receiver(addr);
        assert!(s.has_receiver());
        assert_eq!(s.receiver, Some(addr));
    }

    #[test]
    fn limit_settings_with_valid_to() {
        let s = LimitOrderAdvancedSettings::default().with_valid_to(1_700_000_000);
        assert!(s.has_valid_to());
        assert_eq!(s.valid_to, Some(1_700_000_000));
    }

    #[test]
    fn limit_settings_with_partner_fee() {
        let fee = PartnerFee::single(PartnerFeeEntry::volume(25, "0xAddr"));
        let s = LimitOrderAdvancedSettings::default().with_partner_fee(fee);
        assert!(s.has_partner_fee());
    }

    #[test]
    fn limit_settings_with_partially_fillable() {
        let s = LimitOrderAdvancedSettings::default().with_partially_fillable(true);
        assert!(s.has_partially_fillable());
        assert_eq!(s.partially_fillable, Some(true));
    }

    #[test]
    fn limit_settings_with_app_data() {
        let s = LimitOrderAdvancedSettings::default().with_app_data("0xabc123");
        assert!(s.has_app_data());
        assert_eq!(s.app_data.as_deref(), Some("0xabc123"));
    }

    #[test]
    fn limit_settings_display() {
        let s = LimitOrderAdvancedSettings::default();
        assert_eq!(format!("{s}"), "limit-settings");
    }

    // ── apply_settings_to_limit_trade_parameters ────────────────────────────

    #[test]
    fn apply_settings_none_returns_unchanged() {
        let params = LimitTradeParameters::sell(
            Address::ZERO,
            Address::ZERO,
            U256::from(1000u32),
            U256::from(900u32),
        );
        let result = apply_settings_to_limit_trade_parameters(params, None);
        assert_eq!(result.sell_amount, U256::from(1000u32));
        assert!(!result.partially_fillable);
    }

    #[test]
    fn apply_settings_overrides_fields() {
        let params = LimitTradeParameters::sell(
            Address::ZERO,
            Address::ZERO,
            U256::from(1000u32),
            U256::from(900u32),
        );
        let settings = LimitOrderAdvancedSettings::default()
            .with_receiver(Address::with_last_byte(0x01))
            .with_valid_to(9999)
            .with_partially_fillable(true)
            .with_app_data("0xbeef");

        let result = apply_settings_to_limit_trade_parameters(params, Some(&settings));
        assert_eq!(result.receiver, Some(Address::with_last_byte(0x01)));
        assert_eq!(result.valid_to, Some(9999));
        assert!(result.partially_fillable);
        assert_eq!(result.app_data.as_deref(), Some("0xbeef"));
    }

    // ── LimitTradeParametersFromQuote ───────────────────────────────────────

    #[test]
    fn limit_from_quote_new() {
        let p = LimitTradeParametersFromQuote::new(
            Address::ZERO,
            Address::with_last_byte(1),
            U256::from(100u32),
            U256::from(90u32),
        );
        assert_eq!(p.sell_token, Address::ZERO);
        assert_eq!(p.buy_token, Address::with_last_byte(1));
        assert_eq!(p.sell_amount, U256::from(100u32));
        assert_eq!(p.buy_amount, U256::from(90u32));
        assert!(!p.has_quote_id());
    }

    #[test]
    fn limit_from_quote_with_quote_id() {
        let p = LimitTradeParametersFromQuote::new(
            Address::ZERO,
            Address::ZERO,
            U256::from(1u32),
            U256::from(1u32),
        )
        .with_quote_id(42);
        assert!(p.has_quote_id());
        assert_eq!(p.quote_id, Some(42));
    }

    #[test]
    fn limit_from_quote_display() {
        let p = LimitTradeParametersFromQuote::new(
            Address::ZERO,
            Address::ZERO,
            U256::from(100u32),
            U256::from(90u32),
        );
        let s = format!("{p}");
        assert!(s.contains("limit-from-quote"));
    }

    // ── TradeParameters ─────────────────────────────────────────────────────

    #[test]
    fn trade_params_sell() {
        let p = TradeParameters::sell(
            Address::ZERO,
            18,
            Address::with_last_byte(1),
            6,
            U256::from(1000u32),
        );
        assert!(p.is_sell());
        assert!(!p.is_buy());
        assert_eq!(p.sell_token_decimals, 18);
        assert_eq!(p.buy_token_decimals, 6);
        assert_eq!(p.amount, U256::from(1000u32));
        assert!(!p.has_slippage_bps());
        assert!(!p.has_receiver());
        assert!(!p.has_partner_fee());
    }

    #[test]
    fn trade_params_buy() {
        let p = TradeParameters::buy(
            Address::ZERO,
            18,
            Address::with_last_byte(1),
            6,
            U256::from(500u32),
        );
        assert!(p.is_buy());
        assert!(!p.is_sell());
    }

    #[test]
    fn trade_params_builders() {
        let recv = Address::with_last_byte(0x99);
        let p = TradeParameters::sell(Address::ZERO, 18, Address::ZERO, 18, U256::from(1u32))
            .with_slippage_bps(50)
            .with_receiver(recv)
            .with_valid_for(600)
            .with_valid_to(1_700_000_000)
            .with_partially_fillable();

        assert!(p.has_slippage_bps());
        assert_eq!(p.slippage_bps, Some(50));
        assert!(p.has_receiver());
        assert_eq!(p.receiver, Some(recv));
        assert_eq!(p.valid_for, Some(600));
        assert_eq!(p.valid_to, Some(1_700_000_000));
        assert_eq!(p.partially_fillable, Some(true));
    }

    #[test]
    fn trade_params_display() {
        let p = TradeParameters::sell(Address::ZERO, 18, Address::ZERO, 18, U256::from(1u32));
        let s = format!("{p}");
        assert!(s.contains("sell"));
    }

    // ── LimitTradeParameters ────────────────────────────────────────────────

    #[test]
    fn limit_trade_params_sell_and_buy() {
        let sell = LimitTradeParameters::sell(
            Address::ZERO,
            Address::with_last_byte(1),
            U256::from(100u32),
            U256::from(90u32),
        );
        assert!(sell.is_sell());
        assert!(!sell.is_buy());
        assert!(!sell.partially_fillable);

        let buy = LimitTradeParameters::buy(
            Address::ZERO,
            Address::with_last_byte(1),
            U256::from(100u32),
            U256::from(90u32),
        );
        assert!(buy.is_buy());
        assert!(!buy.is_sell());
    }

    #[test]
    fn limit_trade_params_builders() {
        let recv = Address::with_last_byte(0x42);
        let p = LimitTradeParameters::sell(
            Address::ZERO,
            Address::ZERO,
            U256::from(1u32),
            U256::from(1u32),
        )
        .with_receiver(recv)
        .with_valid_for(300)
        .with_valid_to(9999)
        .with_partially_fillable();

        assert!(p.has_receiver());
        assert_eq!(p.receiver, Some(recv));
        assert!(p.has_valid_for());
        assert_eq!(p.valid_for, Some(300));
        assert!(p.has_valid_to());
        assert_eq!(p.valid_to, Some(9999));
        assert!(p.partially_fillable);
    }

    #[test]
    fn limit_trade_params_has_app_data_and_partner_fee() {
        let p = LimitTradeParameters::sell(
            Address::ZERO,
            Address::ZERO,
            U256::from(1u32),
            U256::from(1u32),
        );
        assert!(!p.has_app_data());
        assert!(!p.has_partner_fee());
    }

    #[test]
    fn limit_trade_params_display() {
        let p = LimitTradeParameters::sell(
            Address::ZERO,
            Address::ZERO,
            U256::from(100u32),
            U256::from(90u32),
        );
        let s = format!("{p}");
        assert!(s.contains("limit"));
        assert!(s.contains("sell"));
    }

    // ── QuoteAmountsAndCosts ────────────────────────────────────────────────

    fn sample_quote_costs() -> QuoteAmountsAndCosts {
        QuoteAmountsAndCosts {
            is_sell: true,
            before_all_fees: Amounts::new(U256::from(200u32), U256::from(110u32)),
            before_network_costs: Amounts::new(U256::from(200u32), U256::from(100u32)),
            after_network_costs: Amounts::new(U256::from(190u32), U256::from(100u32)),
            after_partner_fees: Amounts::new(U256::from(190u32), U256::from(95u32)),
            after_slippage: Amounts::new(U256::from(190u32), U256::from(90u32)),
            network_fee: NetworkFee::new(U256::from(10u32), U256::ZERO),
            partner_fee: PartnerFeeCost::new(U256::from(5u32), 50),
            protocol_fee: ProtocolFeeCost::new(U256::from(3u32), 30),
        }
    }

    #[test]
    fn quote_costs_is_buy() {
        let sell = sample_quote_costs();
        assert!(!sell.is_buy());

        let mut buy = sample_quote_costs();
        buy.is_sell = false;
        assert!(buy.is_buy());
    }

    #[test]
    fn quote_costs_max_slippage_atoms() {
        let q = sample_quote_costs();
        // after_partner_fees.buy = 95, after_slippage.buy = 90 => slippage = 5
        assert_eq!(q.max_slippage_atoms(), U256::from(5u32));
    }

    #[test]
    fn quote_costs_total_fees_atoms() {
        let q = sample_quote_costs();
        // network_fee.sell=10 + partner_fee.amount=5 + protocol_fee.amount=3 = 18
        assert_eq!(q.total_fees_atoms(), U256::from(18u32));
    }

    #[test]
    fn quote_costs_has_fees() {
        let q = sample_quote_costs();
        assert!(q.has_network_fee());
        assert!(q.has_partner_fee());
        assert!(q.has_protocol_fee());

        let zero_q = QuoteAmountsAndCosts {
            is_sell: true,
            before_all_fees: Amounts::default(),
            before_network_costs: Amounts::default(),
            after_network_costs: Amounts::default(),
            after_partner_fees: Amounts::default(),
            after_slippage: Amounts::default(),
            network_fee: NetworkFee::default(),
            partner_fee: PartnerFeeCost::default(),
            protocol_fee: ProtocolFeeCost::default(),
        };
        assert!(!zero_q.has_network_fee());
        assert!(!zero_q.has_partner_fee());
        assert!(!zero_q.has_protocol_fee());
    }

    #[test]
    fn quote_costs_display() {
        let q = sample_quote_costs();
        let s = format!("{q}");
        assert!(s.contains("sell"));
        assert!(s.contains("network-fee"));
        assert!(s.contains("partner-fee"));
        assert!(s.contains("protocol-fee"));
    }

    // ── map_quote_amounts_and_costs ─────────────────────────────────────────

    #[test]
    fn map_doubles_amounts_preserves_bps() {
        let q = sample_quote_costs();
        let doubled = map_quote_amounts_and_costs(&q, |a| a * U256::from(2u32));

        assert_eq!(doubled.before_all_fees.sell_amount, U256::from(400u32));
        assert_eq!(doubled.network_fee.amount_in_sell_currency, U256::from(20u32));
        assert_eq!(doubled.partner_fee.amount, U256::from(10u32));
        assert_eq!(doubled.protocol_fee.amount, U256::from(6u32));
        // bps preserved
        assert_eq!(doubled.partner_fee.bps, 50);
        assert_eq!(doubled.protocol_fee.bps, 30);
        assert!(doubled.is_sell);
    }

    // ── OrderPostingResult ──────────────────────────────────────────────────

    fn sample_unsigned_order() -> UnsignedOrder {
        UnsignedOrder {
            sell_token: Address::ZERO,
            buy_token: Address::ZERO,
            receiver: Address::ZERO,
            sell_amount: U256::from(100u32),
            buy_amount: U256::from(90u32),
            valid_to: 0,
            app_data: B256::ZERO,
            fee_amount: U256::ZERO,
            kind: OrderKind::Sell,
            partially_fillable: false,
            sell_token_balance: TokenBalance::Erc20,
            buy_token_balance: TokenBalance::Erc20,
        }
    }

    #[test]
    fn order_posting_result_new() {
        let r = OrderPostingResult::new(
            "uid123",
            SigningScheme::Eip712,
            "0xsig",
            sample_unsigned_order(),
        );
        assert_eq!(r.order_id_ref(), "uid123");
        assert_eq!(r.signature_ref(), "0xsig");
    }

    #[test]
    fn order_posting_result_signing_scheme_predicates() {
        let eip712 =
            OrderPostingResult::new("a", SigningScheme::Eip712, "", sample_unsigned_order());
        assert!(eip712.is_eip712());
        assert!(!eip712.is_eth_sign());
        assert!(!eip712.is_eip1271());
        assert!(!eip712.is_presign());

        let eth_sign =
            OrderPostingResult::new("b", SigningScheme::EthSign, "", sample_unsigned_order());
        assert!(eth_sign.is_eth_sign());

        let eip1271 =
            OrderPostingResult::new("c", SigningScheme::Eip1271, "", sample_unsigned_order());
        assert!(eip1271.is_eip1271());

        let presign =
            OrderPostingResult::new("d", SigningScheme::PreSign, "", sample_unsigned_order());
        assert!(presign.is_presign());
    }

    #[test]
    fn order_posting_result_display() {
        let r =
            OrderPostingResult::new("uid-xyz", SigningScheme::Eip712, "", sample_unsigned_order());
        let s = format!("{r}");
        assert!(s.contains("order"));
        assert!(s.contains("uid-xyz"));
    }

    // ── BuildAppDataParams ──────────────────────────────────────────────────

    #[test]
    fn build_app_data_params_new() {
        let p = BuildAppDataParams::new("CoW Swap", 50, OrderClassKind::Market);
        assert_eq!(p.app_code, "CoW Swap");
        assert_eq!(p.slippage_bps, 50);
        assert!(!p.has_partner_fee());
    }

    #[test]
    fn build_app_data_params_with_partner_fee() {
        let fee = PartnerFee::single(PartnerFeeEntry::volume(50, "0xRecip"));
        let p = BuildAppDataParams::new("App", 25, OrderClassKind::Limit).with_partner_fee(fee);
        assert!(p.has_partner_fee());
    }

    #[test]
    fn build_app_data_params_display() {
        let p = BuildAppDataParams::new("MyApp", 100, OrderClassKind::Market);
        let s = format!("{p}");
        assert!(s.contains("build-app-data"));
        assert!(s.contains("MyApp"));
        assert!(s.contains("100bps"));
    }

    // ── SlippageToleranceRequest ────────────────────────────────────────────

    #[test]
    fn slippage_request_new() {
        let r = SlippageToleranceRequest::new(1, Address::ZERO, Address::with_last_byte(1));
        assert_eq!(r.chain_id, 1);
        assert_eq!(r.sell_token, Address::ZERO);
        assert_eq!(r.buy_token, Address::with_last_byte(1));
        assert!(r.sell_amount.is_none());
        assert!(r.buy_amount.is_none());
    }

    #[test]
    fn slippage_request_with_amounts() {
        let r = SlippageToleranceRequest::new(1, Address::ZERO, Address::ZERO)
            .with_sell_amount(U256::from(100u32))
            .with_buy_amount(U256::from(90u32));
        assert_eq!(r.sell_amount, Some(U256::from(100u32)));
        assert_eq!(r.buy_amount, Some(U256::from(90u32)));
    }

    #[test]
    fn slippage_request_display() {
        let r = SlippageToleranceRequest::new(1, Address::ZERO, Address::ZERO);
        let s = format!("{r}");
        assert!(s.contains("slippage-req"));
        assert!(s.contains("chain=1"));
    }

    // ── SlippageToleranceResponse ───────────────────────────────────────────

    #[test]
    fn slippage_response_new() {
        let with = SlippageToleranceResponse::new(Some(50));
        assert!(with.has_suggestion());
        assert_eq!(with.slippage_bps, Some(50));

        let without = SlippageToleranceResponse::new(None);
        assert!(!without.has_suggestion());
    }

    #[test]
    fn slippage_response_display() {
        let with = SlippageToleranceResponse::new(Some(100));
        assert_eq!(format!("{with}"), "slippage-resp(100bps)");

        let without = SlippageToleranceResponse::new(None);
        assert_eq!(format!("{without}"), "slippage-resp(none)");
    }
}
