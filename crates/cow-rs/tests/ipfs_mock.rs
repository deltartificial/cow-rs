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
//! Wiremock-based integration tests for IPFS fetch/upload in the `app_data`
//! module.

use cow_rs::app_data::{
    AppDataDoc, Ipfs, fetch_doc_from_cid, get_app_data_info, upload_app_data_to_pinata,
    validate_app_data_doc,
};
use serde_json::json;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

// ── fetch_doc_from_cid ───────────────────────────────────────────────────────

#[tokio::test]
async fn fetch_doc_from_cid_parses_valid_document() {
    let server = MockServer::start().await;

    let doc_json = json!({
        "version": "1.14.0",
        "appCode": "TestApp",
        "metadata": {}
    });

    // The CID is used as a path segment.
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/my-cid-value"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&doc_json))
        .mount(&server)
        .await;

    let doc = fetch_doc_from_cid("my-cid-value", Some(&server.uri())).await.unwrap();
    assert_eq!(doc.version, "1.14.0");
    assert_eq!(doc.app_code.as_deref(), Some("TestApp"));
}

#[tokio::test]
async fn fetch_doc_from_cid_returns_error_on_invalid_json() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/bad-cid"))
        .respond_with(ResponseTemplate::new(200).set_body_string("this is not json"))
        .mount(&server)
        .await;

    let result = fetch_doc_from_cid("bad-cid", Some(&server.uri())).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn fetch_doc_from_cid_returns_error_on_http_failure() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/missing-cid"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    // reqwest::get on 404 still succeeds at HTTP level, but the body
    // won't parse as valid JSON for AppDataDoc.
    let result = fetch_doc_from_cid("missing-cid", Some(&server.uri())).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn fetch_doc_from_cid_with_metadata() {
    let server = MockServer::start().await;

    let doc_json = json!({
        "version": "1.14.0",
        "appCode": "SwapApp",
        "environment": "production",
        "metadata": {
            "referrer": {
                "address": "0x1111111111111111111111111111111111111111",
                "version": "1.0.0"
            }
        }
    });

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/rich-cid"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&doc_json))
        .mount(&server)
        .await;

    let doc = fetch_doc_from_cid("rich-cid", Some(&server.uri())).await.unwrap();
    assert_eq!(doc.version, "1.14.0");
    assert_eq!(doc.app_code.as_deref(), Some("SwapApp"));
    assert_eq!(doc.environment.as_deref(), Some("production"));
    assert!(doc.metadata.referrer.is_some());
}

// ── upload_app_data_to_pinata ────────────────────────────────────────────────

#[tokio::test]
async fn upload_to_pinata_success() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .and(matchers::header_exists("pinata_api_key"))
        .and(matchers::header_exists("pinata_secret_api_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "IpfsHash": "QmTestHash123",
            "PinSize": 42,
            "Timestamp": "2025-01-01T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let doc = AppDataDoc::new("TestApp");
    let ipfs = Ipfs::default()
        .with_write_uri(&server.uri())
        .with_pinata("test-key", "test-secret");

    let cid = upload_app_data_to_pinata(&doc, &ipfs).await.unwrap();
    assert_eq!(cid, "QmTestHash123");
}

#[tokio::test]
async fn upload_to_pinata_returns_error_without_credentials() {
    let doc = AppDataDoc::new("TestApp");
    let ipfs = Ipfs::default();

    let result = upload_app_data_to_pinata(&doc, &ipfs).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn upload_to_pinata_returns_error_without_secret() {
    let doc = AppDataDoc::new("TestApp");
    // pinata_api_key is None, pinata_api_secret is Some — should fail on missing key.
    let ipfs_no_key = Ipfs {
        pinata_api_key: None,
        pinata_api_secret: Some("secret".to_owned()),
        ..Ipfs::default()
    };
    let result = upload_app_data_to_pinata(&doc, &ipfs_no_key).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn upload_to_pinata_handles_api_error() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let doc = AppDataDoc::new("TestApp");
    let ipfs = Ipfs::default()
        .with_write_uri(&server.uri())
        .with_pinata("bad-key", "bad-secret");

    let result = upload_app_data_to_pinata(&doc, &ipfs).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn upload_to_pinata_handles_malformed_response() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("{\"unexpected\": \"format\"}"),
        )
        .mount(&server)
        .await;

    let doc = AppDataDoc::new("TestApp");
    let ipfs = Ipfs::default()
        .with_write_uri(&server.uri())
        .with_pinata("key", "secret");

    let result = upload_app_data_to_pinata(&doc, &ipfs).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn upload_to_pinata_sends_correct_headers() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .and(matchers::header("pinata_api_key", "my-key"))
        .and(matchers::header("pinata_secret_api_key", "my-secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "IpfsHash": "QmCorrectHeaders",
            "PinSize": 10,
            "Timestamp": "2025-06-01T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let doc = AppDataDoc::new("HeaderTest");
    let ipfs = Ipfs::default()
        .with_write_uri(&server.uri())
        .with_pinata("my-key", "my-secret");

    let cid = upload_app_data_to_pinata(&doc, &ipfs).await.unwrap();
    assert_eq!(cid, "QmCorrectHeaders");
}

// ── get_app_data_info (offline, but exercised here) ──────────────────────────

#[test]
fn get_app_data_info_produces_consistent_results() {
    let doc = AppDataDoc::new("TestApp");
    let info1 = get_app_data_info(&doc).unwrap();
    let info2 = get_app_data_info(&doc).unwrap();

    assert_eq!(info1.cid, info2.cid);
    assert_eq!(info1.app_data_hex, info2.app_data_hex);
    assert_eq!(info1.app_data_content, info2.app_data_content);
    assert!(info1.app_data_hex.starts_with("0x"));
    assert!(!info1.cid.is_empty());
}

#[test]
fn get_app_data_info_different_apps_produce_different_hashes() {
    let doc1 = AppDataDoc::new("App1");
    let doc2 = AppDataDoc::new("App2");
    let info1 = get_app_data_info(&doc1).unwrap();
    let info2 = get_app_data_info(&doc2).unwrap();

    assert_ne!(info1.app_data_hex, info2.app_data_hex);
    assert_ne!(info1.cid, info2.cid);
}

// ── validate_app_data_doc ────────────────────────────────────────────────────

#[test]
fn validate_app_data_doc_accepts_valid_document() {
    let doc = AppDataDoc::new("CoW Swap");
    let result = validate_app_data_doc(&doc);
    assert!(result.is_valid());
    assert!(!result.has_errors());
    assert_eq!(result.error_count(), 0);
}

#[test]
fn validate_app_data_doc_rejects_empty_version() {
    let mut doc = AppDataDoc::new("Test");
    doc.version = String::new();
    let result = validate_app_data_doc(&doc);
    assert!(!result.is_valid());
    assert!(result.has_errors());
}

#[test]
fn validate_app_data_doc_rejects_invalid_semver() {
    let mut doc = AppDataDoc::new("Test");
    doc.version = "not-semver".to_owned();
    let result = validate_app_data_doc(&doc);
    assert!(!result.is_valid());
}

// ── Ipfs builder ─────────────────────────────────────────────────────────────

#[test]
fn ipfs_builder_chain() {
    let ipfs = Ipfs::default()
        .with_read_uri("https://read.example.com/ipfs")
        .with_write_uri("https://write.example.com")
        .with_pinata("key", "secret");

    assert_eq!(ipfs.read_uri.as_deref(), Some("https://read.example.com/ipfs"));
    assert_eq!(ipfs.write_uri.as_deref(), Some("https://write.example.com"));
    assert_eq!(ipfs.pinata_api_key.as_deref(), Some("key"));
    assert_eq!(ipfs.pinata_api_secret.as_deref(), Some("secret"));
}

#[test]
fn ipfs_display() {
    let ipfs = Ipfs::default();
    let display = format!("{ipfs}");
    assert!(display.contains("ipfs(read="));
}
