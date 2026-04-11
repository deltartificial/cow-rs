//! Core signing types for `CoW` Protocol orders.

use std::fmt;

use alloy_primitives::{Address, B256, U256};

use crate::types::{OrderKind, SigningScheme, TokenBalance};

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
    ///
    /// # Arguments
    ///
    /// * `receiver` - Address that will receive the bought tokens.
    ///
    /// # Returns
    ///
    /// The order with the updated receiver.
    #[must_use]
    pub const fn with_receiver(mut self, receiver: Address) -> Self {
        self.receiver = receiver;
        self
    }

    /// Set the order expiry as a Unix timestamp.
    ///
    /// # Arguments
    ///
    /// * `valid_to` - Unix timestamp after which the order expires.
    ///
    /// # Returns
    ///
    /// The order with the updated expiry.
    #[must_use]
    pub const fn with_valid_to(mut self, valid_to: u32) -> Self {
        self.valid_to = valid_to;
        self
    }

    /// Set the app-data hash.
    ///
    /// # Arguments
    ///
    /// * `app_data` - A 32-byte hash identifying application-specific metadata.
    ///
    /// # Returns
    ///
    /// The order with the updated app-data hash.
    #[must_use]
    pub const fn with_app_data(mut self, app_data: B256) -> Self {
        self.app_data = app_data;
        self
    }

    /// Override the fee amount (defaults to zero).
    ///
    /// # Arguments
    ///
    /// * `fee_amount` - Protocol fee included in `sell_amount` (in atoms).
    ///
    /// # Returns
    ///
    /// The order with the updated fee amount.
    #[must_use]
    pub const fn with_fee_amount(mut self, fee_amount: U256) -> Self {
        self.fee_amount = fee_amount;
        self
    }

    /// Allow partial fills.
    ///
    /// # Returns
    ///
    /// The order with `partially_fillable` set to `true`.
    #[must_use]
    pub const fn with_partially_fillable(mut self) -> Self {
        self.partially_fillable = true;
        self
    }

    /// Override the sell-token balance source.
    ///
    /// # Arguments
    ///
    /// * `balance` - The [`TokenBalance`] variant describing the sell-token source.
    ///
    /// # Returns
    ///
    /// The order with the updated sell-token balance source.
    #[must_use]
    pub const fn with_sell_token_balance(mut self, balance: TokenBalance) -> Self {
        self.sell_token_balance = balance;
        self
    }

    /// Override the buy-token balance destination.
    ///
    /// # Arguments
    ///
    /// * `balance` - The [`TokenBalance`] variant describing the buy-token destination.
    ///
    /// # Returns
    ///
    /// The order with the updated buy-token balance destination.
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
    /// use cow_rs::UnsignedOrder;
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
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::UnsignedOrder;
    ///
    /// let order = UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::ZERO, U256::ZERO);
    /// assert!(order.is_sell());
    /// assert!(!order.is_buy());
    /// ```
    #[must_use]
    pub const fn is_sell(&self) -> bool {
        self.kind.is_sell()
    }

    /// Returns `true` if this is a buy-direction order.
    ///
    /// # Returns
    ///
    /// `true` when `kind` is [`OrderKind::Buy`].
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        self.kind.is_buy()
    }

    /// Returns `true` if a non-zero receiver address is set.
    ///
    /// When `receiver` is [`Address::ZERO`], the settlement contract uses the
    /// order owner as the effective receiver.
    ///
    /// ```
    /// use alloy_primitives::{Address, U256, address};
    /// use cow_rs::UnsignedOrder;
    ///
    /// let order = UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::ZERO, U256::ZERO);
    /// assert!(!order.has_custom_receiver());
    ///
    /// let with_recv = order.with_receiver(address!("0000000000000000000000000000000000000001"));
    /// assert!(with_recv.has_custom_receiver());
    /// ```
    #[must_use]
    pub fn has_custom_receiver(&self) -> bool {
        !self.receiver.is_zero()
    }

    /// Returns `true` if a non-zero app-data hash is attached.
    ///
    /// The zero hash (`B256::ZERO`) means no app-data was set.
    ///
    /// # Returns
    ///
    /// `true` when `app_data` is not [`B256::ZERO`].
    #[must_use]
    pub fn has_app_data(&self) -> bool {
        !self.app_data.is_zero()
    }

    /// Returns `true` if the fee amount is non-zero.
    ///
    /// # Returns
    ///
    /// `true` when `fee_amount` is not [`U256::ZERO`].
    #[must_use]
    pub fn has_fee(&self) -> bool {
        !self.fee_amount.is_zero()
    }

    /// Returns `true` if this order allows partial fills.
    ///
    /// Getter for the `partially_fillable` field — useful in generic contexts
    /// where the field is accessed through a trait or closure.
    ///
    /// # Returns
    ///
    /// `true` when the order may be partially filled.
    #[must_use]
    pub const fn is_partially_fillable(&self) -> bool {
        self.partially_fillable
    }

    /// Returns the total token amount at stake: `sell_amount + buy_amount`.
    ///
    /// Useful for quick balance-sufficiency checks. Uses saturating addition
    /// to avoid overflow on extreme values.
    ///
    /// # Returns
    ///
    /// The saturating sum of `sell_amount` and `buy_amount`.
    #[must_use]
    pub const fn total_amount(&self) -> U256 {
        self.sell_amount.saturating_add(self.buy_amount)
    }

    /// Compute the EIP-712 struct hash for this order.
    ///
    /// This is the `keccak256` hash of the ABI-encoded order fields used as
    /// the leaf in the EIP-712 digest. To obtain the full signing digest
    /// (with domain separator), use [`crate::order_signing::signing_digest`].
    ///
    /// ```
    /// use alloy_primitives::{Address, B256, U256};
    /// use cow_rs::UnsignedOrder;
    ///
    /// let order = UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::ZERO, U256::ZERO);
    /// let h = order.hash();
    /// assert_ne!(h, B256::ZERO);
    /// ```
    #[must_use]
    pub fn hash(&self) -> alloy_primitives::B256 {
        crate::order_signing::order_hash(self)
    }
}

/// The EIP-712 domain for `CoW` Protocol orders.
///
/// Mirrors `TypedDataDomain` from the `TypeScript` SDK.
#[derive(Debug, Clone)]
pub struct OrderDomain {
    /// Protocol name (`"Gnosis Protocol v2"`).
    pub name: &'static str,
    /// Protocol version (`"v2"`).
    pub version: &'static str,
    /// Chain ID where orders are settled.
    pub chain_id: u64,
    /// `GPv2Settlement` contract address (the EIP-712 verifying contract).
    pub verifying_contract: Address,
}

impl OrderDomain {
    /// Construct the standard `CoW` Protocol EIP-712 domain for `chain_id`.
    ///
    /// Uses the canonical [`SETTLEMENT_CONTRACT`](crate::config::contracts::SETTLEMENT_CONTRACT)
    /// address as the verifying contract.
    ///
    /// # Arguments
    ///
    /// * `chain_id` - The EVM chain ID where orders will be settled.
    ///
    /// # Returns
    ///
    /// An [`OrderDomain`] configured for the given chain.
    #[must_use]
    pub const fn for_chain(chain_id: u64) -> Self {
        Self {
            name: "Gnosis Protocol v2",
            version: "v2",
            chain_id,
            verifying_contract: crate::config::contracts::SETTLEMENT_CONTRACT,
        }
    }

    /// Compute the EIP-712 domain separator for this domain.
    ///
    /// Convenience wrapper around [`crate::order_signing::domain_separator`]
    /// using `self.chain_id`.
    ///
    /// ```
    /// use cow_rs::OrderDomain;
    ///
    /// let domain = OrderDomain::for_chain(1);
    /// let sep = domain.domain_separator();
    /// assert_ne!(sep, alloy_primitives::B256::ZERO);
    /// ```
    #[must_use]
    pub fn domain_separator(&self) -> alloy_primitives::B256 {
        crate::order_signing::domain_separator(self.chain_id)
    }
}

/// The full EIP-712 typed data envelope for a `CoW` Protocol order.
///
/// Mirrors `OrderTypedData` from the `TypeScript` SDK.  Pass this to a hardware
/// wallet or any EIP-712-aware signer that needs the structured domain and types
/// alongside the order message.
#[derive(Debug, Clone)]
pub struct OrderTypedData {
    /// The EIP-712 domain for `CoW` Protocol.
    pub domain: OrderDomain,
    /// EIP-712 primary type name (`"GPv2Order.Data"`).
    pub primary_type: &'static str,
    /// The order message to sign.
    pub order: UnsignedOrder,
}

impl OrderTypedData {
    /// Construct an [`OrderTypedData`] envelope for the given domain and order.
    ///
    /// The primary type is always `"GPv2Order.Data"` per the `CoW` Protocol EIP-712 spec.
    ///
    /// # Arguments
    ///
    /// * `domain` - The EIP-712 domain for the target chain.
    /// * `order` - The unsigned order to wrap.
    ///
    /// # Returns
    ///
    /// An [`OrderTypedData`] envelope ready for signing.
    #[must_use]
    pub const fn new(domain: OrderDomain, order: UnsignedOrder) -> Self {
        Self { domain, primary_type: "GPv2Order.Data", order }
    }

    /// Returns a reference to the underlying [`UnsignedOrder`].
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::{OrderDomain, OrderTypedData, UnsignedOrder};
    ///
    /// let order = UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::ZERO, U256::ZERO);
    /// let typed = OrderTypedData::new(OrderDomain::for_chain(1), order.clone());
    /// assert_eq!(typed.order_ref().kind, order.kind);
    /// ```
    #[must_use]
    pub const fn order_ref(&self) -> &UnsignedOrder {
        &self.order
    }

    /// Returns a reference to the [`OrderDomain`].
    ///
    /// # Returns
    ///
    /// A shared reference to the inner [`OrderDomain`].
    #[must_use]
    pub const fn domain_ref(&self) -> &OrderDomain {
        &self.domain
    }

    /// Compute the full EIP-712 signing digest for this typed data.
    ///
    /// This is the `keccak256("\x19\x01" ‖ domainSep ‖ orderHash)` value that
    /// must be signed with a private key to produce a signature accepted by the
    /// `CoW` Protocol settlement contract.
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::{OrderDomain, OrderTypedData, UnsignedOrder};
    ///
    /// let order = UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::ZERO, U256::ZERO);
    /// let typed = OrderTypedData::new(OrderDomain::for_chain(11_155_111), order);
    /// let digest = typed.signing_digest();
    /// assert_ne!(digest, alloy_primitives::B256::ZERO);
    /// ```
    #[must_use]
    pub fn signing_digest(&self) -> alloy_primitives::B256 {
        let domain_sep = crate::order_signing::domain_separator(self.domain.chain_id);
        let o_hash = crate::order_signing::order_hash(&self.order);
        crate::order_signing::signing_digest(domain_sep, o_hash)
    }
}

/// The result of signing an order — signature bytes and the scheme used.
#[derive(Debug, Clone)]
pub struct SigningResult {
    /// `0x`-prefixed hex-encoded signature.
    ///
    /// - EIP-712 / EIP-191: 65-byte `r | s | v` encoding.
    /// - EIP-1271: arbitrary bytes returned by the smart-contract signer.
    /// - Pre-sign: the 20-byte owner address.
    pub signature: String,
    /// The signing scheme that produced this signature.
    pub signing_scheme: SigningScheme,
}

impl SigningResult {
    /// Construct a [`SigningResult`] from a signature hex string and scheme.
    ///
    /// # Arguments
    ///
    /// * `signature` - A `0x`-prefixed hex-encoded signature string.
    /// * `signing_scheme` - The [`SigningScheme`] that produced the signature.
    ///
    /// # Returns
    ///
    /// A new [`SigningResult`].
    #[must_use]
    pub fn new(signature: impl Into<String>, signing_scheme: SigningScheme) -> Self {
        Self { signature: signature.into(), signing_scheme }
    }

    /// Returns `true` if this result used the EIP-712 signing scheme.
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::{EcdsaSigningScheme, OrderDomain, UnsignedOrder};
    ///
    /// let result = cow_rs::SigningResult::new("0xdeadbeef", cow_rs::SigningScheme::Eip712);
    /// assert!(result.is_eip712());
    /// assert!(!result.is_presign());
    /// ```
    #[must_use]
    pub const fn is_eip712(&self) -> bool {
        matches!(self.signing_scheme, SigningScheme::Eip712)
    }

    /// Returns `true` if this result used the EIP-191 (`eth_sign`) scheme.
    ///
    /// # Returns
    ///
    /// `true` when `signing_scheme` is [`SigningScheme::EthSign`].
    #[must_use]
    pub const fn is_eth_sign(&self) -> bool {
        matches!(self.signing_scheme, SigningScheme::EthSign)
    }

    /// Returns `true` if this result used the EIP-1271 smart-contract scheme.
    ///
    /// ```
    /// use cow_rs::order_signing::eip1271_result;
    ///
    /// let result = eip1271_result(&[0xde, 0xad]);
    /// assert!(result.is_eip1271());
    /// assert!(!result.is_eip712());
    /// ```
    #[must_use]
    pub const fn is_eip1271(&self) -> bool {
        matches!(self.signing_scheme, SigningScheme::Eip1271)
    }

    /// Returns `true` if this result used the on-chain pre-sign scheme.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::order_signing::presign_result;
    ///
    /// let result = presign_result(Address::ZERO);
    /// assert!(result.is_presign());
    /// assert!(!result.is_eip712());
    /// ```
    #[must_use]
    pub const fn is_presign(&self) -> bool {
        matches!(self.signing_scheme, SigningScheme::PreSign)
    }

    /// Returns the length of the signature string in bytes.
    ///
    /// ```
    /// use cow_rs::SigningResult;
    ///
    /// let result = SigningResult::new("0xdeadbeef", cow_rs::SigningScheme::Eip712);
    /// assert_eq!(result.signature_len(), 10);
    /// ```
    #[must_use]
    pub const fn signature_len(&self) -> usize {
        self.signature.len()
    }

    /// Returns the signature as a `0x`-prefixed hex string slice.
    ///
    /// ```
    /// use cow_rs::SigningResult;
    ///
    /// let result = SigningResult::new("0xdeadbeef", cow_rs::SigningScheme::Eip712);
    /// assert_eq!(result.signature_ref(), "0xdeadbeef");
    /// ```
    #[must_use]
    pub fn signature_ref(&self) -> &str {
        &self.signature
    }
}

impl fmt::Display for UnsignedOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:#x} → {:#x}", self.kind, self.sell_token, self.buy_token)
    }
}

impl fmt::Display for OrderDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "domain(chain={}, contract={:#x})", self.chain_id, self.verifying_contract)
    }
}

impl fmt::Display for OrderTypedData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "typed-data(chain={}, {})", self.domain.chain_id, self.order)
    }
}

impl fmt::Display for SigningResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sig = &self.signature;
        let short = if sig.len() > 10 { &sig[..10] } else { sig };
        write!(f, "sig({}, {}…)", self.signing_scheme, short)
    }
}

// ── Signing parameter types ──────────────────────────────────────────────────

/// Parameters for signing a `CoW` Protocol order.
///
/// Mirrors `SignOrderParams` from the `TypeScript` SDK. In Rust the signer
/// is typically passed separately to `sign_order`, so this struct bundles
/// the remaining context needed to produce a valid signature.
#[derive(Debug, Clone)]
pub struct SignOrderParams {
    /// Chain ID on which the order will be settled.
    pub chain_id: u64,
    /// The unsigned order intent to sign.
    pub order: UnsignedOrder,
    /// The ECDSA signing scheme to use.
    pub signing_scheme: crate::types::EcdsaSigningScheme,
}

impl SignOrderParams {
    /// Construct a [`SignOrderParams`] from its three core fields.
    ///
    /// # Arguments
    ///
    /// * `chain_id` - Chain ID on which the order will be settled.
    /// * `order` - The unsigned order intent to sign.
    /// * `signing_scheme` - The ECDSA signing scheme to use.
    ///
    /// # Returns
    ///
    /// A new [`SignOrderParams`].
    #[must_use]
    pub const fn new(
        chain_id: u64,
        order: UnsignedOrder,
        signing_scheme: crate::types::EcdsaSigningScheme,
    ) -> Self {
        Self { chain_id, order, signing_scheme }
    }
}

impl fmt::Display for SignOrderParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sign-order(chain={}, {})", self.chain_id, self.order)
    }
}

/// Parameters for signing a single order cancellation.
///
/// Mirrors `SignOrderCancellationParams` from the `TypeScript` SDK.
#[derive(Debug, Clone)]
pub struct SignOrderCancellationParams {
    /// Chain ID on which the order was placed.
    pub chain_id: u64,
    /// The unique identifier of the order to cancel.
    pub order_uid: String,
    /// The ECDSA signing scheme to use.
    pub signing_scheme: crate::types::EcdsaSigningScheme,
}

impl SignOrderCancellationParams {
    /// Construct a [`SignOrderCancellationParams`].
    ///
    /// # Arguments
    ///
    /// * `chain_id` - Chain ID on which the order was placed.
    /// * `order_uid` - The unique identifier of the order to cancel.
    /// * `signing_scheme` - The ECDSA signing scheme to use.
    ///
    /// # Returns
    ///
    /// A new [`SignOrderCancellationParams`].
    #[must_use]
    pub fn new(
        chain_id: u64,
        order_uid: impl Into<String>,
        signing_scheme: crate::types::EcdsaSigningScheme,
    ) -> Self {
        Self { chain_id, order_uid: order_uid.into(), signing_scheme }
    }
}

impl fmt::Display for SignOrderCancellationParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let uid = &self.order_uid;
        let short = if uid.len() > 10 { &uid[..10] } else { uid };
        write!(f, "cancel-sign(chain={}, uid={short}…)", self.chain_id)
    }
}

/// Parameters for signing multiple order cancellations in bulk.
///
/// Mirrors `SignOrderCancellationsParams` from the `TypeScript` SDK.
#[derive(Debug, Clone)]
pub struct SignOrderCancellationsParams {
    /// Chain ID on which the orders were placed.
    pub chain_id: u64,
    /// Unique identifiers of the orders to cancel.
    pub order_uids: Vec<String>,
    /// The ECDSA signing scheme to use.
    pub signing_scheme: crate::types::EcdsaSigningScheme,
}

impl SignOrderCancellationsParams {
    /// Construct a [`SignOrderCancellationsParams`].
    ///
    /// # Arguments
    ///
    /// * `chain_id` - Chain ID on which the orders were placed.
    /// * `order_uids` - Unique identifiers of the orders to cancel.
    /// * `signing_scheme` - The ECDSA signing scheme to use.
    ///
    /// # Returns
    ///
    /// A new [`SignOrderCancellationsParams`].
    #[must_use]
    pub const fn new(
        chain_id: u64,
        order_uids: Vec<String>,
        signing_scheme: crate::types::EcdsaSigningScheme,
    ) -> Self {
        Self { chain_id, order_uids, signing_scheme }
    }

    /// Returns the number of orders to cancel.
    ///
    /// # Returns
    ///
    /// The length of the `order_uids` list.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.order_uids.len()
    }
}

impl fmt::Display for SignOrderCancellationsParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cancel-signs(chain={}, count={})", self.chain_id, self.order_uids.len())
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::address;

    use super::*;
    use crate::types::EcdsaSigningScheme;

    fn default_order() -> UnsignedOrder {
        UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::ZERO, U256::ZERO)
    }

    // ── UnsignedOrder constructors ──────────────────────────────────────

    #[test]
    fn sell_order_has_sell_kind() {
        let o = UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::from(100u64), U256::from(50u64));
        assert_eq!(o.kind, OrderKind::Sell);
        assert!(o.is_sell());
        assert!(!o.is_buy());
        assert_eq!(o.sell_amount, U256::from(100u64));
        assert_eq!(o.buy_amount, U256::from(50u64));
    }

    #[test]
    fn buy_order_has_buy_kind() {
        let o = UnsignedOrder::buy(Address::ZERO, Address::ZERO, U256::from(100u64), U256::from(50u64));
        assert_eq!(o.kind, OrderKind::Buy);
        assert!(o.is_buy());
        assert!(!o.is_sell());
    }

    #[test]
    fn sell_order_defaults() {
        let o = default_order();
        assert_eq!(o.receiver, Address::ZERO);
        assert_eq!(o.valid_to, 0);
        assert_eq!(o.app_data, B256::ZERO);
        assert_eq!(o.fee_amount, U256::ZERO);
        assert!(!o.partially_fillable);
        assert_eq!(o.sell_token_balance, TokenBalance::Erc20);
        assert_eq!(o.buy_token_balance, TokenBalance::Erc20);
    }

    // ── Builder methods ─────────────────────────────────────────────────

    #[test]
    fn with_receiver() {
        let addr = address!("0000000000000000000000000000000000000001");
        let o = default_order().with_receiver(addr);
        assert_eq!(o.receiver, addr);
    }

    #[test]
    fn with_valid_to() {
        let o = default_order().with_valid_to(1_700_000_000);
        assert_eq!(o.valid_to, 1_700_000_000);
    }

    #[test]
    fn with_app_data() {
        let data = B256::from([0xab; 32]);
        let o = default_order().with_app_data(data);
        assert_eq!(o.app_data, data);
    }

    #[test]
    fn with_fee_amount() {
        let o = default_order().with_fee_amount(U256::from(42u64));
        assert_eq!(o.fee_amount, U256::from(42u64));
    }

    #[test]
    fn with_partially_fillable() {
        let o = default_order().with_partially_fillable();
        assert!(o.partially_fillable);
        assert!(o.is_partially_fillable());
    }

    #[test]
    fn with_sell_token_balance() {
        let o = default_order().with_sell_token_balance(TokenBalance::External);
        assert_eq!(o.sell_token_balance, TokenBalance::External);
    }

    #[test]
    fn with_buy_token_balance() {
        let o = default_order().with_buy_token_balance(TokenBalance::Internal);
        assert_eq!(o.buy_token_balance, TokenBalance::Internal);
    }

    // ── Query methods ───────────────────────────────────────────────────

    #[test]
    fn is_expired_boundary() {
        let o = default_order().with_valid_to(1_000_000);
        assert!(!o.is_expired(999_999));
        assert!(!o.is_expired(1_000_000));
        assert!(o.is_expired(1_000_001));
    }

    #[test]
    fn has_custom_receiver_false_for_zero() {
        assert!(!default_order().has_custom_receiver());
    }

    #[test]
    fn has_custom_receiver_true_for_nonzero() {
        let o = default_order().with_receiver(address!("0000000000000000000000000000000000000001"));
        assert!(o.has_custom_receiver());
    }

    #[test]
    fn has_app_data_false_for_zero() {
        assert!(!default_order().has_app_data());
    }

    #[test]
    fn has_app_data_true_for_nonzero() {
        let o = default_order().with_app_data(B256::from([1u8; 32]));
        assert!(o.has_app_data());
    }

    #[test]
    fn has_fee_false_for_zero() {
        assert!(!default_order().has_fee());
    }

    #[test]
    fn has_fee_true_for_nonzero() {
        let o = default_order().with_fee_amount(U256::from(1u64));
        assert!(o.has_fee());
    }

    #[test]
    fn is_partially_fillable_default_false() {
        assert!(!default_order().is_partially_fillable());
    }

    #[test]
    fn total_amount_sums_sell_and_buy() {
        let o = UnsignedOrder::sell(
            Address::ZERO,
            Address::ZERO,
            U256::from(100u64),
            U256::from(50u64),
        );
        assert_eq!(o.total_amount(), U256::from(150u64));
    }

    #[test]
    fn total_amount_saturates_on_overflow() {
        let o = UnsignedOrder::sell(Address::ZERO, Address::ZERO, U256::MAX, U256::from(1u64));
        assert_eq!(o.total_amount(), U256::MAX);
    }

    #[test]
    fn order_hash_is_deterministic() {
        let o = default_order();
        assert_eq!(o.hash(), o.hash());
        assert_ne!(o.hash(), B256::ZERO);
    }

    // ── OrderDomain ─────────────────────────────────────────────────────

    #[test]
    fn order_domain_for_chain() {
        let d = OrderDomain::for_chain(1);
        assert_eq!(d.name, "Gnosis Protocol v2");
        assert_eq!(d.version, "v2");
        assert_eq!(d.chain_id, 1);
    }

    #[test]
    fn domain_separator_is_deterministic() {
        let d = OrderDomain::for_chain(1);
        let sep1 = d.domain_separator();
        let sep2 = d.domain_separator();
        assert_eq!(sep1, sep2);
        assert_ne!(sep1, B256::ZERO);
    }

    #[test]
    fn different_chains_different_separators() {
        let s1 = OrderDomain::for_chain(1).domain_separator();
        let s2 = OrderDomain::for_chain(100).domain_separator();
        assert_ne!(s1, s2);
    }

    // ── OrderTypedData ──────────────────────────────────────────────────

    #[test]
    fn order_typed_data_primary_type() {
        let td = OrderTypedData::new(OrderDomain::for_chain(1), default_order());
        assert_eq!(td.primary_type, "GPv2Order.Data");
    }

    #[test]
    fn order_typed_data_refs() {
        let order = default_order();
        let td = OrderTypedData::new(OrderDomain::for_chain(1), order.clone());
        assert_eq!(td.order_ref().kind, order.kind);
        assert_eq!(td.domain_ref().chain_id, 1);
    }

    #[test]
    fn signing_digest_is_deterministic_and_nonzero() {
        let td = OrderTypedData::new(OrderDomain::for_chain(1), default_order());
        let d1 = td.signing_digest();
        let d2 = td.signing_digest();
        assert_eq!(d1, d2);
        assert_ne!(d1, B256::ZERO);
    }

    // ── SigningResult ───────────────────────────────────────────────────

    #[test]
    fn signing_result_new() {
        let r = SigningResult::new("0xdeadbeef", SigningScheme::Eip712);
        assert_eq!(r.signature, "0xdeadbeef");
        assert_eq!(r.signing_scheme, SigningScheme::Eip712);
    }

    #[test]
    fn signing_result_scheme_checks() {
        assert!(SigningResult::new("0x", SigningScheme::Eip712).is_eip712());
        assert!(SigningResult::new("0x", SigningScheme::EthSign).is_eth_sign());
        assert!(SigningResult::new("0x", SigningScheme::Eip1271).is_eip1271());
        assert!(SigningResult::new("0x", SigningScheme::PreSign).is_presign());
    }

    #[test]
    fn signing_result_scheme_exclusivity() {
        let r = SigningResult::new("0x", SigningScheme::Eip712);
        assert!(r.is_eip712());
        assert!(!r.is_eth_sign());
        assert!(!r.is_eip1271());
        assert!(!r.is_presign());
    }

    #[test]
    fn signing_result_len_and_ref() {
        let r = SigningResult::new("0xdeadbeef", SigningScheme::Eip712);
        assert_eq!(r.signature_len(), 10);
        assert_eq!(r.signature_ref(), "0xdeadbeef");
    }

    // ── SignOrderParams ─────────────────────────────────────────────────

    #[test]
    fn sign_order_params_new() {
        let p = SignOrderParams::new(1, default_order(), EcdsaSigningScheme::Eip712);
        assert_eq!(p.chain_id, 1);
        assert_eq!(p.signing_scheme, EcdsaSigningScheme::Eip712);
    }

    // ── SignOrderCancellationParams ─────────────────────────────────────

    #[test]
    fn sign_order_cancellation_params_new() {
        let p = SignOrderCancellationParams::new(1, "0xabc123", EcdsaSigningScheme::EthSign);
        assert_eq!(p.chain_id, 1);
        assert_eq!(p.order_uid, "0xabc123");
        assert_eq!(p.signing_scheme, EcdsaSigningScheme::EthSign);
    }

    // ── SignOrderCancellationsParams ────────────────────────────────────

    #[test]
    fn sign_order_cancellations_params_count() {
        let p = SignOrderCancellationsParams::new(
            1,
            vec!["a".into(), "b".into(), "c".into()],
            EcdsaSigningScheme::Eip712,
        );
        assert_eq!(p.count(), 3);
    }

    #[test]
    fn sign_order_cancellations_params_empty() {
        let p = SignOrderCancellationsParams::new(1, vec![], EcdsaSigningScheme::Eip712);
        assert_eq!(p.count(), 0);
    }

    // ── Display impls ───────────────────────────────────────────────────

    #[test]
    fn unsigned_order_display() {
        let o = default_order();
        let s = o.to_string();
        assert!(s.contains("sell"), "expected 'sell' in: {s}");
    }

    #[test]
    fn order_domain_display() {
        let d = OrderDomain::for_chain(42);
        let s = d.to_string();
        assert!(s.contains("chain=42"), "expected chain=42 in: {s}");
    }

    #[test]
    fn order_typed_data_display() {
        let td = OrderTypedData::new(OrderDomain::for_chain(1), default_order());
        let s = td.to_string();
        assert!(s.contains("typed-data"), "expected typed-data in: {s}");
        assert!(s.contains("chain=1"), "expected chain=1 in: {s}");
    }

    #[test]
    fn signing_result_display_truncates() {
        let r = SigningResult::new("0xdeadbeefcafe1234567890", SigningScheme::Eip712);
        let s = r.to_string();
        assert!(s.contains("sig("), "expected sig( in: {s}");
        assert!(s.contains("0xdeadbee"), "expected truncated sig in: {s}");
    }

    #[test]
    fn signing_result_display_short_sig() {
        let r = SigningResult::new("0xab", SigningScheme::PreSign);
        let s = r.to_string();
        assert!(s.contains("0xab"), "expected full short sig in: {s}");
    }

    #[test]
    fn sign_order_params_display() {
        let p = SignOrderParams::new(100, default_order(), EcdsaSigningScheme::Eip712);
        let s = p.to_string();
        assert!(s.contains("sign-order"), "expected sign-order in: {s}");
        assert!(s.contains("chain=100"), "expected chain=100 in: {s}");
    }

    #[test]
    fn sign_order_cancellation_params_display() {
        let p = SignOrderCancellationParams::new(1, "0x1234567890ab", EcdsaSigningScheme::Eip712);
        let s = p.to_string();
        assert!(s.contains("cancel-sign"), "expected cancel-sign in: {s}");
        assert!(s.contains("chain=1"), "expected chain=1 in: {s}");
    }

    #[test]
    fn sign_order_cancellations_params_display() {
        let p = SignOrderCancellationsParams::new(
            5,
            vec!["a".into(), "b".into()],
            EcdsaSigningScheme::Eip712,
        );
        let s = p.to_string();
        assert!(s.contains("cancel-signs"), "expected cancel-signs in: {s}");
        assert!(s.contains("count=2"), "expected count=2 in: {s}");
    }
}
