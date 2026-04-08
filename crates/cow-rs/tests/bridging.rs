#![allow(
    clippy::allow_attributes_without_reason,
    clippy::tests_outside_test_module,
    clippy::doc_markdown,
    clippy::type_complexity,
    clippy::missing_const_for_fn,
    clippy::assertions_on_constants,
    clippy::missing_assert_message,
    clippy::map_err_ignore,
    clippy::deref_by_slicing,
    clippy::redundant_clone,
    clippy::single_match_else,
    clippy::single_match
)]
//! Unit tests for the bridging module.

use alloy_primitives::{U256, address};
use cow_rs::{
    CowError,
    bridging::{
        BridgeProvider, BridgingSdk, QuoteBridgeRequest, QuoteBridgeResponse,
        provider::QuoteFuture, types::BridgeError,
    },
};

fn sample_request() -> QuoteBridgeRequest {
    QuoteBridgeRequest {
        sell_chain_id: 1,
        buy_chain_id: 100,
        sell_token: address!("1111111111111111111111111111111111111111"),
        sell_token_decimals: 18,
        buy_token: address!("2222222222222222222222222222222222222222"),
        buy_token_decimals: 18,
        sell_amount: U256::from(1_000_000_u64),
        account: address!("3333333333333333333333333333333333333333"),
        owner: None,
        receiver: None,
        bridge_recipient: None,
        slippage_bps: 50,
        bridge_slippage_bps: None,
        kind: cow_rs::OrderKind::Sell,
    }
}

// ── SDK construction ──────────────────────────────────────────────────────────

#[test]
fn bridging_sdk_new_has_no_providers() {
    let sdk = BridgingSdk::new();
    assert_eq!(sdk.provider_count(), 0);
}

#[test]
fn bridging_sdk_with_bungee_has_one_provider() {
    let sdk = BridgingSdk::new().with_bungee("test-key");
    assert_eq!(sdk.provider_count(), 1);
}

#[test]
fn bridging_sdk_add_provider_increments_count() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(DummyProvider);
    assert_eq!(sdk.provider_count(), 1);
    sdk.add_provider(DummyProvider);
    assert_eq!(sdk.provider_count(), 2);
}

// ── QuoteBridgeResponse helpers ───────────────────────────────────────────────

#[test]
fn quote_bridge_response_has_bridge_hook_false() {
    let resp = QuoteBridgeResponse {
        provider: "test".to_owned(),
        sell_amount: U256::from(100_u64),
        buy_amount: U256::from(90_u64),
        fee_amount: U256::from(5_u64),
        estimated_secs: 30,
        bridge_hook: None,
    };
    assert!(!resp.has_bridge_hook());
}

#[test]
fn quote_bridge_response_net_buy_amount() {
    let resp = QuoteBridgeResponse {
        provider: "test".to_owned(),
        sell_amount: U256::from(100_u64),
        buy_amount: U256::from(90_u64),
        fee_amount: U256::from(5_u64),
        estimated_secs: 30,
        bridge_hook: None,
    };
    assert_eq!(resp.net_buy_amount(), U256::from(85_u64));
}

#[test]
fn quote_bridge_response_provider_ref() {
    let resp = QuoteBridgeResponse {
        provider: "bungee".to_owned(),
        sell_amount: U256::ZERO,
        buy_amount: U256::ZERO,
        fee_amount: U256::ZERO,
        estimated_secs: 0,
        bridge_hook: None,
    };
    assert_eq!(resp.provider_ref(), "bungee");
}

// ── get_best_quote — no providers ─────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn bridging_sdk_get_best_quote_no_providers_returns_error() {
    let sdk = BridgingSdk::new();
    let result = sdk.get_best_quote(&sample_request()).await;
    assert!(matches!(result, Err(BridgeError::NoProviders)));
}

// ── get_best_quote — all providers fail ───────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn bridging_sdk_get_best_quote_all_fail_returns_no_quote() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(FailingProvider);
    let result = sdk.get_best_quote(&sample_request()).await;
    assert!(matches!(result, Err(BridgeError::NoQuote)));
}

// ── get_all_quotes — returns per-provider results ─────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn bridging_sdk_get_all_quotes_returns_one_per_provider() {
    let mut sdk = BridgingSdk::new();
    sdk.add_provider(DummyProvider);
    let results = sdk.get_all_quotes(&sample_request()).await;
    assert_eq!(results.len(), 1);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// A test provider that always succeeds.
struct DummyProvider;

impl BridgeProvider for DummyProvider {
    fn name(&self) -> &str {
        "dummy"
    }

    fn supports_route(&self, _sell_chain: u64, _buy_chain: u64) -> bool {
        true
    }

    fn get_quote<'a>(&'a self, req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
        let sell_amount = req.sell_amount;
        Box::pin(async move {
            Ok(QuoteBridgeResponse {
                provider: "dummy".to_owned(),
                sell_amount,
                buy_amount: U256::from(900_u64),
                fee_amount: U256::ZERO,
                estimated_secs: 60,
                bridge_hook: None,
            })
        })
    }
}

/// A test provider that always fails.
struct FailingProvider;

impl BridgeProvider for FailingProvider {
    fn name(&self) -> &str {
        "failing"
    }

    fn supports_route(&self, _sell_chain: u64, _buy_chain: u64) -> bool {
        true
    }

    fn get_quote<'a>(&'a self, _req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
        Box::pin(async { Err(CowError::Api { status: 500, body: "error".to_owned() }) })
    }
}

impl std::fmt::Debug for DummyProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DummyProvider")
    }
}

impl std::fmt::Debug for FailingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FailingProvider")
    }
}
