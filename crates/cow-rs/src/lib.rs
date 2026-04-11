//! `cow-rs` — Rust SDK for the `CoW` Protocol.
//!
//! Organised into sub-modules mirroring the `TypeScript` SDK packages:
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`config`] | Chain IDs, contract addresses, token constants |
//! | [`order_book`] | Orderbook HTTP client and API types |
//! | [`order_signing`] | `EIP-712` digest and ECDSA signing |
//! | [`trading`] | High-level [`TradingSdk`] and fee-breakdown types |
//! | [`app_data`] | Order metadata schema and `keccak256` hashing |
//! | [`subgraph`] | Historical trading data via `GraphQL` |
//! | [`composable`] | Conditional orders (`TWAP`) and Merkle multiplexer |
//! | [`onchain`] | On-chain reading via JSON-RPC `eth_call` |
//!
//! # Quick start — `TradingSdk`
//!
//! ```rust,no_run
//! use alloy_primitives::U256;
//! use cow_rs::{OrderKind, SupportedChainId, TradeParameters, TradingSdk, TradingSdkConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let sdk = TradingSdk::new(
//!     TradingSdkConfig::prod(SupportedChainId::Sepolia, "MyApp"),
//!     "0xdeadbeef...",
//! )?;
//! let result = sdk
//!     .post_swap_order(TradeParameters {
//!         kind: OrderKind::Sell,
//!         sell_token: "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14".parse()?,
//!         sell_token_decimals: 18,
//!         buy_token: "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238".parse()?,
//!         buy_token_decimals: 6,
//!         amount: U256::from(100_000_000_000_000_u64),
//!         slippage_bps: Some(50),
//!         receiver: None,
//!         valid_for: None,
//!         valid_to: None,
//!         partially_fillable: None,
//!         partner_fee: None,
//!     })
//!     .await?;
//! println!("order: {}", result.order_id);
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod app_data;
pub mod bridging;
pub mod common;
pub mod composable;
pub mod config;
pub mod cow_shed;
pub mod erc20;
pub mod error;
pub mod ethflow;
pub mod flash_loans;
pub mod onchain;
pub mod order_book;
pub mod order_signing;
pub mod permit;
pub mod settlement;
pub mod subgraph;
pub mod trading;
pub mod traits;
pub mod types;
pub mod weiroll;

#[cfg(feature = "wasm")]
#[allow(unsafe_code, reason = "wasm-bindgen macro generates unsafe glue code")]
pub mod browser_wallet;

#[cfg(feature = "wasm")]
#[allow(unsafe_code, reason = "wasm-bindgen macro generates unsafe glue code")]
pub mod wasm;

// ── Convenience re-exports ────────────────────────────────────────────────────

pub use app_data::{
    AppDataDoc, AppDataInfo, CidComponents, CowHook, DEFAULT_IPFS_READ_URI, DEFAULT_IPFS_WRITE_URI,
    Ipfs, IpfsUploadResult, LATEST_APP_DATA_VERSION, Metadata, MetadataApi, OrderClassKind,
    OrderInteractionHooks, PartnerFee, PartnerFeeEntry, Quote, Referrer, ReplacedOrder, Utm,
    ValidationError, ValidationResult, Widget, appdata_hex, appdata_hex_to_cid, appdata_json,
    assert_cid, build_app_data_doc, build_app_data_doc_full, build_order_app_data,
    cid_to_appdata_hex, decode_cid, extract_digest, fetch_doc_from_app_data_hex,
    fetch_doc_from_cid, get_app_data_info, get_app_data_schema, get_partner_fee_bps, import_schema,
    merge_app_data_doc, parse_cid, pin_json_in_pinata_ipfs, stringify_deterministic,
    upload_app_data_to_pinata, validate_app_data_doc,
};
#[allow(
    deprecated,
    reason = "re-exporting deprecated legacy functions for backwards compatibility"
)]
pub use app_data::{
    app_data_hex_to_cid_legacy, fetch_doc_from_app_data_hex_legacy, get_app_data_info_legacy,
    upload_metadata_doc_to_ipfs_legacy,
};
pub use bridging::{
    ACROSS_DEPOSIT_EVENT_INTERFACE, BridgeError, BridgeProvider, BridgingSdk,
    COW_TRADE_EVENT_INTERFACE, QuoteBridgeRequest, QuoteBridgeResponse, QuoteStrategy,
    create_strategies, get_cache_key, safe_call_best_quote_callback,
    safe_call_progressive_callback,
};
pub use composable::{
    BlockInfo, COMPOSABLE_COW_ADDRESS, CURRENT_BLOCK_TIMESTAMP_FACTORY_ADDRESS,
    ConditionalOrderFactory, ConditionalOrderKind, ConditionalOrderParams, DurationOfPart,
    GAT_HANDLER_ADDRESS, GatData, GatOrder, GpV2OrderStruct, IsValidResult, MAX_FREQUENCY,
    Multiplexer, OrderProof, PollResult, ProofLocation, ProofStruct, ProofWithParams,
    STOP_LOSS_HANDLER_ADDRESS, StopLossData, StopLossOrder, TWAP_HANDLER_ADDRESS,
    TestConditionalOrderParams, TwapData, TwapOrder, TwapStartTime, TwapStruct, balance_to_string,
    create_calldata, create_set_domain_verifier_tx, create_test_conditional_order,
    create_with_context_calldata, data_to_struct, decode_gat_static_input, decode_params,
    decode_stop_loss_static_input, decode_twap_static_input, decode_twap_struct,
    default_token_formatter, encode_gat_struct, encode_params, encode_stop_loss_struct,
    encode_twap_struct, format_epoch, from_struct_to_order, get_block_info, get_domain_verifier,
    get_domain_verifier_calldata, get_is_valid_result, is_composable_cow,
    is_extensible_fallback_handler, is_valid_abi, kind_to_string, order_id, remove_calldata,
    set_root_calldata, set_root_with_context_calldata, struct_to_data, transform_data_to_struct,
    transform_struct_to_data,
};
pub use config::{
    AdditionalTargetChainId, AddressPerChain, ApiBaseUrls, ApiContext, BARN_ETH_FLOW,
    BTC_CURRENCY_ADDRESS, BUY_ETH_ADDRESS, COMPOSABLE_COW, ChainContract, ChainContracts,
    ChainInfo, ChainRpcUrls, ChainTokenInfo, CowSwapConfig, DEPLOYER_CONTRACT, ETH_FLOW_PROD,
    ETH_FLOW_STAGING, EVM_NATIVE_CURRENCY_ADDRESS, EXTENSIBLE_FALLBACK_HANDLER, Env, EvmCall,
    EvmChainInfo, EvmChains, IpfsConfig, MAX_VALID_TO_EPOCH, NATIVE_CURRENCY_ADDRESS,
    NonEvmChainInfo, NonEvmChains, ProtocolOptions, RAW_CHAINS_FILES_PATH, RAW_FILES_PATH, SALT,
    SETTLEMENT_CONTRACT, SETTLEMENT_CONTRACT_STAGING, SOL_NATIVE_CURRENCY_ADDRESS,
    SupportedChainId, TOKEN_LIST_IMAGES_PATH, TargetChainId, ThemedImage, TokenInfo, TokenRegistry,
    VAULT_RELAYER, VAULT_RELAYER_STAGING, WebUrl, additional_target_chain_info,
    all_additional_target_chain_ids, all_additional_target_chains, all_chain_ids, all_chains,
    all_supported_chain_ids, all_supported_chains, api_base_url, api_url, composable_cow,
    composable_cow_contract_address, cow_protocol_settlement_contract_address,
    cow_protocol_vault_relayer_address, cow_protocol_vault_relayer_address_staging,
    deterministic_deployment_address, eth_flow_for_env, extensible_fallback_handler,
    extensible_fallback_handler_contract_address, get_chain_info, get_wrapped_token_for_chain,
    implementation_address_slot, is_additional_target_chain, is_btc_chain, is_chain_deprecated,
    is_chain_under_development, is_evm_chain, is_evm_chain_info, is_non_evm_chain,
    is_non_evm_chain_info, is_supported_chain, is_target_chain_id, is_zk_sync_chain,
    map_address_to_supported_networks, map_all_networks, map_supported_networks,
    order_explorer_link, owner_address_slot, settlement_contract, settlement_contract_for_env,
    supported_chain_info, tradable_supported_chain_ids, tradable_supported_chains, vault_relayer,
    vault_relayer_for_env, wrapped_native_currency,
};
pub use cow_shed::{CowShedCall, CowShedHookParams, CowShedSdk};
pub use erc20::{
    build_eip2612_nonces_calldata, build_eip2612_version_calldata, build_erc20_allowance_calldata,
    build_erc20_approve_calldata, build_erc20_balance_of_calldata, build_erc20_decimals_calldata,
    build_erc20_name_calldata, build_erc20_transfer_calldata, build_erc20_transfer_from_calldata,
};
pub use error::CowError;
pub use ethflow::{
    EthFlowOrderData, EthFlowTransaction, build_eth_flow_transaction, encode_eth_flow_create_order,
    is_eth_flow_order_data,
};
pub use flash_loans::{FlashLoanParams, FlashLoanProvider, FlashLoanSdk};
pub use onchain::{OnchainReader, permit::OnchainTokenInfo};
// `implementation_address` and `owner_address` are methods on `OnchainReader`.
pub use order_book::{
    AppDataObject, Auction, CompetitionAuction, CompetitionOrderStatus, CompetitionOrderStatusKind,
    DEFAULT_RETRY_STATUS_CODES, EthflowData, GetOrdersRequest, GetTradesRequest, InteractionData,
    OnchainOrderData, Order, OrderBookApi, OrderCancellations, OrderClass, OrderCreation,
    OrderInteractions, OrderQuoteRequest, OrderQuoteResponse, OrderStatus, OrderUid,
    PartnerFeeResult, ProtocolFeeAmountParams, QuoteAmounts, QuoteAmountsAndCostsParams,
    QuoteAmountsAndCostsResult, QuoteCosts, QuoteData, QuoteFeeComponent, QuoteNetworkFee,
    QuoteOrderParams, QuoteSide, RateLimiter, RetryPolicy, SolverCompetition, SolverExecution,
    SolverSettlement, TotalSurplus, Trade, get_protocol_fee_amount,
    get_quote_amounts_after_partner_fee, get_quote_amounts_after_slippage,
    get_quote_amounts_and_costs, is_eth_flow_order, transform_order,
};
pub use order_signing::{
    EIP1271_MAGICVALUE, ORDER_PRIMARY_TYPE, ORDER_TYPE_HASH, ORDER_UID_LENGTH, OrderDomain,
    OrderFlags, OrderTypedData, OrderUidParams, PRE_SIGNED, SignOrderCancellationParams,
    SignOrderCancellationsParams, SignOrderParams, SigningResult, TradeFlags, UnsignedOrder,
    build_order_typed_data, cancellations_hash, compute_order_uid, decode_order_flags,
    decode_signature_owner, decode_signing_scheme, decode_trade_flags, domain_separator,
    domain_separator_from, eip1271_result, encode_order_flags, encode_signing_scheme,
    encode_trade_flags, extract_order_uid_params, generate_order_id, get_domain, hash_order,
    hash_order_cancellation, hash_order_cancellations, hash_typed_data, hashify,
    invalidate_order_calldata, normalize_buy_token_balance, normalize_order, order_hash,
    pack_order_uid_params, presign_result, set_pre_signature_calldata, sign_order,
    sign_order_cancellation, sign_order_cancellations, signing_digest,
};
pub use permit::{
    Erc20PermitInfo, PERMIT_GAS_LIMIT, PermitHookData, PermitInfo, build_permit_calldata,
    build_permit_hook, permit_digest, permit_domain_separator, permit_type_hash, sign_permit,
};
pub use settlement::{
    encoder::{EncodedInteraction, InteractionStage, SettlementEncoder},
    reader::{AllowListReader, SettlementReader},
    vault::{
        VAULT_ACTIONS, grant_role_calldata, required_vault_role_calls,
        required_vault_role_selectors, revoke_role_calldata, vault_role_hash,
    },
};
pub use subgraph::{
    Bundle, DailyTotal, DailyVolume, HourlyTotal, HourlyVolume, LAST_DAYS_VOLUME_QUERY,
    LAST_HOURS_VOLUME_QUERY, PairDaily, PairHourly, SubgraphApi, SubgraphBlock, SubgraphMeta,
    SubgraphOrder, SubgraphPair, SubgraphSettlement, SubgraphToken, SubgraphTrade, SubgraphUser,
    TOTALS_QUERY, TokenDailyTotal, TokenHourlyTotal, TokenTradingEvent, Total, Totals, UniswapPool,
    UniswapToken,
};
pub use trading::{
    Amounts, BuildAppDataParams, DEFAULT_FEE_SLIPPAGE_FACTOR_PCT, DEFAULT_QUOTE_VALIDITY,
    DEFAULT_SLIPPAGE_BPS, DEFAULT_VOLUME_SLIPPAGE_BPS, ETH_FLOW_DEFAULT_SLIPPAGE_BPS,
    GAS_LIMIT_DEFAULT, LimitOrderAdvancedSettings, LimitTradeParameters,
    LimitTradeParametersFromQuote, MAX_SLIPPAGE_BPS, NetworkFee, OrderPostingResult,
    PartnerFeeCost, PostTradeAdditionalParams, ProtocolFeeCost, QuoteAmountsAndCosts, QuoteResults,
    QuoteResultsWithSigner, QuoterParameters, SlippageToleranceRequest, SlippageToleranceResponse,
    SwapAdvancedSettings, TradeParameters, TradingAppDataInfo, TradingSdk, TradingSdkConfig,
    TradingTransactionParams, adjust_eth_flow_limit_order_params, adjust_eth_flow_order_params,
    apply_percentage, apply_settings_to_limit_trade_parameters, bps_to_percentage, build_app_data,
    calculate_gas_margin, calculate_unique_order_id, generate_app_data_from_doc,
    get_default_slippage_bps, get_default_utm_params, get_eth_flow_cancellation,
    get_eth_flow_contract, get_is_eth_flow_order, get_order_deadline_from_now, get_order_to_sign,
    get_order_typed_data, get_quote_raw, get_quote_with_signer, get_settlement_cancellation,
    get_settlement_contract, get_slippage_percent, get_trade_parameters_after_quote, get_trader,
    map_quote_amounts_and_costs, percentage_to_bps, post_co_w_protocol_trade,
    post_cow_protocol_trade, post_sell_native_currency_order, resolve_order_book_api,
    resolve_signer, resolve_slippage_suggestion, suggest_slippage_bps, suggest_slippage_from_fee,
    suggest_slippage_from_volume, swap_params_to_limit_order_params, unsigned_order_for_signing,
};
pub use traits::{CowSigner, IpfsClient, OrderbookClient, RpcProvider};
pub use types::{
    ATTESTATION_PREFIX_CONST, ATTESTATOR_ADDRESS, ATTESTION_VERSION_BYTE, EcdsaSigningScheme,
    HUNDRED_THOUSANDS, LIMIT_CONCURRENT_REQUESTS, MAX_UINT32, MAX_UINT256, ONE, ONE_HUNDRED_BPS,
    OrderKind, PriceQuality, SigningScheme, TokenBalance, ZERO, ZERO_ADDRESS, ZERO_HASH,
};
pub use weiroll::{
    WEIROLL_ADDRESS, WeirollCommand, WeirollCommandFlags, WeirollContractRef, WeirollPlanner,
    WeirollScript, create_weiroll_contract, create_weiroll_delegate_call, create_weiroll_library,
    define_read_only, get_static,
};
