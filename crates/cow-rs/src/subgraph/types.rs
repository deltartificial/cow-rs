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
/// Mirrors the `Total` GraphQL type. Unlike [`Totals`] (which is the
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
/// Mirrors the `UniswapToken` GraphQL type.
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
/// Mirrors the `UniswapPool` GraphQL type.
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
/// Mirrors the `_Block_` GraphQL type.
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
/// Mirrors the `_Meta_` GraphQL type.
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
