//! `CoW` Protocol orderbook HTTP client.
//!
//! Provides [`OrderBookApi`], an async HTTP client wrapping every endpoint of
//! the `CoW` Protocol orderbook REST API (quotes, orders, trades, auctions,
//! solver competitions, app-data, …).
//!
//! Also exports [`request`] for low-level generic HTTP requests and
//! [`mock_get_order`] for testing.

use crate::{
    config::{Env, SupportedChainId, api_base_url, order_explorer_link},
    error::CowError,
    order_book::types::{
        AppDataObject, Auction, CompetitionOrderStatus, GetOrdersRequest, GetTradesRequest, Order,
        OrderCancellations, OrderCreation, OrderQuoteRequest, OrderQuoteResponse, OrderUid,
        SolverCompetition, TotalSurplus, Trade,
    },
};

/// Async HTTP client for the `CoW` Protocol orderbook REST API.
///
/// Wraps a `reqwest::Client` and provides typed methods for every orderbook
/// endpoint (quotes, orders, trades, auctions, solver competitions,
/// app-data). Each method returns a strongly-typed response or a
/// [`CowError`] on failure.
///
/// Instantiate with [`new`](Self::new) (derives the base URL from chain +
/// env) or [`new_with_url`](Self::new_with_url) (explicit URL, useful for
/// tests pointing at a local mock server).
///
/// # Example
///
/// ```rust,no_run
/// use cow_rs::{Env, OrderBookApi, SupportedChainId};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let api = OrderBookApi::new(SupportedChainId::Sepolia, Env::Prod);
/// let version = api.get_version().await?;
/// println!("orderbook version: {version}");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct OrderBookApi {
    client: reqwest::Client,
    base_url: String,
    chain: SupportedChainId,
    env: Env,
}

impl OrderBookApi {
    /// Build a `reqwest::Client` with platform-appropriate settings.
    ///
    /// On native targets a 30-second timeout is applied; on WASM the browser
    /// `fetch` API does not support client-level timeouts so it is omitted.
    #[allow(clippy::shadow_reuse, reason = "builder pattern chains naturally shadow")]
    fn build_client() -> reqwest::Client {
        let builder = reqwest::Client::builder();
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.timeout(std::time::Duration::from_secs(30));
        builder.build().unwrap_or_default()
    }

    /// Create a new client for `chain` in `env`.
    ///
    /// The base URL is derived automatically via
    /// [`api_base_url`]. A 30-second timeout
    /// is applied on native targets; on WASM no timeout is set.
    ///
    /// # Parameters
    ///
    /// * `chain` — the target [`SupportedChainId`].
    /// * `env` — the orderbook [`Env`] (production or staging).
    ///
    /// # Returns
    ///
    /// A new [`OrderBookApi`] instance ready for use.
    ///
    /// # Example
    ///
    /// ```rust
    /// use cow_rs::{Env, OrderBookApi, SupportedChainId};
    ///
    /// let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    /// ```
    #[must_use]
    pub fn new(chain: SupportedChainId, env: Env) -> Self {
        Self { client: Self::build_client(), base_url: api_base_url(chain, env).into(), chain, env }
    }

    /// Create a new client with an explicit `base_url`, overriding the
    /// default derived from `chain` and `env`.
    ///
    /// Useful in tests that point at a local mock server.
    ///
    /// # Parameters
    ///
    /// * `chain` — the target [`SupportedChainId`].
    /// * `env` — the orderbook [`Env`].
    /// * `base_url` — the custom base URL (no trailing slash).
    ///
    /// # Returns
    ///
    /// A new [`OrderBookApi`] using the provided URL.
    ///
    /// # Example
    ///
    /// ```rust
    /// use cow_rs::{Env, OrderBookApi, SupportedChainId};
    ///
    /// let api =
    ///     OrderBookApi::new_with_url(SupportedChainId::Mainnet, Env::Prod, "http://localhost:8080");
    /// ```
    #[must_use]
    pub fn new_with_url(chain: SupportedChainId, env: Env, base_url: impl Into<String>) -> Self {
        Self { client: Self::build_client(), base_url: base_url.into(), chain, env }
    }

    /// `GET /api/v1/version` — return the orderbook service version string.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if the server responds with a non-2xx status,
    /// or [`CowError::Http`] on transport failure.
    pub async fn get_version(&self) -> Result<String, CowError> {
        self.get("/api/v1/version").await
    }

    /// `POST /api/v1/quote` — obtain a price quote for an order.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cow_rs::{Env, OrderBookApi, OrderQuoteRequest, SupportedChainId};
    ///
    /// # async fn example(req: &OrderQuoteRequest) -> Result<(), Box<dyn std::error::Error>> {
    /// let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    /// let quote = api.get_quote(&req).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if the server rejects the quote request (e.g.
    /// unsupported token pair, insufficient liquidity) or responds with a
    /// non-2xx status, or [`CowError::Http`] on transport failure.
    pub async fn get_quote(&self, req: &OrderQuoteRequest) -> Result<OrderQuoteResponse, CowError> {
        self.post("/api/v1/quote", req).await
    }

    /// `POST /api/v1/orders` — submit a signed order.
    ///
    /// Returns the `orderUid` string assigned by the orderbook on success.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cow_rs::{Env, OrderBookApi, OrderCreation, SupportedChainId};
    ///
    /// # async fn example(order: &OrderCreation) -> Result<(), Box<dyn std::error::Error>> {
    /// let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    /// let uid = api.send_order(&order).await?;
    /// assert!(!uid.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if the order is rejected (e.g. invalid
    /// signature, insufficient balance, duplicate order) or the server responds
    /// with a non-2xx status, or [`CowError::Http`] on transport failure.
    pub async fn send_order(&self, order: &OrderCreation) -> Result<String, CowError> {
        let uid: OrderUid = self.post("/api/v1/orders", order).await?;
        Ok(uid.0)
    }

    /// `GET /api/v1/orders/{uid}` — fetch an order by its unique identifier.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cow_rs::{Env, OrderBookApi, SupportedChainId};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    /// let order = api.get_order("0xabc123").await?;
    /// assert!(!order.uid.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if the order is not found (HTTP 404) or the
    /// server responds with a non-2xx status, or [`CowError::Http`] on
    /// transport failure.
    pub async fn get_order(&self, uid: &str) -> Result<Order, CowError> {
        self.get(&format!("/api/v1/orders/{uid}")).await
    }

    /// `DELETE /api/v1/orders` — cancel one or more orders (best-effort).
    ///
    /// Requires a valid EIP-712 or EIP-191 signature from the order owner. Note
    /// that cancellation is best-effort: orders already in an in-flight
    /// settlement transaction may still be executed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cow_rs::{EcdsaSigningScheme, Env, OrderBookApi, OrderCancellations, SupportedChainId};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    /// let cancellation = OrderCancellations {
    ///     order_uids: vec!["0xabc123".to_string()],
    ///     signature: "0x".to_string(),
    ///     signing_scheme: EcdsaSigningScheme::Eip712,
    /// };
    /// api.cancel_orders(&cancellation).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if the cancellation is rejected (e.g. invalid
    /// signature, unknown order UIDs) or the server responds with a non-2xx
    /// status, or [`CowError::Http`] on transport failure.
    pub async fn cancel_orders(&self, body: &OrderCancellations) -> Result<(), CowError> {
        let url = format!("{}/api/v1/orders", self.base_url);
        let resp = self.client.delete(&url).json(body).send().await?;
        if resp.status().is_success() { Ok(()) } else { Err(api_error(resp).await) }
    }

    /// `GET /api/v1/token/{address}/native_price` — native-currency price of a token.
    ///
    /// Returns the price of the given token denominated in the chain's native
    /// currency (e.g. ETH on mainnet).
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if the token is unknown or the server responds
    /// with a non-2xx status, or [`CowError::Http`] on transport failure.
    pub async fn get_native_price(
        &self,
        token: alloy_primitives::Address,
    ) -> Result<f64, CowError> {
        #[derive(serde::Deserialize)]
        struct NativePrice {
            price: f64,
        }
        let r: NativePrice = self.get(&format!("/api/v1/token/{token}/native_price")).await?;
        Ok(r.price)
    }

    /// `GET /api/v1/account/{address}/orders` — list orders for an owner.
    ///
    /// Returns up to `limit` orders (default: 1000) for the given `owner`
    /// address, sorted newest-first.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] on a non-2xx server response, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_orders_for_account(
        &self,
        owner: alloy_primitives::Address,
        limit: Option<u32>,
    ) -> Result<Vec<Order>, CowError> {
        let n = limit.map_or(1000, |v| v);
        self.get(&format!("/api/v1/account/{owner}/orders?limit={n}")).await
    }

    /// `GET /api/v1/account/{owner}/orders` with pagination — flexible order query.
    ///
    /// Mirrors `getOrders` from the `TypeScript` SDK.  Use [`GetOrdersRequest`]
    /// to control `owner`, `limit`, and `offset`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use alloy_primitives::address;
    /// use cow_rs::{Env, GetOrdersRequest, OrderBookApi, SupportedChainId};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    /// let req = GetOrdersRequest {
    ///     owner: address!("1111111111111111111111111111111111111111"),
    ///     limit: Some(10),
    ///     offset: Some(0),
    /// };
    /// let orders = api.get_orders(&req).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] on a non-2xx server response, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_orders(&self, req: &GetOrdersRequest) -> Result<Vec<Order>, CowError> {
        let limit = req.limit.map_or(1000, |v| v);
        let offset = req.offset.map_or(0, |v| v);
        self.get(&format!("/api/v1/account/{}/orders?limit={limit}&offset={offset}", req.owner))
            .await
    }

    /// Fetch an order, trying both production and staging environments.
    ///
    /// If the configured environment returns a 404, the opposite environment is
    /// tried.  Useful during development and integration testing.
    ///
    /// Mirrors `getOrderMultiEnv` from the `TypeScript` SDK.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if the order is not found in either
    /// environment or the server responds with a non-404 error, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_order_multi_env(&self, uid: &str) -> Result<Order, CowError> {
        match self.get_order(uid).await {
            Ok(order) => return Ok(order),
            Err(CowError::Api { status: 404, .. }) => {}
            Err(e) => return Err(e),
        }
        // Flip to the other environment.
        let other_env = if matches!(self.env, Env::Prod) { Env::Staging } else { Env::Prod };
        let other_url = api_base_url(self.chain, other_env);
        let other = Self {
            client: self.client.clone(),
            base_url: other_url.into(),
            chain: self.chain,
            env: other_env,
        };
        other.get_order(uid).await
    }

    /// Return the `CoW` Explorer URL for an order UID.
    ///
    /// Mirrors `getOrderLink` from the `TypeScript` SDK.
    ///
    /// # Example
    ///
    /// ```rust
    /// use cow_rs::{Env, OrderBookApi, SupportedChainId};
    ///
    /// let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    /// let link = api.get_order_link("0xabc");
    /// assert!(link.starts_with("https://explorer.cow.fi/orders/"));
    /// ```
    #[must_use]
    pub fn get_order_link(&self, uid: &str) -> String {
        order_explorer_link(self.chain, uid)
    }

    /// `GET /api/v2/trades` — list trades filtered by owner address.
    ///
    /// Returns up to `limit` trades (default: 10) for the given `owner`
    /// address, sorted by block number and log index descending.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] on a non-2xx server response, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_trades_for_account(
        &self,
        owner: alloy_primitives::Address,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>, CowError> {
        let n = limit.map_or(10, |v| v);
        self.get(&format!("/api/v2/trades?owner={owner}&limit={n}")).await
    }

    /// `GET /api/v2/trades` — list trades filtered by order UID.
    ///
    /// Returns trades matching the given `order_uid` (if provided), or all
    /// recent trades. Because a partially-fillable order may be settled across
    /// multiple transactions, a single `order_uid` can correspond to multiple
    /// trades.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cow_rs::{Env, OrderBookApi, SupportedChainId};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = OrderBookApi::new(SupportedChainId::Mainnet, Env::Prod);
    /// let trades = api.get_trades(Some("0xabc123"), Some(5)).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] on a non-2xx server response, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_trades(
        &self,
        order_uid: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>, CowError> {
        let n = limit.map_or(10, |v| v);
        let path = match order_uid {
            Some(uid) => format!("/api/v2/trades?orderUid={uid}&limit={n}"),
            None => format!("/api/v2/trades?limit={n}"),
        };
        self.get(&path).await
    }

    /// `GET /api/v2/trades` — list trades using a unified [`GetTradesRequest`].
    ///
    /// Filters by `owner`, `order_uid`, or both.  Supports `offset` and `limit`
    /// pagination parameters.  Mirrors `getTrades` from the `TypeScript` SDK.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] on a non-2xx server response, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_trades_with_request(
        &self,
        req: &GetTradesRequest,
    ) -> Result<Vec<Trade>, CowError> {
        let limit = req.limit.map_or(10, |v| v);
        let offset = req.offset.map_or(0, |v| v);
        let mut params = format!("limit={limit}&offset={offset}");
        if let Some(owner) = req.owner {
            params.push_str(&format!("&owner={owner}"));
        }
        if let Some(uid) = &req.order_uid {
            params.push_str(&format!("&orderUid={uid}"));
        }
        self.get(&format!("/api/v2/trades?{params}")).await
    }

    /// `GET /api/v1/auction` — fetch the current batch auction.
    ///
    /// Returns the set of solvable orders and reference token prices that make
    /// up the live auction being solved.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] on a non-2xx server response, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_auction(&self) -> Result<Auction, CowError> {
        self.get("/api/v1/auction").await
    }

    /// `GET /api/v1/solver_competition/{auction_id}` — fetch solver competition
    /// details for a specific auction.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if the auction is not found or the server
    /// responds with a non-2xx status, or [`CowError::Http`] on transport
    /// failure.
    pub async fn get_solver_competition(
        &self,
        auction_id: i64,
    ) -> Result<SolverCompetition, CowError> {
        self.get(&format!("/api/v1/solver_competition/{auction_id}")).await
    }

    /// `GET /api/v1/solver_competition/by_tx_hash/{tx_hash}` — fetch solver
    /// competition details by settlement transaction hash.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if no competition is found for the given
    /// transaction hash or the server responds with a non-2xx status, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_solver_competition_by_tx(
        &self,
        tx_hash: &str,
    ) -> Result<SolverCompetition, CowError> {
        self.get(&format!("/api/v1/solver_competition/by_tx_hash/{tx_hash}")).await
    }

    /// `GET /api/v1/orders/{uid}/status` — competition status of an order.
    ///
    /// Returns the fine-grained lifecycle status of an order within the current
    /// batch auction (open, scheduled, active, solved, executing, traded, cancelled).
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if the order is not found or the server
    /// responds with a non-2xx status, or [`CowError::Http`] on transport
    /// failure.
    pub async fn get_order_status(&self, uid: &str) -> Result<CompetitionOrderStatus, CowError> {
        self.get(&format!("/api/v1/orders/{uid}/status")).await
    }

    /// `GET /api/v1/transactions/{tx_hash}/orders` — orders settled in a transaction.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if the transaction is not found or the server
    /// responds with a non-2xx status, or [`CowError::Http`] on transport
    /// failure.
    pub async fn get_orders_by_tx(&self, tx_hash: &str) -> Result<Vec<Order>, CowError> {
        self.get(&format!("/api/v1/transactions/{tx_hash}/orders")).await
    }

    /// `GET /api/v1/solver_competition/latest` — latest solver competition (v1, deprecated).
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] on a non-2xx server response, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_solver_competition_latest(&self) -> Result<SolverCompetition, CowError> {
        self.get("/api/v1/solver_competition/latest").await
    }

    /// `GET /api/v2/solver_competition/{auction_id}` — solver competition details (v2).
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if the auction is not found or the server
    /// responds with a non-2xx status, or [`CowError::Http`] on transport
    /// failure.
    pub async fn get_solver_competition_v2(
        &self,
        auction_id: i64,
    ) -> Result<SolverCompetition, CowError> {
        self.get(&format!("/api/v2/solver_competition/{auction_id}")).await
    }

    /// `GET /api/v2/solver_competition/by_tx_hash/{tx_hash}` — solver competition by tx (v2).
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if no competition is found for the given
    /// transaction hash or the server responds with a non-2xx status, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_solver_competition_by_tx_v2(
        &self,
        tx_hash: &str,
    ) -> Result<SolverCompetition, CowError> {
        self.get(&format!("/api/v2/solver_competition/by_tx_hash/{tx_hash}")).await
    }

    /// `GET /api/v2/solver_competition/latest` — most recent solver competition (v2).
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] on a non-2xx server response, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_solver_competition_latest_v2(&self) -> Result<SolverCompetition, CowError> {
        self.get("/api/v2/solver_competition/latest").await
    }

    /// `GET /api/v1/users/{address}/total_surplus` — total surplus earned by an address.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] on a non-2xx server response, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_total_surplus(
        &self,
        address: alloy_primitives::Address,
    ) -> Result<TotalSurplus, CowError> {
        self.get(&format!("/api/v1/users/{address}/total_surplus")).await
    }

    /// `GET /api/v1/app_data/{app_data_hash}` — retrieve full app-data for a hash.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if no app-data is registered for the given
    /// hash or the server responds with a non-2xx status, or
    /// [`CowError::Http`] on transport failure.
    pub async fn get_app_data(&self, app_data_hash: &str) -> Result<AppDataObject, CowError> {
        self.get(&format!("/api/v1/app_data/{app_data_hash}")).await
    }

    /// `PUT /api/v1/app_data/{app_data_hash}` — register full app-data for a hash.
    ///
    /// Uploads `full_app_data` and associates it with `app_data_hash`. The hash
    /// must be the `keccak256` of the UTF-8 encoded `full_app_data` string.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] if the hash does not match the data or the
    /// server responds with a non-2xx status, or [`CowError::Http`] on
    /// transport failure.
    pub async fn upload_app_data(
        &self,
        app_data_hash: &str,
        full_app_data: &str,
    ) -> Result<AppDataObject, CowError> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            full_app_data: &'a str,
        }

        let url = format!("{}/api/v1/app_data/{app_data_hash}", self.base_url);
        let resp = self.client.put(&url).json(&Body { full_app_data }).send().await?;
        if resp.status().is_success() {
            Ok(resp.json::<AppDataObject>().await?)
        } else {
            Err(api_error(resp).await)
        }
    }

    /// `PUT /api/v1/app_data` — register full app-data and let the server compute the hash.
    ///
    /// Unlike [`upload_app_data`](Self::upload_app_data), this endpoint derives the
    /// `keccak256` hash server-side and returns it in the response body.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Api`] on a non-2xx server response, or
    /// [`CowError::Http`] on transport failure.
    pub async fn upload_app_data_auto(
        &self,
        full_app_data: &str,
    ) -> Result<AppDataObject, CowError> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            full_app_data: &'a str,
        }

        let url = format!("{}/api/v1/app_data", self.base_url);
        let resp = self.client.put(&url).json(&Body { full_app_data }).send().await?;
        if resp.status().is_success() {
            Ok(resp.json::<AppDataObject>().await?)
        } else {
            Err(api_error(resp).await)
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Send a GET request and deserialize the JSON response.
    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, CowError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.get(&url).send().await?;
        if resp.status().is_success() {
            Ok(resp.json::<T>().await?)
        } else {
            Err(api_error(resp).await)
        }
    }

    /// Send a POST request with a JSON body and deserialize the response.
    async fn post<B, T>(&self, path: &str, body: &B) -> Result<T, CowError>
    where
        B: serde::Serialize,
        T: serde::de::DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.post(&url).json(body).send().await?;
        if resp.status().is_success() {
            Ok(resp.json::<T>().await?)
        } else {
            Err(api_error(resp).await)
        }
    }
}

/// Generic HTTP request helper for the orderbook API.
///
/// Mirrors the `request` function from the `TypeScript` SDK's `order-book` package.
/// Performs a JSON HTTP request with retries and returns the deserialized response.
///
/// For most use cases, prefer the typed methods on [`OrderBookApi`] instead.
///
/// # Errors
///
/// Returns [`CowError::Api`] on non-2xx responses, or [`CowError::Http`] on
/// transport failure.
pub async fn request<T: serde::de::DeserializeOwned>(
    base_url: &str,
    path: &str,
    method: reqwest::Method,
    body: Option<&impl serde::Serialize>,
) -> Result<T, CowError> {
    let client = reqwest::Client::new();
    let url = format!("{base_url}{path}");
    let mut req = client.request(method, &url).header("Accept", "application/json");
    if let Some(b) = body {
        req = req.json(b);
    }
    let resp = req.send().await?;
    if resp.status().is_success() { Ok(resp.json().await?) } else { Err(api_error(resp).await) }
}

/// Return a mock [`Order`] for testing purposes.
///
/// Mirrors `mockGetOrder` from the `TypeScript` SDK's `order-book` package.
/// The returned order has sensible defaults with the given `uid`.
#[must_use]
pub fn mock_get_order(uid: &str) -> Order {
    use crate::{OrderKind, SigningScheme, order_book::types::OrderStatus};
    use alloy_primitives::Address;
    Order {
        uid: uid.to_owned(),
        owner: Address::ZERO,
        creation_date: "2024-01-01T00:00:00Z".to_owned(),
        status: OrderStatus::Open,
        class: None,
        sell_token: Address::ZERO,
        buy_token: Address::ZERO,
        receiver: Some(Address::ZERO),
        sell_amount: "1000000000000000000".to_owned(),
        buy_amount: "900000000000000000".to_owned(),
        valid_to: 1_999_999_999,
        app_data: "0x0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        full_app_data: None,
        fee_amount: "0".to_owned(),
        kind: OrderKind::Sell,
        partially_fillable: false,
        executed_sell_amount: "0".to_owned(),
        executed_buy_amount: "0".to_owned(),
        executed_sell_amount_before_fees: "0".to_owned(),
        executed_fee_amount: "0".to_owned(),
        invalidated: false,
        is_liquidity_order: None,
        signing_scheme: SigningScheme::Eip712,
        signature: "0x".to_owned(),
        interactions: None,
        total_fee: None,
        full_fee_amount: None,
        available_balance: None,
        quote_id: None,
        executed_fee: None,
        ethflow_data: None,
        onchain_order_data: None,
        onchain_user: None,
    }
}

/// Extract a [`CowError::Api`] from a non-success HTTP response.
async fn api_error(resp: reqwest::Response) -> CowError {
    let status = resp.status().as_u16();
    let body = match resp.text().await {
        Ok(text) => text,
        Err(err) => {
            tracing::warn!(%status, %err, "failed to read API error response body");
            String::new()
        }
    };
    CowError::Api { status, body }
}
