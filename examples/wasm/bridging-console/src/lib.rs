//! Browser bridging console demo for the cow-rs WASM bindings.
//!
//! Ships a minimal `BridgingConsole` surface exported to JS with:
//! - `supported_chains()` — JSON list of the 11 NEAR-supported chains
//! - `near_intents_info()` — provider metadata
//! - `verify_near_attestation()` — pure-compute attestation recovery
//!   exercising the byte-exact message layout
//! - `canonical_quote_hash()` — deterministic SHA-256 of a canonical
//!   NEAR quote payload (useful for cross-checking the Rust / TS
//!   implementations byte-for-byte from the browser)
//!
//! Live network quotes are intentionally out of scope — the demo
//! focuses on the bits that are deterministic in-browser. Wiring a
//! live `NearIntentsApi::get_quote` call requires a CORS-proxied NEAR
//! endpoint; the scaffolding here makes adding it a drop-in change.
//!
//! # Build
//!
//! ```bash
//! cd examples/wasm/bridging-console
//! wasm-pack build --target web
//! ```
//!
//! Serve `index.html` with `python3 -m http.server` and open
//! `http://localhost:8000`.

use alloy_primitives::{Address, B256};
use cow_rs::bridging::near_intents::{
    NEAR_INTENTS_DEFAULT_VALIDITY_SECS, default_near_intents_info,
    types::{
        NearDepositMode, NearDepositType, NearQuote, NearQuoteRequest, NearRecipientType,
        NearRefundType, NearSwapType,
    },
    util::{hash_quote_payload, recover_attestation},
};
use wasm_bindgen::prelude::*;

fn log(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}

/// Handle exported to JS.
#[wasm_bindgen]
pub struct BridgingConsole;

#[wasm_bindgen]
impl BridgingConsole {
    /// Construct a new handle.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        log("BridgingConsole ready — NEAR Intents + attestation helpers loaded");
        Self
    }

    /// Return metadata about the NEAR Intents bridge provider as a JSON
    /// string: `{ name, dapp_id, kind }`.
    #[wasm_bindgen(js_name = "nearIntentsInfo")]
    #[must_use]
    pub fn near_intents_info(&self) -> String {
        let info = default_near_intents_info();
        serde_json::json!({
            "name": info.name,
            "dapp_id": info.dapp_id,
            "provider_type": format!("{:?}", info.provider_type),
            "default_validity_secs": NEAR_INTENTS_DEFAULT_VALIDITY_SECS,
        })
        .to_string()
    }

    /// Return the 11 chains NEAR Intents supports as a JSON array of
    /// `{ chain_id, key }` objects.
    #[wasm_bindgen(js_name = "supportedChains")]
    #[must_use]
    pub fn supported_chains(&self) -> String {
        // The EVM 9 + BTC (1_000_000_000) + SOL (900) — keep the table
        // flat for browser-side display; `blockchain_key_to_chain_id`
        // holds the canonical mapping.
        let pairs: Vec<serde_json::Value> = [
            (1_u64, "eth"),
            (100, "gnosis"),
            (42_161, "arb"),
            (8_453, "base"),
            (137, "pol"),
            (43_114, "avax"),
            (56, "bsc"),
            (59_144, "linea"),
            (57_073, "ink"),
            (1_000_000_000, "btc"),
            (900, "sol"),
        ]
        .iter()
        .map(|(id, key)| serde_json::json!({ "chain_id": id, "key": key }))
        .collect();
        serde_json::Value::Array(pairs).to_string()
    }

    /// Compute the canonical SHA-256 hash of a sample NEAR quote
    /// payload — a browser-side mirror of the Rust `hash_quote_payload`
    /// fixture so developers can diff the TS SDK output against ours.
    ///
    /// Accepts an optional `amount` override (as a decimal string).
    #[wasm_bindgen(js_name = "canonicalQuoteHash")]
    #[must_use]
    pub fn canonical_quote_hash(&self, amount: Option<String>) -> String {
        let amount = amount.unwrap_or_else(|| "1000000".into());
        let quote = NearQuote {
            deposit_address: "0xdead00000000000000000000000000000000beef".into(),
            amount_in: amount.clone(),
            amount_in_formatted: "1.0".into(),
            amount_in_usd: "1.0".into(),
            min_amount_in: amount.clone(),
            amount_out: amount.clone(),
            amount_out_formatted: "1.0".into(),
            amount_out_usd: "1.0".into(),
            min_amount_out: "999500".into(),
            time_estimate: 120,
            deadline: "2099-01-01T00:00:00.000Z".into(),
            time_when_inactive: "2099-01-01T01:00:00.000Z".into(),
        };
        let req = NearQuoteRequest {
            dry: false,
            swap_type: NearSwapType::ExactInput,
            deposit_mode: NearDepositMode::Simple,
            slippage_tolerance: 50,
            origin_asset: "nep141:0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".into(),
            deposit_type: NearDepositType::OriginChain,
            destination_asset: "nep141:0x0000000000000000000000000000000000000000".into(),
            amount,
            refund_to: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266".into(),
            refund_type: NearRefundType::OriginChain,
            recipient: "bc1q000000000000000000000000000000000000".into(),
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
        match hash_quote_payload(&quote, &req, "2025-09-05T12:00:40.000Z") {
            Ok((hash, canonical)) => serde_json::json!({
                "hash": format!("{hash:#x}"),
                "canonical_json": canonical,
            })
            .to_string(),
            Err(e) => format!("{{\"error\":\"{e}\"}}"),
        }
    }

    /// Recover the attestation signer from a deposit-address /
    /// quote-hash / 65-byte signature triple. Returns the recovered
    /// EVM address as a hex string, or an error JSON.
    #[wasm_bindgen(js_name = "verifyNearAttestation")]
    #[must_use]
    pub fn verify_near_attestation(
        &self,
        deposit_address_hex: String,
        quote_hash_hex: String,
        signature_hex: String,
    ) -> String {
        let deposit_address: Address = match deposit_address_hex.parse() {
            Ok(a) => a,
            Err(e) => return format!("{{\"error\":\"deposit_address: {e}\"}}"),
        };
        let quote_hash: B256 = match quote_hash_hex.parse() {
            Ok(h) => h,
            Err(e) => return format!("{{\"error\":\"quote_hash: {e}\"}}"),
        };
        match recover_attestation(deposit_address, quote_hash, &signature_hex) {
            Ok(recovered) => {
                serde_json::json!({ "recovered": format!("{recovered:#x}") }).to_string()
            }
            Err(e) => format!("{{\"error\":\"{e}\"}}"),
        }
    }
}

impl Default for BridgingConsole {
    fn default() -> Self {
        Self::new()
    }
}
