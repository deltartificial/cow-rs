//! [`UnsignedOrder`] — the canonical `CoW` Protocol order struct before signing.
//!
//! This type used to live in `cow-signing::types`, but it is referenced by
//! both `cow-signing` (for EIP-712 hashing) and `cow-settlement` (for encoder
//! and trade building). Keeping it in an L2 sibling crate would require a
//! sibling dependency between the two, so it has been pushed down to L1.
//!
//! The former convenience method `UnsignedOrder::hash` (which delegated to
//! `cow_signing::order_hash`) has been dropped during the move. Call
//! [`cow_signing::order_hash`](https://docs.rs/cow-signing) directly instead.

use std::fmt;

use alloy_primitives::{Address, B256, U256};

use crate::{OrderKind, TokenBalance};

/// An unsigned `CoW` Protocol order ready to be hashed and signed.
#[derive(Debug, Clone)]
pub struct UnsignedOrder {
    /// Token to sell.
    pub sell_token: Address,
    /// Token to buy.
    pub buy_token: Address,
    /// Address that receives the bought tokens.
    pub receiver: Address,
    /// Amount of `sell_token` to sell (after fee, in atoms).
    pub sell_amount: U256,
    /// Minimum amount of `buy_token` to receive (in atoms).
    pub buy_amount: U256,
    /// Order expiry as Unix timestamp.
    pub valid_to: u32,
    /// App-data hash (`bytes32`).
    pub app_data: B256,
    /// Protocol fee included in `sell_amount` (in atoms).
    pub fee_amount: U256,
    /// Sell or buy direction.
    pub kind: OrderKind,
    /// Whether the order may be partially filled.
    pub partially_fillable: bool,
    /// Source of sell funds.
    pub sell_token_balance: TokenBalance,
    /// Destination of buy funds.
    pub buy_token_balance: TokenBalance,
}

impl UnsignedOrder {
    /// Construct a **sell** order with defaults: ERC-20 balances, `fee_amount = 0`,
    /// `app_data = B256::ZERO`, `valid_to = 0`, `receiver = Address::ZERO`.
    ///
    /// Use the builder methods to override any field before signing.
    ///
    /// # Arguments
    ///
    /// * `sell_token` - Address of the token to sell.
    /// * `buy_token` - Address of the token to buy.
    /// * `sell_amount` - Amount of `sell_token` to sell (in atoms).
    /// * `buy_amount` - Minimum amount of `buy_token` to receive (in atoms).
    ///
    /// # Returns
    ///
    /// A new [`UnsignedOrder`] with [`OrderKind::Sell`] and sensible defaults.
    #[must_use]
    pub const fn sell(
        sell_token: Address,
        buy_token: Address,
        sell_amount: U256,
        buy_amount: U256,
    ) -> Self {
        Self {
            sell_token,
            buy_token,
            receiver: Address::ZERO,
            sell_amount,
            buy_amount,
            valid_to: 0,
            app_data: B256::ZERO,
            fee_amount: U256::ZERO,
            kind: OrderKind::Sell,
            partially_fillable: false,
            sell_token_balance: TokenBalance::Erc20,
            buy_token_balance: TokenBalance::Erc20,
        }
    }

    /// Construct a **buy** order with defaults: ERC-20 balances, `fee_amount = 0`,
    /// `app_data = B256::ZERO`, `valid_to = 0`, `receiver = Address::ZERO`.
    ///
    /// # Arguments
    ///
    /// * `sell_token` - Address of the token to sell.
    /// * `buy_token` - Address of the token to buy.
    /// * `sell_amount` - Maximum amount of `sell_token` willing to sell (in atoms).
    /// * `buy_amount` - Amount of `buy_token` to buy (in atoms).
    ///
    /// # Returns
    ///
    /// A new [`UnsignedOrder`] with [`OrderKind::Buy`] and sensible defaults.
    #[must_use]
    pub const fn buy(
        sell_token: Address,
        buy_token: Address,
        sell_amount: U256,
        buy_amount: U256,
    ) -> Self {
        Self {
            sell_token,
            buy_token,
            receiver: Address::ZERO,
            sell_amount,
            buy_amount,
            valid_to: 0,
            app_data: B256::ZERO,
            fee_amount: U256::ZERO,
            kind: OrderKind::Buy,
            partially_fillable: false,
            sell_token_balance: TokenBalance::Erc20,
            buy_token_balance: TokenBalance::Erc20,
        }
    }

    /// Override the receiver address.
    #[must_use]
    pub const fn with_receiver(mut self, receiver: Address) -> Self {
        self.receiver = receiver;
        self
    }

    /// Set the order expiry as a Unix timestamp.
    #[must_use]
    pub const fn with_valid_to(mut self, valid_to: u32) -> Self {
        self.valid_to = valid_to;
        self
    }

    /// Set the app-data hash.
    #[must_use]
    pub const fn with_app_data(mut self, app_data: B256) -> Self {
        self.app_data = app_data;
        self
    }

    /// Override the fee amount (defaults to zero).
    #[must_use]
    pub const fn with_fee_amount(mut self, fee_amount: U256) -> Self {
        self.fee_amount = fee_amount;
        self
    }

    /// Allow partial fills.
    #[must_use]
    pub const fn with_partially_fillable(mut self) -> Self {
        self.partially_fillable = true;
        self
    }

    /// Override the sell-token balance source.
    #[must_use]
    pub const fn with_sell_token_balance(mut self, balance: TokenBalance) -> Self {
        self.sell_token_balance = balance;
        self
    }

    /// Override the buy-token balance destination.
    #[must_use]
    pub const fn with_buy_token_balance(mut self, balance: TokenBalance) -> Self {
        self.buy_token_balance = balance;
        self
    }

    /// Returns `true` if the order has expired at the given Unix timestamp.
    ///
    /// An order is expired when `timestamp > valid_to`.
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_types::UnsignedOrder;
    ///
    /// let order = UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::ZERO, U256::ZERO)
    ///     .with_valid_to(1_000_000);
    /// assert!(!order.is_expired(999_999));
    /// assert!(!order.is_expired(1_000_000)); // valid_to is inclusive
    /// assert!(order.is_expired(1_000_001));
    /// ```
    #[must_use]
    pub const fn is_expired(&self, timestamp: u64) -> bool {
        timestamp > self.valid_to as u64
    }

    /// Returns `true` if this is a sell-direction order.
    #[must_use]
    pub const fn is_sell(&self) -> bool {
        self.kind.is_sell()
    }

    /// Returns `true` if this is a buy-direction order.
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        self.kind.is_buy()
    }

    /// Returns `true` if a non-zero receiver address is set.
    #[must_use]
    pub fn has_custom_receiver(&self) -> bool {
        !self.receiver.is_zero()
    }

    /// Returns `true` if a non-zero app-data hash is attached.
    #[must_use]
    pub fn has_app_data(&self) -> bool {
        !self.app_data.is_zero()
    }

    /// Returns `true` if the fee amount is non-zero.
    #[must_use]
    pub fn has_fee(&self) -> bool {
        !self.fee_amount.is_zero()
    }

    /// Returns `true` if this order allows partial fills.
    #[must_use]
    pub const fn is_partially_fillable(&self) -> bool {
        self.partially_fillable
    }

    /// Returns the total token amount at stake: `sell_amount + buy_amount`.
    ///
    /// Uses saturating addition to avoid overflow on extreme values.
    #[must_use]
    pub const fn total_amount(&self) -> U256 {
        self.sell_amount.saturating_add(self.buy_amount)
    }
}

impl fmt::Display for UnsignedOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:#x} → {:#x}", self.kind, self.sell_token, self.buy_token)
    }
}
