//! Response types for the `CoW` Protocol subgraph.
//!
//! All types in this module derive `Serialize` and `Deserialize` with
//! `camelCase` field names to match the `GraphQL` response format. String
//! fields are used for large integers (volume, amounts) to avoid overflow.
//!
//! # Key types
//!
//! | Type | Represents |
//! |---|---|
//! | [`Totals`] | Protocol-wide aggregate statistics |
//! | [`DailyVolume`] / [`HourlyVolume`] | Volume snapshots |
//! | [`DailyTotal`] / [`HourlyTotal`] | Full per-period statistics |
//! | [`Token`] | An ERC-20 token indexed by the subgraph |
//! | [`Trade`] | A single trade within a settlement |
//! | [`Order`] | An order indexed by the subgraph |
//! | [`Settlement`] | An on-chain batch settlement |
//! | [`Pair`] | A trading pair (token0/token1) |
//! | [`Bundle`] | Current ETH/USD price |
//! | [`User`] | A trader address with aggregate stats |

use std::fmt;

use serde::{Deserialize, Serialize};

// ── Aggregate stats ───────────────────────────────────────────────────────────

/// Protocol-wide aggregate statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    /// Number of distinct ERC-20 tokens seen.
    pub tokens: String,
    /// Total number of orders.
    pub orders: String,
    /// Total number of unique traders.
    pub traders: String,
    /// Total number of on-chain batch settlements.
    pub settlements: String,
    /// Cumulative volume in USD.
    pub volume_usd: String,
    /// Cumulative volume in ETH.
    pub volume_eth: String,
    /// Cumulative fees collected in USD.
    pub fees_usd: String,
    /// Cumulative fees collected in ETH.
    pub fees_eth: String,
}

impl fmt::Display for Totals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "totals(orders={}, traders={})", self.orders, self.traders)
    }
}

/// Per-day volume snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyVolume {
    /// Unix timestamp (start of day, UTC).
    pub timestamp: String,
    /// USD volume for this day.
    pub volume_usd: String,
}

impl fmt::Display for DailyVolume {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "daily-vol(ts={}, ${})", self.timestamp, self.volume_usd)
    }
}

/// Per-hour volume snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HourlyVolume {
    /// Unix timestamp (start of hour, UTC).
    pub timestamp: String,
    /// USD volume for this hour.
    pub volume_usd: String,
}

impl fmt::Display for HourlyVolume {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hourly-vol(ts={}, ${})", self.timestamp, self.volume_usd)
    }
}

// ── DailyTotal / HourlyTotal (full schema entities) ───────────────────────────

/// Full per-day protocol statistics entity from the subgraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyTotal {
    /// Start-of-day Unix timestamp.
    pub timestamp: String,
    /// Total orders settled this day.
    pub orders: String,
    /// Unique traders active this day.
    pub traders: String,
    /// Unique tokens traded this day.
    pub tokens: String,
    /// Number of batch settlements this day.
    pub settlements: String,
    /// Total volume in ETH.
    pub volume_eth: String,
    /// Total volume in USD.
    pub volume_usd: String,
    /// Fees collected in ETH.
    pub fees_eth: String,
    /// Fees collected in USD.
    pub fees_usd: String,
}

impl fmt::Display for DailyTotal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "daily-total(ts={}, orders={})", self.timestamp, self.orders)
    }
}

/// Full per-hour protocol statistics entity from the subgraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HourlyTotal {
    /// Start-of-hour Unix timestamp.
    pub timestamp: String,
    /// Total orders settled this hour.
    pub orders: String,
    /// Unique traders active this hour.
    pub traders: String,
    /// Unique tokens traded this hour.
    pub tokens: String,
    /// Number of batch settlements this hour.
    pub settlements: String,
    /// Total volume in ETH.
    pub volume_eth: String,
    /// Total volume in USD.
    pub volume_usd: String,
    /// Fees collected in ETH.
    pub fees_eth: String,
    /// Fees collected in USD.
    pub fees_usd: String,
}

impl fmt::Display for HourlyTotal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hourly-total(ts={}, orders={})", self.timestamp, self.orders)
    }
}

// ── Token ─────────────────────────────────────────────────────────────────────

/// An ERC-20 token indexed by the subgraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    /// Subgraph entity ID (token address in lowercase hex).
    pub id: String,
    /// Checksummed ERC-20 contract address.
    pub address: String,
    /// Unix timestamp of the first trade involving this token.
    pub first_trade_timestamp: String,
    /// Token name from ERC-20 metadata.
    pub name: String,
    /// Token symbol from ERC-20 metadata.
    pub symbol: String,
    /// Decimal places (ERC-20 `decimals()`).
    pub decimals: String,
    /// Cumulative volume traded (in token units).
    pub total_volume: String,
    /// Current price in ETH.
    pub price_eth: String,
    /// Current price in USD.
    pub price_usd: String,
    /// Total number of trades involving this token.
    pub number_of_trades: String,
}

impl Token {
    /// Returns the token symbol as a string slice.
    ///
    /// # Returns
    ///
    /// A `&str` referencing the token's symbol field.
    #[must_use]
    pub fn symbol_ref(&self) -> &str {
        &self.symbol
    }

    /// Returns the token address as a string slice.
    ///
    /// # Returns
    ///
    /// A `&str` referencing the token's address field.
    #[must_use]
    pub fn address_ref(&self) -> &str {
        &self.address
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.symbol, self.address)
    }
}

/// Per-day volume and price statistics for a specific token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenDailyTotal {
    /// Entity ID: `{tokenAddress}-{dayTimestamp}`.
    pub id: String,
    /// Token this snapshot belongs to.
    pub token: Token,
    /// Start-of-day Unix timestamp.
    pub timestamp: String,
    /// Volume traded in token units.
    pub total_volume: String,
    /// Volume in USD.
    pub total_volume_usd: String,
    /// Total number of trades.
    pub total_trades: String,
    /// Opening price in USD.
    pub open_price: String,
    /// Closing price in USD.
    pub close_price: String,
    /// Highest price in USD.
    pub higher_price: String,
    /// Lowest price in USD.
    pub lower_price: String,
    /// Volume-weighted average price in USD.
    pub average_price: String,
}

impl fmt::Display for TokenDailyTotal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "token-daily({}, ts={})", self.token, self.timestamp)
    }
}

/// Per-hour volume and price statistics for a specific token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenHourlyTotal {
    /// Entity ID: `{tokenAddress}-{hourTimestamp}`.
    pub id: String,
    /// Token this snapshot belongs to.
    pub token: Token,
    /// Start-of-hour Unix timestamp.
    pub timestamp: String,
    /// Volume traded in token units.
    pub total_volume: String,
    /// Volume in USD.
    pub total_volume_usd: String,
    /// Total number of trades.
    pub total_trades: String,
    /// Opening price in USD.
    pub open_price: String,
    /// Closing price in USD.
    pub close_price: String,
    /// Highest price in USD.
    pub higher_price: String,
    /// Lowest price in USD.
    pub lower_price: String,
    /// Volume-weighted average price in USD.
    pub average_price: String,
}

impl fmt::Display for TokenHourlyTotal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "token-hourly({}, ts={})", self.token, self.timestamp)
    }
}

/// A price-changing event for a token (used to reconstruct OHLC data).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenTradingEvent {
    /// Entity ID.
    pub id: String,
    /// Token this event belongs to.
    pub token: Token,
    /// USD price at this event.
    pub price_usd: String,
    /// Event timestamp.
    pub timestamp: String,
}

impl fmt::Display for TokenTradingEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "trade-event({}, ts={}, price=${})", self.token, self.timestamp, self.price_usd)
    }
}

// ── User ──────────────────────────────────────────────────────────────────────

/// A trader address indexed by the subgraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    /// Subgraph entity ID (address in lowercase hex).
    pub id: String,
    /// Trader address.
    pub address: String,
    /// Unix timestamp of the first trade.
    pub first_trade_timestamp: String,
    /// Total number of trades executed.
    pub number_of_trades: String,
    /// Cumulative volume of tokens sold, measured in ETH.
    pub solved_amount_eth: String,
    /// Cumulative volume of tokens sold, measured in USD.
    pub solved_amount_usd: String,
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.address)
    }
}

// ── Settlement ────────────────────────────────────────────────────────────────

/// An on-chain batch settlement transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settlement {
    /// Entity ID (transaction hash).
    pub id: String,
    /// Transaction hash.
    pub tx_hash: String,
    /// Timestamp of the first trade in this settlement.
    pub first_trade_timestamp: String,
    /// Solver address that submitted the settlement.
    pub solver: String,
    /// Gas cost of the settlement transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_cost: Option<String>,
    /// Transaction fee paid in ETH.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_fee_in_eth: Option<String>,
}

impl Settlement {
    /// Returns `true` if a gas-cost estimate is available for this settlement.
    ///
    /// # Returns
    ///
    /// `true` if `tx_cost` is `Some`.
    #[must_use]
    pub const fn has_gas_cost(&self) -> bool {
        self.tx_cost.is_some()
    }

    /// Returns `true` if a transaction fee (in ETH) is available for this settlement.
    ///
    /// # Returns
    ///
    /// `true` if `tx_fee_in_eth` is `Some`.
    #[must_use]
    pub const fn has_tx_fee(&self) -> bool {
        self.tx_fee_in_eth.is_some()
    }
}

impl fmt::Display for Settlement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "settlement({})", self.tx_hash)
    }
}

// ── Trade ─────────────────────────────────────────────────────────────────────

/// A single trade executed within a batch settlement.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    /// Entity ID: `{txHash}-{tradeIndex}`.
    pub id: String,
    /// Trade execution timestamp.
    pub timestamp: String,
    /// Gas price of the settlement transaction.
    pub gas_price: String,
    /// Protocol fee collected for this trade (in sell token).
    pub fee_amount: String,
    /// Settlement transaction hash.
    pub tx_hash: String,
    /// ID of the [`Settlement`] containing this trade.
    pub settlement: String,
    /// Amount of buy token received.
    pub buy_amount: String,
    /// Amount of sell token used (after fee).
    pub sell_amount: String,
    /// Amount of sell token before the fee was deducted.
    pub sell_amount_before_fees: String,
    /// Buy token.
    pub buy_token: Token,
    /// Sell token.
    pub sell_token: Token,
    /// Trader that placed the order.
    pub owner: User,
    /// ID of the [`Order`] this trade fills.
    pub order: String,
}

impl Trade {
    /// Returns `true` if a settlement transaction hash is available for this trade.
    ///
    /// # Returns
    ///
    /// `true` if the `tx_hash` field is non-empty.
    #[must_use]
    pub const fn has_tx_hash(&self) -> bool {
        !self.tx_hash.is_empty()
    }
}

impl fmt::Display for Trade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "trade({} {} → {})", self.tx_hash, self.sell_token, self.buy_token)
    }
}

// ── Order ─────────────────────────────────────────────────────────────────────

/// An order indexed by the subgraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    /// Entity ID (order UID hex).
    pub id: String,
    /// Order owner / trader.
    pub owner: User,
    /// Token to sell.
    pub sell_token: Token,
    /// Token to buy.
    pub buy_token: Token,
    /// Optional receiver address (if different from owner).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>,
    /// Total sell amount.
    pub sell_amount: String,
    /// Total buy amount (minimum for sell orders).
    pub buy_amount: String,
    /// Order validity deadline (Unix timestamp).
    pub valid_to: String,
    /// App-data hash (hex).
    pub app_data: String,
    /// Protocol fee amount.
    pub fee_amount: String,
    /// Order kind: `"sell"` or `"buy"`.
    pub kind: String,
    /// Whether the order can be partially filled.
    pub partially_fillable: bool,
    /// Order status: `"open"`, `"filled"`, `"cancelled"`, or `"expired"`.
    pub status: String,
    /// Cumulative sell amount executed so far.
    pub executed_sell_amount: String,
    /// Executed sell amount before fees.
    pub executed_sell_amount_before_fees: String,
    /// Cumulative buy amount executed so far.
    pub executed_buy_amount: String,
    /// Cumulative fee amount executed so far.
    pub executed_fee_amount: String,
    /// Timestamp of order cancellation (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalidate_timestamp: Option<String>,
    /// Order creation timestamp.
    pub timestamp: String,
    /// Transaction hash of the first fill.
    pub tx_hash: String,
    /// Whether the signer is a smart contract (`EIP-1271`).
    pub is_signer_safe: bool,
    /// Signing scheme used (e.g. `"eip712"`, `"ethsign"`, `"eip1271"`).
    pub signing_scheme: String,
    /// The full order UID (bytes).
    pub uid: String,
    /// Surplus generated by this order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surplus: Option<String>,
}

impl Order {
    /// Returns `true` if this is a sell order (`kind == "sell"`).
    ///
    /// # Returns
    ///
    /// `true` if the order kind is `"sell"`.
    #[must_use]
    pub fn is_sell(&self) -> bool {
        self.kind == "sell"
    }

    /// Returns `true` if this is a buy order (`kind == "buy"`).
    ///
    /// # Returns
    ///
    /// `true` if the order kind is `"buy"`.
    #[must_use]
    pub fn is_buy(&self) -> bool {
        self.kind == "buy"
    }

    /// Returns `true` if the order status is `"open"`.
    ///
    /// # Returns
    ///
    /// `true` if the order status is `"open"`.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.status == "open"
    }

    /// Returns `true` if the order status is `"filled"`.
    ///
    /// # Returns
    ///
    /// `true` if the order status is `"filled"`.
    #[must_use]
    pub fn is_filled(&self) -> bool {
        self.status == "filled"
    }

    /// Returns `true` if the order status is `"cancelled"`.
    ///
    /// # Returns
    ///
    /// `true` if the order status is `"cancelled"`.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.status == "cancelled"
    }

    /// Returns `true` if the order status is `"expired"`.
    ///
    /// # Returns
    ///
    /// `true` if the order status is `"expired"`.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.status == "expired"
    }

    /// Returns `true` if the order is in a terminal state (filled, cancelled, or expired).
    ///
    /// # Returns
    ///
    /// `true` if the order is filled, cancelled, or expired.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.is_filled() || self.is_cancelled() || self.is_expired()
    }

    /// Returns `true` if a custom receiver address is set (differs from owner).
    ///
    /// # Returns
    ///
    /// `true` if `receiver` is `Some`.
    #[must_use]
    pub const fn has_receiver(&self) -> bool {
        self.receiver.is_some()
    }

    /// Returns `true` if a cancellation/invalidation timestamp is recorded.
    ///
    /// # Returns
    ///
    /// `true` if `invalidate_timestamp` is `Some`.
    #[must_use]
    pub const fn has_invalidate_timestamp(&self) -> bool {
        self.invalidate_timestamp.is_some()
    }

    /// Returns `true` if a surplus value is available for this order.
    ///
    /// # Returns
    ///
    /// `true` if `surplus` is `Some`.
    #[must_use]
    pub const fn has_surplus(&self) -> bool {
        self.surplus.is_some()
    }

    /// Returns `true` if the order may be partially filled.
    ///
    /// # Returns
    ///
    /// `true` if the `partially_fillable` flag is set.
    #[must_use]
    pub const fn is_partially_fillable(&self) -> bool {
        self.partially_fillable
    }

    /// Returns `true` if the order signer is a smart contract (`EIP-1271`).
    ///
    /// # Returns
    ///
    /// `true` if the `is_signer_safe` flag is set.
    #[must_use]
    pub const fn is_signer_safe(&self) -> bool {
        self.is_signer_safe
    }
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let short_uid = if self.uid.len() > 10 { &self.uid[..10] } else { &self.uid };
        write!(f, "order({short_uid}… {} {})", self.kind, self.status)
    }
}

// ── Pair ──────────────────────────────────────────────────────────────────────

/// A trading pair (two tokens) indexed by the subgraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pair {
    /// Entity ID: `{token0Address}-{token1Address}`.
    pub id: String,
    /// First token (lower address).
    pub token0: Token,
    /// Second token (higher address).
    pub token1: Token,
    /// Total volume in token0 units.
    pub volume_token0: String,
    /// Total volume in token1 units.
    pub volume_token1: String,
    /// Total number of trades for this pair.
    pub number_of_trades: String,
}

impl fmt::Display for Pair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pair({}/{})", self.token0, self.token1)
    }
}

/// Per-day statistics for a token pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairDaily {
    /// Entity ID: `{token0Address}-{token1Address}-{dayTimestamp}`.
    pub id: String,
    /// First token in the pair (lower address).
    pub token0: Token,
    /// Second token in the pair (higher address).
    pub token1: Token,
    /// Start-of-day Unix timestamp.
    pub timestamp: String,
    /// Volume in token0 units for this day.
    pub volume_token0: String,
    /// Volume in token1 units for this day.
    pub volume_token1: String,
    /// Number of trades this day.
    pub number_of_trades: String,
}

impl fmt::Display for PairDaily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pair-daily({}/{}, ts={})", self.token0, self.token1, self.timestamp)
    }
}

/// Per-hour statistics for a token pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairHourly {
    /// Entity ID: `{token0Address}-{token1Address}-{hourTimestamp}`.
    pub id: String,
    /// First token in the pair.
    pub token0: Token,
    /// Second token in the pair.
    pub token1: Token,
    /// Start-of-hour Unix timestamp.
    pub timestamp: String,
    /// Volume in token0 units.
    pub volume_token0: String,
    /// Volume in token1 units.
    pub volume_token1: String,
    /// Number of trades this hour.
    pub number_of_trades: String,
}

impl fmt::Display for PairHourly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pair-hourly({}/{}, ts={})", self.token0, self.token1, self.timestamp)
    }
}

// ── Bundle ────────────────────────────────────────────────────────────────────

/// Aggregate price bundle — contains the current ETH/USD price.
///
/// The subgraph maintains a single `Bundle` entity with `id = "1"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bundle {
    /// Always `"1"` (singleton entity).
    pub id: String,
    /// Current ETH price in USD.
    pub eth_price_usd: String,
}

impl Bundle {
    /// Returns the current ETH/USD price as a string slice.
    ///
    /// # Returns
    ///
    /// A `&str` referencing the `eth_price_usd` field.
    #[must_use]
    pub fn eth_price_usd_ref(&self) -> &str {
        &self.eth_price_usd
    }
}

impl fmt::Display for Bundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "eth-price=${}", self.eth_price_usd)
    }
}

// ── Total (singleton accumulator) ────────────────────────────────────────────

/// Protocol-wide singleton accumulator entity from the subgraph.
///
/// Mirrors the `Total` `GraphQL` type. Unlike [`Totals`] (which is the
/// flattened query-response shape), this type matches the full subgraph
/// entity including its `id` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Total {
    /// Singleton entity ID (always `"1"`).
    pub id: String,
    /// Total number of orders placed.
    pub orders: String,
    /// Total number of batch settlements.
    pub settlements: String,
    /// Total number of distinct tokens traded.
    pub tokens: String,
    /// Total number of unique traders.
    pub traders: String,
    /// Total number of trades executed.
    pub number_of_trades: String,
    /// Cumulative volume in ETH.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_eth: Option<String>,
    /// Cumulative volume in USD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_usd: Option<String>,
    /// Cumulative fees in ETH.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fees_eth: Option<String>,
    /// Cumulative fees in USD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fees_usd: Option<String>,
}

impl fmt::Display for Total {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "total(orders={}, traders={})", self.orders, self.traders)
    }
}

// ── UniswapToken ─────────────────────────────────────────────────────────────

/// A Uniswap token entity indexed by the `CoW` Protocol subgraph.
///
/// Mirrors the `UniswapToken` `GraphQL` type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniswapToken {
    /// Entity ID (token address hex).
    pub id: String,
    /// Token contract address (bytes).
    pub address: String,
    /// Token name from ERC-20 metadata.
    pub name: String,
    /// Token symbol from ERC-20 metadata.
    pub symbol: String,
    /// Decimal places (ERC-20 `decimals()`).
    pub decimals: i32,
    /// Derived price in ETH.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_eth: Option<String>,
    /// Derived price in USD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_usd: Option<String>,
}

impl fmt::Display for UniswapToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.symbol, self.address)
    }
}

// ── UniswapPool ──────────────────────────────────────────────────────────────

/// A Uniswap pool entity indexed by the `CoW` Protocol subgraph.
///
/// Mirrors the `UniswapPool` `GraphQL` type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniswapPool {
    /// Pool contract address.
    pub id: String,
    /// In-range liquidity.
    pub liquidity: String,
    /// Current tick (may be absent for inactive pools).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick: Option<String>,
    /// First token in the pool.
    pub token0: UniswapToken,
    /// Price of token0 in terms of token1.
    pub token0_price: String,
    /// Second token in the pool.
    pub token1: UniswapToken,
    /// Price of token1 in terms of token0.
    pub token1_price: String,
    /// Total token0 locked across all ticks.
    pub total_value_locked_token0: String,
    /// Total token1 locked across all ticks.
    pub total_value_locked_token1: String,
}

impl fmt::Display for UniswapPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pool({}, {}/{})", self.id, self.token0, self.token1)
    }
}

// ── Subgraph block / meta ────────────────────────────────────────────────────

/// Block information returned by the subgraph `_meta` field.
///
/// Mirrors the `_Block_` `GraphQL` type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubgraphBlock {
    /// Block hash (may be `None` if a block-number constraint was used).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Block number.
    pub number: i64,
    /// Parent block hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_hash: Option<String>,
    /// Block timestamp (integer representation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
}

impl fmt::Display for SubgraphBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "block(#{})", self.number)
    }
}

/// Top-level subgraph indexing metadata returned by the `_meta` field.
///
/// Mirrors the `_Meta_` `GraphQL` type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubgraphMeta {
    /// Block the subgraph has indexed up to.
    pub block: SubgraphBlock,
    /// Subgraph deployment ID.
    pub deployment: String,
    /// Whether the subgraph encountered indexing errors at some past block.
    pub has_indexing_errors: bool,
}

impl fmt::Display for SubgraphMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "meta(deploy={}, block={})", self.deployment, self.block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper constructors ──────────────────────────────────────────────────

    fn sample_token() -> Token {
        Token {
            id: "0xabc".into(),
            address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".into(),
            first_trade_timestamp: "1700000000".into(),
            name: "USD Coin".into(),
            symbol: "USDC".into(),
            decimals: "6".into(),
            total_volume: "1000000".into(),
            price_eth: "0.0003".into(),
            price_usd: "1.0".into(),
            number_of_trades: "42".into(),
        }
    }

    fn sample_user() -> User {
        User {
            id: "0xuser".into(),
            address: "0xUserAddress".into(),
            first_trade_timestamp: "1700000000".into(),
            number_of_trades: "10".into(),
            solved_amount_eth: "5.0".into(),
            solved_amount_usd: "10000".into(),
        }
    }

    // ── Display impls ────────────────────────────────────────────────────────

    #[test]
    fn totals_display() {
        let t = Totals {
            tokens: "100".into(),
            orders: "500".into(),
            traders: "50".into(),
            settlements: "20".into(),
            volume_usd: "1000000".into(),
            volume_eth: "500".into(),
            fees_usd: "1000".into(),
            fees_eth: "0.5".into(),
        };
        assert_eq!(t.to_string(), "totals(orders=500, traders=50)");
    }

    #[test]
    fn daily_volume_display() {
        let d = DailyVolume { timestamp: "1700000000".into(), volume_usd: "500000".into() };
        assert_eq!(d.to_string(), "daily-vol(ts=1700000000, $500000)");
    }

    #[test]
    fn hourly_volume_display() {
        let h = HourlyVolume { timestamp: "1700000000".into(), volume_usd: "10000".into() };
        assert_eq!(h.to_string(), "hourly-vol(ts=1700000000, $10000)");
    }

    #[test]
    fn daily_total_display() {
        let d = DailyTotal {
            timestamp: "1700000000".into(),
            orders: "100".into(),
            traders: "10".into(),
            tokens: "5".into(),
            settlements: "3".into(),
            volume_eth: "50".into(),
            volume_usd: "100000".into(),
            fees_eth: "0.1".into(),
            fees_usd: "200".into(),
        };
        assert_eq!(d.to_string(), "daily-total(ts=1700000000, orders=100)");
    }

    #[test]
    fn hourly_total_display() {
        let h = HourlyTotal {
            timestamp: "1700000000".into(),
            orders: "10".into(),
            traders: "5".into(),
            tokens: "3".into(),
            settlements: "1".into(),
            volume_eth: "10".into(),
            volume_usd: "20000".into(),
            fees_eth: "0.01".into(),
            fees_usd: "20".into(),
        };
        assert_eq!(h.to_string(), "hourly-total(ts=1700000000, orders=10)");
    }

    #[test]
    fn token_display() {
        let t = sample_token();
        assert_eq!(t.to_string(), "USDC (0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48)");
    }

    #[test]
    fn token_accessors() {
        let t = sample_token();
        assert_eq!(t.symbol_ref(), "USDC");
        assert_eq!(t.address_ref(), "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    }

    #[test]
    fn user_display() {
        let u = sample_user();
        assert_eq!(u.to_string(), "0xUserAddress");
    }

    #[test]
    fn settlement_display_and_methods() {
        let s = Settlement {
            id: "0xtx".into(),
            tx_hash: "0xdeadbeef".into(),
            first_trade_timestamp: "1700000000".into(),
            solver: "0xsolver".into(),
            tx_cost: Some("1000".into()),
            tx_fee_in_eth: None,
        };
        assert_eq!(s.to_string(), "settlement(0xdeadbeef)");
        assert!(s.has_gas_cost());
        assert!(!s.has_tx_fee());

        let s2 = Settlement {
            id: "0xtx".into(),
            tx_hash: "0xdeadbeef".into(),
            first_trade_timestamp: "1700000000".into(),
            solver: "0xsolver".into(),
            tx_cost: None,
            tx_fee_in_eth: Some("0.01".into()),
        };
        assert!(!s2.has_gas_cost());
        assert!(s2.has_tx_fee());
    }

    #[test]
    fn trade_display_and_has_tx_hash() {
        let t = Trade {
            id: "0x-0".into(),
            timestamp: "1700000000".into(),
            gas_price: "20".into(),
            fee_amount: "100".into(),
            tx_hash: "0xabc".into(),
            settlement: "0xsettle".into(),
            buy_amount: "500".into(),
            sell_amount: "1000".into(),
            sell_amount_before_fees: "1100".into(),
            buy_token: sample_token(),
            sell_token: sample_token(),
            owner: sample_user(),
            order: "0xorder".into(),
        };
        assert!(t.has_tx_hash());
        assert!(t.to_string().contains("0xabc"));
    }

    #[test]
    fn order_methods() {
        let o = Order {
            id: "0xorderuid1234567890".into(),
            owner: sample_user(),
            sell_token: sample_token(),
            buy_token: sample_token(),
            receiver: Some("0xreceiver".into()),
            sell_amount: "1000".into(),
            buy_amount: "500".into(),
            valid_to: "1700000000".into(),
            app_data: "0xappdata".into(),
            fee_amount: "10".into(),
            kind: "sell".into(),
            partially_fillable: true,
            status: "open".into(),
            executed_sell_amount: "0".into(),
            executed_sell_amount_before_fees: "0".into(),
            executed_buy_amount: "0".into(),
            executed_fee_amount: "0".into(),
            invalidate_timestamp: None,
            timestamp: "1700000000".into(),
            tx_hash: "0xtx".into(),
            is_signer_safe: false,
            signing_scheme: "eip712".into(),
            uid: "0xorderuid1234567890abcdef".into(),
            surplus: Some("50".into()),
        };
        assert!(o.is_sell());
        assert!(!o.is_buy());
        assert!(o.is_open());
        assert!(!o.is_filled());
        assert!(!o.is_cancelled());
        assert!(!o.is_expired());
        assert!(!o.is_terminal());
        assert!(o.has_receiver());
        assert!(!o.has_invalidate_timestamp());
        assert!(o.has_surplus());
        assert!(o.is_partially_fillable());
        assert!(!o.is_signer_safe());
        assert!(o.to_string().contains("sell"));
        assert!(o.to_string().contains("open"));
    }

    #[test]
    fn order_terminal_states() {
        let make = |status: &str| Order {
            id: "x".into(),
            owner: sample_user(),
            sell_token: sample_token(),
            buy_token: sample_token(),
            receiver: None,
            sell_amount: "0".into(),
            buy_amount: "0".into(),
            valid_to: "0".into(),
            app_data: "0x".into(),
            fee_amount: "0".into(),
            kind: "buy".into(),
            partially_fillable: false,
            status: status.into(),
            executed_sell_amount: "0".into(),
            executed_sell_amount_before_fees: "0".into(),
            executed_buy_amount: "0".into(),
            executed_fee_amount: "0".into(),
            invalidate_timestamp: Some("100".into()),
            timestamp: "0".into(),
            tx_hash: String::new(),
            is_signer_safe: true,
            signing_scheme: "eip1271".into(),
            uid: "short".into(),
            surplus: None,
        };
        assert!(make("filled").is_filled());
        assert!(make("filled").is_terminal());
        assert!(make("cancelled").is_cancelled());
        assert!(make("cancelled").is_terminal());
        assert!(make("expired").is_expired());
        assert!(make("expired").is_terminal());

        let buy_order = make("open");
        assert!(buy_order.is_buy());
        assert!(buy_order.has_invalidate_timestamp());
        assert!(!buy_order.has_surplus());
        assert!(!buy_order.is_partially_fillable());
        assert!(buy_order.is_signer_safe());
    }

    #[test]
    fn bundle_display_and_accessor() {
        let b = Bundle { id: "1".into(), eth_price_usd: "3500.00".into() };
        assert_eq!(b.to_string(), "eth-price=$3500.00");
        assert_eq!(b.eth_price_usd_ref(), "3500.00");
    }

    #[test]
    fn total_display() {
        let t = Total {
            id: "1".into(),
            orders: "100".into(),
            settlements: "10".into(),
            tokens: "20".into(),
            traders: "30".into(),
            number_of_trades: "200".into(),
            volume_eth: None,
            volume_usd: None,
            fees_eth: None,
            fees_usd: None,
        };
        assert_eq!(t.to_string(), "total(orders=100, traders=30)");
    }

    #[test]
    fn subgraph_block_display() {
        let b = SubgraphBlock {
            hash: Some("0xabc".into()),
            number: 12345,
            parent_hash: None,
            timestamp: Some(1700000000),
        };
        assert_eq!(b.to_string(), "block(#12345)");
    }

    #[test]
    fn subgraph_meta_display() {
        let m = SubgraphMeta {
            block: SubgraphBlock { hash: None, number: 999, parent_hash: None, timestamp: None },
            deployment: "deploy-123".into(),
            has_indexing_errors: false,
        };
        assert_eq!(m.to_string(), "meta(deploy=deploy-123, block=block(#999))");
    }

    #[test]
    fn uniswap_token_display() {
        let t = UniswapToken {
            id: "0xabc".into(),
            address: "0xABC".into(),
            name: "Token".into(),
            symbol: "TKN".into(),
            decimals: 18,
            price_eth: None,
            price_usd: None,
        };
        assert_eq!(t.to_string(), "TKN (0xABC)");
    }

    #[test]
    fn uniswap_pool_display() {
        let t0 = UniswapToken {
            id: "0xa".into(),
            address: "0xA".into(),
            name: "A".into(),
            symbol: "A".into(),
            decimals: 18,
            price_eth: None,
            price_usd: None,
        };
        let t1 = UniswapToken {
            id: "0xb".into(),
            address: "0xB".into(),
            name: "B".into(),
            symbol: "B".into(),
            decimals: 18,
            price_eth: None,
            price_usd: None,
        };
        let p = UniswapPool {
            id: "0xpool".into(),
            liquidity: "1000".into(),
            tick: Some("100".into()),
            token0: t0,
            token0_price: "1.0".into(),
            token1: t1,
            token1_price: "1.0".into(),
            total_value_locked_token0: "500".into(),
            total_value_locked_token1: "500".into(),
        };
        assert_eq!(p.to_string(), "pool(0xpool, A (0xA)/B (0xB))");
    }

    #[test]
    fn pair_display() {
        let p = Pair {
            id: "0xa-0xb".into(),
            token0: sample_token(),
            token1: sample_token(),
            volume_token0: "100".into(),
            volume_token1: "200".into(),
            number_of_trades: "5".into(),
        };
        assert!(p.to_string().starts_with("pair("));
    }

    #[test]
    fn pair_daily_display() {
        let pd = PairDaily {
            id: "0xa-0xb-123".into(),
            token0: sample_token(),
            token1: sample_token(),
            timestamp: "1700000000".into(),
            volume_token0: "100".into(),
            volume_token1: "200".into(),
            number_of_trades: "5".into(),
        };
        assert!(pd.to_string().contains("pair-daily("));
    }

    #[test]
    fn pair_hourly_display() {
        let ph = PairHourly {
            id: "0xa-0xb-123".into(),
            token0: sample_token(),
            token1: sample_token(),
            timestamp: "1700000000".into(),
            volume_token0: "100".into(),
            volume_token1: "200".into(),
            number_of_trades: "5".into(),
        };
        assert!(ph.to_string().contains("pair-hourly("));
    }

    #[test]
    fn token_daily_total_display() {
        let tdt = TokenDailyTotal {
            id: "0x-123".into(),
            token: sample_token(),
            timestamp: "1700000000".into(),
            total_volume: "1000".into(),
            total_volume_usd: "1000".into(),
            total_trades: "10".into(),
            open_price: "1.0".into(),
            close_price: "1.01".into(),
            higher_price: "1.02".into(),
            lower_price: "0.99".into(),
            average_price: "1.005".into(),
        };
        assert!(tdt.to_string().contains("token-daily("));
    }

    #[test]
    fn token_hourly_total_display() {
        let tht = TokenHourlyTotal {
            id: "0x-123".into(),
            token: sample_token(),
            timestamp: "1700000000".into(),
            total_volume: "500".into(),
            total_volume_usd: "500".into(),
            total_trades: "5".into(),
            open_price: "1.0".into(),
            close_price: "1.01".into(),
            higher_price: "1.02".into(),
            lower_price: "0.99".into(),
            average_price: "1.005".into(),
        };
        assert!(tht.to_string().contains("token-hourly("));
    }

    #[test]
    fn token_trading_event_display() {
        let e = TokenTradingEvent {
            id: "evt1".into(),
            token: sample_token(),
            price_usd: "1.01".into(),
            timestamp: "1700000000".into(),
        };
        assert!(e.to_string().contains("trade-event("));
        assert!(e.to_string().contains("price=$1.01"));
    }

    // ── Serde roundtrips ─────────────────────────────────────────────────────

    #[test]
    fn totals_serde_roundtrip() {
        let t = Totals {
            tokens: "100".into(),
            orders: "500".into(),
            traders: "50".into(),
            settlements: "20".into(),
            volume_usd: "1000000".into(),
            volume_eth: "500".into(),
            fees_usd: "1000".into(),
            fees_eth: "0.5".into(),
        };
        let json = serde_json::to_string(&t).unwrap();
        let t2: Totals = serde_json::from_str(&json).unwrap();
        assert_eq!(t2.orders, "500");
    }

    #[test]
    fn settlement_serde_skips_none() {
        let s = Settlement {
            id: "x".into(),
            tx_hash: "0x".into(),
            first_trade_timestamp: "0".into(),
            solver: "0x".into(),
            tx_cost: None,
            tx_fee_in_eth: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("txCost"));
        assert!(!json.contains("txFeeInEth"));
    }

    #[test]
    fn total_serde_roundtrip_with_optional_fields() {
        let t = Total {
            id: "1".into(),
            orders: "10".into(),
            settlements: "5".into(),
            tokens: "3".into(),
            traders: "2".into(),
            number_of_trades: "20".into(),
            volume_eth: Some("100".into()),
            volume_usd: Some("200000".into()),
            fees_eth: None,
            fees_usd: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("volumeEth"));
        assert!(!json.contains("feesEth"));
        let t2: Total = serde_json::from_str(&json).unwrap();
        assert_eq!(t2.volume_eth, Some("100".into()));
        assert_eq!(t2.fees_eth, None);
    }

    #[test]
    fn bundle_serde_roundtrip() {
        let b = Bundle { id: "1".into(), eth_price_usd: "3500".into() };
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains("ethPriceUsd"));
        let b2: Bundle = serde_json::from_str(&json).unwrap();
        assert_eq!(b2.eth_price_usd, "3500");
    }
}
