//! Extended constraint validation for [`AppDataDoc`] — strict schema rules.
//!
//! This module provides the [`ValidationError`] type that describes specific
//! constraint violations, and the `pub(super)` helper
//! [`validate_constraints`] used by
//! [`validate_app_data_doc`](super::ipfs::validate_app_data_doc) to check
//! every field of an [`AppDataDoc`].
//!
//! # Validation rules
//!
//! | Field | Rule |
//! |---|---|
//! | `appCode` | Non-empty, ≤ 50 characters |
//! | `version` | Valid semver (`x.y.z`) |
//! | Hook `target` | `0x` + 40 hex chars |
//! | Hook `gasLimit` | Parseable as decimal `u64` |
//! | `partnerFee` bps | Each ≤ 10 000 |
//! | `orderClass` | One of `market`, `limit`, `liquidity`, `twap` |
//! | `replacedOrder.uid` | `0x` + 112 hex chars (56 bytes) |

use super::types::{AppDataDoc, CowHook, Metadata, OrderInteractionHooks, PartnerFee};

// ── ValidationError ────────────────────────────────────────────────────────

/// A specific constraint violation found when validating an [`AppDataDoc`].
///
/// Every variant carries enough context to display a useful diagnostic
/// message via its [`Display`](std::fmt::Display) implementation. Variants
/// are returned inside
/// [`ValidationResult::typed_errors`](super::ipfs::ValidationResult::typed_errors)
/// for programmatic inspection.
///
/// # Example
///
/// ```
/// use cow_app_data::{AppDataDoc, ValidationError, validate_app_data_doc};
///
/// let doc = AppDataDoc::new(""); // empty appCode triggers InvalidAppCode
/// let result = validate_app_data_doc(&doc);
/// assert!(result.errors_ref().iter().any(|e| matches!(e, ValidationError::InvalidAppCode(_))));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// `appCode` is empty or exceeds 50 characters.
    InvalidAppCode(String),
    /// `metadata.version` is not a valid semver string (`x.y.z`).
    InvalidVersion(String),
    /// A hook `target` is not a valid Ethereum address (`0x` + 40 hex chars).
    InvalidHookTarget {
        /// The invalid target string.
        hook: String,
        /// Human-readable reason.
        reason: String,
    },
    /// A hook `gasLimit` is not parseable as a decimal `u64`.
    InvalidHookGasLimit {
        /// The invalid gas-limit string.
        gas_limit: String,
    },
    /// `partnerFee` entry `volumeBps` / `surplusBps` / `priceImprovementBps` exceeds 10 000 (100
    /// %).
    PartnerFeeBpsTooHigh(u32),
    /// `orderClass.orderClass` contains an unrecognised variant.
    UnknownOrderClass(String),
    /// A `replacedOrder` UID is not 56 bytes (i.e. not `"0x"` + 112 hex chars).
    InvalidReplacedOrderUid(String),
    /// A structural violation reported by the bundled JSON Schema validator
    /// in [`super::schema`] — e.g. a missing required field, an unknown
    /// property, or a value that does not match a regex / enum constraint.
    SchemaViolation {
        /// JSON pointer into the instance that triggered the violation.
        path: String,
        /// Human-readable description of the violation.
        message: String,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAppCode(s) => write!(f, "invalid appCode: {s}"),
            Self::InvalidVersion(s) => write!(f, "invalid version: {s}"),
            Self::InvalidHookTarget { hook, reason } => {
                write!(f, "invalid hook target '{hook}': {reason}")
            }
            Self::InvalidHookGasLimit { gas_limit } => {
                write!(f, "invalid hook gasLimit '{gas_limit}': not a valid u64")
            }
            Self::PartnerFeeBpsTooHigh(bps) => {
                write!(f, "partnerFee bps {bps} exceeds 10 000 (100 %)")
            }
            Self::UnknownOrderClass(s) => write!(f, "unknown orderClass '{s}'"),
            Self::InvalidReplacedOrderUid(s) => {
                write!(f, "invalid replacedOrder uid '{s}': expected 0x + 112 hex chars")
            }
            Self::SchemaViolation { path, message } => {
                if path.is_empty() {
                    write!(f, "schema: {message}")
                } else {
                    write!(f, "schema: {message} at {path}")
                }
            }
        }
    }
}

// ── Public entry-point ─────────────────────────────────────────────────────

/// Validate all constraint rules of `doc` and push any violations into
/// `errors`.
///
/// Called by
/// [`validate_app_data_doc`](super::ipfs::validate_app_data_doc) after the
/// basic version check. Walks the document's `app_code` and `metadata`
/// fields, delegating to per-field validators that append
/// [`ValidationError`] entries for every violated rule.
///
/// # Parameters
///
/// * `doc` — the [`AppDataDoc`] to validate.
/// * `errors` — mutable list to which violations are appended.
pub(super) fn validate_constraints(doc: &AppDataDoc, errors: &mut Vec<ValidationError>) {
    validate_app_code(doc.app_code.as_deref(), errors);
    validate_metadata(&doc.metadata, errors);
}

// ── Private helpers ────────────────────────────────────────────────────────

/// Validate the `appCode` field.
///
/// Allowed: non-empty string of at most 50 characters. `None` values are
/// valid (the field is optional in the schema).
///
/// # Parameters
///
/// * `app_code` — the `appCode` value to validate (`None` = not set).
/// * `errors` — mutable list to which violations are appended.
fn validate_app_code(app_code: Option<&str>, errors: &mut Vec<ValidationError>) {
    let Some(code) = app_code else { return };
    if code.is_empty() {
        errors.push(ValidationError::InvalidAppCode("appCode must not be empty".to_owned()));
    } else if code.len() > 50 {
        errors.push(ValidationError::InvalidAppCode(format!(
            "appCode '{}' exceeds 50 characters (got {})",
            code,
            code.len()
        )));
    }
}

/// Validate the `metadata` block.
///
/// Delegates to per-field validators for hooks, partner fees, order class,
/// and replaced-order UID. Fields that are `None` are silently skipped.
///
/// # Parameters
///
/// * `meta` — the [`Metadata`] block to validate.
/// * `errors` — mutable list to which violations are appended.
fn validate_metadata(meta: &Metadata, errors: &mut Vec<ValidationError>) {
    if let Some(hooks) = &meta.hooks {
        validate_hooks(hooks, errors);
    }
    if let Some(fee) = &meta.partner_fee {
        validate_partner_fee(fee, errors);
    }
    if let Some(oc) = &meta.order_class {
        // OrderClassKind is an enum — all known variants are valid; only a
        // round-trip through serde could produce an unknown variant, but we
        // still expose the check through the `as_str` method for completeness.
        let known = ["market", "limit", "liquidity", "twap"];
        let s = oc.order_class.as_str();
        if !known.contains(&s) {
            errors.push(ValidationError::UnknownOrderClass(s.to_owned()));
        }
    }
    if let Some(ro) = &meta.replaced_order {
        validate_replaced_order_uid(&ro.uid, errors);
    }
}

/// Validate all hooks inside an [`OrderInteractionHooks`] block.
///
/// Iterates over both `pre` and `post` hook lists and validates each
/// individual [`CowHook`] via [`validate_single_hook`].
///
/// # Parameters
///
/// * `hooks` — the hooks block to validate.
/// * `errors` — mutable list to which violations are appended.
fn validate_hooks(hooks: &OrderInteractionHooks, errors: &mut Vec<ValidationError>) {
    if let Some(pre) = &hooks.pre {
        for hook in pre {
            validate_single_hook(hook, errors);
        }
    }
    if let Some(post) = &hooks.post {
        for hook in post {
            validate_single_hook(hook, errors);
        }
    }
}

/// Validate a single [`CowHook`].
///
/// Two rules are checked:
///
/// 1. `target` must be a valid Ethereum address (`"0x"` + 40 hex chars, case-insensitive). Produces
///    [`ValidationError::InvalidHookTarget`].
/// 2. `gas_limit` must parse as a decimal `u64`. Produces [`ValidationError::InvalidHookGasLimit`].
///
/// # Parameters
///
/// * `hook` — the [`CowHook`] to validate.
/// * `errors` — mutable list to which violations are appended.
fn validate_single_hook(hook: &CowHook, errors: &mut Vec<ValidationError>) {
    if !is_eth_address(&hook.target) {
        errors.push(ValidationError::InvalidHookTarget {
            hook: hook.target.clone(),
            reason: "expected 0x-prefixed 20-byte hex address".to_owned(),
        });
    }
    if hook.gas_limit.parse::<u64>().is_err() {
        errors.push(ValidationError::InvalidHookGasLimit { gas_limit: hook.gas_limit.clone() });
    }
}

/// Validate every basis-point value in a [`PartnerFee`].
///
/// Iterates all entries (single or multiple) and checks that each of
/// `volume_bps`, `surplus_bps`, and `price_improvement_bps` is ≤ 10 000
/// (= 100 %) when present. Produces [`ValidationError::PartnerFeeBpsTooHigh`]
/// for every value that exceeds the cap.
///
/// # Parameters
///
/// * `fee` — the [`PartnerFee`] to validate.
/// * `errors` — mutable list to which violations are appended.
fn validate_partner_fee(fee: &PartnerFee, errors: &mut Vec<ValidationError>) {
    for entry in fee.entries() {
        for bps in
            [entry.volume_bps, entry.surplus_bps, entry.price_improvement_bps].into_iter().flatten()
        {
            if bps > 10_000 {
                errors.push(ValidationError::PartnerFeeBpsTooHigh(bps));
            }
        }
    }
}

/// Validate a `replacedOrder.uid` string.
///
/// Expected format: `"0x"` followed by exactly 112 hex characters
/// (= 56 bytes = the `CoW` Protocol order-UID format: 32 bytes order hash
/// + 20 bytes owner address + 4 bytes valid-to timestamp).
///
/// Hex digits are accepted in any case (the protocol normalises to
/// lowercase).
///
/// # Parameters
///
/// * `uid` — the order UID string to validate.
/// * `errors` — mutable list to which violations are appended.
fn validate_replaced_order_uid(uid: &str, errors: &mut Vec<ValidationError>) {
    // 2 (prefix) + 112 (hex) = 114
    let valid = uid.len() == 114 &&
        uid.starts_with("0x") &&
        uid[2..].chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        errors.push(ValidationError::InvalidReplacedOrderUid(uid.to_owned()));
    }
}

/// Return `true` when `s` looks like a valid Ethereum address.
///
/// Accepts `"0x"` + exactly 40 ASCII hex characters (case-insensitive).
/// Does **not** enforce `EIP-55` mixed-case checksum because that would
/// require a `keccak256` call and is not part of the `CoW` Protocol
/// app-data schema validation rules.
///
/// # Parameters
///
/// * `s` — the string to check.
///
/// # Returns
///
/// `true` if `s` matches the `0x[0-9a-fA-F]{40}` pattern.
fn is_eth_address(s: &str) -> bool {
    s.len() == 42 && s.starts_with("0x") && s[2..].chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{OrderClass, OrderClassKind, PartnerFeeEntry, ReplacedOrder};

    #[test]
    fn validate_app_code_empty() {
        let mut errors = Vec::new();
        validate_app_code(Some(""), &mut errors);
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], ValidationError::InvalidAppCode(_)));
    }

    #[test]
    fn validate_app_code_too_long() {
        let mut errors = Vec::new();
        let long = "A".repeat(51);
        validate_app_code(Some(&long), &mut errors);
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], ValidationError::InvalidAppCode(_)));
    }

    #[test]
    fn validate_app_code_none_is_ok() {
        let mut errors = Vec::new();
        validate_app_code(None, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_app_code_valid() {
        let mut errors = Vec::new();
        validate_app_code(Some("MyApp"), &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_hook_invalid_target() {
        let mut errors = Vec::new();
        let hook = CowHook {
            target: "not-an-address".to_owned(),
            call_data: "0x".to_owned(),
            gas_limit: "100000".to_owned(),
            dapp_id: None,
        };
        validate_single_hook(&hook, &mut errors);
        assert!(errors.iter().any(|e| matches!(e, ValidationError::InvalidHookTarget { .. })));
    }

    #[test]
    fn validate_hook_invalid_gas_limit() {
        let mut errors = Vec::new();
        let hook = CowHook {
            target: "0x1111111111111111111111111111111111111111".to_owned(),
            call_data: "0x".to_owned(),
            gas_limit: "not-a-number".to_owned(),
            dapp_id: None,
        };
        validate_single_hook(&hook, &mut errors);
        assert!(errors.iter().any(|e| matches!(e, ValidationError::InvalidHookGasLimit { .. })));
    }

    #[test]
    fn validate_hook_valid() {
        let mut errors = Vec::new();
        let hook = CowHook {
            target: "0x1111111111111111111111111111111111111111".to_owned(),
            call_data: "0x".to_owned(),
            gas_limit: "100000".to_owned(),
            dapp_id: None,
        };
        validate_single_hook(&hook, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_partner_fee_bps_too_high() {
        let mut errors = Vec::new();
        let fee = PartnerFee::Single(PartnerFeeEntry {
            recipient: "0x1111111111111111111111111111111111111111".to_owned(),
            volume_bps: Some(10_001),
            surplus_bps: None,
            price_improvement_bps: None,
            max_volume_bps: None,
        });
        validate_partner_fee(&fee, &mut errors);
        assert!(errors.iter().any(|e| matches!(e, ValidationError::PartnerFeeBpsTooHigh(10_001))));
    }

    #[test]
    fn validate_partner_fee_valid() {
        let mut errors = Vec::new();
        let fee = PartnerFee::Single(PartnerFeeEntry {
            recipient: "0x1111111111111111111111111111111111111111".to_owned(),
            volume_bps: Some(500),
            surplus_bps: None,
            price_improvement_bps: None,
            max_volume_bps: None,
        });
        validate_partner_fee(&fee, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_replaced_order_uid_valid() {
        let mut errors = Vec::new();
        let uid = format!("0x{}", "ab".repeat(56));
        validate_replaced_order_uid(&uid, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_replaced_order_uid_invalid() {
        let mut errors = Vec::new();
        validate_replaced_order_uid("0xshort", &mut errors);
        assert!(errors.iter().any(|e| matches!(e, ValidationError::InvalidReplacedOrderUid(_))));
    }

    #[test]
    fn validation_error_display_all_variants() {
        let err = ValidationError::InvalidAppCode("test".into());
        assert!(err.to_string().contains("test"));

        let err = ValidationError::InvalidVersion("bad".into());
        assert!(err.to_string().contains("bad"));

        let err = ValidationError::InvalidHookTarget { hook: "foo".into(), reason: "bar".into() };
        assert!(err.to_string().contains("foo"));

        let err = ValidationError::InvalidHookGasLimit { gas_limit: "xyz".into() };
        assert!(err.to_string().contains("xyz"));

        let err = ValidationError::PartnerFeeBpsTooHigh(20_000);
        assert!(err.to_string().contains("20000"));

        let err = ValidationError::UnknownOrderClass("unknown".into());
        assert!(err.to_string().contains("unknown"));

        let err = ValidationError::InvalidReplacedOrderUid("0xshort".into());
        assert!(err.to_string().contains("0xshort"));

        let err = ValidationError::SchemaViolation { path: "/foo".into(), message: "bad".into() };
        assert!(err.to_string().contains("/foo"));

        let err = ValidationError::SchemaViolation { path: String::new(), message: "root".into() };
        assert!(err.to_string().contains("root"));
        assert!(!err.to_string().contains(" at "));
    }

    #[test]
    fn validate_metadata_with_order_class() {
        let mut errors = Vec::new();
        let meta = Metadata {
            order_class: Some(OrderClass { order_class: OrderClassKind::Market }),
            ..Metadata::default()
        };
        validate_metadata(&meta, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_metadata_with_replaced_order() {
        let mut errors = Vec::new();
        let uid = format!("0x{}", "ab".repeat(56));
        let meta = Metadata { replaced_order: Some(ReplacedOrder { uid }), ..Metadata::default() };
        validate_metadata(&meta, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_hooks_runs_pre_and_post_lists() {
        // Drives the `if let Some(pre)` and `if let Some(post)` arms of
        // `validate_hooks`. We mix one valid hook with one invalid hook in
        // each list to confirm both lists are walked end-to-end.
        let valid_target = "0x1111111111111111111111111111111111111111".to_owned();
        let invalid_target = "not-an-address".to_owned();
        let mk = |target: &str| CowHook {
            target: target.to_owned(),
            call_data: "0x".to_owned(),
            gas_limit: "100000".to_owned(),
            dapp_id: None,
        };
        let hooks = OrderInteractionHooks {
            version: None,
            pre: Some(vec![mk(&valid_target), mk(&invalid_target)]),
            post: Some(vec![mk(&valid_target), mk(&invalid_target)]),
        };
        let mut errors = Vec::new();
        validate_hooks(&hooks, &mut errors);
        let invalid_target_count = errors
            .iter()
            .filter(|e| matches!(e, ValidationError::InvalidHookTarget { .. }))
            .count();
        // Two invalid hooks (one in `pre`, one in `post`) → two violations.
        assert_eq!(invalid_target_count, 2);
    }
}
