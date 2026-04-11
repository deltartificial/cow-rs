//! Canonical JSON → `keccak256` hashing for `CoW` Protocol app-data.
//!
//! Every `CoW` Protocol order carries a 32-byte `appData` field that commits
//! to a JSON document describing the order's intent, referral, hooks, and
//! more. This module provides the functions that build that document,
//! serialise it to deterministic JSON (sorted keys, no whitespace), and hash
//! it with `keccak256`.
//!
//! # Key functions
//!
//! | Function | Use case |
//! |---|---|
//! | [`appdata_hex`] | Hash an existing [`AppDataDoc`] → [`B256`] |
//! | [`build_order_app_data`] | Simple order → `0x`-prefixed hex string |
//! | [`build_app_data_doc`] | Order with metadata → hex string |
//! | [`build_app_data_doc_full`] | Order with metadata → `(json, hex)` |
//! | [`appdata_json`] | Get the canonical JSON without hashing |
//! | [`stringify_deterministic`] | Low-level sorted-key JSON serialiser |
//! | [`merge_app_data_doc`] | Deep-merge two documents |

use alloy_primitives::{B256, keccak256};
use serde_json::Value;

use crate::error::CowError;

use super::types::{AppDataDoc, LATEST_APP_DATA_VERSION, Metadata};

/// Serialise `doc` to canonical JSON with sorted keys, then return
/// `keccak256(json_bytes)`.
///
/// The returned [`B256`] is the 32-byte digest used as the `appData` field in
/// every [`UnsignedOrder`](crate::order_signing::types::UnsignedOrder).
/// Internally, the document is first passed through [`stringify_deterministic`]
/// (which sorts all object keys alphabetically and strips whitespace) before
/// hashing, guaranteeing the same [`AppDataDoc`] always produces the same hash
/// regardless of field insertion order.
///
/// Mirrors `appDataHex` from the `@cowprotocol/app-data` `TypeScript` package.
///
/// # Parameters
///
/// * `doc` — the [`AppDataDoc`] to hash. Only its JSON-serialisable fields contribute to the
///   digest; `#[serde(skip)]` fields are excluded.
///
/// # Returns
///
/// A 32-byte [`B256`] containing the `keccak256` hash of the canonical JSON.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if the document cannot be serialised to JSON
/// (e.g. a custom `Serialize` impl fails).
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{AppDataDoc, appdata_hex};
///
/// let doc = AppDataDoc::new("MyDApp");
/// let hash = appdata_hex(&doc).unwrap();
/// // The hash is deterministic — calling again yields the same value.
/// assert_eq!(hash, appdata_hex(&doc).unwrap());
/// ```
pub fn appdata_hex(doc: &AppDataDoc) -> Result<B256, CowError> {
    let json = stringify_deterministic(doc)?;
    Ok(keccak256(json.as_bytes()))
}

/// Build the `0x`-prefixed app-data hex string for an order identified by
/// `app_code`.
///
/// This is the simplest entry point for generating the `appData` value needed
/// by every `CoW` Protocol order. It creates an [`AppDataDoc`] with the given
/// `app_code`, no extra metadata, and the latest schema version, then
/// serialises it deterministically and returns the `keccak256` hex digest.
///
/// For orders that carry structured metadata (hooks, partner fees, UTM, …),
/// use [`build_app_data_doc`] or [`build_app_data_doc_full`] instead.
///
/// Mirrors the simple-case path of `buildAppData` from the `TypeScript` SDK.
///
/// # Parameters
///
/// * `app_code` — application identifier embedded in the order metadata (e.g. `"CoW Swap"`,
///   `"MyDApp"`). Must be ≤ 50 characters.
///
/// # Returns
///
/// A `0x`-prefixed, lowercase hex string of the 32-byte `keccak256` hash.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if serialisation fails.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::build_order_app_data;
///
/// let hex = build_order_app_data("MyDApp").unwrap();
/// assert!(hex.starts_with("0x"));
/// assert_eq!(hex.len(), 66); // "0x" + 64 hex chars
/// ```
pub fn build_order_app_data(app_code: &str) -> Result<String, CowError> {
    let doc = AppDataDoc::new(app_code);
    let hash = appdata_hex(&doc)?;
    Ok(format!("0x{}", alloy_primitives::hex::encode(hash.as_slice())))
}

/// Build the `0x`-prefixed app-data hex string with the given `app_code` and
/// [`Metadata`].
///
/// This is the full-featured variant of [`build_order_app_data`]: it lets
/// callers embed structured metadata (order class, quote slippage, UTM params,
/// hooks, partner fees, …) in the app-data document before hashing.
///
/// Internally delegates to [`build_app_data_doc_full`] and discards the full
/// JSON string, returning only the `keccak256` hex digest. If you also need
/// the canonical JSON (e.g. to upload to IPFS), call [`build_app_data_doc_full`]
/// directly.
///
/// Mirrors `buildAppData` from the `@cowprotocol/app-data` `TypeScript` package.
///
/// # Parameters
///
/// * `app_code` — application identifier (e.g. `"CoW Swap"`).
/// * `metadata` — structured metadata to embed. Use [`Metadata::default()`] for an empty metadata
///   block, or the builder methods on [`Metadata`] to populate individual fields.
///
/// # Returns
///
/// A `0x`-prefixed, lowercase hex string of the 32-byte `keccak256` hash.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if serialisation fails.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{Metadata, Quote, build_app_data_doc};
///
/// let meta = Metadata::default().with_quote(Quote::new(50));
/// let hex = build_app_data_doc("MyDApp", meta).unwrap();
/// assert!(hex.starts_with("0x"));
/// ```
pub fn build_app_data_doc(app_code: &str, metadata: Metadata) -> Result<String, CowError> {
    let (_, hash_hex) = build_app_data_doc_full(app_code, metadata)?;
    Ok(hash_hex)
}

/// Build the canonical JSON app-data document together with its `keccak256`
/// hash.
///
/// This is the lowest-level entry point for app-data construction. It
/// assembles an [`AppDataDoc`] from the given `app_code` and [`Metadata`],
/// serialises it to deterministic JSON (sorted keys, no whitespace), and
/// returns both the full JSON string and the `0x`-prefixed `keccak256` hex
/// digest.
///
/// Use this when you need both the canonical JSON (e.g. to pin on IPFS) and
/// the hash (to submit in the on-chain order). If you only need the hash,
/// prefer [`build_app_data_doc`]; for simple orders without metadata, use
/// [`build_order_app_data`].
///
/// Mirrors the `TypeScript` SDK's `buildAppData` which returns a
/// `TradingAppDataInfo` object with `fullAppData` and `appDataKeccak256`.
///
/// # Parameters
///
/// * `app_code` — application identifier (e.g. `"CoW Swap"`).
/// * `metadata` — structured metadata to embed in the document.
///
/// # Returns
///
/// A tuple `(full_app_data_json, app_data_keccak256_hex)` where:
/// - `full_app_data_json` is the canonical JSON string with sorted keys.
/// - `app_data_keccak256_hex` is the `0x`-prefixed 32-byte hex digest.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if the document cannot be serialised to JSON.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{Metadata, build_app_data_doc_full};
///
/// let (json, hex) = build_app_data_doc_full("MyDApp", Metadata::default()).unwrap();
/// assert!(json.contains("MyDApp"));
/// assert!(hex.starts_with("0x"));
/// assert_eq!(hex.len(), 66);
/// ```
// The return type is a transparent (json, hash_hex) pair — the tuple is intentional.
#[allow(
    clippy::type_complexity,
    reason = "transparent (json, hash_hex) pair; no named type needed"
)]
pub fn build_app_data_doc_full(
    app_code: &str,
    metadata: Metadata,
) -> Result<(String, String), CowError> {
    let doc = AppDataDoc {
        version: LATEST_APP_DATA_VERSION.to_owned(),
        app_code: Some(app_code.to_owned()),
        environment: None,
        metadata,
    };
    let json = stringify_deterministic(&doc)?;
    let hash = alloy_primitives::keccak256(json.as_bytes());
    let hash_hex = format!("0x{}", alloy_primitives::hex::encode(hash.as_slice()));
    Ok((json, hash_hex))
}

/// Return the canonical JSON string for `doc` with all object keys sorted
/// alphabetically and no extraneous whitespace.
///
/// This is a thin convenience wrapper around [`stringify_deterministic`]. Use
/// it when you need the raw JSON pre-image that, when hashed with
/// `keccak256`, yields the `appData` value stored on-chain.
///
/// # Parameters
///
/// * `doc` — the [`AppDataDoc`] to serialise.
///
/// # Returns
///
/// A compact JSON string with deterministically ordered keys.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if serialisation fails.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{AppDataDoc, appdata_json};
///
/// let doc = AppDataDoc::new("MyDApp");
/// let json = appdata_json(&doc).unwrap();
/// // Keys are sorted: "appCode" comes before "metadata" comes before "version"
/// assert!(json.starts_with('{'));
/// assert!(json.contains("\"appCode\":\"MyDApp\""));
/// ```
pub fn appdata_json(doc: &AppDataDoc) -> Result<String, CowError> {
    stringify_deterministic(doc)
}

/// Serialise `doc` to a deterministic JSON string with all object keys
/// sorted alphabetically at every nesting level.
///
/// This is the core serialisation primitive that underpins all app-data
/// hashing in this crate. It guarantees that two [`AppDataDoc`] values with
/// identical logical content always produce byte-identical JSON, regardless
/// of Rust struct field order or `serde` attribute ordering.
///
/// Matches the behaviour of `json-stringify-deterministic` used by the
/// `TypeScript` SDK, ensuring cross-language hash compatibility.
///
/// # Parameters
///
/// * `doc` — the [`AppDataDoc`] to serialise.
///
/// # Returns
///
/// A compact JSON string with no whitespace between tokens and all object
/// keys recursively sorted in lexicographic order.
///
/// # Errors
///
/// Returns [`CowError::AppData`] on serialisation failure.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{AppDataDoc, stringify_deterministic};
///
/// let doc = AppDataDoc::new("Test");
/// let json = stringify_deterministic(&doc).unwrap();
/// // Deterministic: calling twice yields the exact same bytes.
/// assert_eq!(json, stringify_deterministic(&doc).unwrap());
/// ```
pub fn stringify_deterministic(doc: &AppDataDoc) -> Result<String, CowError> {
    let value = serde_json::to_value(doc).map_err(|e| CowError::AppData(e.to_string()))?;
    let sorted = sort_keys(value);
    serde_json::to_string(&sorted).map_err(|e| CowError::AppData(e.to_string()))
}

/// Deep-merge `other` into `base` and return the result.
///
/// Scalar fields (`version`, `app_code`, `environment`) use `other`'s value
/// when it is non-empty / `Some`. Each [`Metadata`] field is replaced
/// independently when the corresponding field in `other.metadata` is `Some`.
///
/// Array-typed fields inside `OrderInteractionHooks` (`pre` / `post` hooks)
/// are **replaced wholesale** when `other.metadata.hooks` is `Some` — this
/// matches the `TypeScript` SDK's array-clearing deep-merge semantics.
///
/// Mirrors `mergeAppDataDoc` from the `@cowprotocol/app-data` package.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{AppDataDoc, merge_app_data_doc};
///
/// let base = AppDataDoc::new("BaseApp");
/// let override_doc = AppDataDoc::new("OverrideApp");
/// let merged = merge_app_data_doc(base, override_doc);
/// assert_eq!(merged.app_code, Some("OverrideApp".to_owned()));
/// ```
#[must_use]
pub fn merge_app_data_doc(mut base: AppDataDoc, other: AppDataDoc) -> AppDataDoc {
    if !other.version.is_empty() {
        base.version = other.version;
    }
    if other.app_code.is_some() {
        base.app_code = other.app_code;
    }
    if other.environment.is_some() {
        base.environment = other.environment;
    }
    let om = other.metadata;
    if om.referrer.is_some() {
        base.metadata.referrer = om.referrer;
    }
    if om.utm.is_some() {
        base.metadata.utm = om.utm;
    }
    if om.quote.is_some() {
        base.metadata.quote = om.quote;
    }
    if om.order_class.is_some() {
        base.metadata.order_class = om.order_class;
    }
    if om.hooks.is_some() {
        base.metadata.hooks = om.hooks;
    }
    if om.widget.is_some() {
        base.metadata.widget = om.widget;
    }
    if om.partner_fee.is_some() {
        base.metadata.partner_fee = om.partner_fee;
    }
    if om.replaced_order.is_some() {
        base.metadata.replaced_order = om.replaced_order;
    }
    if om.signer.is_some() {
        base.metadata.signer = om.signer;
    }
    base
}

/// Recursively sort all object keys in a [`Value`] alphabetically.
fn sort_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut pairs: Vec<(String, Value)> =
                map.into_iter().map(|(k, v)| (k, sort_keys(v))).collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(pairs.into_iter().collect())
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(sort_keys).collect()),
        other @ (Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)) => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appdata_hex_is_deterministic() {
        let doc = AppDataDoc::new("Test");
        let h1 = appdata_hex(&doc).unwrap();
        let h2 = appdata_hex(&doc).unwrap();
        assert_eq!(h1, h2);
        assert_ne!(h1, B256::ZERO);
    }

    #[test]
    fn build_order_app_data_format() {
        let hex = build_order_app_data("MyDApp").unwrap();
        assert!(hex.starts_with("0x"));
        assert_eq!(hex.len(), 66);
    }

    #[test]
    fn build_app_data_doc_returns_hex() {
        let hex = build_app_data_doc("MyDApp", Metadata::default()).unwrap();
        assert!(hex.starts_with("0x"));
        assert_eq!(hex.len(), 66);
    }

    #[test]
    fn build_app_data_doc_full_returns_json_and_hex() {
        let (json, hex) = build_app_data_doc_full("MyDApp", Metadata::default()).unwrap();
        assert!(json.contains("MyDApp"));
        assert!(hex.starts_with("0x"));
        assert_eq!(hex.len(), 66);
    }

    #[test]
    fn appdata_json_deterministic() {
        let doc = AppDataDoc::new("Test");
        let j1 = appdata_json(&doc).unwrap();
        let j2 = appdata_json(&doc).unwrap();
        assert_eq!(j1, j2);
        assert!(j1.starts_with('{'));
    }

    #[test]
    fn stringify_deterministic_sorts_keys() {
        let doc = AppDataDoc::new("Test");
        let json = stringify_deterministic(&doc).unwrap();
        // Keys should be sorted: appCode before metadata before version
        let app_idx = json.find("appCode").unwrap();
        let meta_idx = json.find("metadata").unwrap();
        let ver_idx = json.find("version").unwrap();
        assert!(app_idx < meta_idx);
        assert!(meta_idx < ver_idx);
    }

    #[test]
    fn merge_app_data_doc_overrides_app_code() {
        let base = AppDataDoc::new("Base");
        let other = AppDataDoc::new("Override");
        let merged = merge_app_data_doc(base, other);
        assert_eq!(merged.app_code, Some("Override".to_owned()));
    }

    #[test]
    fn merge_app_data_doc_overrides_version() {
        let base = AppDataDoc::new("Base");
        let mut other = AppDataDoc::new("Other");
        other.version = "2.0.0".to_owned();
        let merged = merge_app_data_doc(base, other);
        assert_eq!(merged.version, "2.0.0");
    }

    #[test]
    fn merge_app_data_doc_preserves_base_when_other_empty() {
        let base = AppDataDoc::new("Base").with_environment("prod");
        let other = AppDataDoc {
            version: String::new(),
            app_code: None,
            environment: None,
            metadata: Metadata::default(),
        };
        let merged = merge_app_data_doc(base, other);
        // Empty version on other means base version is kept
        assert_eq!(merged.app_code, Some("Base".to_owned()));
        assert_eq!(merged.environment, Some("prod".to_owned()));
    }

    #[test]
    fn merge_app_data_doc_overrides_environment() {
        let base = AppDataDoc::new("Base");
        let other = AppDataDoc::new("Other").with_environment("staging");
        let merged = merge_app_data_doc(base, other);
        assert_eq!(merged.environment.as_deref(), Some("staging"));
    }

    #[test]
    fn merge_app_data_doc_overrides_metadata_fields() {
        use crate::app_data::types::{Quote, Referrer, Utm, Widget};
        let base = AppDataDoc::new("Base");
        let other = AppDataDoc {
            version: LATEST_APP_DATA_VERSION.to_owned(),
            app_code: None,
            environment: None,
            metadata: Metadata::default()
                .with_referrer(Referrer::code("ABC"))
                .with_utm(Utm { utm_source: Some("test".into()), ..Default::default() })
                .with_quote(Quote::new(50))
                .with_widget(Widget { app_code: "w".into(), environment: None })
                .with_signer("0x1111111111111111111111111111111111111111"),
        };
        let merged = merge_app_data_doc(base, other);
        assert!(merged.metadata.referrer.is_some());
        assert!(merged.metadata.utm.is_some());
        assert!(merged.metadata.quote.is_some());
        assert!(merged.metadata.widget.is_some());
        assert!(merged.metadata.signer.is_some());
    }

    #[test]
    fn merge_app_data_doc_overrides_hooks() {
        use crate::app_data::types::{CowHook, OrderInteractionHooks};
        let base = AppDataDoc::new("Base");
        let hook =
            CowHook::new("0x0000000000000000000000000000000000000001", "0xdeadbeef", "100000");
        let other = AppDataDoc {
            version: LATEST_APP_DATA_VERSION.to_owned(),
            app_code: None,
            environment: None,
            metadata: Metadata::default()
                .with_hooks(OrderInteractionHooks::new(vec![hook], vec![])),
        };
        let merged = merge_app_data_doc(base, other);
        assert!(merged.metadata.hooks.is_some());
    }

    #[test]
    fn merge_app_data_doc_overrides_order_class() {
        use crate::app_data::types::OrderClassKind;
        let base = AppDataDoc::new("Base");
        let other = AppDataDoc::new("Other").with_order_class(OrderClassKind::Twap);
        let merged = merge_app_data_doc(base, other);
        assert!(merged.metadata.order_class.is_some());
    }

    #[test]
    fn merge_app_data_doc_overrides_partner_fee() {
        use crate::app_data::types::{PartnerFee, PartnerFeeEntry};
        let base = AppDataDoc::new("Base");
        let other = AppDataDoc {
            version: LATEST_APP_DATA_VERSION.to_owned(),
            app_code: None,
            environment: None,
            metadata: Metadata::default().with_partner_fee(PartnerFee::Single(
                PartnerFeeEntry::volume(50, "0x0000000000000000000000000000000000000001"),
            )),
        };
        let merged = merge_app_data_doc(base, other);
        assert!(merged.metadata.partner_fee.is_some());
    }

    #[test]
    fn merge_app_data_doc_overrides_replaced_order() {
        let base = AppDataDoc::new("Base");
        let uid = format!("0x{}", "ab".repeat(56));
        let other = AppDataDoc::new("Other").with_replaced_order(uid);
        let merged = merge_app_data_doc(base, other);
        assert!(merged.metadata.replaced_order.is_some());
    }

    #[test]
    fn sort_keys_handles_arrays_and_nested() {
        let v = serde_json::json!({
            "b": [{"z": 1, "a": 2}],
            "a": null,
        });
        let sorted = sort_keys(v);
        let s = serde_json::to_string(&sorted).unwrap();
        // "a" should come before "b" in the output
        let a_idx = s.find("\"a\"").unwrap();
        let b_idx = s.find("\"b\"").unwrap();
        assert!(a_idx < b_idx);
        // Inside the array, "a" should come before "z"
        let inner_a = s.rfind("\"a\"").unwrap();
        let inner_z = s.find("\"z\"").unwrap();
        assert!(inner_a < inner_z);
    }
}
