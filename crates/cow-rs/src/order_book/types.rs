//! Orderbook API request and response types.
//!
//! Defines all structs used to communicate with the `CoW` Protocol orderbook
//! REST API: quote requests/responses, order creation/retrieval, trades,
//! auctions, solver competitions, and cancellation payloads.
//!
//! All types derive `Serialize` and/or `Deserialize` with `camelCase` field
//! names to match the API's JSON format.

use std::fmt;

use alloy_primitives::U256;
use foldhash::HashMap;
use serde::{Deserialize, Serialize};

use crate::{
    error::CowError,
    types::{EcdsaSigningScheme, OrderKind, PriceQuality, SigningScheme, TokenBalance},
};

// ── Quote ────────────────────────────────────────────────────────────────────

/// Request body for `POST /api/v1/quote`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderQuoteRequest {
    /// Token to sell.
    pub sell_token: alloy_primitives::Address,
    /// Token to buy.
    pub buy_token: alloy_primitives::Address,
    /// Who receives the `buyToken` (defaults to `from`).
    pub receiver: Option<alloy_primitives::Address>,
    /// Order expiry as Unix timestamp.  Omit to use `DEFAULT_QUOTE_VALIDITY`.
    pub valid_to: Option<u32>,
    /// `bytes32` keccak256 of the app-data JSON, or the JSON itself.
    pub app_data: String,
    /// Whether the order may be partially filled.
    pub partially_fillable: bool,
    /// Source of `sellToken` funds.
    pub sell_token_balance: TokenBalance,
    /// Destination of `buyToken` funds.
    pub buy_token_balance: TokenBalance,
    /// Address placing the order.
    pub from: alloy_primitives::Address,
    /// Price quality hint.
    pub price_quality: PriceQuality,
    /// Signing scheme to use when submitting this order.
    pub signing_scheme: EcdsaSigningScheme,
    /// Direction and amount — must contain exactly one variant.
    #[serde(flatten)]
    pub side: QuoteSide,
}

/// The directional "side" of a quote request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteSide {
    /// `"sell"` or `"buy"`.
    pub kind: OrderKind,
    /// Gross sell amount (before protocol fee) for `kind = "sell"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sell_amount_before_fee: Option<String>,
    /// Exact buy amount (after protocol fee) for `kind = "buy"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buy_amount_after_fee: Option<String>,
}

impl QuoteSide {
    /// Construct a sell-side request: sell exactly `amount` tokens (gross,
    /// before protocol fee).
    ///
    /// # Parameters
    ///
    /// * `amount` — the sell amount in token atoms (decimal string or integer).
    ///
    /// # Returns
    ///
    /// A [`QuoteSide`] with `kind = Sell` and `sell_amount_before_fee` set.
    #[must_use]
    pub fn sell(amount: impl ToString) -> Self {
        Self {
            kind: OrderKind::Sell,
            sell_amount_before_fee: Some(amount.to_string()),
            buy_amount_after_fee: None,
        }
    }

    /// Construct a buy-side request: receive exactly `amount` tokens
    /// (after protocol fee).
    ///
    /// # Parameters
    ///
    /// * `amount` — the buy amount in token atoms (decimal string or integer).
    ///
    /// # Returns
    ///
    /// A [`QuoteSide`] with `kind = Buy` and `buy_amount_after_fee` set.
    #[must_use]
    pub fn buy(amount: impl ToString) -> Self {
        Self {
            kind: OrderKind::Buy,
            sell_amount_before_fee: None,
            buy_amount_after_fee: Some(amount.to_string()),
        }
    }

    /// Returns `true` if this is a sell-side quote request.
    #[must_use]
    pub const fn is_sell(&self) -> bool {
        matches!(self.kind, OrderKind::Sell)
    }

    /// Returns `true` if this is a buy-side quote request.
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        matches!(self.kind, OrderKind::Buy)
    }
}

impl fmt::Display for QuoteSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            crate::types::OrderKind::Sell => {
                let amt = self.sell_amount_before_fee.as_deref().map_or("?", |s| s);
                write!(f, "sell {amt}")
            }
            crate::types::OrderKind::Buy => {
                let amt = self.buy_amount_after_fee.as_deref().map_or("?", |s| s);
                write!(f, "buy {amt}")
            }
        }
    }
}

// ── Quote response ────────────────────────────────────────────────────────────

/// Response from `POST /api/v1/quote`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderQuoteResponse {
    /// The computed quote data — ready to be signed and submitted.
    pub quote: QuoteData,
    /// Wallet address that requested the quote.
    pub from: alloy_primitives::Address,
    /// ISO 8601 datetime at which the quote expires.
    pub expiration: String,
    /// Numerical quote identifier for referencing when submitting the order.
    pub id: Option<i64>,
    /// Whether the solver verified this quote on-chain before returning it.
    pub verified: bool,
    /// Protocol fee in basis points (only set when a protocol fee applies).
    #[serde(default)]
    pub protocol_fee_bps: Option<String>,
}
impl OrderQuoteResponse {
    /// Returns `true` if a numerical quote ID was returned.
    #[must_use]
    pub const fn has_id(&self) -> bool {
        self.id.is_some()
    }

    /// Returns `true` if a protocol fee in basis points is available.
    #[must_use]
    pub const fn has_protocol_fee_bps(&self) -> bool {
        self.protocol_fee_bps.is_some()
    }

    /// Returns `true` if the solver verified this quote on-chain.
    #[must_use]
    pub const fn is_verified(&self) -> bool {
        self.verified
    }
}

impl fmt::Display for OrderQuoteResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "quote-resp({})", self.quote)
    }
}

/// The core quote amounts returned by the orderbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteData {
    /// Token to sell.
    pub sell_token: alloy_primitives::Address,
    /// Token to buy.
    pub buy_token: alloy_primitives::Address,
    /// Who receives the bought tokens.
    pub receiver: Option<alloy_primitives::Address>,
    /// Amount of `sell_token` to sell (after fee, in atoms).
    pub sell_amount: String,
    /// Minimum amount of `buy_token` to receive (in atoms).
    pub buy_amount: String,
    /// Order expiry as Unix timestamp.
    pub valid_to: u32,
    /// App-data hash (`bytes32` hex).
    pub app_data: String,
    /// Protocol fee included in `sell_amount` (in atoms).
    pub fee_amount: String,
    /// Sell or buy.
    pub kind: OrderKind,
    /// Whether the order may be partially filled.
    pub partially_fillable: bool,
    /// Source of sell funds.
    pub sell_token_balance: TokenBalance,
    /// Destination of buy funds.
    pub buy_token_balance: TokenBalance,
}
impl fmt::Display for QuoteData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "quote({} {:#x} sell={} → buy={})",
            self.kind, self.sell_token, self.sell_amount, self.buy_amount
        )
    }
}

impl QuoteData {
    /// Returns `true` if this is a sell-side quote.
    #[must_use]
    pub const fn is_sell(&self) -> bool {
        self.kind.is_sell()
    }

    /// Returns `true` if this is a buy-side quote.
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        self.kind.is_buy()
    }

    /// Returns `true` if the order may be partially filled.
    #[must_use]
    pub const fn is_partially_fillable(&self) -> bool {
        self.partially_fillable
    }

    /// Returns `true` if a custom receiver address has been set.
    #[must_use]
    pub const fn has_receiver(&self) -> bool {
        self.receiver.is_some()
    }
}

impl OrderQuoteRequest {
    /// Construct a minimal quote request with sensible defaults.
    ///
    /// Defaults: `app_data` = zero `bytes32` hash, token balances =
    /// [`TokenBalance::Erc20`], `price_quality` =
    /// [`PriceQuality::Optimal`], `signing_scheme` =
    /// [`EcdsaSigningScheme::Eip712`], `partially_fillable` = `false`.
    ///
    /// Use the `with_*` builder methods to override individual fields.
    ///
    /// # Parameters
    ///
    /// * `sell_token` — the ERC-20 [`Address`](alloy_primitives::Address) to sell.
    /// * `buy_token` — the ERC-20 [`Address`](alloy_primitives::Address) to buy.
    /// * `from` — the wallet [`Address`](alloy_primitives::Address) placing the order.
    /// * `side` — the [`QuoteSide`] specifying direction and amount.
    ///
    /// # Returns
    ///
    /// A new [`OrderQuoteRequest`] ready to be sent to
    /// [`OrderBookApi::get_quote`](super::api::OrderBookApi::get_quote).
    #[must_use]
    pub fn new(
        sell_token: alloy_primitives::Address,
        buy_token: alloy_primitives::Address,
        from: alloy_primitives::Address,
        side: QuoteSide,
    ) -> Self {
        Self {
            sell_token,
            buy_token,
            from,
            side,
            receiver: None,
            valid_to: None,
            app_data: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            partially_fillable: false,
            sell_token_balance: TokenBalance::Erc20,
            buy_token_balance: TokenBalance::Erc20,
            price_quality: PriceQuality::Optimal,
            signing_scheme: EcdsaSigningScheme::Eip712,
        }
    }

    /// Override the receiver address (defaults to `from`).
    #[must_use]
    pub const fn with_receiver(mut self, receiver: alloy_primitives::Address) -> Self {
        self.receiver = Some(receiver);
        self
    }

    /// Set an explicit `validTo` Unix timestamp.
    #[must_use]
    pub const fn with_valid_to(mut self, valid_to: u32) -> Self {
        self.valid_to = Some(valid_to);
        self
    }

    /// Override the `bytes32` app-data hash.
    #[must_use]
    pub fn with_app_data(mut self, app_data: impl Into<String>) -> Self {
        self.app_data = app_data.into();
        self
    }

    /// Allow partial fills.
    #[must_use]
    pub const fn with_partially_fillable(mut self) -> Self {
        self.partially_fillable = true;
        self
    }

    /// Override the price quality hint.
    #[must_use]
    pub const fn with_price_quality(mut self, quality: PriceQuality) -> Self {
        self.price_quality = quality;
        self
    }

    /// Override the source of `sellToken` funds.
    #[must_use]
    pub const fn with_sell_token_balance(mut self, balance: TokenBalance) -> Self {
        self.sell_token_balance = balance;
        self
    }

    /// Override the destination of `buyToken` funds.
    #[must_use]
    pub const fn with_buy_token_balance(mut self, balance: TokenBalance) -> Self {
        self.buy_token_balance = balance;
        self
    }

    /// Override the signing scheme.
    #[must_use]
    pub const fn with_signing_scheme(mut self, scheme: EcdsaSigningScheme) -> Self {
        self.signing_scheme = scheme;
        self
    }

    /// Returns `true` if a custom receiver address has been set.
    ///
    /// When `false`, the protocol defaults the receiver to `from`.
    #[must_use]
    pub const fn has_receiver(&self) -> bool {
        self.receiver.is_some()
    }

    /// Returns `true` if an explicit `validTo` Unix timestamp has been set.
    #[must_use]
    pub const fn has_valid_to(&self) -> bool {
        self.valid_to.is_some()
    }

    /// Returns `true` if this is a sell-side quote request.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::order_book::{OrderQuoteRequest, QuoteSide};
    ///
    /// let req = OrderQuoteRequest::new(
    ///     Address::ZERO,
    ///     Address::ZERO,
    ///     Address::ZERO,
    ///     QuoteSide::sell("1000"),
    /// );
    /// assert!(req.is_sell());
    /// assert!(!req.is_buy());
    /// ```
    #[must_use]
    pub const fn is_sell(&self) -> bool {
        self.side.is_sell()
    }

    /// Returns `true` if this is a buy-side quote request.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::order_book::{OrderQuoteRequest, QuoteSide};
    ///
    /// let req =
    ///     OrderQuoteRequest::new(Address::ZERO, Address::ZERO, Address::ZERO, QuoteSide::buy("500"));
    /// assert!(req.is_buy());
    /// assert!(!req.is_sell());
    /// ```
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        self.side.is_buy()
    }

    /// Returns `true` if the order may be partially filled.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::order_book::{OrderQuoteRequest, QuoteSide};
    ///
    /// let req = OrderQuoteRequest::new(
    ///     Address::ZERO,
    ///     Address::ZERO,
    ///     Address::ZERO,
    ///     QuoteSide::sell("1000"),
    /// );
    /// assert!(!req.is_partially_fillable());
    /// let req = req.with_partially_fillable();
    /// assert!(req.is_partially_fillable());
    /// ```
    #[must_use]
    pub const fn is_partially_fillable(&self) -> bool {
        self.partially_fillable
    }
}
impl fmt::Display for OrderQuoteRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "quote-req({:#x} → {:#x}, {})", self.sell_token, self.buy_token, self.side)
    }
}

// ── EthFlow order data ────────────────────────────────────────────────────────

/// Additional data present on `EthFlow` (native sell) orders.
///
/// Returned in the `ethflowData` field of orders submitted through the
/// `EthFlow` contract, where the user sells native ETH rather than an `ERC-20`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthflowData {
    /// The `validTo` the user actually requested (may differ from the order
    /// `validTo` set by the `EthFlow` contract).
    pub user_valid_to: u32,
    /// Whether the `EthFlow` refund has already been claimed.
    pub is_refund_claimed: bool,
}
impl EthflowData {
    /// Construct an [`EthflowData`] record.
    #[must_use]
    pub const fn new(user_valid_to: u32, is_refund_claimed: bool) -> Self {
        Self { user_valid_to, is_refund_claimed }
    }
}

impl fmt::Display for EthflowData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ethflow(valid_to={}, refunded={})", self.user_valid_to, self.is_refund_claimed)
    }
}

/// On-chain placement metadata for orders submitted directly on-chain
/// (as opposed to the off-chain API).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnchainOrderData {
    /// The address that created the on-chain order (may differ from `owner` for
    /// `EthFlow` orders where the contract is the technical owner).
    pub sender: alloy_primitives::Address,
    /// Non-`None` when the orderbook rejected the order due to a placement error.
    pub placement_error: Option<String>,
}
impl OnchainOrderData {
    /// Construct an [`OnchainOrderData`] record.
    #[must_use]
    pub const fn new(sender: alloy_primitives::Address) -> Self {
        Self { sender, placement_error: None }
    }

    /// Returns `true` if a placement error was reported for this on-chain order.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::order_book::OnchainOrderData;
    ///
    /// let data = OnchainOrderData::new(Address::ZERO);
    /// assert!(!data.has_placement_error());
    /// ```
    #[must_use]
    pub const fn has_placement_error(&self) -> bool {
        self.placement_error.is_some()
    }
}

impl fmt::Display for OnchainOrderData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "onchain(sender={:#x})", self.sender)
    }
}

// ── Order creation ────────────────────────────────────────────────────────────

/// Request body for `POST /api/v1/orders` — a signed order ready to submit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderCreation {
    /// Token to sell.
    pub sell_token: alloy_primitives::Address,
    /// Token to buy.
    pub buy_token: alloy_primitives::Address,
    /// Who receives the bought tokens.
    pub receiver: alloy_primitives::Address,
    /// Amount of `sell_token` to sell (after fee, in atoms).
    pub sell_amount: String,
    /// Minimum amount of `buy_token` to receive (in atoms).
    pub buy_amount: String,
    /// Order expiry as Unix timestamp.
    pub valid_to: u32,
    /// App-data hash (`bytes32` hex).
    pub app_data: String,
    /// Protocol fee included in `sell_amount` (in atoms).
    pub fee_amount: String,
    /// Sell or buy.
    pub kind: OrderKind,
    /// Whether the order may be partially filled.
    pub partially_fillable: bool,
    /// Source of sell funds.
    pub sell_token_balance: TokenBalance,
    /// Destination of buy funds.
    pub buy_token_balance: TokenBalance,
    /// How the order was signed.
    pub signing_scheme: SigningScheme,
    /// Hex-encoded signature bytes (format depends on `signing_scheme`).
    pub signature: String,
    /// The signer / wallet address.
    pub from: alloy_primitives::Address,
    /// Quote ID returned by `/quote` (for analytics).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_id: Option<i64>,
}

impl OrderCreation {
    /// Build an [`OrderCreation`] directly from a [`QuoteData`] response.
    ///
    /// `receiver` defaults to `from` when [`alloy_primitives::Address::ZERO`] is
    /// passed. The `quote_id` field is left as `None`; set it with [`Self::with_quote_id`].
    #[must_use]
    pub fn from_quote(
        quote: &QuoteData,
        from: alloy_primitives::Address,
        receiver: alloy_primitives::Address,
        signing_scheme: SigningScheme,
        signature: impl Into<String>,
    ) -> Self {
        let effective_receiver = if receiver.is_zero() { from } else { receiver };
        Self {
            sell_token: quote.sell_token,
            buy_token: quote.buy_token,
            receiver: effective_receiver,
            sell_amount: quote.sell_amount.clone(),
            buy_amount: quote.buy_amount.clone(),
            valid_to: quote.valid_to,
            app_data: quote.app_data.clone(),
            fee_amount: quote.fee_amount.clone(),
            kind: quote.kind,
            partially_fillable: quote.partially_fillable,
            sell_token_balance: quote.sell_token_balance,
            buy_token_balance: quote.buy_token_balance,
            signing_scheme,
            signature: signature.into(),
            from,
            quote_id: None,
        }
    }

    /// Build an [`OrderCreation`] from a signed `UnsignedOrder` and a `SigningResult`.
    ///
    /// This is the counterpart to [`Self::from_quote`] for workflows that sign
    /// an order independently (e.g. via a hardware wallet or `EIP-1271` contract)
    /// before constructing the API submission payload.
    ///
    /// `from` must be the signer's wallet address. `receiver` defaults to `from`
    /// when [`alloy_primitives::Address::ZERO`] is passed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use alloy_primitives::{Address, U256, address};
    /// use alloy_signer_local::PrivateKeySigner;
    /// use cow_rs::{EcdsaSigningScheme, OrderCreation, UnsignedOrder, order_signing::sign_order};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let signer: PrivateKeySigner =
    ///     "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".parse()?;
    /// let order = UnsignedOrder::sell(
    ///     address!("fFf9976782d46CC05630D1f6eBAb18b2324d6B14"),
    ///     address!("1c7D4B196Cb0C7B01d743Fbc6116a902379C7238"),
    ///     U256::from(1_000_000u64),
    ///     U256::from(990_000u64),
    /// );
    /// let chain_id = 11_155_111u64;
    /// let signing = sign_order(&order, chain_id, &signer, EcdsaSigningScheme::Eip712).await?;
    /// let creation =
    ///     OrderCreation::from_unsigned_order(&order, signer.address(), Address::ZERO, signing);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn from_unsigned_order(
        order: &crate::order_signing::types::UnsignedOrder,
        from: alloy_primitives::Address,
        receiver: alloy_primitives::Address,
        signing: crate::order_signing::types::SigningResult,
    ) -> Self {
        let effective_receiver = if receiver.is_zero() { from } else { receiver };
        Self {
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            receiver: effective_receiver,
            sell_amount: order.sell_amount.to_string(),
            buy_amount: order.buy_amount.to_string(),
            valid_to: order.valid_to,
            app_data: format!("{:#x}", order.app_data),
            fee_amount: order.fee_amount.to_string(),
            kind: order.kind,
            partially_fillable: order.partially_fillable,
            sell_token_balance: order.sell_token_balance,
            buy_token_balance: order.buy_token_balance,
            signing_scheme: signing.signing_scheme,
            signature: signing.signature,
            from,
            quote_id: None,
        }
    }

    /// Override the source of `sellToken` funds.
    #[must_use]
    pub const fn with_sell_token_balance(mut self, balance: TokenBalance) -> Self {
        self.sell_token_balance = balance;
        self
    }

    /// Override the destination of `buyToken` funds.
    #[must_use]
    pub const fn with_buy_token_balance(mut self, balance: TokenBalance) -> Self {
        self.buy_token_balance = balance;
        self
    }

    /// Attach the quote ID for analytics.
    #[must_use]
    pub const fn with_quote_id(mut self, quote_id: i64) -> Self {
        self.quote_id = Some(quote_id);
        self
    }

    /// Returns `true` if a quote ID has been attached to this order.
    #[must_use]
    pub const fn has_quote_id(&self) -> bool {
        self.quote_id.is_some()
    }

    /// Returns `true` if this is a sell order.
    #[must_use]
    pub const fn is_sell(&self) -> bool {
        self.kind.is_sell()
    }

    /// Returns `true` if this is a buy order.
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        self.kind.is_buy()
    }

    /// Returns `true` if this order may be partially filled.
    #[must_use]
    pub const fn is_partially_fillable(&self) -> bool {
        self.partially_fillable
    }
}

impl fmt::Display for OrderCreation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "order-creation({} {:#x} \u{2192} {:#x})",
            self.kind, self.sell_token, self.buy_token
        )
    }
}

// ── Order status ──────────────────────────────────────────────────────────────

/// Lifecycle state of an order on the orderbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderStatus {
    /// Awaiting an on-chain pre-signature.
    PresignaturePending,
    /// Awaiting a solver fill.
    Open,
    /// Fully matched and settled.
    Fulfilled,
    /// Cancelled by the owner.
    Cancelled,
    /// Past `validTo` without being filled.
    Expired,
}

impl OrderStatus {
    /// Returns the camelCase string used by the `CoW` Protocol API.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PresignaturePending => "presignaturePending",
            Self::Open => "open",
            Self::Fulfilled => "fulfilled",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
        }
    }

    /// Returns `true` if the order is pending or actively seeking a fill.
    ///
    /// Both [`Self::PresignaturePending`] and [`Self::Open`] indicate the order
    /// has not yet been settled, cancelled, or expired.
    #[must_use]
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::PresignaturePending | Self::Open)
    }

    /// Returns `true` if the order was fully matched and settled on-chain.
    #[must_use]
    pub const fn is_fulfilled(self) -> bool {
        matches!(self, Self::Fulfilled)
    }

    /// Returns `true` if the order was cancelled by the owner.
    #[must_use]
    pub const fn is_cancelled(self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Returns `true` if the order passed its `validTo` without being filled.
    #[must_use]
    pub const fn is_expired(self) -> bool {
        matches!(self, Self::Expired)
    }

    /// Returns `true` if the order is in a terminal state (no longer tradeable).
    ///
    /// Terminal states are [`Self::Fulfilled`], [`Self::Cancelled`], and
    /// [`Self::Expired`].
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Fulfilled | Self::Cancelled | Self::Expired)
    }
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for OrderStatus {
    type Error = CowError;

    /// Parse an [`OrderStatus`] from the `CoW` Protocol API string.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "presignaturePending" => Ok(Self::PresignaturePending),
            "open" => Ok(Self::Open),
            "fulfilled" => Ok(Self::Fulfilled),
            "cancelled" => Ok(Self::Cancelled),
            "expired" => Ok(Self::Expired),
            other => Err(CowError::Parse {
                field: "OrderStatus",
                reason: format!("unknown value: {other}"),
            }),
        }
    }
}

/// Order class assigned by the orderbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderClass {
    /// Market (swap) order.
    Market,
    /// Limit order.
    Limit,
    /// Liquidity provision order.
    Liquidity,
}

impl OrderClass {
    /// Returns the camelCase string used by the `CoW` Protocol API.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::Limit => "limit",
            Self::Liquidity => "liquidity",
        }
    }

    /// Returns `true` if this is a market (swap) order.
    #[must_use]
    pub const fn is_market(self) -> bool {
        matches!(self, Self::Market)
    }

    /// Returns `true` if this is a limit order.
    #[must_use]
    pub const fn is_limit(self) -> bool {
        matches!(self, Self::Limit)
    }

    /// Returns `true` if this is a liquidity provision order.
    #[must_use]
    pub const fn is_liquidity(self) -> bool {
        matches!(self, Self::Liquidity)
    }
}

impl fmt::Display for OrderClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for OrderClass {
    type Error = CowError;

    /// Parse an [`OrderClass`] from the `CoW` Protocol API string.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "market" => Ok(Self::Market),
            "limit" => Ok(Self::Limit),
            "liquidity" => Ok(Self::Liquidity),
            other => Err(CowError::Parse {
                field: "OrderClass",
                reason: format!("unknown value: {other}"),
            }),
        }
    }
}

/// A single on-chain interaction (pre- or post-settlement hook).
///
/// Interactions are executed atomically within the settlement transaction,
/// either before or after the actual token swaps take place.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionData {
    /// Target contract address.
    pub target: alloy_primitives::Address,
    /// Native-token value sent with the call (in atoms, as a decimal string).
    pub value: String,
    /// ABI-encoded call data (`0x`-prefixed hex).
    pub call_data: String,
}

impl InteractionData {
    /// Construct an [`InteractionData`] with `value = "0"`.
    #[must_use]
    pub fn new(target: alloy_primitives::Address, call_data: impl Into<String>) -> Self {
        Self { target, value: "0".to_owned(), call_data: call_data.into() }
    }

    /// Override the native-token value (in atoms, decimal string).
    #[must_use]
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    /// Returns `true` if this interaction sends a non-zero native-token value.
    #[must_use]
    pub fn has_value(&self) -> bool {
        self.value != "0"
    }
}

impl fmt::Display for InteractionData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "interaction(target={:#x})", self.target)
    }
}

/// Pre- and post-settlement interaction hooks attached to an order.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInteractions {
    /// Hooks executed before settlement.
    #[serde(default)]
    pub pre: Vec<InteractionData>,
    /// Hooks executed after settlement.
    #[serde(default)]
    pub post: Vec<InteractionData>,
}
impl OrderInteractions {
    /// Construct an [`OrderInteractions`] with pre- and post-hooks.
    #[must_use]
    pub const fn new(pre: Vec<InteractionData>, post: Vec<InteractionData>) -> Self {
        Self { pre, post }
    }

    /// Returns `true` if there is at least one pre-settlement interaction.
    #[must_use]
    pub const fn has_pre(&self) -> bool {
        !self.pre.is_empty()
    }

    /// Returns `true` if there is at least one post-settlement interaction.
    #[must_use]
    pub const fn has_post(&self) -> bool {
        !self.post.is_empty()
    }

    /// Returns the total number of interactions (pre + post).
    #[must_use]
    pub const fn total(&self) -> usize {
        self.pre.len() + self.post.len()
    }

    /// Replace the pre-settlement hooks.
    #[must_use]
    pub fn with_pre(mut self, pre: Vec<InteractionData>) -> Self {
        self.pre = pre;
        self
    }

    /// Replace the post-settlement hooks.
    #[must_use]
    pub fn with_post(mut self, post: Vec<InteractionData>) -> Self {
        self.post = post;
        self
    }

    /// Append a single pre-settlement hook.
    pub fn add_pre(&mut self, interaction: InteractionData) {
        self.pre.push(interaction);
    }

    /// Append a single post-settlement hook.
    pub fn add_post(&mut self, interaction: InteractionData) {
        self.post.push(interaction);
    }

    /// Returns `true` if there are no pre- or post-settlement interactions.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.pre.is_empty() && self.post.is_empty()
    }
}

impl fmt::Display for OrderInteractions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "interactions(pre={}, post={})", self.pre.len(), self.post.len())
    }
}

/// A full order record returned by `GET /api/v1/orders/{uid}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    /// Unique order identifier.
    pub uid: String,
    /// Owner / signer address.
    pub owner: alloy_primitives::Address,
    /// When the order was submitted (ISO 8601).
    pub creation_date: String,
    /// Current lifecycle status.
    pub status: OrderStatus,
    /// Market, limit, or liquidity.
    pub class: Option<OrderClass>,
    /// Token to sell.
    pub sell_token: alloy_primitives::Address,
    /// Token to buy.
    pub buy_token: alloy_primitives::Address,
    /// Receiver address.
    pub receiver: Option<alloy_primitives::Address>,
    /// Requested sell amount.
    pub sell_amount: String,
    /// Minimum buy amount.
    pub buy_amount: String,
    /// Order expiry.
    pub valid_to: u32,
    /// App-data hash (`bytes32` hex).
    pub app_data: String,
    /// Full app-data JSON, if it was previously uploaded to the orderbook.
    pub full_app_data: Option<String>,
    /// Protocol fee.
    pub fee_amount: String,
    /// Sell or buy.
    pub kind: OrderKind,
    /// Partial fill flag.
    pub partially_fillable: bool,
    /// Amount of sell token executed so far.
    pub executed_sell_amount: String,
    /// Amount of buy token executed so far.
    pub executed_buy_amount: String,
    /// Sell amount executed before fees.
    pub executed_sell_amount_before_fees: String,
    /// Fee amount executed so far.
    pub executed_fee_amount: String,
    /// Whether the order has been invalidated on-chain.
    pub invalidated: bool,
    /// Whether this is a liquidity (solver-internal) order rather than an
    /// active user order.
    pub is_liquidity_order: Option<bool>,
    /// Signing scheme.
    pub signing_scheme: SigningScheme,
    /// Hex-encoded signature.
    pub signature: String,
    /// On-chain interaction hooks attached to this order.
    pub interactions: Option<OrderInteractions>,
    /// Total fee paid by the order (network + protocol fees, in sell-token atoms).
    ///
    /// Present on enriched order responses (`EnrichedOrder` in the `TypeScript` SDK).
    #[serde(default)]
    pub total_fee: Option<String>,
    /// Unsubsidised fee amount (what the fee would be without `CoW` subsidies).
    #[serde(default)]
    pub full_fee_amount: Option<String>,
    /// Available sell-token balance of the order owner at the time of query.
    #[serde(default)]
    pub available_balance: Option<String>,
    /// Quote ID used when placing the order (for analytics / fee attribution).
    #[serde(default)]
    pub quote_id: Option<i64>,
    /// Fee actually executed by the solver (separate from `executed_fee_amount`
    /// for orders with both network and protocol fee components).
    #[serde(default)]
    pub executed_fee: Option<String>,
    /// For `EthFlow` orders: metadata set by the `EthFlow` contract.
    #[serde(default)]
    pub ethflow_data: Option<EthflowData>,
    /// For on-chain placed orders: the sender address and any placement error.
    #[serde(default)]
    pub onchain_order_data: Option<OnchainOrderData>,
    /// For `EthFlow` orders: the true user address (the `EthFlow` contract is
    /// the technical `owner`; `onchain_user` is the human behind it).
    #[serde(default)]
    pub onchain_user: Option<alloy_primitives::Address>,
}

impl Order {
    /// Returns `true` if this is a sell order.
    #[must_use]
    pub const fn is_sell(&self) -> bool {
        self.kind.is_sell()
    }

    /// Returns `true` if this is a buy order.
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        self.kind.is_buy()
    }

    /// Returns the effective receiver: `receiver` if set, otherwise `owner`.
    ///
    /// When an order omits the explicit receiver the protocol routes proceeds
    /// back to the order owner.
    #[must_use]
    pub fn effective_receiver(&self) -> alloy_primitives::Address {
        self.receiver.map_or(self.owner, |r| r)
    }

    /// Returns `true` if this order has at least one on-chain interaction hook.
    #[must_use]
    pub fn has_interactions(&self) -> bool {
        self.interactions.as_ref().is_some_and(|i| !i.pre.is_empty() || !i.post.is_empty())
    }

    /// Returns `true` if a surplus value is available (enriched order response).
    #[must_use]
    pub const fn has_surplus(&self) -> bool {
        self.total_fee.is_some()
    }

    /// Returns `true` if the executed fee is available (enriched order response).
    #[must_use]
    pub const fn has_executed_fee(&self) -> bool {
        self.executed_fee.is_some()
    }

    /// Returns `true` if the available sell-token balance of the owner is populated.
    #[must_use]
    pub const fn has_available_balance(&self) -> bool {
        self.available_balance.is_some()
    }

    /// Returns `true` if this order has been invalidated on-chain.
    #[must_use]
    pub const fn is_invalidated(&self) -> bool {
        self.invalidated
    }

    /// Returns `true` if the full app-data JSON is available.
    #[must_use]
    pub const fn has_full_app_data(&self) -> bool {
        self.full_app_data.is_some()
    }

    /// Returns `true` if this order carries `EthFlow` metadata.
    #[must_use]
    pub const fn has_ethflow_data(&self) -> bool {
        self.ethflow_data.is_some()
    }

    /// Returns `true` if this order carries on-chain placement metadata.
    #[must_use]
    pub const fn has_onchain_data(&self) -> bool {
        self.onchain_order_data.is_some()
    }

    /// Returns `true` if the `onchain_user` address (real user behind an `EthFlow` order) is set.
    #[must_use]
    pub const fn has_onchain_user(&self) -> bool {
        self.onchain_user.is_some()
    }

    /// Returns `true` if a custom receiver address is set.
    ///
    /// When `false`, proceeds are routed to `owner` (see [`Self::effective_receiver`]).
    #[must_use]
    pub const fn has_receiver(&self) -> bool {
        self.receiver.is_some()
    }

    /// Returns `true` if the order class is available.
    #[must_use]
    pub const fn has_class(&self) -> bool {
        self.class.is_some()
    }

    /// Returns `true` if a quote ID is attached to this order.
    #[must_use]
    pub const fn has_quote_id(&self) -> bool {
        self.quote_id.is_some()
    }

    /// Returns `true` if the unsubsidised fee amount is available (enriched order response).
    #[must_use]
    pub const fn has_full_fee_amount(&self) -> bool {
        self.full_fee_amount.is_some()
    }

    /// Returns `true` if this order may be partially filled.
    #[must_use]
    pub const fn is_partially_fillable(&self) -> bool {
        self.partially_fillable
    }

    /// Returns `true` if this order is explicitly marked as a liquidity (solver-internal) order.
    ///
    /// Returns `false` when the `is_liquidity_order` field is absent or `false`.
    #[must_use]
    pub fn is_liquidity_order(&self) -> bool {
        self.is_liquidity_order.is_some_and(|v| v)
    }

    /// Returns `true` if this is an `EthFlow` (native sell) order.
    ///
    /// An order is considered an `EthFlow` order when it carries on-chain
    /// placement metadata ([`Self::onchain_order_data`] is `Some`).
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::order_book::{OnchainOrderData, Order};
    ///
    /// // Minimal Order construction is not possible without a full JSON fixture,
    /// // so we just assert the predicate's behaviour here.
    /// let data = OnchainOrderData::new(Address::ZERO);
    /// assert!(!data.has_placement_error()); // just checking it's accessible
    /// ```
    #[must_use]
    pub const fn is_eth_flow(&self) -> bool {
        self.onchain_order_data.is_some()
    }

    /// Compute the total executed fee for this order, if available.
    ///
    /// Returns `Some(executed_fee_amount + executed_fee)` when both fields can
    /// be parsed as `U256`.  Returns `None` when either field is missing or
    /// unparseable.
    ///
    /// The executed fee is the sum of:
    /// - `executed_fee_amount` — the network (gas) fee taken from the sell token
    /// - `executed_fee` — the additional protocol fee (present on enriched responses)
    ///
    /// ```
    /// use alloy_primitives::U256;
    /// use cow_rs::order_book::Order;
    ///
    /// // Verify we can access total_executed_fee without panicking.
    /// // Full Order construction requires a JSON fixture; just check the return type.
    /// fn _typecheck(order: &Order) -> Option<U256> {
    ///     order.total_executed_fee()
    /// }
    /// ```
    #[must_use]
    pub fn total_executed_fee(&self) -> Option<U256> {
        let fee_amount: U256 = self.executed_fee_amount.parse().ok()?;
        let extra: U256 =
            self.executed_fee.as_deref().and_then(|s| s.parse().ok()).map_or(U256::ZERO, |v| v);
        Some(fee_amount.saturating_add(extra))
    }

    /// Normalise an `EthFlow` order so it looks like a regular order.
    ///
    /// Applies the following transformations:
    /// - `sell_token` is replaced with the native-currency sentinel address for `chain_id`
    ///   (currently always `0xEeee…EeEe`).
    /// - `owner` is replaced with `onchain_user` (the real user behind the `EthFlow` contract), if
    ///   present.
    ///
    /// This mirrors `transformEthFlowOrder` from the `TypeScript` SDK.
    /// Non-`EthFlow` orders are returned unchanged.
    ///
    /// ```
    /// use cow_rs::order_book::Order;
    ///
    /// // Verify the method is accessible; full construction requires a JSON fixture.
    /// fn _typecheck(order: Order) -> Order {
    ///     order.transform_eth_flow(1)
    /// }
    /// ```
    #[must_use]
    pub const fn transform_eth_flow(mut self, _chain_id: u64) -> Self {
        if self.onchain_order_data.is_none() {
            return self;
        }
        // Replace sell_token with the native currency sentinel.
        self.sell_token = crate::config::NATIVE_CURRENCY_ADDRESS;
        // Replace owner with the real user behind the EthFlow contract.
        if let Some(user) = self.onchain_user {
            self.owner = user;
        }
        self
    }
}

/// Returns `true` if `order` is an `EthFlow` (native sell) order.
///
/// Equivalent to [`Order::is_eth_flow`].  A free function is provided for
/// use in iterator adapters and other contexts where a method reference is
/// not convenient.
///
/// ```
/// use alloy_primitives::Address;
/// use cow_rs::order_book::{OnchainOrderData, is_eth_flow_order};
///
/// // Verify the function signature compiles.
/// let _fn: fn(&cow_rs::order_book::Order) -> bool = is_eth_flow_order;
/// ```
#[must_use]
pub const fn is_eth_flow_order(order: &Order) -> bool {
    order.onchain_order_data.is_some()
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let short_uid = if self.uid.len() > 10 { &self.uid[..10] } else { &self.uid };
        write!(f, "order({short_uid}… {} {})", self.kind, self.status)
    }
}

/// Query parameters for `GET /api/v1/account/{owner}/orders`.
#[derive(Debug, Clone)]
pub struct GetOrdersRequest {
    /// Owner address whose orders to fetch.
    pub owner: alloy_primitives::Address,
    /// Number of orders to skip (for pagination).
    pub offset: Option<u32>,
    /// Maximum number of orders to return (default: 1000).
    pub limit: Option<u32>,
}

impl GetOrdersRequest {
    /// Create a request for all orders belonging to `owner`.
    #[must_use]
    pub const fn for_owner(owner: alloy_primitives::Address) -> Self {
        Self { owner, offset: None, limit: None }
    }

    /// Skip the first `offset` orders (pagination).
    #[must_use]
    pub const fn with_offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Return at most `limit` orders.
    #[must_use]
    pub const fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Returns `true` if a pagination offset has been set.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::order_book::GetOrdersRequest;
    ///
    /// let req = GetOrdersRequest::for_owner(Address::ZERO);
    /// assert!(!req.has_offset());
    /// let req = req.with_offset(10);
    /// assert!(req.has_offset());
    /// ```
    #[must_use]
    pub const fn has_offset(&self) -> bool {
        self.offset.is_some()
    }

    /// Returns `true` if a result limit has been set.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::order_book::GetOrdersRequest;
    ///
    /// let req = GetOrdersRequest::for_owner(Address::ZERO);
    /// assert!(!req.has_limit());
    /// let req = req.with_limit(50);
    /// assert!(req.has_limit());
    /// ```
    #[must_use]
    pub const fn has_limit(&self) -> bool {
        self.limit.is_some()
    }
}

impl fmt::Display for GetOrdersRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "orders(owner={:#x})", self.owner)
    }
}

/// Query parameters for `GET /api/v2/trades`.
///
/// Either `owner` or `order_uid` must be set; both can be provided together.
/// Mirrors `GetTradesRequest` from the `TypeScript` SDK.
#[derive(Debug, Clone, Default)]
pub struct GetTradesRequest {
    /// Filter by trader / order owner address.
    pub owner: Option<alloy_primitives::Address>,
    /// Filter by order UID.
    pub order_uid: Option<String>,
    /// Number of trades to skip (for pagination).
    pub offset: Option<u32>,
    /// Maximum number of trades to return (default: 10).
    pub limit: Option<u32>,
}

impl GetTradesRequest {
    /// Create a request that filters trades by `owner` address.
    #[must_use]
    pub const fn for_owner(owner: alloy_primitives::Address) -> Self {
        Self { owner: Some(owner), order_uid: None, offset: None, limit: None }
    }

    /// Create a request that filters trades by order UID.
    #[must_use]
    pub fn for_order_uid(uid: impl Into<String>) -> Self {
        Self { owner: None, order_uid: Some(uid.into()), offset: None, limit: None }
    }

    /// Skip the first `offset` trades (pagination).
    #[must_use]
    pub const fn with_offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Return at most `limit` trades.
    #[must_use]
    pub const fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Returns `true` if the request filters by owner address.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::order_book::GetTradesRequest;
    ///
    /// let req = GetTradesRequest::for_owner(Address::ZERO);
    /// assert!(req.has_owner());
    /// assert!(!req.has_order_uid());
    /// ```
    #[must_use]
    pub const fn has_owner(&self) -> bool {
        self.owner.is_some()
    }

    /// Returns `true` if the request filters by order UID.
    ///
    /// ```
    /// use cow_rs::order_book::GetTradesRequest;
    ///
    /// let req = GetTradesRequest::for_order_uid("0xabc");
    /// assert!(req.has_order_uid());
    /// assert!(!req.has_owner());
    /// ```
    #[must_use]
    pub const fn has_order_uid(&self) -> bool {
        self.order_uid.is_some()
    }

    /// Returns `true` if a pagination offset has been set.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::order_book::GetTradesRequest;
    ///
    /// let req = GetTradesRequest::for_owner(Address::ZERO);
    /// assert!(!req.has_offset());
    /// let req = req.with_offset(5);
    /// assert!(req.has_offset());
    /// ```
    #[must_use]
    pub const fn has_offset(&self) -> bool {
        self.offset.is_some()
    }

    /// Returns `true` if a result limit has been set.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::order_book::GetTradesRequest;
    ///
    /// let req = GetTradesRequest::for_owner(Address::ZERO);
    /// assert!(!req.has_limit());
    /// let req = req.with_limit(20);
    /// assert!(req.has_limit());
    /// ```
    #[must_use]
    pub const fn has_limit(&self) -> bool {
        self.limit.is_some()
    }
}

impl fmt::Display for GetTradesRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(uid) = &self.order_uid {
            write!(f, "trades(uid={uid})")
        } else if let Some(owner) = self.owner {
            write!(f, "trades(owner={owner:#x})")
        } else {
            f.write_str("trades(all)")
        }
    }
}

/// The unique order identifier returned by `POST /api/v1/orders`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderUid(pub String);

impl OrderUid {
    /// Return the inner UID string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the length of the UID string in bytes.
    ///
    /// ```
    /// use cow_rs::order_book::OrderUid;
    ///
    /// let uid = OrderUid::from("0xabc123");
    /// assert_eq!(uid.len(), 8);
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the UID string is empty.
    ///
    /// ```
    /// use cow_rs::order_book::OrderUid;
    ///
    /// let uid = OrderUid::from("");
    /// assert!(uid.is_empty());
    /// let uid = OrderUid::from("0xabc");
    /// assert!(!uid.is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Display for OrderUid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for OrderUid {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<OrderUid> for String {
    fn from(uid: OrderUid) -> Self {
        uid.0
    }
}

impl From<&str> for OrderUid {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

// ── Order cancellations ───────────────────────────────────────────────────────

/// Request body for `DELETE /api/v1/orders` — batch cancellation of orders.
///
/// Contains an EIP-712 or EIP-191 signature over the list of order UIDs to
/// cancel. Authentication is required; the signature must be from the order
/// owner.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderCancellations {
    /// UIDs of orders to cancel (56 bytes each, hex-encoded with `0x` prefix).
    pub order_uids: Vec<String>,
    /// ECDSA signature (65 bytes, hex-encoded with `0x` prefix).
    pub signature: String,
    /// Whether the signature uses EIP-712 or EIP-191 (`ethsign`).
    pub signing_scheme: EcdsaSigningScheme,
}

impl OrderCancellations {
    /// Construct a cancellation request.
    #[must_use]
    pub fn new(
        order_uids: Vec<String>,
        signature: impl Into<String>,
        signing_scheme: EcdsaSigningScheme,
    ) -> Self {
        Self { order_uids, signature: signature.into(), signing_scheme }
    }

    /// Returns the number of orders to be cancelled.
    ///
    /// ```
    /// use cow_rs::{EcdsaSigningScheme, order_book::OrderCancellations};
    ///
    /// let cancel = OrderCancellations::new(
    ///     vec!["0xabc".to_owned(), "0xdef".to_owned()],
    ///     "0xsig",
    ///     EcdsaSigningScheme::Eip712,
    /// );
    /// assert_eq!(cancel.order_count(), 2);
    /// ```
    #[must_use]
    pub const fn order_count(&self) -> usize {
        self.order_uids.len()
    }
}

impl fmt::Display for OrderCancellations {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cancel({} orders)", self.order_uids.len())
    }
}

// ── Trades ────────────────────────────────────────────────────────────────────

/// A single trade event returned by `GET /api/v1/trades` or `GET /api/v2/trades`.
///
/// A partially-fillable order may produce multiple trades as it is progressively
/// matched by solvers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    /// Block in which the trade occurred.
    pub block_number: u64,
    /// Log index of the trade event within the block.
    pub log_index: u64,
    /// UID of the order matched by this trade.
    pub order_uid: String,
    /// Trader address (order owner).
    pub owner: String,
    /// Address of the token sold.
    pub sell_token: String,
    /// Address of the token bought.
    pub buy_token: String,
    /// Total sell amount executed (including fees), in atoms.
    pub sell_amount: String,
    /// Sell amount executed before fees, in atoms.
    pub sell_amount_before_fees: String,
    /// Total buy amount received, in atoms.
    pub buy_amount: String,
    /// Transaction hash of the settlement that included this trade, if available.
    pub tx_hash: Option<String>,
}

impl Trade {
    /// Returns `true` if a settlement transaction hash is available for this trade.
    #[must_use]
    pub const fn has_tx_hash(&self) -> bool {
        self.tx_hash.is_some()
    }
}

impl fmt::Display for Trade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let short_uid =
            if self.order_uid.len() > 10 { &self.order_uid[..10] } else { &self.order_uid };
        write!(f, "trade({short_uid}… block={})", self.block_number)
    }
}

// ── Auction ───────────────────────────────────────────────────────────────────

/// A batch auction returned by `GET /api/v1/auction`.
///
/// Represents the current solvable set of orders and the reference token prices
/// that solvers use to compute surplus and fees.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    /// Monotonically-increasing auction identifier.
    pub id: Option<i64>,
    /// Block number at which the auction was created.
    pub block: u64,
    /// The solvable orders included in this auction.
    pub orders: Vec<Order>,
    /// Reference prices keyed by token address (hex string) → decimal `uint256` string.
    ///
    /// Each price is denominated in the native token (1e18 = 1:1 with native).
    pub prices: HashMap<String, String>,
}

impl Auction {
    /// Returns the number of solvable orders in this auction.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.orders.len()
    }

    /// Returns `true` if the auction contains no orders.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    /// Returns `true` if the auction has at least one reference price.
    #[must_use]
    pub fn has_prices(&self) -> bool {
        !self.prices.is_empty()
    }

    /// Look up the reference price for a token address string.
    ///
    /// `token` should be a lowercase `0x`-prefixed hex address.
    /// Returns `None` if no price is available for that token.
    #[must_use]
    pub fn get_price(&self, token: &str) -> Option<&str> {
        self.prices.get(token).map(String::as_str)
    }

    /// Returns the order at `index`, or `None` if out of bounds.
    #[must_use]
    pub fn order_at(&self, index: usize) -> Option<&Order> {
        self.orders.get(index)
    }

    /// Find the first order with the given UID, or `None` if not present.
    #[must_use]
    pub fn find_order_by_uid(&self, uid: &str) -> Option<&Order> {
        self.orders.iter().find(|o| o.uid == uid)
    }
}

impl fmt::Display for Auction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = self.id.map_or(-1_i64, |i| i);
        write!(f, "auction({id}, {} orders, block={})", self.orders.len(), self.block)
    }
}

// ── Solver competition ────────────────────────────────────────────────────────

/// The set of order UIDs and reference prices that make up a competition auction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionAuction {
    /// UIDs of orders included in the auction.
    pub orders: Vec<String>,
    /// Reference prices keyed by token address → decimal `uint256` string.
    pub prices: HashMap<String, String>,
}
impl CompetitionAuction {
    /// Returns `true` if the competition auction contains at least one order UID.
    #[must_use]
    pub const fn has_orders(&self) -> bool {
        !self.orders.is_empty()
    }

    /// Look up the reference price for a token address string.
    ///
    /// `token` should be a lowercase `0x`-prefixed hex address.
    /// Returns `None` if no price is available for that token.
    #[must_use]
    pub fn get_price(&self, token: &str) -> Option<&str> {
        self.prices.get(token).map(String::as_str)
    }

    /// Returns the number of order UIDs in this competition auction.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.orders.len()
    }

    /// Returns `true` if the competition auction contains no order UIDs.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    /// Returns `true` if at least one reference price is available.
    #[must_use]
    pub fn has_prices(&self) -> bool {
        !self.prices.is_empty()
    }
}

impl fmt::Display for CompetitionAuction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "comp-auction({} orders)", self.orders.len())
    }
}

/// A single solver's proposed settlement within a competition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolverSettlement {
    /// Rank achieved in the competition (lower is better).
    pub ranking: Option<f64>,
    /// On-chain address the solver used to submit the settlement.
    pub solver_address: Option<String>,
    /// Objective score as a decimal `uint256` string.
    pub score: Option<String>,
    /// Reference score as defined in CIP-67, if available.
    pub reference_score: Option<String>,
    /// Transaction hash if the solution was executed on-chain.
    pub tx_hash: Option<String>,
    /// Clearing prices used in the settlement, keyed by token address.
    pub clearing_prices: Option<HashMap<String, String>>,
    /// Whether this solution won the competition.
    pub is_winner: Option<bool>,
    /// Whether this solution was filtered out under CIP-67 rules.
    pub filtered_out: Option<bool>,
}
impl SolverSettlement {
    /// Returns `true` if this solution won the competition.
    ///
    /// Returns `false` when the `is_winner` field is absent or `false`.
    #[must_use]
    pub fn is_winner(&self) -> bool {
        self.is_winner.is_some_and(|w| w)
    }

    /// Returns `true` if a competition rank is available for this solution.
    #[must_use]
    pub const fn has_ranking(&self) -> bool {
        self.ranking.is_some()
    }

    /// Returns `true` if the solver address is available.
    #[must_use]
    pub const fn has_solver_address(&self) -> bool {
        self.solver_address.is_some()
    }

    /// Returns `true` if an objective score is available.
    #[must_use]
    pub const fn has_score(&self) -> bool {
        self.score.is_some()
    }

    /// Returns `true` if a reference score (CIP-67) is available.
    #[must_use]
    pub const fn has_reference_score(&self) -> bool {
        self.reference_score.is_some()
    }

    /// Returns `true` if a transaction hash is available (solution was executed on-chain).
    #[must_use]
    pub const fn has_tx_hash(&self) -> bool {
        self.tx_hash.is_some()
    }

    /// Returns `true` if clearing prices are available.
    #[must_use]
    pub const fn has_clearing_prices(&self) -> bool {
        self.clearing_prices.is_some()
    }

    /// Returns `true` if this solution was filtered out under CIP-67 rules.
    ///
    /// Returns `false` when the `filtered_out` field is absent or `false`.
    #[must_use]
    pub fn is_filtered_out(&self) -> bool {
        self.filtered_out.is_some_and(|f| f)
    }

    /// Look up a clearing price for the given token address string.
    ///
    /// `token` should be a lowercase `0x`-prefixed hex address.
    /// Returns `None` when clearing prices are unavailable or the token is not present.
    #[must_use]
    pub fn get_clearing_price(&self, token: &str) -> Option<&str> {
        self.clearing_prices.as_ref()?.get(token).map(String::as_str)
    }
}

impl fmt::Display for SolverSettlement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rank = self.ranking.map_or_else(|| "?".to_owned(), |r| r.to_string());
        write!(f, "settlement(rank={rank})")
    }
}

/// Solver competition response from `GET /api/v1/solver_competition/{auctionId}`.
///
/// Contains the full details of which solvers participated, their proposed
/// solutions, and which solution won the auction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolverCompetition {
    /// Auction ID this competition corresponds to.
    pub auction_id: Option<i64>,
    /// Block at which the auction started.
    pub auction_start_block: Option<u64>,
    /// Deadline block by which the auction must be settled.
    pub auction_deadline_block: Option<u64>,
    /// Transaction hashes for the winning solutions.
    pub transaction_hashes: Option<Vec<String>>,
    /// The orders and prices that made up the competition auction.
    pub auction: Option<CompetitionAuction>,
    /// All solutions submitted by solvers, in ascending score order.
    pub solutions: Option<Vec<SolverSettlement>>,
}

impl SolverCompetition {
    /// Returns `true` if an auction ID is available.
    #[must_use]
    pub const fn has_auction_id(&self) -> bool {
        self.auction_id.is_some()
    }

    /// Returns `true` if the auction start block is available.
    #[must_use]
    pub const fn has_start_block(&self) -> bool {
        self.auction_start_block.is_some()
    }

    /// Returns `true` if the auction deadline block is available.
    #[must_use]
    pub const fn has_deadline_block(&self) -> bool {
        self.auction_deadline_block.is_some()
    }

    /// Returns `true` if there are on-chain settlement transaction hashes
    /// (indicating the competition has been settled on-chain).
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.transaction_hashes.as_ref().is_some_and(|v| !v.is_empty())
    }

    /// Returns `true` if the competition auction data is available.
    #[must_use]
    pub const fn has_auction(&self) -> bool {
        self.auction.is_some()
    }

    /// Returns `true` if solver solutions are available.
    #[must_use]
    pub const fn has_solutions(&self) -> bool {
        self.solutions.is_some()
    }

    /// Returns the number of solver solutions, or `0` if unavailable.
    #[must_use]
    pub fn num_solutions(&self) -> usize {
        self.solutions.as_ref().map_or(0, Vec::len)
    }

    /// Returns a reference to the winning solution, if one exists.
    ///
    /// Searches through all solutions and returns the first one where
    /// [`SolverSettlement::is_winner`] is `true`.
    #[must_use]
    pub fn winning_solution(&self) -> Option<&SolverSettlement> {
        self.solutions.as_ref()?.iter().find(|s| s.is_winner())
    }

    /// Returns `true` if on-chain settlement transaction hashes are available.
    ///
    /// This indicates the competition has been settled on-chain.
    #[must_use]
    pub const fn has_transaction_hashes(&self) -> bool {
        self.transaction_hashes.is_some()
    }
}

impl fmt::Display for SolverCompetition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = self.auction_id.map_or(-1_i64, |i| i);
        write!(f, "competition(auction={id})")
    }
}

// ── Order status (competition) ────────────────────────────────────────────────

/// Fine-grained lifecycle state of an order within the current batch auction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompetitionOrderStatusKind {
    /// Awaiting inclusion in an auction.
    Open,
    /// Scheduled to be executed.
    Scheduled,
    /// Currently being solved.
    Active,
    /// A solution including the order has been found.
    Solved,
    /// The winning solution is being submitted on-chain.
    Executing,
    /// The order has been traded on-chain.
    Traded,
    /// The order has been cancelled.
    Cancelled,
}

impl CompetitionOrderStatusKind {
    /// Returns the camelCase string used by the `CoW` Protocol API.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Scheduled => "scheduled",
            Self::Active => "active",
            Self::Solved => "solved",
            Self::Executing => "executing",
            Self::Traded => "traded",
            Self::Cancelled => "cancelled",
        }
    }

    /// Returns `true` if the order is awaiting inclusion in an auction.
    #[must_use]
    pub const fn is_open(self) -> bool {
        matches!(self, Self::Open)
    }

    /// Returns `true` if the order is scheduled to be executed.
    #[must_use]
    pub const fn is_scheduled(self) -> bool {
        matches!(self, Self::Scheduled)
    }

    /// Returns `true` if the order is currently being solved.
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    /// Returns `true` if a solution including the order has been found.
    #[must_use]
    pub const fn is_solved(self) -> bool {
        matches!(self, Self::Solved)
    }

    /// Returns `true` if the winning solution is being submitted on-chain.
    #[must_use]
    pub const fn is_executing(self) -> bool {
        matches!(self, Self::Executing)
    }

    /// Returns `true` if the order has been traded on-chain.
    #[must_use]
    pub const fn is_traded(self) -> bool {
        matches!(self, Self::Traded)
    }

    /// Returns `true` if the order has been cancelled.
    #[must_use]
    pub const fn is_cancelled(self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Returns `true` if the order is in a terminal state (no further progression).
    ///
    /// Terminal states are [`Self::Traded`] and [`Self::Cancelled`].
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Traded | Self::Cancelled)
    }

    /// Returns `true` if the order is still in a non-terminal state.
    ///
    /// Pending states are all variants except [`Self::Traded`] and [`Self::Cancelled`].
    #[must_use]
    pub const fn is_pending(self) -> bool {
        !self.is_terminal()
    }
}

impl fmt::Display for CompetitionOrderStatusKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for CompetitionOrderStatusKind {
    type Error = CowError;

    /// Parse a [`CompetitionOrderStatusKind`] from the `CoW` Protocol API string.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "open" => Ok(Self::Open),
            "scheduled" => Ok(Self::Scheduled),
            "active" => Ok(Self::Active),
            "solved" => Ok(Self::Solved),
            "executing" => Ok(Self::Executing),
            "traded" => Ok(Self::Traded),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(CowError::Parse {
                field: "CompetitionOrderStatusKind",
                reason: format!("unknown value: {other}"),
            }),
        }
    }
}

/// Per-solver execution amounts within the competition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolverExecution {
    /// Solver name.
    pub solver: String,
    /// Sell amount executed by this solver (if any).
    pub executed_sell_amount: Option<String>,
    /// Buy amount executed by this solver (if any).
    pub executed_buy_amount: Option<String>,
}

impl SolverExecution {
    /// Returns `true` if a sell execution amount is available.
    #[must_use]
    pub const fn has_executed_sell_amount(&self) -> bool {
        self.executed_sell_amount.is_some()
    }

    /// Returns `true` if a buy execution amount is available.
    #[must_use]
    pub const fn has_executed_buy_amount(&self) -> bool {
        self.executed_buy_amount.is_some()
    }

    /// Returns `true` if both executed sell and buy amounts are available.
    #[must_use]
    pub const fn both_amounts_available(&self) -> bool {
        self.executed_sell_amount.is_some() && self.executed_buy_amount.is_some()
    }
}

impl fmt::Display for SolverExecution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exec({})", self.solver)
    }
}

/// Response from `GET /api/v1/orders/{UID}/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionOrderStatus {
    /// Lifecycle status of the order in the current auction.
    #[serde(rename = "type")]
    pub kind: CompetitionOrderStatusKind,
    /// Solver execution details (present when `kind` is `Solved`, `Executing`, or `Traded`).
    pub value: Option<Vec<SolverExecution>>,
}

impl CompetitionOrderStatus {
    /// Returns `true` if solver execution details are attached.
    #[must_use]
    pub const fn has_value(&self) -> bool {
        self.value.is_some()
    }

    /// Returns the number of solver executions, or `0` if unavailable.
    #[must_use]
    pub fn value_len(&self) -> usize {
        self.value.as_ref().map_or(0, Vec::len)
    }
}

impl fmt::Display for CompetitionOrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.kind, f)
    }
}

// ── Total surplus ─────────────────────────────────────────────────────────────

/// Response from `GET /api/v1/users/{address}/total_surplus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotalSurplus {
    /// Total surplus accumulated by the user (decimal `uint256` string, in native-token wei).
    pub total_surplus: String,
}

impl TotalSurplus {
    /// Construct a [`TotalSurplus`] from its string value.
    #[must_use]
    pub fn new(total_surplus: impl Into<String>) -> Self {
        Self { total_surplus: total_surplus.into() }
    }

    /// Returns the total surplus as a string slice.
    ///
    /// ```
    /// use cow_rs::order_book::TotalSurplus;
    ///
    /// let s = TotalSurplus::new("12345678");
    /// assert_eq!(s.as_str(), "12345678");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.total_surplus
    }
}

impl fmt::Display for TotalSurplus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "surplus({})", self.total_surplus)
    }
}

// ── App data ──────────────────────────────────────────────────────────────────

/// Response from `GET /api/v1/app_data/{appDataHash}` and
/// `PUT /api/v1/app_data/{appDataHash}`.
///
/// Contains the full app-data JSON string associated with a given `bytes32`
/// app-data hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDataObject {
    /// Full app-data JSON document registered against the hash.
    pub full_app_data: String,
}

impl AppDataObject {
    /// Construct an [`AppDataObject`] from the full app-data JSON string.
    #[must_use]
    pub fn new(full_app_data: impl Into<String>) -> Self {
        Self { full_app_data: full_app_data.into() }
    }

    /// Returns the full app-data JSON as a string slice.
    ///
    /// ```
    /// use cow_rs::order_book::AppDataObject;
    ///
    /// let obj = AppDataObject::new("{\"version\":\"1.0.0\"}");
    /// assert_eq!(obj.as_str(), "{\"version\":\"1.0.0\"}");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.full_app_data
    }

    /// Returns the length of the full app-data JSON string in bytes.
    ///
    /// ```
    /// use cow_rs::order_book::AppDataObject;
    ///
    /// let obj = AppDataObject::new("{}");
    /// assert_eq!(obj.len(), 2);
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.full_app_data.len()
    }

    /// Returns `true` if the full app-data JSON string is empty.
    ///
    /// ```
    /// use cow_rs::order_book::AppDataObject;
    ///
    /// let obj = AppDataObject::new("");
    /// assert!(obj.is_empty());
    /// let obj = AppDataObject::new("{}");
    /// assert!(!obj.is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.full_app_data.is_empty()
    }
}

impl From<String> for AppDataObject {
    fn from(s: String) -> Self {
        Self { full_app_data: s }
    }
}

impl From<AppDataObject> for String {
    fn from(a: AppDataObject) -> Self {
        a.full_app_data
    }
}

impl fmt::Display for AppDataObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = &self.full_app_data;
        let short = if s.len() > 20 { &s[..20] } else { s };
        write!(f, "app-data({short}\u{2026})")
    }
}
