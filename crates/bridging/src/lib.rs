//! `cow-bridging` — Layer 5 cross-chain bridge aggregator for the `CoW` Protocol SDK.
//!
//! Provides:
//! - A [`BridgingSdk`] that queries multiple bridge providers concurrently
//! - Provider implementations for [Bungee](bungee) and [Across](across)
//! - Utility functions for fee math, token selection, and hook handling
//! - Comprehensive types for quotes, statuses, and deposit parameters

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod across;
pub mod bungee;
pub mod provider;
pub mod sdk;
pub mod types;
pub mod utils;

pub use across::{
    ACROSS_API_BASE, ACROSS_DEPOSIT_EVENT_INTERFACE, ACROSS_FUNDS_DEPOSITED_TOPIC,
    ACROSS_HOOK_DAPP_ID as ACROSS_PROVIDER_HOOK_DAPP_ID, AcrossBridgeProvider,
    AcrossBridgeProviderOptions, AcrossDepositCallParams, COW_TRADE_EVENT_INTERFACE,
    COW_TRADE_EVENT_SIGNATURE, CowTradeEvent, EvmLogEntry, across_math_contract_addresses,
    across_spoke_pool_addresses, across_supported_chains, across_token_mapping,
    create_across_deposit_call, default_across_info, get_across_deposit_events, get_chain_configs,
    get_cow_trade_events, get_deposit_params, get_token_address, get_token_by_address_and_chain_id,
    get_token_symbol, is_valid_across_status_response, map_across_status_to_bridge_status,
    to_bridge_quote_result,
};

pub use bungee::{
    BungeeDepositCallParams, BungeeProvider, bungee_to_bridge_quote_result,
    create_bungee_deposit_call, decode_amounts_bungee_tx_data, decode_bungee_bridge_tx_data,
    default_bungee_info, get_bridging_status_from_events, get_bungee_bridge_from_display_name,
    get_display_name_from_bungee_bridge, is_valid_bungee_events_response, is_valid_quote_response,
    resolve_api_endpoint_from_options,
};

pub use provider::{
    BridgeNetworkInfo, BridgeProvider, BridgeStatusFuture, BridgingParamsFuture,
    BridgingParamsResult, BuyTokensFuture, GasEstimationFuture, HookBridgeProvider,
    IntermediateTokensFuture, MaybeSendSync, NetworksFuture, QuoteFuture,
    ReceiverAccountBridgeProvider, ReceiverOverrideFuture, SignedHookFuture, UnsignedCallFuture,
    is_hook_bridge_provider, is_receiver_account_bridge_provider,
};

pub use sdk::{
    ACROSS_API_URL, ACROSS_HOOK_DAPP_ID, BUNGEE_API_FALLBACK_TIMEOUT, BUNGEE_API_PATH,
    BUNGEE_API_URL, BUNGEE_BASE_URL, BUNGEE_EVENTS_API_URL, BUNGEE_HOOK_DAPP_ID,
    BUNGEE_MANUAL_API_PATH, BUNGEE_MANUAL_API_URL, BridgeQuoteAndPost, BridgingSdk,
    CrossChainQuoteAndPost, DEFAULT_BRIDGE_SLIPPAGE_BPS, DEFAULT_EXTRA_GAS_FOR_HOOK_ESTIMATION,
    DEFAULT_EXTRA_GAS_PROXY_CREATION, DEFAULT_GAS_COST_FOR_HOOK_ESTIMATION,
    DEFAULT_PROVIDER_TIMEOUT_MS, DEFAULT_TOTAL_TIMEOUT_MS, GetCrossChainOrderParams,
    GetQuoteWithBridgeParams, HOOK_DAPP_BRIDGE_PROVIDER_PREFIX, NEAR_INTENTS_HOOK_DAPP_ID,
    QuoteAndPost, QuoteStrategy, assert_is_bridge_quote_and_post, assert_is_quote_and_post,
    create_post_swap_order_from_quote, create_strategies, get_bridge_signed_hook, get_cache_key,
    get_cross_chain_order, get_intermediate_swap_result, get_quote_with_bridge,
    get_quote_with_hook_bridge, get_quote_with_receiver_account_bridge, get_quote_without_bridge,
    get_swap_quote, is_bridge_quote_and_post, is_quote_and_post, safe_call_best_quote_callback,
    safe_call_progressive_callback,
};

#[cfg(feature = "native")]
pub use sdk::{
    create_bridge_request_timeout,
    create_bridge_request_timeout as create_bridge_request_timeout_promise,
    execute_provider_quotes, fetch_multi_quote,
};

pub use types::{
    AcrossChainConfig, AcrossDepositEvent, AcrossDepositStatus, AcrossDepositStatusResponse,
    AcrossPctFee, AcrossSuggestedFeesLimits, AcrossSuggestedFeesResponse, BridgeAmounts,
    BridgeCallDetails, BridgeCosts, BridgeDeposit, BridgeError, BridgeFees, BridgeHook,
    BridgeLimits, BridgeProviderInfo, BridgeProviderType, BridgeQuoteAmountsAndCosts,
    BridgeQuoteResult, BridgeQuoteResults, BridgeStatus, BridgeStatusResult, BridgingDepositParams,
    BridgingFee, BungeeBridge, BungeeBridgeName, BungeeEvent, BungeeEventStatus,
    BungeeTxDataBytesIndex, BuyTokensParams, CrossChainOrder, DecodedBungeeAmounts,
    DecodedBungeeTxData, GetProviderBuyTokens, IntermediateTokenInfo, MultiQuoteResult,
    QuoteBridgeRequest, QuoteBridgeResponse,
};

pub use utils::{
    COW_SHED_PROXY_CREATION_GAS, QueryParam, adapt_token, adapt_tokens, apply_bps, apply_pct_fee,
    are_hooks_equal, calculate_deadline, calculate_fee_bps, determine_intermediate_token,
    fill_timeout_results, find_bridge_provider_dapp_id, find_bridge_provider_from_hook,
    get_gas_limit_estimation_for_hook, get_hook_mock_for_cost_estimation, get_post_hooks,
    hash_quote, hook_mock_for_cost_estimation, is_app_doc, is_better_error, is_better_quote,
    is_client_fetch_error, is_correlated_token, is_infrastructure_error,
    is_stablecoin_priority_token, object_to_search_params, pct_to_bps, priority_stablecoin_tokens,
    resolve_providers_to_query, validate_cross_chain_request,
};
