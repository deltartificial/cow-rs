//! Runtime JSON Schema validation for [`AppDataDoc`].
//!
//! Validates an [`AppDataDoc`] against the bundled `CoW` Protocol `AppData`
//! JSON Schema at `specs/app-data-schema.json` (draft-07, sourced from the
//! upstream `cowprotocol/app-data` repository).
//!
//! # Why this exists
//!
//! The existing [`validate_app_data_doc`](super::ipfs::validate_app_data_doc)
//! already enforces business-rule constraints (address format, `bps` caps,
//! order-class enums, `appCode` length, …) via the helpers in the private
//! `validation` helper module. Those checks do **not** verify that the
//! document's **structural shape** — required fields, additional-property
//! rules, `$ref` targets, `anyOf`/`oneOf` variants — matches the upstream
//! spec.
//!
//! This module fills that gap with an [`jsonschema::Validator`] compiled
//! once at first use via [`LazyLock`]. Consumers can:
//!
//! | Entry point | Use when … |
//! |---|---|
//! | [`validate`] | You have an [`AppDataDoc`] and want a structured list of violations |
//! | [`validate_json`] | You have a raw `serde_json::Value` (e.g. freshly fetched from IPFS) |
//! | [`super::ipfs::validate_app_data_doc`] | You want business rules **and** schema rules in one call |
//!
//! # Single-version limitation
//!
//! The bundled schema is a **single snapshot** — the upstream
//! [`cowprotocol/app-data`](https://github.com/cowprotocol/app-data)
//! repository publishes 20+ versioned schemas (`v0.1.0.json` →
//! `v1.6.0.json`) and the `TypeScript` SDK picks the right validator based on
//! the incoming `doc.version`. Matching that behaviour would require
//! vendoring each historical schema and dispatching at runtime — that work
//! is tracked separately.

use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::Value;

use super::types::AppDataDoc;

/// The bundled `CoW` Protocol `AppData` JSON Schema, resolved with every
/// `$ref` inlined (see the `make fetch-appdata-schema` Makefile target).
///
/// Exposed mainly for downstream tooling that needs to hand the raw schema
/// to a different validator (e.g. a JSON Schema debugger). Most consumers
/// should use [`validate`] or [`validate_json`] instead.
pub const APP_DATA_SCHEMA: &str = include_str!("../../../../specs/app-data-schema.json");

/// A single structural violation reported by [`validate`] or
/// [`validate_json`], anchored to a JSON path inside the document.
///
/// The [`Display`](std::fmt::Display) impl renders as
/// `"{message} at {path}"`, matching the format used by the existing
/// [`super::ipfs::ValidationResult::errors`] list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaViolation {
    /// JSON pointer into the instance that triggered the violation, e.g.
    /// `/metadata/referrer/address`. Empty for top-level violations.
    pub path: String,
    /// Human-readable description of the violation, copied verbatim from
    /// the `Display` impl of `jsonschema::ValidationError`.
    pub message: String,
}

impl std::fmt::Display for SchemaViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.path.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{} at {}", self.message, self.path)
        }
    }
}

/// Lazily-compiled, process-wide validator for the bundled schema.
///
/// Compilation happens on first use and is shared across all subsequent
/// calls. Panics on first access if the bundled schema is not valid JSON
/// or not a valid draft-07 schema — both of which can only happen from a
/// broken `make fetch-appdata-schema` and should be caught in CI via the
/// test suite of this module.
#[allow(
    clippy::expect_used,
    reason = "bundled compile-time schema; a parse/compile failure indicates \
              a broken build artifact and must panic loudly at startup"
)]
static VALIDATOR: LazyLock<Validator> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(APP_DATA_SCHEMA)
        .expect("bundled AppData JSON Schema must be valid JSON");
    Validator::new(&schema).expect("bundled AppData JSON Schema must compile")
});

/// Validate a pre-serialised JSON value against the bundled schema.
///
/// Returns all violations found in document order — an empty `Vec` means
/// the value is structurally valid. Use this when you have a raw
/// `serde_json::Value` (for example, freshly fetched from IPFS before it
/// has been deserialised into an [`AppDataDoc`]).
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{validate_schema_json};
/// use serde_json::json;
///
/// let good = json!({ "version": "1.6.0", "metadata": {} });
/// assert!(validate_schema_json(&good).is_empty());
///
/// let bad = json!({ "version": "1.6.0" }); // missing required `metadata`
/// assert!(!validate_schema_json(&bad).is_empty());
/// ```
#[must_use]
pub fn validate_json(value: &Value) -> Vec<SchemaViolation> {
    VALIDATOR
        .iter_errors(value)
        .map(|e| SchemaViolation { path: e.instance_path.to_string(), message: e.to_string() })
        .collect()
}

/// Validate a typed [`AppDataDoc`] against the bundled schema.
///
/// Returns `Ok(())` when the document is structurally valid, or
/// `Err(violations)` with every rule that failed. Use this when you have
/// built a doc through the [`AppDataDoc`] builders and want a quick
/// schema gate before submitting it to the orderbook.
///
/// # Errors
///
/// Returns a non-empty `Vec<SchemaViolation>` when the serialised document
/// does not match the bundled JSON Schema.
///
/// # Panics
///
/// Panics only if [`AppDataDoc`] fails to serialise to JSON, which would
/// indicate a bug in its `Serialize` impl. Downstream callers can treat
/// this as "cannot happen in practice".
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{AppDataDoc, validate_schema};
///
/// let doc = AppDataDoc::new("my-app");
/// validate_schema(&doc).expect("minimal doc should validate");
/// ```
#[allow(
    clippy::expect_used,
    reason = "AppDataDoc serialisation is total; failure would be a bug in serde"
)]
pub fn validate(doc: &AppDataDoc) -> Result<(), Vec<SchemaViolation>> {
    let value = serde_json::to_value(doc).expect("AppDataDoc serialises without failure");
    let errors = validate_json(&value);
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::app_data::{
        CowHook, Metadata, OrderClassKind, OrderInteractionHooks, PartnerFee, PartnerFeeEntry,
        Quote, Referrer, Utm, Widget,
    };

    fn must_validate(doc: &AppDataDoc) {
        if let Err(errs) = validate(doc) {
            panic!(
                "doc should validate but produced {} error(s):\n  {}",
                errs.len(),
                errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n  ")
            );
        }
    }

    #[test]
    fn minimal_doc_validates() {
        must_validate(&AppDataDoc::new("TestApp"));
    }

    #[test]
    fn doc_with_environment_validates() {
        must_validate(&AppDataDoc::new("TestApp").with_environment("production"));
    }

    #[test]
    fn doc_with_referrer_validates() {
        must_validate(
            &AppDataDoc::new("TestApp")
                .with_referrer(Referrer::new("0xb6BAd41ae76A11D10f7b0E664C5007b908bC77C9")),
        );
    }

    #[test]
    fn doc_with_malformed_referrer_is_rejected() {
        let doc = AppDataDoc::new("TestApp").with_referrer(Referrer::new("not-an-address"));
        let errors = validate(&doc).expect_err("malformed referrer must fail");
        assert!(
            errors.iter().any(|e| e.path.contains("referrer")),
            "expected at least one violation on /metadata/referrer, got: {errors:?}"
        );
    }

    #[test]
    fn doc_with_utm_validates() {
        must_validate(&AppDataDoc::new("TestApp").with_utm(Utm {
            utm_source: Some("twitter".into()),
            utm_medium: Some("social".into()),
            utm_campaign: Some("launch".into()),
            utm_content: Some("banner".into()),
            utm_term: Some("cow".into()),
        }));
    }

    #[test]
    fn doc_with_quote_validates() {
        let mut doc = AppDataDoc::new("TestApp");
        doc.metadata = doc.metadata.with_quote(Quote::new(50));
        must_validate(&doc);
    }

    #[test]
    fn doc_with_each_order_class_validates() {
        for kind in [
            OrderClassKind::Market,
            OrderClassKind::Limit,
            OrderClassKind::Liquidity,
            OrderClassKind::Twap,
        ] {
            must_validate(&AppDataDoc::new("TestApp").with_order_class(kind));
        }
    }

    #[test]
    fn doc_with_hooks_validates() {
        let hook =
            CowHook::new("0x0000000000000000000000000000000000000001", "0xdeadbeef", "100000");
        let hooks = OrderInteractionHooks::new(vec![hook.clone()], vec![hook]);
        must_validate(&AppDataDoc::new("TestApp").with_hooks(hooks));
    }

    #[test]
    fn doc_with_widget_validates() {
        let mut doc = AppDataDoc::new("TestApp");
        doc.metadata = doc.metadata.with_widget(Widget::new("WidgetApp"));
        must_validate(&doc);
    }

    #[test]
    fn doc_with_partner_fee_validates() {
        let fee = PartnerFee::Single(PartnerFeeEntry::volume(
            50,
            "0x0000000000000000000000000000000000000001",
        ));
        must_validate(&AppDataDoc::new("TestApp").with_partner_fee(fee));
    }

    #[test]
    fn doc_with_replaced_order_validates() {
        let uid = format!("0x{}", "ab".repeat(56));
        must_validate(&AppDataDoc::new("TestApp").with_replaced_order(uid));
    }

    #[test]
    fn doc_with_signer_validates() {
        must_validate(
            &AppDataDoc::new("TestApp").with_signer("0x0000000000000000000000000000000000000001"),
        );
    }

    #[test]
    fn fully_populated_doc_validates() {
        let hook =
            CowHook::new("0x0000000000000000000000000000000000000001", "0xdeadbeef", "100000");
        let metadata = Metadata::default()
            .with_referrer(Referrer::new("0xb6BAd41ae76A11D10f7b0E664C5007b908bC77C9"))
            .with_utm(Utm {
                utm_source: Some("src".into()),
                utm_medium: Some("med".into()),
                utm_campaign: Some("camp".into()),
                utm_content: None,
                utm_term: None,
            })
            .with_quote(Quote::new(100))
            .with_hooks(OrderInteractionHooks::new(vec![hook.clone()], vec![hook]))
            .with_widget(Widget::new("MyWidget"))
            .with_partner_fee(PartnerFee::Single(PartnerFeeEntry::volume(
                25,
                "0x0000000000000000000000000000000000000002",
            )));
        let mut doc = AppDataDoc::new("FullApp")
            .with_environment("production")
            .with_order_class(OrderClassKind::Limit)
            .with_signer("0x0000000000000000000000000000000000000003");
        doc.metadata = metadata;
        must_validate(&doc);
    }

    #[test]
    fn schema_rejects_unknown_top_level_fields() {
        let bad = json!({
            "version": "1.6.0",
            "metadata": {},
            "unknownField": "should fail",
        });
        assert!(!validate_json(&bad).is_empty());
    }

    #[test]
    fn schema_rejects_unknown_metadata_fields() {
        let bad = json!({
            "version": "1.6.0",
            "metadata": { "unknownMetadata": {} },
        });
        assert!(!validate_json(&bad).is_empty());
    }

    #[test]
    fn schema_requires_version_and_metadata() {
        let no_version = json!({ "metadata": {} });
        assert!(!validate_json(&no_version).is_empty());

        let no_metadata = json!({ "version": "1.6.0" });
        assert!(!validate_json(&no_metadata).is_empty());
    }

    #[test]
    fn validator_is_shared_across_calls() {
        // Exercise the LazyLock cache on two consecutive calls — if the
        // validator were re-built each time this would still work but be
        // very slow. We just assert both calls return the same thing.
        let doc = AppDataDoc::new("cache-test");
        let a = validate(&doc);
        let b = validate(&doc);
        assert_eq!(a.is_ok(), b.is_ok());
    }

    #[test]
    fn schema_violation_display_format() {
        let v = SchemaViolation {
            path: "/metadata/referrer".to_owned(),
            message: "missing required field".to_owned(),
        };
        assert_eq!(v.to_string(), "missing required field at /metadata/referrer");

        let v = SchemaViolation { path: String::new(), message: "root-level error".to_owned() };
        assert_eq!(v.to_string(), "root-level error");
    }
}
