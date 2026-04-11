//! `TWAP` (Time-Weighted Average Price) conditional order and composable-order utilities.

use std::fmt;

use alloy_primitives::{Address, B256, U256, keccak256};

use crate::{error::CowError, types::OrderKind};

use super::types::{
    ConditionalOrderParams, DurationOfPart, PollResult, TWAP_HANDLER_ADDRESS, TwapData,
    TwapStartTime, TwapStruct,
};

/// A `TWAP` order ready to be submitted to `ComposableCow`.
#[derive(Debug, Clone)]
pub struct TwapOrder {
    /// The underlying `TWAP` configuration.
    pub data: TwapData,
    /// 32-byte salt uniquely identifying this order instance.
    pub salt: B256,
}

impl TwapOrder {
    /// Create a new `TWAP` order.
    ///
    /// The salt is derived deterministically from the order parameters.
    /// Use [`TwapOrder::with_salt`] to supply an explicit salt.
    ///
    /// # Example
    ///
    /// ```rust
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::composable::{TwapData, TwapOrder};
    ///
    /// let data = TwapData::sell(Address::ZERO, Address::ZERO, U256::from(1000u64), 4, 3600);
    /// let order = TwapOrder::new(data);
    /// assert_eq!(order.data.num_parts, 4);
    /// assert_eq!(order.data.part_duration, 3600);
    /// ```
    #[must_use]
    pub fn new(data: TwapData) -> Self {
        let salt = deterministic_salt(&data);
        Self { data, salt }
    }

    /// Create a `TWAP` order with an explicit salt.
    ///
    /// # Arguments
    ///
    /// * `data` - The `TWAP` order configuration.
    /// * `salt` - A caller-chosen 32-byte salt to uniquely identify this order.
    ///
    /// # Returns
    ///
    /// A [`TwapOrder`] using the provided salt verbatim.
    #[must_use]
    pub const fn with_salt(data: TwapData, salt: B256) -> Self {
        Self { data, salt }
    }

    /// Returns the on-chain [`ConditionalOrderParams`] for this order.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] if ABI encoding fails.
    pub fn to_params(&self) -> Result<ConditionalOrderParams, CowError> {
        Ok(ConditionalOrderParams {
            handler: TWAP_HANDLER_ADDRESS,
            salt: self.salt,
            static_input: encode_twap_static_input(&self.data)?,
        })
    }

    /// Unique order ID: `keccak256(abi.encode(ConditionalOrderParams))`.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] if encoding fails.
    pub fn id(&self) -> Result<B256, CowError> {
        Ok(order_id(&self.to_params()?))
    }

    /// Validate the order parameters.
    ///
    /// Mirrors the `TypeScript` SDK `Twap.isValid()` logic:
    /// - Tokens must differ and must not be the zero address
    /// - `sell_amount` and `buy_amount` must be non-zero
    /// - `num_parts` must be ≥ 2
    /// - `part_duration` must be > 0 and ≤ [`MAX_FREQUENCY`](super::types::MAX_FREQUENCY)
    /// - `sell_amount` must be divisible by `num_parts`
    /// - If [`DurationOfPart::LimitDuration`]: `duration` must be ≤ `part_duration`
    #[must_use]
    pub fn is_valid(&self) -> bool {
        use super::types::MAX_FREQUENCY;
        let d = &self.data;
        if d.sell_token == d.buy_token {
            return false;
        }
        if d.sell_token.is_zero() || d.buy_token.is_zero() {
            return false;
        }
        if d.sell_amount.is_zero() || d.buy_amount.is_zero() {
            return false;
        }
        if d.num_parts < 2 {
            return false;
        }
        if d.part_duration == 0 || d.part_duration > MAX_FREQUENCY {
            return false;
        }
        if !(d.sell_amount % U256::from(d.num_parts)).is_zero() {
            return false;
        }
        if let DurationOfPart::LimitDuration { duration } = d.duration_of_part &&
            duration > d.part_duration
        {
            return false;
        }
        true
    }

    /// Return the per-part sell and buy amounts `(part_sell, min_part_buy)`.
    ///
    /// These are the amounts used in each individual order slice:
    /// `sell_amount / num_parts` and `buy_amount / num_parts`.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] if `num_parts` is zero.
    ///
    /// # Example
    ///
    /// ```rust
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::composable::{TwapData, TwapOrder};
    ///
    /// let data = TwapData::sell(Address::ZERO, Address::ZERO, U256::from(1000u64), 4, 3600)
    ///     .with_buy_amount(U256::from(800u64));
    /// let order = TwapOrder::new(data);
    /// let (sell, buy) = order.per_part_amounts().unwrap();
    /// assert_eq!(sell, U256::from(250u64));
    /// assert_eq!(buy, U256::from(200u64));
    /// ```
    #[allow(clippy::type_complexity, reason = "two-element tuple is readable as-is")]
    pub fn per_part_amounts(&self) -> Result<(U256, U256), CowError> {
        let n = self.data.num_parts;
        if n == 0 {
            return Err(CowError::AppData("num_parts must be > 0".into()));
        }
        let divisor = U256::from(n);
        Ok((self.data.sell_amount / divisor, self.data.buy_amount / divisor))
    }

    /// Convert this order's user-facing data into the on-chain [`TwapStruct`] representation.
    ///
    /// The struct uses per-part amounts and the raw `span`/`t0` fields.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] if `num_parts` is zero.
    pub fn to_struct(&self) -> Result<TwapStruct, CowError> {
        data_to_struct(&self.data)
    }

    /// Check tradability of this `TWAP` order at the given block timestamp.
    ///
    /// Returns [`PollResult::Success`] if the order is within its execution
    /// window, [`PollResult::TryAtEpoch`] if the order has not started yet,
    /// or [`PollResult::DontTryAgain`] if the order has fully expired.
    ///
    /// For [`TwapStartTime::AtMiningTime`] orders, always returns
    /// [`PollResult::Success`] because the start time is not known until mined.
    #[must_use]
    pub fn poll_validate(&self, block_timestamp: u64) -> PollResult {
        let d = &self.data;
        let start = match d.start_time {
            TwapStartTime::AtMiningTime => {
                return PollResult::Success { order: None, signature: None };
            }
            TwapStartTime::At(ts) => u64::from(ts),
        };
        let end = start + u64::from(d.num_parts) * u64::from(d.part_duration);

        if block_timestamp < start {
            return PollResult::TryAtEpoch { epoch: start };
        }
        if block_timestamp >= end {
            return PollResult::DontTryAgain { reason: "TWAP order has fully expired".into() };
        }
        PollResult::Success { order: None, signature: None }
    }
}

impl TwapOrder {
    /// Returns a reference to the 32-byte salt.
    ///
    /// # Returns
    ///
    /// A shared reference to the [`B256`] salt that uniquely identifies this
    /// order instance within a `ComposableCow` safe.
    #[must_use]
    pub const fn salt_ref(&self) -> &B256 {
        &self.salt
    }

    /// Returns a reference to the underlying [`TwapData`].
    ///
    /// # Returns
    ///
    /// A shared reference to the [`TwapData`] configuration backing this order.
    #[must_use]
    pub const fn data_ref(&self) -> &TwapData {
        &self.data
    }

    /// Returns the total sell amount across all parts.
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::composable::{TwapData, TwapOrder};
    ///
    /// let data = TwapData::sell(Address::ZERO, Address::ZERO, U256::from(1_000u64), 4, 3_600);
    /// let order = TwapOrder::new(data);
    /// assert_eq!(order.total_sell_amount(), U256::from(1_000u64));
    /// ```
    #[must_use]
    pub const fn total_sell_amount(&self) -> U256 {
        self.data.sell_amount
    }

    /// Returns the total minimum buy amount across all parts.
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::composable::{TwapData, TwapOrder};
    ///
    /// let data = TwapData::sell(Address::ZERO, Address::ZERO, U256::ZERO, 4, 3_600)
    ///     .with_buy_amount(U256::from(800u64));
    /// let order = TwapOrder::new(data);
    /// assert_eq!(order.total_buy_amount(), U256::from(800u64));
    /// ```
    #[must_use]
    pub const fn total_buy_amount(&self) -> U256 {
        self.data.buy_amount
    }

    /// Returns `true` if this is a sell-direction `TWAP` order.
    ///
    /// # Returns
    ///
    /// `true` when the order kind is [`OrderKind::Sell`], `false` otherwise.
    #[must_use]
    pub const fn is_sell(&self) -> bool {
        self.data.is_sell()
    }

    /// Returns `true` if this is a buy-direction `TWAP` order.
    ///
    /// # Returns
    ///
    /// `true` when the order kind is [`OrderKind::Buy`], `false` otherwise.
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        self.data.is_buy()
    }

    /// Returns `true` if the order has fully expired at the given Unix timestamp.
    ///
    /// Delegates to [`TwapData::is_expired`]. Returns `false` when the start
    /// time is [`TwapStartTime::AtMiningTime`] (end time is unknown until mined).
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::composable::{TwapData, TwapOrder, TwapStartTime};
    ///
    /// let data = TwapData::sell(Address::ZERO, Address::ZERO, U256::ZERO, 4, 3_600)
    ///     .with_start_time(TwapStartTime::At(1_000_000));
    /// // ends at 1_000_000 + 4 × 3600 = 1_014_400
    /// let order = TwapOrder::new(data);
    /// assert!(!order.is_expired_at(1_014_399));
    /// assert!(order.is_expired_at(1_014_400));
    /// ```
    #[must_use]
    pub const fn is_expired_at(&self, block_timestamp: u64) -> bool {
        self.data.is_expired(block_timestamp)
    }

    /// Return the fixed start timestamp, or `None` when the order starts at mining time.
    ///
    /// Mirrors [`TwapData::start_time`]`::timestamp()`.
    #[must_use]
    pub const fn start_timestamp(&self) -> Option<u32> {
        self.data.start_time.timestamp()
    }
}

impl fmt::Display for TwapOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.data, f)
    }
}

/// Compute the on-chain order ID from [`ConditionalOrderParams`].
///
/// The order ID is `keccak256(abi.encode(handler, salt, staticInput))` where
/// `staticInput` is encoded as a dynamic `bytes` field with a 32-byte length
/// prefix, zero-padded to a 32-byte boundary.
///
/// Mirrors `ConditionalOrder.id` in the `TypeScript` SDK.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, B256};
/// use cow_rs::composable::{ConditionalOrderParams, order_id};
///
/// let params = ConditionalOrderParams {
///     handler: Address::ZERO,
///     salt: B256::ZERO,
///     static_input: vec![0xab, 0xcd],
/// };
/// let id = order_id(&params);
/// assert_ne!(id, B256::ZERO);
/// ```
#[must_use]
pub fn order_id(params: &ConditionalOrderParams) -> B256 {
    let mut buf = Vec::with_capacity(4 * 32 + pad32_len(params.static_input.len()));
    buf.extend_from_slice(&pad_address(params.handler.as_slice()));
    buf.extend_from_slice(params.salt.as_slice());
    buf.extend_from_slice(&u256_be(96u64));
    buf.extend_from_slice(&u256_be(params.static_input.len() as u64));
    pad_into(&mut buf, &params.static_input);
    keccak256(&buf)
}

/// ABI-encode [`ConditionalOrderParams`] into a `0x`-prefixed hex string.
///
/// Encodes `(address handler, bytes32 salt, bytes staticInput)` using the
/// standard ABI tuple encoding used by `ComposableCow` watchtowers. The
/// `staticInput` field is encoded as dynamic `bytes` with a 32-byte length
/// prefix, zero-padded to a 32-byte boundary.
///
/// Decode with [`decode_params`]. Mirrors `encodeParams` from the `TypeScript`
/// SDK's composable utils.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::Address;
/// use cow_rs::composable::{ConditionalOrderParams, encode_params};
///
/// let params = ConditionalOrderParams {
///     handler: Address::ZERO,
///     salt: alloy_primitives::B256::ZERO,
///     static_input: vec![0xab, 0xcd],
/// };
/// let hex = encode_params(&params);
/// assert!(hex.starts_with("0x"));
/// assert_eq!(hex.len(), 2 + 2 * (5 * 32)); // head (3 × 32) + len (32) + padded data (32)
/// ```
#[must_use]
pub fn encode_params(params: &ConditionalOrderParams) -> String {
    let static_len = params.static_input.len();
    let padded_len = pad32_len(static_len);
    let mut buf = Vec::with_capacity(4 * 32 + padded_len);
    buf.extend_from_slice(&pad_address(params.handler.as_slice()));
    buf.extend_from_slice(params.salt.as_slice());
    buf.extend_from_slice(&u256_be(96u64)); // offset to dynamic bytes = 3 * 32
    buf.extend_from_slice(&u256_be(static_len as u64));
    pad_into(&mut buf, &params.static_input);
    format!("0x{}", alloy_primitives::hex::encode(&buf))
}

/// ABI-decode a hex string into [`ConditionalOrderParams`].
///
/// Reverses [`encode_params`]: reads `(address, bytes32, bytes)` from a
/// `0x`-prefixed hex string.  Mirrors `decodeParams` from the `TypeScript` SDK.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if the hex is invalid or the data is too short.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::Address;
/// use cow_rs::composable::{ConditionalOrderParams, decode_params, encode_params};
///
/// let params = ConditionalOrderParams {
///     handler: Address::ZERO,
///     salt: alloy_primitives::B256::ZERO,
///     static_input: vec![0xde, 0xad],
/// };
/// let encoded = encode_params(&params);
/// let decoded = decode_params(&encoded).unwrap();
/// assert_eq!(decoded.handler, params.handler);
/// assert_eq!(decoded.salt, params.salt);
/// assert_eq!(decoded.static_input, params.static_input);
/// ```
pub fn decode_params(hex: &str) -> Result<ConditionalOrderParams, CowError> {
    let stripped = hex.trim_start_matches("0x");
    let bytes = alloy_primitives::hex::decode(stripped)
        .map_err(|e| CowError::AppData(format!("decode_params hex: {e}")))?;
    if bytes.len() < 4 * 32 {
        return Err(CowError::AppData(format!(
            "decode_params: too short ({} bytes, need ≥ 128)",
            bytes.len()
        )));
    }
    let mut handler_bytes = [0u8; 20];
    handler_bytes.copy_from_slice(&bytes[12..32]);
    let handler = Address::new(handler_bytes);

    let mut salt_bytes = [0u8; 32];
    salt_bytes.copy_from_slice(&bytes[32..64]);
    let salt = B256::new(salt_bytes);

    // bytes[64..96] = offset (should be 96, but we tolerate any valid offset)
    let data_len = usize::try_from(U256::from_be_slice(&bytes[96..128]))
        .map_err(|_e| CowError::AppData("decode_params: static_input length overflow".into()))?;
    let data_end = 128usize
        .checked_add(data_len)
        .ok_or_else(|| CowError::AppData("decode_params: static_input length overflow".into()))?;
    if bytes.len() < data_end {
        return Err(CowError::AppData(format!(
            "decode_params: data truncated (need {data_end} bytes, have {})",
            bytes.len()
        )));
    }
    let static_input = bytes[128..data_end].to_vec();
    Ok(ConditionalOrderParams { handler, salt, static_input })
}

/// Format a Unix timestamp as an `RFC 3339` / `ISO 8601` date-time string.
///
/// Mirrors `formatEpoch` from the `TypeScript` SDK composable utils.
///
/// # Example
///
/// ```rust
/// use cow_rs::composable::format_epoch;
///
/// let s = format_epoch(1_700_000_000);
/// assert!(s.starts_with("2023-11-14"));
/// ```
#[must_use]
pub fn format_epoch(epoch: u32) -> String {
    use chrono::{DateTime, Utc};
    DateTime::<Utc>::from_timestamp(i64::from(epoch), 0)
        .map_or_else(|| format!("{epoch}"), |dt| dt.to_rfc3339())
}

// ── ABI encoding ──────────────────────────────────────────────────────────────

/// ABI-encode the `TWAP` static input from user-facing [`TwapData`].
///
/// Converts total amounts to per-part via [`data_to_struct`], then encodes
/// the resulting [`TwapStruct`] as a 320-byte ABI tuple:
///
/// ```text
/// (address sellToken, address buyToken, address receiver,
///  uint256 partSellAmount, uint256 minPartLimit,
///  uint32 t0, uint32 n, uint32 t, uint32 span,
///  bytes32 appData)
/// ```
///
/// Note: the contract takes **per-part** amounts, not totals.
///
/// # Arguments
///
/// * `d` - The user-facing `TWAP` order data with total amounts.
///
/// # Returns
///
/// A 320-byte `Vec<u8>` containing the ABI-encoded `TWAP` static input,
/// or a [`CowError::AppData`] if `num_parts` is zero.
fn encode_twap_static_input(d: &TwapData) -> Result<Vec<u8>, CowError> {
    let s = data_to_struct(d)?;
    Ok(encode_struct(&s))
}

/// Convert [`TwapData`] (user-facing, total amounts) into a [`TwapStruct`] (per-part amounts).
///
/// Divides `sell_amount` and `buy_amount` by `num_parts` to produce
/// `part_sell_amount` and `min_part_limit`.  Maps [`TwapStartTime`] and
/// [`DurationOfPart`] to their raw `t0` / `span` `u32` representations.
///
/// This is the inverse of [`struct_to_data`].
///
/// # Errors
///
/// Returns [`CowError::AppData`] if `num_parts` is zero.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, U256};
/// use cow_rs::composable::{TwapData, data_to_struct};
///
/// let data = TwapData::sell(Address::ZERO, Address::ZERO, U256::from(1000u64), 4, 3600)
///     .with_buy_amount(U256::from(800u64));
/// let s = data_to_struct(&data).unwrap();
/// assert_eq!(s.part_sell_amount, U256::from(250u64));
/// assert_eq!(s.min_part_limit, U256::from(200u64));
/// assert_eq!(s.n, 4);
/// assert_eq!(s.t, 3600);
/// ```
pub fn data_to_struct(d: &TwapData) -> Result<TwapStruct, CowError> {
    if d.num_parts == 0 {
        return Err(CowError::AppData("num_parts must be > 0".into()));
    }
    let n = U256::from(d.num_parts);
    Ok(TwapStruct {
        sell_token: d.sell_token,
        buy_token: d.buy_token,
        receiver: d.receiver,
        part_sell_amount: d.sell_amount / n,
        min_part_limit: d.buy_amount / n,
        t0: match d.start_time {
            TwapStartTime::AtMiningTime => 0,
            TwapStartTime::At(ts) => ts,
        },
        n: d.num_parts,
        t: d.part_duration,
        span: match d.duration_of_part {
            DurationOfPart::Auto => 0,
            DurationOfPart::LimitDuration { duration } => duration,
        },
        app_data: d.app_data,
    })
}

/// Convert a [`TwapStruct`] (per-part, on-chain view) back into [`TwapData`].
///
/// Multiplies `part_sell_amount` and `min_part_limit` by `n` to recover total
/// amounts. Maps `t0` and `span` back into [`TwapStartTime`] and
/// [`DurationOfPart`] enums. Sets `kind` to [`OrderKind::Sell`] and
/// `partially_fillable` to `false` (these fields are not encoded on-chain).
///
/// This is the inverse of [`data_to_struct`].
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, B256, U256};
/// use cow_rs::composable::{TwapStruct, struct_to_data};
///
/// let s = TwapStruct {
///     sell_token: Address::ZERO,
///     buy_token: Address::ZERO,
///     receiver: Address::ZERO,
///     part_sell_amount: U256::from(250u64),
///     min_part_limit: U256::from(200u64),
///     t0: 1_000_000,
///     n: 4,
///     t: 3600,
///     span: 0,
///     app_data: B256::ZERO,
/// };
/// let data = struct_to_data(&s);
/// assert_eq!(data.sell_amount, U256::from(1000u64));
/// assert_eq!(data.buy_amount, U256::from(800u64));
/// assert_eq!(data.num_parts, 4);
/// ```
#[must_use]
pub fn struct_to_data(s: &TwapStruct) -> TwapData {
    TwapData {
        sell_token: s.sell_token,
        buy_token: s.buy_token,
        receiver: s.receiver,
        sell_amount: s.part_sell_amount * U256::from(s.n),
        buy_amount: s.min_part_limit * U256::from(s.n),
        start_time: if s.t0 == 0 { TwapStartTime::AtMiningTime } else { TwapStartTime::At(s.t0) },
        part_duration: s.t,
        num_parts: s.n,
        app_data: s.app_data,
        partially_fillable: false,
        kind: OrderKind::Sell,
        duration_of_part: if s.span == 0 {
            DurationOfPart::Auto
        } else {
            DurationOfPart::LimitDuration { duration: s.span }
        },
    }
}

/// ABI-encode a [`TwapStruct`] into raw bytes for on-chain submission.
///
/// # Arguments
///
/// * `s` - The per-part `TWAP` struct to encode.
///
/// # Returns
///
/// A 320-byte `Vec<u8>` containing the 10-word ABI-encoded tuple.
fn encode_struct(s: &TwapStruct) -> Vec<u8> {
    encode_twap_struct(s)
}

/// ABI-encode a [`TwapStruct`] into the 320-byte `staticInput` bytes expected by the
/// on-chain `TWAP` handler.
///
/// Encoding layout (each word is 32 bytes, big-endian):
/// - Words 0-2: `sell_token`, `buy_token`, `receiver` (address, left-padded)
/// - Words 3-4: `part_sell_amount`, `min_part_limit` (uint256)
/// - Words 5-8: `t0`, `n`, `t`, `span` (uint32, right-aligned in 32 bytes)
/// - Word 9: `app_data` (bytes32)
///
/// Total: 10 x 32 = 320 bytes.
///
/// This is the symmetric counterpart to [`decode_twap_struct`].
///
/// ```
/// use alloy_primitives::{Address, U256};
/// use cow_rs::composable::{
///     TwapData, TwapStruct, data_to_struct, decode_twap_struct, encode_twap_struct,
/// };
///
/// let data = TwapData::sell(Address::ZERO, Address::ZERO, U256::from(1000u64), 4, 3600)
///     .with_buy_amount(U256::from(800u64));
/// let s = data_to_struct(&data).unwrap();
/// let bytes = encode_twap_struct(&s);
/// assert_eq!(bytes.len(), 320); // 10 × 32-byte ABI words
/// let decoded = decode_twap_struct(&bytes).unwrap();
/// assert_eq!(decoded.n, s.n);
/// ```
#[must_use]
pub fn encode_twap_struct(s: &TwapStruct) -> Vec<u8> {
    let mut buf = Vec::with_capacity(10 * 32);
    buf.extend_from_slice(&pad_address(s.sell_token.as_slice()));
    buf.extend_from_slice(&pad_address(s.buy_token.as_slice()));
    buf.extend_from_slice(&pad_address(s.receiver.as_slice()));
    buf.extend_from_slice(&u256_bytes(s.part_sell_amount));
    buf.extend_from_slice(&u256_bytes(s.min_part_limit));
    buf.extend_from_slice(&u256_be(u64::from(s.t0)));
    buf.extend_from_slice(&u256_be(u64::from(s.n)));
    buf.extend_from_slice(&u256_be(u64::from(s.t)));
    buf.extend_from_slice(&u256_be(u64::from(s.span)));
    buf.extend_from_slice(s.app_data.as_slice());
    buf
}

/// ABI-decode a 320-byte `staticInput` buffer into [`TwapData`].
///
/// Decodes the 10-word ABI tuple produced by [`encode_twap_struct`] and then
/// converts the per-part [`TwapStruct`] into the user-facing [`TwapData`]
/// representation via [`struct_to_data`].
///
/// # Errors
///
/// Returns [`CowError::AppData`] if `bytes` is shorter than 320 bytes.
pub fn decode_twap_static_input(bytes: &[u8]) -> Result<TwapData, CowError> {
    Ok(struct_to_data(&decode_twap_struct(bytes)?))
}

/// ABI-decode a 320-byte `staticInput` buffer into the raw [`TwapStruct`].
///
/// Reads 10 consecutive 32-byte ABI words in the order:
/// `sell_token`, `buy_token`, `receiver`, `part_sell_amount`, `min_part_limit`,
/// `t0`, `n`, `t`, `span`, `app_data`.
///
/// This is the symmetric counterpart to [`encode_twap_struct`].
///
/// # Errors
///
/// Returns [`CowError::AppData`] if `bytes` is shorter than 320 bytes.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::{Address, U256};
/// use cow_rs::composable::{TwapData, data_to_struct, decode_twap_struct, encode_twap_struct};
///
/// let data = TwapData::sell(Address::ZERO, Address::ZERO, U256::from(1000u64), 4, 3600)
///     .with_buy_amount(U256::from(800u64));
/// let s = data_to_struct(&data).unwrap();
/// let bytes = encode_twap_struct(&s);
/// let decoded = decode_twap_struct(&bytes).unwrap();
/// assert_eq!(decoded.part_sell_amount, s.part_sell_amount);
/// assert_eq!(decoded.min_part_limit, s.min_part_limit);
/// assert_eq!(decoded.n, s.n);
/// assert_eq!(decoded.t, s.t);
/// ```
pub fn decode_twap_struct(bytes: &[u8]) -> Result<TwapStruct, CowError> {
    if bytes.len() < 10 * 32 {
        return Err(CowError::AppData(format!(
            "TWAP static input too short: {} bytes (need 320)",
            bytes.len()
        )));
    }
    let addr = |off: usize| -> Address {
        let mut a = [0u8; 20];
        a.copy_from_slice(&bytes[off + 12..off + 32]);
        Address::new(a)
    };
    let u256 = |off: usize| -> U256 { U256::from_be_slice(&bytes[off..off + 32]) };
    let u32v = |off: usize| -> u32 {
        u32::from_be_bytes([bytes[off + 28], bytes[off + 29], bytes[off + 30], bytes[off + 31]])
    };

    let mut app_data = [0u8; 32];
    app_data.copy_from_slice(&bytes[288..320]);

    Ok(TwapStruct {
        sell_token: addr(0),
        buy_token: addr(32),
        receiver: addr(64),
        part_sell_amount: u256(96),
        min_part_limit: u256(128),
        t0: u32v(160),
        n: u32v(192),
        t: u32v(224),
        span: u32v(256),
        app_data: B256::new(app_data),
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Left-pad an address (or shorter slice) to 32 bytes.
///
/// # Arguments
///
/// * `bytes` - A 20-byte (or shorter) address slice to pad.
///
/// # Returns
///
/// A 32-byte array with the input right-aligned and zero-filled on the left.
fn pad_address(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(bytes);
    out
}

/// Convert a `U256` to its 32-byte big-endian representation.
///
/// # Arguments
///
/// * `v` - The 256-bit unsigned integer to convert.
///
/// # Returns
///
/// A 32-byte big-endian byte array.
const fn u256_bytes(v: U256) -> [u8; 32] {
    v.to_be_bytes()
}

/// Encode a `u64` as a 32-byte big-endian ABI word.
///
/// # Arguments
///
/// * `v` - The `u64` value to encode.
///
/// # Returns
///
/// A 32-byte array with the value right-aligned in big-endian order and
/// zero-filled on the left.
fn u256_be(v: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&v.to_be_bytes());
    out
}

/// Round `n` up to the next multiple of 32.
///
/// # Arguments
///
/// * `n` - The byte length to round up.
///
/// # Returns
///
/// The smallest multiple of 32 that is greater than or equal to `n`.
const fn pad32_len(n: usize) -> usize {
    if n.is_multiple_of(32) { n } else { n + (32 - n % 32) }
}

/// Append `data` to `buf` and zero-pad to a 32-byte boundary.
///
/// # Arguments
///
/// * `buf` - The output buffer to extend.
/// * `data` - The raw bytes to append.
///
/// # Returns
///
/// Nothing; `buf` is extended in place with `data` followed by zero bytes
/// so that the appended segment is a multiple of 32 bytes long.
fn pad_into(buf: &mut Vec<u8>, data: &[u8]) {
    buf.extend_from_slice(data);
    let rem = data.len() % 32;
    if rem != 0 {
        buf.resize(buf.len() + (32 - rem), 0);
    }
}

/// Derive a deterministic salt by hashing all TWAP parameters.
///
/// # Arguments
///
/// * `d` - The `TWAP` order data whose key fields are hashed.
///
/// # Returns
///
/// A [`B256`] salt computed as
/// `keccak256(sell_token ++ buy_token ++ sell_amount ++ num_parts)`.
fn deterministic_salt(d: &TwapData) -> B256 {
    let mut buf = Vec::with_capacity(20 + 20 + 32 + 4);
    buf.extend_from_slice(d.sell_token.as_slice());
    buf.extend_from_slice(d.buy_token.as_slice());
    buf.extend_from_slice(&u256_bytes(d.sell_amount));
    buf.extend_from_slice(&d.num_parts.to_be_bytes());
    keccak256(&buf)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sell_token() -> Address {
        Address::repeat_byte(0x11)
    }

    fn buy_token() -> Address {
        Address::repeat_byte(0x22)
    }

    fn sample_data() -> TwapData {
        TwapData::sell(sell_token(), buy_token(), U256::from(1000u64), 4, 3600)
            .with_buy_amount(U256::from(800u64))
    }

    // ── encode / decode roundtrip ────────────────────────────────────────

    #[test]
    fn encode_decode_twap_struct_roundtrip() {
        let data = sample_data();
        let s = data_to_struct(&data).unwrap();
        let bytes = encode_twap_struct(&s);
        assert_eq!(bytes.len(), 320);
        let decoded = decode_twap_struct(&bytes).unwrap();
        assert_eq!(decoded.sell_token, s.sell_token);
        assert_eq!(decoded.buy_token, s.buy_token);
        assert_eq!(decoded.receiver, s.receiver);
        assert_eq!(decoded.part_sell_amount, s.part_sell_amount);
        assert_eq!(decoded.min_part_limit, s.min_part_limit);
        assert_eq!(decoded.t0, s.t0);
        assert_eq!(decoded.n, s.n);
        assert_eq!(decoded.t, s.t);
        assert_eq!(decoded.span, s.span);
        assert_eq!(decoded.app_data, s.app_data);
    }

    #[test]
    fn data_to_struct_to_data_roundtrip() {
        let data = sample_data();
        let s = data_to_struct(&data).unwrap();
        let back = struct_to_data(&s);
        assert_eq!(back.sell_token, data.sell_token);
        assert_eq!(back.buy_token, data.buy_token);
        assert_eq!(back.sell_amount, data.sell_amount);
        assert_eq!(back.buy_amount, data.buy_amount);
        assert_eq!(back.num_parts, data.num_parts);
        assert_eq!(back.part_duration, data.part_duration);
    }

    #[test]
    fn decode_twap_static_input_roundtrip() {
        let data = sample_data();
        let s = data_to_struct(&data).unwrap();
        let bytes = encode_twap_struct(&s);
        let decoded_data = decode_twap_static_input(&bytes).unwrap();
        assert_eq!(decoded_data.sell_amount, data.sell_amount);
        assert_eq!(decoded_data.buy_amount, data.buy_amount);
    }

    // ── error cases ──────────────────────────────────────────────────────

    #[test]
    fn decode_twap_struct_too_short() {
        let result = decode_twap_struct(&[0u8; 319]);
        assert!(result.is_err());
    }

    #[test]
    fn data_to_struct_zero_num_parts() {
        let mut data = sample_data();
        data.num_parts = 0;
        assert!(data_to_struct(&data).is_err());
    }

    // ── encode_params / decode_params roundtrip ──────────────────────────

    #[test]
    fn encode_decode_params_roundtrip() {
        let params = ConditionalOrderParams {
            handler: Address::repeat_byte(0xaa),
            salt: B256::new([0xbb; 32]),
            static_input: vec![0xcc; 50],
        };
        let hex = encode_params(&params);
        let decoded = decode_params(&hex).unwrap();
        assert_eq!(decoded.handler, params.handler);
        assert_eq!(decoded.salt, params.salt);
        assert_eq!(decoded.static_input, params.static_input);
    }

    #[test]
    fn decode_params_invalid_hex() {
        assert!(decode_params("0xZZZZ").is_err());
    }

    #[test]
    fn decode_params_too_short() {
        assert!(decode_params("0xabcd").is_err());
    }

    // ── order_id ─────────────────────────────────────────────────────────

    #[test]
    fn order_id_deterministic() {
        let params = ConditionalOrderParams {
            handler: Address::ZERO,
            salt: B256::ZERO,
            static_input: vec![0xab, 0xcd],
        };
        let id1 = order_id(&params);
        let id2 = order_id(&params);
        assert_eq!(id1, id2);
        assert_ne!(id1, B256::ZERO);
    }

    #[test]
    fn order_id_changes_with_salt() {
        let p1 = ConditionalOrderParams {
            handler: Address::ZERO,
            salt: B256::ZERO,
            static_input: vec![],
        };
        let p2 = ConditionalOrderParams {
            handler: Address::ZERO,
            salt: B256::new([1u8; 32]),
            static_input: vec![],
        };
        assert_ne!(order_id(&p1), order_id(&p2));
    }

    // ── TwapOrder methods ────────────────────────────────────────────────

    #[test]
    fn twap_order_new_deterministic_salt() {
        let data = sample_data();
        let order1 = TwapOrder::new(data.clone());
        let order2 = TwapOrder::new(data);
        assert_eq!(order1.salt, order2.salt);
    }

    #[test]
    fn twap_order_with_salt() {
        let data = sample_data();
        let salt = B256::new([0xff; 32]);
        let order = TwapOrder::with_salt(data, salt);
        assert_eq!(order.salt, salt);
    }

    #[test]
    fn twap_order_to_params_and_id() {
        let order = TwapOrder::new(sample_data());
        let params = order.to_params().unwrap();
        assert_eq!(params.handler, TWAP_HANDLER_ADDRESS);
        let id = order.id().unwrap();
        assert_ne!(id, B256::ZERO);
    }

    #[test]
    fn twap_order_per_part_amounts() {
        let data = sample_data();
        let order = TwapOrder::new(data);
        let (sell, buy) = order.per_part_amounts().unwrap();
        assert_eq!(sell, U256::from(250u64));
        assert_eq!(buy, U256::from(200u64));
    }

    #[test]
    fn twap_order_per_part_amounts_zero_parts() {
        let mut data = sample_data();
        data.num_parts = 0;
        let order = TwapOrder::new(data);
        assert!(order.per_part_amounts().is_err());
    }

    #[test]
    fn twap_order_is_valid_happy_path() {
        let order = TwapOrder::new(sample_data());
        assert!(order.is_valid());
    }

    #[test]
    fn twap_order_is_valid_same_tokens() {
        let mut data = sample_data();
        data.buy_token = data.sell_token;
        let order = TwapOrder::new(data);
        assert!(!order.is_valid());
    }

    #[test]
    fn twap_order_is_valid_zero_sell_token() {
        let mut data = sample_data();
        data.sell_token = Address::ZERO;
        assert!(!TwapOrder::new(data).is_valid());
    }

    #[test]
    fn twap_order_is_valid_zero_sell_amount() {
        let mut data = sample_data();
        data.sell_amount = U256::ZERO;
        assert!(!TwapOrder::new(data).is_valid());
    }

    #[test]
    fn twap_order_is_valid_one_part() {
        let mut data = sample_data();
        data.num_parts = 1;
        assert!(!TwapOrder::new(data).is_valid());
    }

    #[test]
    fn twap_order_is_valid_zero_duration() {
        let mut data = sample_data();
        data.part_duration = 0;
        assert!(!TwapOrder::new(data).is_valid());
    }

    #[test]
    fn twap_order_is_valid_sell_amount_not_divisible() {
        let mut data = sample_data();
        data.sell_amount = U256::from(1001u64); // not divisible by 4
        assert!(!TwapOrder::new(data).is_valid());
    }

    #[test]
    fn twap_order_is_valid_limit_duration_exceeds_part_duration() {
        let mut data = sample_data();
        data.duration_of_part = DurationOfPart::LimitDuration { duration: 7200 }; // > 3600
        assert!(!TwapOrder::new(data).is_valid());
    }

    #[test]
    fn twap_order_is_valid_limit_duration_within_bounds() {
        let mut data = sample_data();
        data.duration_of_part = DurationOfPart::LimitDuration { duration: 1800 };
        assert!(TwapOrder::new(data).is_valid());
    }

    // ── poll_validate ────────────────────────────────────────────────────

    #[test]
    fn poll_validate_at_mining_time_always_success() {
        let order = TwapOrder::new(sample_data());
        match order.poll_validate(0) {
            PollResult::Success { .. } => {}
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[test]
    fn poll_validate_before_start() {
        let mut data = sample_data();
        data.start_time = TwapStartTime::At(1_000_000);
        let order = TwapOrder::new(data);
        match order.poll_validate(999_999) {
            PollResult::TryAtEpoch { epoch } => assert_eq!(epoch, 1_000_000),
            other => panic!("expected TryAtEpoch, got {other:?}"),
        }
    }

    #[test]
    fn poll_validate_within_window() {
        let mut data = sample_data();
        data.start_time = TwapStartTime::At(1_000_000);
        let order = TwapOrder::new(data);
        // end = 1_000_000 + 4 * 3600 = 1_014_400
        match order.poll_validate(1_007_000) {
            PollResult::Success { .. } => {}
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[test]
    fn poll_validate_after_expiry() {
        let mut data = sample_data();
        data.start_time = TwapStartTime::At(1_000_000);
        let order = TwapOrder::new(data);
        match order.poll_validate(1_014_400) {
            PollResult::DontTryAgain { .. } => {}
            other => panic!("expected DontTryAgain, got {other:?}"),
        }
    }

    // ── Accessor methods ─────────────────────────────────────────────────

    #[test]
    fn twap_order_accessors() {
        let mut data = sample_data();
        data.start_time = TwapStartTime::At(42);
        let order = TwapOrder::new(data);
        assert_eq!(order.total_sell_amount(), U256::from(1000u64));
        assert_eq!(order.total_buy_amount(), U256::from(800u64));
        assert!(order.is_sell());
        assert!(!order.is_buy());
        assert_eq!(order.start_timestamp(), Some(42));
        assert_eq!(order.salt_ref(), &order.salt);
        assert_eq!(order.data_ref().num_parts, 4);
    }

    #[test]
    fn twap_order_is_expired_at() {
        let mut data = sample_data();
        data.start_time = TwapStartTime::At(1_000_000);
        let order = TwapOrder::new(data);
        assert!(!order.is_expired_at(1_014_399));
        assert!(order.is_expired_at(1_014_400));
    }

    #[test]
    fn twap_order_at_mining_time_start_timestamp_none() {
        let order = TwapOrder::new(sample_data());
        assert_eq!(order.start_timestamp(), None);
    }

    // ── format_epoch ─────────────────────────────────────────────────────

    #[test]
    fn format_epoch_known_timestamp() {
        let s = format_epoch(1_700_000_000);
        assert!(s.starts_with("2023-11-14"));
    }

    // ── to_struct ────────────────────────────────────────────────────────

    #[test]
    fn to_struct_with_limit_duration() {
        let mut data = sample_data();
        data.duration_of_part = DurationOfPart::LimitDuration { duration: 1800 };
        let order = TwapOrder::new(data);
        let s = order.to_struct().unwrap();
        assert_eq!(s.span, 1800);
    }

    #[test]
    fn to_struct_auto_duration() {
        let order = TwapOrder::new(sample_data());
        let s = order.to_struct().unwrap();
        assert_eq!(s.span, 0);
    }

    // ── struct_to_data edge cases ────────────────────────────────────────

    #[test]
    fn struct_to_data_at_mining_time() {
        let s = data_to_struct(&sample_data()).unwrap();
        let data = struct_to_data(&s);
        assert!(matches!(data.start_time, TwapStartTime::AtMiningTime));
        assert!(matches!(data.duration_of_part, DurationOfPart::Auto));
    }

    #[test]
    fn struct_to_data_with_fixed_start() {
        let mut d = sample_data();
        d.start_time = TwapStartTime::At(12345);
        d.duration_of_part = DurationOfPart::LimitDuration { duration: 600 };
        let s = data_to_struct(&d).unwrap();
        let back = struct_to_data(&s);
        assert!(matches!(back.start_time, TwapStartTime::At(12345)));
        assert!(matches!(back.duration_of_part, DurationOfPart::LimitDuration { duration: 600 }));
    }

    // ── Display ──────────────────────────────────────────────────────────

    #[test]
    fn twap_order_display_does_not_panic() {
        let order = TwapOrder::new(sample_data());
        let s = format!("{order}");
        assert!(!s.is_empty());
    }
}
