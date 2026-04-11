//! EIP-712 order digest, signing utilities, and core signing types.

pub mod calldata;
pub mod eip712;
pub mod flags;
pub mod interaction;
pub mod trade;
pub mod types;
pub mod utils;

pub use calldata::{invalidate_order_calldata, set_pre_signature_calldata};
pub use eip712::{
    BUY_ETH_ADDRESS, EIP1271_MAGICVALUE, ORDER_PRIMARY_TYPE, ORDER_TYPE_HASH, ORDER_UID_LENGTH,
    OrderUidParams, PRE_SIGNED, build_order_typed_data, cancellations_hash, domain_separator,
    domain_separator_from, extract_order_uid_params, hash_order_cancellation,
    hash_order_cancellations, hash_typed_data, hashify, normalize_order, order_hash,
    order_hash as hash_order, pack_order_uid_params, signing_digest,
};
pub use flags::{
    OrderFlags, TradeFlags, decode_order_flags, decode_signing_scheme, decode_trade_flags,
    encode_order_flags, encode_signing_scheme, encode_trade_flags, normalize_buy_token_balance,
};
pub use interaction::{
    Interaction, InteractionLike, normalize_interaction, normalize_interactions,
};
pub use trade::{
    BatchSwapStep, Eip1271SignatureData, EncodedTrade, SettlementTokenRegistry, SignatureData,
    Swap, decode_eip1271_signature_data, decode_order, decode_signature_owner,
    encode_eip1271_signature_data, encode_signature_data, encode_swap_step, encode_trade,
};
pub use types::{
    OrderDomain, OrderTypedData, SignOrderCancellationParams, SignOrderCancellationsParams,
    SignOrderParams, SigningResult, UnsignedOrder,
};
pub use utils::{
    compute_order_uid, eip1271_result, generate_order_id, get_domain, presign_result, sign_order,
    sign_order_cancellation, sign_order_cancellations,
};
