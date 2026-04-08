//! `CoW` Protocol orderbook API client and types.
//!
//! This module provides the HTTP client ([`OrderBookApi`]) for interacting
//! with the `CoW` Protocol orderbook REST API, plus all request/response
//! types and quote-amount calculation helpers.
//!
//! # Submodules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`api`] | [`OrderBookApi`] async HTTP client with typed endpoint methods |
//! | [`types`] | Request/response structs (`OrderQuoteRequest`, `Order`, `Trade`, …) |
//! | [`quote_amounts`] | Fee breakdown, slippage, and cost-stage calculations |
//!
//! # Quick start
//!
//! ```rust,no_run
//! use cow_rs::{Env, OrderBookApi, SupportedChainId};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
//! let version = api.get_version().await?;
//! println!("orderbook version: {version}");
//! # Ok(())
//! # }
//! ```

pub mod api;
pub mod generated;
pub mod quote_amounts;
pub mod types;

pub use api::{OrderBookApi, mock_get_order, request};
pub use quote_amounts::{
    PartnerFeeResult, ProtocolFeeAmountParams, QuoteAmounts, QuoteAmountsAndCostsParams,
    QuoteAmountsAndCostsResult, QuoteCosts, QuoteFeeComponent, QuoteNetworkFee, QuoteOrderParams,
    get_protocol_fee_amount, get_quote_amounts_after_partner_fee, get_quote_amounts_after_slippage,
    get_quote_amounts_and_costs, transform_order,
};
pub use types::{
    AppDataObject, Auction, CompetitionAuction, CompetitionOrderStatus, CompetitionOrderStatusKind,
    EthflowData, GetOrdersRequest, GetTradesRequest, InteractionData, OnchainOrderData, Order,
    OrderCancellations, OrderClass, OrderCreation, OrderInteractions, OrderQuoteRequest,
    OrderQuoteResponse, OrderStatus, OrderUid, QuoteData, QuoteSide, SolverCompetition,
    SolverExecution, SolverSettlement, TotalSurplus, Trade, is_eth_flow_order,
};
