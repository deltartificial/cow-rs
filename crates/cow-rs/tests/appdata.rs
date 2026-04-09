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
//! Tests for app-data construction, hashing, and serialization.

use cow_rs::{
    AppDataDoc, CowHook, LATEST_APP_DATA_VERSION, Metadata, OrderClassKind, OrderInteractionHooks,
    PartnerFee, PartnerFeeEntry, ReplacedOrder, Utm, ValidationError, Widget, appdata_json,
    build_app_data_doc, build_order_app_data, get_partner_fee_bps, merge_app_data_doc,
    stringify_deterministic, validate_app_data_doc,
};

// ── LATEST_APP_DATA_VERSION ───────────────────────────────────────────────────

#[test]
fn latest_app_data_version_non_empty() {
    assert!(!LATEST_APP_DATA_VERSION.is_empty());
}

// ── build_app_data_doc ────────────────────────────────────────────────────────

#[test]
fn build_app_data_doc_returns_hex_string() {
    let hex = build_app_data_doc("TestApp", Metadata::default()).unwrap();
    assert!(hex.starts_with("0x"), "expected 0x prefix, got: {hex}");
}

#[test]
fn build_app_data_doc_hex_length_is_66() {
    // keccak256 = 32 bytes = 64 hex chars + "0x" prefix = 66
    let hex = build_app_data_doc("TestApp", Metadata::default()).unwrap();
    assert_eq!(hex.len(), 66);
}

#[test]
fn build_app_data_doc_is_deterministic() {
    let meta = Metadata::default();
    let hex1 = build_app_data_doc("TestApp", meta.clone()).unwrap();
    let hex2 = build_app_data_doc("TestApp", meta).unwrap();
    assert_eq!(hex1, hex2);
}

#[test]
fn build_app_data_doc_different_app_names_differ() {
    let hex1 = build_app_data_doc("AppA", Metadata::default()).unwrap();
    let hex2 = build_app_data_doc("AppB", Metadata::default()).unwrap();
    assert_ne!(hex1, hex2);
}

// ── appdata_json ──────────────────────────────────────────────────────────────

#[test]
fn appdata_json_returns_valid_json() {
    let doc = AppDataDoc::new("TestApp");
    let json = appdata_json(&doc).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_object());
}

#[test]
fn appdata_json_contains_app_code() {
    let doc = AppDataDoc::new("MyDApp");
    let json = appdata_json(&doc).unwrap();
    assert!(json.contains("MyDApp"));
}

#[test]
fn appdata_json_contains_version() {
    let doc = AppDataDoc::new("TestApp");
    let json = appdata_json(&doc).unwrap();
    assert!(json.contains(LATEST_APP_DATA_VERSION));
}

// ── build_order_app_data ──────────────────────────────────────────────────────

#[test]
fn build_order_app_data_returns_hex() {
    let result = build_order_app_data("TestApp").unwrap();
    assert!(result.starts_with("0x"));
    assert_eq!(result.len(), 66);
}

// ── validate_app_data_doc ─────────────────────────────────────────────────────

#[test]
fn validate_valid_app_data_doc_passes() {
    let doc = AppDataDoc::new("TestApp");
    let result = validate_app_data_doc(&doc);
    assert!(result.is_valid());
}

// ── stringify_deterministic ───────────────────────────────────────────────────

#[test]
fn stringify_deterministic_simple_object() {
    let doc = AppDataDoc::new("TestApp");
    let s = stringify_deterministic(&doc).unwrap();
    assert!(!s.is_empty());
}

#[test]
fn stringify_deterministic_is_consistent() {
    let doc = AppDataDoc::new("TestApp");
    let s1 = stringify_deterministic(&doc).unwrap();
    let s2 = stringify_deterministic(&doc).unwrap();
    assert_eq!(s1, s2);
}

// ── merge_app_data_doc ────────────────────────────────────────────────────────

#[test]
fn merge_app_data_doc_merges_partner_fee() {
    let fee = PartnerFee::single(PartnerFeeEntry::volume(50, "0xdeadbeef"));
    let base = AppDataDoc::default();
    let other = AppDataDoc::default().with_partner_fee(fee);
    let merged = merge_app_data_doc(base, other);
    assert!(merged.metadata.partner_fee.is_some());
}

#[test]
fn merge_app_data_doc_empty_is_noop() {
    let base = AppDataDoc::default();
    let other = AppDataDoc::default();
    let merged = merge_app_data_doc(base, other);
    assert!(merged.metadata.partner_fee.is_none());
}

// ── AppDataDoc serde roundtrip ────────────────────────────────────────────────

#[test]
fn app_data_doc_default_serializes() {
    let doc = AppDataDoc::default();
    let json = serde_json::to_string(&doc).unwrap();
    let doc2: AppDataDoc = serde_json::from_str(&json).unwrap();
    assert_eq!(doc.version, doc2.version);
}

// ── Metadata serde ────────────────────────────────────────────────────────────

#[test]
fn metadata_default_has_no_hooks() {
    let meta = Metadata::default();
    assert!(meta.hooks.is_none());
}

#[test]
fn metadata_with_hooks() {
    let hook = CowHook {
        target: "0x1234".into(),
        call_data: "0xabcd".into(),
        gas_limit: "50000".into(),
        dapp_id: None,
    };
    let hooks = OrderInteractionHooks { version: None, pre: Some(vec![hook]), post: None };
    let meta = Metadata { hooks: Some(hooks), ..Default::default() };
    assert!(meta.hooks.is_some());
}

// ── get_partner_fee_bps ───────────────────────────────────────────────────────

#[test]
fn get_partner_fee_bps_returns_bps() {
    let fee = PartnerFee::single(PartnerFeeEntry::volume(50, "0xrecipient"));
    let meta = Metadata { partner_fee: Some(fee), ..Default::default() };
    assert_eq!(get_partner_fee_bps(meta.partner_fee.as_ref()), Some(50));
}

#[test]
fn get_partner_fee_bps_none_when_absent() {
    assert_eq!(get_partner_fee_bps(None), None);
}

// ── PartnerFee enum predicates ────────────────────────────────────────────────

#[test]
fn partner_fee_single_is_single() {
    let fee = PartnerFee::single(PartnerFeeEntry::volume(50, "0x1234"));
    assert!(fee.is_single());
    assert!(!fee.is_multiple());
    assert_eq!(fee.count(), 1);
}

#[test]
fn partner_fee_multiple_is_multiple() {
    let fee = PartnerFee::Multiple(vec![
        PartnerFeeEntry::volume(50, "0x1234"),
        PartnerFeeEntry::surplus(30, 100, "0x5678"),
    ]);
    assert!(fee.is_multiple());
    assert!(!fee.is_single());
    assert_eq!(fee.count(), 2);
}

// ── Utm predicates ────────────────────────────────────────────────────────────

#[test]
fn utm_has_source_predicate() {
    let utm_with_source = Utm { utm_source: Some("cowswap".into()), ..Default::default() };
    assert!(utm_with_source.has_source());
    let utm_empty = Utm::default();
    assert!(!utm_empty.has_source());
}

// ── Widget predicates ─────────────────────────────────────────────────────────

#[test]
fn widget_has_environment_predicate() {
    let widget_with_env = Widget::new("TestApp").with_environment("production");
    assert!(widget_with_env.has_environment());
    let widget_plain = Widget::new("TestApp");
    assert!(!widget_plain.has_environment());
}

// ── Extended schema validation ─────────────────────────────────────────────

#[test]
fn validate_empty_app_code_fails() {
    let doc = AppDataDoc::new("");
    let result = validate_app_data_doc(&doc);
    assert!(!result.is_valid());
    assert!(result.has_errors());
    assert!(result.first_error().is_some());
}

#[test]
fn validate_app_code_too_long_fails() {
    let long_name = "A".repeat(51);
    let doc = AppDataDoc::new(&long_name);
    let result = validate_app_data_doc(&doc);
    assert!(!result.is_valid());
    assert!(result.has_errors());
    let first = result.first_error().expect("expected an error");
    assert!(matches!(first, ValidationError::InvalidAppCode(_)));
}

#[test]
fn validate_app_code_exactly_50_chars_passes() {
    let name = "A".repeat(50);
    let doc = AppDataDoc::new(&name);
    let result = validate_app_data_doc(&doc);
    assert!(result.is_valid());
    assert!(!result.has_errors());
    assert_eq!(result.error_count(), 0);
}

#[test]
fn validate_partner_fee_bps_too_high_fails() {
    // 10_001 bps exceeds the 10_000 cap.
    let fee = PartnerFee::single(PartnerFeeEntry::volume(10_001, "0xdeadbeef"));
    let doc = AppDataDoc::new("TestApp").with_partner_fee(fee);
    let result = validate_app_data_doc(&doc);
    assert!(!result.is_valid());
    assert!(
        result.errors_ref().iter().any(|e| matches!(e, ValidationError::PartnerFeeBpsTooHigh(_)))
    );
}

#[test]
fn validate_partner_fee_bps_at_cap_passes() {
    // `validate_app_data_doc` enforces TWO independent caps on partner-fee
    // bps: the hand-written constraint check (≤ 10 000 = 100 %) and the
    // upstream JSON Schema (≤ 100 = 1 %, the actual on-chain protocol
    // limit). The stricter cap wins, so this test exercises the schema
    // boundary at exactly 100 bps. A valid 20-byte recipient address is
    // also required by the schema (`^0x[a-fA-F0-9]{40}$`).
    let fee = PartnerFee::single(PartnerFeeEntry::volume(
        100,
        "0xb6BAd41ae76A11D10f7b0E664C5007b908bC77C9",
    ));
    let doc = AppDataDoc::new("TestApp").with_partner_fee(fee);
    let result = validate_app_data_doc(&doc);
    assert!(result.is_valid(), "100 bps should be accepted by both business and schema rules");
}

#[test]
fn validate_invalid_hook_target_fails() {
    let hook = CowHook::new("not-an-address", "0xabcd", "50000");
    let hooks = OrderInteractionHooks::new(vec![hook], vec![]);
    let doc = AppDataDoc::new("TestApp").with_hooks(hooks);
    let result = validate_app_data_doc(&doc);
    assert!(!result.is_valid());
    assert!(
        result.errors_ref().iter().any(|e| matches!(e, ValidationError::InvalidHookTarget { .. }))
    );
}

#[test]
fn validate_invalid_hook_gas_limit_fails() {
    // Valid address but non-numeric gas limit.
    let hook = CowHook::new("0xaabbccddaabbccddaabbccddaabbccddaabbccdd", "0xabcd", "abc");
    let hooks = OrderInteractionHooks::new(vec![hook], vec![]);
    let doc = AppDataDoc::new("TestApp").with_hooks(hooks);
    let result = validate_app_data_doc(&doc);
    assert!(!result.is_valid());
    assert!(
        result
            .errors_ref()
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidHookGasLimit { .. }))
    );
}

#[test]
fn validate_valid_hook_passes() {
    // Valid checksummed-style address and decimal gas limit.
    let hook = CowHook::new("0xaabbccddaabbccddaabbccddaabbccddaabbccdd", "0xabcd", "50000");
    let hooks = OrderInteractionHooks::new(vec![hook], vec![]);
    let doc = AppDataDoc::new("TestApp").with_hooks(hooks);
    let result = validate_app_data_doc(&doc);
    assert!(result.is_valid());
}

#[test]
fn validate_unknown_order_class_is_impossible_via_typed_api() {
    // OrderClassKind is a closed enum — every variant produced by the typed API
    // is valid.  This test confirms that creating a doc with a known variant
    // never generates an UnknownOrderClass error.
    for kind in [
        OrderClassKind::Market,
        OrderClassKind::Limit,
        OrderClassKind::Liquidity,
        OrderClassKind::Twap,
    ] {
        let doc = AppDataDoc::new("TestApp").with_order_class(kind);
        let result = validate_app_data_doc(&doc);
        assert!(result.is_valid(), "expected valid for OrderClassKind::{kind:?}");
        assert!(
            !result.errors_ref().iter().any(|e| matches!(e, ValidationError::UnknownOrderClass(_))),
            "unexpected UnknownOrderClass for {kind:?}"
        );
    }
}

#[test]
fn validate_invalid_replaced_order_uid_fails() {
    // "0x1234" is far too short (4 hex chars, not 112).
    let doc = AppDataDoc::new("TestApp").with_replaced_order("0x1234");
    let result = validate_app_data_doc(&doc);
    assert!(!result.is_valid());
    assert!(
        result
            .errors_ref()
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidReplacedOrderUid(_)))
    );
}

#[test]
fn validate_valid_replaced_order_uid_passes() {
    // 56-byte order UID = "0x" + 112 hex chars.
    let uid = format!("0x{}", "ab".repeat(56));
    let doc = AppDataDoc::new("TestApp").with_replaced_order(&uid);
    let result = validate_app_data_doc(&doc);
    assert!(result.is_valid(), "56-byte UID should be accepted");
}

#[test]
fn validate_result_error_count() {
    // Empty appCode AND a hook with bad gas-limit gives at least 2 errors.
    let hook = CowHook::new("0xaabbccddaabbccddaabbccddaabbccddaabbccdd", "0xabcd", "not-a-number");
    let hooks = OrderInteractionHooks::new(vec![hook], vec![]);
    let doc = AppDataDoc::new("").with_hooks(hooks);
    let result = validate_app_data_doc(&doc);
    assert!(result.error_count() >= 2);
}

#[test]
fn validate_result_first_error_is_some_on_invalid() {
    let doc = AppDataDoc::new("");
    let result = validate_app_data_doc(&doc);
    assert!(result.first_error().is_some());
}

#[test]
fn validate_result_first_error_is_none_on_valid() {
    let doc = AppDataDoc::new("ValidApp");
    let result = validate_app_data_doc(&doc);
    assert!(result.first_error().is_none());
}

#[test]
fn validate_result_errors_ref_matches_error_count() {
    let doc = AppDataDoc::new("");
    let result = validate_app_data_doc(&doc);
    assert_eq!(result.errors_ref().len(), result.error_count());
}

#[test]
fn validate_replaced_order_struct_api() {
    // Ensure ReplacedOrder can be constructed and used directly.
    let good_uid = format!("0x{}", "cd".repeat(56));
    let ro = ReplacedOrder::new(&good_uid);
    assert_eq!(ro.uid, good_uid);
}
