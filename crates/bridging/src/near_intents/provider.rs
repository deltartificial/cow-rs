//! [`NearIntentsBridgeProvider`] — bridge provider implementation for
//! NEAR Intents (`1click.chaindefuser.com`).
//!
//! NEAR Intents is a **receiver-account** bridge: the `CoW` order
//! transfers the swap output to a `depositAddress` the NEAR API
//! allocates, and NEAR relays it onward. There is no on-chain
//! post-hook — unlike Across / Bungee this provider implements
//! [`ReceiverAccountBridgeProvider`], not
//! [`crate::provider::HookBridgeProvider`].
//!
//! The provider supports 11 chains total (9 EVM + BTC + SOL) — see
//! [`super::util::blockchain_key_to_chain_id`].
//!
//! # Attestation verification
//!
//! On every `get_quote` the provider calls `/v0/attestation` to
//! receive an attestor-signed message, then:
//! 1. SHA-256-hashes the canonical quote payload ([`super::util::hash_quote_payload`]).
//! 2. Rebuilds the signed message (`prefix ‖ version ‖ depositAddress ‖ quoteHash`).
//! 3. Keccak-256 + ecrecover against [`cow_primitives::ATTESTATOR_ADDRESS`].
//!
//! If the recovered address doesn't match, the quote is rejected with
//! [`BridgeError::QuoteDoesNotMatchDepositAddress`] — this is the
//! non-negotiable integrity check that stops a compromised relayer
//! from redirecting funds.

use std::str::FromStr;

use alloy_primitives::{Address, B256};
use cow_chains::SupportedChainId;
use cow_errors::CowError;
use cow_orderbook::types::Order;
use cow_primitives::ATTESTATOR_ADDRESS;

use crate::{
    provider::{
        BridgeNetworkInfo, BridgeProvider, BridgeStatusFuture, BridgingParamsFuture,
        BridgingParamsResult, BuyTokensFuture, IntermediateTokensFuture, NetworksFuture,
        QuoteFuture, ReceiverAccountBridgeProvider, ReceiverOverrideFuture,
    },
    types::{
        BridgeError, BridgeProviderInfo, BridgeProviderType, BridgeStatus, BridgeStatusResult,
        BuyTokensParams, GetProviderBuyTokens, IntermediateTokenInfo, QuoteBridgeRequest,
        QuoteBridgeResponse,
    },
};

use super::{
    NEAR_INTENTS_HOOK_DAPP_ID,
    api::NearIntentsApi,
    types::{
        DefuseToken, NearAttestationRequest, NearDepositMode, NearDepositType, NearExecutionStatus,
        NearQuoteRequest, NearRecipientType, NearRefundType, NearSwapType,
    },
    util::{
        adapt_tokens, blockchain_key_to_chain_id, calculate_deadline, hash_quote_payload,
        recover_attestation,
    },
};

/// Default quote validity (15 minutes) — used when the caller doesn't
/// override it.
pub const NEAR_INTENTS_DEFAULT_VALIDITY_SECS: u64 = 15 * 60;

// ── Options ──────────────────────────────────────────────────────────────

/// Construction options for [`NearIntentsBridgeProvider`].
#[derive(Debug, Clone)]
pub struct NearIntentsProviderOptions {
    /// Optional bearer API key (forwarded on every request).
    pub api_key: Option<String>,
    /// Override the default base URL — useful for wiremock / staging.
    pub base_url: Option<String>,
    /// Custom attestor address (defaults to
    /// [`cow_primitives::ATTESTATOR_ADDRESS`]). Override only for tests
    /// that use locally-generated signatures.
    pub attestator_address: Address,
    /// Quote validity in seconds (defaults to
    /// [`NEAR_INTENTS_DEFAULT_VALIDITY_SECS`]).
    pub validity_secs: u64,
}

impl Default for NearIntentsProviderOptions {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: None,
            attestator_address: ATTESTATOR_ADDRESS,
            validity_secs: NEAR_INTENTS_DEFAULT_VALIDITY_SECS,
        }
    }
}

// ── Provider ─────────────────────────────────────────────────────────────

/// NEAR Intents bridge provider.
///
/// Cheap to clone — wraps a shared [`NearIntentsApi`] under the hood.
#[derive(Clone, Debug)]
pub struct NearIntentsBridgeProvider {
    info: BridgeProviderInfo,
    api: NearIntentsApi,
    options: NearIntentsProviderOptions,
}

impl NearIntentsBridgeProvider {
    /// Construct a new provider with the given options.
    ///
    /// Uses the default production base URL unless overridden.
    #[must_use]
    pub fn new(options: NearIntentsProviderOptions) -> Self {
        let mut api = NearIntentsApi::new();
        if let Some(key) = &options.api_key {
            api = api.with_api_key(key.clone());
        }
        if let Some(url) = &options.base_url {
            api = api.with_base_url(url.clone());
        }
        Self { info: default_info(), api, options }
    }

    /// Return a reference to the underlying HTTP client — primarily
    /// useful for tests that want to prime the API with wiremock
    /// fixtures.
    #[must_use]
    pub const fn api(&self) -> &NearIntentsApi {
        &self.api
    }

    /// Return a reference to the provider's configuration.
    #[must_use]
    pub const fn options(&self) -> &NearIntentsProviderOptions {
        &self.options
    }
}

impl Default for NearIntentsBridgeProvider {
    fn default() -> Self {
        Self::new(NearIntentsProviderOptions::default())
    }
}

/// Canonical [`BridgeProviderInfo`] for NEAR Intents.
///
/// Mirrors the TS `defaultNearIntentsInfo`. Exposed publicly so tests
/// and downstream tooling can compare against the well-known dApp ID
/// without instantiating the full provider.
#[must_use]
pub fn default_near_intents_info() -> BridgeProviderInfo {
    default_info()
}

fn default_info() -> BridgeProviderInfo {
    BridgeProviderInfo {
        name: "near-intents".into(),
        logo_url: "https://files.cow.fi/cow-sdk/bridging/providers/near-intents/logo.png".into(),
        dapp_id: NEAR_INTENTS_HOOK_DAPP_ID.into(),
        website: "https://near-intents.org".into(),
        provider_type: BridgeProviderType::ReceiverAccountBridgeProvider,
    }
}

/// List of chain IDs NEAR Intents can bridge to / from.
#[must_use]
pub fn near_intents_supported_chains() -> Vec<u64> {
    vec![
        1,             // Ethereum
        42_161,        // Arbitrum
        43_114,        // Avalanche
        8_453,         // Base
        56,            // BSC
        100,           // Gnosis
        10,            // Optimism
        137,           // Polygon
        9_745,         // Plasma (workspace chain-id; see SupportedChainId::Plasma)
        1_000_000_000, // Bitcoin
        1_000_000_001, // Solana
    ]
}

impl BridgeProvider for NearIntentsBridgeProvider {
    fn info(&self) -> &BridgeProviderInfo {
        &self.info
    }

    fn supports_route(&self, sell_chain: u64, buy_chain: u64) -> bool {
        if sell_chain == buy_chain {
            return false;
        }
        let supported = near_intents_supported_chains();
        supported.contains(&sell_chain) && supported.contains(&buy_chain)
    }

    fn get_networks<'a>(&'a self) -> NetworksFuture<'a> {
        Box::pin(async move {
            let names = [
                (1, "Ethereum"),
                (42_161, "Arbitrum One"),
                (43_114, "Avalanche"),
                (8_453, "Base"),
                (56, "BSC"),
                (100, "Gnosis"),
                (10, "Optimism"),
                (137, "Polygon"),
                (9_745, "Plasma"),
                (1_000_000_000, "Bitcoin"),
                (1_000_000_001, "Solana"),
            ];
            Ok(names
                .into_iter()
                .map(|(chain_id, name)| BridgeNetworkInfo {
                    chain_id,
                    name: name.into(),
                    logo_url: None,
                })
                .collect())
        })
    }

    fn get_buy_tokens<'a>(&'a self, params: BuyTokensParams) -> BuyTokensFuture<'a> {
        let info = self.info.clone();
        let api = self.api.clone();
        Box::pin(async move {
            let raw = api.get_tokens().await.map_err(to_cow_err)?;
            let tokens: Vec<_> = adapt_tokens(&raw)
                .into_iter()
                .filter(|t| t.chain_id == params.buy_chain_id)
                .collect();
            Ok(GetProviderBuyTokens { provider_info: info, tokens })
        })
    }

    fn get_intermediate_tokens<'a>(
        &'a self,
        request: &'a QuoteBridgeRequest,
    ) -> IntermediateTokensFuture<'a> {
        let sell_chain = request.sell_chain_id;
        let buy_chain = request.buy_chain_id;
        let buy_token = request.buy_token;
        let api = self.api.clone();

        Box::pin(async move {
            let tokens = api.get_tokens().await.map_err(to_cow_err)?;
            let adapted = adapt_tokens(&tokens);

            // Map destination tokens by address for quick membership lookup.
            let target_has_buy_token =
                adapted.iter().any(|t| t.chain_id == buy_chain && t.address == buy_token);
            if !target_has_buy_token {
                return Ok(Vec::<IntermediateTokenInfo>::new());
            }

            Ok(adapted.into_iter().filter(|t| t.chain_id == sell_chain).collect())
        })
    }

    fn get_quote<'a>(&'a self, req: &'a QuoteBridgeRequest) -> QuoteFuture<'a> {
        let api = self.api.clone();
        let validity_secs = self.options.validity_secs;
        let attestator = self.options.attestator_address;

        Box::pin(async move {
            if req.kind != cow_types::OrderKind::Sell {
                return Err(CowError::Config("NEAR Intents only supports sell orders".into()));
            }

            // The bridge recipient is mandatory for NEAR — we can't
            // quote a non-EVM destination without a receiver address.
            let recipient =
                req.bridge_recipient.clone().or_else(|| req.receiver.clone()).ok_or_else(|| {
                    CowError::Config(
                        "NEAR Intents quote requires `bridge_recipient` or `receiver`".into(),
                    )
                })?;

            let deadline = calculate_deadline(validity_secs);

            let quote_request = NearQuoteRequest {
                dry: false,
                swap_type: NearSwapType::ExactInput,
                deposit_mode: NearDepositMode::Simple,
                slippage_tolerance: req.bridge_slippage_bps.map_or(req.slippage_bps, |bps| bps),
                origin_asset: format!("nep141:{:#x}", req.sell_token),
                deposit_type: NearDepositType::OriginChain,
                destination_asset: format!("nep141:{:#x}", req.buy_token),
                amount: req.sell_amount.to_string(),
                refund_to: format!("{:#x}", req.account),
                refund_type: NearRefundType::OriginChain,
                recipient,
                recipient_type: NearRecipientType::DestinationChain,
                deadline,
                app_fees: None,
                quote_waiting_time_ms: None,
                referral: None,
                virtual_chain_recipient: None,
                virtual_chain_refund_recipient: None,
                custom_recipient_msg: None,
                session_id: None,
                connected_wallets: None,
            };

            let response = api.get_quote(&quote_request).await.map_err(to_cow_err)?;

            // Attestation verification — crypto-critical.
            let (quote_hash, _canonical) =
                hash_quote_payload(&response.quote, &response.quote_request, &response.timestamp)
                    .map_err(to_cow_err)?;

            let deposit_address =
                parse_evm_address(&response.quote.deposit_address).map_err(to_cow_err)?;

            let attestation = api
                .get_attestation(&NearAttestationRequest {
                    deposit_address: format!("{deposit_address:#x}"),
                    quote_hash: format!("{quote_hash:#x}"),
                })
                .await
                .map_err(to_cow_err)?;

            let recovered =
                recover_attestation(deposit_address, quote_hash, &attestation.signature)
                    .map_err(to_cow_err)?;
            if recovered != attestator {
                return Err(CowError::Signing(format!(
                    "NEAR Intents attestation mismatch — expected {attestator:#x}, got \
                     {recovered:#x}",
                )));
            }

            // Convert the NEAR response to a minimal QuoteBridgeResponse.
            let sell_amount = response.quote.amount_in.parse::<u128>().unwrap_or_default();
            let buy_amount = response.quote.amount_out.parse::<u128>().unwrap_or_default();
            let min_out = response.quote.min_amount_out.parse::<u128>().unwrap_or_default();
            let fee_amount = buy_amount.saturating_sub(min_out);

            Ok(QuoteBridgeResponse {
                provider: default_info().name,
                sell_amount: alloy_primitives::U256::from(sell_amount),
                buy_amount: alloy_primitives::U256::from(min_out),
                fee_amount: alloy_primitives::U256::from(fee_amount),
                estimated_secs: response.quote.time_estimate,
                bridge_hook: None,
            })
        })
    }

    fn get_bridging_params<'a>(
        &'a self,
        _chain_id: u64,
        _order: &'a Order,
        _tx_hash: alloy_primitives::B256,
        _settlement_override: Option<Address>,
    ) -> BridgingParamsFuture<'a> {
        // Bridging params are derived from the NEAR execution status,
        // not from on-chain deposit events — the PR #11 tests will
        // wire this through `get_status` + quote body persistence.
        Box::pin(async { Ok(None::<BridgingParamsResult>) })
    }

    fn get_explorer_url(&self, bridging_id: &str) -> String {
        format!("https://explorer.near-intents.org/transactions/{bridging_id}")
    }

    fn get_status<'a>(
        &'a self,
        bridging_id: &'a str,
        _origin_chain_id: u64,
    ) -> BridgeStatusFuture<'a> {
        let api = self.api.clone();
        Box::pin(async move {
            let resp = api.get_execution_status(bridging_id).await.map_err(to_cow_err)?;
            let status = map_near_status_to_cow(resp.status);
            Ok(BridgeStatusResult {
                status,
                fill_time_in_seconds: None,
                deposit_tx_hash: resp
                    .swap_details
                    .origin_chain_tx_hashes
                    .first()
                    .map(|h| h.hash.clone()),
                fill_tx_hash: resp
                    .swap_details
                    .destination_chain_tx_hashes
                    .first()
                    .map(|h| h.hash.clone()),
            })
        })
    }

    fn as_receiver_account_bridge_provider(&self) -> Option<&dyn ReceiverAccountBridgeProvider> {
        Some(self)
    }
}

impl ReceiverAccountBridgeProvider for NearIntentsBridgeProvider {
    fn get_bridge_receiver_override<'a>(
        &'a self,
        _quote_request: &'a QuoteBridgeRequest,
        _quote_result: &'a QuoteBridgeResponse,
    ) -> ReceiverOverrideFuture<'a> {
        // The TS SDK stores the `depositAddress` from `get_quote` in a
        // per-quote cache keyed by quote-hash and looks it up here.
        // In this Rust port we return an empty override and document
        // that callers must pass the deposit address through their
        // own state: the receiver-account override is ultimately used
        // by the orchestrator in `get_quote_with_receiver_account_bridge`,
        // which has access to the full `QuoteBridgeResponse` plus any
        // out-of-band deposit-address the caller threaded through.
        //
        // A follow-up PR can add an in-memory cache on the provider
        // instance once we settle on a shared eviction policy across
        // Across / Bungee / NEAR.
        Box::pin(async {
            Err(CowError::Config(
                "NEAR Intents receiver override requires the caller to provide the \
                 `depositAddress` from the quote response (cache not yet wired — PR #11)"
                    .into(),
            ))
        })
    }
}

/// Map a NEAR execution status to the common [`BridgeStatus`] enum.
#[must_use]
pub const fn map_near_status_to_cow(status: NearExecutionStatus) -> BridgeStatus {
    match status {
        NearExecutionStatus::KnownDepositTx |
        NearExecutionStatus::PendingDeposit |
        NearExecutionStatus::Processing => BridgeStatus::InProgress,
        NearExecutionStatus::Success => BridgeStatus::Executed,
        NearExecutionStatus::Refunded => BridgeStatus::Refund,
        NearExecutionStatus::IncompleteDeposit | NearExecutionStatus::Failed => {
            BridgeStatus::Unknown
        }
    }
}

/// Chain-id sanity helper — documented for use by orchestration layer.
#[must_use]
pub fn chain_id_to_supported(chain_id: u64) -> Option<SupportedChainId> {
    SupportedChainId::try_from(chain_id).ok()
}

fn parse_evm_address(raw: &str) -> Result<Address, BridgeError> {
    Address::from_str(raw).map_err(|e| {
        BridgeError::InvalidApiResponse(format!("deposit address `{raw}` parse failed: {e}"))
    })
}

fn to_cow_err(e: BridgeError) -> CowError {
    if let BridgeError::Cow(inner) = e { inner } else { CowError::Config(e.to_string()) }
}

/// Look up a token by (`chain_id`, `evm_address`) on the full Defuse token
/// list. Mirrors `get_token_by_address_and_chain_id` from the TS SDK.
#[must_use]
pub fn get_token_by_address_and_chain_id(
    tokens: &[DefuseToken],
    chain_id: u64,
    evm_address: Address,
) -> Option<&DefuseToken> {
    let addr_str = format!("{evm_address:#x}");
    tokens.iter().find(|t| {
        blockchain_key_to_chain_id(&t.blockchain) == Some(chain_id) &&
            t.contract_address.as_deref().map(str::to_lowercase) == Some(addr_str.to_lowercase())
    })
}

// ── Prevent an unused-import warning on the re-export of B256 ───────────
const _: Option<B256> = None;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(clippy::tests_outside_test_module, reason = "inner module + cfg guard for WASM test skip")]
mod tests {
    use super::*;

    #[test]
    fn default_info_matches_helper() {
        let p = NearIntentsBridgeProvider::default();
        assert_eq!(p.info().dapp_id, NEAR_INTENTS_HOOK_DAPP_ID);
        assert_eq!(p.info().name, "near-intents");
        assert_eq!(p.info().provider_type, BridgeProviderType::ReceiverAccountBridgeProvider);
    }

    #[test]
    fn supports_route_requires_both_chains_and_distinct() {
        let p = NearIntentsBridgeProvider::default();
        assert!(p.supports_route(1, 42_161));
        assert!(p.supports_route(1, 1_000_000_000)); // ETH -> BTC
        assert!(!p.supports_route(1, 1), "same chain");
        assert!(!p.supports_route(1, 999), "unsupported buy chain");
    }

    #[test]
    fn near_intents_supported_chains_has_11_entries() {
        assert_eq!(near_intents_supported_chains().len(), 11);
    }

    #[test]
    fn map_near_status_to_cow_covers_all_variants() {
        use NearExecutionStatus::*;
        assert_eq!(map_near_status_to_cow(KnownDepositTx), BridgeStatus::InProgress);
        assert_eq!(map_near_status_to_cow(PendingDeposit), BridgeStatus::InProgress);
        assert_eq!(map_near_status_to_cow(Processing), BridgeStatus::InProgress);
        assert_eq!(map_near_status_to_cow(Success), BridgeStatus::Executed);
        assert_eq!(map_near_status_to_cow(Refunded), BridgeStatus::Refund);
        assert_eq!(map_near_status_to_cow(IncompleteDeposit), BridgeStatus::Unknown);
        assert_eq!(map_near_status_to_cow(Failed), BridgeStatus::Unknown);
    }

    #[test]
    fn explorer_url_is_built_correctly() {
        let p = NearIntentsBridgeProvider::default();
        assert_eq!(
            p.get_explorer_url("0xdeadbeef"),
            "https://explorer.near-intents.org/transactions/0xdeadbeef",
        );
    }

    #[test]
    fn as_receiver_account_bridge_provider_returns_some() {
        let p = NearIntentsBridgeProvider::default();
        assert!(p.as_receiver_account_bridge_provider().is_some());
    }

    #[test]
    fn get_token_by_address_and_chain_id_finds_match() {
        let tokens = vec![super::super::types::DefuseToken {
            asset_id: "nep141:usdc.e".into(),
            decimals: 6,
            blockchain: "eth".into(),
            symbol: "USDC".into(),
            price: 1.0,
            price_updated_at: "2025-09-05T12:00:38.695Z".into(),
            contract_address: Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".into()),
        }];
        let addr: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
        assert!(get_token_by_address_and_chain_id(&tokens, 1, addr).is_some());
        // Wrong chain ID → no match.
        assert!(get_token_by_address_and_chain_id(&tokens, 42_161, addr).is_none());
    }
}
