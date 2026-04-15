//! Bitfield encoding and decoding of order and trade flags.
//!
//! Mirrors `encodeOrderFlags`, `decodeOrderFlags`, `encodeTradeFlags`,
//! `decodeTradeFlags`, `encodeSigningScheme`, and `decodeSigningScheme`
//! from the `TypeScript` `contracts-ts` package.

use cow_sdk_error::CowError;
use cow_sdk_types::{OrderKind, SigningScheme, TokenBalance};

/// Order flags extracted from a bitfield.
///
/// Corresponds to the `TypeScript` `OrderFlags` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderFlags {
    /// The order kind (sell or buy).
    pub kind: OrderKind,
    /// Whether the order is partially fillable.
    pub partially_fillable: bool,
    /// Source of sell token balance.
    pub sell_token_balance: TokenBalance,
    /// Destination of buy token balance.
    pub buy_token_balance: TokenBalance,
}

/// Trade flags: order flags plus a signing scheme.
///
/// Corresponds to the `TypeScript` `TradeFlags` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeFlags {
    /// The underlying order flags.
    pub order_flags: OrderFlags,
    /// The signing scheme used to encode the signature.
    pub signing_scheme: SigningScheme,
}

// ── Bit layout ──────────────────────────────────────────────────────────────
//
// Bit 0:     kind              (0 = sell, 1 = buy)
// Bit 1:     partiallyFillable (0 = false, 1 = true)
// Bits 2-3:  sellTokenBalance  (0 = erc20, 1 = unused, 2 = external, 3 = internal)
// Bit 4:     buyTokenBalance   (0 = erc20, 1 = internal)
// Bits 5-6:  signingScheme     (0 = eip712, 1 = ethsign, 2 = eip1271, 3 = presign)

const KIND_OFFSET: u8 = 0;
const PARTIALLY_FILLABLE_OFFSET: u8 = 1;
const SELL_TOKEN_BALANCE_OFFSET: u8 = 2;
const BUY_TOKEN_BALANCE_OFFSET: u8 = 4;
const SIGNING_SCHEME_OFFSET: u8 = 5;

/// Encode an [`OrderKind`] into its flag bits.
///
/// # Arguments
///
/// * `kind` — the order kind to encode.
///
/// # Returns
///
/// A `u8` with bit 0 set to `0` for [`OrderKind::Sell`] or `1` for
/// [`OrderKind::Buy`].
#[must_use]
pub const fn encode_kind(kind: OrderKind) -> u8 {
    match kind {
        OrderKind::Sell => 0,
        OrderKind::Buy => 1,
    }
}

/// Encode a `partially_fillable` bool into its flag bits.
///
/// # Arguments
///
/// * `pf` — `true` if the order is partially fillable.
///
/// # Returns
///
/// A `u8` with bit 1 set according to `pf`.
#[must_use]
pub const fn encode_partially_fillable(pf: bool) -> u8 {
    (pf as u8) << PARTIALLY_FILLABLE_OFFSET
}

/// Encode a sell [`TokenBalance`] into its flag bits.
///
/// # Arguments
///
/// * `balance` — the sell token balance source.
///
/// # Returns
///
/// A `u8` with bits 2-3 encoding the balance type (`Erc20` = 0, `External` = 2,
/// `Internal` = 3).
#[must_use]
pub const fn encode_sell_token_balance(balance: TokenBalance) -> u8 {
    let index = match balance {
        TokenBalance::Erc20 => 0u8,
        TokenBalance::External => 2,
        TokenBalance::Internal => 3,
    };
    index << SELL_TOKEN_BALANCE_OFFSET
}

/// Encode a buy [`TokenBalance`] into its flag bits.
///
/// Note: `External` is normalized to `Erc20` for buy token balance, matching
/// the `TypeScript` `normalizeBuyTokenBalance` behavior.
///
/// # Arguments
///
/// * `balance` — the buy token balance destination.
///
/// # Returns
///
/// A `u8` with bit 4 encoding the balance type (`Erc20`/`External` = 0,
/// `Internal` = 1).
#[must_use]
pub const fn encode_buy_token_balance(balance: TokenBalance) -> u8 {
    let index = match balance {
        TokenBalance::Erc20 | TokenBalance::External => 0u8,
        TokenBalance::Internal => 1,
    };
    index << BUY_TOKEN_BALANCE_OFFSET
}

/// Encode a [`SigningScheme`] into its flag bits.
///
/// Mirrors `encodeSigningScheme` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `scheme` — the signing scheme to encode.
///
/// # Returns
///
/// A `u8` with bits 5-6 encoding the scheme (`Eip712` = 0, `EthSign` = 1,
/// `Eip1271` = 2, `PreSign` = 3).
///
/// ```
/// use cow_sdk_signing::order_signing::flags::encode_signing_scheme;
/// use cow_sdk_types::SigningScheme;
///
/// assert_eq!(encode_signing_scheme(SigningScheme::Eip712), 0b00_00000);
/// assert_eq!(encode_signing_scheme(SigningScheme::EthSign), 0b01_00000);
/// assert_eq!(encode_signing_scheme(SigningScheme::Eip1271), 0b10_00000);
/// assert_eq!(encode_signing_scheme(SigningScheme::PreSign), 0b11_00000);
/// ```
#[must_use]
pub const fn encode_signing_scheme(scheme: SigningScheme) -> u8 {
    let index = match scheme {
        SigningScheme::Eip712 => 0u8,
        SigningScheme::EthSign => 1,
        SigningScheme::Eip1271 => 2,
        SigningScheme::PreSign => 3,
    };
    index << SIGNING_SCHEME_OFFSET
}

/// Encode order flags as a single byte bitfield.
///
/// Mirrors `encodeOrderFlags` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `flags` — the order flags to encode.
///
/// # Returns
///
/// A `u8` bitfield combining the kind, partially-fillable, sell-token-balance,
/// and buy-token-balance flags.
///
/// ```
/// use cow_rs::{
///     OrderKind, TokenBalance,
///     order_signing::flags::{OrderFlags, encode_order_flags},
/// };
///
/// let flags = OrderFlags {
///     kind: OrderKind::Sell,
///     partially_fillable: false,
///     sell_token_balance: TokenBalance::Erc20,
///     buy_token_balance: TokenBalance::Erc20,
/// };
/// assert_eq!(encode_order_flags(&flags), 0);
///
/// let flags = OrderFlags {
///     kind: OrderKind::Buy,
///     partially_fillable: true,
///     sell_token_balance: TokenBalance::External,
///     buy_token_balance: TokenBalance::Internal,
/// };
/// assert_eq!(encode_order_flags(&flags), 0b1_1011);
/// ```
#[must_use]
pub const fn encode_order_flags(flags: &OrderFlags) -> u8 {
    encode_kind(flags.kind) |
        encode_partially_fillable(flags.partially_fillable) |
        encode_sell_token_balance(flags.sell_token_balance) |
        encode_buy_token_balance(flags.buy_token_balance)
}

/// Decode order flags from a bitfield.
///
/// Mirrors `decodeOrderFlags` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `bits` — the encoded bitfield byte.
///
/// # Returns
///
/// The decoded [`OrderFlags`] struct.
///
/// # Errors
///
/// Returns [`CowError::Parse`] if any flag field has an invalid index.
///
/// ```
/// use cow_rs::{
///     OrderKind, TokenBalance,
///     order_signing::flags::{OrderFlags, decode_order_flags, encode_order_flags},
/// };
///
/// let flags = OrderFlags {
///     kind: OrderKind::Buy,
///     partially_fillable: true,
///     sell_token_balance: TokenBalance::Internal,
///     buy_token_balance: TokenBalance::Internal,
/// };
/// let encoded = encode_order_flags(&flags);
/// let decoded = decode_order_flags(encoded).unwrap();
/// assert_eq!(decoded, flags);
/// ```
pub fn decode_order_flags(bits: u8) -> Result<OrderFlags, CowError> {
    let kind_index = (bits >> KIND_OFFSET) & 0x01;
    let pf_index = (bits >> PARTIALLY_FILLABLE_OFFSET) & 0x01;
    let sell_index = (bits >> SELL_TOKEN_BALANCE_OFFSET) & 0x03;
    let buy_index = (bits >> BUY_TOKEN_BALANCE_OFFSET) & 0x01;

    let kind = match kind_index {
        0 => OrderKind::Sell,
        1 => OrderKind::Buy,
        _ => unreachable!(),
    };

    let partially_fillable = pf_index != 0;

    let sell_token_balance = match sell_index {
        0 => TokenBalance::Erc20,
        2 => TokenBalance::External,
        3 => TokenBalance::Internal,
        other => {
            return Err(CowError::Parse {
                field: "sellTokenBalance",
                reason: format!("invalid flag index: {other}"),
            });
        }
    };

    let buy_token_balance = match buy_index {
        0 => TokenBalance::Erc20,
        1 => TokenBalance::Internal,
        _ => unreachable!(),
    };

    Ok(OrderFlags { kind, partially_fillable, sell_token_balance, buy_token_balance })
}

/// Decode a [`SigningScheme`] from a trade-flags bitfield.
///
/// Mirrors `decodeSigningScheme` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `bits` — the encoded trade-flags bitfield byte.
///
/// # Returns
///
/// The decoded [`SigningScheme`] variant.
///
/// # Errors
///
/// Returns [`CowError::Parse`] if the signing scheme index is invalid.
///
/// ```
/// use cow_rs::{
///     SigningScheme,
///     order_signing::flags::{decode_signing_scheme, encode_signing_scheme},
/// };
///
/// let bits = encode_signing_scheme(SigningScheme::Eip1271);
/// assert_eq!(decode_signing_scheme(bits).unwrap(), SigningScheme::Eip1271);
/// ```
pub fn decode_signing_scheme(bits: u8) -> Result<SigningScheme, CowError> {
    let index = (bits >> SIGNING_SCHEME_OFFSET) & 0x03;
    match index {
        0 => Ok(SigningScheme::Eip712),
        1 => Ok(SigningScheme::EthSign),
        2 => Ok(SigningScheme::Eip1271),
        3 => Ok(SigningScheme::PreSign),
        _ => unreachable!(),
    }
}

/// Encode trade flags (order flags + signing scheme) as a single byte bitfield.
///
/// Mirrors `encodeTradeFlags` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `flags` — the trade flags to encode.
///
/// # Returns
///
/// A `u8` bitfield combining order flags (bits 0-4) and signing scheme
/// (bits 5-6).
///
/// ```
/// use cow_rs::{
///     OrderKind, SigningScheme, TokenBalance,
///     order_signing::flags::{OrderFlags, TradeFlags, encode_trade_flags},
/// };
///
/// let flags = TradeFlags {
///     order_flags: OrderFlags {
///         kind: OrderKind::Sell,
///         partially_fillable: false,
///         sell_token_balance: TokenBalance::Erc20,
///         buy_token_balance: TokenBalance::Erc20,
///     },
///     signing_scheme: SigningScheme::Eip712,
/// };
/// assert_eq!(encode_trade_flags(&flags), 0);
/// ```
#[must_use]
pub const fn encode_trade_flags(flags: &TradeFlags) -> u8 {
    encode_order_flags(&flags.order_flags) | encode_signing_scheme(flags.signing_scheme)
}

/// Decode trade flags from a bitfield.
///
/// Mirrors `decodeTradeFlags` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `bits` — the encoded trade-flags bitfield byte.
///
/// # Returns
///
/// The decoded [`TradeFlags`] struct containing order flags and signing scheme.
///
/// # Errors
///
/// Returns [`CowError::Parse`] if any flag field has an invalid index.
///
/// ```
/// use cow_rs::{
///     OrderKind, SigningScheme, TokenBalance,
///     order_signing::flags::{OrderFlags, TradeFlags, decode_trade_flags, encode_trade_flags},
/// };
///
/// let flags = TradeFlags {
///     order_flags: OrderFlags {
///         kind: OrderKind::Buy,
///         partially_fillable: true,
///         sell_token_balance: TokenBalance::External,
///         buy_token_balance: TokenBalance::Internal,
///     },
///     signing_scheme: SigningScheme::PreSign,
/// };
/// let encoded = encode_trade_flags(&flags);
/// let decoded = decode_trade_flags(encoded).unwrap();
/// assert_eq!(decoded, flags);
/// ```
pub fn decode_trade_flags(bits: u8) -> Result<TradeFlags, CowError> {
    let order_flags = decode_order_flags(bits)?;
    let signing_scheme = decode_signing_scheme(bits)?;
    Ok(TradeFlags { order_flags, signing_scheme })
}

/// Normalize the buy token balance, converting `External` to `Erc20`.
///
/// In the `CoW` Protocol, the `External` balance type only applies to sell
/// tokens. For buy tokens, `External` is treated as `Erc20`.
///
/// Mirrors `normalizeBuyTokenBalance` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `balance` — the buy token balance to normalize.
///
/// # Returns
///
/// The normalized [`TokenBalance`], with [`TokenBalance::External`] mapped to
/// [`TokenBalance::Erc20`].
///
/// ```
/// use cow_sdk_signing::order_signing::flags::normalize_buy_token_balance;
/// use cow_sdk_types::TokenBalance;
///
/// assert_eq!(normalize_buy_token_balance(TokenBalance::Erc20), TokenBalance::Erc20);
/// assert_eq!(normalize_buy_token_balance(TokenBalance::External), TokenBalance::Erc20);
/// assert_eq!(normalize_buy_token_balance(TokenBalance::Internal), TokenBalance::Internal);
/// ```
#[must_use]
pub const fn normalize_buy_token_balance(balance: TokenBalance) -> TokenBalance {
    match balance {
        TokenBalance::Erc20 | TokenBalance::External => TokenBalance::Erc20,
        TokenBalance::Internal => TokenBalance::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_order_flags() {
        let cases = [
            OrderFlags {
                kind: OrderKind::Sell,
                partially_fillable: false,
                sell_token_balance: TokenBalance::Erc20,
                buy_token_balance: TokenBalance::Erc20,
            },
            OrderFlags {
                kind: OrderKind::Buy,
                partially_fillable: true,
                sell_token_balance: TokenBalance::External,
                buy_token_balance: TokenBalance::Internal,
            },
            OrderFlags {
                kind: OrderKind::Sell,
                partially_fillable: true,
                sell_token_balance: TokenBalance::Internal,
                buy_token_balance: TokenBalance::Erc20,
            },
        ];

        for flags in &cases {
            let encoded = encode_order_flags(flags);
            let decoded = decode_order_flags(encoded).unwrap();
            assert_eq!(&decoded, flags, "roundtrip failed for {flags:?}");
        }
    }

    #[test]
    fn roundtrip_trade_flags() {
        let flags = TradeFlags {
            order_flags: OrderFlags {
                kind: OrderKind::Buy,
                partially_fillable: false,
                sell_token_balance: TokenBalance::Erc20,
                buy_token_balance: TokenBalance::Internal,
            },
            signing_scheme: SigningScheme::Eip1271,
        };
        let encoded = encode_trade_flags(&flags);
        let decoded = decode_trade_flags(encoded).unwrap();
        assert_eq!(decoded, flags);
    }

    #[test]
    fn invalid_sell_token_balance_flag() {
        // Bit pattern 01 at offset 2 is unused
        let bits = 0b000_0100;
        let result = decode_order_flags(bits);
        assert!(result.is_err());
    }

    #[test]
    fn signing_scheme_round_trip() {
        for scheme in [
            SigningScheme::Eip712,
            SigningScheme::EthSign,
            SigningScheme::Eip1271,
            SigningScheme::PreSign,
        ] {
            let encoded = encode_signing_scheme(scheme);
            let decoded = decode_signing_scheme(encoded).unwrap();
            assert_eq!(decoded, scheme);
        }
    }
}
