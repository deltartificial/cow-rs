//! Types for `CoW` Protocol composable (conditional) orders.

use std::fmt;

use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};

use crate::types::OrderKind;

// ── Handler addresses ─────────────────────────────────────────────────────────

/// `ComposableCow` factory contract — same address on all supported chains.
///
/// `0xfdaFc9d1902f4e0b84f65F49f244b32b31013b74`
pub const COMPOSABLE_COW_ADDRESS: Address = Address::new([
    0xfd, 0xaf, 0xc9, 0xd1, 0x90, 0x2f, 0x4e, 0x0b, 0x84, 0xf6, 0x5f, 0x49, 0xf2, 0x44, 0xb3, 0x2b,
    0x31, 0x01, 0x3b, 0x74,
]);

/// Default `TWAP` handler contract address.
///
/// `0x6cF1e9cA41f7611dEf408122793c358a3d11E5a5`
pub const TWAP_HANDLER_ADDRESS: Address = Address::new([
    0x6c, 0xf1, 0xe9, 0xca, 0x41, 0xf7, 0x61, 0x1d, 0xef, 0x40, 0x81, 0x22, 0x79, 0x3c, 0x35, 0x8a,
    0x3d, 0x11, 0xe5, 0xa5,
]);

/// `CurrentBlockTimestampFactory` contract address.
///
/// Used as the `ContextFactory` when a `TWAP` order has `start_time =
/// AtMiningTime` (`t0 = 0`). The factory reads `block.timestamp` at order
/// creation and writes it into the `ComposableCow` cabinet so that every part
/// is measured from the same anchor.
///
/// `0x52eD56Da04309Aca4c3FECC595298d80C2f16BAc`
pub const CURRENT_BLOCK_TIMESTAMP_FACTORY_ADDRESS: Address = Address::new([
    0x52, 0xed, 0x56, 0xda, 0x04, 0x30, 0x9a, 0xca, 0x4c, 0x3f, 0xec, 0xc5, 0x95, 0x29, 0x8d, 0x80,
    0xc2, 0xf1, 0x6b, 0xac,
]);

/// Maximum allowed `part_duration` in seconds (1 year).
///
/// Mirrors `MAX_FREQUENCY` from the `TypeScript` SDK.
pub const MAX_FREQUENCY: u32 = 365 * 24 * 60 * 60; // 31_536_000 s

// ── ConditionalOrderParams ────────────────────────────────────────────────────

/// ABI-encoded parameters identifying a conditional order on-chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionalOrderParams {
    /// Address of the handler contract that validates the order.
    pub handler: Address,
    /// 32-byte salt providing uniqueness per order.
    pub salt: B256,
    /// ABI-encoded static input consumed by the handler.
    pub static_input: Vec<u8>,
}

impl ConditionalOrderParams {
    /// Construct [`ConditionalOrderParams`] from its three constituent fields.
    ///
    /// # Arguments
    ///
    /// * `handler` - Address of the handler contract that validates the order.
    /// * `salt` - 32-byte salt providing uniqueness per order.
    /// * `static_input` - ABI-encoded static input consumed by the handler.
    ///
    /// # Returns
    ///
    /// A new [`ConditionalOrderParams`] instance.
    #[must_use]
    pub const fn new(handler: Address, salt: B256, static_input: Vec<u8>) -> Self {
        Self { handler, salt, static_input }
    }

    /// Override the handler contract address.
    ///
    /// # Arguments
    ///
    /// * `handler` - The new handler contract address.
    ///
    /// # Returns
    ///
    /// The modified [`ConditionalOrderParams`] with the updated handler (builder pattern).
    #[must_use]
    pub const fn with_handler(mut self, handler: Address) -> Self {
        self.handler = handler;
        self
    }

    /// Override the 32-byte salt.
    ///
    /// # Arguments
    ///
    /// * `salt` - The new 32-byte salt value.
    ///
    /// # Returns
    ///
    /// The modified [`ConditionalOrderParams`] with the updated salt (builder pattern).
    #[must_use]
    pub const fn with_salt(mut self, salt: B256) -> Self {
        self.salt = salt;
        self
    }

    /// Override the ABI-encoded static input.
    ///
    /// # Arguments
    ///
    /// * `static_input` - The new ABI-encoded static input bytes.
    ///
    /// # Returns
    ///
    /// The modified [`ConditionalOrderParams`] with the updated static input (builder pattern).
    #[must_use]
    pub fn with_static_input(mut self, static_input: Vec<u8>) -> Self {
        self.static_input = static_input;
        self
    }

    /// Returns `true` if the static input bytes are empty.
    ///
    /// # Returns
    ///
    /// `true` if the `static_input` field contains zero bytes, `false` otherwise.
    #[must_use]
    pub const fn is_empty_static_input(&self) -> bool {
        self.static_input.is_empty()
    }

    /// Returns the length of the static input bytes.
    ///
    /// # Returns
    ///
    /// The number of bytes in the `static_input` field.
    #[must_use]
    pub const fn static_input_len(&self) -> usize {
        self.static_input.len()
    }

    /// Returns a reference to the 32-byte salt.
    ///
    /// ```
    /// use alloy_primitives::{Address, B256};
    /// use cow_rs::composable::ConditionalOrderParams;
    ///
    /// let params = ConditionalOrderParams::new(Address::ZERO, B256::ZERO, vec![]);
    /// assert_eq!(params.salt_ref(), &B256::ZERO);
    /// ```
    #[must_use]
    pub const fn salt_ref(&self) -> &B256 {
        &self.salt
    }
}

impl fmt::Display for ConditionalOrderParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "params(handler={:#x})", self.handler)
    }
}

// ── TWAP ──────────────────────────────────────────────────────────────────────

/// Start time specification for a `TWAP` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TwapStartTime {
    /// Start immediately at the block containing the order creation tx.
    AtMiningTime,
    /// Start at a specific Unix timestamp.
    At(u32),
}

impl TwapStartTime {
    /// Returns a human-readable string label for the start time.
    ///
    /// # Returns
    ///
    /// `"at-mining-time"` for [`AtMiningTime`](Self::AtMiningTime), or
    /// `"at-unix"` for [`At`](Self::At).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AtMiningTime => "at-mining-time",
            Self::At(_) => "at-unix",
        }
    }

    /// Returns `true` if the order starts at the block it is mined in.
    ///
    /// # Returns
    ///
    /// `true` for [`AtMiningTime`](Self::AtMiningTime), `false` for [`At`](Self::At).
    #[must_use]
    pub const fn is_at_mining_time(self) -> bool {
        matches!(self, Self::AtMiningTime)
    }

    /// Returns `true` if the order starts at a fixed Unix timestamp.
    ///
    /// # Returns
    ///
    /// `true` for [`At`](Self::At), `false` for [`AtMiningTime`](Self::AtMiningTime).
    #[must_use]
    pub const fn is_fixed(self) -> bool {
        matches!(self, Self::At(_))
    }

    /// Return the fixed start timestamp, or `None` for [`AtMiningTime`](Self::AtMiningTime).
    ///
    /// # Returns
    ///
    /// `Some(ts)` containing the Unix timestamp for [`At`](Self::At),
    /// or `None` for [`AtMiningTime`](Self::AtMiningTime).
    #[must_use]
    pub const fn timestamp(self) -> Option<u32> {
        match self {
            Self::At(ts) => Some(ts),
            Self::AtMiningTime => None,
        }
    }
}

impl fmt::Display for TwapStartTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AtMiningTime => f.write_str("at-mining-time"),
            Self::At(ts) => write!(f, "at-unix-{ts}"),
        }
    }
}

impl From<u32> for TwapStartTime {
    /// Convert a Unix timestamp into a [`TwapStartTime`].
    ///
    /// `0` maps to [`TwapStartTime::AtMiningTime`]; any other value maps to
    /// [`TwapStartTime::At`].  This mirrors the on-chain `t0` field encoding.
    fn from(ts: u32) -> Self {
        if ts == 0 { Self::AtMiningTime } else { Self::At(ts) }
    }
}

impl From<TwapStartTime> for u32 {
    /// Encode a [`TwapStartTime`] as the on-chain `t0` field.
    ///
    /// [`TwapStartTime::AtMiningTime`] encodes as `0`; [`TwapStartTime::At`]
    /// encodes as the contained Unix timestamp.
    fn from(t: TwapStartTime) -> Self {
        match t {
            TwapStartTime::AtMiningTime => 0,
            TwapStartTime::At(ts) => ts,
        }
    }
}

impl From<Option<u32>> for TwapStartTime {
    /// Convert an optional Unix timestamp to a [`TwapStartTime`].
    ///
    /// `Some(ts)` maps to [`TwapStartTime::At`];
    /// `None` maps to [`TwapStartTime::AtMiningTime`].
    fn from(ts: Option<u32>) -> Self {
        match ts {
            Some(t) => Self::At(t),
            None => Self::AtMiningTime,
        }
    }
}

/// Duration constraint for each individual `TWAP` part.
///
/// - [`DurationOfPart::Auto`] encodes `span = 0` on-chain, meaning each part is valid for the
///   entire `part_duration` window.
/// - [`DurationOfPart::LimitDuration`] encodes `span = duration`, restricting each part to a
///   shorter window within the overall interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DurationOfPart {
    /// Each part is valid for the full `part_duration` window (default).
    #[default]
    Auto,
    /// Each part is valid only for `duration` seconds within the window.
    LimitDuration {
        /// Active window for the part, in seconds. Must be ≤ `part_duration`.
        duration: u32,
    },
}

impl DurationOfPart {
    /// Return the limit duration in seconds, or `None` for [`Auto`](Self::Auto).
    #[must_use]
    pub const fn duration(self) -> Option<u32> {
        match self {
            Self::LimitDuration { duration } => Some(duration),
            Self::Auto => None,
        }
    }

    /// Returns `true` if the part spans the full `part_duration` window.
    #[must_use]
    pub const fn is_auto(self) -> bool {
        matches!(self, Self::Auto)
    }

    /// Construct a [`LimitDuration`](Self::LimitDuration) variant.
    ///
    /// ```
    /// use cow_rs::composable::DurationOfPart;
    ///
    /// let d = DurationOfPart::limit(1_800);
    /// assert!(!d.is_auto());
    /// assert_eq!(d.duration(), Some(1_800));
    /// ```
    #[must_use]
    pub const fn limit(duration: u32) -> Self {
        Self::LimitDuration { duration }
    }

    /// Returns `true` if this is a [`LimitDuration`](Self::LimitDuration) variant.
    ///
    /// ```
    /// use cow_rs::composable::DurationOfPart;
    ///
    /// assert!(DurationOfPart::limit(600).is_limit_duration());
    /// assert!(!DurationOfPart::Auto.is_limit_duration());
    /// ```
    #[must_use]
    pub const fn is_limit_duration(self) -> bool {
        matches!(self, Self::LimitDuration { .. })
    }
}

impl fmt::Display for DurationOfPart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => f.write_str("auto"),
            Self::LimitDuration { duration } => write!(f, "limit-duration({duration}s)"),
        }
    }
}

impl From<Option<u32>> for DurationOfPart {
    /// Convert an optional duration to a [`DurationOfPart`].
    ///
    /// `Some(d)` maps to [`DurationOfPart::LimitDuration`];
    /// `None` maps to [`DurationOfPart::Auto`].
    fn from(d: Option<u32>) -> Self {
        match d {
            Some(duration) => Self::LimitDuration { duration },
            None => Self::Auto,
        }
    }
}

/// Parameters for a Time-Weighted Average Price (`TWAP`) order.
///
/// A `TWAP` order splits a large trade into `num_parts` equal parts executed
/// over `num_parts × part_duration` seconds, reducing market impact.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapData {
    /// Token to sell.
    pub sell_token: Address,
    /// Token to buy.
    pub buy_token: Address,
    /// Address to receive bought tokens (use [`Address::ZERO`] for the order owner).
    pub receiver: Address,
    /// Total amount to sell across all parts.
    pub sell_amount: U256,
    /// Minimum total amount to buy across all parts.
    pub buy_amount: U256,
    /// When to start the `TWAP`.
    pub start_time: TwapStartTime,
    /// Duration of each part in seconds.
    pub part_duration: u32,
    /// Number of parts to split the order into.
    pub num_parts: u32,
    /// App-data hash (use [`B256::ZERO`] for none).
    pub app_data: B256,
    /// Whether each individual part may be partially filled.
    pub partially_fillable: bool,
    /// Order kind (`Sell` or `Buy`).
    pub kind: OrderKind,
    /// How long each part remains valid within its window.
    ///
    /// Defaults to [`DurationOfPart::Auto`] (full window, `span = 0`).
    #[serde(default)]
    pub duration_of_part: DurationOfPart,
}

impl fmt::Display for TwapData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TWAP {} × {}s [{}] sell {} {:#x} → buy ≥ {} {:#x}",
            self.num_parts,
            self.part_duration,
            self.start_time,
            self.sell_amount,
            self.sell_token,
            self.buy_amount,
            self.buy_token,
        )
    }
}

impl TwapData {
    /// Total duration of the `TWAP` order in seconds.
    ///
    /// Equals `num_parts × part_duration`.
    ///
    /// # Returns
    ///
    /// The total duration in seconds as a `u64`.
    #[must_use]
    pub const fn total_duration_secs(&self) -> u64 {
        self.num_parts as u64 * self.part_duration as u64
    }

    /// Absolute Unix timestamp at which the last part expires, if the start
    /// time is known.
    ///
    /// # Returns
    ///
    /// `Some(end_timestamp)` when `start_time` is [`TwapStartTime::At`], computed
    /// as `start + total_duration_secs()`. Returns `None` when `start_time` is
    /// [`TwapStartTime::AtMiningTime`] (the exact start is only known at mining time).
    #[must_use]
    pub const fn end_time(&self) -> Option<u64> {
        match self.start_time {
            TwapStartTime::At(ts) => Some(ts as u64 + self.total_duration_secs()),
            TwapStartTime::AtMiningTime => None,
        }
    }

    /// Returns `true` if this is a sell-direction `TWAP` order.
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::composable::TwapData;
    ///
    /// let twap = TwapData::sell(Address::ZERO, Address::ZERO, U256::ZERO, 4, 3_600);
    /// assert!(twap.is_sell());
    /// assert!(!twap.is_buy());
    /// ```
    #[must_use]
    pub const fn is_sell(&self) -> bool {
        self.kind.is_sell()
    }

    /// Returns `true` if this is a buy-direction `TWAP` order.
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::{
    ///     OrderKind,
    ///     composable::{TwapData, TwapStartTime},
    /// };
    ///
    /// let mut twap = TwapData::sell(Address::ZERO, Address::ZERO, U256::ZERO, 4, 3_600);
    /// twap.kind = OrderKind::Buy;
    /// assert!(twap.is_buy());
    /// assert!(!twap.is_sell());
    /// ```
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        self.kind.is_buy()
    }

    /// Returns `true` if the `TWAP` has fully expired at the given Unix timestamp.
    ///
    /// Returns `false` when `start_time` is [`TwapStartTime::AtMiningTime`]
    /// (the end time is not yet known).
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::composable::{TwapData, TwapStartTime};
    ///
    /// let twap = TwapData::sell(Address::ZERO, Address::ZERO, U256::ZERO, 4, 3_600)
    ///     .with_start_time(TwapStartTime::At(1_000_000));
    /// // ends at 1_000_000 + 4 × 3600 = 1_014_400
    /// assert!(!twap.is_expired(1_014_399));
    /// assert!(twap.is_expired(1_014_400));
    /// ```
    #[must_use]
    pub const fn is_expired(&self, timestamp: u64) -> bool {
        match self.end_time() {
            Some(end) => timestamp >= end,
            None => false,
        }
    }

    /// Create a minimal sell-kind TWAP order.
    ///
    /// Defaults: `receiver = Address::ZERO`, `buy_amount = U256::ZERO` (no min),
    /// `start_time = TwapStartTime::AtMiningTime`, `app_data = B256::ZERO`,
    /// `partially_fillable = false`, `duration_of_part = DurationOfPart::Auto`.
    ///
    /// Use the `with_*` builder methods to set optional fields.
    ///
    /// # Arguments
    ///
    /// * `sell_token` - Address of the token to sell.
    /// * `buy_token` - Address of the token to buy.
    /// * `sell_amount` - Total amount of `sell_token` to sell across all parts.
    /// * `num_parts` - Number of parts to split the order into.
    /// * `part_duration` - Duration of each part in seconds.
    ///
    /// # Returns
    ///
    /// A new [`TwapData`] configured as a sell order with sensible defaults.
    #[must_use]
    pub const fn sell(
        sell_token: Address,
        buy_token: Address,
        sell_amount: U256,
        num_parts: u32,
        part_duration: u32,
    ) -> Self {
        Self {
            sell_token,
            buy_token,
            receiver: Address::ZERO,
            sell_amount,
            buy_amount: U256::ZERO,
            start_time: TwapStartTime::AtMiningTime,
            part_duration,
            num_parts,
            app_data: B256::ZERO,
            partially_fillable: false,
            kind: OrderKind::Sell,
            duration_of_part: DurationOfPart::Auto,
        }
    }

    /// Create a minimal buy-kind TWAP order.
    ///
    /// Defaults: `receiver = Address::ZERO`, `sell_amount = U256::MAX` (unlimited),
    /// `start_time = TwapStartTime::AtMiningTime`, `app_data = B256::ZERO`,
    /// `partially_fillable = false`, `duration_of_part = DurationOfPart::Auto`.
    ///
    /// Use the `with_*` builder methods to set optional fields.
    ///
    /// # Arguments
    ///
    /// * `sell_token` - Address of the token to sell.
    /// * `buy_token` - Address of the token to buy.
    /// * `buy_amount` - Minimum total amount of `buy_token` to receive across all parts.
    /// * `num_parts` - Number of parts to split the order into.
    /// * `part_duration` - Duration of each part in seconds.
    ///
    /// # Returns
    ///
    /// A new [`TwapData`] configured as a buy order with sensible defaults.
    #[must_use]
    pub const fn buy(
        sell_token: Address,
        buy_token: Address,
        buy_amount: U256,
        num_parts: u32,
        part_duration: u32,
    ) -> Self {
        Self {
            sell_token,
            buy_token,
            receiver: Address::ZERO,
            sell_amount: U256::MAX,
            buy_amount,
            start_time: TwapStartTime::AtMiningTime,
            part_duration,
            num_parts,
            app_data: B256::ZERO,
            partially_fillable: false,
            kind: OrderKind::Buy,
            duration_of_part: DurationOfPart::Auto,
        }
    }

    /// Set the receiver address for bought tokens.
    ///
    /// [`Address::ZERO`] means the order owner (default).
    ///
    /// # Returns
    ///
    /// The modified [`TwapData`] with the updated receiver (builder pattern).
    #[must_use]
    pub const fn with_receiver(mut self, receiver: Address) -> Self {
        self.receiver = receiver;
        self
    }

    /// Set the minimum amount of `buy_token` to receive across all parts.
    ///
    /// Useful when building a sell-kind order to set a price floor.
    ///
    /// # Returns
    ///
    /// The modified [`TwapData`] with the updated buy amount (builder pattern).
    #[must_use]
    pub const fn with_buy_amount(mut self, buy_amount: U256) -> Self {
        self.buy_amount = buy_amount;
        self
    }

    /// Set the maximum amount of `sell_token` to sell across all parts.
    ///
    /// Useful when building a buy-kind order to cap spending.
    ///
    /// # Returns
    ///
    /// The modified [`TwapData`] with the updated sell amount (builder pattern).
    #[must_use]
    pub const fn with_sell_amount(mut self, sell_amount: U256) -> Self {
        self.sell_amount = sell_amount;
        self
    }

    /// Set when the TWAP order starts executing.
    ///
    /// # Returns
    ///
    /// The modified [`TwapData`] with the updated start time (builder pattern).
    #[must_use]
    pub const fn with_start_time(mut self, start_time: TwapStartTime) -> Self {
        self.start_time = start_time;
        self
    }

    /// Attach an app-data hash to the order.
    ///
    /// # Returns
    ///
    /// The modified [`TwapData`] with the updated app-data hash (builder pattern).
    #[must_use]
    pub const fn with_app_data(mut self, app_data: B256) -> Self {
        self.app_data = app_data;
        self
    }

    /// Allow each individual part to be partially filled.
    ///
    /// # Returns
    ///
    /// The modified [`TwapData`] with the updated partial-fill setting (builder pattern).
    #[must_use]
    pub const fn with_partially_fillable(mut self, partially_fillable: bool) -> Self {
        self.partially_fillable = partially_fillable;
        self
    }

    /// Restrict each part to a shorter validity window within its overall interval.
    ///
    /// # Returns
    ///
    /// The modified [`TwapData`] with the updated duration-of-part setting (builder pattern).
    #[must_use]
    pub const fn with_duration_of_part(mut self, duration_of_part: DurationOfPart) -> Self {
        self.duration_of_part = duration_of_part;
        self
    }

    /// Returns `true` if a non-zero app-data hash is attached.
    ///
    /// The zero hash (`B256::ZERO`) means no app-data was set.
    ///
    /// ```
    /// use alloy_primitives::{Address, B256, U256};
    /// use cow_rs::composable::TwapData;
    ///
    /// let twap = TwapData::sell(Address::ZERO, Address::ZERO, U256::ZERO, 4, 3_600);
    /// assert!(!twap.has_app_data());
    ///
    /// let with_data = twap.with_app_data(B256::repeat_byte(0x01));
    /// assert!(with_data.has_app_data());
    /// ```
    #[must_use]
    pub fn has_app_data(&self) -> bool {
        !self.app_data.is_zero()
    }
}

/// On-chain `TwapStruct` representation with per-part amounts.
///
/// This mirrors the Solidity struct passed to the handler as `staticInput`.
/// Use [`TwapData`] for the user-facing SDK type; use `TwapStruct` only when
/// you need direct access to the ABI-level fields.
#[derive(Debug, Clone)]
pub struct TwapStruct {
    /// Token to sell.
    pub sell_token: Address,
    /// Token to buy.
    pub buy_token: Address,
    /// Receiver of bought tokens.
    pub receiver: Address,
    /// Amount of `sell_token` to sell in **each** part (not total).
    pub part_sell_amount: U256,
    /// Minimum amount of `buy_token` to buy in **each** part.
    pub min_part_limit: U256,
    /// Start timestamp (`0` = use `CurrentBlockTimestampFactory`).
    pub t0: u32,
    /// Number of parts.
    pub n: u32,
    /// Duration of each part in seconds.
    pub t: u32,
    /// Part validity window in seconds (`0` = full window).
    pub span: u32,
    /// App-data hash.
    pub app_data: B256,
}

impl TwapStruct {
    /// Returns `true` if a non-zero app-data hash is set.
    ///
    /// The zero hash (`B256::ZERO`) means no app-data was attached.
    ///
    /// # Returns
    ///
    /// `true` if the `app_data` field is not [`B256::ZERO`], `false` otherwise.
    #[must_use]
    pub fn has_app_data(&self) -> bool {
        !self.app_data.is_zero()
    }

    /// Returns `true` if the receiver is not the zero address.
    ///
    /// When `receiver == Address::ZERO`, the settlement contract uses the order
    /// owner as the effective receiver.
    ///
    /// # Returns
    ///
    /// `true` if `receiver` is not [`Address::ZERO`], `false` otherwise.
    #[must_use]
    pub fn has_custom_receiver(&self) -> bool {
        !self.receiver.is_zero()
    }

    /// Returns `true` if a fixed start timestamp is set (`t0 != 0`).
    ///
    /// When `t0 == 0`, the order uses `CurrentBlockTimestampFactory` to
    /// determine the start time at mining time.
    ///
    /// # Returns
    ///
    /// `true` if `t0` is non-zero, `false` otherwise.
    #[must_use]
    pub const fn start_is_fixed(&self) -> bool {
        self.t0 != 0
    }
}

impl TryFrom<&TwapData> for TwapStruct {
    type Error = crate::CowError;

    /// Convert a high-level [`TwapData`] into the ABI-level [`TwapStruct`].
    ///
    /// Delegates to [`crate::composable::data_to_struct`].
    fn try_from(d: &TwapData) -> Result<Self, Self::Error> {
        crate::composable::data_to_struct(d)
    }
}

impl From<&TwapStruct> for TwapData {
    /// Convert an ABI-level [`TwapStruct`] back into a high-level [`TwapData`].
    ///
    /// Delegates to [`crate::composable::struct_to_data`].
    fn from(s: &TwapStruct) -> Self {
        crate::composable::struct_to_data(s)
    }
}

impl fmt::Display for TwapStruct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "twap-struct {} × {}s sell {} {:#x} → ≥{} {:#x}",
            self.n,
            self.t,
            self.part_sell_amount,
            self.sell_token,
            self.min_part_limit,
            self.buy_token,
        )
    }
}

// ── GpV2OrderStruct ───────────────────────────────────────────────────────────

/// Raw on-chain `GPv2Order.DataStruct` as emitted by the `GPv2Settlement` contract.
///
/// Unlike [`UnsignedOrder`](crate::order_signing::types::UnsignedOrder), the
/// `kind`, `sell_token_balance`, and `buy_token_balance` fields are stored as
/// `keccak256` hashes rather than typed enums.
///
/// Use [`from_struct_to_order`](crate::composable::from_struct_to_order) to
/// decode them into a fully typed
/// [`UnsignedOrder`](crate::order_signing::types::UnsignedOrder).
///
/// Mirrors `GPv2Order.DataStruct` from the `@cowprotocol/composable` SDK.
#[derive(Debug, Clone)]
pub struct GpV2OrderStruct {
    /// Token to sell.
    pub sell_token: Address,
    /// Token to buy.
    pub buy_token: Address,
    /// Address that receives the bought tokens.
    pub receiver: Address,
    /// Amount of `sell_token` to sell (in atoms).
    pub sell_amount: U256,
    /// Minimum amount of `buy_token` to receive (in atoms).
    pub buy_amount: U256,
    /// Order expiry as a Unix timestamp.
    pub valid_to: u32,
    /// App-data hash (`bytes32`).
    pub app_data: B256,
    /// Protocol fee included in `sell_amount` (in atoms).
    pub fee_amount: U256,
    /// `keccak256("sell")` or `keccak256("buy")`.
    pub kind: B256,
    /// Whether the order may be partially filled.
    pub partially_fillable: bool,
    /// `keccak256("erc20")`, `keccak256("external")`, or `keccak256("internal")`.
    pub sell_token_balance: B256,
    /// `keccak256("erc20")`, `keccak256("external")`, or `keccak256("internal")`.
    pub buy_token_balance: B256,
}
impl fmt::Display for GpV2OrderStruct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "gpv2-order({:#x} sell={} → {:#x} buy={})",
            self.sell_token, self.sell_amount, self.buy_token, self.buy_amount
        )
    }
}

impl GpV2OrderStruct {
    /// Returns `true` if the receiver is not the zero address.
    ///
    /// When `receiver == Address::ZERO`, the settlement contract uses the order
    /// owner as the effective receiver.
    ///
    /// # Returns
    ///
    /// `true` if `receiver` is not [`Address::ZERO`], `false` otherwise.
    #[must_use]
    pub fn has_custom_receiver(&self) -> bool {
        !self.receiver.is_zero()
    }

    /// Returns `true` if this order allows partial fills.
    ///
    /// # Returns
    ///
    /// `true` if `partially_fillable` is set, `false` for fill-or-kill orders.
    #[must_use]
    pub const fn is_partially_fillable(&self) -> bool {
        self.partially_fillable
    }
}

impl TryFrom<&GpV2OrderStruct> for crate::order_signing::types::UnsignedOrder {
    type Error = crate::CowError;

    /// Decode a raw [`GpV2OrderStruct`] into a fully typed `UnsignedOrder`.
    ///
    /// Resolves the hashed `kind`, `sell_token_balance`, and `buy_token_balance`
    /// fields back into their enum representations via
    /// [`crate::composable::from_struct_to_order`].
    fn try_from(s: &GpV2OrderStruct) -> Result<Self, Self::Error> {
        crate::composable::from_struct_to_order(s)
    }
}

// ── PollResult ────────────────────────────────────────────────────────────────

/// Result returned when polling a conditional order for tradability.
///
/// On `Success`, contains the on-chain order struct and the pre-signature bytes
/// ready for submission to the orderbook.
#[derive(Debug, Clone)]
pub enum PollResult {
    /// The order is valid and can be submitted now.
    ///
    /// When returned by a full signing poll, `order` and `signature` are set to
    /// the resolved `GPv2Order.Data` struct and the ABI-encoded signature.
    /// When returned by an offline validity check (e.g. `TwapOrder::poll_validate`),
    /// both fields are `None`.
    Success {
        /// The resolved order ready for submission (`None` for offline checks).
        order: Option<crate::order_signing::types::UnsignedOrder>,
        /// Hex-encoded signature bytes, `0x`-prefixed (`None` for offline checks).
        signature: Option<String>,
    },
    /// Retry on the next block.
    TryNextBlock,
    /// Retry once the given block number is reached.
    TryOnBlock {
        /// Target block number.
        block_number: u64,
    },
    /// Retry once the given Unix timestamp is reached.
    TryAtEpoch {
        /// Target Unix timestamp in seconds.
        epoch: u64,
    },
    /// An unexpected error occurred.
    UnexpectedError {
        /// Human-readable error description.
        message: String,
    },
    /// This order should never be polled again.
    DontTryAgain {
        /// Reason the order is permanently inactive.
        reason: String,
    },
}

impl PollResult {
    /// Returns `true` if the order is ready to be submitted.
    ///
    /// # Returns
    ///
    /// `true` for the [`Success`](Self::Success) variant, `false` for all others.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Returns `true` if polling should be retried in a future block or epoch.
    ///
    /// # Returns
    ///
    /// `true` for [`TryNextBlock`](Self::TryNextBlock), [`TryOnBlock`](Self::TryOnBlock),
    /// or [`TryAtEpoch`](Self::TryAtEpoch); `false` otherwise.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::TryNextBlock | Self::TryOnBlock { .. } | Self::TryAtEpoch { .. })
    }

    /// Returns `true` if this order should never be polled again.
    ///
    /// # Returns
    ///
    /// `true` for the [`DontTryAgain`](Self::DontTryAgain) variant, `false` otherwise.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::DontTryAgain { .. })
    }

    /// Returns `true` if polling should retry on the very next block.
    ///
    /// # Returns
    ///
    /// `true` for the [`TryNextBlock`](Self::TryNextBlock) variant, `false` otherwise.
    #[must_use]
    pub const fn is_try_next_block(&self) -> bool {
        matches!(self, Self::TryNextBlock)
    }

    /// Returns `true` if polling should retry once a specific block is reached.
    ///
    /// # Returns
    ///
    /// `true` for the [`TryOnBlock`](Self::TryOnBlock) variant, `false` otherwise.
    #[must_use]
    pub const fn is_try_on_block(&self) -> bool {
        matches!(self, Self::TryOnBlock { .. })
    }

    /// Returns `true` if polling should retry once a specific Unix epoch is reached.
    ///
    /// # Returns
    ///
    /// `true` for the [`TryAtEpoch`](Self::TryAtEpoch) variant, `false` otherwise.
    #[must_use]
    pub const fn is_try_at_epoch(&self) -> bool {
        matches!(self, Self::TryAtEpoch { .. })
    }

    /// Returns `true` if an unexpected error occurred during polling.
    ///
    /// # Returns
    ///
    /// `true` for the [`UnexpectedError`](Self::UnexpectedError) variant, `false` otherwise.
    #[must_use]
    pub const fn is_unexpected_error(&self) -> bool {
        matches!(self, Self::UnexpectedError { .. })
    }

    /// Returns `true` if this order should never be polled again (terminal failure).
    ///
    /// # Returns
    ///
    /// `true` for the [`DontTryAgain`](Self::DontTryAgain) variant, `false` otherwise.
    #[must_use]
    pub const fn is_dont_try_again(&self) -> bool {
        matches!(self, Self::DontTryAgain { .. })
    }

    /// Extract the target block number from a [`TryOnBlock`](Self::TryOnBlock) variant.
    ///
    /// Returns `None` for all other variants.
    ///
    /// ```
    /// use cow_rs::composable::PollResult;
    ///
    /// let r = PollResult::TryOnBlock { block_number: 12_345_678 };
    /// assert_eq!(r.get_block_number(), Some(12_345_678));
    /// assert_eq!(PollResult::TryNextBlock.get_block_number(), None);
    /// ```
    #[must_use]
    pub const fn get_block_number(&self) -> Option<u64> {
        if let Self::TryOnBlock { block_number } = self { Some(*block_number) } else { None }
    }

    /// Extract the target Unix epoch from a [`TryAtEpoch`](Self::TryAtEpoch) variant.
    ///
    /// Returns `None` for all other variants.
    ///
    /// ```
    /// use cow_rs::composable::PollResult;
    ///
    /// let r = PollResult::TryAtEpoch { epoch: 1_700_000_000 };
    /// assert_eq!(r.get_epoch(), Some(1_700_000_000));
    /// assert_eq!(PollResult::TryNextBlock.get_epoch(), None);
    /// ```
    #[must_use]
    pub const fn get_epoch(&self) -> Option<u64> {
        if let Self::TryAtEpoch { epoch } = self { Some(*epoch) } else { None }
    }

    /// Extract the resolved [`UnsignedOrder`](crate::order_signing::types::UnsignedOrder)
    /// from a [`PollResult::Success`] variant, if present.
    ///
    /// Returns `None` for all other variants, or when `order` is `None`
    /// inside `Success` (e.g. an offline validity check result).
    #[must_use]
    pub const fn order_ref(&self) -> Option<&crate::order_signing::types::UnsignedOrder> {
        if let Self::Success { order, .. } = self { order.as_ref() } else { None }
    }

    /// Extract the error message from an [`UnexpectedError`](Self::UnexpectedError)
    /// or [`DontTryAgain`](Self::DontTryAgain) variant.
    ///
    /// Returns `None` for all other variants.
    #[must_use]
    pub const fn as_error_message(&self) -> Option<&str> {
        match self {
            Self::UnexpectedError { message } => Some(message.as_str()),
            Self::DontTryAgain { reason } => Some(reason.as_str()),
            Self::Success { .. } |
            Self::TryNextBlock |
            Self::TryOnBlock { .. } |
            Self::TryAtEpoch { .. } => None,
        }
    }
}

impl fmt::Display for PollResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success { .. } => f.write_str("success"),
            Self::TryNextBlock => f.write_str("try-next-block"),
            Self::TryOnBlock { block_number } => write!(f, "try-on-block({block_number})"),
            Self::TryAtEpoch { epoch } => write!(f, "try-at-epoch({epoch})"),
            Self::UnexpectedError { message } => write!(f, "unexpected-error({message})"),
            Self::DontTryAgain { reason } => write!(f, "dont-try-again({reason})"),
        }
    }
}

// ── ProofLocation ─────────────────────────────────────────────────────────────

/// Where the Merkle proof for a conditional order is stored / communicated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum ProofLocation {
    /// Proof is kept private; only the owner polls.
    #[default]
    Private = 0,
    /// Proof emitted as an on-chain event.
    Emitted = 1,
    /// Proof stored on Swarm.
    Swarm = 2,
    /// Proof communicated via Waku.
    Waku = 3,
    /// Reserved for future use.
    Reserved = 4,
    /// Proof stored on IPFS.
    Ipfs = 5,
}

impl ProofLocation {
    /// Returns a lowercase string label for the proof location.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Emitted => "emitted",
            Self::Swarm => "swarm",
            Self::Waku => "waku",
            Self::Reserved => "reserved",
            Self::Ipfs => "ipfs",
        }
    }

    /// Returns `true` if the proof is kept private (owner-polled only).
    #[must_use]
    pub const fn is_private(self) -> bool {
        matches!(self, Self::Private)
    }

    /// Returns `true` if the proof is emitted as an on-chain event.
    #[must_use]
    pub const fn is_emitted(self) -> bool {
        matches!(self, Self::Emitted)
    }

    /// Returns `true` if the proof is stored on Swarm.
    #[must_use]
    pub const fn is_swarm(self) -> bool {
        matches!(self, Self::Swarm)
    }

    /// Returns `true` if the proof is communicated via Waku.
    #[must_use]
    pub const fn is_waku(self) -> bool {
        matches!(self, Self::Waku)
    }

    /// Returns `true` if this location is the reserved (future-use) discriminant.
    #[must_use]
    pub const fn is_reserved(self) -> bool {
        matches!(self, Self::Reserved)
    }

    /// Returns `true` if the proof is stored on IPFS.
    #[must_use]
    pub const fn is_ipfs(self) -> bool {
        matches!(self, Self::Ipfs)
    }
}

impl fmt::Display for ProofLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<u8> for ProofLocation {
    type Error = crate::CowError;

    /// Parse a [`ProofLocation`] from its on-chain `uint8` discriminant.
    fn try_from(n: u8) -> Result<Self, Self::Error> {
        match n {
            0 => Ok(Self::Private),
            1 => Ok(Self::Emitted),
            2 => Ok(Self::Swarm),
            3 => Ok(Self::Waku),
            4 => Ok(Self::Reserved),
            5 => Ok(Self::Ipfs),
            other => Err(crate::CowError::Parse {
                field: "ProofLocation",
                reason: format!("unknown discriminant: {other}"),
            }),
        }
    }
}

impl TryFrom<&str> for ProofLocation {
    type Error = crate::CowError;

    /// Parse a [`ProofLocation`] from its string label.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "private" => Ok(Self::Private),
            "emitted" => Ok(Self::Emitted),
            "swarm" => Ok(Self::Swarm),
            "waku" => Ok(Self::Waku),
            "reserved" => Ok(Self::Reserved),
            "ipfs" => Ok(Self::Ipfs),
            other => Err(crate::CowError::Parse {
                field: "ProofLocation",
                reason: format!("unknown value: {other}"),
            }),
        }
    }
}

impl From<ProofLocation> for u8 {
    /// Encode a [`ProofLocation`] as its on-chain `uint8` discriminant.
    ///
    /// This is the inverse of [`TryFrom<u8>`] for [`ProofLocation`].
    fn from(loc: ProofLocation) -> Self {
        loc as Self
    }
}

impl ProofStruct {
    /// Construct a [`ProofStruct`] with the given location and data bytes.
    ///
    /// # Arguments
    ///
    /// * `location` - Where the Merkle proof is stored or communicated.
    /// * `data` - Location-specific proof bytes (empty for private or emitted proofs).
    ///
    /// # Returns
    ///
    /// A new [`ProofStruct`] instance.
    #[must_use]
    pub const fn new(location: ProofLocation, data: Vec<u8>) -> Self {
        Self { location, data }
    }

    /// A private proof (no location data needed).
    ///
    /// # Returns
    ///
    /// A [`ProofStruct`] with [`ProofLocation::Private`] and empty data.
    #[must_use]
    pub const fn private() -> Self {
        Self { location: ProofLocation::Private, data: Vec::new() }
    }

    /// An emitted proof (no location data needed — the proof is in the tx log).
    ///
    /// # Returns
    ///
    /// A [`ProofStruct`] with [`ProofLocation::Emitted`] and empty data.
    #[must_use]
    pub const fn emitted() -> Self {
        Self { location: ProofLocation::Emitted, data: Vec::new() }
    }

    /// Override the proof location.
    ///
    /// # Returns
    ///
    /// The modified [`ProofStruct`] with the updated location (builder pattern).
    #[must_use]
    pub const fn with_location(mut self, location: ProofLocation) -> Self {
        self.location = location;
        self
    }

    /// Override the location-specific proof data bytes.
    ///
    /// # Returns
    ///
    /// The modified [`ProofStruct`] with the updated data (builder pattern).
    #[must_use]
    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    /// Returns `true` if the proof location is [`ProofLocation::Private`].
    ///
    /// # Returns
    ///
    /// `true` if `location` is [`ProofLocation::Private`], `false` otherwise.
    #[must_use]
    pub const fn is_private(&self) -> bool {
        self.location.is_private()
    }

    /// Returns `true` if the proof location is [`ProofLocation::Emitted`].
    ///
    /// # Returns
    ///
    /// `true` if `location` is [`ProofLocation::Emitted`], `false` otherwise.
    #[must_use]
    pub const fn is_emitted(&self) -> bool {
        self.location.is_emitted()
    }

    /// Returns `true` if the proof location is [`ProofLocation::Swarm`].
    ///
    /// # Returns
    ///
    /// `true` if `location` is [`ProofLocation::Swarm`], `false` otherwise.
    #[must_use]
    pub const fn is_swarm(&self) -> bool {
        self.location.is_swarm()
    }

    /// Returns `true` if the proof location is [`ProofLocation::Waku`].
    ///
    /// # Returns
    ///
    /// `true` if `location` is [`ProofLocation::Waku`], `false` otherwise.
    #[must_use]
    pub const fn is_waku(&self) -> bool {
        self.location.is_waku()
    }

    /// Returns `true` if the proof location is [`ProofLocation::Ipfs`].
    ///
    /// # Returns
    ///
    /// `true` if `location` is [`ProofLocation::Ipfs`], `false` otherwise.
    #[must_use]
    pub const fn is_ipfs(&self) -> bool {
        self.location.is_ipfs()
    }

    /// Returns `true` if the proof location is [`ProofLocation::Reserved`].
    ///
    /// # Returns
    ///
    /// `true` if `location` is [`ProofLocation::Reserved`], `false` otherwise.
    #[must_use]
    pub const fn is_reserved(&self) -> bool {
        self.location.is_reserved()
    }

    /// Returns `true` if this proof has non-empty data bytes.
    ///
    /// [`ProofLocation::Private`] and [`ProofLocation::Emitted`] proofs carry no
    /// data; IPFS, Swarm, and Waku proofs carry location-specific bytes.
    #[must_use]
    pub const fn has_data(&self) -> bool {
        !self.data.is_empty()
    }

    /// Returns `true` if this proof has no data bytes (complement of [`has_data`](Self::has_data)).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the number of data bytes in this proof.
    #[must_use]
    pub const fn data_len(&self) -> usize {
        self.data.len()
    }
}

impl fmt::Display for ProofStruct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "proof({})", self.location)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twap_data_display_at_mining_time() {
        let data = TwapData {
            sell_token: Address::ZERO,
            buy_token: Address::ZERO,
            receiver: Address::ZERO,
            sell_amount: U256::from(24_000u64),
            buy_amount: U256::from(1_000u64),
            start_time: TwapStartTime::AtMiningTime,
            part_duration: 3_600,
            num_parts: 24,
            app_data: B256::ZERO,
            partially_fillable: false,
            kind: OrderKind::Sell,
            duration_of_part: DurationOfPart::Auto,
        };
        let s = data.to_string();
        assert!(s.contains("24 × 3600s"));
        assert!(s.contains("at-mining-time"));
        assert!(s.contains("24000"));
        assert!(s.contains("1000"));
    }

    #[test]
    fn twap_data_display_fixed_start() {
        let data = TwapData {
            sell_token: Address::ZERO,
            buy_token: Address::ZERO,
            receiver: Address::ZERO,
            sell_amount: U256::from(1_000u64),
            buy_amount: U256::from(500u64),
            start_time: TwapStartTime::At(1_700_000_000),
            part_duration: 7_200,
            num_parts: 6,
            app_data: B256::ZERO,
            partially_fillable: false,
            kind: OrderKind::Sell,
            duration_of_part: DurationOfPart::Auto,
        };
        let s = data.to_string();
        assert!(s.contains("at-unix-1700000000"));
    }
}

// ── ProofStruct ───────────────────────────────────────────────────────────────

/// On-chain `Proof` argument passed to `ComposableCow::setRoot`.
///
/// Bundles the proof location discriminant with location-specific data
/// (e.g. an IPFS CID, Swarm hash, or Waku message).  Pass `data: vec![]` for
/// [`ProofLocation::Private`] or [`ProofLocation::Emitted`].
#[derive(Debug, Clone)]
pub struct ProofStruct {
    /// Where the Merkle proof is stored/communicated.
    pub location: ProofLocation,
    /// Location-specific proof bytes (empty for private or emitted proofs).
    pub data: Vec<u8>,
}

// ── Block info ──────────────────────────────────────────────────────────────

/// Block information used for conditional order validation.
///
/// Mirrors `BlockInfo` from the `TypeScript` SDK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockInfo {
    /// Block number.
    pub block_number: u64,
    /// Block timestamp (Unix seconds).
    pub block_timestamp: u64,
}

impl BlockInfo {
    /// Construct a new [`BlockInfo`].
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block number.
    /// * `block_timestamp` - The block timestamp in Unix seconds.
    ///
    /// # Returns
    ///
    /// A new [`BlockInfo`] instance.
    #[must_use]
    pub const fn new(block_number: u64, block_timestamp: u64) -> Self {
        Self { block_number, block_timestamp }
    }
}

impl fmt::Display for BlockInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "block(#{}, ts={})", self.block_number, self.block_timestamp)
    }
}

// ── IsValidResult ───────────────────────────────────────────────────────────

/// Result of validating a conditional order.
///
/// Mirrors the `IsValidResult` union type from the `TypeScript` SDK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IsValidResult {
    /// The order is valid.
    Valid,
    /// The order is invalid, with a reason.
    Invalid {
        /// Human-readable reason why the order is invalid.
        reason: String,
    },
}

/// Type alias for the valid variant, mirroring the `TypeScript` SDK's `IsValid` interface.
pub type IsValid = ();

/// Type alias for the invalid variant, mirroring the `TypeScript` SDK's `IsNotValid` interface.
pub type IsNotValid = String;

impl IsValidResult {
    /// Returns `true` if the result represents a valid order.
    ///
    /// # Returns
    ///
    /// `true` for the [`Valid`](Self::Valid) variant, `false` for [`Invalid`](Self::Invalid).
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Returns the reason string if the result represents an invalid order.
    ///
    /// # Returns
    ///
    /// `Some(reason)` for the [`Invalid`](Self::Invalid) variant, `None` for
    /// [`Valid`](Self::Valid).
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Valid => None,
            Self::Invalid { reason } => Some(reason),
        }
    }

    /// Create a valid result.
    ///
    /// # Returns
    ///
    /// An [`IsValidResult::Valid`] instance.
    #[must_use]
    pub const fn valid() -> Self {
        Self::Valid
    }

    /// Create an invalid result with the given reason.
    ///
    /// # Arguments
    ///
    /// * `reason` - A human-readable explanation of why the order is invalid.
    ///
    /// # Returns
    ///
    /// An [`IsValidResult::Invalid`] instance containing the reason.
    #[must_use]
    pub fn invalid(reason: impl Into<String>) -> Self {
        Self::Invalid { reason: reason.into() }
    }
}

impl fmt::Display for IsValidResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Valid => f.write_str("valid"),
            Self::Invalid { reason } => write!(f, "invalid: {reason}"),
        }
    }
}

// ── Test helpers ────────────────────────────────────────────────────────────

/// Default parameters for a test conditional order.
///
/// Mirrors `DEFAULT_ORDER_PARAMS` from the `TypeScript` SDK test helper.
pub const DEFAULT_TEST_HANDLER: &str = "0x910d00a310f7Dc5B29FE73458F47f519be547D3d";

/// Default salt for test conditional orders.
pub const DEFAULT_TEST_SALT: &str =
    "0x9379a0bf532ff9a66ffde940f94b1a025d6f18803054c1aef52dc94b15255bbe";

/// Parameters for creating a test conditional order.
///
/// Mirrors `TestConditionalOrderParams` from the `TypeScript` SDK.
#[derive(Debug, Clone)]
pub struct TestConditionalOrderParams {
    /// Handler contract address.
    pub handler: Address,
    /// 32-byte salt.
    pub salt: B256,
    /// Static input data.
    pub static_input: Vec<u8>,
    /// Whether this is a single order (true) or part of a Merkle tree (false).
    pub is_single_order: bool,
}

impl Default for TestConditionalOrderParams {
    fn default() -> Self {
        Self {
            handler: DEFAULT_TEST_HANDLER.parse().map_or(Address::ZERO, |a| a),
            salt: DEFAULT_TEST_SALT.parse().map_or(B256::ZERO, |s| s),
            static_input: Vec::new(),
            is_single_order: true,
        }
    }
}

/// Create a test [`ConditionalOrderParams`] with optional overrides.
///
/// Mirrors `createTestConditionalOrder` from the `TypeScript` SDK.
/// Useful in tests to quickly construct valid conditional order params.
///
/// # Example
///
/// ```rust
/// use cow_rs::composable::create_test_conditional_order;
///
/// let params = create_test_conditional_order(None);
/// assert!(!params.handler.is_zero());
/// ```
#[must_use]
pub fn create_test_conditional_order(
    overrides: Option<TestConditionalOrderParams>,
) -> ConditionalOrderParams {
    let test = overrides.unwrap_or_default();
    ConditionalOrderParams {
        handler: test.handler,
        salt: test.salt,
        static_input: test.static_input,
    }
}
