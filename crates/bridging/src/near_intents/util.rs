//! Utility functions for the NEAR Intents bridge provider.
//!
//! Contains:
//! - [`adapt_token`] / [`adapt_tokens`] — Defuse `TokenResponse` → `IntermediateTokenInfo`
//!   conversion with the `#850` sentinel fallback (BTC / SOL / EVM natives when `contractAddress`
//!   is absent).
//! - [`blockchain_key_to_chain_id`] — `"eth"` / `"arb"` / `"btc"` / … → EIP-155 chain-id mapping.
//! - [`hash_quote_payload`] — canonical JSON serialisation of the quote + SHA-256 hash.
//!   **Crypto-critical**: must match the TS `json-stable-stringify`-based hash byte-for-byte.
//! - [`recover_attestation`] — rebuild the signed message (`prefix ‖ version ‖ depositAddress ‖
//!   quoteHash`), keccak256 it, and ecrecover the address. Caller compares against
//!   [`cow_primitives::ATTESTATOR_ADDRESS`].
//! - [`calculate_deadline`] — ISO-8601 UTC timestamp.

use std::collections::BTreeMap;

use alloy_primitives::{Address, B256, keccak256};
use sha2::{Digest, Sha256};

use crate::types::BridgeError;

use super::types::{DefuseToken, NearQuote, NearQuoteRequest};

/// Prefix bytes prepended to every attestation message (`0x0a773570`).
pub const ATTESTATION_PREFIX_BYTES: [u8; 4] = [0x0a, 0x77, 0x35, 0x70];

/// Version byte prepended to every attestation message (`0x00`).
pub const ATTESTATION_VERSION_BYTES: [u8; 1] = [0x00];

/// Expected length of the NEAR Intents attestation signature (r ‖ s ‖ v).
pub const ATTESTATION_SIG_LEN: usize = 65;

// ── Token adaptation (cow-sdk#850 — fallback for empty contractAddress) ──

/// Convert a Defuse [`DefuseToken`] to the workspace-wide
/// [`crate::types::IntermediateTokenInfo`].
///
/// Returns `None` when the blockchain key maps to a chain ID the
/// workspace doesn't know about (see [`blockchain_key_to_chain_id`]).
///
/// ## Non-EVM destinations (BTC / SOL)
///
/// `IntermediateTokenInfo.address` is a [`crate::types::TokenAddress`]
/// — an enum that carries either an EVM [`Address`] or a raw string.
/// For non-EVM entries we emit `TokenAddress::Raw(token.contract_address)`
/// (or `Raw("")` when `contract_address` is missing, e.g. native BTC).
/// Callers should pair this with a matching `TokenAddress::Raw(..)` on
/// the `QuoteBridgeRequest.buy_token`. The real destination address
/// for the bridge is still carried through the quote's `depositAddress`.
///
/// ## cow-sdk#850 fix
///
/// When `contract_address` is missing and the chain is an EVM chain,
/// we substitute the canonical native sentinel
/// ([`cow_chains::EVM_NATIVE_CURRENCY_ADDRESS`]).
#[must_use]
pub fn adapt_token(token: &DefuseToken) -> Option<crate::types::IntermediateTokenInfo> {
    let chain_id = blockchain_key_to_chain_id(&token.blockchain)?;

    let address: crate::types::TokenAddress = if is_non_evm_chain_id(chain_id) {
        // BTC / SOL — preserve whatever NEAR returned as the raw
        // identifier (empty string for native BTC, SPL mint for
        // Solana). Callers get the byte-exact string the API emitted.
        crate::types::TokenAddress::Raw(token.contract_address.clone().unwrap_or_default())
    } else {
        let evm: Address = match token.contract_address.as_deref() {
            Some(raw) => raw.parse::<Address>().ok()?,
            // #850 fallback — empty `contractAddress` on an EVM chain
            // means the Defuse asset is the chain's native currency.
            None => cow_chains::EVM_NATIVE_CURRENCY_ADDRESS,
        };
        evm.into()
    };

    Some(crate::types::IntermediateTokenInfo {
        chain_id,
        address,
        decimals: token.decimals,
        // The `GET /v0/tokens` response doesn't carry the token's full
        // human name, only its symbol; reuse the symbol as name.
        symbol: token.symbol.clone(),
        name: token.symbol.clone(),
        logo_url: None,
    })
}

/// Adapt an array of [`DefuseToken`]s, dropping anything
/// [`adapt_token`] can't represent (non-EVM chains, unknown blockchain
/// keys, malformed addresses).
#[must_use]
pub fn adapt_tokens(tokens: &[DefuseToken]) -> Vec<crate::types::IntermediateTokenInfo> {
    tokens.iter().filter_map(adapt_token).collect()
}

/// Return `true` for non-EVM chain IDs the workspace models (Bitcoin,
/// Solana — [`cow_chains::AdditionalTargetChainId`]).
#[must_use]
pub const fn is_non_evm_chain_id(chain_id: u64) -> bool {
    chain_id == 1_000_000_000 || chain_id == 1_000_000_001
}

// ── Blockchain-key mapping ───────────────────────────────────────────────

/// Map a Defuse `NEAR_INTENTS_BLOCKCHAIN_CHAIN_IDS` key to an EIP-155
/// chain id (or workspace `AdditionalTargetChainId` for non-EVM).
///
/// Mirrors the TS constant of the same name. Unknown keys return
/// `None`.
#[must_use]
pub fn blockchain_key_to_chain_id(key: &str) -> Option<u64> {
    match key {
        "eth" => Some(1),
        "arb" => Some(42_161),
        "avax" => Some(43_114),
        "base" => Some(8_453),
        "bsc" => Some(56),
        "gnosis" => Some(100),
        "op" => Some(10),
        "pol" => Some(137),
        "plasma" => Some(9_745),
        "btc" => Some(1_000_000_000),
        "sol" => Some(1_000_000_001),
        _ => None,
    }
}

// ── Canonical JSON + SHA-256 (hashQuote equivalent) ──────────────────────

/// Canonicalise a [`serde_json::Value`] by recursively sorting object
/// keys and dropping `null` values. Mirrors the semantics of
/// `json-stable-stringify` (recursive lexicographic key ordering,
/// `undefined` → dropped).
///
/// `null` is dropped because the TS SDK uses
/// `undefined`-for-omit semantics and `json-stable-stringify` strips
/// those; in JSON-over-the-wire the same slot shows up as absent,
/// which `serde_json::Value` represents as `Null` when explicitly
/// threaded through.
fn canonicalise_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let sorted: BTreeMap<String, serde_json::Value> = map
                .iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), canonicalise_value(v)))
                .collect();
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonicalise_value).collect())
        }
        serde_json::Value::Null |
        serde_json::Value::Bool(_) |
        serde_json::Value::Number(_) |
        serde_json::Value::String(_) => value.clone(),
    }
}

/// `(quote-hash, canonical-json-string)` pair returned by
/// [`hash_quote_payload`].
pub type QuoteHashOutput = (B256, String);

/// Serialize a quote payload into the same canonical byte string the
/// TS `hashQuote` produces, then SHA-256 it.
///
/// Returns `(hash, canonical_json)` so callers can attach the raw
/// payload to an attestation request while also using the hash for
/// signature recovery.
///
/// # Errors
///
/// Returns [`BridgeError::InvalidApiResponse`] when any of the inputs
/// fails to serialize to JSON (essentially never for owned types, but
/// we surface the error rather than panic).
pub fn hash_quote_payload(
    quote: &NearQuote,
    quote_request: &NearQuoteRequest,
    timestamp: &str,
) -> Result<QuoteHashOutput, BridgeError> {
    // Build the flat object the TS `hashQuote` serialises. Cheating a
    // bit: we construct a `serde_json::Value` from typed slices so we
    // can then run `canonicalise_value` + `serde_json::to_string`.
    let mut payload = serde_json::Map::<String, serde_json::Value>::new();

    macro_rules! insert_str {
        ($map:ident, $field:expr, $val:expr) => {
            $map.insert($field.to_owned(), serde_json::Value::String($val.to_owned()));
        };
    }
    macro_rules! insert_opt_str {
        ($map:ident, $field:expr, $val:expr) => {
            if let Some(v) = $val.as_ref() {
                $map.insert($field.to_owned(), serde_json::Value::String(v.clone()));
            }
        };
    }

    // Mirror the TS field order — `json-stable-stringify` re-sorts
    // anyway, so the insertion order here is cosmetic.
    payload.insert("dry".into(), serde_json::Value::Bool(false));
    payload.insert("swapType".into(), serde_json::to_value(quote_request.swap_type).map_err(j)?);
    payload.insert(
        "slippageTolerance".into(),
        serde_json::to_value(quote_request.slippage_tolerance).map_err(j)?,
    );
    insert_str!(payload, "originAsset", &quote_request.origin_asset);
    payload
        .insert("depositType".into(), serde_json::to_value(quote_request.deposit_type).map_err(j)?);
    insert_str!(payload, "destinationAsset", &quote_request.destination_asset);
    insert_str!(payload, "amount", &quote_request.amount);
    insert_str!(payload, "refundTo", &quote_request.refund_to);
    payload
        .insert("refundType".into(), serde_json::to_value(quote_request.refund_type).map_err(j)?);
    insert_str!(payload, "recipient", &quote_request.recipient);
    payload.insert(
        "recipientType".into(),
        serde_json::to_value(quote_request.recipient_type).map_err(j)?,
    );
    insert_str!(payload, "deadline", &quote_request.deadline);
    if let Some(ms) = quote_request.quote_waiting_time_ms {
        payload.insert("quoteWaitingTimeMs".into(), serde_json::Value::from(ms));
    }
    insert_opt_str!(payload, "referral", quote_request.referral);
    insert_opt_str!(payload, "virtualChainRecipient", quote_request.virtual_chain_recipient);
    insert_opt_str!(
        payload,
        "virtualChainRefundRecipient",
        quote_request.virtual_chain_refund_recipient
    );
    payload
        .insert("depositMode".into(), serde_json::to_value(quote_request.deposit_mode).map_err(j)?);
    insert_str!(payload, "amountIn", &quote.amount_in);
    insert_str!(payload, "amountInFormatted", &quote.amount_in_formatted);
    insert_str!(payload, "amountInUsd", &quote.amount_in_usd);
    insert_str!(payload, "minAmountIn", &quote.min_amount_in);
    insert_str!(payload, "amountOut", &quote.amount_out);
    insert_str!(payload, "amountOutFormatted", &quote.amount_out_formatted);
    insert_str!(payload, "amountOutUsd", &quote.amount_out_usd);
    insert_str!(payload, "minAmountOut", &quote.min_amount_out);
    insert_str!(payload, "timestamp", timestamp);

    let value = serde_json::Value::Object(payload);
    let canonical = canonicalise_value(&value);
    let stringified = serde_json::to_string(&canonical).map_err(j)?;

    let digest = Sha256::digest(stringified.as_bytes());
    let hash = B256::from_slice(&digest);
    Ok((hash, stringified))
}

fn j(e: serde_json::Error) -> BridgeError {
    BridgeError::InvalidApiResponse(format!("quote hash serialization failed: {e}"))
}

// ── Attestation recovery ─────────────────────────────────────────────────

/// Reconstruct the signed attestation message (57 bytes:
/// `prefix ‖ version ‖ depositAddress ‖ quoteHash`), keccak-256 it,
/// and ecrecover the EIP-191 signer.
///
/// Caller compares the returned address against
/// [`cow_primitives::ATTESTATOR_ADDRESS`].
///
/// # Errors
///
/// * [`BridgeError::InvalidApiResponse`] if the signature hex cannot be parsed or has the wrong
///   length.
/// * [`BridgeError::QuoteError`] if ecrecover fails (malformed sig).
pub fn recover_attestation(
    deposit_address: Address,
    quote_hash: B256,
    signature: &str,
) -> Result<Address, BridgeError> {
    let sig_bytes = parse_hex_bytes(signature)?;
    if sig_bytes.len() != ATTESTATION_SIG_LEN {
        return Err(BridgeError::InvalidApiResponse(format!(
            "attestation signature must be {ATTESTATION_SIG_LEN} bytes, got {}",
            sig_bytes.len(),
        )));
    }

    // Build message: prefix (4) || version (1) || deposit addr (20) ||
    // quote hash (32) = 57 bytes.
    let mut message = Vec::with_capacity(4 + 1 + 20 + 32);
    message.extend_from_slice(&ATTESTATION_PREFIX_BYTES);
    message.extend_from_slice(&ATTESTATION_VERSION_BYTES);
    message.extend_from_slice(deposit_address.as_slice());
    message.extend_from_slice(quote_hash.as_slice());
    debug_assert_eq!(
        message.len(),
        57,
        "attestation message must be exactly 57 bytes (prefix+version+addr+hash)",
    );

    let digest = keccak256(&message);

    let sig = alloy_primitives::Signature::try_from(sig_bytes.as_slice()).map_err(|e| {
        BridgeError::InvalidApiResponse(format!("invalid attestation signature bytes: {e}"))
    })?;
    sig.recover_address_from_prehash(&digest)
        .map_err(|e| BridgeError::QuoteError(format!("attestation signature recovery failed: {e}")))
}

/// Parse a `0x`-prefixed (or bare) hex string into bytes.
fn parse_hex_bytes(hex: &str) -> Result<Vec<u8>, BridgeError> {
    let trimmed = hex.trim_start_matches("0x");
    if !trimmed.len().is_multiple_of(2) {
        return Err(BridgeError::InvalidApiResponse(format!(
            "hex string has odd length: {}",
            trimmed.len(),
        )));
    }
    let mut out = Vec::with_capacity(trimmed.len() / 2);
    for chunk in trimmed.as_bytes().chunks_exact(2) {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, BridgeError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(10 + (b - b'a')),
        b'A'..=b'F' => Ok(10 + (b - b'A')),
        _ => Err(BridgeError::InvalidApiResponse(format!("non-hex byte: 0x{b:02x}"))),
    }
}

// ── Deadline helper ──────────────────────────────────────────────────────

/// Produce an ISO-8601 UTC deadline `seconds_from_now` seconds in the
/// future. Matches the `new Date(Date.now() + seconds * 1000).toISOString()`
/// pattern from the TS SDK.
///
/// Native only — WASM builds must pass a deadline explicitly (the
/// browser `Date.now()` requires `js-sys` which we don't want to pull
/// into the bridging crate).
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn calculate_deadline(seconds_from_now: u64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    format_iso8601_utc(now.saturating_add(seconds_from_now))
}

/// WASM fallback — returns a fixed deadline so the module compiles
/// on `wasm32`. Callers should always override the deadline via
/// [`NearIntentsProviderOptions::validity_secs`] and format the
/// timestamp themselves using [`format_iso8601_utc`].
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn calculate_deadline(_seconds_from_now: u64) -> String {
    format_iso8601_utc(0)
}

/// Format a UNIX timestamp (seconds) as `YYYY-MM-DDTHH:MM:SS.000Z`.
///
/// Light-weight — no chrono dep. Accurate to the second (milliseconds
/// are always `000` because the TS uses `Date#toISOString` on a
/// rounded-second input anyway).
#[must_use]
pub fn format_iso8601_utc(unix_secs: u64) -> String {
    // Civil-time conversion from days-since-epoch. Algorithm from
    // Howard Hinnant's date library (public domain).
    let days_raw = unix_secs / 86_400;
    let days = i64::try_from(days_raw).unwrap_or_default();
    let secs_of_day = unix_secs % 86_400;
    let h = secs_of_day / 3_600;
    let m = (secs_of_day / 60) % 60;
    let s = secs_of_day % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y_base = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5) + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y_base + 1 } else { y_base };

    format!("{year:04}-{month:02}-{d:02}T{h:02}:{m:02}:{s:02}.000Z")
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(clippy::tests_outside_test_module, reason = "inner module + cfg guard for WASM test skip")]
mod tests {
    use super::*;

    fn sample_token() -> DefuseToken {
        DefuseToken {
            asset_id: "nep141:usdc.e".into(),
            decimals: 6,
            blockchain: "eth".into(),
            symbol: "USDC".into(),
            price: 1.0,
            price_updated_at: "2025-09-05T12:00:38.695Z".into(),
            contract_address: Some("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".into()),
        }
    }

    // ── blockchain_key_to_chain_id ───────────────────────────────────────

    #[test]
    fn blockchain_key_to_chain_id_covers_all_11_keys() {
        for (k, expected) in [
            ("eth", 1),
            ("arb", 42_161),
            ("avax", 43_114),
            ("base", 8_453),
            ("bsc", 56),
            ("gnosis", 100),
            ("op", 10),
            ("pol", 137),
            ("plasma", 9_745),
            ("btc", 1_000_000_000),
            ("sol", 1_000_000_001),
        ] {
            assert_eq!(blockchain_key_to_chain_id(k), Some(expected), "key {k}");
        }
    }

    #[test]
    fn blockchain_key_to_chain_id_unknown_returns_none() {
        assert_eq!(blockchain_key_to_chain_id("unknown"), None);
        assert_eq!(blockchain_key_to_chain_id(""), None);
    }

    // ── adapt_token ──────────────────────────────────────────────────────

    #[test]
    fn adapt_token_parses_evm_with_contract_address() {
        let t = sample_token();
        let out = adapt_token(&t).expect("EVM USDC should adapt");
        assert_eq!(out.chain_id, 1);
        assert_eq!(out.decimals, 6);
        assert_eq!(out.symbol, "USDC");
    }

    #[test]
    fn adapt_token_850_fallback_for_evm_native() {
        let mut t = sample_token();
        t.contract_address = None;
        t.symbol = "ETH".into();
        let out = adapt_token(&t).expect("EVM native should adapt with sentinel");
        assert_eq!(out.address, cow_chains::EVM_NATIVE_CURRENCY_ADDRESS);
    }

    #[test]
    fn adapt_token_non_evm_emits_raw_variant() {
        use crate::types::TokenAddress;
        let mut t = sample_token();
        t.blockchain = "btc".into();
        t.contract_address = None;
        t.symbol = "BTC".into();
        let out = adapt_token(&t).expect("non-EVM token adapts to Raw variant");
        assert_eq!(out.chain_id, 1_000_000_000);
        assert!(matches!(out.address, TokenAddress::Raw(ref s) if s.is_empty()));

        let mut t = sample_token();
        t.blockchain = "sol".into();
        t.contract_address = Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into());
        t.symbol = "USDC-SOL".into();
        let out = adapt_token(&t).expect("SOL SPL token adapts to Raw variant");
        assert_eq!(out.chain_id, 1_000_000_001);
        assert!(matches!(
            out.address,
            TokenAddress::Raw(ref s) if s == "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
        ));
    }

    #[test]
    fn adapt_token_unknown_chain_is_none() {
        let mut t = sample_token();
        t.blockchain = "some-new-chain".into();
        assert!(adapt_token(&t).is_none());
    }

    #[test]
    fn adapt_tokens_drops_unknown_chains_but_keeps_non_evm() {
        let mut btc = sample_token();
        btc.blockchain = "btc".into();
        btc.contract_address = None;
        btc.symbol = "BTC".into();
        let mut unknown = sample_token();
        unknown.blockchain = "future".into();

        let out = adapt_tokens(&[sample_token(), btc, unknown]);
        // EVM USDC + BTC sentinel survive; only the "future" key is dropped.
        assert_eq!(out.len(), 2);
    }

    // ── canonicalise_value ───────────────────────────────────────────────

    #[test]
    fn canonicalise_value_sorts_keys_recursively() {
        let input = serde_json::json!({
            "z": 1,
            "a": {
                "c": 3,
                "b": 2,
            },
        });
        let out = canonicalise_value(&input);
        let s = serde_json::to_string(&out).unwrap();
        assert_eq!(s, r#"{"a":{"b":2,"c":3},"z":1}"#);
    }

    #[test]
    fn canonicalise_value_drops_null_properties() {
        let input = serde_json::json!({ "a": 1, "b": null, "c": 3 });
        let s = serde_json::to_string(&canonicalise_value(&input)).unwrap();
        assert_eq!(s, r#"{"a":1,"c":3}"#);
    }

    // ── hash_quote_payload ───────────────────────────────────────────────

    fn sample_quote_request() -> NearQuoteRequest {
        NearQuoteRequest {
            dry: false,
            swap_type: super::super::types::NearSwapType::ExactInput,
            deposit_mode: super::super::types::NearDepositMode::Simple,
            slippage_tolerance: 50,
            origin_asset: "nep141:eth".into(),
            deposit_type: super::super::types::NearDepositType::OriginChain,
            destination_asset: "nep141:btc".into(),
            amount: "1000000".into(),
            refund_to: "0xabc".into(),
            refund_type: super::super::types::NearRefundType::OriginChain,
            recipient: "bc1q...".into(),
            recipient_type: super::super::types::NearRecipientType::DestinationChain,
            deadline: "2099-01-01T00:00:00.000Z".into(),
            app_fees: None,
            quote_waiting_time_ms: None,
            referral: None,
            virtual_chain_recipient: None,
            virtual_chain_refund_recipient: None,
            custom_recipient_msg: None,
            session_id: None,
            connected_wallets: None,
        }
    }

    fn sample_quote() -> NearQuote {
        NearQuote {
            amount_in: "1000000".into(),
            amount_in_formatted: "1.0".into(),
            amount_in_usd: "1.0".into(),
            min_amount_in: "1000000".into(),
            amount_out: "999500".into(),
            amount_out_formatted: "0.9995".into(),
            amount_out_usd: "0.99".into(),
            min_amount_out: "999000".into(),
            time_estimate: 120,
            deadline: "2099-01-01T00:00:00.000Z".into(),
            time_when_inactive: "2099-01-01T01:00:00.000Z".into(),
            deposit_address: "0xdead000000000000000000000000000000000000".into(),
        }
    }

    #[test]
    fn hash_quote_payload_is_deterministic() {
        let (h1, s1) = hash_quote_payload(&sample_quote(), &sample_quote_request(), "t").unwrap();
        let (h2, s2) = hash_quote_payload(&sample_quote(), &sample_quote_request(), "t").unwrap();
        assert_eq!(h1, h2);
        assert_eq!(s1, s2);
    }

    #[test]
    fn hash_quote_payload_canonical_string_sorts_keys() {
        let (_, s) = hash_quote_payload(&sample_quote(), &sample_quote_request(), "t").unwrap();
        // Verify the first few keys are lexicographically ordered. If
        // someone breaks canonicalisation by accident the hash will
        // change (tests in provider.rs cross-check against a known
        // vector).
        let amount_in_pos = s.find("\"amountIn\"").unwrap();
        let amount_out_pos = s.find("\"amountOut\"").unwrap();
        let deadline_pos = s.find("\"deadline\"").unwrap();
        assert!(amount_in_pos < amount_out_pos);
        assert!(amount_out_pos < deadline_pos);
    }

    #[test]
    fn hash_quote_payload_changes_when_amount_changes() {
        let mut r1 = sample_quote_request();
        r1.amount = "1".into();
        let mut r2 = sample_quote_request();
        r2.amount = "2".into();
        let (h1, _) = hash_quote_payload(&sample_quote(), &r1, "t").unwrap();
        let (h2, _) = hash_quote_payload(&sample_quote(), &r2, "t").unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_quote_payload_changes_when_timestamp_changes() {
        let (h1, _) = hash_quote_payload(&sample_quote(), &sample_quote_request(), "a").unwrap();
        let (h2, _) = hash_quote_payload(&sample_quote(), &sample_quote_request(), "b").unwrap();
        assert_ne!(h1, h2);
    }

    // ── recover_attestation ──────────────────────────────────────────────

    #[test]
    fn recover_attestation_rejects_wrong_length_signature() {
        let addr = Address::repeat_byte(0xab);
        let hash = B256::repeat_byte(0xcd);
        let err = recover_attestation(addr, hash, "0x1234").unwrap_err();
        assert!(matches!(err, BridgeError::InvalidApiResponse(_)));
    }

    #[test]
    fn recover_attestation_rejects_non_hex_signature() {
        let addr = Address::repeat_byte(0xab);
        let hash = B256::repeat_byte(0xcd);
        let err = recover_attestation(addr, hash, "0xzzzz").unwrap_err();
        assert!(matches!(err, BridgeError::InvalidApiResponse(_)));
    }

    #[test]
    fn recover_attestation_round_trips_with_local_signer() {
        // Sign a message with a known key, then recover — the returned
        // address must be the signer's address.
        use alloy_signer::SignerSync;
        use alloy_signer_local::PrivateKeySigner;
        use std::str::FromStr;
        let signer = PrivateKeySigner::from_str(
            "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        )
        .unwrap();
        let deposit_address = Address::repeat_byte(0x11);
        let quote_hash = B256::repeat_byte(0x22);

        // Same message layout as recover_attestation.
        let mut message = Vec::with_capacity(57);
        message.extend_from_slice(&ATTESTATION_PREFIX_BYTES);
        message.extend_from_slice(&ATTESTATION_VERSION_BYTES);
        message.extend_from_slice(deposit_address.as_slice());
        message.extend_from_slice(quote_hash.as_slice());
        let digest = keccak256(&message);

        let sig = signer.sign_hash_sync(&digest).unwrap();
        let sig_hex = format!("0x{}", hex_encode(&sig.as_bytes()));

        let recovered = recover_attestation(deposit_address, quote_hash, &sig_hex).unwrap();
        assert_eq!(recovered, signer.address());
    }

    #[allow(clippy::unwrap_used, reason = "fmt::Write to String is infallible")]
    fn hex_encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            use std::fmt::Write;
            write!(&mut s, "{b:02x}").unwrap();
        }
        s
    }

    // ── format_iso8601_utc ───────────────────────────────────────────────

    #[test]
    fn format_iso8601_utc_epoch() {
        assert_eq!(format_iso8601_utc(0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn format_iso8601_utc_known_timestamps() {
        // 2021-01-01T00:00:00Z
        assert_eq!(format_iso8601_utc(1_609_459_200), "2021-01-01T00:00:00.000Z");
        // 2024-06-15T12:48:16Z (arbitrary, hand-verified).
        assert_eq!(format_iso8601_utc(1_718_455_696), "2024-06-15T12:48:16.000Z");
    }

    #[test]
    fn format_iso8601_utc_day_month_year_boundaries() {
        // 1999-12-31T23:59:59Z
        assert_eq!(format_iso8601_utc(946_684_799), "1999-12-31T23:59:59.000Z");
        // 2000-01-01T00:00:00Z (leap-year boundary).
        assert_eq!(format_iso8601_utc(946_684_800), "2000-01-01T00:00:00.000Z");
    }
}
