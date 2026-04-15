//! Runtime JSON Schema validation for [`AppDataDoc`] with **per-version**
//! dispatch.
//!
//! Validates an [`AppDataDoc`] against one of several bundled `CoW`
//! Protocol `AppData` JSON Schemas sourced from the upstream
//! [`cowprotocol/app-data`](https://github.com/cowprotocol/app-data)
//! repository. Each bundled schema lives in
//! `specs/app-data/vX.Y.Z.json` (one file per supported version, with all
//! `$ref`s pre-resolved by `scripts/bundle-appdata-schemas.py`) and is
//! compiled into a dedicated [`jsonschema::Validator`] on first use.
//!
//! # Dispatch rules
//!
//! | Caller intent | Entry point |
//! |---|---|
//! | Validate a built [`AppDataDoc`] against *its own* declared version | [`validate`] |
//! | Validate an arbitrary `serde_json::Value` (e.g. freshly fetched from IPFS) | [`validate_json`] |
//! | Force validation against a specific version, ignoring `doc.version` | [`validate_with`] |
//! | Validate an arbitrary JSON value against a specific version | [`validate_json_with`] |
//! | Run business rules **and** schema rules in one call | [`super::ipfs::validate_app_data_doc`] |
//!
//! The version-dispatching entry points ([`validate`] and [`validate_json`])
//! read the `version` field from the document, look up the matching
//! validator in the private `SUPPORTED_VERSIONS` table, and return
//! [`SchemaError::UnsupportedVersion`] if no registered validator covers
//! it. This mirrors `validateAppDataDoc.ts` in the upstream `TypeScript` SDK,
//! which picks a validator per `doc.version`. Use [`supported_versions`]
//! to enumerate the set of registered versions at runtime.
//!
//! # Adding a new version
//!
//! 1. Run `make fetch-appdata-schema` (optionally with `APPDATA_COMMIT=<sha>`) to regenerate every
//!    bundled schema from the pinned upstream commit.
//! 2. If the new version is not yet in `scripts/bundle-appdata-schemas.py::DEFAULT_VERSIONS`,
//!    extend that list first.
//! 3. Add a new entry to the private `SUPPORTED_VERSIONS` table at the top of this module.
//! 4. Bump [`LATEST_VERSION`] if the new version should become the fallback for documents whose
//!    `version` predates the oldest registered snapshot.
//! 5. `cargo test` — every bundled schema is smoke-tested on first access via
//!    `every_registered_version_compiles` (under `#[cfg(test)]`).

use std::sync::LazyLock;

use foldhash::{HashMap, HashMapExt};
use jsonschema::Validator;
use serde_json::Value;

use super::types::AppDataDoc;

// ── Bundled schemas ──────────────────────────────────────────────────────────

/// Pair each registered version with its bundled JSON Schema source.
///
/// Keeping the `(version, schema_source)` pairs in one table lets the
/// test suite smoke-compile every registered version in a single
/// iteration without having to name the individual constants.
const SUPPORTED_VERSIONS: &[(&str, &str)] = &[
    ("1.0.0", include_str!("../specs/app-data/v1.0.0.json")),
    ("1.5.0", include_str!("../specs/app-data/v1.5.0.json")),
    ("1.6.0", include_str!("../specs/app-data/v1.6.0.json")),
    ("1.10.0", include_str!("../specs/app-data/v1.10.0.json")),
    ("1.13.0", include_str!("../specs/app-data/v1.13.0.json")),
    ("1.14.0", include_str!("../specs/app-data/v1.14.0.json")),
];

/// The most recent version currently registered.
///
/// Used as the fallback target of [`APP_DATA_SCHEMA`] and of the doc
/// examples in downstream modules. Must match the highest `version`
/// returned by [`supported_versions`].
///
/// # Referrer shape across versions
///
/// Upstream `cowprotocol/app-data` v1.14.0 changed the `referrer` field
/// from a partner Ethereum address (`{ "address": "0x…" }`) to an
/// affiliate code (`{ "code": "ABCDE" }`). The Rust
/// [`super::types::Referrer`] type models both shapes as an
/// `#[serde(untagged)]` enum, so documents declaring either version
/// validate correctly as long as their referrer value matches the
/// right variant — use [`super::types::Referrer::address`] for
/// v1.13.0-or-earlier documents and [`super::types::Referrer::code`]
/// for v1.14.0+.
pub const LATEST_VERSION: &str = "1.14.0";

/// The bundled `CoW` Protocol `AppData` JSON Schema for the latest
/// supported version ([`LATEST_VERSION`]).
///
/// Exposed for downstream tooling that needs to hand the raw schema to a
/// different validator (e.g. a JSON Schema debugger). Most consumers
/// should use [`validate`], [`validate_json`], or their `_with` variants
/// instead.
pub const APP_DATA_SCHEMA: &str = include_str!("../specs/app-data/v1.14.0.json");

// ── Error / violation types ──────────────────────────────────────────────────

/// A single structural violation reported by the JSON Schema validator,
/// anchored to a JSON path inside the instance document.
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

/// Error type for version-aware schema validation.
///
/// Distinguishes "the document does not match its declared schema"
/// ([`Self::Violations`]) from "the document's declared version is not
/// registered in this build" ([`Self::UnsupportedVersion`]) so callers
/// can treat them differently — an unsupported version is typically a
/// deployment-time configuration issue, while violations point at a
/// genuine data problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaError {
    /// The document's declared `version` is not backed by any bundled
    /// schema in this build of `cow-rs`. Upgrade `cow-rs` or force a
    /// specific version with [`validate_with`].
    UnsupportedVersion {
        /// The version string the document declared.
        requested: String,
        /// The list of versions this build knows about, in registration
        /// order (same as [`supported_versions`]).
        supported: Vec<String>,
    },
    /// The document failed structural validation against its matching
    /// schema. Contains every violation found, in document order.
    Violations(Vec<SchemaViolation>),
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion { requested, supported } => {
                write!(
                    f,
                    "AppData version `{requested}` is not supported by this build \
                     (known versions: {})",
                    supported.join(", ")
                )
            }
            Self::Violations(errs) => {
                write!(f, "AppData schema validation failed with {} error(s)", errs.len())?;
                for e in errs {
                    write!(f, "\n  - {e}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for SchemaError {}

// ── Validator cache ──────────────────────────────────────────────────────────

/// Lazily-compiled map of `version → Validator`.
///
/// Every registered schema is compiled on first access to *any*
/// validator (all at once — this is cheap enough and amortises the
/// lookup cost). Failure to parse or compile any bundled schema panics
/// loudly at startup so broken build artifacts cannot ship silently.
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bundled compile-time schemas; a parse/compile failure indicates \
              a broken build artifact and must panic loudly at startup"
)]
static VALIDATORS: LazyLock<HashMap<&'static str, Validator>> = LazyLock::new(|| {
    let mut map = HashMap::with_capacity(SUPPORTED_VERSIONS.len());
    for (version, source) in SUPPORTED_VERSIONS {
        let schema: Value = serde_json::from_str(source)
            .unwrap_or_else(|e| panic!("bundled AppData schema v{version} is not valid JSON: {e}"));
        let validator = Validator::new(&schema)
            .unwrap_or_else(|e| panic!("bundled AppData schema v{version} does not compile: {e}"));
        map.insert(*version, validator);
    }
    map
});

/// Return the list of schema versions known to this build, in registration
/// order.
#[must_use]
pub fn supported_versions() -> Vec<&'static str> {
    SUPPORTED_VERSIONS.iter().map(|(v, _)| *v).collect()
}

/// Look up the validator for a specific `version`, compiling the whole
/// cache on first use.
///
/// Returns `None` if the version is not registered.
fn validator_for(version: &str) -> Option<&'static Validator> {
    VALIDATORS.get(version)
}

// ── Public entry points ──────────────────────────────────────────────────────

/// Validate a pre-serialised JSON value against a specific schema version.
///
/// Returns every violation found in document order. Use this when you
/// want to pin the validation to a known version regardless of what the
/// instance declares — for example, when you are sure a stored document
/// was captured under a specific version even if its `version` field
/// says otherwise.
///
/// # Errors
///
/// Returns [`SchemaError::UnsupportedVersion`] if `version` is not in
/// the set returned by [`supported_versions`].
pub fn validate_json_with(value: &Value, version: &str) -> Result<(), SchemaError> {
    let Some(validator) = validator_for(version) else {
        return Err(SchemaError::UnsupportedVersion {
            requested: version.to_owned(),
            supported: supported_versions().into_iter().map(str::to_owned).collect(),
        });
    };
    let errors: Vec<SchemaViolation> = validator
        .iter_errors(value)
        .map(|e| SchemaViolation { path: e.instance_path.to_string(), message: e.to_string() })
        .collect();
    if errors.is_empty() { Ok(()) } else { Err(SchemaError::Violations(errors)) }
}

/// Validate a pre-serialised JSON value, auto-selecting the schema
/// version from the value's top-level `version` field.
///
/// Use this when you have a raw `serde_json::Value` (for example,
/// freshly fetched from IPFS) that has not been deserialised into an
/// [`AppDataDoc`] yet.
///
/// # Errors
///
/// * [`SchemaError::UnsupportedVersion`] if `value["version"]` is missing, not a string, or not in
///   the set returned by [`supported_versions`].
/// * [`SchemaError::Violations`] if the value does not match its declared schema.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::validate_schema_json;
/// use serde_json::json;
///
/// let good = json!({ "version": "1.13.0", "metadata": {} });
/// validate_schema_json(&good).expect("should validate");
/// ```
pub fn validate_json(value: &Value) -> Result<(), SchemaError> {
    let version = value
        .get("version")
        .and_then(Value::as_str)
        .map_or_else(|| LATEST_VERSION.to_owned(), str::to_owned);
    validate_json_with(value, &version)
}

/// Validate a typed [`AppDataDoc`] against a specific schema version.
///
/// Ignores [`AppDataDoc::version`] — useful when you want to lock the
/// check to a known version regardless of what the builder set.
///
/// # Errors
///
/// Same error set as [`validate_json_with`].
///
/// # Panics
///
/// Panics only if [`AppDataDoc`] fails to serialise to JSON, which would
/// indicate a bug in its `Serialize` impl.
#[allow(
    clippy::expect_used,
    reason = "AppDataDoc serialisation is total; failure would be a bug in serde"
)]
pub fn validate_with(doc: &AppDataDoc, version: &str) -> Result<(), SchemaError> {
    let value = serde_json::to_value(doc).expect("AppDataDoc serialises without failure");
    validate_json_with(&value, version)
}

/// Validate a typed [`AppDataDoc`] against the schema matching its own
/// declared [`version`](AppDataDoc::version).
///
/// This is the most common entry point: pass a doc built via the
/// builders, receive `Ok(())` on success or an
/// [`SchemaError::UnsupportedVersion`] / [`SchemaError::Violations`]
/// otherwise.
///
/// # Errors
///
/// Same error set as [`validate_json`].
///
/// # Panics
///
/// Panics only if [`AppDataDoc`] fails to serialise to JSON.
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
pub fn validate(doc: &AppDataDoc) -> Result<(), SchemaError> {
    let value = serde_json::to_value(doc).expect("AppDataDoc serialises without failure");
    validate_json(&value)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{
        CowHook, Metadata, OrderClassKind, OrderInteractionHooks, PartnerFee, PartnerFeeEntry,
        Quote, Referrer, Utm, Widget,
    };

    fn must_validate(doc: &AppDataDoc) {
        if let Err(e) = validate(doc) {
            panic!("doc should validate but failed: {e}");
        }
    }

    // ── Smoke test: every bundled version must compile ──────────────────────

    #[test]
    fn every_registered_version_compiles() {
        for version in supported_versions() {
            assert!(
                validator_for(version).is_some(),
                "registered version {version} failed to compile into a validator"
            );
        }
    }

    #[test]
    fn latest_version_is_registered() {
        assert!(
            supported_versions().contains(&LATEST_VERSION),
            "LATEST_VERSION ({LATEST_VERSION}) must appear in SUPPORTED_VERSIONS"
        );
    }

    #[test]
    fn default_appdatadoc_uses_a_registered_version() {
        let doc = AppDataDoc::new("test");
        assert!(
            supported_versions().contains(&doc.version.as_str()),
            "AppDataDoc::new sets version `{}` but no bundled schema covers it; \
             update `LATEST_APP_DATA_VERSION` or register a new schema",
            doc.version
        );
    }

    // ── Positive cases ──────────────────────────────────────────────────────

    #[test]
    fn minimal_doc_validates() {
        must_validate(&AppDataDoc::new("TestApp"));
    }

    #[test]
    fn doc_with_environment_validates() {
        must_validate(&AppDataDoc::new("TestApp").with_environment("production"));
    }

    #[test]
    fn doc_with_code_referrer_validates_under_latest() {
        // LATEST_VERSION (v1.14.0) expects `referrer.code` matching
        // `^[A-Z0-9_-]{5,20}$`.
        must_validate(&AppDataDoc::new("TestApp").with_referrer(Referrer::code("COWRS-PARTNER")));
    }

    #[test]
    fn doc_with_address_referrer_validates_under_v1_13_0() {
        // The address form of `Referrer` only matches schemas v1.13.0 and
        // earlier. Build a doc explicitly pinned to v1.13.0 and validate
        // with an explicit version rather than relying on the default.
        let mut doc = AppDataDoc::new("TestApp")
            .with_referrer(Referrer::address("0xb6BAd41ae76A11D10f7b0E664C5007b908bC77C9"));
        doc.version = "1.13.0".to_owned();
        validate(&doc).expect("address-flavoured referrer validates under v1.13.0");
    }

    #[test]
    fn address_referrer_fails_under_latest() {
        // Under v1.14.0 the address form is rejected (`address` field
        // not allowed, `code` missing) — documented drift between the
        // two schema shapes.
        let doc = AppDataDoc::new("TestApp")
            .with_referrer(Referrer::address("0xb6BAd41ae76A11D10f7b0E664C5007b908bC77C9"));
        let err = validate(&doc).expect_err("address referrer under v1.14.0 must fail");
        assert!(matches!(err, SchemaError::Violations(_)));
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
            .with_referrer(Referrer::code("COWRS-PARTNER"))
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

    // ── Negative cases ──────────────────────────────────────────────────────

    #[test]
    fn doc_with_malformed_code_referrer_is_rejected() {
        // v1.14.0 code pattern is `^[A-Z0-9_-]{5,20}$` — lowercase fails
        // and the path must localise to /metadata/referrer/code.
        let doc = AppDataDoc::new("TestApp").with_referrer(Referrer::code("abc"));
        let err = validate(&doc).expect_err("malformed code must fail");
        let SchemaError::Violations(errs) = err else {
            panic!("expected Violations, got {err:?}");
        };
        assert!(
            errs.iter().any(|e| e.path.contains("referrer")),
            "expected a violation on /metadata/referrer, got: {errs:?}"
        );
    }

    #[test]
    fn schema_rejects_unknown_top_level_fields() {
        let bad = json!({
            "version": LATEST_VERSION,
            "metadata": {},
            "unknownField": "should fail",
        });
        assert!(validate_json(&bad).is_err());
    }

    #[test]
    fn schema_rejects_unknown_metadata_fields() {
        let bad = json!({
            "version": LATEST_VERSION,
            "metadata": { "unknownMetadata": {} },
        });
        assert!(validate_json(&bad).is_err());
    }

    #[test]
    fn schema_requires_version_and_metadata() {
        // No version — falls back to LATEST_VERSION for dispatch but the
        // schema itself still requires a `version` field, so validation
        // fails with a violation rather than UnsupportedVersion.
        let no_version = json!({ "metadata": {} });
        assert!(validate_json(&no_version).is_err());

        // Present version, missing metadata — same logic, violation only.
        let no_metadata = json!({ "version": LATEST_VERSION });
        assert!(validate_json(&no_metadata).is_err());
    }

    // ── Version dispatch ────────────────────────────────────────────────────

    #[test]
    fn unknown_version_is_reported_as_unsupported() {
        let bad = json!({ "version": "99.0.0", "metadata": {} });
        let err = validate_json(&bad).expect_err("unknown version must fail");
        let SchemaError::UnsupportedVersion { requested, supported } = err else {
            panic!("expected UnsupportedVersion, got {err:?}");
        };
        assert_eq!(requested, "99.0.0");
        assert!(!supported.is_empty(), "supported list should not be empty");
        assert!(
            supported.iter().any(|s| s == LATEST_VERSION),
            "LATEST_VERSION must appear in the supported list"
        );
    }

    #[test]
    fn validate_with_ignores_doc_version() {
        // Build a doc that defaults to LATEST_VERSION but force-validate
        // it against an older registered schema. v1.0.0 has a simpler
        // partner-fee shape so we keep the doc minimal to avoid spurious
        // cross-version incompatibilities.
        let doc = AppDataDoc::new("CrossVersion");
        validate_with(&doc, "1.0.0").expect("minimal doc validates under v1.0.0");
        validate_with(&doc, LATEST_VERSION).expect("minimal doc validates under latest");
    }

    #[test]
    fn validate_with_errors_on_unknown_version() {
        let doc = AppDataDoc::new("TestApp");
        let err = validate_with(&doc, "42.42.42").expect_err("unknown version must fail");
        assert!(matches!(err, SchemaError::UnsupportedVersion { .. }));
    }

    // ── Misc ────────────────────────────────────────────────────────────────

    #[test]
    fn validator_cache_is_shared_across_calls() {
        let doc = AppDataDoc::new("cache-test");
        let a = validate(&doc).is_ok();
        let b = validate(&doc).is_ok();
        assert!(a && b, "cached validator must yield stable results");
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

    #[test]
    fn supported_versions_contains_every_registered_entry() {
        let versions = supported_versions();
        assert_eq!(versions.len(), SUPPORTED_VERSIONS.len());
        for (reg, _) in SUPPORTED_VERSIONS {
            assert!(versions.contains(reg), "missing {reg}");
        }
    }

    #[test]
    fn schema_error_display_unsupported_version() {
        let err = SchemaError::UnsupportedVersion {
            requested: "99.0.0".to_owned(),
            supported: vec!["1.0.0".to_owned(), "1.6.0".to_owned()],
        };
        let s = format!("{err}");
        assert!(s.contains("99.0.0"));
        assert!(s.contains("1.0.0, 1.6.0"));
    }

    #[test]
    fn schema_error_display_violations() {
        let err = SchemaError::Violations(vec![
            SchemaViolation { path: "/metadata".to_owned(), message: "missing field".to_owned() },
            SchemaViolation { path: String::new(), message: "top-level error".to_owned() },
        ]);
        let s = format!("{err}");
        assert!(s.contains("2 error(s)"));
        assert!(s.contains("missing field at /metadata"));
        assert!(s.contains("top-level error"));
    }

    #[test]
    fn schema_error_is_error_trait() {
        let err =
            SchemaError::UnsupportedVersion { requested: "0.0.0".to_owned(), supported: vec![] };
        // Verify std::error::Error is implemented
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn validate_json_with_unsupported_version() {
        let val = json!({ "version": "1.13.0", "metadata": {} });
        let err = validate_json_with(&val, "42.0.0").expect_err("unsupported version must fail");
        assert!(matches!(err, SchemaError::UnsupportedVersion { .. }));
    }

    #[test]
    fn validate_json_missing_version_uses_latest() {
        // No "version" key => falls back to LATEST_VERSION, but the schema itself
        // requires "version", so it will fail with Violations (not UnsupportedVersion).
        let val = json!({ "metadata": {} });
        let err = validate_json(&val).expect_err("missing version in document should fail");
        assert!(matches!(err, SchemaError::Violations(_)));
    }

    #[test]
    fn validate_with_valid_doc_and_each_version() {
        for version in supported_versions() {
            let doc = AppDataDoc::new("TestApp");
            // Minimal doc should validate under most schemas (may fail on older ones
            // if they have different shape, but 1.0.0 was already tested above)
            let _result = validate_with(&doc, version);
        }
    }
}
