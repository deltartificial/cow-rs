//! Trade and swap encoding for `CoW` Protocol settlements.
//!
//! Mirrors `encodeTrade`, `encodeSwapStep`, `decodeOrder`, and the
//! `TokenRegistry` class from the `TypeScript` `contracts-ts` package.

use alloy_primitives::{Address, B256, Bytes, U256};
use cow_errors::CowError;
use cow_types::SigningScheme;
use foldhash::HashMap;

use crate::{
    flags::{
        OrderFlags, TradeFlags, decode_order_flags, encode_trade_flags, normalize_buy_token_balance,
    },
    types::UnsignedOrder,
};

/// Encoded trade data as used in the settlement contract.
///
/// Corresponds to the `TypeScript` `Trade` type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedTrade {
    /// Index of the sell token in the settlement token array.
    pub sell_token_index: u64,
    /// Index of the buy token in the settlement token array.
    pub buy_token_index: u64,
    /// Address that receives the bought tokens.
    pub receiver: Address,
    /// Amount of sell token.
    pub sell_amount: U256,
    /// Amount of buy token.
    pub buy_amount: U256,
    /// Order expiry as Unix timestamp.
    pub valid_to: u32,
    /// App-data hash.
    pub app_data: B256,
    /// Fee amount.
    pub fee_amount: U256,
    /// Encoded trade flags.
    pub flags: u8,
    /// The executed trade amount.
    pub executed_amount: U256,
    /// Signature data.
    pub signature: Bytes,
}

/// An encoded Balancer batch swap step.
///
/// Corresponds to the `TypeScript` `BatchSwapStep` type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchSwapStep {
    /// The Balancer pool ID.
    pub pool_id: B256,
    /// Index of the input token in the token array.
    pub asset_in_index: u64,
    /// Index of the output token in the token array.
    pub asset_out_index: u64,
    /// The amount to swap.
    pub amount: U256,
    /// Additional pool user data.
    pub user_data: Bytes,
}

/// A Balancer swap used for settling an order against Balancer pools.
///
/// Corresponds to the `TypeScript` `Swap` type.
#[derive(Debug, Clone)]
pub struct Swap {
    /// The Balancer pool ID.
    pub pool_id: B256,
    /// The swap input token address.
    pub asset_in: Address,
    /// The swap output token address.
    pub asset_out: Address,
    /// The amount to swap.
    pub amount: U256,
    /// Optional additional pool user data.
    pub user_data: Option<Bytes>,
}

/// A registry for tracking token addresses by index in a settlement.
///
/// Mirrors the `TypeScript` `TokenRegistry` class. Tokens are indexed by their
/// checksummed address to ensure consistent lookups.
#[derive(Debug, Clone, Default)]
pub struct SettlementTokenRegistry {
    tokens: Vec<Address>,
    token_map: HashMap<Address, u64>,
}

impl SettlementTokenRegistry {
    /// Create a new empty token registry.
    ///
    /// # Returns
    ///
    /// An empty [`SettlementTokenRegistry`] with no tokens registered.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the list of token addresses in the registry.
    ///
    /// # Returns
    ///
    /// A slice of [`Address`] values in registration order.
    #[must_use]
    pub fn addresses(&self) -> &[Address] {
        &self.tokens
    }

    /// Get the index for a token, adding it to the registry if not yet seen.
    ///
    /// # Arguments
    ///
    /// * `token` — the token address to look up or register.
    ///
    /// # Returns
    ///
    /// The zero-based index of `token` in the registry. If the token was not
    /// previously registered, it is appended and its new index is returned.
    ///
    /// ```
    /// use alloy_primitives::address;
    /// use cow_signing::trade::SettlementTokenRegistry;
    ///
    /// let mut registry = SettlementTokenRegistry::new();
    /// let token_a = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    /// let token_b = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    ///
    /// assert_eq!(registry.index(token_a), 0);
    /// assert_eq!(registry.index(token_b), 1);
    /// assert_eq!(registry.index(token_a), 0); // same token returns same index
    /// assert_eq!(registry.addresses().len(), 2);
    /// ```
    pub fn index(&mut self, token: Address) -> u64 {
        if let Some(&idx) = self.token_map.get(&token) {
            return idx;
        }
        let idx = self.tokens.len() as u64;
        self.tokens.push(token);
        self.token_map.insert(token, idx);
        idx
    }
}

/// Signature data that includes a signing scheme.
///
/// This is a simplified Rust version of the `TypeScript` `Signature` union type.
#[derive(Debug, Clone)]
pub struct SignatureData {
    /// The signing scheme.
    pub scheme: SigningScheme,
    /// The encoded signature bytes.
    pub data: Bytes,
}

/// Encode a trade for the settlement contract.
///
/// Mirrors `encodeTrade` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `tokens` — the settlement token registry (tokens are added as needed).
/// * `order` — the unsigned order to encode.
/// * `signature` — the signature data including scheme and encoded bytes.
/// * `executed_amount` — the amount already executed for this trade.
///
/// # Returns
///
/// An [`EncodedTrade`] ready for inclusion in a settlement transaction.
///
/// ```ignore
/// use alloy_primitives::{Address, B256, Bytes, U256, address};
/// use cow_rs::{
///     order_signing::{
///         trade::{EncodedTrade, SettlementTokenRegistry, SignatureData, encode_trade},
///         types::UnsignedOrder,
///     },
///     types::{OrderKind, SigningScheme, TokenBalance},
/// };
///
/// let mut tokens = SettlementTokenRegistry::new();
/// let sell = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
/// let buy = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
///
/// let order = UnsignedOrder {
///     sell_token: sell,
///     buy_token: buy,
///     receiver: Address::ZERO,
///     sell_amount: U256::from(1000),
///     buy_amount: U256::from(900),
///     valid_to: 1_000_000,
///     app_data: B256::ZERO,
///     fee_amount: U256::ZERO,
///     kind: OrderKind::Sell,
///     partially_fillable: false,
///     sell_token_balance: TokenBalance::Erc20,
///     buy_token_balance: TokenBalance::Erc20,
/// };
///
/// let signature =
///     SignatureData { scheme: SigningScheme::Eip712, data: Bytes::from(vec![0u8; 65]) };
///
/// let trade = encode_trade(&mut tokens, &order, &signature, U256::ZERO);
/// assert_eq!(trade.sell_token_index, 0);
/// assert_eq!(trade.buy_token_index, 1);
/// ```
#[must_use]
pub fn encode_trade(
    tokens: &mut SettlementTokenRegistry,
    order: &UnsignedOrder,
    signature: &SignatureData,
    executed_amount: U256,
) -> EncodedTrade {
    let trade_flags = TradeFlags {
        order_flags: OrderFlags {
            kind: order.kind,
            partially_fillable: order.partially_fillable,
            sell_token_balance: order.sell_token_balance,
            buy_token_balance: normalize_buy_token_balance(order.buy_token_balance),
        },
        signing_scheme: signature.scheme,
    };

    EncodedTrade {
        sell_token_index: tokens.index(order.sell_token),
        buy_token_index: tokens.index(order.buy_token),
        receiver: order.receiver,
        sell_amount: order.sell_amount,
        buy_amount: order.buy_amount,
        valid_to: order.valid_to,
        app_data: order.app_data,
        fee_amount: order.fee_amount,
        flags: encode_trade_flags(&trade_flags),
        executed_amount,
        signature: signature.data.clone(),
    }
}

/// Decode an order from a settlement trade and token list.
///
/// Mirrors `decodeOrder` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `trade` — the encoded trade to decode.
/// * `tokens` — the token address list used to resolve token indices.
///
/// # Returns
///
/// The reconstructed [`UnsignedOrder`].
///
/// # Errors
///
/// Returns [`CowError::Parse`] if token indices are out of bounds or flags are
/// invalid.
///
/// ```ignore
/// use alloy_primitives::{Address, B256, Bytes, U256, address};
/// use cow_rs::{
///     order_signing::trade::{EncodedTrade, decode_order},
///     types::{OrderKind, TokenBalance},
/// };
///
/// let sell = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
/// let buy = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
///
/// let trade = EncodedTrade {
///     sell_token_index: 0,
///     buy_token_index: 1,
///     receiver: Address::ZERO,
///     sell_amount: U256::from(1000),
///     buy_amount: U256::from(900),
///     valid_to: 1_000_000,
///     app_data: B256::ZERO,
///     fee_amount: U256::ZERO,
///     flags: 0,
///     executed_amount: U256::ZERO,
///     signature: Bytes::new(),
/// };
///
/// let tokens = vec![sell, buy];
/// let order = decode_order(&trade, &tokens).unwrap();
/// assert_eq!(order.sell_token, sell);
/// assert_eq!(order.buy_token, buy);
/// assert_eq!(order.kind, OrderKind::Sell);
/// ```
pub fn decode_order(trade: &EncodedTrade, tokens: &[Address]) -> Result<UnsignedOrder, CowError> {
    let sell_index = trade.sell_token_index as usize;
    let buy_index = trade.buy_token_index as usize;

    if sell_index >= tokens.len() || buy_index >= tokens.len() {
        return Err(CowError::Parse {
            field: "trade",
            reason: format!(
                "token index out of bounds: sell={sell_index}, buy={buy_index}, tokens={}",
                tokens.len()
            ),
        });
    }

    let flags = decode_order_flags(trade.flags)?;

    Ok(UnsignedOrder {
        sell_token: tokens[sell_index],
        buy_token: tokens[buy_index],
        receiver: trade.receiver,
        sell_amount: trade.sell_amount,
        buy_amount: trade.buy_amount,
        valid_to: trade.valid_to,
        app_data: trade.app_data,
        fee_amount: trade.fee_amount,
        kind: flags.kind,
        partially_fillable: flags.partially_fillable,
        sell_token_balance: flags.sell_token_balance,
        buy_token_balance: flags.buy_token_balance,
    })
}

/// Encode a Balancer swap step for the settlement contract.
///
/// Mirrors `encodeSwapStep` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `tokens` — the settlement token registry (tokens are added as needed).
/// * `swap` — the swap to encode.
///
/// # Returns
///
/// A [`BatchSwapStep`] with token addresses resolved to registry indices.
///
/// ```
/// use alloy_primitives::{B256, U256, address};
/// use cow_signing::trade::{SettlementTokenRegistry, Swap, encode_swap_step};
///
/// let mut tokens = SettlementTokenRegistry::new();
/// let swap = Swap {
///     pool_id: B256::ZERO,
///     asset_in: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
///     asset_out: address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
///     amount: U256::from(1000),
///     user_data: None,
/// };
///
/// let step = encode_swap_step(&mut tokens, &swap);
/// assert_eq!(step.asset_in_index, 0);
/// assert_eq!(step.asset_out_index, 1);
/// ```
#[must_use]
pub fn encode_swap_step(tokens: &mut SettlementTokenRegistry, swap: &Swap) -> BatchSwapStep {
    BatchSwapStep {
        pool_id: swap.pool_id,
        asset_in_index: tokens.index(swap.asset_in),
        asset_out_index: tokens.index(swap.asset_out),
        amount: swap.amount,
        user_data: swap.user_data.clone().unwrap_or_default(),
    }
}

/// Encode signature data based on the signing scheme.
///
/// Mirrors `encodeSignatureData` from the `TypeScript` SDK.
///
/// - For ECDSA schemes (EIP-712, EIP-191): returns the raw signature bytes.
/// - For EIP-1271: returns the verifier address + signature (see
///   [`encode_eip1271_signature_data`]).
/// - For `PreSign`: returns the signer's address as 20 bytes.
///
/// # Arguments
///
/// * `signature` — the signature data to encode, including its scheme.
///
/// # Returns
///
/// The encoded signature as [`Bytes`].
#[must_use]
pub fn encode_signature_data(signature: &SignatureData) -> Bytes {
    signature.data.clone()
}

/// EIP-1271 signature data: a verifier address and the actual signature bytes.
///
/// Corresponds to the `TypeScript` `Eip1271SignatureData` type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Eip1271SignatureData {
    /// The verifying contract address.
    pub verifier: Address,
    /// The arbitrary signature bytes used for verification.
    pub signature: Bytes,
}

/// Encode EIP-1271 signature data as `abi.encodePacked(address, bytes)`.
///
/// The `CoW` Protocol settlement contract expects EIP-1271 signatures to be
/// encoded as the 20-byte verifier address followed by the arbitrary
/// signature bytes.
///
/// Mirrors `encodeEip1271SignatureData` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `data` — the EIP-1271 verifier address and signature bytes.
///
/// # Returns
///
/// The packed [`Bytes`] containing `verifier ++ signature`.
///
/// ```
/// use alloy_primitives::{Bytes, address};
/// use cow_signing::trade::{Eip1271SignatureData, encode_eip1271_signature_data};
///
/// let data = Eip1271SignatureData {
///     verifier: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
///     signature: Bytes::from(vec![0xde, 0xad, 0xbe, 0xef]),
/// };
/// let encoded = encode_eip1271_signature_data(&data);
/// assert_eq!(encoded.len(), 24); // 20 + 4
/// ```
#[must_use]
pub fn encode_eip1271_signature_data(data: &Eip1271SignatureData) -> Bytes {
    let mut buf = Vec::with_capacity(20 + data.signature.len());
    buf.extend_from_slice(data.verifier.as_slice());
    buf.extend_from_slice(&data.signature);
    Bytes::from(buf)
}

/// Extract the owner (verifier) address from an EIP-1271 packed signature.
///
/// The first 20 bytes of an EIP-1271 signature encode the verifying contract
/// address. This function returns that address without parsing the remaining
/// signature bytes.
///
/// Mirrors `decodeSignatureOwner` from the `TypeScript` `contracts-ts` package.
///
/// # Arguments
///
/// * `data` — the packed EIP-1271 signature bytes.
///
/// # Returns
///
/// The 20-byte verifier [`Address`] extracted from the start of `data`.
///
/// # Errors
///
/// Returns [`CowError::Parse`] if the input is less than 20 bytes.
///
/// ```
/// use alloy_primitives::{Bytes, address};
/// use cow_signing::trade::{
///     Eip1271SignatureData, decode_signature_owner, encode_eip1271_signature_data,
/// };
///
/// let data = Eip1271SignatureData {
///     verifier: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
///     signature: Bytes::from(vec![0xde, 0xad]),
/// };
/// let encoded = encode_eip1271_signature_data(&data);
/// let owner = decode_signature_owner(&encoded).unwrap();
/// assert_eq!(owner, address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
/// ```
pub fn decode_signature_owner(data: &[u8]) -> Result<Address, CowError> {
    if data.len() < 20 {
        return Err(CowError::Parse {
            field: "eip1271_signature",
            reason: format!("data too short: {} bytes, need at least 20", data.len()),
        });
    }
    Ok(Address::from_slice(&data[..20]))
}

/// Decode EIP-1271 signature data from the packed format.
///
/// The first 20 bytes are the verifier address, and the remaining bytes are
/// the signature data.
///
/// Mirrors `decodeEip1271SignatureData` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `data` — the packed EIP-1271 signature bytes to decode.
///
/// # Returns
///
/// An [`Eip1271SignatureData`] with the verifier address and signature bytes
/// separated.
///
/// # Errors
///
/// Returns [`CowError::Parse`] if the input is less than 20 bytes.
///
/// ```
/// use alloy_primitives::{Bytes, address};
/// use cow_signing::trade::{
///     Eip1271SignatureData, decode_eip1271_signature_data, encode_eip1271_signature_data,
/// };
///
/// let original = Eip1271SignatureData {
///     verifier: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
///     signature: Bytes::from(vec![0xde, 0xad]),
/// };
/// let encoded = encode_eip1271_signature_data(&original);
/// let decoded = decode_eip1271_signature_data(&encoded).unwrap();
/// assert_eq!(decoded, original);
/// ```
pub fn decode_eip1271_signature_data(data: &[u8]) -> Result<Eip1271SignatureData, CowError> {
    if data.len() < 20 {
        return Err(CowError::Parse {
            field: "eip1271_signature",
            reason: format!("data too short: {} bytes, need at least 20", data.len()),
        });
    }
    let verifier = Address::from_slice(&data[..20]);
    let signature = Bytes::from(data[20..].to_vec());
    Ok(Eip1271SignatureData { verifier, signature })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;
    use cow_types::{OrderKind, TokenBalance};

    #[test]
    fn token_registry_basic() {
        let mut reg = SettlementTokenRegistry::new();
        let a = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let b = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

        assert_eq!(reg.index(a), 0);
        assert_eq!(reg.index(b), 1);
        assert_eq!(reg.index(a), 0);
        assert_eq!(reg.addresses().len(), 2);
    }

    #[test]
    fn encode_decode_trade_roundtrip() {
        let sell = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let buy = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let mut tokens = SettlementTokenRegistry::new();

        let order = UnsignedOrder {
            sell_token: sell,
            buy_token: buy,
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
        };

        let signature =
            SignatureData { scheme: SigningScheme::Eip712, data: Bytes::from(vec![0u8; 65]) };

        let encoded = encode_trade(&mut tokens, &order, &signature, U256::ZERO);
        let decoded = decode_order(&encoded, tokens.addresses()).unwrap();

        assert_eq!(decoded.sell_token, sell);
        assert_eq!(decoded.buy_token, buy);
        assert_eq!(decoded.kind, OrderKind::Sell);
        assert_eq!(decoded.sell_amount, U256::from(1000));
    }

    #[test]
    fn decode_order_out_of_bounds() {
        let trade = EncodedTrade {
            sell_token_index: 5,
            buy_token_index: 0,
            receiver: Address::ZERO,
            sell_amount: U256::ZERO,
            buy_amount: U256::ZERO,
            valid_to: 0,
            app_data: B256::ZERO,
            fee_amount: U256::ZERO,
            flags: 0,
            executed_amount: U256::ZERO,
            signature: Bytes::new(),
        };
        let tokens = vec![Address::ZERO];
        assert!(decode_order(&trade, &tokens).is_err());
    }

    #[test]
    fn eip1271_roundtrip() {
        let data = Eip1271SignatureData {
            verifier: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            signature: Bytes::from(vec![1, 2, 3, 4, 5]),
        };
        let encoded = encode_eip1271_signature_data(&data);
        let decoded = decode_eip1271_signature_data(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn eip1271_too_short() {
        assert!(decode_eip1271_signature_data(&[0u8; 19]).is_err());
    }

    #[test]
    fn encode_swap_step_basic() {
        let mut tokens = SettlementTokenRegistry::new();
        let swap = Swap {
            pool_id: B256::ZERO,
            asset_in: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            asset_out: address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            amount: U256::from(500),
            user_data: None,
        };
        let step = encode_swap_step(&mut tokens, &swap);
        assert_eq!(step.asset_in_index, 0);
        assert_eq!(step.asset_out_index, 1);
        assert_eq!(step.amount, U256::from(500));
        assert!(step.user_data.is_empty());
    }

    #[test]
    fn encode_swap_step_with_user_data() {
        let mut tokens = SettlementTokenRegistry::new();
        let swap = Swap {
            pool_id: B256::ZERO,
            asset_in: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            asset_out: address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            amount: U256::from(500),
            user_data: Some(Bytes::from(vec![0xDE, 0xAD])),
        };
        let step = encode_swap_step(&mut tokens, &swap);
        assert_eq!(step.user_data, Bytes::from(vec![0xDE, 0xAD]));
    }

    #[test]
    fn encode_signature_data_returns_clone() {
        let sig = SignatureData { scheme: SigningScheme::Eip712, data: Bytes::from(vec![1, 2, 3]) };
        let encoded = encode_signature_data(&sig);
        assert_eq!(encoded, Bytes::from(vec![1, 2, 3]));
    }

    #[test]
    fn decode_signature_owner_too_short() {
        assert!(decode_signature_owner(&[0u8; 19]).is_err());
    }

    #[test]
    fn decode_signature_owner_exact_20_bytes() {
        let mut data = [0u8; 20];
        data[19] = 0x42;
        let owner = decode_signature_owner(&data).unwrap();
        assert_eq!(owner.as_slice()[19], 0x42);
    }

    #[test]
    fn decode_eip1271_exact_20_bytes_empty_sig() {
        let data = [0u8; 20];
        let result = decode_eip1271_signature_data(&data).unwrap();
        assert_eq!(result.verifier, Address::ZERO);
        assert!(result.signature.is_empty());
    }

    #[test]
    fn encode_decode_trade_partially_fillable() {
        let sell = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let buy = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let mut tokens = SettlementTokenRegistry::new();

        let order = UnsignedOrder {
            sell_token: sell,
            buy_token: buy,
            receiver: Address::ZERO,
            sell_amount: U256::from(1000),
            buy_amount: U256::from(900),
            valid_to: 1_000_000,
            app_data: B256::ZERO,
            fee_amount: U256::ZERO,
            kind: OrderKind::Buy,
            partially_fillable: true,
            sell_token_balance: TokenBalance::Erc20,
            buy_token_balance: TokenBalance::Erc20,
        };

        let signature =
            SignatureData { scheme: SigningScheme::Eip712, data: Bytes::from(vec![0u8; 65]) };

        let encoded = encode_trade(&mut tokens, &order, &signature, U256::from(100u64));
        assert_eq!(encoded.executed_amount, U256::from(100u64));
        let decoded = decode_order(&encoded, tokens.addresses()).unwrap();
        assert_eq!(decoded.kind, OrderKind::Buy);
        assert!(decoded.partially_fillable);
    }
}
