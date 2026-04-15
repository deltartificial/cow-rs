//! `StopLoss` conditional order handler.
//!
//! A stop-loss order triggers when the price of `sell_token` in terms of
//! `buy_token` falls to or below `strike_price`.  The handler verifies the
//! price against on-chain Chainlink-compatible oracles.
//!
//! Handler address (mainnet): `0xe8212F30C28B4AAB467DF3725C14d6e89C2eB972`

use alloy_primitives::{Address, B256, U256, keccak256};

use cow_errors::CowError;

use super::types::ConditionalOrderParams;

// ── Handler address ───────────────────────────────────────────────────────────

/// `StopLoss` handler contract address (Ethereum mainnet).
///
/// `0xe8212F30C28B4AAB467DF3725C14d6e89C2eB972`
pub const STOP_LOSS_HANDLER_ADDRESS: Address = Address::new([
    0xe8, 0x21, 0x2f, 0x30, 0xc2, 0x8b, 0x4a, 0xab, 0x46, 0x7d, 0xf3, 0x72, 0x5c, 0x14, 0xd6, 0xe8,
    0x9c, 0x2e, 0xb9, 0x72,
]);

// ── StopLossData ─────────────────────────────────────────────────────────────

/// Parameters for a stop-loss conditional order.
///
/// A stop-loss order is triggered when the spot price of `sell_token` in
/// units of `buy_token` falls to or below `strike_price` (18-decimal
/// fixed-point).  Both token prices are read from Chainlink-compatible oracle
/// contracts.
#[derive(Debug, Clone)]
pub struct StopLossData {
    /// Token to sell when the condition triggers.
    pub sell_token: Address,
    /// Token to buy when the condition triggers.
    pub buy_token: Address,
    /// Amount of `sell_token` to sell (in atoms).
    pub sell_amount: U256,
    /// Minimum amount of `buy_token` to receive (in atoms).
    pub buy_amount: U256,
    /// App-data hash (`bytes32`).
    pub app_data: B256,
    /// Receiver of bought tokens (`Address::ZERO` = order owner).
    pub receiver: Address,
    /// Whether this is a sell-direction (`true`) or buy-direction (`false`) order.
    pub is_sell_order: bool,
    /// Whether the order may be partially filled.
    pub is_partially_fillable: bool,
    /// Order expiry as a Unix timestamp.
    pub valid_to: u32,
    /// Strike price as an 18-decimal fixed-point `uint256`.
    ///
    /// The order triggers when the oracle-reported price falls to or below this
    /// value.
    pub strike_price: U256,
    /// Chainlink-compatible price oracle for `sell_token`.
    pub sell_token_price_oracle: Address,
    /// Chainlink-compatible price oracle for `buy_token`.
    pub buy_token_price_oracle: Address,
    /// When `true`, the oracle price is expressed in ETH units rather than
    /// token-atom units.
    pub token_amount_in_eth: bool,
}

// ── StopLossOrder ─────────────────────────────────────────────────────────────

/// A stop-loss conditional order ready to be submitted to `ComposableCow`.
#[derive(Debug, Clone)]
pub struct StopLossOrder {
    /// Stop-loss configuration.
    pub data: StopLossData,
    /// 32-byte salt uniquely identifying this order instance.
    pub salt: B256,
}

impl StopLossOrder {
    /// Create a new stop-loss order with a deterministic salt derived from the
    /// order parameters.
    ///
    /// # Returns
    ///
    /// A [`StopLossOrder`] whose salt is the `keccak256` hash of
    /// `(sell_token, buy_token, sell_amount, strike_price)`.
    #[must_use]
    pub fn new(data: StopLossData) -> Self {
        let salt = deterministic_salt(&data);
        Self { data, salt }
    }

    /// Create a stop-loss order with an explicit salt.
    ///
    /// # Arguments
    ///
    /// * `data` - The stop-loss order configuration.
    /// * `salt` - A caller-chosen 32-byte salt to uniquely identify this order.
    ///
    /// # Returns
    ///
    /// A [`StopLossOrder`] using the provided salt verbatim.
    #[must_use]
    pub const fn with_salt(data: StopLossData, salt: B256) -> Self {
        Self { data, salt }
    }

    /// Returns `true` if the order parameters are logically valid:
    ///
    /// - `sell_amount > 0`
    /// - `buy_amount > 0`
    /// - `sell_token != buy_token`
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let d = &self.data;
        !d.sell_amount.is_zero() && !d.buy_amount.is_zero() && d.sell_token != d.buy_token
    }

    /// Build the on-chain [`ConditionalOrderParams`] for this order.
    ///
    /// # Returns
    ///
    /// A [`ConditionalOrderParams`] containing the `StopLoss` handler address,
    /// the order salt, and the ABI-encoded static input.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] if ABI encoding fails.
    pub fn to_params(&self) -> Result<ConditionalOrderParams, CowError> {
        Ok(ConditionalOrderParams {
            handler: STOP_LOSS_HANDLER_ADDRESS,
            salt: self.salt,
            static_input: encode_stop_loss_struct(&self.data),
        })
    }

    /// Returns a reference to the stop-loss data.
    ///
    /// # Returns
    ///
    /// A shared reference to the underlying [`StopLossData`] configuration.
    #[must_use]
    pub const fn data_ref(&self) -> &StopLossData {
        &self.data
    }

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
}

// ── ABI encoding ──────────────────────────────────────────────────────────────

/// ABI-encode a [`StopLossData`] struct into the 416-byte `staticInput` bytes
/// expected by the on-chain `StopLoss` handler.
///
/// The encoding follows the Solidity ABI packed-tuple format:
/// 13 fields × 32 bytes = 416 bytes.
///
/// Field order:
/// 1. `sellToken` (address, left-padded)
/// 2. `buyToken` (address, left-padded)
/// 3. `sellAmount` (uint256)
/// 4. `buyAmount` (uint256)
/// 5. `appData` (bytes32)
/// 6. `receiver` (address, left-padded)
/// 7. `isSellOrder` (bool)
/// 8. `isPartiallyFillable` (bool)
/// 9. `validTo` (uint32)
/// 10. `strikePrice` (uint256)
/// 11. `sellTokenPriceOracle` (address, left-padded)
/// 12. `buyTokenPriceOracle` (address, left-padded)
/// 13. `tokenAmountInEth` (bool)
///
/// ```
/// use alloy_primitives::{Address, B256, U256};
/// use cow_composable::{StopLossData, encode_stop_loss_struct};
///
/// let data = StopLossData {
///     sell_token: Address::ZERO,
///     buy_token: Address::ZERO,
///     sell_amount: U256::from(1_000u64),
///     buy_amount: U256::from(900u64),
///     app_data: B256::ZERO,
///     receiver: Address::ZERO,
///     is_sell_order: true,
///     is_partially_fillable: false,
///     valid_to: 9_999_999,
///     strike_price: U256::from(1_000_000_000_000_000_000u64),
///     sell_token_price_oracle: Address::ZERO,
///     buy_token_price_oracle: Address::ZERO,
///     token_amount_in_eth: false,
/// };
/// let encoded = encode_stop_loss_struct(&data);
/// assert_eq!(encoded.len(), 416);
/// ```
#[must_use]
pub fn encode_stop_loss_struct(d: &StopLossData) -> Vec<u8> {
    let mut buf = Vec::with_capacity(13 * 32);
    buf.extend_from_slice(&pad_address(d.sell_token.as_slice()));
    buf.extend_from_slice(&pad_address(d.buy_token.as_slice()));
    buf.extend_from_slice(&u256_bytes(d.sell_amount));
    buf.extend_from_slice(&u256_bytes(d.buy_amount));
    buf.extend_from_slice(d.app_data.as_slice());
    buf.extend_from_slice(&pad_address(d.receiver.as_slice()));
    buf.extend_from_slice(&bool_word(d.is_sell_order));
    buf.extend_from_slice(&bool_word(d.is_partially_fillable));
    buf.extend_from_slice(&u256_be(u64::from(d.valid_to)));
    buf.extend_from_slice(&u256_bytes(d.strike_price));
    buf.extend_from_slice(&pad_address(d.sell_token_price_oracle.as_slice()));
    buf.extend_from_slice(&pad_address(d.buy_token_price_oracle.as_slice()));
    buf.extend_from_slice(&bool_word(d.token_amount_in_eth));
    buf
}

/// ABI-decode a 416-byte `staticInput` buffer into a [`StopLossData`].
///
/// # Errors
///
/// Returns [`CowError::AppData`] if `bytes` is shorter than 416 bytes or if
/// a field cannot be decoded.
///
/// ```
/// use alloy_primitives::{Address, B256, U256};
/// use cow_composable::{StopLossData, decode_stop_loss_static_input, encode_stop_loss_struct};
///
/// let data = StopLossData {
///     sell_token: Address::ZERO,
///     buy_token: Address::ZERO,
///     sell_amount: U256::from(500u64),
///     buy_amount: U256::from(400u64),
///     app_data: B256::ZERO,
///     receiver: Address::ZERO,
///     is_sell_order: true,
///     is_partially_fillable: false,
///     valid_to: 1_234_567,
///     strike_price: U256::from(1u64),
///     sell_token_price_oracle: Address::ZERO,
///     buy_token_price_oracle: Address::ZERO,
///     token_amount_in_eth: false,
/// };
/// let encoded = encode_stop_loss_struct(&data);
/// let decoded = decode_stop_loss_static_input(&encoded).unwrap();
/// assert_eq!(decoded.sell_amount, data.sell_amount);
/// assert_eq!(decoded.valid_to, data.valid_to);
/// assert_eq!(decoded.is_sell_order, data.is_sell_order);
/// ```
pub fn decode_stop_loss_static_input(bytes: &[u8]) -> Result<StopLossData, CowError> {
    if bytes.len() < 13 * 32 {
        return Err(CowError::AppData(format!(
            "StopLoss static input too short: {} bytes (need 416)",
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
    let bool_v = |off: usize| -> bool { bytes[off + 31] != 0 };

    let mut app_data_bytes = [0u8; 32];
    app_data_bytes.copy_from_slice(&bytes[4 * 32..5 * 32]);

    Ok(StopLossData {
        sell_token: addr(0),
        buy_token: addr(32),
        sell_amount: u256(64),
        buy_amount: u256(96),
        app_data: B256::new(app_data_bytes),
        receiver: addr(5 * 32),
        is_sell_order: bool_v(6 * 32),
        is_partially_fillable: bool_v(7 * 32),
        valid_to: u32v(8 * 32),
        strike_price: u256(9 * 32),
        sell_token_price_oracle: addr(10 * 32),
        buy_token_price_oracle: addr(11 * 32),
        token_amount_in_eth: bool_v(12 * 32),
    })
}

// ── Private helpers ───────────────────────────────────────────────────────────

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

/// Encode a `bool` as a 32-byte ABI word (0 or 1).
///
/// # Arguments
///
/// * `v` - The boolean value to encode.
///
/// # Returns
///
/// A 32-byte array where the last byte is `1` if `v` is `true`, `0` otherwise.
const fn bool_word(v: bool) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[31] = if v { 1 } else { 0 };
    out
}

/// Derive a deterministic salt by hashing all stop-loss parameters.
///
/// # Arguments
///
/// * `d` - The stop-loss order data whose key fields are hashed.
///
/// # Returns
///
/// A [`B256`] salt computed as `keccak256(sell_token ++ buy_token ++ sell_amount ++ strike_price)`.
fn deterministic_salt(d: &StopLossData) -> B256 {
    let mut buf = Vec::with_capacity(20 + 20 + 32 + 32);
    buf.extend_from_slice(d.sell_token.as_slice());
    buf.extend_from_slice(d.buy_token.as_slice());
    buf.extend_from_slice(&u256_bytes(d.sell_amount));
    buf.extend_from_slice(&u256_bytes(d.strike_price));
    keccak256(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_data() -> StopLossData {
        StopLossData {
            sell_token: Address::repeat_byte(0x01),
            buy_token: Address::repeat_byte(0x02),
            sell_amount: U256::from(1_000u64),
            buy_amount: U256::from(900u64),
            app_data: B256::ZERO,
            receiver: Address::ZERO,
            is_sell_order: true,
            is_partially_fillable: false,
            valid_to: 9_999_999,
            strike_price: U256::from(1_000_000_000_000_000_000u64),
            sell_token_price_oracle: Address::repeat_byte(0x03),
            buy_token_price_oracle: Address::repeat_byte(0x04),
            token_amount_in_eth: false,
        }
    }

    #[test]
    fn encode_is_416_bytes() {
        let data = make_data();
        let encoded = encode_stop_loss_struct(&data);
        assert_eq!(encoded.len(), 416);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let data = make_data();
        let encoded = encode_stop_loss_struct(&data);
        let decoded = decode_stop_loss_static_input(&encoded).unwrap();
        assert_eq!(decoded.sell_token, data.sell_token);
        assert_eq!(decoded.buy_token, data.buy_token);
        assert_eq!(decoded.sell_amount, data.sell_amount);
        assert_eq!(decoded.buy_amount, data.buy_amount);
        assert_eq!(decoded.app_data, data.app_data);
        assert_eq!(decoded.receiver, data.receiver);
        assert_eq!(decoded.is_sell_order, data.is_sell_order);
        assert_eq!(decoded.is_partially_fillable, data.is_partially_fillable);
        assert_eq!(decoded.valid_to, data.valid_to);
        assert_eq!(decoded.strike_price, data.strike_price);
        assert_eq!(decoded.sell_token_price_oracle, data.sell_token_price_oracle);
        assert_eq!(decoded.buy_token_price_oracle, data.buy_token_price_oracle);
        assert_eq!(decoded.token_amount_in_eth, data.token_amount_in_eth);
    }

    #[test]
    fn is_valid_returns_false_when_same_token() {
        let mut data = make_data();
        data.buy_token = data.sell_token;
        let order = StopLossOrder::new(data);
        assert!(!order.is_valid());
    }

    #[test]
    fn is_valid_returns_false_when_zero_amounts() {
        let mut data = make_data();
        data.sell_amount = U256::ZERO;
        let order = StopLossOrder::new(data);
        assert!(!order.is_valid());
    }

    #[test]
    fn is_valid_returns_true_for_valid_order() {
        let order = StopLossOrder::new(make_data());
        assert!(order.is_valid());
    }

    #[test]
    fn to_params_sets_correct_handler() {
        let order = StopLossOrder::new(make_data());
        let params = order.to_params().unwrap();
        assert_eq!(params.handler, STOP_LOSS_HANDLER_ADDRESS);
    }

    #[test]
    fn decode_too_short_returns_error() {
        let result = decode_stop_loss_static_input(&[0u8; 100]);
        assert!(result.is_err());
    }
}
