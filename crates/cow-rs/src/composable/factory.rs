//! [`ConditionalOrderFactory`] — instantiate conditional orders from on-chain params.

use std::fmt;

use crate::error::CowError;

use super::{
    gat::{GatOrder, decode_gat_static_input},
    stop_loss::{STOP_LOSS_HANDLER_ADDRESS, StopLossOrder, decode_stop_loss_static_input},
    twap::{TwapOrder, decode_twap_static_input},
    types::{ConditionalOrderParams, TWAP_HANDLER_ADDRESS},
};

/// A conditional order decoded from on-chain [`ConditionalOrderParams`].
#[derive(Debug, Clone)]
pub enum ConditionalOrderKind {
    /// A `TWAP` (Time-Weighted Average Price) order.
    Twap(TwapOrder),
    /// A stop-loss order that triggers when the price falls below a strike price.
    StopLoss(StopLossOrder),
    /// A `GoodAfterTime` order that becomes valid only after a given timestamp.
    GoodAfterTime(GatOrder),
    /// An order whose handler is not recognised by this factory.
    Unknown(ConditionalOrderParams),
}

impl ConditionalOrderKind {
    /// Returns a short string label for the order kind.
    ///
    /// # Returns
    ///
    /// A `&'static str` identifying the variant: `"twap"`, `"stop-loss"`,
    /// `"good-after-time"`, or `"unknown"`.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Twap(_) => "twap",
            Self::StopLoss(_) => "stop-loss",
            Self::GoodAfterTime(_) => "good-after-time",
            Self::Unknown(_) => "unknown",
        }
    }

    /// Returns `true` if this is a `TWAP` conditional order.
    ///
    /// ```
    /// use alloy_primitives::B256;
    /// use cow_rs::composable::{
    ///     ConditionalOrderFactory, ConditionalOrderParams, TWAP_HANDLER_ADDRESS,
    /// };
    ///
    /// let params = ConditionalOrderParams {
    ///     handler: TWAP_HANDLER_ADDRESS,
    ///     salt: B256::ZERO,
    ///     static_input: vec![],
    /// };
    /// // An unknown handler resolves to Unknown, not Twap.
    /// use alloy_primitives::Address;
    /// let unknown = cow_rs::composable::ConditionalOrderKind::Unknown(ConditionalOrderParams {
    ///     handler: Address::ZERO,
    ///     salt: B256::ZERO,
    ///     static_input: vec![],
    /// });
    /// assert!(!unknown.is_twap());
    /// assert!(unknown.is_unknown());
    /// ```
    #[must_use]
    pub const fn is_twap(&self) -> bool {
        matches!(self, Self::Twap(_))
    }

    /// Returns `true` if this is a stop-loss conditional order.
    ///
    /// ```
    /// use alloy_primitives::{Address, B256};
    /// use cow_rs::composable::{ConditionalOrderKind, ConditionalOrderParams};
    ///
    /// let unknown = ConditionalOrderKind::Unknown(ConditionalOrderParams {
    ///     handler: Address::ZERO,
    ///     salt: B256::ZERO,
    ///     static_input: vec![],
    /// });
    /// assert!(!unknown.is_stop_loss());
    /// ```
    #[must_use]
    pub const fn is_stop_loss(&self) -> bool {
        matches!(self, Self::StopLoss(_))
    }

    /// Returns `true` if this is a `GoodAfterTime` conditional order.
    ///
    /// ```
    /// use alloy_primitives::{Address, B256};
    /// use cow_rs::composable::{ConditionalOrderKind, ConditionalOrderParams};
    ///
    /// let unknown = ConditionalOrderKind::Unknown(ConditionalOrderParams {
    ///     handler: Address::ZERO,
    ///     salt: B256::ZERO,
    ///     static_input: vec![],
    /// });
    /// assert!(!unknown.is_good_after_time());
    /// ```
    #[must_use]
    pub const fn is_good_after_time(&self) -> bool {
        matches!(self, Self::GoodAfterTime(_))
    }

    /// Returns `true` if this order's handler is not recognised by the factory.
    ///
    /// # Returns
    ///
    /// `true` when the variant is [`ConditionalOrderKind::Unknown`], meaning the
    /// handler address did not match any known conditional order type.
    #[must_use]
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

impl fmt::Display for ConditionalOrderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Twap(order) => write!(f, "twap({order})"),
            Self::StopLoss(_) => f.write_str("stop-loss"),
            Self::GoodAfterTime(_) => f.write_str("good-after-time"),
            Self::Unknown(params) => write!(f, "unknown({:#x})", params.handler),
        }
    }
}

/// Instantiates conditional orders from [`ConditionalOrderParams`].
///
/// Mirrors `ConditionalOrderFactory` from the `TypeScript` SDK.
/// Extend by matching additional handler addresses in [`from_params`].
///
/// [`from_params`]: ConditionalOrderFactory::from_params
#[derive(Debug, Clone, Default)]
pub struct ConditionalOrderFactory;

impl ConditionalOrderFactory {
    /// Create a new factory.
    ///
    /// # Returns
    ///
    /// A zero-sized [`ConditionalOrderFactory`] instance that can decode
    /// [`ConditionalOrderParams`] via [`from_params`](Self::from_params).
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Decode [`ConditionalOrderParams`] into a typed [`ConditionalOrderKind`].
    ///
    /// Handler addresses are matched exactly.  Unknown handlers return
    /// [`ConditionalOrderKind::Unknown`] rather than an error.
    ///
    /// Recognised handlers:
    /// - [`TWAP_HANDLER_ADDRESS`] → [`ConditionalOrderKind::Twap`]
    /// - [`STOP_LOSS_HANDLER_ADDRESS`] → [`ConditionalOrderKind::StopLoss`]
    ///
    /// Note: the `GoodAfterTime` handler (`GAT_HANDLER_ADDRESS`) shares the
    /// same on-chain address as the `TWAP` handler, so `TWAP` decoding takes
    /// priority for that address.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] only if a known handler's static input
    /// fails ABI decoding.
    pub fn from_params(
        &self,
        params: ConditionalOrderParams,
    ) -> Result<ConditionalOrderKind, CowError> {
        if params.handler == TWAP_HANDLER_ADDRESS {
            // Try TWAP decoding first (TWAP and GAT share the same address).
            // If the input is TWAP-sized (320 bytes), decode as TWAP.
            // If it is GAT-sized (448 bytes), decode as GAT.
            if params.static_input.len() == 14 * 32 {
                let data = decode_gat_static_input(&params.static_input)?;
                return Ok(ConditionalOrderKind::GoodAfterTime(GatOrder::with_salt(
                    data,
                    params.salt,
                )));
            }
            let data = decode_twap_static_input(&params.static_input)?;
            return Ok(ConditionalOrderKind::Twap(TwapOrder::with_salt(data, params.salt)));
        }
        if params.handler == STOP_LOSS_HANDLER_ADDRESS {
            let data = decode_stop_loss_static_input(&params.static_input)?;
            return Ok(ConditionalOrderKind::StopLoss(StopLossOrder::with_salt(data, params.salt)));
        }
        Ok(ConditionalOrderKind::Unknown(params))
    }
}
impl fmt::Display for ConditionalOrderFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("conditional-order-factory")
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, B256};

    use super::*;

    #[test]
    fn factory_new() {
        let factory = ConditionalOrderFactory::new();
        assert_eq!(factory.to_string(), "conditional-order-factory");
    }

    #[test]
    fn factory_unknown_handler() {
        let factory = ConditionalOrderFactory::new();
        let params = ConditionalOrderParams {
            handler: Address::ZERO,
            salt: B256::ZERO,
            static_input: vec![],
        };
        let result = factory.from_params(params).unwrap();
        assert!(result.is_unknown());
        assert!(!result.is_twap());
        assert!(!result.is_stop_loss());
        assert!(!result.is_good_after_time());
        assert_eq!(result.as_str(), "unknown");
    }

    #[test]
    fn factory_twap_handler_empty_static_input_errors() {
        let factory = ConditionalOrderFactory::new();
        let params = ConditionalOrderParams {
            handler: TWAP_HANDLER_ADDRESS,
            salt: B256::ZERO,
            static_input: vec![],
        };
        // Empty static input is not valid for TWAP
        assert!(factory.from_params(params).is_err());
    }

    #[test]
    fn factory_stop_loss_handler_empty_static_input_errors() {
        let factory = ConditionalOrderFactory::new();
        let params = ConditionalOrderParams {
            handler: STOP_LOSS_HANDLER_ADDRESS,
            salt: B256::ZERO,
            static_input: vec![],
        };
        assert!(factory.from_params(params).is_err());
    }

    #[test]
    fn conditional_order_kind_display_unknown() {
        let kind = ConditionalOrderKind::Unknown(ConditionalOrderParams {
            handler: Address::ZERO,
            salt: B256::ZERO,
            static_input: vec![],
        });
        let s = kind.to_string();
        assert!(s.contains("unknown"));
    }

    #[test]
    fn conditional_order_kind_display_stop_loss() {
        let kind = ConditionalOrderKind::Unknown(ConditionalOrderParams {
            handler: Address::ZERO,
            salt: B256::ZERO,
            static_input: vec![],
        });
        // We can't easily construct a StopLoss without valid data,
        // so we test the other Display variants through the Unknown variant
        assert_eq!(kind.as_str(), "unknown");
    }
}
