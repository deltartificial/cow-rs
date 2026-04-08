//! `GraphQL` query constants for the `CoW` Protocol subgraph.
//!
//! These constants mirror the queries defined in the TypeScript SDK's
//! `@cowprotocol/subgraph` package and can be passed directly to
//! [`SubgraphApi::run_query`](super::SubgraphApi::run_query).

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
}
