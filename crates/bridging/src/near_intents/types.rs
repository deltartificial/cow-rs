//! Serde types mirroring the NEAR Intents (`1click.chaindefuser.com`) API wire shapes.
//!
//! **All types use `#[serde(rename_all = "camelCase")]`** — the API emits
//! camelCase (`amountIn`, `priceUpdatedAt`, `depositAddress`). This is
//! load-bearing; we got burned on Across/Bungee earlier by defaulting
//! to `snake_case`, so every struct in this module is explicit.
//!
//! Enum variants that map to `SCREAMING_SNAKE_CASE` API values (e.g.
//! `"EXACT_INPUT"`, `"ORIGIN_CHAIN"`) carry `#[serde(rename_all =
//! "SCREAMING_SNAKE_CASE")]` on the enum itself.
//!
//! Mirrors:
//! - `@defuse-protocol/one-click-sdk-typescript`: `TokenResponse`, `QuoteRequest`, `QuoteResponse`,
//!   `GetExecutionStatusResponse`
//! - `packages/bridging/src/providers/near-intents/*`: request / response envelopes for
//!   `/v0/attestation`

use serde::{Deserialize, Serialize};

// ── /v0/tokens ────────────────────────────────────────────────────────────

/// Blockchain an asset lives on (`TokenResponse.blockchain` in the TS SDK).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DefuseBlockchain {
    /// Ethereum mainnet.
    Eth,
    /// Base L2.
    Base,
    /// Bitcoin.
    Btc,
    /// Solana.
    Sol,
    /// TON.
    Ton,
}

/// A single entry in the `GET /v0/tokens` response array.
///
/// Mirrors the Defuse `TokenResponse` interface:
///
/// ```text
/// interface TokenResponse {
///   assetId: string
///   decimals: number
///   blockchain: 'ETH' | 'BASE' | 'BTC' | 'SOL' | 'TON'
///   symbol: string
///   price: number
///   priceUpdatedAt: string  // ISO 8601 timestamp
///   contractAddress?: string  // Optional, undefined for native tokens
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefuseToken {
    /// Unique identifier in the format `nep141:{address_or_chain_key}`.
    pub asset_id: String,
    /// ERC-20-style decimals (`6` for USDC, `18` for ETH, …).
    pub decimals: u8,
    /// Chain the asset lives on.
    pub blockchain: DefuseBlockchain,
    /// Short symbol (e.g. `"USDC"`, `"ETH"`).
    pub symbol: String,
    /// Last-known USD price.
    pub price: f64,
    /// ISO 8601 timestamp of the last price update.
    pub price_updated_at: String,
    /// Optional on-chain contract address. Absent for native tokens
    /// (ETH, BTC, SOL, native TON).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_address: Option<String>,
}

// ── /v0/quote ─────────────────────────────────────────────────────────────

/// Swap-type discriminator on the quote request.
///
/// `EXACT_INPUT` = the caller specifies `amount` as the input; the API
/// computes the output. `FLEX_INPUT` = the API can tune the input to
/// hit a target output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NearSwapType {
    /// Caller commits to the input amount exactly.
    ExactInput,
    /// Caller leaves input flexible.
    FlexInput,
}

/// How the user will deposit the input funds.
///
/// The TS SDK only exposes `SIMPLE` today; we keep the enum extensible
/// for future variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NearDepositMode {
    /// Simple deposit — the user transfers the asset to `depositAddress`.
    #[default]
    Simple,
}

/// Which chain is debited on deposit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NearDepositType {
    /// Deposit is made on the origin chain.
    #[default]
    OriginChain,
}

/// Where refunds go when the swap cannot complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NearRefundType {
    /// Refund goes back to the origin chain.
    #[default]
    OriginChain,
}

/// Where the bought tokens land.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NearRecipientType {
    /// Recipient is addressed on the destination chain.
    #[default]
    DestinationChain,
}

/// Partner / referrer fee entry attached to a quote request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearAppFee {
    /// Address collecting the fee.
    pub recipient: String,
    /// Fee amount in basis points.
    pub fee: u32,
}

/// Body of `POST /v0/quote`.
///
/// Mirrors the Defuse `QuoteRequest` interface. Field names match the
/// wire format 1:1 (camelCase).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearQuoteRequest {
    /// If `true` the API treats the request as a dry-run; no deposit
    /// address is allocated.
    pub dry: bool,
    /// Swap-type selector.
    pub swap_type: NearSwapType,
    /// Deposit mode (TS ships `SIMPLE` only).
    pub deposit_mode: NearDepositMode,
    /// Slippage tolerance in basis points (50 = 0.5 %).
    pub slippage_tolerance: u32,
    /// Origin asset identifier (`nep141:…`).
    pub origin_asset: String,
    /// How the deposit reaches the bridge.
    pub deposit_type: NearDepositType,
    /// Destination asset identifier (`nep141:…`).
    pub destination_asset: String,
    /// Atomic input amount as a numeric string.
    pub amount: String,
    /// Refund destination address.
    pub refund_to: String,
    /// Refund-destination chain discriminator.
    pub refund_type: NearRefundType,
    /// Recipient address on the destination chain.
    pub recipient: String,
    /// Recipient-chain discriminator.
    pub recipient_type: NearRecipientType,
    /// Quote deadline (ISO 8601).
    pub deadline: String,
    /// Optional partner / referrer fees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_fees: Option<Vec<NearAppFee>>,
    /// Optional quote waiting time in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_waiting_time_ms: Option<u64>,
    /// Optional referral handle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referral: Option<String>,
    /// Optional virtual-chain recipient.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub virtual_chain_recipient: Option<String>,
    /// Optional virtual-chain refund recipient.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub virtual_chain_refund_recipient: Option<String>,
    /// Optional custom recipient-side message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_recipient_msg: Option<String>,
    /// Optional session ID for analytics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Optional wallets connected at quote time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected_wallets: Option<Vec<String>>,
}

/// Inner `quote` object carried by `NearQuoteResponse` and
/// `NearExecutionStatusResponse.quoteResponse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearQuote {
    /// Input amount echoed back (atomic string).
    pub amount_in: String,
    /// Human-formatted input amount (e.g. `"1.0"`).
    pub amount_in_formatted: String,
    /// USD value of `amount_in`.
    pub amount_in_usd: String,
    /// Minimum acceptable input amount.
    pub min_amount_in: String,
    /// Expected output amount (atomic string).
    pub amount_out: String,
    /// Human-formatted output amount.
    pub amount_out_formatted: String,
    /// USD value of `amount_out`.
    pub amount_out_usd: String,
    /// Minimum output respecting slippage.
    pub min_amount_out: String,
    /// Estimated completion time in seconds.
    pub time_estimate: u64,
    /// Quote deadline (ISO 8601).
    pub deadline: String,
    /// Timestamp after which the quote is inactive (ISO 8601).
    pub time_when_inactive: String,
    /// Address where the user must deposit the origin asset.
    pub deposit_address: String,
}

/// Body of `POST /v0/quote` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearQuoteResponse {
    /// Quote details.
    pub quote: NearQuote,
    /// Echo of the originating request.
    pub quote_request: NearQuoteRequest,
    /// Attestation signature from the NEAR Intents relayer
    /// (`ed25519:…` prefix).
    pub signature: String,
    /// Server-side issue timestamp (ISO 8601).
    pub timestamp: String,
}

// ── /v0/execution-status ──────────────────────────────────────────────────

/// Execution status enum mirroring the string values the API emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NearExecutionStatus {
    /// Known deposit tx observed on the origin chain.
    KnownDepositTx,
    /// Deposit is pending — relayer hasn't picked it up yet.
    PendingDeposit,
    /// Partial deposit (amount mismatch, etc.).
    IncompleteDeposit,
    /// Relayer is processing the swap.
    Processing,
    /// Swap completed successfully.
    Success,
    /// Swap was refunded.
    Refunded,
    /// Swap failed.
    Failed,
}

/// On-chain transaction hash + explorer URL pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearChainTxHash {
    /// Transaction hash.
    pub hash: String,
    /// Explorer URL for the transaction.
    pub explorer_url: String,
}

/// Nested `swapDetails` object on an execution status response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearSwapDetails {
    /// Defuse intent hashes.
    pub intent_hashes: Vec<String>,
    /// NEAR tx hashes (Defuse execution layer).
    pub near_tx_hashes: Vec<String>,
    /// Atomic input amount.
    pub amount_in: String,
    /// Human-formatted input amount.
    pub amount_in_formatted: String,
    /// USD value of input.
    pub amount_in_usd: String,
    /// Atomic output amount.
    pub amount_out: String,
    /// Human-formatted output amount.
    pub amount_out_formatted: String,
    /// USD value of output.
    pub amount_out_usd: String,
    /// Observed slippage (signed percentage).
    pub slippage: f64,
    /// Atomic refunded amount (0 if no refund).
    pub refunded_amount: String,
    /// Human-formatted refunded amount.
    pub refunded_amount_formatted: String,
    /// USD value of the refund.
    pub refunded_amount_usd: String,
    /// Origin-chain txs.
    pub origin_chain_tx_hashes: Vec<NearChainTxHash>,
    /// Destination-chain txs.
    pub destination_chain_tx_hashes: Vec<NearChainTxHash>,
}

/// Body of `GET /v0/execution-status/{deposit_address}` response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearExecutionStatusResponse {
    /// Current status enum.
    pub status: NearExecutionStatus,
    /// Last-update timestamp (ISO 8601).
    pub updated_at: String,
    /// Swap details.
    pub swap_details: NearSwapDetails,
    /// Optional cached quote response (present once the relayer
    /// acknowledges the deposit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_response: Option<NearQuoteResponse>,
}

// ── /v0/attestation ───────────────────────────────────────────────────────

/// Body of `POST /v0/attestation` request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearAttestationRequest {
    /// Deposit address from the matching `NearQuote`.
    pub deposit_address: String,
    /// Hex-encoded hash of the canonical quote payload.
    pub quote_hash: String,
}

/// Body of `POST /v0/attestation` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearAttestationResponse {
    /// `0x`-prefixed 65-byte secp256k1 signature from the attestor.
    pub signature: String,
    /// Signature-format version tag (currently `1`).
    pub version: u32,
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(clippy::tests_outside_test_module, reason = "inner module + cfg guard for WASM test skip")]
mod tests {
    use super::*;

    #[test]
    fn defuse_blockchain_roundtrips_through_uppercase_json() {
        let t = DefuseBlockchain::Btc;
        let j = serde_json::to_string(&t).unwrap();
        assert_eq!(j, "\"BTC\"");
        let back: DefuseBlockchain = serde_json::from_str(&j).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn defuse_token_optional_contract_address_deserializes() {
        let sample = r#"{
            "assetId": "nep141:eth",
            "decimals": 18,
            "blockchain": "ETH",
            "symbol": "ETH",
            "price": 4463.25,
            "priceUpdatedAt": "2025-09-05T12:00:38.695Z"
        }"#;
        let t: DefuseToken = serde_json::from_str(sample).unwrap();
        assert_eq!(t.decimals, 18);
        assert!(t.contract_address.is_none());
    }

    #[test]
    fn near_swap_type_serializes_as_screaming_snake_case() {
        let t = NearSwapType::ExactInput;
        assert_eq!(serde_json::to_string(&t).unwrap(), "\"EXACT_INPUT\"");
    }

    #[test]
    fn near_execution_status_roundtrips_all_variants() {
        for (v, expected) in [
            (NearExecutionStatus::KnownDepositTx, "\"KNOWN_DEPOSIT_TX\""),
            (NearExecutionStatus::PendingDeposit, "\"PENDING_DEPOSIT\""),
            (NearExecutionStatus::IncompleteDeposit, "\"INCOMPLETE_DEPOSIT\""),
            (NearExecutionStatus::Processing, "\"PROCESSING\""),
            (NearExecutionStatus::Success, "\"SUCCESS\""),
            (NearExecutionStatus::Refunded, "\"REFUNDED\""),
            (NearExecutionStatus::Failed, "\"FAILED\""),
        ] {
            let j = serde_json::to_string(&v).unwrap();
            assert_eq!(j, expected);
            let back: NearExecutionStatus = serde_json::from_str(&j).unwrap();
            assert_eq!(back, v);
        }
    }

    #[test]
    fn attestation_request_response_roundtrip() {
        let req = NearAttestationRequest {
            deposit_address: "0xdead000000000000000000000000000000000000".into(),
            quote_hash: "0xabc".into(),
        };
        let j = serde_json::to_string(&req).unwrap();
        assert!(j.contains("depositAddress"));
        assert!(j.contains("quoteHash"));
        let back: NearAttestationRequest = serde_json::from_str(&j).unwrap();
        assert_eq!(back, req);

        let resp = NearAttestationResponse { signature: "0x1234".into(), version: 1 };
        let j = serde_json::to_string(&resp).unwrap();
        let back: NearAttestationResponse = serde_json::from_str(&j).unwrap();
        assert_eq!(back, resp);
    }

    #[test]
    fn near_quote_request_serializes_camel_case() {
        let req = NearQuoteRequest {
            dry: false,
            swap_type: NearSwapType::ExactInput,
            deposit_mode: NearDepositMode::Simple,
            slippage_tolerance: 50,
            origin_asset: "nep141:eth".into(),
            deposit_type: NearDepositType::OriginChain,
            destination_asset: "nep141:btc".into(),
            amount: "1000000".into(),
            refund_to: "0xabc".into(),
            refund_type: NearRefundType::OriginChain,
            recipient: "bc1q…".into(),
            recipient_type: NearRecipientType::DestinationChain,
            deadline: "2099-01-01T00:00:00.000Z".into(),
            app_fees: None,
            quote_waiting_time_ms: None,
            referral: None,
            virtual_chain_recipient: None,
            virtual_chain_refund_recipient: None,
            custom_recipient_msg: None,
            session_id: None,
            connected_wallets: None,
        };
        let j = serde_json::to_string(&req).unwrap();
        assert!(j.contains("\"swapType\":\"EXACT_INPUT\""));
        assert!(j.contains("\"slippageTolerance\":50"));
        assert!(j.contains("\"originAsset\":\"nep141:eth\""));
        // Optional fields absent from JSON when None.
        assert!(!j.contains("appFees"));
        assert!(!j.contains("sessionId"));
    }

    #[test]
    fn defuse_token_with_contract_address_roundtrips() {
        let sample = r#"{
            "assetId": "nep141:usdc.e",
            "decimals": 6,
            "blockchain": "ETH",
            "symbol": "USDC",
            "price": 1.0,
            "priceUpdatedAt": "2025-09-05T12:00:38.695Z",
            "contractAddress": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        }"#;
        let t: DefuseToken = serde_json::from_str(sample).unwrap();
        assert_eq!(
            t.contract_address.as_deref(),
            Some("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
        );
        // Roundtrip preserves it.
        let j = serde_json::to_string(&t).unwrap();
        assert!(j.contains("contractAddress"));
    }
}
