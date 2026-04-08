//! [`SubgraphApi`] — `GraphQL` client for the `CoW` Protocol subgraph.
//!
//! Provides typed query methods for protocol statistics, volume history,
//! tokens, trades, settlements, pairs, and user data. All queries are
//! executed via [`run_query`](SubgraphApi::run_query) which handles the
//! `GraphQL` request/response envelope.

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::{
    config::chain::{Env, SupportedChainId},
    error::CowError,
};

use super::types::{
    Bundle, DailyTotal, DailyVolume, HourlyTotal, HourlyVolume, Order, Pair, PairDaily, PairHourly,
    Settlement, Token, TokenDailyTotal, TokenHourlyTotal, TokenTradingEvent, Totals, Trade, User,
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
}

impl SubgraphApi {
    /// Build a `reqwest::Client` with platform-appropriate settings.
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
    /// * `env` — the orderbook [`Env`] (currently unused — the subgraph
    ///   URL is the same for prod and staging).
    ///
    /// # Returns
    ///
    /// A new [`SubgraphApi`] or an error if `chain` has no subgraph
    /// deployment.
    ///
    /// # Example
    ///
    /// ```rust
    /// use cow_rs::{Env, SubgraphApi, SupportedChainId};
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
        Ok(Self { client: Self::build_client(), base_url: base_url.to_owned() })
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
    /// use cow_rs::SubgraphApi;
    /// let api = SubgraphApi::new_with_url("http://localhost:8080/graphql");
    /// ```
    #[must_use]
    pub fn new_with_url(url: impl Into<String>) -> Self {
        Self { client: Self::build_client(), base_url: url.into() }
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
    /// use cow_rs::{Env, SubgraphApi, SupportedChainId};
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
        const Q: &str =
            "{ totals { tokens orders traders settlements volumeUsd volumeEth feesUsd feesEth } }";
        let data = self.run_query(Q, None).await?;
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
    /// use cow_rs::{Env, SubgraphApi, SupportedChainId};
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
        const Q: &str = "query($days:Int!){ dailyTotals(first:$days,orderBy:timestamp,orderDirection:desc){ timestamp volumeUsd } }";
        let data = self.run_query(Q, Some(json!({ "days": days }))).await?;
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
        const Q: &str = "query($hours:Int!){ hourlyTotals(first:$hours,orderBy:timestamp,orderDirection:desc){ timestamp volumeUsd } }";
        let data = self.run_query(Q, Some(json!({ "hours": hours }))).await?;
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
        const Q: &str = "query($n:Int!){ dailyTotals(first:$n,orderBy:timestamp,orderDirection:desc){ timestamp orders traders tokens settlements volumeEth volumeUsd feesEth feesUsd } }";
        let data = self.run_query(Q, Some(json!({ "n": days }))).await?;
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
        const Q: &str = "query($n:Int!){ hourlyTotals(first:$n,orderBy:timestamp,orderDirection:desc){ timestamp orders traders tokens settlements volumeEth volumeUsd feesEth feesUsd } }";
        let data = self.run_query(Q, Some(json!({ "n": hours }))).await?;
        parse_field(&data, "hourlyTotals")
    }

    /// Fetch orders for a specific `owner` address (most recent first).
    ///
    /// # Parameters
    ///
    /// * `owner` — the trader's Ethereum address (lowercase hex).
    /// * `limit` — maximum number of orders to return (max 1 000 per
    ///   subgraph page).
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
        const Q: &str = "query($owner:String!,$n:Int!){ orders(first:$n,where:{owner:$owner},orderBy:timestamp,orderDirection:desc){ id owner{id address} sellToken{id address name symbol decimals} buyToken{id address name symbol decimals} sellAmount buyAmount validTo appData feeAmount kind partiallyFillable status executedSellAmount executedSellAmountBeforeFees executedBuyAmount executedFeeAmount timestamp txHash isSignerSafe signingScheme uid } }";
        let data = self.run_query(Q, Some(json!({ "owner": owner, "n": limit }))).await?;
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
        const Q: &str = "{ bundle(id:\"1\") { id ethPriceUSD } }";
        let data = self.run_query(Q, None).await?;
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
        const Q: &str = "query($n:Int!){ trades(first:$n,orderBy:timestamp,orderDirection:desc){ id timestamp gasPrice feeAmount txHash settlement buyAmount sellAmount sellAmountBeforeFees buyToken{id address name symbol decimals} sellToken{id address name symbol decimals} owner{id address} order } }";
        let data = self.run_query(Q, Some(json!({ "n": limit }))).await?;
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
        const Q: &str = "query($n:Int!){ settlements(first:$n,orderBy:firstTradeTimestamp,orderDirection:desc){ id txHash firstTradeTimestamp solver txCost txFeeInEth } }";
        let data = self.run_query(Q, Some(json!({ "n": limit }))).await?;
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
        const Q: &str = "query($id:String!){ user(id:$id){ id address firstTradeTimestamp numberOfTrades solvedAmountEth solvedAmountUsd } }";
        let data = self.run_query(Q, Some(json!({ "id": address }))).await?;
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
        const Q: &str = "query($n:Int!){ tokens(first:$n,orderBy:numberOfTrades,orderDirection:desc){ id address firstTradeTimestamp name symbol decimals totalVolume priceEth priceUsd numberOfTrades } }";
        let data = self.run_query(Q, Some(json!({ "n": limit }))).await?;
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
        const Q: &str = "query($id:String!){ token(id:$id){ id address firstTradeTimestamp name symbol decimals totalVolume priceEth priceUsd numberOfTrades } }";
        let data = self.run_query(Q, Some(json!({ "id": address }))).await?;
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
        const Q: &str = "query($token:String!,$n:Int!){ tokenDailyTotals(first:$n,where:{token:$token},orderBy:timestamp,orderDirection:desc){ id token{id address name symbol decimals} timestamp totalVolume totalVolumeUsd totalTrades openPrice closePrice higherPrice lowerPrice averagePrice } }";
        let data = self.run_query(Q, Some(json!({ "token": token_address, "n": days }))).await?;
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
        const Q: &str = "query($token:String!,$n:Int!){ tokenHourlyTotals(first:$n,where:{token:$token},orderBy:timestamp,orderDirection:desc){ id token{id address name symbol decimals} timestamp totalVolume totalVolumeUsd totalTrades openPrice closePrice higherPrice lowerPrice averagePrice } }";
        let data = self.run_query(Q, Some(json!({ "token": token_address, "n": hours }))).await?;
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
        const Q: &str = "query($token:String!,$n:Int!){ tokenTradingEvents(first:$n,where:{token:$token},orderBy:timestamp,orderDirection:desc){ id token{id address name symbol decimals} priceUsd timestamp } }";
        let data = self.run_query(Q, Some(json!({ "token": token_address, "n": limit }))).await?;
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
        const Q: &str = "query($n:Int!){ pairs(first:$n,orderBy:numberOfTrades,orderDirection:desc){ id token0{id address name symbol decimals} token1{id address name symbol decimals} volumeToken0 volumeToken1 numberOfTrades } }";
        let data = self.run_query(Q, Some(json!({ "n": limit }))).await?;
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
        const Q: &str = "query($id:String!){ pair(id:$id){ id token0{id address name symbol decimals} token1{id address name symbol decimals} volumeToken0 volumeToken1 numberOfTrades } }";
        let data = self.run_query(Q, Some(json!({ "id": id }))).await?;
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
        const Q: &str = "query($pair:String!,$n:Int!){ pairDailies(first:$n,where:{id_starts_with:$pair},orderBy:timestamp,orderDirection:desc){ id token0{id address name symbol decimals} token1{id address name symbol decimals} timestamp volumeToken0 volumeToken1 numberOfTrades } }";
        let data = self.run_query(Q, Some(json!({ "pair": pair_id, "n": days }))).await?;
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
        const Q: &str = "query($pair:String!,$n:Int!){ pairHourlies(first:$n,where:{id_starts_with:$pair},orderBy:timestamp,orderDirection:desc){ id token0{id address name symbol decimals} token1{id address name symbol decimals} timestamp volumeToken0 volumeToken1 numberOfTrades } }";
        let data = self.run_query(Q, Some(json!({ "pair": pair_id, "n": hours }))).await?;
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

        let resp = self.client.post(&self.base_url).json(&body).send().await?;
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

/// Extract and deserialize a named field from a GraphQL response `data` object.
fn parse_field<T: DeserializeOwned>(data: &Value, field: &'static str) -> Result<T, CowError> {
    let val =
        data.get(field).ok_or_else(|| CowError::Parse { field, reason: "field missing".into() })?;
    serde_json::from_value(val.clone())
        .map_err(|e| CowError::Parse { field, reason: e.to_string() })
}
