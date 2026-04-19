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
//! SDK. Two specialisations layered on top — `HookBridgeProvider` and
//! `ReceiverAccountBridgeProvider` — will be added in follow-up PRs to
//! cover providers that submit a signed hook vs. providers that redirect
//! funds to a deposit address.

use std::pin::Pin;

use alloy_primitives::{Address, B256};
use alloy_signer_local::PrivateKeySigner;
use cow_chains::SupportedChainId;
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
provider_future!(UnsignedCallFuture, cow_chains::EvmCall);
provider_future!(SignedHookFuture, crate::types::BridgeHook);
provider_future!(ReceiverOverrideFuture, String);
provider_future!(GasEstimationFuture, u64);

// ── Thread-safety marker ──────────────────────────────────────────────────────
//
// On native targets the trait is `Send + Sync` so providers can be shared
// between tasks. On WASM the bounds are dropped because the browser
// `fetch` API is single-threaded. Encoding this via a blanket auto-trait
// lets us keep a single trait definition below (avoids duplicating ~60
// lines of method signatures under two `cfg` branches).

/// Auto-implemented marker adding `Send + Sync` on native and nothing on
/// WASM. Used as a bound on [`BridgeProvider`].
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSendSync: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: ?Sized + Send + Sync> MaybeSendSync for T {}

/// Auto-implemented marker adding `Send + Sync` on native and nothing on
/// WASM. Used as a bound on [`BridgeProvider`].
#[cfg(target_arch = "wasm32")]
pub trait MaybeSendSync {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSendSync for T {}

// ── Trait definition ──────────────────────────────────────────────────────────

/// Trait implemented by cross-chain bridge providers (Across, Bungee,
/// NEAR Intents, …).
///
/// Mirrors the `BridgeProvider<Q>` interface from the `TypeScript` SDK.
/// Concrete providers typically implement one of the two specialisations
/// `HookBridgeProvider` or `ReceiverAccountBridgeProvider` on top of
/// this base trait (both coming in follow-up PRs).
///
/// # Native vs. WASM
///
/// On native targets the trait requires `Send + Sync` so providers can
/// be shared between tasks. On `wasm32` targets those bounds are dropped
/// via [`MaybeSendSync`] because the browser `fetch` API is
/// single-threaded.
pub trait BridgeProvider: MaybeSendSync {
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

// ── Sub-traits ────────────────────────────────────────────────────────────────

/// A [`BridgeProvider`] that triggers the bridge through a signed `CoW` Shed
/// post-hook (e.g. Across, Bungee).
///
/// Mirrors the `HookBridgeProvider<Q>` specialisation of the `TypeScript`
/// SDK. Implementors build an EVM call that the settlement solver
/// executes as a post-interaction, then sign it under the user's
/// `CoW` Shed proxy so the bridge contract can pull the intermediate funds.
///
/// # Required methods
///
/// | Method | Purpose |
/// |---|---|
/// | [`get_unsigned_bridge_call`](Self::get_unsigned_bridge_call) | Build the raw EVM call targeting the bridge contract |
/// | [`get_gas_limit_estimation_for_hook`](Self::get_gas_limit_estimation_for_hook) | Estimate gas without knowing the final amount |
/// | [`get_signed_hook`](Self::get_signed_hook) | Wrap the call in a `CoW` Shed EIP-712 signed hook |
pub trait HookBridgeProvider: BridgeProvider {
    /// Build the unsigned EVM call that initiates the bridge.
    ///
    /// The call is later wrapped into a `CoW` Shed post-hook and signed
    /// with [`get_signed_hook`](Self::get_signed_hook). Mirrors
    /// `getUnsignedBridgeCall(request, quote)` from the `TypeScript` SDK.
    fn get_unsigned_bridge_call<'a>(
        &'a self,
        request: &'a QuoteBridgeRequest,
        quote: &'a QuoteBridgeResponse,
    ) -> UnsignedCallFuture<'a>;

    /// Estimate the gas limit for the bridge post-hook before the final
    /// amount is known.
    ///
    /// Used upstream of the quote to avoid a chicken-and-egg problem
    /// between amount and gas cost. Mirrors
    /// `getGasLimitEstimationForHook(request, extraGas, extraGasProxyCreation)`.
    ///
    /// The default implementation delegates to the free-standing
    /// [`get_gas_limit_estimation_for_hook`](crate::utils::get_gas_limit_estimation_for_hook)
    /// helper; providers can override for route-specific logic (e.g.
    /// Bungee's +350k buffer for mainnet → gnosis).
    fn get_gas_limit_estimation_for_hook<'a>(
        &'a self,
        proxy_deployed: bool,
        extra_gas: Option<u64>,
        extra_gas_proxy_creation: Option<u64>,
    ) -> GasEstimationFuture<'a> {
        let gas = crate::utils::get_gas_limit_estimation_for_hook(
            proxy_deployed,
            extra_gas,
            extra_gas_proxy_creation,
        );
        Box::pin(async move { Ok(gas) })
    }

    /// Produce a signed bridge hook ready to attach to the order's app data.
    ///
    /// Mirrors `getSignedHook(chainId, unsignedCall, bridgeHookNonce, deadline,
    /// hookGasLimit, signer)` from the `TypeScript` SDK. Typically delegates
    /// to the `cow-shed` `sign_hook` helper once PR #5 lands.
    #[allow(clippy::too_many_arguments, reason = "1:1 mirror of the TS signature")]
    fn get_signed_hook<'a>(
        &'a self,
        chain_id: SupportedChainId,
        unsigned_call: &'a cow_chains::EvmCall,
        bridge_hook_nonce: &'a str,
        deadline: u64,
        hook_gas_limit: u64,
        signer: &'a PrivateKeySigner,
    ) -> SignedHookFuture<'a>;
}

/// A [`BridgeProvider`] that relies on a deposit address (e.g. NEAR Intents).
///
/// Mirrors the `ReceiverAccountBridgeProvider<Q>` specialisation of the
/// `TypeScript` SDK. Instead of injecting a post-hook, the provider
/// declares a deposit address that the user swaps into; the bridge
/// detects the deposit off-chain and relays it to the destination chain.
pub trait ReceiverAccountBridgeProvider: BridgeProvider {
    /// Return the deposit address that the `CoW` swap should pay into to
    /// trigger this bridge.
    ///
    /// Mirrors `getBridgeReceiverOverride(quoteRequest, quoteResult)` from
    /// the `TypeScript` SDK.
    fn get_bridge_receiver_override<'a>(
        &'a self,
        quote_request: &'a QuoteBridgeRequest,
        quote_result: &'a QuoteBridgeResponse,
    ) -> ReceiverOverrideFuture<'a>;
}

// ── Type guards ───────────────────────────────────────────────────────────────

/// Returns `true` if the provider's [`BridgeProviderInfo::provider_type`] is
/// [`BridgeProviderType::HookBridgeProvider`](crate::types::BridgeProviderType::HookBridgeProvider).
///
/// Mirrors `isHookBridgeProvider` from the `TypeScript` SDK. Useful for
/// dispatching over a collection of `&dyn BridgeProvider` trait objects
/// without downcasting.
#[must_use]
pub fn is_hook_bridge_provider<P: BridgeProvider + ?Sized>(provider: &P) -> bool {
    provider.info().is_hook_bridge_provider()
}

/// Returns `true` if the provider's [`BridgeProviderInfo::provider_type`] is
/// [`BridgeProviderType::ReceiverAccountBridgeProvider`](crate::types::BridgeProviderType::ReceiverAccountBridgeProvider).
///
/// Mirrors `isReceiverAccountBridgeProvider` from the `TypeScript` SDK.
#[must_use]
pub fn is_receiver_account_bridge_provider<P: BridgeProvider + ?Sized>(provider: &P) -> bool {
    provider.info().is_receiver_account_bridge_provider()
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

    fn sample_request() -> QuoteBridgeRequest {
        QuoteBridgeRequest {
            sell_chain_id: 1,
            buy_chain_id: 10,
            sell_token: Address::ZERO,
            sell_token_decimals: 18,
            buy_token: Address::ZERO,
            buy_token_decimals: 18,
            sell_amount: U256::from(1u64),
            account: Address::ZERO,
            owner: None,
            receiver: None,
            bridge_recipient: None,
            slippage_bps: 50,
            bridge_slippage_bps: None,
            kind: cow_types::OrderKind::Sell,
        }
    }

    #[tokio::test]
    async fn fake_provider_get_intermediate_tokens_is_callable() {
        let provider = FakeProvider { info: fake_info() };
        let tokens = provider.get_intermediate_tokens(&sample_request()).await.unwrap();
        assert!(tokens.is_empty());
    }

    #[tokio::test]
    async fn fake_provider_get_quote_returns_default_fake_response() {
        let provider = FakeProvider { info: fake_info() };
        let response = provider.get_quote(&sample_request()).await.unwrap();
        assert_eq!(response.provider, "fake");
        assert_eq!(response.sell_amount, U256::ZERO);
        assert_eq!(response.buy_amount, U256::ZERO);
    }

    #[tokio::test]
    async fn fake_provider_get_bridging_params_returns_none() {
        let provider = FakeProvider { info: fake_info() };
        let order = cow_orderbook::api::mock_get_order(&format!("0x{}", "aa".repeat(56)));
        let result = provider.get_bridging_params(1, &order, B256::ZERO, None).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fake_provider_get_status_returns_unknown() {
        let provider = FakeProvider { info: fake_info() };
        let result = provider.get_status("deposit", 1).await.unwrap();
        assert_eq!(result.status, BridgeStatus::Unknown);
        assert!(result.fill_tx_hash.is_none());
        assert!(result.deposit_tx_hash.is_none());
    }

    // ── Type guards ─────────────────────────────────────────────────────

    #[test]
    fn is_hook_bridge_provider_matches_info_type() {
        let hook_info = fake_info();
        let hook_provider = FakeProvider { info: hook_info };
        assert!(is_hook_bridge_provider(&hook_provider));
        assert!(!is_receiver_account_bridge_provider(&hook_provider));
    }

    #[test]
    fn is_receiver_account_bridge_provider_matches_info_type() {
        let receiver_info = BridgeProviderInfo {
            name: "rcv".into(),
            logo_url: String::new(),
            dapp_id: "cow-sdk://bridging/providers/rcv".into(),
            website: String::new(),
            provider_type: BridgeProviderType::ReceiverAccountBridgeProvider,
        };
        let provider = FakeProvider { info: receiver_info };
        assert!(is_receiver_account_bridge_provider(&provider));
        assert!(!is_hook_bridge_provider(&provider));
    }

    #[test]
    fn type_guards_work_through_trait_object() {
        let hook_provider: Box<dyn BridgeProvider> = Box::new(FakeProvider { info: fake_info() });
        assert!(is_hook_bridge_provider(&*hook_provider));
        assert!(!is_receiver_account_bridge_provider(&*hook_provider));
    }

    // ── HookBridgeProvider default impl ─────────────────────────────────

    struct FakeHookProvider {
        info: BridgeProviderInfo,
    }

    impl BridgeProvider for FakeHookProvider {
        fn info(&self) -> &BridgeProviderInfo {
            &self.info
        }
        fn supports_route(&self, _s: u64, _b: u64) -> bool {
            true
        }
        fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn get_buy_tokens<'a>(&'a self, _p: BuyTokensParams) -> BuyTokensFuture<'a> {
            let info = self.info.clone();
            Box::pin(
                async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) },
            )
        }
        fn get_intermediate_tokens<'a>(
            &'a self,
            _req: &'a QuoteBridgeRequest,
        ) -> IntermediateTokensFuture<'a> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
            Box::pin(async {
                Ok(QuoteBridgeResponse {
                    provider: "hook".into(),
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
            _c: u64,
            _o: &'a Order,
            _t: B256,
            _s: Option<Address>,
        ) -> BridgingParamsFuture<'a> {
            Box::pin(async { Ok(None) })
        }
        fn get_explorer_url(&self, _id: &str) -> String {
            String::new()
        }
        fn get_status<'a>(&'a self, _id: &'a str, _c: u64) -> BridgeStatusFuture<'a> {
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

    impl HookBridgeProvider for FakeHookProvider {
        fn get_unsigned_bridge_call<'a>(
            &'a self,
            _req: &'a QuoteBridgeRequest,
            _quote: &'a QuoteBridgeResponse,
        ) -> UnsignedCallFuture<'a> {
            Box::pin(async {
                Ok(cow_chains::EvmCall { to: Address::ZERO, data: vec![], value: U256::ZERO })
            })
        }
        fn get_signed_hook<'a>(
            &'a self,
            _chain: SupportedChainId,
            _call: &'a cow_chains::EvmCall,
            _nonce: &'a str,
            _deadline: u64,
            _gas: u64,
            _signer: &'a PrivateKeySigner,
        ) -> SignedHookFuture<'a> {
            Box::pin(async {
                Ok(crate::types::BridgeHook {
                    post_hook: cow_types::CowHook {
                        target: String::new(),
                        call_data: String::new(),
                        gas_limit: String::new(),
                        dapp_id: None,
                    },
                    recipient: String::new(),
                })
            })
        }
    }

    #[tokio::test]
    async fn hook_provider_default_gas_estimation_deployed() {
        let provider = FakeHookProvider { info: fake_info() };
        let gas = provider.get_gas_limit_estimation_for_hook(true, None, None).await.unwrap();
        // Matches the free-standing helper with proxy_deployed = true.
        assert_eq!(gas, crate::utils::get_gas_limit_estimation_for_hook(true, None, None));
    }

    #[tokio::test]
    async fn hook_provider_default_gas_estimation_needs_proxy_creation() {
        let provider = FakeHookProvider { info: fake_info() };
        let gas =
            provider.get_gas_limit_estimation_for_hook(false, None, Some(10_000)).await.unwrap();
        assert_eq!(gas, crate::utils::get_gas_limit_estimation_for_hook(false, None, Some(10_000)));
    }

    #[tokio::test]
    async fn hook_provider_required_methods_callable_through_trait() {
        let provider = FakeHookProvider { info: fake_info() };
        let req = sample_request();
        let quote = provider.get_quote(&req).await.unwrap();
        let call = provider.get_unsigned_bridge_call(&req, &quote).await.unwrap();
        assert_eq!(call.to, Address::ZERO);
        assert!(call.data.is_empty());

        let signer: PrivateKeySigner =
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".parse().unwrap();
        let hook = provider
            .get_signed_hook(SupportedChainId::Mainnet, &call, "0", 0, 0, &signer)
            .await
            .unwrap();
        assert!(hook.recipient.is_empty());
    }

    #[tokio::test]
    async fn fake_hook_provider_bridge_provider_surface_is_callable() {
        let provider = FakeHookProvider { info: fake_info() };
        assert!(provider.supports_route(1, 10));
        assert_eq!(provider.info().dapp_id, "cow-sdk://bridging/providers/fake");
        assert!(provider.get_networks().await.unwrap().is_empty());
        let tokens = provider
            .get_buy_tokens(BuyTokensParams {
                sell_chain_id: 1,
                buy_chain_id: 10,
                sell_token_address: None,
            })
            .await
            .unwrap();
        assert!(tokens.tokens.is_empty());
        assert!(provider.get_intermediate_tokens(&sample_request()).await.unwrap().is_empty());
        let order = cow_orderbook::api::mock_get_order(&format!("0x{}", "aa".repeat(56)));
        assert!(provider.get_bridging_params(1, &order, B256::ZERO, None).await.unwrap().is_none());
        assert!(provider.get_explorer_url("x").is_empty());
        assert_eq!(provider.get_status("x", 1).await.unwrap().status, BridgeStatus::Unknown);
    }

    // ── ReceiverAccountBridgeProvider ───────────────────────────────────

    struct FakeReceiverProvider {
        info: BridgeProviderInfo,
    }

    impl BridgeProvider for FakeReceiverProvider {
        fn info(&self) -> &BridgeProviderInfo {
            &self.info
        }
        fn supports_route(&self, _s: u64, _b: u64) -> bool {
            true
        }
        fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn get_buy_tokens<'a>(&'a self, _p: BuyTokensParams) -> BuyTokensFuture<'a> {
            let info = self.info.clone();
            Box::pin(
                async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) },
            )
        }
        fn get_intermediate_tokens<'a>(
            &'a self,
            _req: &'a QuoteBridgeRequest,
        ) -> IntermediateTokensFuture<'a> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
            Box::pin(async {
                Ok(QuoteBridgeResponse {
                    provider: "rcv".into(),
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
            _c: u64,
            _o: &'a Order,
            _t: B256,
            _s: Option<Address>,
        ) -> BridgingParamsFuture<'a> {
            Box::pin(async { Ok(None) })
        }
        fn get_explorer_url(&self, _id: &str) -> String {
            String::new()
        }
        fn get_status<'a>(&'a self, _id: &'a str, _c: u64) -> BridgeStatusFuture<'a> {
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

    impl ReceiverAccountBridgeProvider for FakeReceiverProvider {
        fn get_bridge_receiver_override<'a>(
            &'a self,
            _req: &'a QuoteBridgeRequest,
            _result: &'a QuoteBridgeResponse,
        ) -> ReceiverOverrideFuture<'a> {
            Box::pin(async { Ok("near-deposit-address".to_owned()) })
        }
    }

    fn fake_receiver_info() -> BridgeProviderInfo {
        BridgeProviderInfo {
            name: "rcv".into(),
            logo_url: String::new(),
            dapp_id: "cow-sdk://bridging/providers/rcv".into(),
            website: String::new(),
            provider_type: BridgeProviderType::ReceiverAccountBridgeProvider,
        }
    }

    #[tokio::test]
    async fn receiver_provider_returns_deposit_address() {
        let provider = FakeReceiverProvider { info: fake_receiver_info() };
        let req = sample_request();
        let quote = provider.get_quote(&req).await.unwrap();
        let addr = provider.get_bridge_receiver_override(&req, &quote).await.unwrap();
        assert_eq!(addr, "near-deposit-address");
    }

    #[tokio::test]
    async fn fake_receiver_provider_bridge_provider_surface_is_callable() {
        let provider = FakeReceiverProvider { info: fake_receiver_info() };
        assert!(provider.supports_route(1, 1_000_000_000));
        assert!(provider.info().is_receiver_account_bridge_provider());
        assert!(provider.get_networks().await.unwrap().is_empty());
        let tokens = provider
            .get_buy_tokens(BuyTokensParams {
                sell_chain_id: 1,
                buy_chain_id: 1_000_000_000,
                sell_token_address: None,
            })
            .await
            .unwrap();
        assert!(tokens.tokens.is_empty());
        assert!(provider.get_intermediate_tokens(&sample_request()).await.unwrap().is_empty());
        let order = cow_orderbook::api::mock_get_order(&format!("0x{}", "bb".repeat(56)));
        assert!(provider.get_bridging_params(1, &order, B256::ZERO, None).await.unwrap().is_none());
        assert!(provider.get_explorer_url("dep").is_empty());
        assert_eq!(provider.get_status("dep", 1).await.unwrap().status, BridgeStatus::Unknown);
    }
}
