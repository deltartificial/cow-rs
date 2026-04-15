//! `cow-sdk` — Rust SDK for the `CoW` Protocol.
//!
//! This is the Layer 6 façade crate of the `cow-sdk` workspace. It exists
//! solely to re-export the layered `cow-sdk-*` crates under a single,
//! ergonomic entry point — it contains **no logic of its own**.
//!
//! Users who want the full SDK can depend on `cow-sdk` and access every
//! module through this one import root. Users who need to keep their dep
//! tree minimal can bypass the façade and depend directly on the
//! individual `cow-sdk-*` crates they actually use (tree-shaking).
//!
//! # Workspace layout (quick reference)
//!
//! | Layer | Crate | Purpose |
//! |---|---|---|
//! | L0 | [`primitives`], [`chains`], [`error`] | Foundations |
//! | L1 | [`types`] | Protocol enums |
//! | L2 | [`signing`], [`app_data`], [`permit`], [`erc20`], [`ethflow`], [`weiroll`], [`cow_shed`], [`settlement`] | Domain modules |
//! | L4 | [`orderbook`], [`subgraph`], [`onchain`] | Transport clients |
//! | L5 | [`trading`], [`composable`], [`bridging`], [`flash_loans`] | Orchestration |
//! | L6 | [`crate`] | Façade (this crate) |
//!
//! # Quick start
//!
//! ```rust,no_run
//! use alloy_primitives::U256;
//! use cow_sdk::{
//!     chains::SupportedChainId,
//!     trading::{TradeParameters, TradingSdk, TradingSdkConfig},
//!     types::OrderKind,
//! };
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let sdk = TradingSdk::new(
//!     TradingSdkConfig::prod(SupportedChainId::Sepolia, "MyApp"),
//!     "0xdeadbeef...",
//! )?;
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

// ── Layer 0 ────────────────────────────────────────────────────────────────

/// Numeric constants and zero addresses (L0).
pub use cow_primitives as primitives;

/// Per-chain configuration, contract addresses and canonical endpoints (L0).
pub use cow_chains as chains;

/// Shared error type (L0).
pub use cow_errors as error;

// ── Layer 1 ────────────────────────────────────────────────────────────────

/// Protocol enums (`OrderKind`, `SigningScheme`, ...) and shared protocol types (L1).
pub use cow_types as types;

// ── Layer 2 ────────────────────────────────────────────────────────────────

/// EIP-712 signing, `OrderUid` computation (L2).
pub use cow_signing as signing;

/// App-data schema, validation, CID encoding, hooks metadata (L2).
pub use cow_app_data as app_data;

/// EIP-2612 permit utilities (L2).
pub use cow_permit as permit;

/// ERC-20 and EIP-2612 calldata builders (L2).
pub use cow_erc20 as erc20;

/// `EthFlow` native-currency order encoding (L2).
pub use cow_ethflow as ethflow;

/// Weiroll script builder (L2).
pub use cow_weiroll as weiroll;

/// `CoW` Shed proxy contract helpers (L2).
pub use cow_shed;

/// Settlement encoder, simulator, vault, refunds (L2).
pub use cow_settlement as settlement;

// ── Layer 3 ────────────────────────────────────────────────────────────────

/// HTTP transport primitives: rate limiter, retry policy (L3).
pub use cow_http as http;

// ── Layer 4 ────────────────────────────────────────────────────────────────

/// Orderbook REST API client (L4).
pub use cow_orderbook as orderbook;

/// Subgraph `GraphQL` client (L4).
pub use cow_subgraph as subgraph;

/// JSON-RPC `eth_call` reader (L4).
pub use cow_onchain as onchain;

// ── Layer 5 ────────────────────────────────────────────────────────────────

/// High-level `TradingSdk` (quote → sign → post → track) (L5).
pub use cow_trading as trading;

/// Composable (conditional) orders: TWAP, stop-loss, GAT (L5).
pub use cow_composable as composable;

/// Cross-chain bridge aggregator (L5).
pub use cow_bridging as bridging;

/// Flash loan orchestration helpers (L5).
pub use cow_flash_loans as flash_loans;

// ── Prelude ────────────────────────────────────────────────────────────────

/// Curated prelude of the most commonly used items.
///
/// `use cow_sdk::prelude::*;` pulls in the handful of types you need for
/// 90% of trading flows.
pub mod prelude {
    pub use cow_chains::{Env, SupportedChainId};
    pub use cow_errors::CowError;
    pub use cow_orderbook::OrderBookApi;
    pub use cow_trading::{TradeParameters, TradingSdk, TradingSdkConfig};
    pub use cow_types::{OrderKind, SigningScheme, TokenBalance};
}
