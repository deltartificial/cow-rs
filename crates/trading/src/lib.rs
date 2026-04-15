//! `cow-trading` — Layer 5 high-level trading SDK for the `CoW` Protocol SDK.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod costs;
pub mod sdk;
pub mod slippage;
pub mod types;

pub use sdk::{
    DEFAULT_QUOTE_VALIDITY, DEFAULT_SLIPPAGE_BPS, ETH_FLOW_DEFAULT_SLIPPAGE_BPS, GAS_LIMIT_DEFAULT,
    QuoteResultsWithSigner, QuoterParameters, TradingSdk, TradingSdkConfig,
    adjust_eth_flow_limit_order_params, adjust_eth_flow_order_params, build_app_data,
    calculate_gas_margin, calculate_unique_order_id, generate_app_data_from_doc,
    get_default_slippage_bps, get_default_utm_params, get_eth_flow_cancellation,
    get_eth_flow_contract, get_is_eth_flow_order, get_order_deadline_from_now, get_order_to_sign,
    get_order_typed_data, get_quote_raw, get_quote_with_signer, get_settlement_cancellation,
    get_settlement_contract, get_slippage_percent, get_trade_parameters_after_quote, get_trader,
    post_cow_protocol_trade, post_cow_protocol_trade as post_co_w_protocol_trade,
    post_sell_native_currency_order, resolve_order_book_api, resolve_signer,
    resolve_slippage_suggestion, swap_params_to_limit_order_params, unsigned_order_for_signing,
};
pub use slippage::{
    DEFAULT_FEE_SLIPPAGE_FACTOR_PCT, DEFAULT_VOLUME_SLIPPAGE_BPS, MAX_SLIPPAGE_BPS,
    apply_percentage, bps_to_percentage, percentage_to_bps, suggest_slippage_bps,
    suggest_slippage_from_fee, suggest_slippage_from_volume,
};
pub use types::{
    Amounts, BuildAppDataParams, LimitOrderAdvancedSettings, LimitTradeParameters,
    LimitTradeParametersFromQuote, NetworkFee, OrderPostingResult, PartnerFeeCost,
    PostTradeAdditionalParams, ProtocolFeeCost, QuoteAmountsAndCosts, QuoteResults,
    SlippageToleranceRequest, SlippageToleranceResponse, SwapAdvancedSettings, TradeParameters,
    TradingAppDataInfo, TradingTransactionParams, apply_settings_to_limit_trade_parameters,
    map_quote_amounts_and_costs,
};
