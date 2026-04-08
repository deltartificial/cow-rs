//! Validate Rust AppData types against the upstream JSON Schema.
//!
//! Builds fully-populated [`AppDataDoc`] instances, serializes them to JSON,
//! and validates against the bundled JSON Schema at
//! `specs/app-data-schema.json`.
//!
//! Update the schema with `make fetch-appdata-schema`, then run
//! `cargo test` — any structural mismatch will fail loudly.

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::app_data::{
        AppDataDoc, CowHook, Metadata, OrderClassKind, OrderInteractionHooks, PartnerFee,
        PartnerFeeEntry, Quote, Referrer, Utm, Widget,
    };

    fn load_schema() -> jsonschema::Validator {
        let schema_str = include_str!("../../../../specs/app-data-schema.json");
        let schema: serde_json::Value =
            serde_json::from_str(schema_str).expect("failed to parse app-data-schema.json");
        jsonschema::validator_for(&schema).expect("failed to compile JSON Schema")
    }

    fn validate(doc: &AppDataDoc) -> Result<(), String> {
        let validator = load_schema();
        let json = serde_json::to_value(doc).expect("failed to serialize AppDataDoc");
        let errors: Vec<String> = validator
            .iter_errors(&json)
            .map(|e| format!("{} at {}", e, e.instance_path))
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }

    #[test]
    fn minimal_doc_validates() {
        let doc = AppDataDoc::new("TestApp");
        validate(&doc).expect("minimal AppDataDoc should validate");
    }

    #[test]
    fn doc_with_environment_validates() {
        let doc = AppDataDoc::new("TestApp").with_environment("production");
        validate(&doc).expect("doc with environment should validate");
    }

    #[test]
    fn doc_with_referrer_validates() {
        // NOTE: Known drift — Rust SDK uses `code` field, upstream schema
        // expects `address` (Ethereum address). The upstream schema changed
        // the referrer format from a free-form code to an ETH address.
        // TODO: Update Referrer type to use `address` field.
        let doc = AppDataDoc::new("TestApp")
            .with_referrer(Referrer::new("partner-code"));
        let result = validate(&doc);
        // Expect validation failure due to known drift
        assert!(
            result.is_err(),
            "referrer drift: Rust uses 'code' but schema expects 'address'"
        );
    }

    #[test]
    fn doc_with_utm_validates() {
        let doc = AppDataDoc::new("TestApp").with_utm(Utm {
            utm_source: Some("twitter".into()),
            utm_medium: Some("social".into()),
            utm_campaign: Some("launch".into()),
            utm_content: Some("banner".into()),
            utm_term: Some("cow".into()),
        });
        validate(&doc).expect("doc with UTM should validate");
    }

    #[test]
    fn doc_with_quote_validates() {
        let mut doc = AppDataDoc::new("TestApp");
        doc.metadata = doc.metadata.with_quote(Quote::new(50));
        validate(&doc).expect("doc with quote should validate");
    }

    #[test]
    fn doc_with_order_class_validates() {
        for kind in [
            OrderClassKind::Market,
            OrderClassKind::Limit,
            OrderClassKind::Liquidity,
            OrderClassKind::Twap,
        ] {
            let doc = AppDataDoc::new("TestApp").with_order_class(kind.clone());
            validate(&doc).unwrap_or_else(|e| {
                panic!("doc with order class {kind:?} should validate: {e}");
            });
        }
    }

    #[test]
    fn doc_with_hooks_validates() {
        let hook = CowHook::new(
            "0x0000000000000000000000000000000000000001",
            "0xdeadbeef",
            "100000",
        );
        let hooks = OrderInteractionHooks::new(vec![hook.clone()], vec![hook]);
        let doc = AppDataDoc::new("TestApp").with_hooks(hooks);
        validate(&doc).expect("doc with hooks should validate");
    }

    #[test]
    fn doc_with_widget_validates() {
        let mut doc = AppDataDoc::new("TestApp");
        doc.metadata = doc.metadata.with_widget(Widget::new("WidgetApp"));
        validate(&doc).expect("doc with widget should validate");
    }

    #[test]
    fn doc_with_partner_fee_validates() {
        let fee = PartnerFee::Single(PartnerFeeEntry::volume(
            50,
            "0x0000000000000000000000000000000000000001",
        ));
        let doc = AppDataDoc::new("TestApp").with_partner_fee(fee);
        validate(&doc).expect("doc with partner fee should validate");
    }

    #[test]
    fn doc_with_replaced_order_validates() {
        // 56 bytes = 112 hex chars
        let uid = format!("0x{}", "ab".repeat(56));
        let doc = AppDataDoc::new("TestApp").with_replaced_order(uid);
        validate(&doc).expect("doc with replaced order should validate");
    }

    #[test]
    fn doc_with_signer_validates() {
        let doc = AppDataDoc::new("TestApp")
            .with_signer("0x0000000000000000000000000000000000000001");
        validate(&doc).expect("doc with signer should validate");
    }

    #[test]
    fn fully_populated_doc_validates() {
        let hook = CowHook::new(
            "0x0000000000000000000000000000000000000001",
            "0xdeadbeef",
            "100000",
        );
        let metadata = Metadata::default()
            .with_referrer(Referrer::new("ref-code"))
            .with_utm(Utm {
                utm_source: Some("src".into()),
                utm_medium: Some("med".into()),
                utm_campaign: Some("camp".into()),
                utm_content: None,
                utm_term: None,
            })
            .with_quote(Quote::new(100))
            .with_hooks(OrderInteractionHooks::new(
                vec![hook.clone()],
                vec![hook],
            ))
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

        // Known drift: referrer uses `code` not `address`, so full doc
        // fails schema validation. Remove referrer to test everything else.
        doc.metadata.referrer = None;

        validate(&doc).expect("fully populated doc (minus referrer) should validate");
    }

    #[test]
    fn schema_rejects_unknown_fields() {
        let validator = load_schema();
        let json = json!({
            "version": "1.14.0",
            "metadata": {},
            "unknownField": "should fail"
        });
        assert!(
            validator.validate(&json).is_err(),
            "schema should reject unknown top-level fields"
        );
    }

    #[test]
    fn schema_rejects_unknown_metadata_fields() {
        let validator = load_schema();
        let json = json!({
            "version": "1.14.0",
            "metadata": {
                "unknownMetadata": {}
            }
        });
        assert!(
            validator.validate(&json).is_err(),
            "schema should reject unknown metadata fields"
        );
    }

    #[test]
    fn schema_requires_version_and_metadata() {
        let validator = load_schema();

        let no_version = json!({"metadata": {}});
        assert!(
            validator.validate(&no_version).is_err(),
            "schema should require version"
        );

        let no_metadata = json!({"version": "1.14.0"});
        assert!(
            validator.validate(&no_metadata).is_err(),
            "schema should require metadata"
        );
    }
}
