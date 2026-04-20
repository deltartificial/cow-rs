#![allow(
    clippy::allow_attributes_without_reason,
    clippy::tests_outside_test_module,
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::assertions_on_constants,
    clippy::missing_assert_message,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::type_complexity
)]
//! Integration tests for [`cow_rs::cross_chain_post::post_cross_chain_order`].
//!
//! These drive the end-to-end hook-branch post flow with a stubbed
//! `HookBridgeProvider` and a wiremock-backed `TradingSdk`, asserting
//! the `SigningStepManager` callbacks fire in the right order on
//! success / failure.

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

#[allow(clippy::disallowed_types, reason = "test recorder only; contention is not a concern")]
use std::sync::Mutex;

use alloy_primitives::{Address, B256, U256};
use alloy_signer_local::PrivateKeySigner;
use cow_app_data::CowHook;
use cow_bridging::{
    SigningStepManager,
    provider::{
        BridgeNetworkInfo, BridgeProvider, BridgeStatusFuture, BridgingParamsFuture,
        BuyTokensFuture, GasEstimationFuture, HookBridgeProvider, IntermediateTokensFuture,
        NetworksFuture, QuoteFuture, SignedHookFuture, UnsignedCallFuture,
    },
    sdk::{BridgeQuoteAndPost, GetQuoteWithBridgeParams, get_quote_with_bridge},
    types::{
        BridgeHook, BridgeProviderInfo, BridgeProviderType, BridgeStatus, BridgeStatusResult,
        BuyTokensParams, GetProviderBuyTokens, IntermediateTokenInfo, QuoteBridgeRequest,
        QuoteBridgeResponse,
    },
};
use cow_chains::{EvmCall, SupportedChainId};
use cow_errors::CowError;
use cow_rs::{
    TradingSdk, TradingSdkConfig,
    cross_chain_post::{PostCrossChainOrderContext, post_cross_chain_order},
    trading_swap_quoter::TradingSwapQuoter,
};
use cow_types::OrderKind;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

const TEST_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const SELL_TOKEN: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
const BUY_TOKEN: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
const USDC_INTERMEDIATE: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

fn hook_info(dapp_id: &str) -> BridgeProviderInfo {
    BridgeProviderInfo {
        name: dapp_id.into(),
        logo_url: String::new(),
        dapp_id: format!("cow-sdk://bridging/providers/{dapp_id}"),
        website: String::new(),
        provider_type: BridgeProviderType::HookBridgeProvider,
    }
}

fn usdc_intermediate() -> IntermediateTokenInfo {
    IntermediateTokenInfo {
        chain_id: 1,
        address: USDC_INTERMEDIATE.parse().unwrap(),
        decimals: 6,
        symbol: "USDC".into(),
        name: "USD Coin".into(),
        logo_url: None,
    }
}

fn sample_request() -> QuoteBridgeRequest {
    QuoteBridgeRequest {
        sell_chain_id: 1,
        buy_chain_id: 42_161,
        sell_token: SELL_TOKEN.parse().unwrap(),
        sell_token_decimals: 18,
        buy_token: BUY_TOKEN.parse().unwrap(),
        buy_token_decimals: 6,
        sell_amount: U256::from(1_000_000u64),
        account: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".parse().unwrap(),
        owner: None,
        receiver: None,
        bridge_recipient: None,
        slippage_bps: 50,
        bridge_slippage_bps: None,
        kind: OrderKind::Sell,
    }
}

/// Hook provider that returns fixed data and optionally fails at specific methods.
struct ScriptedHookProvider {
    info: BridgeProviderInfo,
    fail_signing: AtomicBool,
}

impl ScriptedHookProvider {
    fn new(dapp_id: &str) -> Self {
        Self { info: hook_info(dapp_id), fail_signing: AtomicBool::new(false) }
    }

    fn with_failing_signing(self) -> Self {
        self.fail_signing.store(true, Ordering::SeqCst);
        self
    }
}

impl BridgeProvider for ScriptedHookProvider {
    fn info(&self) -> &BridgeProviderInfo {
        &self.info
    }
    fn supports_route(&self, _s: u64, _b: u64) -> bool {
        true
    }
    fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
        Box::pin(async { Ok(Vec::<BridgeNetworkInfo>::new()) })
    }
    fn get_buy_tokens<'a>(&'a self, _p: BuyTokensParams) -> BuyTokensFuture<'a> {
        let info = self.info.clone();
        Box::pin(async move { Ok(GetProviderBuyTokens { provider_info: info, tokens: vec![] }) })
    }
    fn get_intermediate_tokens<'a>(
        &'a self,
        _req: &'a QuoteBridgeRequest,
    ) -> IntermediateTokensFuture<'a> {
        Box::pin(async { Ok(vec![usdc_intermediate()]) })
    }
    fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
        Box::pin(async {
            Ok(QuoteBridgeResponse {
                provider: "scripted".into(),
                sell_amount: U256::from(1_000_000u64),
                buy_amount: U256::from(998_000u64),
                fee_amount: U256::from(1_500u64),
                estimated_secs: 42,
                bridge_hook: None,
            })
        })
    }
    fn get_bridging_params<'a>(
        &'a self,
        _c: u64,
        _o: &'a cow_orderbook::types::Order,
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
    fn as_hook_bridge_provider(&self) -> Option<&dyn HookBridgeProvider> {
        Some(self)
    }
}

impl HookBridgeProvider for ScriptedHookProvider {
    fn get_unsigned_bridge_call<'a>(
        &'a self,
        _req: &'a QuoteBridgeRequest,
        _quote: &'a QuoteBridgeResponse,
    ) -> UnsignedCallFuture<'a> {
        Box::pin(async {
            Ok(EvmCall {
                to: Address::repeat_byte(0xAC),
                data: vec![0xde, 0xad, 0xbe, 0xef],
                value: U256::ZERO,
            })
        })
    }
    fn get_gas_limit_estimation_for_hook<'a>(
        &'a self,
        _proxy_deployed: bool,
        _extra_gas: Option<u64>,
        _extra_gas_proxy_creation: Option<u64>,
    ) -> GasEstimationFuture<'a> {
        Box::pin(async move { Ok(500_000u64) })
    }
    fn get_signed_hook<'a>(
        &'a self,
        _chain_id: SupportedChainId,
        _unsigned_call: &'a EvmCall,
        _nonce: &'a str,
        _deadline: u64,
        hook_gas_limit: u64,
        _signer: &'a PrivateKeySigner,
    ) -> SignedHookFuture<'a> {
        let fail = self.fail_signing.load(Ordering::SeqCst);
        Box::pin(async move {
            if fail {
                return Err(CowError::Signing("forced signing failure".into()));
            }
            Ok(BridgeHook {
                post_hook: CowHook {
                    call_data: "0xfeedface".into(),
                    gas_limit: hook_gas_limit.to_string(),
                    target: "0x0000000000000000000000000000000000000000".into(),
                    dapp_id: Some("cow-sdk://bridging/providers/".into()),
                },
                recipient: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".into(),
            })
        })
    }
}

fn quote_response_json() -> serde_json::Value {
    serde_json::json!({
        "quote": {
            "sellToken":        SELL_TOKEN,
            "buyToken":         USDC_INTERMEDIATE,
            "receiver":         null,
            "sellAmount":       "1000000",
            "buyAmount":        "500000",
            "validTo":          9_999_999,
            "appData":          "0x0000000000000000000000000000000000000000000000000000000000000000",
            "feeAmount":        "1000",
            "kind":             "sell",
            "partiallyFillable": false,
            "sellTokenBalance": "erc20",
            "buyTokenBalance":  "erc20"
        },
        "from":       "0x0000000000000000000000000000000000000000",
        "expiration": "2099-01-01T00:00:00.000Z",
        "id":         42,
        "verified":   false
    })
}

async fn build_bridge_quote_and_post(
    provider: &ScriptedHookProvider,
    quoter: &TradingSwapQuoter,
) -> BridgeQuoteAndPost {
    let params = GetQuoteWithBridgeParams {
        swap_and_bridge_request: sample_request(),
        slippage_bps: 50,
        advanced_settings_metadata: None,
        quote_signer: None,
        hook_deadline: None,
    };
    get_quote_with_bridge(&params, provider, quoter).await.unwrap()
}

fn make_sdk(server: &MockServer) -> TradingSdk {
    let config = TradingSdkConfig::prod(cow_chains::SupportedChainId::Mainnet, "TestApp");
    TradingSdk::new_with_url(config, TEST_KEY, server.uri()).expect("valid test key")
}

fn order_submit_body() -> String {
    "0x".to_owned() + &"aa".repeat(56)
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn post_cross_chain_order_fires_callbacks_in_order() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_response_json()))
        .mount(&server)
        .await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(order_submit_body()))
        .mount(&server)
        .await;

    let sdk = Arc::new(make_sdk(&server));
    let quoter = TradingSwapQuoter::new(Arc::clone(&sdk));
    let provider = ScriptedHookProvider::new("across");
    let bqp = build_bridge_quote_and_post(&provider, &quoter).await;

    #[allow(clippy::disallowed_types, reason = "test-only recorder")]
    let order: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
    let o1 = Arc::clone(&order);
    let o2 = Arc::clone(&order);
    let o3 = Arc::clone(&order);
    let o4 = Arc::clone(&order);

    let mgr = SigningStepManager::new()
        .with_before_bridging_sign(move || {
            let o = Arc::clone(&o1);
            Box::pin(async move {
                o.lock().unwrap().push("before_bridging_sign");
                Ok(())
            })
        })
        .with_after_bridging_sign(move || {
            let o = Arc::clone(&o2);
            Box::pin(async move {
                o.lock().unwrap().push("after_bridging_sign");
                Ok(())
            })
        })
        .with_before_order_sign(move || {
            let o = Arc::clone(&o3);
            Box::pin(async move {
                o.lock().unwrap().push("before_order_sign");
                Ok(())
            })
        })
        .with_after_order_sign(move || {
            let o = Arc::clone(&o4);
            Box::pin(async move {
                o.lock().unwrap().push("after_order_sign");
                Ok(())
            })
        });

    let hook_signer =
        Arc::new(TEST_KEY.trim_start_matches("0x").parse::<PrivateKeySigner>().unwrap());
    let request = sample_request();
    let ctx = PostCrossChainOrderContext {
        request: &request,
        hook_provider: &provider,
        quote_and_post: &bqp,
        trading_sdk: &sdk,
        hook_signer: &hook_signer,
        hook_deadline: Some(9_999_999),
        advanced_settings: None,
        signing_step_manager: Some(&mgr),
    };

    let result = post_cross_chain_order(ctx).await.unwrap();
    assert!(result.order_id.starts_with("0x"));

    let recorded: Vec<&str> = order.lock().unwrap().clone();
    assert_eq!(
        recorded,
        vec![
            "before_bridging_sign",
            "after_bridging_sign",
            "before_order_sign",
            "after_order_sign",
        ],
    );
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn post_cross_chain_order_fires_on_bridging_sign_error_and_aborts() {
    let server = MockServer::start().await;
    // The initial BridgeQuoteAndPost construction re-quotes an
    // intermediate swap through the wiremock. The post-phase will fail
    // at bridge signing before hitting the orderbook again, so no
    // /orders mock is needed.
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_response_json()))
        .mount(&server)
        .await;
    let sdk = Arc::new(make_sdk(&server));
    let quoter = TradingSwapQuoter::new(Arc::clone(&sdk));

    // First, produce a valid BridgeQuoteAndPost using a **non-failing**
    // provider so the quote phase succeeds.
    let quote_provider = ScriptedHookProvider::new("across");
    let bqp = build_bridge_quote_and_post(&quote_provider, &quoter).await;

    // Now swap in a signing-failing provider for the post phase.
    let post_provider = ScriptedHookProvider::new("across").with_failing_signing();

    let bridging_err_seen = Arc::new(AtomicBool::new(false));
    let order_err_seen = Arc::new(AtomicBool::new(false));
    let bs = Arc::clone(&bridging_err_seen);
    let os = Arc::clone(&order_err_seen);

    let mgr = SigningStepManager::new()
        .with_on_bridging_sign_error(move |_err| {
            bs.store(true, Ordering::SeqCst);
        })
        .with_on_order_sign_error(move |_err| {
            os.store(true, Ordering::SeqCst);
        });

    let hook_signer =
        Arc::new(TEST_KEY.trim_start_matches("0x").parse::<PrivateKeySigner>().unwrap());
    let request = sample_request();
    let ctx = PostCrossChainOrderContext {
        request: &request,
        hook_provider: &post_provider,
        quote_and_post: &bqp,
        trading_sdk: &sdk,
        hook_signer: &hook_signer,
        hook_deadline: None,
        advanced_settings: None,
        signing_step_manager: Some(&mgr),
    };

    let err = post_cross_chain_order(ctx).await.unwrap_err();
    assert!(err.to_string().contains("forced signing failure"), "unexpected: {err}");
    assert!(bridging_err_seen.load(Ordering::SeqCst), "on_bridging_sign_error should fire");
    assert!(!order_err_seen.load(Ordering::SeqCst), "on_order_sign_error should NOT fire");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn post_cross_chain_order_fires_on_order_sign_error_when_post_fails() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_response_json()))
        .mount(&server)
        .await;
    // Return a 400 on order POST to trigger on_order_sign_error.
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad order"))
        .mount(&server)
        .await;

    let sdk = Arc::new(make_sdk(&server));
    let quoter = TradingSwapQuoter::new(Arc::clone(&sdk));
    let provider = ScriptedHookProvider::new("across");
    let bqp = build_bridge_quote_and_post(&provider, &quoter).await;

    let order_err_count = Arc::new(AtomicU8::new(0));
    let c = Arc::clone(&order_err_count);
    let mgr = SigningStepManager::new().with_on_order_sign_error(move |_err| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    let hook_signer =
        Arc::new(TEST_KEY.trim_start_matches("0x").parse::<PrivateKeySigner>().unwrap());
    let request = sample_request();
    let ctx = PostCrossChainOrderContext {
        request: &request,
        hook_provider: &provider,
        quote_and_post: &bqp,
        trading_sdk: &sdk,
        hook_signer: &hook_signer,
        hook_deadline: None,
        advanced_settings: None,
        signing_step_manager: Some(&mgr),
    };

    let err = post_cross_chain_order(ctx).await.unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("400") || err.to_string().contains("bad order")
    );
    assert_eq!(order_err_count.load(Ordering::SeqCst), 1);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn post_cross_chain_order_aborts_when_before_bridging_sign_errors() {
    let server = MockServer::start().await;
    // Only the initial BridgeQuoteAndPost construction hits /quote;
    // the post flow aborts before the re-quote + orders calls.
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_response_json()))
        .mount(&server)
        .await;
    let sdk = Arc::new(make_sdk(&server));
    let quoter = TradingSwapQuoter::new(Arc::clone(&sdk));
    let provider = ScriptedHookProvider::new("across");
    let bqp = build_bridge_quote_and_post(&provider, &quoter).await;

    let mgr = SigningStepManager::new()
        .with_before_bridging_sign(|| Box::pin(async { Err(CowError::Config("stop".into())) }));

    let hook_signer =
        Arc::new(TEST_KEY.trim_start_matches("0x").parse::<PrivateKeySigner>().unwrap());
    let request = sample_request();
    let ctx = PostCrossChainOrderContext {
        request: &request,
        hook_provider: &provider,
        quote_and_post: &bqp,
        trading_sdk: &sdk,
        hook_signer: &hook_signer,
        hook_deadline: None,
        advanced_settings: None,
        signing_step_manager: Some(&mgr),
    };

    let err = post_cross_chain_order(ctx).await.unwrap_err();
    assert!(err.to_string().contains("stop"));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn post_cross_chain_order_rejects_unsupported_sell_chain() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_response_json()))
        .mount(&server)
        .await;

    let sdk = Arc::new(make_sdk(&server));
    let quoter = TradingSwapQuoter::new(Arc::clone(&sdk));
    let provider = ScriptedHookProvider::new("across");
    let bqp = build_bridge_quote_and_post(&provider, &quoter).await;

    let hook_signer =
        Arc::new(TEST_KEY.trim_start_matches("0x").parse::<PrivateKeySigner>().unwrap());
    // Use a chain id that no `SupportedChainId` recognises.
    let mut request = sample_request();
    request.sell_chain_id = 0xDEAD_BEEF;
    let ctx = PostCrossChainOrderContext {
        request: &request,
        hook_provider: &provider,
        quote_and_post: &bqp,
        trading_sdk: &sdk,
        hook_signer: &hook_signer,
        hook_deadline: None,
        advanced_settings: None,
        signing_step_manager: None,
    };

    let err = post_cross_chain_order(ctx).await.unwrap_err();
    assert!(err.to_string().to_lowercase().contains("unsupported"), "unexpected: {err}");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn post_cross_chain_order_threads_caller_advanced_settings() {
    use cow_rs::SwapAdvancedSettings;

    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_response_json()))
        .mount(&server)
        .await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(order_submit_body()))
        .mount(&server)
        .await;

    let sdk = Arc::new(make_sdk(&server));
    let quoter = TradingSwapQuoter::new(Arc::clone(&sdk));
    let provider = ScriptedHookProvider::new("across");
    let bqp = build_bridge_quote_and_post(&provider, &quoter).await;

    let hook_signer =
        Arc::new(TEST_KEY.trim_start_matches("0x").parse::<PrivateKeySigner>().unwrap());
    let settings = SwapAdvancedSettings::default()
        .with_app_data(serde_json::json!({ "metadata": { "partnerFee": { "bps": 25 } } }))
        .with_slippage_bps(123);

    let request = sample_request();
    let ctx = PostCrossChainOrderContext {
        request: &request,
        hook_provider: &provider,
        quote_and_post: &bqp,
        trading_sdk: &sdk,
        hook_signer: &hook_signer,
        hook_deadline: None,
        advanced_settings: Some(&settings),
        signing_step_manager: None,
    };

    let result = post_cross_chain_order(ctx).await.unwrap();
    assert!(result.order_id.starts_with("0x"));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn post_cross_chain_order_errors_when_intermediate_tokens_empty() {
    /// Provider whose `get_intermediate_tokens` returns empty at post time
    /// (but non-empty during quote construction so the upstream
    /// `BridgeQuoteAndPost` can be built).
    struct DrainingProvider {
        info: BridgeProviderInfo,
        first_call: AtomicBool,
    }
    impl BridgeProvider for DrainingProvider {
        fn info(&self) -> &BridgeProviderInfo {
            &self.info
        }
        fn supports_route(&self, _s: u64, _b: u64) -> bool {
            true
        }
        fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
            Box::pin(async { Ok(Vec::<BridgeNetworkInfo>::new()) })
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
            // The quote phase uses a different provider (ScriptedHookProvider);
            // this one is only invoked at post time, where we want it to
            // return empty so resolve_intermediate_token errors.
            let _ = self.first_call.load(Ordering::SeqCst);
            Box::pin(async { Ok(Vec::<IntermediateTokenInfo>::new()) })
        }
        fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
            Box::pin(async {
                Ok(QuoteBridgeResponse {
                    provider: "draining".into(),
                    sell_amount: U256::from(1_000_000u64),
                    buy_amount: U256::from(998_000u64),
                    fee_amount: U256::from(1_500u64),
                    estimated_secs: 42,
                    bridge_hook: None,
                })
            })
        }
        fn get_bridging_params<'a>(
            &'a self,
            _c: u64,
            _o: &'a cow_orderbook::types::Order,
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
        fn as_hook_bridge_provider(&self) -> Option<&dyn HookBridgeProvider> {
            Some(self)
        }
    }
    impl HookBridgeProvider for DrainingProvider {
        fn get_unsigned_bridge_call<'a>(
            &'a self,
            _req: &'a QuoteBridgeRequest,
            _quote: &'a QuoteBridgeResponse,
        ) -> UnsignedCallFuture<'a> {
            Box::pin(async {
                Ok(EvmCall {
                    to: Address::repeat_byte(0xAC),
                    data: vec![0xde, 0xad],
                    value: U256::ZERO,
                })
            })
        }
        fn get_gas_limit_estimation_for_hook<'a>(
            &'a self,
            _proxy_deployed: bool,
            _extra_gas: Option<u64>,
            _extra_gas_proxy_creation: Option<u64>,
        ) -> GasEstimationFuture<'a> {
            Box::pin(async move { Ok(500_000u64) })
        }
        fn get_signed_hook<'a>(
            &'a self,
            _chain_id: SupportedChainId,
            _unsigned_call: &'a EvmCall,
            _nonce: &'a str,
            _deadline: u64,
            hook_gas_limit: u64,
            _signer: &'a PrivateKeySigner,
        ) -> SignedHookFuture<'a> {
            Box::pin(async move {
                Ok(BridgeHook {
                    post_hook: CowHook {
                        call_data: "0xfeedface".into(),
                        gas_limit: hook_gas_limit.to_string(),
                        target: "0x0000000000000000000000000000000000000000".into(),
                        dapp_id: Some("cow-sdk://bridging/providers/".into()),
                    },
                    recipient: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".into(),
                })
            })
        }
    }

    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_response_json()))
        .mount(&server)
        .await;

    let sdk = Arc::new(make_sdk(&server));
    let quoter = TradingSwapQuoter::new(Arc::clone(&sdk));
    let provider =
        DrainingProvider { info: hook_info("draining"), first_call: AtomicBool::new(true) };
    // Build BridgeQuoteAndPost via a separate always-full provider so the
    // quote construction succeeds, then pivot to the draining one for post.
    let quote_provider = ScriptedHookProvider::new("draining");
    let bqp = build_bridge_quote_and_post(&quote_provider, &quoter).await;

    let hook_signer =
        Arc::new(TEST_KEY.trim_start_matches("0x").parse::<PrivateKeySigner>().unwrap());
    let request = sample_request();
    let ctx = PostCrossChainOrderContext {
        request: &request,
        hook_provider: &provider,
        quote_and_post: &bqp,
        trading_sdk: &sdk,
        hook_signer: &hook_signer,
        hook_deadline: None,
        advanced_settings: None,
        signing_step_manager: None,
    };

    let err = post_cross_chain_order(ctx).await.unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("no intermediate tokens") ||
            err.to_string().contains("intermediate"),
        "unexpected: {err}"
    );
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn post_cross_chain_order_works_without_signing_step_manager() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(quote_response_json()))
        .mount(&server)
        .await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(order_submit_body()))
        .mount(&server)
        .await;

    let sdk = Arc::new(make_sdk(&server));
    let quoter = TradingSwapQuoter::new(Arc::clone(&sdk));
    let provider = ScriptedHookProvider::new("across");
    let bqp = build_bridge_quote_and_post(&provider, &quoter).await;

    let hook_signer =
        Arc::new(TEST_KEY.trim_start_matches("0x").parse::<PrivateKeySigner>().unwrap());
    let request = sample_request();
    let ctx = PostCrossChainOrderContext {
        request: &request,
        hook_provider: &provider,
        quote_and_post: &bqp,
        trading_sdk: &sdk,
        hook_signer: &hook_signer,
        hook_deadline: None,
        advanced_settings: None,
        signing_step_manager: None,
    };

    let result = post_cross_chain_order(ctx).await.unwrap();
    assert!(result.order_id.starts_with("0x"));
}
