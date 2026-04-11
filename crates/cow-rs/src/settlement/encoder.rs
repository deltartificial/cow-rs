//! Settlement encoder for building `GPv2Settlement.settle()` calldata.
//!
//! The [`SettlementEncoder`] orchestrates assembling tokens, clearing prices,
//! trades, and pre/intra/post interactions into the ABI-encoded calldata
//! expected by the on-chain settlement contract.

use std::fmt;

use alloy_primitives::{Address, U256, keccak256};

use crate::order_signing::{
    trade::{EncodedTrade, SettlementTokenRegistry, SignatureData, encode_trade},
    types::UnsignedOrder,
};

/// The three interaction stages in a `CoW` Protocol settlement.
///
/// Interactions execute at different points during the settlement transaction:
/// - **Pre**: before any trades are executed (e.g., token approvals).
/// - **Intra**: between trade execution and balance verification.
/// - **Post**: after all balances are verified (e.g., surplus withdrawals).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractionStage {
    /// Interactions executed before any trades.
    Pre = 0,
    /// Interactions executed between trades and balance checks.
    Intra = 1,
    /// Interactions executed after balance verification.
    Post = 2,
}

impl fmt::Display for InteractionStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pre => write!(f, "Pre"),
            Self::Intra => write!(f, "Intra"),
            Self::Post => write!(f, "Post"),
        }
    }
}

/// An encoded interaction ready for inclusion in a settlement.
///
/// Contains the target contract address, ETH value to send, and the
/// ABI-encoded calldata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedInteraction {
    /// Target contract address.
    pub target: Address,
    /// ETH value to send with the call (in wei).
    pub value: U256,
    /// ABI-encoded calldata for the interaction.
    pub calldata: Vec<u8>,
}

/// Orchestrates building a complete `GPv2Settlement.settle()` calldata payload.
///
/// Collects tokens, clearing prices, trades, and interactions, then
/// ABI-encodes everything into the format expected by the settlement
/// contract's `settle` function.
///
/// # Example
///
/// ```
/// use alloy_primitives::{Address, B256, Bytes, U256, address};
/// use cow_rs::{
///     OrderKind, SigningScheme, TokenBalance,
///     order_signing::{trade::SignatureData, types::UnsignedOrder},
///     settlement::encoder::{InteractionStage, SettlementEncoder},
/// };
///
/// let mut encoder = SettlementEncoder::new();
///
/// let sell = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
/// let buy = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
///
/// let sell_idx = encoder.add_token(sell);
/// let buy_idx = encoder.add_token(buy);
///
/// encoder.set_clearing_price(sell_idx, U256::from(1000));
/// encoder.set_clearing_price(buy_idx, U256::from(900));
///
/// assert_eq!(encoder.token_count(), 2);
/// // Encoder with only tokens/prices but no trades is still "empty".
/// assert!(encoder.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct SettlementEncoder {
    /// Registry mapping token addresses to indices.
    tokens: SettlementTokenRegistry,
    /// Clearing prices indexed by token position.
    clearing_prices: Vec<U256>,
    /// Encoded trades to include in the settlement.
    trades: Vec<EncodedTrade>,
    /// Interactions for each stage: [Pre, Intra, Post].
    interactions: [Vec<EncodedInteraction>; 3],
}

impl Default for SettlementEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SettlementEncoder {
    /// Create a new empty settlement encoder.
    ///
    /// # Returns
    ///
    /// An empty [`SettlementEncoder`] with no tokens, trades, or interactions.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tokens: SettlementTokenRegistry::new(),
            clearing_prices: Vec::new(),
            trades: Vec::new(),
            interactions: [Vec::new(), Vec::new(), Vec::new()],
        }
    }

    /// Register a token in the settlement and return its index.
    ///
    /// If the token is already registered, its existing index is returned.
    /// New tokens also get a default clearing price of zero.
    ///
    /// # Arguments
    ///
    /// * `token` - The token [`Address`] to register.
    ///
    /// # Returns
    ///
    /// The zero-based index of the token in the settlement.
    pub fn add_token(&mut self, token: Address) -> usize {
        let idx = self.tokens.index(token) as usize;
        // Extend clearing prices if a new token was added.
        if self.clearing_prices.len() <= idx {
            self.clearing_prices.resize(idx + 1, U256::ZERO);
        }
        idx
    }

    /// Set the clearing price for a token at the given index.
    ///
    /// The clearing price array is extended with zeros if needed to
    /// accommodate the index.
    ///
    /// # Arguments
    ///
    /// * `token_index` - The zero-based index of the token.
    /// * `price` - The clearing price as [`U256`].
    pub fn set_clearing_price(&mut self, token_index: usize, price: U256) {
        if self.clearing_prices.len() <= token_index {
            self.clearing_prices.resize(token_index + 1, U256::ZERO);
        }
        self.clearing_prices[token_index] = price;
    }

    /// Encode and append a trade to the settlement.
    ///
    /// The order's sell and buy tokens are automatically registered in the
    /// token registry if not already present.
    ///
    /// # Arguments
    ///
    /// * `order` - The unsigned order to encode.
    /// * `signature` - The signature data (scheme + bytes).
    /// * `executed_amount` - The amount already executed for this trade.
    pub fn add_trade(
        &mut self,
        order: &UnsignedOrder,
        signature: &SignatureData,
        executed_amount: U256,
    ) {
        let trade = encode_trade(&mut self.tokens, order, signature, executed_amount);
        // Ensure clearing prices vector covers newly registered tokens.
        let token_count = self.tokens.addresses().len();
        if self.clearing_prices.len() < token_count {
            self.clearing_prices.resize(token_count, U256::ZERO);
        }
        self.trades.push(trade);
    }

    /// Add an interaction to the specified stage.
    ///
    /// # Arguments
    ///
    /// * `stage` - When the interaction should execute ([`InteractionStage`]).
    /// * `target` - The contract [`Address`] to call.
    /// * `value` - ETH value to send with the call (in wei).
    /// * `calldata` - ABI-encoded calldata for the interaction.
    pub fn add_interaction(
        &mut self,
        stage: InteractionStage,
        target: Address,
        value: U256,
        calldata: Vec<u8>,
    ) {
        self.interactions[stage as usize].push(EncodedInteraction { target, value, calldata });
    }

    /// ABI-encode the full `settle(tokens[], clearingPrices[], trades[], interactions[][])`
    /// calldata.
    ///
    /// Produces the calldata for the `GPv2Settlement.settle()` function,
    /// including the 4-byte function selector.
    ///
    /// # Returns
    ///
    /// The complete ABI-encoded calldata as a `Vec<u8>`.
    #[must_use]
    pub fn encode_settlement(&self) -> Vec<u8> {
        let selector = &keccak256(
            b"settle(address[],uint256[],(uint256,uint256,address,uint256,uint256,uint32,bytes32,uint256,uint256,uint256,bytes)[],(address,uint256,bytes)[][3])",
        )[..4];

        let tokens = self.tokens.addresses();
        let mut buf = Vec::with_capacity(4 + 256);
        buf.extend_from_slice(selector);

        // Dynamic ABI encoding: four parameters, all dynamic.
        // Head: 4 offsets (each 32 bytes) = 128 bytes from start of params.
        let head_size: usize = 4 * 32;

        // Encode each dynamic section to calculate offsets.
        let tokens_enc = abi_encode_address_array(tokens);
        let prices_enc = abi_encode_u256_array(&self.clearing_prices);
        let trades_enc = self.abi_encode_trades();
        let interactions_enc = self.abi_encode_interactions();

        let offset_tokens = head_size;
        let offset_prices = offset_tokens + tokens_enc.len();
        let offset_trades = offset_prices + prices_enc.len();
        let offset_interactions = offset_trades + trades_enc.len();

        // Write head (offsets).
        buf.extend_from_slice(&abi_u256(U256::from(offset_tokens)));
        buf.extend_from_slice(&abi_u256(U256::from(offset_prices)));
        buf.extend_from_slice(&abi_u256(U256::from(offset_trades)));
        buf.extend_from_slice(&abi_u256(U256::from(offset_interactions)));

        // Write tail (data).
        buf.extend_from_slice(&tokens_enc);
        buf.extend_from_slice(&prices_enc);
        buf.extend_from_slice(&trades_enc);
        buf.extend_from_slice(&interactions_enc);

        buf
    }

    /// Return the number of registered tokens.
    ///
    /// # Returns
    ///
    /// The count of unique tokens in the settlement.
    #[must_use]
    pub fn token_count(&self) -> usize {
        self.tokens.addresses().len()
    }

    /// Return the number of trades in the settlement.
    ///
    /// # Returns
    ///
    /// The count of encoded trades.
    #[must_use]
    pub const fn trade_count(&self) -> usize {
        self.trades.len()
    }

    /// Return the number of interactions for a given stage.
    ///
    /// # Arguments
    ///
    /// * `stage` - The interaction stage to count.
    ///
    /// # Returns
    ///
    /// The count of interactions in the specified stage.
    #[must_use]
    pub const fn interaction_count(&self, stage: InteractionStage) -> usize {
        self.interactions[stage as usize].len()
    }

    /// Check whether the encoder contains any trades or interactions.
    ///
    /// # Returns
    ///
    /// `true` if there are no trades and no interactions in any stage.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.trades.is_empty() && self.interactions.iter().all(Vec::is_empty)
    }

    /// Reset the encoder, removing all tokens, prices, trades, and interactions.
    pub fn clear(&mut self) {
        self.tokens = SettlementTokenRegistry::new();
        self.clearing_prices.clear();
        self.trades.clear();
        for stage in &mut self.interactions {
            stage.clear();
        }
    }

    /// ABI-encode the trades array as a dynamic array of tuples.
    fn abi_encode_trades(&self) -> Vec<u8> {
        // Array of dynamic tuples: length + offsets + data.
        let count = self.trades.len();
        let mut buf = Vec::new();

        // Array length.
        buf.extend_from_slice(&abi_u256(U256::from(count)));

        if count == 0 {
            return buf;
        }

        // Each trade is a dynamic tuple (contains `bytes signature`).
        // First write offsets, then data.
        let mut encoded_trades: Vec<Vec<u8>> = Vec::with_capacity(count);
        for trade in &self.trades {
            encoded_trades.push(abi_encode_trade(trade));
        }

        // Offsets are relative to the start of the offsets block.
        let offsets_size = count * 32;
        let mut cumulative = offsets_size;
        for enc in &encoded_trades {
            buf.extend_from_slice(&abi_u256(U256::from(cumulative)));
            cumulative += enc.len();
        }

        // Actual trade data.
        for enc in encoded_trades {
            buf.extend_from_slice(&enc);
        }

        buf
    }

    /// ABI-encode the three interaction arrays.
    fn abi_encode_interactions(&self) -> Vec<u8> {
        // Fixed-size array of 3 dynamic arrays.
        // Head: 3 offsets. Tail: 3 encoded arrays.
        let mut buf = Vec::new();

        let enc0 = abi_encode_interaction_array(&self.interactions[0]);
        let enc1 = abi_encode_interaction_array(&self.interactions[1]);
        let enc2 = abi_encode_interaction_array(&self.interactions[2]);

        let head_size = 3 * 32;
        let offset0 = head_size;
        let offset1 = offset0 + enc0.len();
        let offset2 = offset1 + enc1.len();

        buf.extend_from_slice(&abi_u256(U256::from(offset0)));
        buf.extend_from_slice(&abi_u256(U256::from(offset1)));
        buf.extend_from_slice(&abi_u256(U256::from(offset2)));

        buf.extend_from_slice(&enc0);
        buf.extend_from_slice(&enc1);
        buf.extend_from_slice(&enc2);

        buf
    }
}

// ── ABI encoding helpers (private) ─────────────────────────────────────────

/// Encode a [`U256`] as a 32-byte big-endian ABI word.
const fn abi_u256(v: U256) -> [u8; 32] {
    v.to_be_bytes()
}

/// Left-pad an [`Address`] to a 32-byte ABI word.
fn abi_address(a: Address) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[12..].copy_from_slice(a.as_slice());
    buf
}

/// ABI-encode a dynamic `address[]` array.
fn abi_encode_address_array(addrs: &[Address]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(32 + addrs.len() * 32);
    buf.extend_from_slice(&abi_u256(U256::from(addrs.len())));
    for addr in addrs {
        buf.extend_from_slice(&abi_address(*addr));
    }
    buf
}

/// ABI-encode a dynamic `uint256[]` array.
fn abi_encode_u256_array(values: &[U256]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(32 + values.len() * 32);
    buf.extend_from_slice(&abi_u256(U256::from(values.len())));
    for v in values {
        buf.extend_from_slice(&abi_u256(*v));
    }
    buf
}

/// ABI-encode a single trade tuple (dynamic due to `bytes signature`).
fn abi_encode_trade(trade: &EncodedTrade) -> Vec<u8> {
    // 10 fixed fields (uint256, uint256, address, uint256, uint256, uint32,
    // bytes32, uint256, uint256, uint256) + 1 dynamic (bytes).
    let mut buf = Vec::with_capacity(11 * 32 + 32 + trade.signature.len());

    // sell_token_index
    buf.extend_from_slice(&abi_u256(U256::from(trade.sell_token_index)));
    // buy_token_index
    buf.extend_from_slice(&abi_u256(U256::from(trade.buy_token_index)));
    // receiver
    buf.extend_from_slice(&abi_address(trade.receiver));
    // sell_amount
    buf.extend_from_slice(&abi_u256(trade.sell_amount));
    // buy_amount
    buf.extend_from_slice(&abi_u256(trade.buy_amount));
    // valid_to (uint32, left-padded to 32 bytes)
    buf.extend_from_slice(&abi_u256(U256::from(trade.valid_to)));
    // app_data (bytes32)
    buf.extend_from_slice(trade.app_data.as_slice());
    // fee_amount
    buf.extend_from_slice(&abi_u256(trade.fee_amount));
    // flags (uint256)
    buf.extend_from_slice(&abi_u256(U256::from(trade.flags)));
    // executed_amount
    buf.extend_from_slice(&abi_u256(trade.executed_amount));
    // signature offset (always 11 * 32 = 352 from tuple start)
    buf.extend_from_slice(&abi_u256(U256::from(11u64 * 32)));
    // signature: length + padded data
    buf.extend_from_slice(&abi_u256(U256::from(trade.signature.len())));
    buf.extend_from_slice(&trade.signature);
    // Pad to 32-byte boundary.
    let padding = (32 - (trade.signature.len() % 32)) % 32;
    buf.extend_from_slice(&vec![0u8; padding]);

    buf
}

/// ABI-encode a dynamic array of interaction tuples.
fn abi_encode_interaction_array(interactions: &[EncodedInteraction]) -> Vec<u8> {
    let count = interactions.len();
    let mut buf = Vec::new();

    buf.extend_from_slice(&abi_u256(U256::from(count)));

    if count == 0 {
        return buf;
    }

    // Each interaction is a dynamic tuple (address, uint256, bytes).
    let mut encoded: Vec<Vec<u8>> = Vec::with_capacity(count);
    for ix in interactions {
        let mut e = Vec::with_capacity(3 * 32 + 32 + ix.calldata.len());
        e.extend_from_slice(&abi_address(ix.target));
        e.extend_from_slice(&abi_u256(ix.value));
        // Offset to bytes data: 3 * 32 = 96 from tuple start.
        e.extend_from_slice(&abi_u256(U256::from(3u64 * 32)));
        // bytes: length + padded data.
        e.extend_from_slice(&abi_u256(U256::from(ix.calldata.len())));
        e.extend_from_slice(&ix.calldata);
        let padding = (32 - (ix.calldata.len() % 32)) % 32;
        e.extend_from_slice(&vec![0u8; padding]);
        encoded.push(e);
    }

    // Offsets then data.
    let offsets_size = count * 32;
    let mut cumulative = offsets_size;
    for enc in &encoded {
        buf.extend_from_slice(&abi_u256(U256::from(cumulative)));
        cumulative += enc.len();
    }
    for enc in encoded {
        buf.extend_from_slice(&enc);
    }

    buf
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{B256, Bytes, address};

    use super::*;
    use crate::{OrderKind, SigningScheme, TokenBalance};

    fn sample_order() -> UnsignedOrder {
        UnsignedOrder {
            sell_token: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            buy_token: address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            receiver: Address::ZERO,
            sell_amount: U256::from(1000),
            buy_amount: U256::from(900),
            valid_to: 1_000_000,
            app_data: B256::ZERO,
            fee_amount: U256::ZERO,
            kind: OrderKind::Sell,
            partially_fillable: false,
            sell_token_balance: TokenBalance::Erc20,
            buy_token_balance: TokenBalance::Erc20,
        }
    }

    fn sample_signature() -> SignatureData {
        SignatureData { scheme: SigningScheme::Eip712, data: Bytes::from(vec![0u8; 65]) }
    }

    #[test]
    fn new_encoder_is_empty() {
        let enc = SettlementEncoder::new();
        assert!(enc.is_empty());
        assert_eq!(enc.token_count(), 0);
        assert_eq!(enc.trade_count(), 0);
        assert_eq!(enc.interaction_count(InteractionStage::Pre), 0);
        assert_eq!(enc.interaction_count(InteractionStage::Intra), 0);
        assert_eq!(enc.interaction_count(InteractionStage::Post), 0);
    }

    #[test]
    fn default_encoder_is_empty() {
        let enc = SettlementEncoder::default();
        assert!(enc.is_empty());
    }

    #[test]
    fn add_token_registers_and_returns_index() {
        let mut enc = SettlementEncoder::new();
        let a = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let b = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

        assert_eq!(enc.add_token(a), 0);
        assert_eq!(enc.add_token(b), 1);
        assert_eq!(enc.add_token(a), 0);
        assert_eq!(enc.token_count(), 2);
    }

    #[test]
    fn set_clearing_price_extends_vec() {
        let mut enc = SettlementEncoder::new();
        enc.set_clearing_price(3, U256::from(42));
        assert_eq!(enc.clearing_prices.len(), 4);
        assert_eq!(enc.clearing_prices[3], U256::from(42));
        assert_eq!(enc.clearing_prices[0], U256::ZERO);
    }

    #[test]
    fn add_trade_registers_tokens() {
        let mut enc = SettlementEncoder::new();
        enc.add_trade(&sample_order(), &sample_signature(), U256::ZERO);
        assert_eq!(enc.token_count(), 2);
        assert_eq!(enc.trade_count(), 1);
        assert!(!enc.is_empty());
    }

    #[test]
    fn add_interaction_counts() {
        let mut enc = SettlementEncoder::new();
        enc.add_interaction(InteractionStage::Pre, Address::ZERO, U256::ZERO, vec![0xab]);
        enc.add_interaction(InteractionStage::Post, Address::ZERO, U256::ZERO, vec![]);
        enc.add_interaction(InteractionStage::Post, Address::ZERO, U256::from(1), vec![0xcd]);

        assert_eq!(enc.interaction_count(InteractionStage::Pre), 1);
        assert_eq!(enc.interaction_count(InteractionStage::Intra), 0);
        assert_eq!(enc.interaction_count(InteractionStage::Post), 2);
        assert!(!enc.is_empty());
    }

    #[test]
    fn clear_resets_everything() {
        let mut enc = SettlementEncoder::new();
        enc.add_trade(&sample_order(), &sample_signature(), U256::ZERO);
        enc.add_interaction(InteractionStage::Pre, Address::ZERO, U256::ZERO, vec![]);
        enc.set_clearing_price(0, U256::from(1000));

        enc.clear();
        assert!(enc.is_empty());
        assert_eq!(enc.token_count(), 0);
        assert_eq!(enc.trade_count(), 0);
        assert_eq!(enc.interaction_count(InteractionStage::Pre), 0);
    }

    #[test]
    fn encode_settlement_starts_with_selector() {
        let enc = SettlementEncoder::new();
        let calldata = enc.encode_settlement();

        let expected_selector = &keccak256(
            b"settle(address[],uint256[],(uint256,uint256,address,uint256,uint256,uint32,bytes32,uint256,uint256,uint256,bytes)[],(address,uint256,bytes)[][3])",
        )[..4];

        assert_eq!(&calldata[..4], expected_selector);
    }

    #[test]
    fn encode_settlement_empty_is_valid() {
        let enc = SettlementEncoder::new();
        let calldata = enc.encode_settlement();
        // 4 (selector) + 4*32 (head offsets) + at least 32*4 (array lengths) = minimum size
        assert!(calldata.len() >= 4 + 4 * 32);
    }

    #[test]
    fn encode_settlement_with_trade() {
        let mut enc = SettlementEncoder::new();
        let order = sample_order();
        let sig = sample_signature();

        let sell_idx = enc.add_token(order.sell_token);
        let buy_idx = enc.add_token(order.buy_token);
        enc.set_clearing_price(sell_idx, U256::from(1000));
        enc.set_clearing_price(buy_idx, U256::from(900));
        enc.add_trade(&order, &sig, U256::ZERO);

        let calldata = enc.encode_settlement();
        // Must be non-trivial size with a trade included.
        assert!(calldata.len() > 4 + 4 * 32 + 2 * 32);
    }

    #[test]
    fn encode_settlement_with_interactions() {
        let mut enc = SettlementEncoder::new();
        enc.add_interaction(
            InteractionStage::Pre,
            address!("cccccccccccccccccccccccccccccccccccccccc"),
            U256::ZERO,
            vec![0xde, 0xad, 0xbe, 0xef],
        );
        enc.add_interaction(InteractionStage::Post, Address::ZERO, U256::from(1), vec![]);

        let calldata = enc.encode_settlement();
        assert!(calldata.len() > 4 + 4 * 32);
    }

    #[test]
    fn interaction_stage_display() {
        assert_eq!(format!("{}", InteractionStage::Pre), "Pre");
        assert_eq!(format!("{}", InteractionStage::Intra), "Intra");
        assert_eq!(format!("{}", InteractionStage::Post), "Post");
    }

    #[test]
    fn interaction_stage_clone_eq() {
        let a = InteractionStage::Intra;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(InteractionStage::Pre, InteractionStage::Post);
    }

    #[test]
    fn encoded_interaction_clone_eq() {
        let ix = EncodedInteraction {
            target: Address::ZERO,
            value: U256::from(42),
            calldata: vec![0xab, 0xcd],
        };
        let ix2 = ix.clone();
        assert_eq!(ix, ix2);
    }

    #[test]
    fn is_empty_with_only_interactions() {
        let mut enc = SettlementEncoder::new();
        assert!(enc.is_empty());
        enc.add_interaction(InteractionStage::Intra, Address::ZERO, U256::ZERO, vec![]);
        assert!(!enc.is_empty());
    }

    #[test]
    fn multiple_trades_encode() {
        let mut enc = SettlementEncoder::new();
        let order = sample_order();
        let sig = sample_signature();

        enc.add_trade(&order, &sig, U256::ZERO);
        enc.add_trade(&order, &sig, U256::from(100));
        assert_eq!(enc.trade_count(), 2);

        let calldata = enc.encode_settlement();
        assert!(calldata.len() > 4 + 4 * 32);
    }

    #[test]
    fn add_token_initializes_clearing_price() {
        let mut enc = SettlementEncoder::new();
        let idx = enc.add_token(address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert_eq!(enc.clearing_prices[idx], U256::ZERO);
    }
}
