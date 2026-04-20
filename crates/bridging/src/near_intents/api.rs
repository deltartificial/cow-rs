//! `NearIntentsApi` — reqwest client for `1click.chaindefuser.com`.
//!
//! Thin HTTP wrapper around the four endpoints the NEAR Intents
//! provider talks to. Keeps **zero** business logic: every method is a
//! one-shot request + serde roundtrip. Error translation is limited to
//! the well-known `"amount is too low"` → [`BridgeError::SellAmountTooSmall`]
//! mapping that mirrors the TS SDK.
//!
//! # Timeouts
//!
//! Per-endpoint timeouts are hard-coded per the NEAR Intents public SLA:
//!
//! | Endpoint | Timeout | Constant |
//! |---|---|---|
//! | `GET /v0/tokens` | 5 s | [`NEAR_INTENTS_DEFAULT_TIMEOUT_MS`] |
//! | `POST /v0/quote` | 15 s | [`NEAR_INTENTS_QUOTE_TIMEOUT_MS`] |
//! | `GET /v0/execution-status/:addr` | 5 s | [`NEAR_INTENTS_DEFAULT_TIMEOUT_MS`] |
//! | `POST /v0/attestation` | 10 s | [`NEAR_INTENTS_ATTESTATION_TIMEOUT_MS`] |
//!
//! # Auth
//!
//! Optional `Authorization: Bearer <token>` header can be set at
//! construction via [`NearIntentsApi::with_api_key`]. The TS SDK
//! applies the header to every endpoint, not just `/attestation`.

use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;

use crate::types::BridgeError;

use super::{
    const_::{
        NEAR_INTENTS_ATTESTATION_TIMEOUT_MS, NEAR_INTENTS_BASE_URL,
        NEAR_INTENTS_DEFAULT_TIMEOUT_MS, NEAR_INTENTS_QUOTE_TIMEOUT_MS,
    },
    types::{
        DefuseToken, NearAttestationRequest, NearAttestationResponse, NearExecutionStatusResponse,
        NearQuoteRequest, NearQuoteResponse,
    },
};

/// HTTP client for the NEAR Intents API.
///
/// Cheap to clone — wraps a shared `reqwest::Client` internally.
///
/// # Example
///
/// ```rust,no_run
/// use cow_bridging::near_intents::api::NearIntentsApi;
///
/// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// let api = NearIntentsApi::new();
/// let tokens = api.get_tokens().await?;
/// let chains: std::collections::HashSet<&str> =
///     tokens.iter().map(|t| t.blockchain.as_str()).collect();
/// println!("{} tokens across {} chains", tokens.len(), chains.len());
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct NearIntentsApi {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl Default for NearIntentsApi {
    fn default() -> Self {
        Self::new()
    }
}

impl NearIntentsApi {
    /// Create a new [`NearIntentsApi`] pointing at the default
    /// production base URL ([`NEAR_INTENTS_BASE_URL`]).
    #[must_use]
    pub fn new() -> Self {
        Self { client: Client::new(), base_url: NEAR_INTENTS_BASE_URL.to_owned(), api_key: None }
    }

    /// Override the base URL — useful for pointing wiremock-backed
    /// tests or staging environments at a custom host.
    ///
    /// The URL is stored verbatim; no trailing-slash normalisation.
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Attach a bearer API key that will be forwarded on every request.
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Return a reference to the configured base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// `GET /v0/tokens` — list all assets supported by the API.
    ///
    /// # Errors
    ///
    /// Returns [`BridgeError::ApiError`] on HTTP failure,
    /// [`BridgeError::InvalidApiResponse`] when the JSON body cannot
    /// be deserialized.
    pub async fn get_tokens(&self) -> Result<Vec<DefuseToken>, BridgeError> {
        let url = format!("{}/v0/tokens", self.base_url);
        let builder =
            self.client.get(url).timeout(Duration::from_millis(NEAR_INTENTS_DEFAULT_TIMEOUT_MS));
        send_and_parse(self.attach_auth_to(builder), "GET /v0/tokens").await
    }

    /// `POST /v0/quote` — request a cross-chain swap quote.
    ///
    /// # Errors
    ///
    /// * [`BridgeError::SellAmountTooSmall`] if the API reports `"amount is too low"`
    ///   (case-insensitive).
    /// * [`BridgeError::ApiError`] on other HTTP failures.
    /// * [`BridgeError::InvalidApiResponse`] on JSON parse errors.
    pub async fn get_quote(
        &self,
        body: &NearQuoteRequest,
    ) -> Result<NearQuoteResponse, BridgeError> {
        let url = format!("{}/v0/quote", self.base_url);
        let builder = self
            .client
            .post(url)
            .timeout(Duration::from_millis(NEAR_INTENTS_QUOTE_TIMEOUT_MS))
            .json(body);
        send_and_parse_with_quote_error_mapping(self.attach_auth_to(builder), "POST /v0/quote")
            .await
    }

    /// `GET /v0/execution-status/{deposit_address}` — poll the
    /// execution status of a swap.
    ///
    /// # Errors
    ///
    /// Returns [`BridgeError::ApiError`] on HTTP failure,
    /// [`BridgeError::InvalidApiResponse`] on JSON parse errors.
    pub async fn get_execution_status(
        &self,
        deposit_address: &str,
    ) -> Result<NearExecutionStatusResponse, BridgeError> {
        let url = format!("{}/v0/execution-status/{deposit_address}", self.base_url);
        let builder =
            self.client.get(url).timeout(Duration::from_millis(NEAR_INTENTS_DEFAULT_TIMEOUT_MS));
        send_and_parse(self.attach_auth_to(builder), "GET /v0/execution-status").await
    }

    /// `POST /v0/attestation` — fetch the attestor's signature over a
    /// `(deposit_address, quote_hash)` pair.
    ///
    /// # Errors
    ///
    /// Returns [`BridgeError::ApiError`] on HTTP failure,
    /// [`BridgeError::InvalidApiResponse`] on JSON parse errors.
    pub async fn get_attestation(
        &self,
        body: &NearAttestationRequest,
    ) -> Result<NearAttestationResponse, BridgeError> {
        let url = format!("{}/v0/attestation", self.base_url);
        let builder = self
            .client
            .post(url)
            .timeout(Duration::from_millis(NEAR_INTENTS_ATTESTATION_TIMEOUT_MS))
            .json(body);
        send_and_parse(self.attach_auth_to(builder), "POST /v0/attestation").await
    }

    fn attach_auth_to(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(key) = &self.api_key { req.bearer_auth(key) } else { req }
    }
}

/// Send a request, map non-2xx → [`BridgeError::ApiError`], and
/// JSON-decode the body into `T`.
async fn send_and_parse<T: DeserializeOwned>(
    req: reqwest::RequestBuilder,
    label: &'static str,
) -> Result<T, BridgeError> {
    let resp = req
        .send()
        .await
        .map_err(|e| BridgeError::ApiError(format!("{label}: transport error: {e}")))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| BridgeError::ApiError(format!("{label}: body read failed: {e}")))?;
    if !status.is_success() {
        return Err(BridgeError::ApiError(format!("{label}: HTTP {status}: {text}")));
    }
    serde_json::from_str::<T>(&text)
        .map_err(|e| BridgeError::InvalidApiResponse(format!("{label}: {e} (body: {text})")))
}

/// Variant of [`send_and_parse`] that maps the `"amount is too low"`
/// error pattern to [`BridgeError::SellAmountTooSmall`] — the only
/// typed error the TS quote flow surfaces.
async fn send_and_parse_with_quote_error_mapping<T: DeserializeOwned>(
    req: reqwest::RequestBuilder,
    label: &'static str,
) -> Result<T, BridgeError> {
    let resp = req
        .send()
        .await
        .map_err(|e| BridgeError::ApiError(format!("{label}: transport error: {e}")))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| BridgeError::ApiError(format!("{label}: body read failed: {e}")))?;
    if !status.is_success() {
        if text.to_lowercase().contains("amount is too low") {
            return Err(BridgeError::SellAmountTooSmall);
        }
        return Err(map_http_error(status, text, label));
    }
    serde_json::from_str::<T>(&text)
        .map_err(|e| BridgeError::InvalidApiResponse(format!("{label}: {e} (body: {text})")))
}

fn map_http_error(status: StatusCode, text: String, label: &'static str) -> BridgeError {
    BridgeError::ApiError(format!("{label}: HTTP {status}: {text}"))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(clippy::tests_outside_test_module, reason = "inner module + cfg guard for WASM test skip")]
mod tests {
    use super::*;

    #[test]
    fn new_uses_default_base_url() {
        let api = NearIntentsApi::new();
        assert_eq!(api.base_url(), NEAR_INTENTS_BASE_URL);
    }

    #[test]
    fn with_base_url_overrides() {
        let api = NearIntentsApi::new().with_base_url("https://example.com");
        assert_eq!(api.base_url(), "https://example.com");
    }

    #[test]
    fn default_matches_new() {
        let a = NearIntentsApi::default();
        let b = NearIntentsApi::new();
        assert_eq!(a.base_url(), b.base_url());
    }

    #[test]
    fn api_key_is_stored() {
        let api = NearIntentsApi::new().with_api_key("super-secret");
        assert_eq!(api.api_key.as_deref(), Some("super-secret"));
    }
}
