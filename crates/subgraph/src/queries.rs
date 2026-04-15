//! `GraphQL` query constants for the `CoW` Protocol subgraph.
//!
//! Holds every query string that [`SubgraphApi`](super::SubgraphApi) sends
//! to the subgraph, centralised in one file so that schema-validation tests
//! (see `super::schema_validation`) can lint all of them in a single pass.
//!
//! # Public surface
//!
//! The three historically-published constants — [`TOTALS_QUERY`],
//! [`LAST_DAYS_VOLUME_QUERY`], and [`LAST_HOURS_VOLUME_QUERY`] — remain
//! `pub` for backwards compatibility. They are intended to be passed to
//! [`SubgraphApi::run_query`](super::SubgraphApi::run_query) for users
//! who want to bypass the typed helper methods.
//!
//! Every other query is `pub(crate)` because it backs a specific typed
//! helper method and is not meant to be consumed directly.
//!
//! # Drift detection
//!
//! The `CRATE_QUERIES` constant lists every internal query paired with its
//! helper method name and root entity. The schema-validation test module
//! walks this list, parses each query into a `GraphQL` AST via
//! `graphql_parser`, and verifies that every selected field exists in the
//! corresponding entity type in `specs/subgraph.graphql`. Adding a new
//! typed helper in `api.rs` therefore requires registering its query
//! here — forgetting to do so leaves the new query undetected by the
//! linter.

// ── Public (historically stable) constants ───────────────────────────────────

/// `GraphQL` query for protocol-wide aggregate statistics.
///
/// Returns a single [`Totals`](super::types::Totals) entity with token count,
/// order count, trader count, settlement count, and cumulative volume/fees in
/// both USD and ETH.
///
/// # Example
///
/// ```rust,no_run
/// use cow_rs::{SubgraphApi, subgraph::queries::TOTALS_QUERY};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let api = SubgraphApi::new_with_url("http://localhost:8080/graphql");
/// let data = api.run_query(TOTALS_QUERY, None).await?;
/// # Ok(())
/// # }
/// ```
pub const TOTALS_QUERY: &str = r#"query Totals {
  totals {
    tokens
    orders
    traders
    settlements
    volumeUsd
    volumeEth
    feesUsd
    feesEth
  }
}"#;

/// `GraphQL` query for daily volume snapshots over the last N days.
///
/// Requires a `$days: Int!` variable specifying how many days of history to
/// retrieve. Results are ordered by timestamp descending (most recent first).
///
/// # Example
///
/// ```rust,no_run
/// use cow_rs::{SubgraphApi, subgraph::queries::LAST_DAYS_VOLUME_QUERY};
/// use serde_json::json;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let api = SubgraphApi::new_with_url("http://localhost:8080/graphql");
/// let data = api.run_query(LAST_DAYS_VOLUME_QUERY, Some(json!({ "days": 7 }))).await?;
/// # Ok(())
/// # }
/// ```
pub const LAST_DAYS_VOLUME_QUERY: &str = r#"query LastDaysVolume($days: Int!) {
  dailyTotals(orderBy: timestamp, orderDirection: desc, first: $days) {
    timestamp
    volumeUsd
  }
}"#;

/// `GraphQL` query for hourly volume snapshots over the last N hours.
///
/// Requires a `$hours: Int!` variable specifying how many hours of history to
/// retrieve. Results are ordered by timestamp descending (most recent first).
///
/// # Example
///
/// ```rust,no_run
/// use cow_rs::{SubgraphApi, subgraph::queries::LAST_HOURS_VOLUME_QUERY};
/// use serde_json::json;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let api = SubgraphApi::new_with_url("http://localhost:8080/graphql");
/// let data = api.run_query(LAST_HOURS_VOLUME_QUERY, Some(json!({ "hours": 24 }))).await?;
/// # Ok(())
/// # }
/// ```
pub const LAST_HOURS_VOLUME_QUERY: &str = r#"query LastHoursVolume($hours: Int!) {
  hourlyTotals(orderBy: timestamp, orderDirection: desc, first: $hours) {
    timestamp
    volumeUsd
  }
}"#;

// ── Internal constants backing the typed helper methods ──────────────────────

pub(crate) const GET_TOTALS: &str =
    "{ totals { tokens orders traders settlements volumeUsd volumeEth feesUsd feesEth } }";

pub(crate) const GET_LAST_DAYS_VOLUME: &str = "query($days:Int!){ dailyTotals(first:$days,orderBy:timestamp,orderDirection:desc){ timestamp volumeUsd } }";

pub(crate) const GET_LAST_HOURS_VOLUME: &str = "query($hours:Int!){ hourlyTotals(first:$hours,orderBy:timestamp,orderDirection:desc){ timestamp volumeUsd } }";

pub(crate) const GET_DAILY_TOTALS: &str = "query($n:Int!){ dailyTotals(first:$n,orderBy:timestamp,orderDirection:desc){ timestamp orders traders tokens settlements volumeEth volumeUsd feesEth feesUsd } }";

pub(crate) const GET_HOURLY_TOTALS: &str = "query($n:Int!){ hourlyTotals(first:$n,orderBy:timestamp,orderDirection:desc){ timestamp orders traders tokens settlements volumeEth volumeUsd feesEth feesUsd } }";

pub(crate) const GET_ORDERS_FOR_OWNER: &str = "query($owner:String!,$n:Int!){ orders(first:$n,where:{owner:$owner},orderBy:timestamp,orderDirection:desc){ id owner{id address} sellToken{id address name symbol decimals} buyToken{id address name symbol decimals} sellAmount buyAmount validTo appData feeAmount kind partiallyFillable status executedSellAmount executedSellAmountBeforeFees executedBuyAmount executedFeeAmount timestamp txHash isSignerSafe signingScheme uid } }";

pub(crate) const GET_ETH_PRICE: &str = "{ bundle(id:\"1\") { id ethPriceUSD } }";

pub(crate) const GET_TRADES: &str = "query($n:Int!){ trades(first:$n,orderBy:timestamp,orderDirection:desc){ id timestamp gasPrice feeAmount txHash settlement buyAmount sellAmount sellAmountBeforeFees buyToken{id address name symbol decimals} sellToken{id address name symbol decimals} owner{id address} order } }";

pub(crate) const GET_SETTLEMENTS: &str = "query($n:Int!){ settlements(first:$n,orderBy:firstTradeTimestamp,orderDirection:desc){ id txHash firstTradeTimestamp solver txCost txFeeInEth } }";

pub(crate) const GET_USER: &str = "query($id:String!){ user(id:$id){ id address firstTradeTimestamp numberOfTrades solvedAmountEth solvedAmountUsd } }";

pub(crate) const GET_TOKENS: &str = "query($n:Int!){ tokens(first:$n,orderBy:numberOfTrades,orderDirection:desc){ id address firstTradeTimestamp name symbol decimals totalVolume priceEth priceUsd numberOfTrades } }";

pub(crate) const GET_TOKEN: &str = "query($id:String!){ token(id:$id){ id address firstTradeTimestamp name symbol decimals totalVolume priceEth priceUsd numberOfTrades } }";

pub(crate) const GET_TOKEN_DAILY_TOTALS: &str = "query($token:String!,$n:Int!){ tokenDailyTotals(first:$n,where:{token:$token},orderBy:timestamp,orderDirection:desc){ id token{id address name symbol decimals} timestamp totalVolume totalVolumeUsd totalTrades openPrice closePrice higherPrice lowerPrice averagePrice } }";

pub(crate) const GET_TOKEN_HOURLY_TOTALS: &str = "query($token:String!,$n:Int!){ tokenHourlyTotals(first:$n,where:{token:$token},orderBy:timestamp,orderDirection:desc){ id token{id address name symbol decimals} timestamp totalVolume totalVolumeUsd totalTrades openPrice closePrice higherPrice lowerPrice averagePrice } }";

pub(crate) const GET_TOKEN_TRADING_EVENTS: &str = "query($token:String!,$n:Int!){ tokenTradingEvents(first:$n,where:{token:$token},orderBy:timestamp,orderDirection:desc){ id token{id address name symbol decimals} priceUsd timestamp } }";

pub(crate) const GET_PAIRS: &str = "query($n:Int!){ pairs(first:$n,orderBy:numberOfTrades,orderDirection:desc){ id token0{id address name symbol decimals} token1{id address name symbol decimals} volumeToken0 volumeToken1 numberOfTrades } }";

pub(crate) const GET_PAIR: &str = "query($id:String!){ pair(id:$id){ id token0{id address name symbol decimals} token1{id address name symbol decimals} volumeToken0 volumeToken1 numberOfTrades } }";

pub(crate) const GET_PAIR_DAILY_TOTALS: &str = "query($pair:String!,$n:Int!){ pairDailies(first:$n,where:{id_starts_with:$pair},orderBy:timestamp,orderDirection:desc){ id token0{id address name symbol decimals} token1{id address name symbol decimals} timestamp volumeToken0 volumeToken1 numberOfTrades } }";

pub(crate) const GET_PAIR_HOURLY_TOTALS: &str = "query($pair:String!,$n:Int!){ pairHourlies(first:$n,where:{id_starts_with:$pair},orderBy:timestamp,orderDirection:desc){ id token0{id address name symbol decimals} token1{id address name symbol decimals} timestamp volumeToken0 volumeToken1 numberOfTrades } }";

// ── Test harness metadata ────────────────────────────────────────────────────

/// Drift-linter entry: `(helper_method_name, query_string, root_entity_name)`.
///
/// The `root_entity_name` column tells the drift-detection linter which
/// entity in `specs/subgraph.graphql` the query's top-level selection set
/// resolves to — subgraphs auto-generate a `Query` root type at build time
/// but our vendored SDL does not carry one, so this mapping is maintained
/// by hand.
#[cfg(test)]
pub(crate) type QueryEntry = (&'static str, &'static str, &'static str);

/// Every query the crate sends to the subgraph, paired with its root entity.
///
/// Every entry is walked by
/// [`super::schema_validation::tests::every_crate_query_matches_schema`]
/// to ensure selected fields exist in the schema.
#[cfg(test)]
pub(crate) const CRATE_QUERIES: &[QueryEntry] = &[
    ("get_totals", GET_TOTALS, "Total"),
    ("get_last_days_volume", GET_LAST_DAYS_VOLUME, "DailyTotal"),
    ("get_last_hours_volume", GET_LAST_HOURS_VOLUME, "HourlyTotal"),
    ("get_daily_totals", GET_DAILY_TOTALS, "DailyTotal"),
    ("get_hourly_totals", GET_HOURLY_TOTALS, "HourlyTotal"),
    ("get_orders_for_owner", GET_ORDERS_FOR_OWNER, "Order"),
    ("get_eth_price", GET_ETH_PRICE, "Bundle"),
    ("get_trades", GET_TRADES, "Trade"),
    ("get_settlements", GET_SETTLEMENTS, "Settlement"),
    ("get_user", GET_USER, "User"),
    ("get_tokens", GET_TOKENS, "Token"),
    ("get_token", GET_TOKEN, "Token"),
    ("get_token_daily_totals", GET_TOKEN_DAILY_TOTALS, "TokenDailyTotal"),
    ("get_token_hourly_totals", GET_TOKEN_HOURLY_TOTALS, "TokenHourlyTotal"),
    ("get_token_trading_events", GET_TOKEN_TRADING_EVENTS, "TokenTradingEvent"),
    ("get_pairs", GET_PAIRS, "Pair"),
    ("get_pair", GET_PAIR, "Pair"),
    ("get_pair_daily_totals", GET_PAIR_DAILY_TOTALS, "PairDaily"),
    ("get_pair_hourly_totals", GET_PAIR_HOURLY_TOTALS, "PairHourly"),
];

/// Public-API constants plus their root entities for exhaustive linting.
#[cfg(test)]
pub(crate) const PUBLIC_QUERIES: &[QueryEntry] = &[
    ("TOTALS_QUERY", TOTALS_QUERY, "Total"),
    ("LAST_DAYS_VOLUME_QUERY", LAST_DAYS_VOLUME_QUERY, "DailyTotal"),
    ("LAST_HOURS_VOLUME_QUERY", LAST_HOURS_VOLUME_QUERY, "HourlyTotal"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_query_contains_expected_fields() {
        assert!(TOTALS_QUERY.contains("tokens"));
        assert!(TOTALS_QUERY.contains("orders"));
        assert!(TOTALS_QUERY.contains("traders"));
        assert!(TOTALS_QUERY.contains("settlements"));
        assert!(TOTALS_QUERY.contains("volumeUsd"));
        assert!(TOTALS_QUERY.contains("volumeEth"));
        assert!(TOTALS_QUERY.contains("feesUsd"));
        assert!(TOTALS_QUERY.contains("feesEth"));
    }

    #[test]
    fn last_days_volume_query_contains_variable() {
        assert!(LAST_DAYS_VOLUME_QUERY.contains("$days: Int!"));
        assert!(LAST_DAYS_VOLUME_QUERY.contains("dailyTotals"));
        assert!(LAST_DAYS_VOLUME_QUERY.contains("volumeUsd"));
    }

    #[test]
    fn last_hours_volume_query_contains_variable() {
        assert!(LAST_HOURS_VOLUME_QUERY.contains("$hours: Int!"));
        assert!(LAST_HOURS_VOLUME_QUERY.contains("hourlyTotals"));
        assert!(LAST_HOURS_VOLUME_QUERY.contains("volumeUsd"));
    }

    #[test]
    fn every_internal_query_is_registered_exactly_once() {
        let names: Vec<&str> = CRATE_QUERIES.iter().map(|(name, _, _)| *name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            names.len(),
            sorted.len(),
            "CRATE_QUERIES has duplicate helper names — every API method must register exactly \
             once"
        );
    }
}
