//! `cow-subgraph` — Layer 4 `CoW` Protocol subgraph `GraphQL` client for the `CoW` Protocol
//! SDK.
//!
//! The subgraph indexes all `CoW` Protocol settlements and exposes aggregate
//! statistics (volume, fees, order counts) through a `GraphQL` API.
//!
//! # Submodules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`api`] | [`SubgraphApi`] `GraphQL` client with typed query methods |
//! | [`queries`] | `GraphQL` query string constants |
//! | [`types`] | Response types (`Totals`, `Token`, `Trade`, `Order`, `Pair`, …) |
//!
//! # Example
//!
//! ```rust,no_run
//! use cow_chains::{Env, SupportedChainId};
//! use cow_subgraph::SubgraphApi;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let api = SubgraphApi::new(SupportedChainId::Mainnet, Env::Prod)?;
//! let totals = api.get_totals().await?;
//! println!("{totals:?}");
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod api;
pub mod queries;
#[cfg(test)]
mod schema_validation;
pub mod types;

pub use api::SubgraphApi;
pub use queries::{LAST_DAYS_VOLUME_QUERY, LAST_HOURS_VOLUME_QUERY, TOTALS_QUERY};
pub use types::{
    Bundle, DailyTotal, DailyVolume, HourlyTotal, HourlyVolume, Order as SubgraphOrder,
    Pair as SubgraphPair, PairDaily, PairHourly, Settlement as SubgraphSettlement, SubgraphBlock,
    SubgraphMeta, Token as SubgraphToken, TokenDailyTotal, TokenHourlyTotal, TokenTradingEvent,
    Total, Totals, Trade as SubgraphTrade, UniswapPool, UniswapToken, User as SubgraphUser,
};
