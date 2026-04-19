//! [`BridgeProvider`] trait for bridge integrations.
//!
//! Providers that implement this trait expose the full surface needed by the
//! [`BridgingSdk`](crate::sdk::BridgingSdk) to orchestrate cross-chain
//! trades: discovery (`info`, `get_networks`, `get_buy_tokens`,
//! `get_intermediate_tokens`), quoting (`get_quote`), routing (`supports_route`),
//! and post-settlement observability (`get_bridging_params`, `get_status`,
//! `get_explorer_url`).
//!
//! The trait mirrors the `BridgeProvider<Q>` interface from the `TypeScript`
//! SDK. Two specialisations layered on top — [`HookBridgeProvider`] and
//! [`ReceiverAccountBridgeProvider`] — will be added in follow-up PRs to
//! cover providers that submit a signed hook vs. providers that redirect
//! funds to a deposit address.

use std::pin::Pin;

use alloy_primitives::{Address, B256};
use cow_errors::CowError;
use cow_orderbook::types::Order;

use super::types::{
    BridgeProviderInfo, BridgeStatusResult, BridgingDepositParams, BuyTokensParams,
    GetProviderBuyTokens, IntermediateTokenInfo, QuoteBridgeRequest, QuoteBridgeResponse,
};

/// Deposit parameters and bridge status returned by
/// [`BridgeProvider::get_bridging_params`].
///
/// Mirrors the `{ params, status }` tuple returned by the `TypeScript`
/// helper of the same name.
#[derive(Debug, Clone)]
pub struct BridgingParamsResult {
    /// Decoded deposit parameters.
    pub params: BridgingDepositParams,
    /// Bridge status at the time of decoding.
    pub status: BridgeStatusResult,
}

/// Network / chain metadata returned by [`BridgeProvider::get_networks`].
///
/// Kept minimal on purpose — providers return the chain ID plus optional
/// display metadata; consumers can resolve richer [`cow_chains::ChainInfo`]
/// from the ID if they need it.
#[derive(Debug, Clone)]
pub struct BridgeNetworkInfo {
    /// Chain ID.
    pub chain_id: u64,
    /// Display name (e.g. `"Ethereum"`).
    pub name: String,
    /// Logo URL, if available.
    pub logo_url: Option<String>,
}

// ── Future type aliases ───────────────────────────────────────────────────────
//
// Each public method that performs I/O returns a pinned, boxed future. On
// native targets the future is `Send` so it can be spawned across tasks;
// on WASM it is not, matching the browser `fetch` API.

macro_rules! provider_future {
    ($name:ident, $output:ty) => {
        #[cfg(not(target_arch = "wasm32"))]
        #[doc = concat!("Future returned by `BridgeProvider::", stringify!($name), "`.")]
        pub type $name<'a> =
            Pin<Box<dyn std::future::Future<Output = Result<$output, CowError>> + Send + 'a>>;

        #[cfg(target_arch = "wasm32")]
        #[doc = concat!("Future returned by `BridgeProvider::", stringify!($name), "`.")]
        pub type $name<'a> =
            Pin<Box<dyn std::future::Future<Output = Result<$output, CowError>> + 'a>>;
    };
}

provider_future!(QuoteFuture, QuoteBridgeResponse);
provider_future!(NetworksFuture, Vec<BridgeNetworkInfo>);
provider_future!(BuyTokensFuture, GetProviderBuyTokens);
provider_future!(IntermediateTokensFuture, Vec<IntermediateTokenInfo>);
provider_future!(BridgingParamsFuture, Option<BridgingParamsResult>);
provider_future!(BridgeStatusFuture, BridgeStatusResult);

// ── Trait definition ──────────────────────────────────────────────────────────

/// Trait implemented by cross-chain bridge providers (Across, Bungee,
/// NEAR Intents, …).
///
/// Mirrors the `BridgeProvider<Q>` interface from the `TypeScript` SDK.
/// Concrete providers typically implement one of the two specialisations
/// [`HookBridgeProvider`] or [`ReceiverAccountBridgeProvider`] on top of
/// this base trait.
///
/// # Native vs. WASM
///
/// On native targets the trait requires `Send + Sync` so providers can
/// be shared between tasks. On `wasm32` targets the bounds are dropped
/// because the browser `fetch` API is single-threaded.
#[cfg(not(target_arch = "wasm32"))]
pub trait BridgeProvider: Send + Sync {
    /// Metadata about this provider (name, logo, dApp ID, …).
    ///
    /// Mirrors the `info` field on the `TypeScript` interface.
    fn info(&self) -> &BridgeProviderInfo;

    /// A short identifier for this provider (e.g. `"bungee"`).
    ///
    /// Default implementation delegates to [`info().name`](BridgeProviderInfo::name).
    fn name(&self) -> &str {
        &self.info().name
    }

    /// Returns `true` if this provider supports the given route.
    ///
    /// # Arguments
    ///
    /// * `sell_chain` — chain ID of the source (sell) chain.
    /// * `buy_chain` — chain ID of the destination (buy) chain.
    fn supports_route(&self, sell_chain: u64, buy_chain: u64) -> bool;

    /// List the networks (source or destination) supported by this provider.
    ///
    /// Mirrors `getNetworks()` from the `TypeScript` SDK.
    fn get_networks<'a>(&'a self) -> NetworksFuture<'a>;

    /// List the tokens this provider can deliver on a destination chain.
    ///
    /// Mirrors `getBuyTokens(params)` from the `TypeScript` SDK.
    fn get_buy_tokens<'a>(&'a self, params: BuyTokensParams) -> BuyTokensFuture<'a>;

    /// List candidate intermediate tokens for a bridging request.
    ///
    /// Used by the orchestration layer to pick the best hop token before
    /// asking `TradingSdk` for a swap quote. Mirrors
    /// `getIntermediateTokens(request)` from the `TypeScript` SDK.
    fn get_intermediate_tokens<'a>(
        &'a self,
        request: &'a QuoteBridgeRequest,
    ) -> IntermediateTokensFuture<'a>;

    /// Fetch a bridge quote for `req`.
    ///
    /// Returns a pinned, boxed future that resolves to a
    /// [`QuoteBridgeResponse`] on success, or a [`CowError`] if the provider
    /// is unreachable or the route is unsupported.
    fn get_quote<'a>(&'a self, req: &'a QuoteBridgeRequest) -> QuoteFuture<'a>;

    /// Reconstruct bridging deposit parameters from a settlement transaction.
    ///
    /// Given a chain, a fully-fetched [`Order`] (including executed amounts,
    /// equivalent to `EnrichedOrder` on the `TypeScript` side), and the
    /// settlement transaction hash, the provider scans the logs and
    /// rebuilds a [`BridgingDepositParams`] alongside the current bridge
    /// status.
    ///
    /// Returns `Ok(None)` if the transaction does not contain a deposit
    /// attributable to this provider.
    ///
    /// The `settlement_override` argument mirrors the upstream
    /// `settlementContractOverride` parameter (cow-sdk#807) — callers can
    /// inject a custom settlement contract address for chains where the
    /// default is not deployed.
    fn get_bridging_params<'a>(
        &'a self,
        chain_id: u64,
        order: &'a Order,
        tx_hash: B256,
        settlement_override: Option<Address>,
    ) -> BridgingParamsFuture<'a>;

    /// Return the provider's explorer URL for a given bridging ID.
    ///
    /// Mirrors `getExplorerUrl(bridgingId)` from the `TypeScript` SDK.
    fn get_explorer_url(&self, bridging_id: &str) -> String;

    /// Fetch the current bridge status for a given `bridging_id`.
    ///
    /// Mirrors `getStatus(bridgingId, originChainId)` from the `TypeScript`
    /// SDK.
    fn get_status<'a>(
        &'a self,
        bridging_id: &'a str,
        origin_chain_id: u64,
    ) -> BridgeStatusFuture<'a>;
}

/// Trait implemented by cross-chain bridge providers (Across, Bungee,
/// NEAR Intents, …).
///
/// WASM variant — see the native variant above for the full documentation.
/// The trait bounds (`Send + Sync`) are dropped because the browser
/// `fetch` API is single-threaded.
#[cfg(target_arch = "wasm32")]
pub trait BridgeProvider {
    /// Metadata about this provider.
    fn info(&self) -> &BridgeProviderInfo;

    /// A short identifier for this provider. Default: delegates to `info().name`.
    fn name(&self) -> &str {
        &self.info().name
    }

    /// Returns `true` if this provider supports the given route.
    fn supports_route(&self, sell_chain: u64, buy_chain: u64) -> bool;

    /// List the networks supported by this provider.
    fn get_networks<'a>(&'a self) -> NetworksFuture<'a>;

    /// List the tokens this provider can deliver on a destination chain.
    fn get_buy_tokens<'a>(&'a self, params: BuyTokensParams) -> BuyTokensFuture<'a>;

    /// List candidate intermediate tokens for a bridging request.
    fn get_intermediate_tokens<'a>(
        &'a self,
        request: &'a QuoteBridgeRequest,
    ) -> IntermediateTokensFuture<'a>;

    /// Fetch a bridge quote for `req`.
    fn get_quote<'a>(&'a self, req: &'a QuoteBridgeRequest) -> QuoteFuture<'a>;

    /// Reconstruct bridging deposit parameters from a settlement transaction.
    fn get_bridging_params<'a>(
        &'a self,
        chain_id: u64,
        order: &'a Order,
        tx_hash: B256,
        settlement_override: Option<Address>,
    ) -> BridgingParamsFuture<'a>;

    /// Return the provider's explorer URL for a given bridging ID.
    fn get_explorer_url(&self, bridging_id: &str) -> String;

    /// Fetch the current bridge status for a given `bridging_id`.
    fn get_status<'a>(
        &'a self,
        bridging_id: &'a str,
        origin_chain_id: u64,
    ) -> BridgeStatusFuture<'a>;
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(clippy::tests_outside_test_module, reason = "inner module + cfg guard for WASM test skip")]
mod tests {
    use alloy_primitives::U256;

    use crate::types::{BridgeProviderType, BridgeStatus};

    use super::*;

    // ── BridgeNetworkInfo ───────────────────────────────────────────────

    #[test]
    fn bridge_network_info_holds_chain_metadata() {
        let info = BridgeNetworkInfo {
            chain_id: 1,
            name: "Ethereum".into(),
            logo_url: Some("https://example.com/eth.png".into()),
        };
        assert_eq!(info.chain_id, 1);
        assert_eq!(info.name, "Ethereum");
        assert!(info.logo_url.is_some());
    }

    #[test]
    fn bridge_network_info_logo_optional() {
        let info = BridgeNetworkInfo { chain_id: 100, name: "Gnosis".into(), logo_url: None };
        assert!(info.logo_url.is_none());
    }

    // ── BridgingParamsResult ────────────────────────────────────────────

    #[test]
    fn bridging_params_result_bundles_params_and_status() {
        let params = BridgingDepositParams {
            input_token_address: Address::ZERO,
            output_token_address: Address::ZERO,
            input_amount: U256::from(1000u64),
            output_amount: None,
            owner: Address::ZERO,
            quote_timestamp: None,
            fill_deadline: None,
            recipient: Address::ZERO,
            source_chain_id: 1,
            destination_chain_id: 10,
            bridging_id: "abc".into(),
        };
        let status = BridgeStatusResult {
            status: BridgeStatus::InProgress,
            fill_time_in_seconds: None,
            deposit_tx_hash: None,
            fill_tx_hash: None,
        };
        let bundle = BridgingParamsResult { params, status };
        assert_eq!(bundle.params.bridging_id, "abc");
        assert_eq!(bundle.status.status, BridgeStatus::InProgress);
    }

    // ── Trait default impl coverage ─────────────────────────────────────

    struct FakeProvider {
        info: BridgeProviderInfo,
    }

    impl BridgeProvider for FakeProvider {
        fn info(&self) -> &BridgeProviderInfo {
            &self.info
        }
        fn supports_route(&self, _sell: u64, _buy: u64) -> bool {
            true
        }
        fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn get_buy_tokens<'a>(&'a self, _params: BuyTokensParams) -> BuyTokensFuture<'a> {
            let info = self.info.clone();
            Box::pin(
                async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) },
            )
        }
        fn get_intermediate_tokens<'a>(
            &'a self,
            _request: &'a QuoteBridgeRequest,
        ) -> IntermediateTokensFuture<'a> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
            Box::pin(async {
                Ok(QuoteBridgeResponse {
                    provider: "fake".into(),
                    sell_amount: U256::ZERO,
                    buy_amount: U256::ZERO,
                    fee_amount: U256::ZERO,
                    estimated_secs: 0,
                    bridge_hook: None,
                })
            })
        }
        fn get_bridging_params<'a>(
            &'a self,
            _chain_id: u64,
            _order: &'a cow_orderbook::types::Order,
            _tx_hash: B256,
            _settlement_override: Option<Address>,
        ) -> BridgingParamsFuture<'a> {
            Box::pin(async { Ok(None) })
        }
        fn get_explorer_url(&self, bridging_id: &str) -> String {
            format!("https://example.com/{bridging_id}")
        }
        fn get_status<'a>(
            &'a self,
            _bridging_id: &'a str,
            _origin_chain_id: u64,
        ) -> BridgeStatusFuture<'a> {
            Box::pin(async {
                Ok(BridgeStatusResult {
                    status: BridgeStatus::Unknown,
                    fill_time_in_seconds: None,
                    deposit_tx_hash: None,
                    fill_tx_hash: None,
                })
            })
        }
    }

    fn fake_info() -> BridgeProviderInfo {
        BridgeProviderInfo {
            name: "fake-provider".into(),
            logo_url: "https://example.com/logo.svg".into(),
            dapp_id: "cow-sdk://bridging/providers/fake".into(),
            website: "https://example.com".into(),
            provider_type: BridgeProviderType::HookBridgeProvider,
        }
    }

    #[test]
    fn default_name_delegates_to_info() {
        let provider = FakeProvider { info: fake_info() };
        assert_eq!(provider.name(), "fake-provider");
        assert_eq!(provider.name(), provider.info().name.as_str());
    }

    #[test]
    fn default_explorer_url_composes_path() {
        let provider = FakeProvider { info: fake_info() };
        assert_eq!(provider.get_explorer_url("deposit-42"), "https://example.com/deposit-42");
    }

    #[tokio::test]
    async fn trait_object_dispatch_works_with_dyn() {
        let provider: Box<dyn BridgeProvider> = Box::new(FakeProvider { info: fake_info() });
        assert!(provider.supports_route(1, 10));
        assert_eq!(provider.info().dapp_id, "cow-sdk://bridging/providers/fake");
        let networks = provider.get_networks().await.unwrap();
        assert!(networks.is_empty());
        let tokens = provider
            .get_buy_tokens(BuyTokensParams {
                sell_chain_id: 1,
                buy_chain_id: 100,
                sell_token_address: None,
            })
            .await
            .unwrap();
        assert!(tokens.tokens.is_empty());
        assert_eq!(tokens.provider_info.name, "fake-provider");
    }
}
