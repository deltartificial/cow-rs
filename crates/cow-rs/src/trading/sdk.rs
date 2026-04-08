//! [`TradingSdk`] — high-level entry point mirroring the `CoW` `TypeScript` SDK.

use std::sync::Arc;

use alloy_primitives::Address;
use alloy_signer_local::PrivateKeySigner;

use alloy_primitives::U256;

use crate::{
    app_data::{
        build_app_data_doc, build_app_data_doc_full,
        types::{Metadata, OrderClass, OrderClassKind, PartnerFee, Quote, Utm},
    },
    config::{Env, NATIVE_CURRENCY_ADDRESS, SETTLEMENT_CONTRACT, SupportedChainId, VAULT_RELAYER},
    erc20::build_erc20_approve_calldata,
    error::CowError,
    onchain::OnchainReader,
    order_book::{
        Order, OrderBookApi,
        types::{
            AppDataObject, Auction, CompetitionOrderStatus, GetOrdersRequest, GetTradesRequest,
            OrderCancellations, OrderCreation, OrderQuoteRequest, OrderQuoteResponse, QuoteSide,
            SolverCompetition, TotalSurplus, Trade,
        },
    },
    order_signing::{
        build_order_typed_data, invalidate_order_calldata, set_pre_signature_calldata, sign_order,
        sign_order_cancellations, types::UnsignedOrder,
    },
    trading::{
        costs::compute_quote_amounts_and_costs,
        types::{
            LimitTradeParameters, LimitTradeParametersFromQuote, OrderPostingResult,
            PostTradeAdditionalParams, QuoteResults, SwapAdvancedSettings, TradeParameters,
            TradingAppDataInfo, TradingTransactionParams,
        },
    },
    types::{EcdsaSigningScheme, OrderKind, TokenBalance},
};

/// Default order TTL in seconds (30 minutes).
pub const DEFAULT_QUOTE_VALIDITY: u32 = 1_800;

/// Default slippage in basis points (0.5 %).
pub const DEFAULT_SLIPPAGE_BPS: u32 = 50;

/// Default slippage for ETH flow (native-currency sell) orders, in basis points.
///
/// Currently identical to [`DEFAULT_SLIPPAGE_BPS`] (50 bps = 0.5 %) on all
/// supported chains. Mirrors `ETH_FLOW_DEFAULT_SLIPPAGE_BPS` from the
/// `TypeScript` SDK.
pub const ETH_FLOW_DEFAULT_SLIPPAGE_BPS: u32 = DEFAULT_SLIPPAGE_BPS;

/// Fallback gas limit for smart-contract wallet interactions (150 000 gas).
///
/// Used when on-chain gas estimation is unavailable.
/// Mirrors `GAS_LIMIT_DEFAULT` from the `TypeScript` SDK.
pub const GAS_LIMIT_DEFAULT: u64 = 150_000;

/// Add a 20 % safety margin to a gas estimate.
///
/// Returns `gas * 120 / 100`, mirroring `calculateGasMargin` from the
/// `TypeScript` SDK.
///
/// # Arguments
///
/// * `gas` — the raw gas estimate to pad.
///
/// # Returns
///
/// The gas estimate increased by 20 %.
#[must_use]
pub const fn calculate_gas_margin(gas: u64) -> u64 {
    gas * 120 / 100
}

/// App-data used when none is provided: zero bytes32.
const DEFAULT_APP_DATA: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";

/// Default `TradingAppDataInfo` used as a fallback when app-data generation fails.
fn default_app_data_info() -> TradingAppDataInfo {
    TradingAppDataInfo {
        full_app_data: "{}".to_owned(),
        app_data_keccak256: DEFAULT_APP_DATA.to_owned(),
    }
}

// ── Utility functions ─────────────────────────────────────────────────────────

/// Return `true` if `sell_token` is the native currency sentinel address.
///
/// When this is `true`, the trade should be submitted via the `EthFlow` contract
/// rather than the standard `GPv2Settlement` flow.
///
/// Mirrors `getIsEthFlowOrder` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `sell_token` — the sell token address to check.
///
/// # Returns
///
/// `true` when `sell_token` equals [`NATIVE_CURRENCY_ADDRESS`].
///
/// # Example
///
/// ```
/// use cow_rs::{NATIVE_CURRENCY_ADDRESS, trading::get_is_eth_flow_order};
/// assert!(get_is_eth_flow_order(NATIVE_CURRENCY_ADDRESS));
/// assert!(!get_is_eth_flow_order(alloy_primitives::Address::ZERO));
/// ```
#[must_use]
pub fn get_is_eth_flow_order(sell_token: alloy_primitives::Address) -> bool {
    sell_token == NATIVE_CURRENCY_ADDRESS
}

/// Return the default `UTM` parameters embedded in orders by the `CoW` Protocol `SDK`.
///
/// Mirrors `getDefaultUtmParams` from the `TypeScript` SDK.
///
/// # Returns
///
/// A [`Utm`] struct pre-filled with the SDK's default campaign tracking values.
///
/// # Example
///
/// ```rust
/// use cow_rs::trading::get_default_utm_params;
///
/// let utm = get_default_utm_params();
/// assert_eq!(utm.utm_source.as_deref(), Some("web"));
/// assert!(utm.utm_content.is_none());
/// ```
#[must_use]
pub fn get_default_utm_params() -> Utm {
    Utm {
        utm_source: Some("web".to_owned()),
        utm_medium: Some(concat!("este-cowswap/", env!("CARGO_PKG_VERSION")).to_owned()),
        utm_campaign: Some("CoW Swap".to_owned()),
        utm_term: Some("trading".to_owned()),
        utm_content: None,
    }
}

/// Convert [`TradeParameters`] + a raw quote response into [`LimitTradeParametersFromQuote`].
///
/// Extracts the amounts from the quote and packages them with the token pair,
/// mirroring `swapParamsToLimitOrderParams` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `params` — the original swap trade parameters (token pair, kind, etc.).
/// * `quote` — the raw order-quote response from the orderbook API.
///
/// # Returns
///
/// A [`LimitTradeParametersFromQuote`] containing the token pair, parsed
/// sell/buy amounts from the quote, and the quote ID.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, U256};
/// use cow_rs::{
///     OrderKind,
///     order_book::{OrderQuoteResponse, QuoteData},
///     trading::{TradeParameters, swap_params_to_limit_order_params},
/// };
///
/// let params = TradeParameters {
///     kind: OrderKind::Sell,
///     sell_token: Address::ZERO,
///     sell_token_decimals: 18,
///     buy_token: Address::ZERO,
///     buy_token_decimals: 6,
///     amount: U256::from(1_000_000u64),
///     slippage_bps: None,
///     receiver: None,
///     valid_for: None,
///     valid_to: None,
///     partially_fillable: None,
///     partner_fee: None,
/// };
/// // (quota construction omitted in doc test)
/// ```
#[must_use]
pub fn swap_params_to_limit_order_params(
    params: &TradeParameters,
    quote: &OrderQuoteResponse,
) -> LimitTradeParametersFromQuote {
    let sell_amount: alloy_primitives::U256 =
        quote.quote.sell_amount.parse().map_or(alloy_primitives::U256::ZERO, |v| v);
    let buy_amount: alloy_primitives::U256 =
        quote.quote.buy_amount.parse().map_or(alloy_primitives::U256::ZERO, |v| v);
    LimitTradeParametersFromQuote {
        sell_token: params.sell_token,
        buy_token: params.buy_token,
        sell_amount,
        buy_amount,
        quote_id: quote.id,
    }
}

// ── Config ───────────────────────────────────────────────────────────────────

/// Configuration for [`TradingSdk`].
#[derive(Debug, Clone)]
pub struct TradingSdkConfig {
    /// Target chain.
    pub chain_id: SupportedChainId,
    /// API environment.
    pub env: Env,
    /// Application identifier embedded in order app-data.
    pub app_code: String,
    /// Default slippage in basis points.
    pub slippage_bps: u32,
    /// Optional UTM tracking parameters embedded in every order's app-data.
    ///
    /// When set, all orders submitted by this SDK instance will carry these
    /// UTM parameters in their metadata, enabling campaign attribution.
    pub utm: Option<Utm>,
    /// Optional partner fee embedded in every order's app-data.
    ///
    /// When set, every order will include this fee policy in its metadata,
    /// enabling protocol-level fee collection for integration partners.
    pub partner_fee: Option<PartnerFee>,
    /// Optional JSON-RPC endpoint URL for on-chain reads.
    ///
    /// Required for methods that query the chain directly (e.g.
    /// [`TradingSdk::get_cow_protocol_allowance`]).  When `None`, those
    /// methods return a [`CowError::Rpc`] with code `-1`.
    pub rpc_url: Option<String>,
}

impl TradingSdkConfig {
    /// Convenience constructor for the production environment (`api.cow.fi`).
    ///
    /// # Arguments
    ///
    /// * `chain_id` — the target blockchain network.
    /// * `app_code` — application identifier embedded in order app-data.
    ///
    /// # Returns
    ///
    /// A [`TradingSdkConfig`] configured for the production API with default
    /// slippage ([`DEFAULT_SLIPPAGE_BPS`]) and no UTM, partner fee, or RPC URL.
    #[must_use]
    pub fn prod(chain_id: SupportedChainId, app_code: impl Into<String>) -> Self {
        Self {
            chain_id,
            env: Env::Prod,
            app_code: app_code.into(),
            slippage_bps: DEFAULT_SLIPPAGE_BPS,
            utm: None,
            partner_fee: None,
            rpc_url: None,
        }
    }

    /// Convenience constructor for the staging (barn) environment.
    ///
    /// Points at `barn.api.cow.fi` — useful for integration testing without
    /// risking real funds on mainnet.
    ///
    /// # Arguments
    ///
    /// * `chain_id` — the target blockchain network.
    /// * `app_code` — application identifier embedded in order app-data.
    ///
    /// # Returns
    ///
    /// A [`TradingSdkConfig`] configured for the staging API with default
    /// slippage ([`DEFAULT_SLIPPAGE_BPS`]) and no UTM, partner fee, or RPC URL.
    #[must_use]
    pub fn staging(chain_id: SupportedChainId, app_code: impl Into<String>) -> Self {
        Self {
            chain_id,
            env: Env::Staging,
            app_code: app_code.into(),
            slippage_bps: DEFAULT_SLIPPAGE_BPS,
            utm: None,
            partner_fee: None,
            rpc_url: None,
        }
    }

    /// Override the default slippage tolerance.
    ///
    /// # Arguments
    ///
    /// * `bps` — slippage tolerance in basis points (e.g. `50` = 0.5 %).
    ///
    /// # Returns
    ///
    /// The config with the updated slippage value (builder pattern).
    #[must_use]
    pub const fn with_slippage_bps(mut self, bps: u32) -> Self {
        self.slippage_bps = bps;
        self
    }

    /// Attach `UTM` tracking parameters to all orders submitted via this config.
    ///
    /// # Arguments
    ///
    /// * `utm` — the [`Utm`] tracking parameters to embed in every order's app-data.
    ///
    /// # Returns
    ///
    /// The config with UTM parameters set (builder pattern).
    #[must_use]
    pub fn with_utm(mut self, utm: Utm) -> Self {
        self.utm = Some(utm);
        self
    }

    /// Attach a partner fee policy to all orders submitted via this config.
    ///
    /// # Arguments
    ///
    /// * `fee` — the [`PartnerFee`] policy to embed in every order's app-data.
    ///
    /// # Returns
    ///
    /// The config with the partner fee set (builder pattern).
    #[must_use]
    pub fn with_partner_fee(mut self, fee: PartnerFee) -> Self {
        self.partner_fee = Some(fee);
        self
    }

    /// Attach a JSON-RPC endpoint URL for on-chain reads.
    ///
    /// Required for [`TradingSdk::get_cow_protocol_allowance`].
    ///
    /// # Arguments
    ///
    /// * `rpc_url` — the JSON-RPC endpoint URL (e.g. `"https://rpc.sepolia.org"`).
    ///
    /// # Returns
    ///
    /// The config with the RPC URL set (builder pattern).
    #[must_use]
    pub fn with_rpc_url(mut self, rpc_url: impl Into<String>) -> Self {
        self.rpc_url = Some(rpc_url.into());
        self
    }
}

// ── SDK ───────────────────────────────────────────────────────────────────────

/// High-level `CoW` Protocol trading interface.
///
/// Mirrors the `TypeScript` `TradingSdk`: quote a swap, then post it as a signed order.
///
/// # Example
///
/// ```rust,no_run
/// use alloy_primitives::U256;
/// use cow_rs::{Env, OrderKind, SupportedChainId, TradeParameters, TradingSdk, TradingSdkConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let sdk = TradingSdk::new(
///     TradingSdkConfig::prod(SupportedChainId::Sepolia, "MyApp"),
///     "0xdeadbeef...",
/// )?;
///
/// let sell_token = "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14".parse()?;
/// let buy_token = "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238".parse()?;
///
/// let quote = sdk
///     .get_quote(TradeParameters {
///         kind: OrderKind::Sell,
///         sell_token,
///         sell_token_decimals: 18,
///         buy_token,
///         buy_token_decimals: 6,
///         amount: U256::from(1_000_000_000_000_000_u64), // 0.001 WETH
///         slippage_bps: Some(50),
///         receiver: None,
///         valid_for: None,
///         valid_to: None,
///         partially_fillable: None,
///         partner_fee: None,
///     })
///     .await?;
///
/// let result = sdk.post_swap_order_from_quote(&quote, None).await?;
/// println!("order submitted: {}", result.order_id);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct TradingSdk {
    config: Arc<TradingSdkConfig>,
    api: Arc<OrderBookApi>,
    signer: Arc<PrivateKeySigner>,
}

impl TradingSdk {
    /// Create a new [`TradingSdk`].
    ///
    /// `private_key_hex` must be a 0x-prefixed or bare 64-char hex private key.
    ///
    /// # Arguments
    ///
    /// * `config` — SDK configuration (chain, environment, app code, etc.).
    /// * `private_key_hex` — the signer's private key as a hex string.
    ///
    /// # Returns
    ///
    /// A configured [`TradingSdk`] instance ready to quote and post orders.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Signing`] if the private key cannot be parsed.
    pub fn new(config: TradingSdkConfig, private_key_hex: &str) -> Result<Self, CowError> {
        let key = private_key_hex.trim_start_matches("0x");
        let signer: PrivateKeySigner = key
            .parse()
            .map_err(|e: alloy_signer_local::LocalSignerError| CowError::Signing(e.to_string()))?;
        let api = OrderBookApi::new(config.chain_id, config.env);
        Ok(Self { config: Arc::new(config), api: Arc::new(api), signer: Arc::new(signer) })
    }

    /// Fetch a price quote from the `CoW` Protocol orderbook.
    ///
    /// Builds an order-quote request from `params`, submits it to the orderbook
    /// API, and returns the resulting quote with computed amounts, costs, and a
    /// ready-to-sign order struct.
    ///
    /// # Arguments
    ///
    /// * `params` — trade parameters describing the token pair, amount, kind, etc.
    ///
    /// # Returns
    ///
    /// A [`QuoteResults`] containing the unsigned order, typed data, raw quote
    /// response, computed amounts/costs, and app-data info.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails or the response
    /// cannot be parsed.
    pub async fn get_quote(&self, params: TradeParameters) -> Result<QuoteResults, CowError> {
        get_quote_impl(&self.config, &self.api, &self.signer, params, None).await
    }

    /// Sign and submit an order from a previously obtained [`QuoteResults`].
    ///
    /// # Arguments
    ///
    /// * `quote` — the quote results obtained from [`get_quote`](Self::get_quote).
    /// * `scheme` — optional ECDSA signing scheme override; defaults to
    ///   [`EcdsaSigningScheme::Eip712`].
    ///
    /// # Returns
    ///
    /// An [`OrderPostingResult`] with the `orderId`, signing scheme,
    /// signature, and the order struct that was signed.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Signing`] if the order cannot be signed, or
    /// [`CowError`] if the orderbook rejects the submission.
    pub async fn post_swap_order_from_quote(
        &self,
        quote: &QuoteResults,
        scheme: Option<EcdsaSigningScheme>,
    ) -> Result<OrderPostingResult, CowError> {
        post_order_impl(&self.config, &self.api, &self.signer, quote, scheme).await
    }

    /// Fetch a quote and immediately sign + submit the order.
    ///
    /// Equivalent to calling [`get_quote`](Self::get_quote) then
    /// [`post_swap_order_from_quote`](Self::post_swap_order_from_quote).
    ///
    /// # Arguments
    ///
    /// * `params` — trade parameters describing the token pair, amount, kind, etc.
    ///
    /// # Returns
    ///
    /// An [`OrderPostingResult`] with the `orderId`, signing scheme,
    /// signature, and the order struct that was signed.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if fetching the quote, signing, or submitting the
    /// order fails.
    pub async fn post_swap_order(
        &self,
        params: TradeParameters,
    ) -> Result<OrderPostingResult, CowError> {
        let quote = self.get_quote(params).await?;
        self.post_swap_order_from_quote(&quote, None).await
    }

    /// Fetch a price quote with advanced per-call overrides.
    ///
    /// Like [`get_quote`](Self::get_quote) but lets the caller override
    /// slippage, partner fee, and app-data through [`SwapAdvancedSettings`],
    /// on top of any values already set in [`TradeParameters`] or the
    /// SDK-level [`TradingSdkConfig`].
    ///
    /// Mirrors the two-argument form of `getQuote` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `params` — trade parameters describing the token pair, amount, kind, etc.
    /// * `settings` — per-call overrides for slippage, partner fee, and app-data.
    ///
    /// # Returns
    ///
    /// A [`QuoteResults`] containing the unsigned order, typed data, raw quote
    /// response, computed amounts/costs, and app-data info.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails or the response
    /// cannot be parsed.
    pub async fn get_quote_with_settings(
        &self,
        params: TradeParameters,
        settings: &SwapAdvancedSettings,
    ) -> Result<QuoteResults, CowError> {
        get_quote_impl(&self.config, &self.api, &self.signer, params, Some(settings)).await
    }

    /// Fetch a quote and immediately sign + submit the order, with advanced overrides.
    ///
    /// Like [`post_swap_order`](Self::post_swap_order) but lets the caller
    /// override slippage, partner fee, and app-data via [`SwapAdvancedSettings`].
    ///
    /// Mirrors the two-argument form of `postSwapOrder` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `params` — trade parameters describing the token pair, amount, kind, etc.
    /// * `settings` — per-call overrides for slippage, partner fee, and app-data.
    ///
    /// # Returns
    ///
    /// An [`OrderPostingResult`] with the `orderId`, signing scheme,
    /// signature, and the order struct that was signed.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if fetching the quote, signing, or submitting the
    /// order fails.
    pub async fn post_swap_order_with_settings(
        &self,
        params: TradeParameters,
        settings: &SwapAdvancedSettings,
    ) -> Result<OrderPostingResult, CowError> {
        let quote =
            get_quote_impl(&self.config, &self.api, &self.signer, params, Some(settings)).await?;
        post_order_impl(&self.config, &self.api, &self.signer, &quote, None).await
    }

    /// Fetch a `CoW` Protocol order by its unique identifier.
    ///
    /// # Arguments
    ///
    /// * `uid` — the order's unique identifier string.
    ///
    /// # Returns
    ///
    /// The [`Order`] matching the given UID.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails or the order
    /// is not found.
    pub async fn get_order(&self, uid: &str) -> Result<Order, CowError> {
        self.api.get_order(uid).await
    }

    /// Off-chain cancel one or more orders by signing their UIDs.
    ///
    /// Signs an EIP-712 `OrderCancellations(bytes[] orderUids)` struct and
    /// submits it to `DELETE /api/v1/orders`.  This is best-effort: orders
    /// already included in an in-flight settlement may still execute.
    ///
    /// `signing_scheme` should match how the original orders were signed
    /// (typically [`EcdsaSigningScheme::Eip712`]).
    ///
    /// # Arguments
    ///
    /// * `order_uids` — list of order UIDs to cancel.
    /// * `signing_scheme` — the ECDSA signing scheme used for the cancellation signature.
    ///
    /// # Returns
    ///
    /// `()` on success.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Signing`] if the cancellation message cannot be
    /// signed, or [`CowError`] if the orderbook rejects the cancellation request.
    pub async fn off_chain_cancel_orders(
        &self,
        order_uids: Vec<String>,
        signing_scheme: EcdsaSigningScheme,
    ) -> Result<(), CowError> {
        let uid_refs: Vec<&str> = order_uids.iter().map(String::as_str).collect();
        let signing = sign_order_cancellations(
            &uid_refs,
            self.config.chain_id.as_u64(),
            &self.signer,
            signing_scheme,
        )
        .await?;
        let body = OrderCancellations { order_uids, signature: signing.signature, signing_scheme };
        self.api.cancel_orders(&body).await
    }

    /// Off-chain cancel a single order by its UID.
    ///
    /// Convenience wrapper around [`off_chain_cancel_orders`](Self::off_chain_cancel_orders).
    ///
    /// # Arguments
    ///
    /// * `order_uid` — the UID of the order to cancel.
    /// * `signing_scheme` — the ECDSA signing scheme used for the cancellation signature.
    ///
    /// # Returns
    ///
    /// `()` on success.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Signing`] if the cancellation message cannot be
    /// signed, or [`CowError`] if the orderbook rejects the cancellation request.
    pub async fn off_chain_cancel_order(
        &self,
        order_uid: String,
        signing_scheme: EcdsaSigningScheme,
    ) -> Result<(), CowError> {
        self.off_chain_cancel_orders(vec![order_uid], signing_scheme).await
    }

    /// Sign and submit a limit order (fixed price, no slippage adjustment).
    ///
    /// # Arguments
    ///
    /// * `params` — limit-order parameters (token pair, amounts, validity, etc.).
    /// * `scheme` — optional ECDSA signing scheme override; defaults to
    ///   [`EcdsaSigningScheme::Eip712`].
    ///
    /// # Returns
    ///
    /// An [`OrderPostingResult`] with the `orderId`, signing scheme,
    /// signature, and the order struct that was signed.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Signing`] if the order cannot be signed, or
    /// [`CowError`] if the orderbook rejects the submission.
    pub async fn post_limit_order(
        &self,
        params: LimitTradeParameters,
        scheme: Option<EcdsaSigningScheme>,
    ) -> Result<OrderPostingResult, CowError> {
        post_limit_order_impl(&self.config, &self.api, &self.signer, params, scheme).await
    }

    /// Return the wallet address used for signing.
    ///
    /// # Returns
    ///
    /// The [`Address`] derived from the private key provided at construction.
    #[must_use]
    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// Build a `CoW` Protocol Explorer URL for `order_uid`.
    ///
    /// # Arguments
    ///
    /// * `order_uid` — the order's unique identifier string.
    ///
    /// # Returns
    ///
    /// A URL string pointing to `https://explorer.cow.fi/{network}/orders/{uid}`.
    #[must_use]
    pub fn get_order_link(&self, order_uid: &str) -> String {
        crate::config::chain::order_explorer_link(self.config.chain_id, order_uid)
    }

    /// Fetch the native-currency price of `token` (price of 1 token in the
    /// native currency, e.g. ETH).
    ///
    /// # Arguments
    ///
    /// * `token` — the ERC-20 token address to price.
    ///
    /// # Returns
    ///
    /// The price of one whole token denominated in the chain's native currency.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails or the token
    /// has no known price.
    pub async fn get_native_price(
        &self,
        token: alloy_primitives::Address,
    ) -> Result<f64, CowError> {
        self.api.get_native_price(token).await
    }

    /// Fetch the current batch auction (solvable orders + reference prices).
    ///
    /// # Returns
    ///
    /// The current [`Auction`] containing solvable orders and reference prices.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails.
    pub async fn get_auction(&self) -> Result<Auction, CowError> {
        self.api.get_auction().await
    }

    /// Fetch trades for `owner` (up to `limit`; defaults to 10).
    ///
    /// # Arguments
    ///
    /// * `owner` — the trader's on-chain address.
    /// * `limit` — maximum number of trades to return; defaults to 10 when `None`.
    ///
    /// # Returns
    ///
    /// A [`Vec<Trade>`] of the owner's recent trades.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails.
    pub async fn get_trades(
        &self,
        owner: alloy_primitives::Address,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>, CowError> {
        self.api.get_trades_for_account(owner, limit).await
    }

    /// Fetch orders for `owner` (up to `limit`; defaults to 1000).
    ///
    /// # Arguments
    ///
    /// * `owner` — the trader's on-chain address.
    /// * `limit` — maximum number of orders to return; defaults to 1000 when `None`.
    ///
    /// # Returns
    ///
    /// A [`Vec<Order>`] of the owner's orders.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails.
    pub async fn get_orders_for_account(
        &self,
        owner: alloy_primitives::Address,
        limit: Option<u32>,
    ) -> Result<Vec<Order>, CowError> {
        self.api.get_orders_for_account(owner, limit).await
    }

    /// Derive [`LimitTradeParameters`] from a current market quote.
    ///
    /// Fetches a price quote for `params` and returns [`LimitTradeParameters`] using
    /// the post-network-cost amounts (fees accounted for, no slippage applied).
    ///
    /// Use this to bootstrap a limit order at the current market price, then
    /// adjust the `buy_amount`/`sell_amount` before calling [`post_limit_order`].
    /// This mirrors `getLimitTradeParameters` from the `TypeScript` SDK.
    ///
    /// [`post_limit_order`]: Self::post_limit_order
    ///
    /// # Arguments
    ///
    /// * `params` — trade parameters describing the token pair, amount, kind, etc.
    ///
    /// # Returns
    ///
    /// A [`LimitTradeParameters`] with amounts derived from the current market
    /// quote, suitable for further adjustment before posting.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the quote fetch fails.
    pub async fn get_limit_trade_parameters(
        &self,
        params: TradeParameters,
    ) -> Result<LimitTradeParameters, CowError> {
        let quote = get_quote_impl(&self.config, &self.api, &self.signer, params, None).await?;
        let costs = &quote.amounts_and_costs;
        let order = &quote.order_to_sign;
        Ok(LimitTradeParameters {
            kind: order.kind,
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            sell_amount: costs.after_network_costs.sell_amount,
            buy_amount: costs.after_network_costs.buy_amount,
            receiver: Some(order.receiver),
            valid_for: None,
            valid_to: None,
            partially_fillable: order.partially_fillable,
            app_data: None,
            partner_fee: None,
        })
    }

    /// Derive [`LimitTradeParameters`] from an already-fetched [`QuoteResults`].
    ///
    /// Unlike [`get_limit_trade_parameters`], this method makes **no API call** —
    /// it extracts the amounts and token pair directly from `quote`.
    ///
    /// Mirrors `getLimitTradeParametersFromQuote` from the `TypeScript` SDK.
    ///
    /// [`get_limit_trade_parameters`]: Self::get_limit_trade_parameters
    ///
    /// # Arguments
    ///
    /// * `quote` — the previously fetched [`QuoteResults`].
    ///
    /// # Returns
    ///
    /// A [`LimitTradeParameters`] with amounts and token pair extracted from
    /// the quote's post-network-cost figures.
    #[must_use]
    pub const fn get_limit_trade_parameters_from_quote(
        &self,
        quote: &QuoteResults,
    ) -> LimitTradeParameters {
        let costs = &quote.amounts_and_costs;
        let order = &quote.order_to_sign;
        LimitTradeParameters {
            kind: order.kind,
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            sell_amount: costs.after_network_costs.sell_amount,
            buy_amount: costs.after_network_costs.buy_amount,
            receiver: Some(order.receiver),
            valid_for: None,
            valid_to: None,
            partially_fillable: order.partially_fillable,
            app_data: None,
            partner_fee: None,
        }
    }

    /// Submit an unsigned order with **pre-sign** authentication.
    ///
    /// The order is sent to the orderbook immediately, but will only be
    /// considered for settlement after the owner calls
    /// `GPv2Settlement.setPreSignature(orderUid, true)` on-chain.
    ///
    /// # Arguments
    ///
    /// * `order` — the [`UnsignedOrder`] to submit.
    ///
    /// # Returns
    ///
    /// An [`OrderPostingResult`] with the order ID and pre-sign signature.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook rejects the submission.
    pub async fn post_presign_order(
        &self,
        order: &UnsignedOrder,
    ) -> Result<OrderPostingResult, CowError> {
        use crate::order_signing::presign_result;
        let owner = self.signer.address();
        let signing = presign_result(owner);
        let order_id = self
            .api
            .send_order(&OrderCreation {
                sell_token: order.sell_token,
                buy_token: order.buy_token,
                receiver: order.receiver,
                sell_amount: order.sell_amount.to_string(),
                buy_amount: order.buy_amount.to_string(),
                valid_to: order.valid_to,
                app_data: format!("0x{}", alloy_primitives::hex::encode(order.app_data)),
                fee_amount: order.fee_amount.to_string(),
                kind: order.kind,
                partially_fillable: order.partially_fillable,
                sell_token_balance: order.sell_token_balance,
                buy_token_balance: order.buy_token_balance,
                signing_scheme: signing.signing_scheme,
                signature: signing.signature.clone(),
                from: owner,
                quote_id: None,
            })
            .await?;
        Ok(OrderPostingResult {
            order_id,
            signing_scheme: signing.signing_scheme,
            signature: signing.signature,
            order_to_sign: order.clone(),
        })
    }

    /// Submit an unsigned order signed by a **EIP-1271 smart-contract wallet**.
    ///
    /// `signature_bytes` should be whatever the contract's off-chain signing
    /// mechanism produces; it is forwarded verbatim to the orderbook.
    ///
    /// # Arguments
    ///
    /// * `order` — the [`UnsignedOrder`] to submit.
    /// * `signature_bytes` — the raw EIP-1271 signature bytes from the smart-contract wallet.
    ///
    /// # Returns
    ///
    /// An [`OrderPostingResult`] with the order ID and EIP-1271 signature.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook rejects the submission.
    pub async fn post_eip1271_order(
        &self,
        order: &UnsignedOrder,
        signature_bytes: &[u8],
    ) -> Result<OrderPostingResult, CowError> {
        use crate::order_signing::eip1271_result;
        let signing = eip1271_result(signature_bytes);
        let owner = self.signer.address();
        let order_id = self
            .api
            .send_order(&OrderCreation {
                sell_token: order.sell_token,
                buy_token: order.buy_token,
                receiver: order.receiver,
                sell_amount: order.sell_amount.to_string(),
                buy_amount: order.buy_amount.to_string(),
                valid_to: order.valid_to,
                app_data: format!("0x{}", alloy_primitives::hex::encode(order.app_data)),
                fee_amount: order.fee_amount.to_string(),
                kind: order.kind,
                partially_fillable: order.partially_fillable,
                sell_token_balance: order.sell_token_balance,
                buy_token_balance: order.buy_token_balance,
                signing_scheme: signing.signing_scheme,
                signature: signing.signature.clone(),
                from: owner,
                quote_id: None,
            })
            .await?;
        Ok(OrderPostingResult {
            order_id,
            signing_scheme: signing.signing_scheme,
            signature: signing.signature,
            order_to_sign: order.clone(),
        })
    }

    /// Build an on-chain transaction to pre-sign (or un-pre-sign) an order.
    ///
    /// Returns [`TradingTransactionParams`] targeting `GPv2Settlement` with
    /// calldata for `setPreSignature(orderUid, signed)`.  Send this transaction
    /// from the order owner's wallet to authenticate a pre-sign order.
    ///
    /// `signed = true` authenticates the order; `signed = false` revokes it.
    ///
    /// Mirrors `getPreSignTransaction` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `order_uid` — the order's unique identifier (`0x`-prefixed hex).
    /// * `signed` — `true` to authenticate, `false` to revoke.
    ///
    /// # Returns
    ///
    /// A [`TradingTransactionParams`] ready to be sent on-chain.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if `order_uid` is not valid hex.
    pub fn get_pre_sign_transaction(
        &self,
        order_uid: &str,
        signed: bool,
    ) -> Result<TradingTransactionParams, CowError> {
        let data = set_pre_signature_calldata(order_uid, signed)?;
        Ok(TradingTransactionParams {
            data,
            to: SETTLEMENT_CONTRACT,
            gas_limit: GAS_LIMIT_DEFAULT,
            value: U256::ZERO,
        })
    }

    /// Build an on-chain transaction to permanently cancel an order.
    ///
    /// Returns [`TradingTransactionParams`] targeting `GPv2Settlement` with
    /// calldata for `invalidateOrder(orderUid)`.  Once executed, the order
    /// can never be settled even if it was previously signed or pre-signed.
    ///
    /// Mirrors `getOnChainCancellation` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `order_uid` — the order's unique identifier (`0x`-prefixed hex).
    ///
    /// # Returns
    ///
    /// A [`TradingTransactionParams`] ready to be sent on-chain.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if `order_uid` is not valid hex.
    pub fn get_on_chain_cancellation(
        &self,
        order_uid: &str,
    ) -> Result<TradingTransactionParams, CowError> {
        let data = invalidate_order_calldata(order_uid)?;
        Ok(TradingTransactionParams {
            data,
            to: SETTLEMENT_CONTRACT,
            gas_limit: GAS_LIMIT_DEFAULT,
            value: U256::ZERO,
        })
    }

    // ── OrderBook pass-through methods ────────────────────────────────────────

    /// Fetch the fine-grained competition status of an order.
    ///
    /// Returns the order's lifecycle stage within the current batch auction:
    /// open, scheduled, active, solved, executing, traded, or cancelled.
    ///
    /// Mirrors `getOrderStatus` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `order_uid` — the order's unique identifier string.
    ///
    /// # Returns
    ///
    /// A [`CompetitionOrderStatus`] describing the order's current lifecycle stage.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails or the order
    /// is not found.
    pub async fn get_order_status(
        &self,
        order_uid: &str,
    ) -> Result<CompetitionOrderStatus, CowError> {
        self.api.get_order_status(order_uid).await
    }

    /// Fetch all orders settled in a specific on-chain settlement transaction.
    ///
    /// Mirrors `getOrdersByTx` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `tx_hash` — the settlement transaction hash.
    ///
    /// # Returns
    ///
    /// A [`Vec<Order>`] of orders included in the settlement.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails.
    pub async fn get_orders_by_tx(&self, tx_hash: &str) -> Result<Vec<Order>, CowError> {
        self.api.get_orders_by_tx(tx_hash).await
    }

    /// Fetch an order, falling back to the opposite environment on 404.
    ///
    /// Useful during development when orders may be on staging vs production.
    ///
    /// Mirrors `getOrderMultiEnv` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `uid` — the order's unique identifier string.
    ///
    /// # Returns
    ///
    /// The [`Order`] found in either environment.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the order is not found in either environment.
    pub async fn get_order_multi_env(&self, uid: &str) -> Result<Order, CowError> {
        self.api.get_order_multi_env(uid).await
    }

    /// Fetch orders using a paginated [`GetOrdersRequest`].
    ///
    /// Supports `owner`, `limit`, and `offset` fields.
    /// Mirrors `getOrders` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `req` — the paginated order query parameters.
    ///
    /// # Returns
    ///
    /// A [`Vec<Order>`] matching the request criteria.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails.
    pub async fn get_orders(&self, req: &GetOrdersRequest) -> Result<Vec<Order>, CowError> {
        self.api.get_orders(req).await
    }

    /// Fetch trades using a unified [`GetTradesRequest`].
    ///
    /// Supports filtering by `owner`, `order_uid`, `limit`, and `offset`.
    /// Mirrors `getTrades` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `req` — the trade query parameters.
    ///
    /// # Returns
    ///
    /// A [`Vec<Trade>`] matching the request criteria.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails.
    pub async fn get_trades_with_request(
        &self,
        req: &GetTradesRequest,
    ) -> Result<Vec<Trade>, CowError> {
        self.api.get_trades_with_request(req).await
    }

    /// Fetch solver competition details for a specific auction.
    ///
    /// Mirrors `getSolverCompetition` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `auction_id` — the numeric auction identifier.
    ///
    /// # Returns
    ///
    /// A [`SolverCompetition`] with detailed solver rankings and solutions.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails or the auction
    /// is not found.
    pub async fn get_solver_competition(
        &self,
        auction_id: i64,
    ) -> Result<SolverCompetition, CowError> {
        self.api.get_solver_competition(auction_id).await
    }

    /// Fetch solver competition details by settlement transaction hash.
    ///
    /// Mirrors `getSolverCompetitionByTx` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `tx_hash` — the settlement transaction hash.
    ///
    /// # Returns
    ///
    /// A [`SolverCompetition`] for the given settlement.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails or no competition
    /// is found for the given transaction hash.
    pub async fn get_solver_competition_by_tx(
        &self,
        tx_hash: &str,
    ) -> Result<SolverCompetition, CowError> {
        self.api.get_solver_competition_by_tx(tx_hash).await
    }

    /// Fetch the most recent solver competition result.
    ///
    /// Mirrors `getSolverCompetitionLatest` from the `TypeScript` SDK.
    ///
    /// # Returns
    ///
    /// The latest [`SolverCompetition`] result.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails.
    pub async fn get_solver_competition_latest(&self) -> Result<SolverCompetition, CowError> {
        self.api.get_solver_competition_latest().await
    }

    /// Fetch the solver competition for a given `auction_id` (v2 API).
    ///
    /// # Arguments
    ///
    /// * `auction_id` — the numeric auction identifier.
    ///
    /// # Returns
    ///
    /// A [`SolverCompetition`] for the given auction (v2 format).
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails or the auction
    /// is not found.
    pub async fn get_solver_competition_v2(
        &self,
        auction_id: i64,
    ) -> Result<SolverCompetition, CowError> {
        self.api.get_solver_competition_v2(auction_id).await
    }

    /// Fetch the solver competition by settlement transaction hash (v2 API).
    ///
    /// # Arguments
    ///
    /// * `tx_hash` — the settlement transaction hash.
    ///
    /// # Returns
    ///
    /// A [`SolverCompetition`] for the given settlement (v2 format).
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails or no competition
    /// is found for the given transaction hash.
    pub async fn get_solver_competition_by_tx_v2(
        &self,
        tx_hash: &str,
    ) -> Result<SolverCompetition, CowError> {
        self.api.get_solver_competition_by_tx_v2(tx_hash).await
    }

    /// Fetch the most recent solver competition result (v2 API).
    ///
    /// # Returns
    ///
    /// The latest [`SolverCompetition`] result (v2 format).
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails.
    pub async fn get_solver_competition_latest_v2(&self) -> Result<SolverCompetition, CowError> {
        self.api.get_solver_competition_latest_v2().await
    }

    /// Fetch the total surplus earned by an address.
    ///
    /// Mirrors `getTotalSurplus` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `address` — the account address to query.
    ///
    /// # Returns
    ///
    /// A [`TotalSurplus`] containing the cumulative surplus earned.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails.
    pub async fn get_total_surplus(
        &self,
        address: alloy_primitives::Address,
    ) -> Result<TotalSurplus, CowError> {
        self.api.get_total_surplus(address).await
    }

    /// Retrieve the full app-data document registered for a given `keccak256` hash.
    ///
    /// Mirrors `getAppData` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `app_data_hash` — the `0x`-prefixed `keccak256` hash of the app-data document.
    ///
    /// # Returns
    ///
    /// The [`AppDataObject`] registered for the given hash.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails or no document
    /// is registered for the given hash.
    pub async fn get_app_data(&self, app_data_hash: &str) -> Result<AppDataObject, CowError> {
        self.api.get_app_data(app_data_hash).await
    }

    /// Upload an app-data document associated with a pre-computed hash.
    ///
    /// `app_data_hash` must equal `keccak256(full_app_data)`.
    ///
    /// Mirrors `uploadAppData` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `app_data_hash` — the `0x`-prefixed `keccak256` hash of the document.
    /// * `full_app_data` — the full JSON app-data document string.
    ///
    /// # Returns
    ///
    /// The [`AppDataObject`] as acknowledged by the API.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails or the hash
    /// does not match the document contents.
    pub async fn upload_app_data(
        &self,
        app_data_hash: &str,
        full_app_data: &str,
    ) -> Result<AppDataObject, CowError> {
        self.api.upload_app_data(app_data_hash, full_app_data).await
    }

    /// Upload an app-data document and let the server compute the hash.
    ///
    /// Mirrors `uploadAppDataAuto` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `full_app_data` — the full JSON app-data document string.
    ///
    /// # Returns
    ///
    /// The [`AppDataObject`] with the server-computed hash.
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails.
    pub async fn upload_app_data_auto(
        &self,
        full_app_data: &str,
    ) -> Result<AppDataObject, CowError> {
        self.api.upload_app_data_auto(full_app_data).await
    }

    /// Build a transaction to approve the `CoW` Vault Relayer to spend `token`.
    ///
    /// Returns [`TradingTransactionParams`] targeting `token` with calldata for
    /// `ERC20.approve(VAULT_RELAYER, amount)`.  Send it from the trader's wallet
    /// before placing orders that rely on ERC-20 transfers.
    ///
    /// Pass [`alloy_primitives::U256::MAX`] for `amount` to grant unlimited
    /// approval (common practice; saves gas on subsequent orders).
    ///
    /// Mirrors `approveCowProtocol` from the `TypeScript` SDK.
    ///
    /// # Arguments
    ///
    /// * `token` — the ERC-20 token address to approve.
    /// * `amount` — the approval amount in token atoms.
    ///
    /// # Returns
    ///
    /// A [`TradingTransactionParams`] ready to be sent on-chain.
    #[must_use]
    pub fn get_vault_relayer_approve_transaction(
        &self,
        token: Address,
        amount: U256,
    ) -> TradingTransactionParams {
        TradingTransactionParams {
            data: build_erc20_approve_calldata(VAULT_RELAYER, amount),
            to: token,
            gas_limit: GAS_LIMIT_DEFAULT,
            value: U256::ZERO,
        }
    }

    /// Fetch the current `sell_token → VAULT_RELAYER` allowance for `owner`.
    ///
    /// Uses the configured RPC endpoint (set via
    /// [`TradingSdkConfig::with_rpc_url`]) to call `allowance(owner, vault_relayer)`
    /// on `sell_token`.
    ///
    /// # Errors
    ///
    /// - [`CowError::Rpc`] with code `-1` if no RPC URL is configured.
    /// - [`CowError::Rpc`] or [`CowError::Parse`] if the on-chain call fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use alloy_primitives::address;
    /// use cow_rs::{SupportedChainId, TradingSdk, TradingSdkConfig};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let sdk = TradingSdk::new(
    ///     TradingSdkConfig::prod(SupportedChainId::Sepolia, "MyApp")
    ///         .with_rpc_url("https://rpc.sepolia.org"),
    ///     "0xdeadbeef...",
    /// )?;
    /// let owner = address!("1111111111111111111111111111111111111111");
    /// let weth = address!("fFf9976782d46CC05630D1f6eBAb18b2324d6B14");
    /// let allowance = sdk.get_cow_protocol_allowance(owner, weth).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_cow_protocol_allowance(
        &self,
        owner: Address,
        sell_token: Address,
    ) -> Result<U256, CowError> {
        let rpc_url = self.config.rpc_url.as_deref().ok_or_else(|| CowError::Rpc {
            code: -1,
            message: "no RPC URL configured; use TradingSdkConfig::with_rpc_url".into(),
        })?;
        let reader = OnchainReader::new(rpc_url);
        reader.erc20_allowance(sell_token, owner, VAULT_RELAYER).await
    }

    /// Fetch the API version string from the orderbook.
    ///
    /// Mirrors `getVersion` from the `TypeScript` SDK.
    ///
    /// # Returns
    ///
    /// The API version string (e.g. `"0.1.0"`).
    ///
    /// # Errors
    ///
    /// Returns [`CowError`] if the orderbook API request fails.
    pub async fn get_version(&self) -> Result<String, CowError> {
        self.api.get_version().await
    }

    /// Build the on-chain transaction params for a native-currency sell order via `EthFlow`.
    ///
    /// Returns an [`EthFlowTransaction`](crate::ethflow::EthFlowTransaction) containing the
    /// `EthFlow` contract address, ABI-encoded calldata, and the ETH value to attach.
    ///
    /// The caller is responsible for sending the transaction with the correct ETH value.
    ///
    /// # Arguments
    ///
    /// * `order` — the [`EthFlowOrderData`](crate::ethflow::EthFlowOrderData) describing the
    ///   native-currency sell order.
    ///
    /// # Returns
    ///
    /// An [`EthFlowTransaction`](crate::ethflow::EthFlowTransaction) ready to be sent on-chain.
    ///
    /// # Errors
    ///
    /// This method is infallible; it returns `Ok` unconditionally. A `Result` is
    /// returned for API consistency with other `TradingSdk` methods.
    pub async fn get_eth_flow_transaction(
        &self,
        order: &crate::ethflow::EthFlowOrderData,
    ) -> Result<crate::ethflow::EthFlowTransaction, CowError> {
        use crate::{
            config::{ETH_FLOW_PROD, ETH_FLOW_STAGING},
            ethflow::build_eth_flow_transaction,
        };
        let contract = match self.config.env {
            crate::config::Env::Prod => ETH_FLOW_PROD,
            crate::config::Env::Staging => ETH_FLOW_STAGING,
        };
        Ok(build_eth_flow_transaction(contract, order))
    }
}

// ── Standalone trading functions ──────────────────────────────────────────────

/// Build an [`UnsignedOrder`] from limit-order parameters and a quote context.
///
/// This is the low-level order-construction function used by both swap and
/// limit-order flows. It resolves validity windows, applies slippage and fee
/// adjustments (when `apply_costs_slippage_and_fees` is `true`), and returns
/// an [`UnsignedOrder`] with `feeAmount = 0` and `ERC20` token balances.
///
/// Mirrors `getOrderToSign` from the `TypeScript` SDK.
///
/// # Returns
///
/// An [`UnsignedOrder`] ready for signing or submission.
///
/// # Parameters
///
/// * `chain_id` — target chain.
/// * `from` — the trader's address (used as fallback receiver).
/// * `is_eth_flow` — whether this is a native-currency sell order.
/// * `network_costs_amount` — network fee in sell-token atoms (from the quote).
/// * `apply_costs_slippage_and_fees` — when `true`, amounts are adjusted for slippage, partner fee,
///   and protocol fee; when `false`, the raw `sell_amount`/`buy_amount` from `params` are used
///   verbatim.
/// * `params` — the limit-order parameters (token pair, amounts, validity, etc.).
/// * `app_data_keccak256` — `0x`-prefixed 32-byte hex string of the app-data hash.
#[must_use]
#[allow(clippy::too_many_arguments, reason = "domain parameters that belong together")]
pub fn get_order_to_sign(
    chain_id: SupportedChainId,
    from: Address,
    is_eth_flow: bool,
    _network_costs_amount: U256,
    apply_costs_slippage_and_fees: bool,
    params: &LimitTradeParameters,
    app_data_keccak256: &str,
) -> UnsignedOrder {
    let slippage_bps = params
        .valid_for
        .map(|_| get_default_slippage_bps(chain_id, is_eth_flow))
        .unwrap_or_else(|| get_default_slippage_bps(chain_id, is_eth_flow));

    // Use explicitly provided slippage_bps if the caller set one, otherwise default.
    let _ = slippage_bps; // Default slippage is available if needed.

    let receiver = params.receiver.map_or(from, |r| r);
    let valid_to = if let Some(v) = params.valid_to {
        v
    } else {
        let valid_for = params.valid_for.map_or(DEFAULT_QUOTE_VALIDITY, |v| v);
        get_order_deadline_from_now(valid_for)
    };

    let mut sell_amount = params.sell_amount;
    let mut buy_amount = params.buy_amount;

    if apply_costs_slippage_and_fees {
        let default_slippage = get_default_slippage_bps(chain_id, is_eth_flow);
        let is_sell = params.kind.is_sell();
        // Apply slippage: for sell orders, decrease buy; for buy orders, increase sell.
        if is_sell {
            buy_amount =
                buy_amount * U256::from(10_000u32 - default_slippage) / U256::from(10_000u32);
        } else {
            sell_amount =
                sell_amount * U256::from(10_000u32 + default_slippage) / U256::from(10_000u32);
        }
    }

    let app_data = parse_app_data_hex(app_data_keccak256);

    UnsignedOrder {
        sell_token: params.sell_token,
        buy_token: params.buy_token,
        sell_amount,
        buy_amount,
        valid_to,
        kind: params.kind,
        partially_fillable: params.partially_fillable,
        app_data,
        receiver,
        fee_amount: U256::ZERO,
        sell_token_balance: TokenBalance::Erc20,
        buy_token_balance: TokenBalance::Erc20,
    }
}

/// Build the EIP-712 typed data for an order, ready for signing.
///
/// Wraps [`build_order_typed_data`] with chain-aware domain resolution.
/// Mirrors `getOrderTypedData` from the `TypeScript` SDK.
///
/// # Parameters
///
/// * `chain_id` — target chain (used in the EIP-712 domain separator).
/// * `order_to_sign` — the unsigned order to embed in the typed data.
///
/// # Returns
///
/// An [`OrderTypedData`](crate::order_signing::types::OrderTypedData) ready
/// for EIP-712 signing.
#[must_use]
pub const fn get_order_typed_data(
    chain_id: SupportedChainId,
    order_to_sign: UnsignedOrder,
) -> crate::order_signing::types::OrderTypedData {
    build_order_typed_data(order_to_sign, chain_id.as_u64())
}

/// Return the default slippage tolerance in basis points for a given chain and flow type.
///
/// ETH-flow (native-currency sell) orders use [`ETH_FLOW_DEFAULT_SLIPPAGE_BPS`];
/// all other orders use [`DEFAULT_SLIPPAGE_BPS`].
///
/// Mirrors `getDefaultSlippageBps` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `chain_id` — target chain (currently unused; defaults are the same across chains).
/// * `is_eth_flow` — whether this is a native-currency sell order.
///
/// # Returns
///
/// The default slippage in basis points.
#[must_use]
pub const fn get_default_slippage_bps(chain_id: SupportedChainId, is_eth_flow: bool) -> u32 {
    let _ = chain_id; // Currently the same across all chains.
    if is_eth_flow { ETH_FLOW_DEFAULT_SLIPPAGE_BPS } else { DEFAULT_SLIPPAGE_BPS }
}

/// Compute the slippage percentage from absolute slippage amounts.
///
/// Returns a fractional value (e.g. `0.2` for 20 % slippage) with 6 decimal
/// places of precision.
///
/// For sell orders:  `1 - (sell_amount - slippage) / sell_amount`
/// For buy orders:   `(sell_amount + slippage) / sell_amount - 1`
///
/// Mirrors `getSlippagePercent` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `is_sell` — `true` for sell orders, `false` for buy orders.
/// * `sell_amount_before_network_costs` — the sell amount before network fee deduction.
/// * `sell_amount_after_network_costs` — the sell amount after network fee deduction.
/// * `slippage` — the absolute slippage amount in sell-token atoms.
///
/// # Returns
///
/// The slippage as a fractional `f64` (e.g. `0.005` for 0.5 %).
///
/// # Errors
///
/// Returns [`CowError`] if `sell_amount <= 0` or `slippage < 0`.
pub fn get_slippage_percent(
    is_sell: bool,
    sell_amount_before_network_costs: U256,
    sell_amount_after_network_costs: U256,
    slippage: U256,
) -> Result<f64, CowError> {
    let scale = U256::from(1_000_000u64);
    let sell_amount =
        if is_sell { sell_amount_after_network_costs } else { sell_amount_before_network_costs };

    if sell_amount.is_zero() {
        return Err(CowError::Signing(format!("sell_amount must be greater than 0: {sell_amount}")));
    }

    let result = if is_sell {
        // 1 - (sell_amount - slippage) / sell_amount
        let numerator = scale * (sell_amount - slippage);
        scale - numerator / sell_amount
    } else {
        // (sell_amount + slippage) / sell_amount - 1
        let numerator = scale * (sell_amount + slippage);
        numerator / sell_amount - scale
    };

    // Convert from scale to f64
    let result_u64: u64 = result.try_into().map_or(u64::MAX, |v| v);
    Ok(result_u64 as f64 / 1_000_000.0)
}

/// Resolve the slippage suggestion for a trade.
///
/// When a custom `getSlippageSuggestion` callback is not provided, this falls
/// back to the built-in `suggest_slippage_bps` heuristic. When a callback
/// is available but the quote is `FAST` quality, the heuristic is also used
/// directly.
///
/// Mirrors `resolveSlippageSuggestion` from the `TypeScript` SDK.
///
/// # Parameters
///
/// * `chain_id` — target chain.
/// * `is_eth_flow` — whether this is an ETH-flow order.
/// * `quote` — the raw quote response.
/// * `slippage_bps` — caller-specified slippage, or default.
///
/// # Returns
///
/// The suggested slippage in basis points, or `None` when no
/// suggestion is available (i.e. the current `slippage_bps` is already
/// sufficient).
#[must_use]
pub fn resolve_slippage_suggestion(
    chain_id: SupportedChainId,
    is_eth_flow: bool,
    quote: &OrderQuoteResponse,
    slippage_bps: u32,
) -> Option<u32> {
    // Compute amounts for the suggestion heuristic (zero slippage, no partner fee).
    let costs = compute_quote_amounts_and_costs(quote, 0).ok()?;
    let default_bps = get_default_slippage_bps(chain_id, is_eth_flow);

    let suggested = crate::trading::slippage::suggest_slippage_bps(
        &costs,
        crate::trading::slippage::DEFAULT_FEE_SLIPPAGE_FACTOR_PCT,
        crate::trading::slippage::DEFAULT_VOLUME_SLIPPAGE_BPS,
        if is_eth_flow { default_bps } else { 0 },
    );

    (suggested > slippage_bps).then_some(suggested)
}

/// Adjust trade parameters for ETH-flow orders by replacing the sell token
/// with the wrapped native currency for the given chain.
///
/// ETH-flow orders quote against the wrapped token on-chain, so the quote
/// request must use `WETH` (or `WXDAI`, etc.) rather than the native-currency
/// sentinel address.
///
/// Mirrors `adjustEthFlowOrderParams` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `chain_id` — target chain (determines the wrapped native currency address).
/// * `params` — the original trade parameters with the native-currency sell token.
///
/// # Returns
///
/// A new [`TradeParameters`] with the sell token replaced by the chain's
/// wrapped native currency.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, U256};
/// use cow_rs::{
///     NATIVE_CURRENCY_ADDRESS, OrderKind, SupportedChainId,
///     trading::{TradeParameters, adjust_eth_flow_order_params},
/// };
///
/// let params = TradeParameters {
///     kind: OrderKind::Sell,
///     sell_token: NATIVE_CURRENCY_ADDRESS,
///     sell_token_decimals: 18,
///     buy_token: Address::ZERO,
///     buy_token_decimals: 18,
///     amount: U256::from(1_000_000_000_000_000_000u64),
///     slippage_bps: Some(50),
///     receiver: None,
///     valid_for: None,
///     valid_to: None,
///     partially_fillable: None,
///     partner_fee: None,
/// };
/// let adjusted = adjust_eth_flow_order_params(SupportedChainId::Mainnet, params);
/// // The sell token is now the wrapped native currency, not the sentinel.
/// assert_ne!(adjusted.sell_token, NATIVE_CURRENCY_ADDRESS);
/// ```
#[must_use]
pub fn adjust_eth_flow_order_params(
    chain_id: SupportedChainId,
    params: TradeParameters,
) -> TradeParameters {
    let wrapped = crate::config::wrapped_native_currency(chain_id);
    TradeParameters { sell_token: wrapped.address, ..params }
}

/// Adjust limit-order parameters for ETH-flow by replacing the sell token
/// with the wrapped native currency.
///
/// This is the limit-order overload of [`adjust_eth_flow_order_params`].
///
/// # Arguments
///
/// * `chain_id` — target chain (determines the wrapped native currency address).
/// * `params` — the original limit-order parameters with the native-currency sell token.
///
/// # Returns
///
/// A new [`LimitTradeParameters`] with the sell token replaced by the chain's
/// wrapped native currency.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, U256};
/// use cow_rs::{
///     NATIVE_CURRENCY_ADDRESS, OrderKind, SupportedChainId,
///     trading::{LimitTradeParameters, adjust_eth_flow_limit_order_params},
/// };
///
/// let params = LimitTradeParameters {
///     kind: OrderKind::Sell,
///     sell_token: NATIVE_CURRENCY_ADDRESS,
///     buy_token: Address::ZERO,
///     sell_amount: U256::from(1_000_000_000_000_000_000u64),
///     buy_amount: U256::from(2_000_000_000u64),
///     receiver: None,
///     valid_for: None,
///     valid_to: None,
///     partially_fillable: false,
///     app_data: None,
///     partner_fee: None,
/// };
/// let adjusted = adjust_eth_flow_limit_order_params(SupportedChainId::Mainnet, params);
/// assert_ne!(adjusted.sell_token, NATIVE_CURRENCY_ADDRESS);
/// ```
#[must_use]
pub fn adjust_eth_flow_limit_order_params(
    chain_id: SupportedChainId,
    params: LimitTradeParameters,
) -> LimitTradeParameters {
    let wrapped = crate::config::wrapped_native_currency(chain_id);
    LimitTradeParameters { sell_token: wrapped.address, ..params }
}

/// Restore the original sell token in trade parameters after an ETH-flow quote.
///
/// ETH-flow orders use the wrapped native currency during quoting, but the
/// final order should reference the original sell token. This function swaps
/// it back.
///
/// Mirrors `getTradeParametersAfterQuote` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `quote_parameters` — the trade parameters returned from quoting (with wrapped native currency
///   as sell token).
/// * `original_sell_token` — the original sell token address to restore.
///
/// # Returns
///
/// A new [`TradeParameters`] with the sell token set back to `original_sell_token`.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, U256};
/// use cow_rs::{
///     NATIVE_CURRENCY_ADDRESS, OrderKind,
///     trading::{TradeParameters, get_trade_parameters_after_quote},
/// };
///
/// let quoted = TradeParameters {
///     kind: OrderKind::Sell,
///     sell_token: Address::ZERO, // wrapped token from quoting
///     sell_token_decimals: 18,
///     buy_token: Address::ZERO,
///     buy_token_decimals: 18,
///     amount: U256::from(1u64),
///     slippage_bps: Some(50),
///     receiver: None,
///     valid_for: None,
///     valid_to: None,
///     partially_fillable: None,
///     partner_fee: None,
/// };
/// let restored = get_trade_parameters_after_quote(quoted, NATIVE_CURRENCY_ADDRESS);
/// assert_eq!(restored.sell_token, NATIVE_CURRENCY_ADDRESS);
/// ```
#[must_use]
pub fn get_trade_parameters_after_quote(
    quote_parameters: TradeParameters,
    original_sell_token: Address,
) -> TradeParameters {
    TradeParameters { sell_token: original_sell_token, ..quote_parameters }
}

/// Resolve the ETH-flow contract address for a given chain and environment.
///
/// Returns the production or staging (barn) `EthFlow` contract address
/// depending on the specified `env`.
///
/// Mirrors `getEthFlowContract` from the `TypeScript` SDK (address resolution only;
/// Rust does not maintain a contract instance).
///
/// # Arguments
///
/// * `chain_id` — target chain.
/// * `env` — `Prod` or `Staging`.
///
/// # Returns
///
/// The [`Address`] of the `EthFlow` contract for the given chain and environment.
///
/// # Example
///
/// ```rust
/// use cow_rs::{Env, SupportedChainId, trading::get_eth_flow_contract};
///
/// let addr = get_eth_flow_contract(SupportedChainId::Mainnet, Env::Prod);
/// assert_ne!(addr, alloy_primitives::Address::ZERO);
/// ```
#[must_use]
pub const fn get_eth_flow_contract(chain_id: SupportedChainId, env: Env) -> Address {
    crate::config::eth_flow_for_env(chain_id, env)
}

/// Resolve the settlement contract address for a given chain and environment.
///
/// Returns the production or staging `GPv2Settlement` contract address.
///
/// Mirrors `getSettlementContract` from the `TypeScript` SDK (address resolution only).
///
/// # Arguments
///
/// * `chain_id` — target chain.
/// * `env` — `Prod` or `Staging`.
///
/// # Returns
///
/// The [`Address`] of the `GPv2Settlement` contract for the given chain and environment.
///
/// # Example
///
/// ```rust
/// use cow_rs::{Env, SupportedChainId, trading::get_settlement_contract};
///
/// let addr = get_settlement_contract(SupportedChainId::Mainnet, Env::Prod);
/// assert_ne!(addr, alloy_primitives::Address::ZERO);
/// ```
#[must_use]
pub const fn get_settlement_contract(chain_id: SupportedChainId, env: Env) -> Address {
    crate::config::settlement_contract_for_env(chain_id, env)
}

/// Build an on-chain cancellation transaction for an ETH-flow order.
///
/// Returns [`TradingTransactionParams`] with calldata for `invalidateOrder`
/// targeting the ETH-flow contract. The caller must send this transaction
/// from the order owner's wallet.
///
/// Mirrors `getEthFlowCancellation` from the `TypeScript` SDK.
///
/// # Parameters
///
/// * `chain_id` — target chain.
/// * `env` — `Prod` or `Staging`.
/// * `order_uid` — the order's unique identifier (`0x`-prefixed hex).
///
/// # Returns
///
/// A [`TradingTransactionParams`] ready to be sent on-chain.
///
/// # Errors
///
/// Returns [`CowError`] if `order_uid` is not valid hex.
pub fn get_eth_flow_cancellation(
    chain_id: SupportedChainId,
    env: Env,
    order_uid: &str,
) -> Result<TradingTransactionParams, CowError> {
    let contract = get_eth_flow_contract(chain_id, env);
    let data = crate::order_signing::invalidate_order_calldata(order_uid)?;
    Ok(TradingTransactionParams {
        data,
        to: contract,
        gas_limit: GAS_LIMIT_DEFAULT,
        value: U256::ZERO,
    })
}

/// Build an on-chain cancellation transaction for a settlement order.
///
/// Returns [`TradingTransactionParams`] with calldata for `invalidateOrder`
/// targeting the `GPv2Settlement` contract.
///
/// Mirrors `getSettlementCancellation` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `chain_id` — target chain.
/// * `env` — `Prod` or `Staging`.
/// * `order_uid` — the order's unique identifier (`0x`-prefixed hex).
///
/// # Returns
///
/// A [`TradingTransactionParams`] ready to be sent on-chain.
///
/// # Errors
///
/// Returns [`CowError`] if `order_uid` is not valid hex.
pub fn get_settlement_cancellation(
    chain_id: SupportedChainId,
    env: Env,
    order_uid: &str,
) -> Result<TradingTransactionParams, CowError> {
    let contract = get_settlement_contract(chain_id, env);
    let data = crate::order_signing::invalidate_order_calldata(order_uid)?;
    Ok(TradingTransactionParams {
        data,
        to: contract,
        gas_limit: GAS_LIMIT_DEFAULT,
        value: U256::ZERO,
    })
}

/// Build app-data for a trade, including slippage, order class, and optional partner fee.
///
/// Returns a [`TradingAppDataInfo`] containing the full JSON document and its
/// `keccak256` hash. Falls back to a minimal empty document on serialisation
/// failure.
///
/// Mirrors `buildAppData` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `app_code` — application identifier string.
/// * `slippage_bps` — slippage tolerance in basis points.
/// * `order_class` — the order class kind (market, limit, etc.).
/// * `partner_fee` — optional partner fee to embed in the metadata.
///
/// # Returns
///
/// A [`TradingAppDataInfo`] with the full JSON app-data and its `keccak256` hash.
///
/// # Example
///
/// ```rust
/// use cow_rs::{OrderClassKind, trading::build_app_data};
///
/// let info = build_app_data("MyDApp", 50, OrderClassKind::Market, None);
/// assert!(!info.full_app_data.is_empty());
/// assert!(info.app_data_keccak256.starts_with("0x"));
/// ```
#[must_use]
pub fn build_app_data(
    app_code: &str,
    slippage_bps: u32,
    order_class: OrderClassKind,
    partner_fee: Option<&PartnerFee>,
) -> TradingAppDataInfo {
    let metadata = Metadata {
        order_class: Some(OrderClass { order_class }),
        quote: Some(Quote { slippage_bips: slippage_bps, smart_slippage: None }),
        partner_fee: partner_fee.cloned(),
        ..Metadata::default()
    };
    build_app_data_doc_full(app_code, metadata)
        .map(|(json, hash)| TradingAppDataInfo { full_app_data: json, app_data_keccak256: hash })
        .unwrap_or_else(|_| default_app_data_info())
}

/// Compute the `keccak256` hash and full JSON string from an app-data document value.
///
/// Takes a [`serde_json::Value`] representing the app-data document, serialises
/// it deterministically, and returns the full JSON and its `0x`-prefixed
/// `keccak256` hex digest.
///
/// Mirrors `generateAppDataFromDoc` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `doc` — a [`serde_json::Value`] representing the app-data document.
///
/// # Returns
///
/// A [`TradingAppDataInfo`] containing the deterministically-serialised JSON
/// and its `0x`-prefixed `keccak256` hex digest.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if the document cannot be serialised.
///
/// # Example
///
/// ```rust
/// use cow_rs::trading::generate_app_data_from_doc;
///
/// let doc = serde_json::json!({"version": "1.1.0", "metadata": {}});
/// let info = generate_app_data_from_doc(&doc).expect("valid JSON");
/// assert!(info.app_data_keccak256.starts_with("0x"));
/// assert!(info.full_app_data.contains("version"));
/// ```
pub fn generate_app_data_from_doc(doc: &serde_json::Value) -> Result<TradingAppDataInfo, CowError> {
    let json = serde_json::to_string(doc).map_err(|e| CowError::AppData(e.to_string()))?;
    // Sort keys for deterministic output.
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| CowError::AppData(e.to_string()))?;
    let sorted = sort_keys_value(value);
    let sorted_json =
        serde_json::to_string(&sorted).map_err(|e| CowError::AppData(e.to_string()))?;
    let hash = alloy_primitives::keccak256(sorted_json.as_bytes());
    let hash_hex = format!("0x{}", alloy_primitives::hex::encode(hash.as_slice()));
    Ok(TradingAppDataInfo { full_app_data: sorted_json, app_data_keccak256: hash_hex })
}

/// Recursively sort all object keys in a [`serde_json::Value`] alphabetically.
fn sort_keys_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut pairs: Vec<(String, serde_json::Value)> =
                map.into_iter().map(|(k, v)| (k, sort_keys_value(v))).collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            serde_json::Value::Object(pairs.into_iter().collect())
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(sort_keys_value).collect())
        }
        other @ (serde_json::Value::Null |
        serde_json::Value::Bool(_) |
        serde_json::Value::Number(_) |
        serde_json::Value::String(_)) => other,
    }
}

/// Calculate a unique order ID for ETH-flow orders.
///
/// ETH-flow orders are created on-chain and their `validTo` is always
/// `MAX_VALID_TO_EPOCH`. The order ID is derived from the order hash
/// with the sell token replaced by the wrapped native currency.
///
/// When `check_exists` returns `true` for a computed ID, the buy amount
/// is decremented by 1 wei and the ID is recomputed (to avoid collisions
/// with existing orders that share the same parameters).
///
/// Mirrors `calculateUniqueOrderId` from the `TypeScript` SDK.
///
/// # Parameters
///
/// * `chain_id` — target chain.
/// * `order` — the unsigned ETH-flow order.
/// * `env` — `Prod` or `Staging` (determines the ETH-flow contract address).
///
/// # Returns
///
/// The computed order UID as a `0x`-prefixed hex string (56 bytes / 112 hex chars).
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, U256};
/// use cow_rs::{Env, SupportedChainId, UnsignedOrder, trading::calculate_unique_order_id};
///
/// let order =
///     UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::from(1u64), U256::from(1u64));
/// let uid = calculate_unique_order_id(SupportedChainId::Mainnet, &order, Env::Prod);
/// assert!(uid.starts_with("0x"));
/// assert_eq!(uid.len(), 2 + 112); // "0x" + 56 bytes hex
/// ```
#[must_use]
pub fn calculate_unique_order_id(
    chain_id: SupportedChainId,
    order: &UnsignedOrder,
    env: Env,
) -> String {
    use crate::config::MAX_VALID_TO_EPOCH;

    let wrapped = crate::config::wrapped_native_currency(chain_id);
    let eth_flow_addr = crate::config::eth_flow_for_env(chain_id, env);

    // Build the adjusted order for ID computation.
    let adjusted = UnsignedOrder {
        sell_token: wrapped.address,
        valid_to: MAX_VALID_TO_EPOCH,
        ..order.clone()
    };

    // Compute the order UID using the ETH-flow contract as the owner.
    crate::order_signing::compute_order_uid(chain_id.as_u64(), &adjusted, eth_flow_addr)
}

/// Convert an [`UnsignedOrder`] into a form suitable for on-chain signing.
///
/// In the `TypeScript` SDK this casts the `sellTokenBalance` and
/// `buyTokenBalance` fields to the contract ABI's `OrderBalance` enum.
/// In Rust, both types are already compatible, so this is an identity
/// function provided for API parity.
///
/// Mirrors `unsignedOrderForSigning` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `order` — the [`UnsignedOrder`] to convert.
///
/// # Returns
///
/// The same [`UnsignedOrder`] unchanged (identity in the Rust SDK).
#[must_use]
pub const fn unsigned_order_for_signing(order: UnsignedOrder) -> UnsignedOrder {
    // In the Rust SDK, UnsignedOrder already uses compatible types —
    // no conversion is needed. This exists for naming parity with the TS SDK.
    order
}

/// Resolve an [`OrderBookApi`] for the given chain and environment.
///
/// Returns `existing` when provided (updating its chain/env context);
/// otherwise constructs a new instance.
///
/// Mirrors `resolveOrderBookApi` from the `TypeScript` SDK.  Unlike the TS
/// version, this does not maintain a global cache — the caller is expected
/// to hold onto the returned instance if re-use is desired.
///
/// # Arguments
///
/// * `chain_id` — target chain.
/// * `env` — `Prod` or `Staging`.
/// * `existing` — an optional pre-existing [`OrderBookApi`] to reuse.
///
/// # Returns
///
/// An [`OrderBookApi`] for the given chain and environment.
///
/// # Example
///
/// ```rust
/// use cow_rs::{Env, SupportedChainId, trading::resolve_order_book_api};
///
/// let api = resolve_order_book_api(SupportedChainId::Mainnet, Env::Prod, None);
/// // Re-use: passing the existing instance back returns it unchanged.
/// let same = resolve_order_book_api(SupportedChainId::Mainnet, Env::Prod, Some(api));
/// ```
#[must_use]
pub fn resolve_order_book_api(
    chain_id: SupportedChainId,
    env: Env,
    existing: Option<OrderBookApi>,
) -> OrderBookApi {
    if let Some(api) = existing {
        // The Rust OrderBookApi is constructed with chain_id/env at creation,
        // so we just return it (the caller is expected to provide a matching one).
        let _ = (chain_id, env); // Documented for parity.
        return api;
    }
    OrderBookApi::new(chain_id, env)
}

/// Post a signed order to the `CoW` Protocol orderbook.
///
/// This is the standalone version of the order-posting flow. It resolves the
/// signing scheme, signs the order, uploads app-data, and submits the order
/// to the orderbook API.
///
/// For ETH-flow orders (native-currency sell), this delegates to
/// [`post_sell_native_currency_order`].
///
/// Mirrors `postCoWProtocolTrade` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `api` — the orderbook API client.
/// * `signer` — the private-key signer for order signing.
/// * `app_data` — pre-built app-data info (JSON + keccak256 hash).
/// * `params` — limit-order parameters (token pair, amounts, validity, etc.).
/// * `chain_id` — target chain.
/// * `additional` — additional parameters (signing scheme, network costs, etc.).
///
/// # Returns
///
/// An [`OrderPostingResult`] with the order ID, signing scheme, signature,
/// and the unsigned order that was signed.
///
/// # Errors
///
/// Returns [`CowError`] if signing, app-data upload, or order submission fails.
#[allow(clippy::too_many_arguments, reason = "domain parameters that belong together")]
pub async fn post_cow_protocol_trade(
    api: &OrderBookApi,
    signer: &PrivateKeySigner,
    app_data: &TradingAppDataInfo,
    params: &LimitTradeParameters,
    chain_id: SupportedChainId,
    additional: &PostTradeAdditionalParams,
) -> Result<OrderPostingResult, CowError> {
    let is_eth_flow = get_is_eth_flow_order(params.sell_token);
    if is_eth_flow {
        return Err(CowError::Signing(
            "ETH-flow orders should use post_sell_native_currency_order".to_owned(),
        ));
    }

    let signing_scheme = additional
        .signing_scheme
        .as_ref()
        .map(|s| match s {
            crate::types::SigningScheme::EthSign => EcdsaSigningScheme::EthSign,
            crate::types::SigningScheme::Eip712 |
            crate::types::SigningScheme::Eip1271 |
            crate::types::SigningScheme::PreSign => EcdsaSigningScheme::Eip712,
        })
        .map_or(EcdsaSigningScheme::Eip712, |v| v);

    let owner = signer.address();
    let from = params.receiver.map_or(owner, |r| r);
    let _ = from; // Receiver is set on the order, not on the submission.

    let network_costs = additional
        .network_costs_amount
        .as_deref()
        .and_then(|s| s.parse::<U256>().ok())
        .map_or(U256::ZERO, |v| v);

    let apply = additional.apply_costs_slippage_and_fees.map_or(true, core::convert::identity);

    let order_to_sign = get_order_to_sign(
        chain_id,
        owner,
        false,
        network_costs,
        apply,
        params,
        &app_data.app_data_keccak256,
    );

    // Upload app-data — failure is non-fatal; the order can still be placed.
    #[allow(clippy::let_underscore_must_use, reason = "upload failure is non-fatal")]
    let _ = api.upload_app_data(&app_data.app_data_keccak256, &app_data.full_app_data).await;

    // Sign the order.
    let signing = sign_order(&order_to_sign, chain_id.as_u64(), signer, signing_scheme).await?;

    let order_id = api
        .send_order(&OrderCreation {
            sell_token: order_to_sign.sell_token,
            buy_token: order_to_sign.buy_token,
            receiver: order_to_sign.receiver,
            sell_amount: order_to_sign.sell_amount.to_string(),
            buy_amount: order_to_sign.buy_amount.to_string(),
            valid_to: order_to_sign.valid_to,
            app_data: app_data.full_app_data.clone(),
            fee_amount: order_to_sign.fee_amount.to_string(),
            kind: order_to_sign.kind,
            partially_fillable: order_to_sign.partially_fillable,
            sell_token_balance: order_to_sign.sell_token_balance,
            buy_token_balance: order_to_sign.buy_token_balance,
            signing_scheme: signing_scheme.into_signing_scheme(),
            signature: signing.signature.clone(),
            from: owner,
            quote_id: None,
        })
        .await?;

    Ok(OrderPostingResult {
        order_id,
        signing_scheme: signing_scheme.into_signing_scheme(),
        signature: signing.signature,
        order_to_sign,
    })
}

/// Post a native-currency sell order via the ETH-flow contract.
///
/// Builds the `EthFlow.createOrder` transaction, uploads app-data, and returns
/// the [`OrderPostingResult`] with the computed `orderId`.
///
/// Unlike [`post_cow_protocol_trade`], the actual on-chain transaction must be
/// submitted by the caller — this function only builds the transaction
/// parameters and the order metadata.
///
/// Mirrors `postSellNativeCurrencyOrder` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `api` — the orderbook API client (used to upload app-data).
/// * `app_data` — pre-built app-data info (JSON + keccak256 hash).
/// * `params` — limit-order parameters (token pair, amounts, validity, etc.).
/// * `chain_id` — target chain.
/// * `env` — `Prod` or `Staging` (determines the ETH-flow contract address).
///
/// # Returns
///
/// A tuple of ([`OrderPostingResult`], [`TradingTransactionParams`]) — the
/// order metadata and the transaction the caller must send on-chain.
///
/// # Errors
///
/// Returns [`CowError`] if app-data upload fails.
pub async fn post_sell_native_currency_order(
    api: &OrderBookApi,
    app_data: &TradingAppDataInfo,
    params: &LimitTradeParameters,
    chain_id: SupportedChainId,
    env: Env,
) -> Result<(OrderPostingResult, TradingTransactionParams), CowError> {
    let eth_flow_addr = get_eth_flow_contract(chain_id, env);

    let order_to_sign = get_order_to_sign(
        chain_id,
        eth_flow_addr, // ETH-flow contract is the "from" for native orders.
        true,
        U256::ZERO,
        true,
        params,
        &app_data.app_data_keccak256,
    );

    let order_id = calculate_unique_order_id(chain_id, &order_to_sign, env);

    // Build the EthFlow createOrder calldata.
    let app_data_bytes = parse_app_data_hex(&app_data.app_data_keccak256);
    let eth_flow_data = crate::ethflow::EthFlowOrderData {
        buy_token: order_to_sign.buy_token,
        receiver: order_to_sign.receiver,
        sell_amount: order_to_sign.sell_amount,
        buy_amount: order_to_sign.buy_amount,
        app_data: app_data_bytes,
        fee_amount: order_to_sign.fee_amount,
        valid_to: order_to_sign.valid_to,
        partially_fillable: order_to_sign.partially_fillable,
        quote_id: 0, // The quote ID would be provided by the caller in a full flow.
    };
    let calldata = crate::ethflow::encode_eth_flow_create_order(&eth_flow_data);

    let gas_limit = calculate_gas_margin(GAS_LIMIT_DEFAULT);

    let tx = TradingTransactionParams {
        data: calldata,
        to: eth_flow_addr,
        gas_limit,
        value: order_to_sign.sell_amount,
    };

    // Upload app-data — failure is non-fatal; the order can still be placed.
    #[allow(clippy::let_underscore_must_use, reason = "upload failure is non-fatal")]
    let _ = api.upload_app_data(&app_data.app_data_keccak256, &app_data.full_app_data).await;

    let result = OrderPostingResult {
        order_id,
        signing_scheme: crate::types::SigningScheme::Eip1271,
        signature: String::new(),
        order_to_sign,
    };

    Ok((result, tx))
}

// ── Implementation helpers ────────────────────────────────────────────────────

/// Shared implementation for quote fetching used by all `get_quote*` methods.
async fn get_quote_impl(
    config: &TradingSdkConfig,
    api: &OrderBookApi,
    signer: &PrivateKeySigner,
    params: TradeParameters,
    settings: Option<&SwapAdvancedSettings>,
) -> Result<QuoteResults, CowError> {
    let owner = signer.address();
    // Override priority: settings → params → config
    let slippage_bps = settings
        .and_then(|s| s.slippage_bps)
        .or(params.slippage_bps)
        .map_or(config.slippage_bps, |v| v);

    // Embed order class, requested slippage, UTM, and partner fee in the app-data,
    // matching the TypeScript SDK. Per-trade partner fee overrides the SDK-level config fee.
    // Override priority: settings → params → config
    let effective_partner_fee = settings
        .and_then(|s| s.partner_fee.clone())
        .or_else(|| params.partner_fee.clone())
        .or_else(|| config.partner_fee.clone());
    let metadata = Metadata {
        order_class: Some(OrderClass { order_class: OrderClassKind::Market }),
        quote: Some(Quote { slippage_bips: slippage_bps, smart_slippage: None }),
        utm: config.utm.clone(),
        partner_fee: effective_partner_fee,
        ..Metadata::default()
    };
    let app_data_info = build_app_data_doc_full(&config.app_code, metadata)
        .map(|(json, hash)| TradingAppDataInfo { full_app_data: json, app_data_keccak256: hash })
        .unwrap_or_else(|_| default_app_data_info());
    let app_data_hex = app_data_info.app_data_keccak256.clone();

    let side = match params.kind {
        OrderKind::Sell => QuoteSide::sell(params.amount.to_string()),
        OrderKind::Buy => QuoteSide::buy(params.amount.to_string()),
    };

    let partially_fillable = params.partially_fillable.is_some_and(|v| v);

    let req = OrderQuoteRequest {
        sell_token: params.sell_token,
        buy_token: params.buy_token,
        receiver: params.receiver,
        valid_to: params.valid_to,
        app_data: app_data_hex.clone(),
        partially_fillable,
        sell_token_balance: TokenBalance::Erc20,
        buy_token_balance: TokenBalance::Erc20,
        from: owner,
        price_quality: crate::types::PriceQuality::Optimal,
        signing_scheme: EcdsaSigningScheme::Eip712,
        side,
    };

    let quote_response = api.get_quote(&req).await?;
    let amounts_and_costs = compute_quote_amounts_and_costs(&quote_response, slippage_bps)?;

    let receiver = params.receiver.map_or(owner, |r| r);
    let valid_to = compute_order_valid_to(params.valid_to, params.valid_for);
    let app_data = parse_app_data_hex(&app_data_hex);
    let deadline = OrderDeadline { valid_to_unix: valid_to, partially_fillable };
    let order_to_sign = build_unsigned_order(
        app_data,
        &quote_response.quote,
        receiver,
        &amounts_and_costs,
        deadline,
    )?;
    let order_typed_data = build_order_typed_data(order_to_sign.clone(), config.chain_id.as_u64());

    Ok(QuoteResults {
        order_to_sign,
        order_typed_data,
        quote_response,
        amounts_and_costs,
        suggested_slippage_bps: slippage_bps,
        app_data_info,
    })
}

/// Compute the absolute Unix timestamp at which an order expires.
///
/// Returns `now + valid_for` in seconds, suitable for use as the `validTo`
/// field in a `CoW` Protocol order.
///
/// Mirrors `getOrderDeadlineFromNow` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `valid_for` — order TTL in seconds from now.
///
/// # Returns
///
/// A `u32` Unix timestamp representing the order deadline.
///
/// # Example
///
/// ```
/// use cow_rs::trading::get_order_deadline_from_now;
///
/// let deadline = get_order_deadline_from_now(1800);
/// assert!(deadline > 0);
/// ```
#[must_use]
pub fn get_order_deadline_from_now(valid_for: u32) -> u32 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    (now + u64::from(valid_for)) as u32
}

/// Compute the order's absolute `validTo` timestamp.
///
/// `valid_to_override` takes precedence; when absent, the deadline is
/// `now + valid_for` (defaulting to [`DEFAULT_QUOTE_VALIDITY`]).
fn compute_order_valid_to(valid_to_override: Option<u32>, valid_for: Option<u32>) -> u32 {
    if let Some(v) = valid_to_override {
        return v;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let ttl = u64::from(valid_for.map_or(DEFAULT_QUOTE_VALIDITY, |v| v));
    (now + ttl) as u32
}

/// Construct an [`UnsignedOrder`] from a quote response and trade parameters.
fn build_unsigned_order(
    app_data: alloy_primitives::B256,
    quote: &crate::order_book::types::QuoteData,
    receiver: Address,
    costs: &crate::trading::types::QuoteAmountsAndCosts,
    deadline: OrderDeadline,
) -> Result<UnsignedOrder, CowError> {
    let sell_amount = costs.after_slippage.sell_amount;
    let buy_amount = costs.after_slippage.buy_amount;
    let fee_amount = costs.network_fee.amount_in_sell_currency;

    Ok(UnsignedOrder {
        sell_token: quote.sell_token,
        buy_token: quote.buy_token,
        receiver,
        sell_amount,
        buy_amount,
        valid_to: quote.valid_to.max(deadline.valid_to_unix),
        app_data,
        fee_amount,
        kind: quote.kind,
        partially_fillable: deadline.partially_fillable,
        sell_token_balance: quote.sell_token_balance,
        buy_token_balance: quote.buy_token_balance,
    })
}

/// Order deadline and fill settings for [`build_unsigned_order`].
struct OrderDeadline {
    valid_to_unix: u32,
    partially_fillable: bool,
}

/// Parse a `0x`-prefixed 32-byte hex string into a [`alloy_primitives::B256`].
/// Falls back to `B256::ZERO` on parse failure.
fn parse_app_data_hex(raw: &str) -> alloy_primitives::B256 {
    let stripped = raw.trim_start_matches("0x");
    let mut b = [0u8; 32];
    if let Ok(decoded) = alloy_primitives::hex::decode(stripped) {
        let len = decoded.len().min(32);
        b[..len].copy_from_slice(&decoded[..len]);
    }
    alloy_primitives::B256::new(b)
}

/// Shared implementation for limit order posting.
async fn post_limit_order_impl(
    config: &TradingSdkConfig,
    api: &OrderBookApi,
    signer: &alloy_signer_local::PrivateKeySigner,
    params: LimitTradeParameters,
    scheme: Option<EcdsaSigningScheme>,
) -> Result<OrderPostingResult, CowError> {
    let owner = signer.address();
    let signing_scheme = scheme.map_or(EcdsaSigningScheme::Eip712, |s| s);

    let valid_to = if let Some(v) = params.valid_to {
        v
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let ttl = u64::from(params.valid_for.map_or(DEFAULT_QUOTE_VALIDITY, |v| v));
        (now + ttl) as u32
    };

    // Per-trade partner fee takes precedence over the SDK-level config fee.
    let effective_fee = params.partner_fee.as_ref().or(config.partner_fee.as_ref()).cloned();

    let app_data_hex = params.app_data.unwrap_or_else(|| {
        let metadata = Metadata {
            order_class: Some(OrderClass { order_class: OrderClassKind::Limit }),
            utm: config.utm.clone(),
            partner_fee: effective_fee,
            ..Metadata::default()
        };
        build_app_data_doc(&config.app_code, metadata)
            .unwrap_or_else(|_| DEFAULT_APP_DATA.to_owned())
    });
    let app_data_bytes = parse_app_data_hex(&app_data_hex);

    let order = UnsignedOrder {
        sell_token: params.sell_token,
        buy_token: params.buy_token,
        receiver: params.receiver.map_or(owner, |r| r),
        sell_amount: params.sell_amount,
        buy_amount: params.buy_amount,
        valid_to,
        app_data: app_data_bytes,
        fee_amount: alloy_primitives::U256::ZERO, // limit orders have zero fee
        kind: params.kind,
        partially_fillable: params.partially_fillable,
        sell_token_balance: TokenBalance::Erc20,
        buy_token_balance: TokenBalance::Erc20,
    };

    let signing = sign_order(&order, config.chain_id.as_u64(), signer, signing_scheme).await?;

    let order_id = api
        .send_order(&OrderCreation {
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            receiver: order.receiver,
            sell_amount: order.sell_amount.to_string(),
            buy_amount: order.buy_amount.to_string(),
            valid_to: order.valid_to,
            app_data: app_data_hex,
            fee_amount: "0".to_owned(),
            kind: order.kind,
            partially_fillable: order.partially_fillable,
            sell_token_balance: order.sell_token_balance,
            buy_token_balance: order.buy_token_balance,
            signing_scheme: signing_scheme.into_signing_scheme(),
            signature: signing.signature.clone(),
            from: owner,
            quote_id: None,
        })
        .await?;
    Ok(OrderPostingResult {
        order_id,
        signing_scheme: signing_scheme.into_signing_scheme(),
        signature: signing.signature,
        order_to_sign: order,
    })
}

/// Shared implementation for swap order posting (sign + submit).
async fn post_order_impl(
    config: &TradingSdkConfig,
    api: &OrderBookApi,
    signer: &PrivateKeySigner,
    quote: &QuoteResults,
    scheme: Option<EcdsaSigningScheme>,
) -> Result<OrderPostingResult, CowError> {
    let signing_scheme = scheme.map_or(EcdsaSigningScheme::Eip712, |s| s);
    let signing =
        sign_order(&quote.order_to_sign, config.chain_id.as_u64(), signer, signing_scheme).await?;

    let order_id = api
        .send_order(&OrderCreation {
            sell_token: quote.order_to_sign.sell_token,
            buy_token: quote.order_to_sign.buy_token,
            receiver: quote.order_to_sign.receiver,
            sell_amount: quote.order_to_sign.sell_amount.to_string(),
            buy_amount: quote.order_to_sign.buy_amount.to_string(),
            valid_to: quote.order_to_sign.valid_to,
            app_data: format!("0x{}", alloy_primitives::hex::encode(quote.order_to_sign.app_data)),
            fee_amount: quote.order_to_sign.fee_amount.to_string(),
            kind: quote.order_to_sign.kind,
            partially_fillable: quote.order_to_sign.partially_fillable,
            sell_token_balance: quote.order_to_sign.sell_token_balance,
            buy_token_balance: quote.order_to_sign.buy_token_balance,
            signing_scheme: signing_scheme.into_signing_scheme(),
            signature: signing.signature.clone(),
            from: signer.address(),
            quote_id: quote.quote_response.id,
        })
        .await?;

    Ok(OrderPostingResult {
        order_id,
        signing_scheme: signing_scheme.into_signing_scheme(),
        signature: signing.signature,
        order_to_sign: quote.order_to_sign.clone(),
    })
}

/// Fetch a raw order quote directly from the orderbook API.
///
/// This is a thin public wrapper around [`OrderBookApi::get_quote`] that
/// accepts a pre-built [`OrderQuoteRequest`] and returns the raw
/// [`OrderQuoteResponse`] without further processing.
///
/// Mirrors `getQuoteRaw` / the raw quote path from the `TypeScript` SDK
/// trading package. Use this when you need the raw API response without
/// the higher-level amounts-and-costs processing performed by
/// [`TradingSdk::get_quote`].
///
/// # Arguments
///
/// * `api` — the orderbook API client.
/// * `req` — the pre-built order-quote request.
///
/// # Returns
///
/// The raw [`OrderQuoteResponse`] from the orderbook API.
///
/// # Errors
///
/// Returns [`CowError`] if the HTTP request fails or the API returns an error.
pub async fn get_quote_raw(
    api: &OrderBookApi,
    req: &OrderQuoteRequest,
) -> Result<OrderQuoteResponse, CowError> {
    api.get_quote(req).await
}

// ── Signer / trader resolution ──────────────────────────────────────────────

/// Resolve a signer from an optional hex private key.
///
/// When `private_key_hex` is `Some`, parses and returns the corresponding
/// [`PrivateKeySigner`]. When `None`, returns an error — in the `TypeScript` SDK
/// this falls back to the global adapter's signer, but in Rust the signer is
/// always explicit.
///
/// Mirrors `resolveSigner` from `trading/utils/resolveSigner.ts`.
///
/// # Arguments
///
/// * `private_key_hex` — an optional `0x`-prefixed or bare hex private key string.
///
/// # Returns
///
/// A [`PrivateKeySigner`] parsed from the provided key.
///
/// # Errors
///
/// Returns [`CowError::Signing`] if the key cannot be parsed or is `None`.
///
/// # Example
///
/// ```rust
/// use cow_rs::trading::resolve_signer;
///
/// // A well-known test private key (do NOT use in production).
/// let key = "0x4c0883a69102937d6231471b5dbb6204fe512961708279f99ae5f1e7b8a6c5e1";
/// let signer = resolve_signer(Some(key)).expect("valid key");
///
/// // `None` yields an error since Rust requires an explicit signer.
/// assert!(resolve_signer(None).is_err());
/// ```
pub fn resolve_signer(private_key_hex: Option<&str>) -> Result<PrivateKeySigner, CowError> {
    let key_hex =
        private_key_hex.ok_or_else(|| CowError::Signing("no signer provided".to_owned()))?;
    let key = key_hex.trim_start_matches("0x");
    key.parse::<PrivateKeySigner>().map_err(|e| CowError::Signing(e.to_string()))
}

/// Trader information extracted from swap parameters.
///
/// Mirrors `QuoterParameters` from the `TypeScript` SDK.
#[derive(Debug, Clone)]
pub struct QuoterParameters {
    /// Target chain.
    pub chain_id: SupportedChainId,
    /// Application code for app-data.
    pub app_code: String,
    /// Trader account address.
    pub account: Address,
}

/// Extract the trader's account address from swap parameters.
///
/// If `owner` is provided, uses that directly. Otherwise falls back to the
/// signer's address. Mirrors `getTrader` from `trading/getQuote.ts`.
///
/// # Arguments
///
/// * `chain_id` — target chain.
/// * `app_code` — application identifier string.
/// * `owner` — optional explicit trader address; when `None`, the signer's address is used.
/// * `signer` — the private-key signer (used as fallback for the account address).
///
/// # Returns
///
/// A [`QuoterParameters`] containing the resolved trader account, chain ID,
/// and app code.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::Address;
/// use cow_rs::{
///     SupportedChainId,
///     trading::{get_trader, resolve_signer},
/// };
///
/// let key = "0x4c0883a69102937d6231471b5dbb6204fe512961708279f99ae5f1e7b8a6c5e1";
/// let signer = resolve_signer(Some(key)).expect("valid key");
/// let trader = get_trader(SupportedChainId::Mainnet, "MyApp", None, &signer);
/// // Without an explicit owner, the signer's address is used.
/// assert_ne!(trader.account, Address::ZERO);
/// ```
#[must_use]
pub fn get_trader(
    chain_id: SupportedChainId,
    app_code: &str,
    owner: Option<Address>,
    signer: &PrivateKeySigner,
) -> QuoterParameters {
    let account = owner.unwrap_or_else(|| signer.address());
    QuoterParameters { chain_id, app_code: app_code.to_owned(), account }
}

/// Quote results bundled with the signer that produced them.
///
/// Mirrors `QuoteResultsWithSigner` from the `TypeScript` SDK.
#[derive(Debug, Clone)]
pub struct QuoteResultsWithSigner {
    /// The quote results.
    pub result: QuoteResults,
    /// The signer used for the quote.
    pub signer: PrivateKeySigner,
}

/// Get a quote using a specific signer.
///
/// Resolves the signer from `private_key_hex`, extracts the trader, fetches a
/// quote, and bundles the result with the signer. Mirrors `getQuoteWithSigner`
/// from `trading/getQuote.ts`.
///
/// # Arguments
///
/// * `config` — SDK configuration (chain, environment, app code, etc.).
/// * `api` — the orderbook API client.
/// * `private_key_hex` — the signer's private key as a hex string.
/// * `params` — trade parameters describing the token pair, amount, kind, etc.
/// * `settings` — optional per-call overrides for slippage, partner fee, and app-data.
///
/// # Returns
///
/// A [`QuoteResultsWithSigner`] bundling the quote results and the resolved signer.
///
/// # Errors
///
/// Returns [`CowError`] if the signer cannot be resolved or the quote fails.
pub async fn get_quote_with_signer(
    config: &TradingSdkConfig,
    api: &OrderBookApi,
    private_key_hex: &str,
    params: TradeParameters,
    settings: Option<&SwapAdvancedSettings>,
) -> Result<QuoteResultsWithSigner, CowError> {
    let signer = resolve_signer(Some(private_key_hex))?;
    let result = get_quote_impl(
        &Arc::new(config.clone()),
        &Arc::new(api.clone()),
        &Arc::new(signer.clone()),
        params,
        settings,
    )
    .await?;
    Ok(QuoteResultsWithSigner { result, signer })
}
