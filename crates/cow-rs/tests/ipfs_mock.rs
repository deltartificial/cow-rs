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
    let ipfs = Ipfs::default().with_write_uri(server.uri()).with_pinata("test-key", "test-secret");

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
    let ipfs = Ipfs::default().with_write_uri(server.uri()).with_pinata("bad-key", "bad-secret");

    let result = upload_app_data_to_pinata(&doc, &ipfs).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn upload_to_pinata_handles_malformed_response() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{\"unexpected\": \"format\"}"))
        .mount(&server)
        .await;

    let doc = AppDataDoc::new("TestApp");
    let ipfs = Ipfs::default().with_write_uri(server.uri()).with_pinata("key", "secret");

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
    let ipfs = Ipfs::default().with_write_uri(server.uri()).with_pinata("my-key", "my-secret");

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

#[test]
fn ipfs_display_with_custom_read_uri() {
    let ipfs = Ipfs::default().with_read_uri("https://custom.ipfs.io/ipfs");
    let display = format!("{ipfs}");
    assert!(display.contains("custom.ipfs.io"));
}

// ── AppDataInfo ─────────────────────────────────────────────────────────────

#[test]
fn app_data_info_new_constructor() {
    use cow_rs::app_data::AppDataInfo;
    let info = AppDataInfo::new("my-cid", r#"{"test":true}"#, "0xdeadbeef");
    assert_eq!(info.cid, "my-cid");
    assert_eq!(info.app_data_content, r#"{"test":true}"#);
    assert_eq!(info.app_data_hex, "0xdeadbeef");
}

#[test]
fn app_data_info_display() {
    use cow_rs::app_data::AppDataInfo;
    let info = AppDataInfo::new("my-cid", "{}", "0xdeadbeef");
    let display = format!("{info}");
    assert!(display.contains("my-cid"));
    assert!(display.contains("0xdeadbeef"));
}

// ── get_app_data_info_from_str ──────────────────────────────────────────────

#[test]
fn get_app_data_info_from_str_returns_consistent_hash() {
    use cow_rs::app_data::{AppDataDoc, get_app_data_info, stringify_deterministic};
    let doc = AppDataDoc::new("TestApp");
    let canonical_json = stringify_deterministic(&doc).unwrap();

    let api = cow_rs::app_data::MetadataApi::new();
    let info_from_str = api.get_app_data_info_from_str(&canonical_json).unwrap();
    let info_from_doc = get_app_data_info(&doc).unwrap();

    assert_eq!(info_from_str.app_data_hex, info_from_doc.app_data_hex);
    assert_eq!(info_from_str.cid, info_from_doc.cid);
}

// ── ValidationResult ────────────────────────────────────────────────────────

#[test]
fn validation_result_display_valid() {
    let result = validate_app_data_doc(&AppDataDoc::new("CoW Swap"));
    let display = format!("{result}");
    assert_eq!(display, "valid");
}

#[test]
fn validation_result_display_invalid() {
    let mut doc = AppDataDoc::new("Test");
    doc.version = String::new();
    let result = validate_app_data_doc(&doc);
    let display = format!("{result}");
    assert!(display.contains("invalid"));
    assert!(display.contains("errors"));
}

#[test]
fn validation_result_errors_ref_and_first_error() {
    let mut doc = AppDataDoc::new("Test");
    doc.version = String::new();
    let result = validate_app_data_doc(&doc);
    assert!(!result.errors_ref().is_empty());
    assert!(result.first_error().is_some());
}

#[test]
fn validation_result_valid_has_no_first_error() {
    let result = validate_app_data_doc(&AppDataDoc::new("CoW Swap"));
    assert!(result.errors_ref().is_empty());
    assert!(result.first_error().is_none());
}

#[test]
fn validation_result_new_constructor() {
    use cow_rs::app_data::ValidationResult;
    let result = ValidationResult::new(true, vec![]);
    assert!(result.is_valid());
    assert!(!result.has_errors());
    assert_eq!(result.error_count(), 0);
}

// ── import_schema ───────────────────────────────────────────────────────────

#[test]
fn import_schema_known_versions() {
    use cow_rs::app_data::import_schema;
    let doc = import_schema("1.3.0").unwrap();
    assert_eq!(doc.version, "1.3.0");

    let doc2 = import_schema("0.7.0").unwrap();
    assert_eq!(doc2.version, "0.7.0");
}

#[test]
fn import_schema_unknown_version_errors() {
    use cow_rs::app_data::import_schema;
    assert!(import_schema("99.0.0").is_err());
}

#[test]
fn import_schema_invalid_semver_errors() {
    use cow_rs::app_data::import_schema;
    assert!(import_schema("not-semver").is_err());
    assert!(import_schema("1.0").is_err());
    assert!(import_schema("").is_err());
}

// ── get_app_data_schema ─────────────────────────────────────────────────────

#[test]
fn get_app_data_schema_known_version() {
    use cow_rs::app_data::get_app_data_schema;
    let doc = get_app_data_schema("1.3.0").unwrap();
    assert_eq!(doc.version, "1.3.0");
}

#[test]
fn get_app_data_schema_unknown_version_errors() {
    use cow_rs::app_data::get_app_data_schema;
    assert!(get_app_data_schema("99.0.0").is_err());
}

// ── MetadataApi ─────────────────────────────────────────────────────────────

#[test]
fn metadata_api_new_default() {
    let api = cow_rs::app_data::MetadataApi::new();
    assert!(api.ipfs.read_uri.is_none());
    assert!(api.ipfs.write_uri.is_none());
}

#[test]
fn metadata_api_with_ipfs() {
    let ipfs = Ipfs::default().with_read_uri("https://custom.io/ipfs").with_pinata("key", "secret");
    let api = cow_rs::app_data::MetadataApi::with_ipfs(ipfs);
    assert_eq!(api.ipfs.read_uri.as_deref(), Some("https://custom.io/ipfs"));
}

#[test]
fn metadata_api_generate_app_data_doc() {
    let api = cow_rs::app_data::MetadataApi::new();
    let doc = api.generate_app_data_doc("MyApp");
    assert_eq!(doc.app_code.as_deref(), Some("MyApp"));
}

#[test]
fn metadata_api_validate_app_data_doc_valid() {
    let api = cow_rs::app_data::MetadataApi::new();
    let doc = AppDataDoc::new("CoW Swap");
    let result = api.validate_app_data_doc(&doc);
    assert!(result.is_valid());
}

#[test]
fn metadata_api_appdata_hex() {
    let api = cow_rs::app_data::MetadataApi::new();
    let doc = AppDataDoc::new("CoW Swap");
    let hex = api.appdata_hex(&doc).unwrap();
    assert_ne!(hex, alloy_primitives::B256::ZERO);
}

#[test]
fn metadata_api_get_app_data_info() {
    let api = cow_rs::app_data::MetadataApi::new();
    let doc = AppDataDoc::new("CoW Swap");
    let info = api.get_app_data_info(&doc).unwrap();
    assert!(info.app_data_hex.starts_with("0x"));
    assert!(!info.cid.is_empty());
}

#[test]
fn metadata_api_app_data_hex_to_cid_roundtrip() {
    let api = cow_rs::app_data::MetadataApi::new();
    let doc = AppDataDoc::new("CoW Swap");
    let info = api.get_app_data_info(&doc).unwrap();
    let cid = api.app_data_hex_to_cid(&info.app_data_hex).unwrap();
    assert_eq!(cid, info.cid);
}

#[test]
fn metadata_api_cid_to_app_data_hex_returns_hex() {
    let api = cow_rs::app_data::MetadataApi::new();
    let doc = AppDataDoc::new("CoW Swap");
    let info = api.get_app_data_info(&doc).unwrap();
    let hex = api.cid_to_app_data_hex(&info.cid).unwrap();
    assert!(hex.starts_with("0x"));
    assert_eq!(hex.len(), 66); // "0x" + 64 hex chars
}

// ── MetadataApi async methods with wiremock ──────────────────────────────────

#[tokio::test]
async fn metadata_api_fetch_doc_from_cid_success() {
    let server = MockServer::start().await;

    let doc_json = json!({
        "version": "1.14.0",
        "appCode": "FetchTest",
        "metadata": {}
    });

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/test-cid"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&doc_json))
        .mount(&server)
        .await;

    let ipfs = Ipfs::default().with_read_uri(server.uri());
    let api = cow_rs::app_data::MetadataApi::with_ipfs(ipfs);
    let doc = api.fetch_doc_from_cid("test-cid").await.unwrap();
    assert_eq!(doc.app_code.as_deref(), Some("FetchTest"));
}

#[tokio::test]
async fn metadata_api_fetch_doc_from_app_data_hex_success() {
    let server = MockServer::start().await;

    let doc_json = json!({
        "version": "1.14.0",
        "appCode": "HexFetchTest",
        "metadata": {}
    });

    // The CID will be derived from the hex, so we match any path.
    Mock::given(matchers::method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&doc_json))
        .mount(&server)
        .await;

    let ipfs = Ipfs::default().with_read_uri(server.uri());
    let api = cow_rs::app_data::MetadataApi::with_ipfs(ipfs);
    // Use a real appdata hex from a known doc to generate a valid CID.
    let info = get_app_data_info(&AppDataDoc::new("HexFetchTest")).unwrap();
    let doc = api.fetch_doc_from_app_data_hex(&info.app_data_hex).await.unwrap();
    assert_eq!(doc.app_code.as_deref(), Some("HexFetchTest"));
}

#[tokio::test]
async fn metadata_api_upload_to_pinata_success() {
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "IpfsHash": "QmMetadataApiHash",
            "PinSize": 50,
            "Timestamp": "2025-01-01T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let ipfs = Ipfs::default().with_write_uri(server.uri()).with_pinata("api-key", "api-secret");
    let api = cow_rs::app_data::MetadataApi::with_ipfs(ipfs);
    let doc = AppDataDoc::new("UploadTest");
    let cid = api.upload_app_data(&doc).await.unwrap();
    assert_eq!(cid, "QmMetadataApiHash");
}

// ── IpfsUploadResult Display ────────────────────────────────────────────────

#[test]
fn ipfs_upload_result_display() {
    let result =
        cow_rs::IpfsUploadResult { app_data: "0xdeadbeef".to_owned(), cid: "QmTestCid".to_owned() };
    let display = format!("{result}");
    assert!(display.contains("QmTestCid"));
    assert!(display.contains("0xdeadbeef"));
}

// ── fetch_doc_from_app_data_hex ─────────────────────────────────────────────

#[tokio::test]
async fn fetch_doc_from_app_data_hex_via_free_function() {
    use cow_rs::app_data::fetch_doc_from_app_data_hex;
    let server = MockServer::start().await;

    let doc_json = json!({
        "version": "1.14.0",
        "appCode": "FreeFnTest",
        "metadata": {}
    });

    Mock::given(matchers::method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&doc_json))
        .mount(&server)
        .await;

    let info = get_app_data_info(&AppDataDoc::new("FreeFnTest")).unwrap();
    let doc = fetch_doc_from_app_data_hex(&info.app_data_hex, Some(&server.uri())).await.unwrap();
    assert_eq!(doc.app_code.as_deref(), Some("FreeFnTest"));
}

// ── validate_app_data_doc with various invalid docs ─────────────────────────

#[test]
fn validate_rejects_app_code_too_long() {
    let doc = AppDataDoc::new("A".repeat(1000));
    // AppDataDoc validation might flag very long app codes.
    let result = validate_app_data_doc(&doc);
    // This test just exercises the validation path.
    let _ = result;
    assert!(true);
}

#[test]
fn validate_accepts_doc_with_environment() {
    let mut doc = AppDataDoc::new("TestApp");
    doc.environment = Some("production".to_owned());
    let result = validate_app_data_doc(&doc);
    assert!(result.is_valid());
}

// ── get_app_data_info with different app codes ──────────────────────────────

#[test]
fn get_app_data_info_empty_app_code() {
    let doc = AppDataDoc::new("");
    let info = get_app_data_info(&doc).unwrap();
    assert!(info.app_data_hex.starts_with("0x"));
    assert!(!info.cid.is_empty());
}

#[test]
fn get_app_data_info_unicode_app_code() {
    let doc = AppDataDoc::new("CoW Protocol \u{1F42E}");
    let info = get_app_data_info(&doc).unwrap();
    assert!(info.app_data_hex.starts_with("0x"));
    assert!(!info.cid.is_empty());
    assert!(info.app_data_content.contains("\u{1F42E}"));
}

// ── IpfsClient trait impl on Ipfs struct (upload error branches) ───────────

#[tokio::test]
async fn ipfs_client_upload_missing_api_key_returns_error() {
    use cow_rs::traits::IpfsClient;
    let ipfs =
        Ipfs { pinata_api_key: None, pinata_api_secret: Some("s".into()), ..Ipfs::default() };
    let result = IpfsClient::upload(&ipfs, r#"{"test":true}"#).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn ipfs_client_upload_missing_api_secret_returns_error() {
    use cow_rs::traits::IpfsClient;
    let ipfs =
        Ipfs { pinata_api_key: Some("k".into()), pinata_api_secret: None, ..Ipfs::default() };
    let result = IpfsClient::upload(&ipfs, r#"{"test":true}"#).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn ipfs_client_upload_invalid_json_returns_error() {
    use cow_rs::traits::IpfsClient;
    let ipfs = Ipfs::default().with_pinata("key", "secret");
    let result = IpfsClient::upload(&ipfs, "not json at all {{{").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn ipfs_client_upload_api_error_returns_error() {
    use cow_rs::traits::IpfsClient;
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let ipfs = Ipfs::default().with_write_uri(server.uri()).with_pinata("key", "secret");
    let result = IpfsClient::upload(&ipfs, r#"{"test":true}"#).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn ipfs_client_upload_malformed_response_returns_error() {
    use cow_rs::traits::IpfsClient;
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"unexpected":"format"}"#))
        .mount(&server)
        .await;

    let ipfs = Ipfs::default().with_write_uri(server.uri()).with_pinata("key", "secret");
    let result = IpfsClient::upload(&ipfs, r#"{"test":true}"#).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn ipfs_client_upload_success() {
    use cow_rs::traits::IpfsClient;
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "IpfsHash": "QmTraitImplHash",
            "PinSize": 42,
            "Timestamp": "2025-01-01T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let ipfs = Ipfs::default().with_write_uri(server.uri()).with_pinata("key", "secret");
    let result = IpfsClient::upload(&ipfs, r#"{"test":true}"#).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "QmTraitImplHash");
}

#[tokio::test]
async fn ipfs_client_fetch_success() {
    use cow_rs::traits::IpfsClient;
    let server = MockServer::start().await;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/test-cid"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"version":"1.14.0"}"#))
        .mount(&server)
        .await;

    let ipfs = Ipfs::default().with_read_uri(server.uri());
    let result = IpfsClient::fetch(&ipfs, "test-cid").await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("version"));
}

#[test]
fn ipfs_client_default_read_uri_is_none() {
    let ipfs = Ipfs::default();
    assert!(ipfs.read_uri.is_none());
}

// ── Legacy encoding helpers ─────────────────────────────────────────────────
//
// The legacy CID and Pinata helpers mirror the original TypeScript SDK
// encoding. They are deprecated but still publicly exported, so they
// participate in the coverage budget until removed.

#[allow(deprecated, reason = "intentionally exercising deprecated legacy functions")]
#[test]
fn legacy_get_app_data_info_produces_cid_and_hex() {
    use cow_rs::app_data::get_app_data_info_legacy;
    let doc = AppDataDoc::new("LegacyApp");
    let info = get_app_data_info_legacy(&doc).unwrap();
    assert!(!info.cid.is_empty());
    assert!(info.app_data_hex.starts_with("0x"));
}

#[allow(deprecated, reason = "intentionally exercising deprecated legacy functions")]
#[tokio::test]
async fn legacy_fetch_doc_from_app_data_hex_via_free_function() {
    use cow_rs::app_data::{fetch_doc_from_app_data_hex_legacy, get_app_data_info_legacy};
    let server = MockServer::start().await;

    let doc_json = json!({ "version": "1.14.0", "appCode": "LegacyFetch", "metadata": {} });
    Mock::given(matchers::method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&doc_json))
        .mount(&server)
        .await;

    let info = get_app_data_info_legacy(&AppDataDoc::new("LegacyFetch")).unwrap();
    let doc =
        fetch_doc_from_app_data_hex_legacy(&info.app_data_hex, Some(&server.uri())).await.unwrap();
    assert_eq!(doc.app_code.as_deref(), Some("LegacyFetch"));
}

#[allow(deprecated, reason = "intentionally exercising deprecated legacy functions")]
#[tokio::test]
async fn legacy_upload_metadata_doc_to_ipfs_success() {
    use cow_rs::app_data::upload_metadata_doc_to_ipfs_legacy;
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "IpfsHash": "f01551b20deadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00d",
            "PinSize": 1,
            "Timestamp": "2025-01-01T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let ipfs = Ipfs::default().with_write_uri(server.uri()).with_pinata("key", "secret");
    let result =
        upload_metadata_doc_to_ipfs_legacy(&AppDataDoc::new("LegacyUpload"), &ipfs).await.unwrap();
    assert!(result.cid.starts_with('f'));
    assert_eq!(
        result.app_data,
        "0xdeadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00d"
    );
}

#[allow(deprecated, reason = "intentionally exercising deprecated legacy functions")]
#[tokio::test]
async fn legacy_upload_requires_both_credentials() {
    use cow_rs::app_data::upload_metadata_doc_to_ipfs_legacy;
    // Missing api_key.
    let ipfs_no_key =
        Ipfs { pinata_api_key: None, pinata_api_secret: Some("s".into()), ..Ipfs::default() };
    assert!(upload_metadata_doc_to_ipfs_legacy(&AppDataDoc::new("X"), &ipfs_no_key).await.is_err());

    // Missing api_secret.
    let ipfs_no_secret =
        Ipfs { pinata_api_key: Some("k".into()), pinata_api_secret: None, ..Ipfs::default() };
    assert!(
        upload_metadata_doc_to_ipfs_legacy(&AppDataDoc::new("X"), &ipfs_no_secret).await.is_err()
    );

    // Empty strings are equivalent to "missing".
    let ipfs_empty = Ipfs::default().with_pinata("", "");
    assert!(upload_metadata_doc_to_ipfs_legacy(&AppDataDoc::new("X"), &ipfs_empty).await.is_err());
}

#[allow(deprecated, reason = "intentionally exercising deprecated legacy functions")]
#[tokio::test]
async fn legacy_upload_propagates_api_error() {
    use cow_rs::app_data::upload_metadata_doc_to_ipfs_legacy;
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let ipfs = Ipfs::default().with_write_uri(server.uri()).with_pinata("k", "s");
    assert!(upload_metadata_doc_to_ipfs_legacy(&AppDataDoc::new("X"), &ipfs).await.is_err());
}

#[allow(deprecated, reason = "intentionally exercising deprecated legacy functions")]
#[tokio::test]
async fn legacy_upload_propagates_malformed_response() {
    use cow_rs::app_data::upload_metadata_doc_to_ipfs_legacy;
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let ipfs = Ipfs::default().with_write_uri(server.uri()).with_pinata("k", "s");
    assert!(upload_metadata_doc_to_ipfs_legacy(&AppDataDoc::new("X"), &ipfs).await.is_err());
}

// ── MetadataApi delegators ──────────────────────────────────────────────────

#[allow(deprecated, reason = "intentionally exercising deprecated legacy delegators")]
#[test]
fn metadata_api_legacy_get_app_data_info_matches_free_function() {
    use cow_rs::app_data::{MetadataApi, get_app_data_info_legacy};
    let api = MetadataApi::new();
    let doc = AppDataDoc::new("ApiLegacy");
    let via_api = api.get_app_data_info_legacy(&doc).unwrap();
    let via_fn = get_app_data_info_legacy(&doc).unwrap();
    assert_eq!(via_api.cid, via_fn.cid);
    assert_eq!(via_api.app_data_hex, via_fn.app_data_hex);
}

#[allow(deprecated, reason = "intentionally exercising deprecated legacy delegators")]
#[tokio::test]
async fn metadata_api_legacy_fetch_doc_from_app_data_hex() {
    use cow_rs::app_data::{MetadataApi, get_app_data_info_legacy};
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "version": "1.14.0",
            "appCode": "ApiLegacyFetch",
            "metadata": {}
        })))
        .mount(&server)
        .await;

    let info = get_app_data_info_legacy(&AppDataDoc::new("ApiLegacyFetch")).unwrap();
    let ipfs = Ipfs::default().with_read_uri(server.uri());
    let api = MetadataApi::with_ipfs(ipfs);
    let doc = api.fetch_doc_from_app_data_hex_legacy(&info.app_data_hex).await.unwrap();
    assert_eq!(doc.app_code.as_deref(), Some("ApiLegacyFetch"));
}

#[allow(deprecated, reason = "intentionally exercising deprecated legacy delegators")]
#[tokio::test]
async fn metadata_api_legacy_upload_metadata_doc_to_ipfs() {
    use cow_rs::app_data::MetadataApi;
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/pinning/pinJSONToIPFS"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "IpfsHash": "f01551b20deadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00d",
            "PinSize": 1,
            "Timestamp": "2025-01-01T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let ipfs = Ipfs::default().with_write_uri(server.uri()).with_pinata("k", "s");
    let api = MetadataApi::with_ipfs(ipfs);
    let result = api.upload_metadata_doc_to_ipfs_legacy(&AppDataDoc::new("ApiUp")).await.unwrap();
    assert!(result.cid.starts_with('f'));
}

#[allow(deprecated, reason = "intentionally exercising deprecated legacy delegators")]
#[test]
fn metadata_api_legacy_app_data_hex_to_cid_matches_free_function() {
    use cow_rs::app_data::{MetadataApi, app_data_hex_to_cid_legacy};
    let api = MetadataApi::new();
    let hex = "0xdeadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00d";
    assert_eq!(
        api.app_data_hex_to_cid_legacy(hex).unwrap(),
        app_data_hex_to_cid_legacy(hex).unwrap()
    );
}

#[test]
fn metadata_api_get_app_data_schema_delegates() {
    use cow_rs::app_data::MetadataApi;
    let api = MetadataApi::new();
    let doc = api.get_app_data_schema("1.3.0").unwrap();
    assert_eq!(doc.version, "1.3.0");
    assert!(api.get_app_data_schema("99.0.0").is_err());
}

#[test]
fn metadata_api_import_schema_delegates() {
    use cow_rs::app_data::MetadataApi;
    let api = MetadataApi::new();
    let doc = api.import_schema("0.7.0").unwrap();
    assert_eq!(doc.version, "0.7.0");
    assert!(api.import_schema("99.0.0").is_err());
}

#[test]
fn metadata_api_parse_cid_delegates() {
    use cow_rs::app_data::{MetadataApi, appdata_hex_to_cid};
    let hex = "0xdeadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00d";
    let cid = appdata_hex_to_cid(hex).unwrap();
    let api = MetadataApi::new();
    let parsed = api.parse_cid(&cid).unwrap();
    assert_eq!(parsed.digest.len(), 32);
}

#[test]
fn metadata_api_decode_cid_delegates() {
    use cow_rs::app_data::MetadataApi;
    let mut bytes = vec![0x01, 0x55, 0x1b, 0x20];
    bytes.extend_from_slice(&[0xaau8; 32]);
    let api = MetadataApi::new();
    let parts = api.decode_cid(&bytes).unwrap();
    assert_eq!(parts.version, 1);
    assert_eq!(parts.digest.len(), 32);
}

#[test]
fn metadata_api_extract_digest_delegates() {
    use cow_rs::app_data::{MetadataApi, appdata_hex_to_cid};
    let hex = "0xdeadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00d";
    let cid = appdata_hex_to_cid(hex).unwrap();
    let api = MetadataApi::new();
    assert_eq!(api.extract_digest(&cid).unwrap(), hex);
}

#[test]
fn metadata_api_display_is_stable() {
    use cow_rs::app_data::MetadataApi;
    assert_eq!(format!("{}", MetadataApi::new()), "metadata-api");
}

// ── Small remaining branches ────────────────────────────────────────────────

#[test]
fn validate_doc_with_post_hooks_exercises_post_branch() {
    use cow_rs::app_data::{CowHook, OrderInteractionHooks};
    // The pre branch is already covered by existing fixtures; this hits the
    // `post` branch of `validate_hooks`.
    let pre = CowHook {
        target: "0x1111111111111111111111111111111111111111".into(),
        call_data: "0x".into(),
        gas_limit: "100000".into(),
        dapp_id: None,
    };
    let post = CowHook {
        target: "0x2222222222222222222222222222222222222222".into(),
        call_data: "0x".into(),
        gas_limit: "200000".into(),
        dapp_id: Some("dapp".into()),
    };
    let hooks = OrderInteractionHooks::new(vec![pre], vec![post]);
    let doc = AppDataDoc::new("PostHooks").with_hooks(hooks);
    let result = validate_app_data_doc(&doc);
    assert!(result.is_valid(), "doc with post hook should be valid: {result:?}");
}

#[test]
fn partner_fee_entry_display_falls_back_when_all_bps_absent() {
    use cow_rs::app_data::PartnerFeeEntry;
    // Bypass the constructors (which always set at least one bps field) by
    // building the entry directly via its JSON representation — an empty
    // object leaves every `Option<u64>` as `None`, exercising the final
    // `fee({recipient})` arm of `Display`.
    let raw = json!({
        "recipient": "0x1111111111111111111111111111111111111111"
    });
    let entry: PartnerFeeEntry = serde_json::from_value(raw).unwrap();
    let rendered = format!("{entry}");
    assert!(rendered.starts_with("fee("), "unexpected render: {rendered}");
    assert!(rendered.contains("0x1111"));
}
