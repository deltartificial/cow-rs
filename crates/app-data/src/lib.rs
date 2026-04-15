//! `cow-app-data` — Layer 2 `CoW` Protocol order app-data metadata schema and hash generation.
//!
//! App-data is a `bytes32` field in every `CoW` order that encodes a
//! `keccak256` hash of a JSON document describing the order's intent,
//! referral, UTM codes, interaction hooks, and more.
//!
//! # Submodules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`types`] | Rust types matching the JSON schema (v1.14.0) |
//! | [`hash`] | Deterministic JSON serialisation and `keccak256` hashing |
//! | [`cid`] | Bidirectional `appDataHex` ↔ IPFS `CIDv1` conversion |
//! | [`ipfs`] | IPFS fetch/upload helpers and the [`MetadataApi`] facade |
//! | [`schema`] | Runtime JSON Schema validation against the bundled upstream spec |
//! | `validation` | Business-rule constraint checks (`appCode` length, `bps` caps, …) |
//!
//! # Quick start
//!
//! ```rust
//! use cow_app_data::{AppDataDoc, MetadataApi};
//!
//! let api = MetadataApi::new();
//! let doc = api.generate_app_data_doc("MyApp");
//! let info = api.get_app_data_info(&doc).unwrap();
//! println!("appData hex : {}", info.app_data_hex);
//! println!("CID         : {}", info.cid);
//! ```
//!
//! # Building app-data for an order
//!
//! ```rust
//! use cow_app_data::{Metadata, Quote, build_app_data_doc, build_order_app_data};
//!
//! // Simple: just an app code
//! let hex = build_order_app_data("MyDApp").unwrap();
//!
//! // With metadata: slippage, partner fees, hooks, …
//! let meta = Metadata::default().with_quote(Quote::new(50));
//! let hex = build_app_data_doc("MyDApp", meta).unwrap();
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod cid;
pub mod hash;
pub mod ipfs;
#[cfg(feature = "schema-validation")]
pub mod schema;
pub mod types;
pub(crate) mod validation;

#[allow(
    deprecated,
    reason = "re-exporting deprecated legacy function for backwards compatibility"
)]
pub use cid::app_data_hex_to_cid_legacy;
pub use cid::{
    CidComponents, appdata_hex_to_cid, assert_cid, cid_to_appdata_hex, decode_cid, extract_digest,
    parse_cid,
};
pub use hash::{
    appdata_hex, appdata_json, build_app_data_doc, build_app_data_doc_full, build_order_app_data,
    merge_app_data_doc, stringify_deterministic,
};
pub use ipfs::{
    AppDataInfo, DEFAULT_IPFS_READ_URI, DEFAULT_IPFS_WRITE_URI, Ipfs, IpfsUploadResult,
    MetadataApi, ValidationResult, fetch_doc_from_app_data_hex, fetch_doc_from_cid,
    get_app_data_info, get_app_data_schema, import_schema, upload_app_data_to_pinata,
    upload_app_data_to_pinata as pin_json_in_pinata_ipfs, validate_app_data_doc,
};
#[allow(
    deprecated,
    reason = "re-exporting deprecated legacy functions for backwards compatibility"
)]
pub use ipfs::{
    fetch_doc_from_app_data_hex_legacy, get_app_data_info_legacy,
    upload_metadata_doc_to_ipfs_legacy,
};
#[cfg(feature = "schema-validation")]
pub use schema::{
    APP_DATA_SCHEMA, LATEST_VERSION as LATEST_APP_DATA_SCHEMA_VERSION, SchemaError,
    SchemaViolation, supported_versions as supported_app_data_schema_versions,
    validate as validate_schema, validate_json as validate_schema_json,
    validate_json_with as validate_schema_json_with, validate_with as validate_schema_with,
};
pub use types::{
    AppDataDoc, CowHook, LATEST_APP_DATA_VERSION, LATEST_HOOKS_METADATA_VERSION,
    LATEST_ORDER_CLASS_METADATA_VERSION, LATEST_PARTNER_FEE_METADATA_VERSION,
    LATEST_QUOTE_METADATA_VERSION, LATEST_REFERRER_METADATA_VERSION,
    LATEST_REPLACED_ORDER_METADATA_VERSION, LATEST_SIGNER_METADATA_VERSION,
    LATEST_USER_CONSENTS_METADATA_VERSION, LATEST_UTM_METADATA_VERSION,
    LATEST_WIDGET_METADATA_VERSION, LATEST_WRAPPERS_METADATA_VERSION, Metadata, OrderClass,
    OrderClassKind, OrderInteractionHooks, PartnerFee, PartnerFeeEntry, Quote, Referrer,
    ReplacedOrder, Utm, Widget, get_partner_fee_bps,
};
pub use validation::ValidationError;
