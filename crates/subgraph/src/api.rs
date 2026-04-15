//! [`SubgraphApi`] — `GraphQL` client for the `CoW` Protocol subgraph.
//!
//! Provides typed query methods for protocol statistics, volume history,
//! tokens, trades, settlements, pairs, and user data. All queries are
//! executed via [`run_query`](SubgraphApi::run_query) which handles the
//! `GraphQL` request/response envelope.

use std::sync::Arc;

use cow_chains::chain::{Env, SupportedChainId};
use cow_errors::CowError;
use cow_http::{RateLimiter, RetryPolicy};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{
    queries,
    types::{
        Bundle, DailyTotal, DailyVolume, HourlyTotal, HourlyVolume, Order, Pair, PairDaily,
        PairHourly, Settlement, Token, TokenDailyTotal, TokenHourlyTotal, TokenTradingEvent,
        Totals, Trade, User,
    },
};

/// Return the subgraph base URL for `chain`, or `None` if unsupported.
///
/// URLs point to `TheGraph` decentralised network deployment IDs used by
/// the `CoW` Protocol team.
const fn subgraph_url(chain: SupportedChainId, _env: Env) -> Option<&'static str> {
    match chain {
        SupportedChainId::Mainnet => {
            Some("https://api.thegraph.com/subgraphs/name/cowprotocol/cow")
        }
        SupportedChainId::GnosisChain => {
            Some("https://api.thegraph.com/subgraphs/name/cowprotocol/cow-gc")
        }
        SupportedChainId::ArbitrumOne => {
            Some("https://api.thegraph.com/subgraphs/name/cowprotocol/cow-arbitrum-one")
        }
        SupportedChainId::Base => {
            Some("https://api.thegraph.com/subgraphs/name/cowprotocol/cow-base")
        }
        SupportedChainId::Sepolia => {
            Some("https://api.thegraph.com/subgraphs/name/cowprotocol/cow-sepolia")
        }
        SupportedChainId::Polygon |
        SupportedChainId::Avalanche |
        SupportedChainId::BnbChain |
        SupportedChainId::Linea |
        SupportedChainId::Lens |
        SupportedChainId::Plasma |
        SupportedChainId::Ink => None,
    }
}

/// `GraphQL` client for the `CoW` Protocol subgraph.
///
/// Provides high-level query methods for protocol statistics, volume
/// history, tokens, trades, settlements, pairs, and user profiles.
/// Wraps a `reqwest::Client` and the subgraph endpoint URL.
///
/// Construct via [`new`](Self::new) (chain + env) or
/// [`new_with_url`](Self::new_with_url) (arbitrary endpoint).
#[derive(Debug, Clone)]
pub struct SubgraphApi {
    client: reqwest::Client,
    base_url: String,
    /// Shared token bucket, identical in shape to the one that backs
    /// [`super::super::order_book::OrderBookApi`]. Default: 5 req/s.
    rate_limiter: Arc<RateLimiter>,
    /// Exponential-backoff retry policy for transient `GraphQL` failures.
    retry_policy: RetryPolicy,
}

impl SubgraphApi {
    /// Build a `reqwest::Client` with platform-appropriate settings.
    #[allow(clippy::shadow_reuse, reason = "builder pattern chains naturally shadow")]
    fn build_client() -> reqwest::Client {
        let builder = reqwest::Client::builder();
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.timeout(std::time::Duration::from_secs(30));
        builder.build().unwrap_or_default()
    }

    /// Create a new [`SubgraphApi`] for the given chain and environment.
    ///
    /// # Parameters
    ///
    /// * `chain` — the target [`SupportedChainId`].
    /// * `env` — the orderbook [`Env`] (currently unused — the subgraph URL is the same for prod
    ///   and staging).
    ///
    /// # Returns
    ///
    /// A new [`SubgraphApi`] or an error if `chain` has no subgraph
    /// deployment.
    ///
    /// # Example
    ///
    /// ```rust
    /// use cow_chains::{Env, SupportedChainId};
    /// use cow_subgraph::SubgraphApi;
    ///
    /// let api = SubgraphApi::new(SupportedChainId::Mainnet, Env::Prod);
    /// assert!(api.is_ok());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CowError::UnknownAsset`] if the chain has no subgraph endpoint.
    pub fn new(chain: SupportedChainId, env: Env) -> Result<Self, CowError> {
        let base_url = subgraph_url(chain, env)
            .ok_or_else(|| CowError::UnknownAsset(format!("no subgraph for chain {chain}")))?;
        Ok(Self {
            client: Self::build_client(),
            base_url: base_url.to_owned(),
            rate_limiter: Arc::new(RateLimiter::default_orderbook()),
            retry_policy: RetryPolicy::default_orderbook(),
        })
    }

    /// Create a [`SubgraphApi`] pointing at an arbitrary `GraphQL` endpoint
    /// URL.
    ///
    /// Useful for integration tests with a local mock server or for querying
    /// a custom/self-hosted subgraph deployment.
    ///
    /// # Parameters
    ///
    /// * `url` — the full `GraphQL` endpoint URL.
    ///
    /// # Returns
    ///
    /// A new [`SubgraphApi`] (infallible).
    ///
    /// ```
    /// use cow_subgraph::SubgraphApi;
    /// let api = SubgraphApi::new_with_url("http://localhost:8080/graphql");
    /// ```
    #[must_use]
    pub fn new_with_url(url: impl Into<String>) -> Self {
        Self {
            client: Self::build_client(),
            base_url: url.into(),
            rate_limiter: Arc::new(RateLimiter::default_orderbook()),
            retry_policy: RetryPolicy::default_orderbook(),
        }
    }

    /// Override the shared rate limiter used for every outbound request.
    ///
    /// Defaults to [`RateLimiter::default_orderbook`] (5 req/s). Pass a
    /// shared `Arc` to make several `SubgraphApi` instances throttle
    /// against the same budget — or to make a single budget shared
    /// between a `SubgraphApi` and an
    /// [`super::super::order_book::OrderBookApi`].
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<RateLimiter>) -> Self {
        self.rate_limiter = limiter;
        self
    }

    /// Override the retry policy used for every outbound request.
    ///
    /// Defaults to [`RetryPolicy::default_orderbook`] (10 attempts,
    /// exponential backoff on 408/425/429/5xx). Use
    /// [`RetryPolicy::no_retry`] to disable retries entirely.
    #[must_use]
    #[allow(
        clippy::missing_const_for_fn,
        reason = "RetryPolicy contains a Duration whose Drop is non-const"
    )]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Fetch protocol-wide aggregate statistics.
    ///
    /// # Returns
    ///
    /// A `Vec<Totals>` (typically a single element) with token count,
    /// order count, trader count, settlement count, and cumulative
    /// volume/fees.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cow_chains::{Env, SupportedChainId};
    /// use cow_subgraph::SubgraphApi;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = SubgraphApi::new(SupportedChainId::Mainnet, Env::Prod)?;
    /// let totals = api.get_totals().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_totals(&self) -> Result<Vec<Totals>, CowError> {
        let data = self.run_query(queries::GET_TOTALS, None).await?;
        parse_field(&data, "totals")
    }

    /// Fetch daily volume snapshots for the last `days` days (most recent
    /// first).
    ///
    /// # Parameters
    ///
    /// * `days` — number of days of history to retrieve.
    ///
    /// # Returns
    ///
    /// A `Vec<DailyVolume>` with up to `days` entries, newest first.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cow_chains::{Env, SupportedChainId};
    /// use cow_subgraph::SubgraphApi;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = SubgraphApi::new(SupportedChainId::Mainnet, Env::Prod)?;
    /// let volumes = api.get_last_days_volume(7).await?;
    /// assert!(volumes.len() <= 7);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_last_days_volume(&self, days: u32) -> Result<Vec<DailyVolume>, CowError> {
        let data =
            self.run_query(queries::GET_LAST_DAYS_VOLUME, Some(json!({ "days": days }))).await?;
        parse_field(&data, "dailyTotals")
    }

    /// Fetch hourly volume snapshots for the last `hours` hours (most
    /// recent first).
    ///
    /// # Parameters
    ///
    /// * `hours` — number of hours of history to retrieve.
    ///
    /// # Returns
    ///
    /// A `Vec<HourlyVolume>` with up to `hours` entries, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_last_hours_volume(&self, hours: u32) -> Result<Vec<HourlyVolume>, CowError> {
        let data =
            self.run_query(queries::GET_LAST_HOURS_VOLUME, Some(json!({ "hours": hours }))).await?;
        parse_field(&data, "hourlyTotals")
    }

    /// Fetch full per-day statistics for the last `days` days.
    ///
    /// Unlike [`get_last_days_volume`](Self::get_last_days_volume), this returns
    /// all fields from the `DailyTotal` entity.
    ///
    /// # Parameters
    ///
    /// * `days` — number of days of history to retrieve.
    ///
    /// # Returns
    ///
    /// A `Vec<DailyTotal>` with up to `days` entries, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_daily_totals(&self, days: u32) -> Result<Vec<DailyTotal>, CowError> {
        let data = self.run_query(queries::GET_DAILY_TOTALS, Some(json!({ "n": days }))).await?;
        parse_field(&data, "dailyTotals")
    }

    /// Fetch full per-hour statistics for the last `hours` hours.
    ///
    /// # Parameters
    ///
    /// * `hours` — number of hours of history to retrieve.
    ///
    /// # Returns
    ///
    /// A `Vec<HourlyTotal>` with up to `hours` entries, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_hourly_totals(&self, hours: u32) -> Result<Vec<HourlyTotal>, CowError> {
        let data = self.run_query(queries::GET_HOURLY_TOTALS, Some(json!({ "n": hours }))).await?;
        parse_field(&data, "hourlyTotals")
    }

    /// Fetch orders for a specific `owner` address (most recent first).
    ///
    /// # Parameters
    ///
    /// * `owner` — the trader's Ethereum address (lowercase hex).
    /// * `limit` — maximum number of orders to return (max 1 000 per subgraph page).
    ///
    /// # Returns
    ///
    /// A `Vec<Order>` with up to `limit` entries, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_orders_for_owner(
        &self,
        owner: &str,
        limit: u32,
    ) -> Result<Vec<Order>, CowError> {
        let data = self
            .run_query(queries::GET_ORDERS_FOR_OWNER, Some(json!({ "owner": owner, "n": limit })))
            .await?;
        parse_field(&data, "orders")
    }

    /// Fetch the current ETH/USD price from the protocol's [`Bundle`] entity.
    ///
    /// # Returns
    ///
    /// A [`Bundle`] containing the current ETH/USD price.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_eth_price(&self) -> Result<Bundle, CowError> {
        let data = self.run_query(queries::GET_ETH_PRICE, None).await?;
        parse_field(&data, "bundle")
    }

    /// Fetch the most recent `limit` protocol-wide trades (most recent first).
    ///
    /// # Parameters
    ///
    /// * `limit` — maximum number of trades to return.
    ///
    /// # Returns
    ///
    /// A `Vec<Trade>` with up to `limit` entries, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_trades(&self, limit: u32) -> Result<Vec<Trade>, CowError> {
        let data = self.run_query(queries::GET_TRADES, Some(json!({ "n": limit }))).await?;
        parse_field(&data, "trades")
    }

    /// Fetch the most recent `limit` on-chain settlements (most recent first).
    ///
    /// # Parameters
    ///
    /// * `limit` — maximum number of settlements to return.
    ///
    /// # Returns
    ///
    /// A `Vec<Settlement>` with up to `limit` entries, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_settlements(&self, limit: u32) -> Result<Vec<Settlement>, CowError> {
        let data = self.run_query(queries::GET_SETTLEMENTS, Some(json!({ "n": limit }))).await?;
        parse_field(&data, "settlements")
    }

    /// Fetch a single [`User`] by Ethereum address.
    ///
    /// # Parameters
    ///
    /// * `address` — the trader's Ethereum address (lowercase hex).
    ///
    /// # Returns
    ///
    /// A [`User`] with aggregate trading statistics.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`], [`CowError::Api`], or [`CowError::Parse`] on failure.
    pub async fn get_user(&self, address: &str) -> Result<User, CowError> {
        let data = self.run_query(queries::GET_USER, Some(json!({ "id": address }))).await?;
        parse_field(&data, "user")
    }

    /// Fetch the most recent `limit` [`Token`]s sorted by number of trades.
    ///
    /// # Parameters
    ///
    /// * `limit` — maximum number of tokens to return.
    ///
    /// # Returns
    ///
    /// A `Vec<Token>` with up to `limit` entries, sorted by trade count
    /// descending.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_tokens(&self, limit: u32) -> Result<Vec<Token>, CowError> {
        let data = self.run_query(queries::GET_TOKENS, Some(json!({ "n": limit }))).await?;
        parse_field(&data, "tokens")
    }

    /// Fetch a single [`Token`] by its contract address (lowercase hex).
    ///
    /// # Parameters
    ///
    /// * `address` — the token's contract address (lowercase hex).
    ///
    /// # Returns
    ///
    /// A [`Token`] with metadata and aggregate statistics.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`], [`CowError::Api`], or [`CowError::Parse`] on failure.
    pub async fn get_token(&self, address: &str) -> Result<Token, CowError> {
        let data = self.run_query(queries::GET_TOKEN, Some(json!({ "id": address }))).await?;
        parse_field(&data, "token")
    }

    /// Fetch per-day statistics for `token_address` over the last `days` days.
    ///
    /// # Parameters
    ///
    /// * `token_address` — the token's contract address (lowercase hex).
    /// * `days` — number of days of history to retrieve.
    ///
    /// # Returns
    ///
    /// A `Vec<TokenDailyTotal>` with up to `days` entries, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_token_daily_totals(
        &self,
        token_address: &str,
        days: u32,
    ) -> Result<Vec<TokenDailyTotal>, CowError> {
        let data = self
            .run_query(
                queries::GET_TOKEN_DAILY_TOTALS,
                Some(json!({ "token": token_address, "n": days })),
            )
            .await?;
        parse_field(&data, "tokenDailyTotals")
    }

    /// Fetch per-hour statistics for `token_address` over the last `hours` hours.
    ///
    /// # Parameters
    ///
    /// * `token_address` — the token's contract address (lowercase hex).
    /// * `hours` — number of hours of history to retrieve.
    ///
    /// # Returns
    ///
    /// A `Vec<TokenHourlyTotal>` with up to `hours` entries, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_token_hourly_totals(
        &self,
        token_address: &str,
        hours: u32,
    ) -> Result<Vec<TokenHourlyTotal>, CowError> {
        let data = self
            .run_query(
                queries::GET_TOKEN_HOURLY_TOTALS,
                Some(json!({ "token": token_address, "n": hours })),
            )
            .await?;
        parse_field(&data, "tokenHourlyTotals")
    }

    /// Fetch price-change events for `token_address` (most recent first).
    ///
    /// # Parameters
    ///
    /// * `token_address` — the token's contract address (lowercase hex).
    /// * `limit` — maximum number of events to return.
    ///
    /// # Returns
    ///
    /// A `Vec<TokenTradingEvent>` with up to `limit` entries, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_token_trading_events(
        &self,
        token_address: &str,
        limit: u32,
    ) -> Result<Vec<TokenTradingEvent>, CowError> {
        let data = self
            .run_query(
                queries::GET_TOKEN_TRADING_EVENTS,
                Some(json!({ "token": token_address, "n": limit })),
            )
            .await?;
        parse_field(&data, "tokenTradingEvents")
    }

    /// Fetch the most actively traded pairs (by number of trades).
    ///
    /// # Parameters
    ///
    /// * `limit` — maximum number of pairs to return.
    ///
    /// # Returns
    ///
    /// A `Vec<Pair>` with up to `limit` entries, sorted by trade count
    /// descending.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_pairs(&self, limit: u32) -> Result<Vec<Pair>, CowError> {
        let data = self.run_query(queries::GET_PAIRS, Some(json!({ "n": limit }))).await?;
        parse_field(&data, "pairs")
    }

    /// Fetch a single [`Pair`] by its subgraph ID (`{token0}-{token1}`).
    ///
    /// # Parameters
    ///
    /// * `id` — the pair's subgraph entity ID (`{token0Address}-{token1Address}`).
    ///
    /// # Returns
    ///
    /// A [`Pair`] with volume and trade count statistics.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`], [`CowError::Api`], or [`CowError::Parse`] on failure.
    pub async fn get_pair(&self, id: &str) -> Result<Pair, CowError> {
        let data = self.run_query(queries::GET_PAIR, Some(json!({ "id": id }))).await?;
        parse_field(&data, "pair")
    }

    /// Fetch per-day statistics for `pair_id` over the last `days` days.
    ///
    /// # Parameters
    ///
    /// * `pair_id` — the pair's subgraph entity ID.
    /// * `days` — number of days of history to retrieve.
    ///
    /// # Returns
    ///
    /// A `Vec<PairDaily>` with up to `days` entries, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_pair_daily_totals(
        &self,
        pair_id: &str,
        days: u32,
    ) -> Result<Vec<PairDaily>, CowError> {
        let data = self
            .run_query(queries::GET_PAIR_DAILY_TOTALS, Some(json!({ "pair": pair_id, "n": days })))
            .await?;
        parse_field(&data, "pairDailies")
    }

    /// Fetch per-hour statistics for `pair_id` over the last `hours` hours.
    ///
    /// # Parameters
    ///
    /// * `pair_id` — the pair's subgraph entity ID.
    /// * `hours` — number of hours of history to retrieve.
    ///
    /// # Returns
    ///
    /// A `Vec<PairHourly>` with up to `hours` entries, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] or [`CowError::Api`] on failure.
    pub async fn get_pair_hourly_totals(
        &self,
        pair_id: &str,
        hours: u32,
    ) -> Result<Vec<PairHourly>, CowError> {
        let data = self
            .run_query(
                queries::GET_PAIR_HOURLY_TOTALS,
                Some(json!({ "pair": pair_id, "n": hours })),
            )
            .await?;
        parse_field(&data, "pairHourlies")
    }

    /// Execute a raw `GraphQL` query against the configured subgraph
    /// endpoint and return the top-level `data` JSON object.
    ///
    /// Use this method when the high-level query methods do not cover your
    /// use case. The query string and variables are sent as a standard
    /// `GraphQL` POST request.
    ///
    /// # Parameters
    ///
    /// * `query` — the `GraphQL` query string.
    /// * `variables` — optional JSON object of query variables.
    ///
    /// # Returns
    ///
    /// The top-level `data` JSON [`Value`] from the `GraphQL` response.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Http`] on transport failure, [`CowError::Api`] if the
    /// subgraph returns a non-200 status or a `GraphQL` `errors` array, or
    /// [`CowError::Parse`] if the response body is not valid JSON or lacks a
    /// `data` field.
    pub async fn run_query(
        &self,
        query: &str,
        variables: Option<Value>,
    ) -> Result<Value, CowError> {
        let mut body = json!({ "query": query });
        if let Some(vars) = variables {
            body["variables"] = vars;
        }

        // Rate-limit + retry policy, mirroring `OrderBookApi::send_with_policy`.
        // The closure captures `&body` and rebuilds a fresh `RequestBuilder`
        // on every attempt because `reqwest::RequestBuilder` is not always
        // `Clone`.
        let max = self.retry_policy.max_attempts.max(1);
        let mut attempt: u32 = 0;
        let resp = loop {
            self.rate_limiter.acquire().await;
            let result = self.client.post(&self.base_url).json(&body).send().await;
            let last_attempt = attempt + 1 >= max;
            match result {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    if !self.retry_policy.should_retry_status(status) || last_attempt {
                        break resp;
                    }
                }
                Err(e) => {
                    if !self.retry_policy.should_retry_error(&e) || last_attempt {
                        return Err(e.into());
                    }
                }
            }
            self.retry_policy.wait(self.retry_policy.delay_for_attempt(attempt)).await;
            attempt += 1;
        };

        let status = resp.status().as_u16();
        let text = resp.text().await?;

        if status != 200 {
            return Err(CowError::Api { status, body: text });
        }

        let parsed: Value = serde_json::from_str(&text)
            .map_err(|e| CowError::Parse { field: "response", reason: e.to_string() })?;

        if let Some(errors) = parsed.get("errors") {
            return Err(CowError::Api { status: 200, body: errors.to_string() });
        }

        parsed
            .get("data")
            .cloned()
            .ok_or_else(|| CowError::Parse { field: "data", reason: "missing data field".into() })
    }
}

/// Extract and deserialize a named field from a `GraphQL` response `data` object.
fn parse_field<T: DeserializeOwned>(data: &Value, field: &'static str) -> Result<T, CowError> {
    let val =
        data.get(field).ok_or_else(|| CowError::Parse { field, reason: "field missing".into() })?;
    serde_json::from_value(val.clone())
        .map_err(|e| CowError::Parse { field, reason: e.to_string() })
}
