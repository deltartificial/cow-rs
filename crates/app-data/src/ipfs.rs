//! `MetadataApi` facade and IPFS fetch/upload helpers for `CoW` Protocol
//! app-data.
//!
//! This module provides two layers of API:
//!
//! 1. **Free functions** — stateless helpers like [`get_app_data_info`], [`validate_app_data_doc`],
//!    [`fetch_doc_from_cid`], and [`upload_app_data_to_pinata`] that operate on explicit
//!    parameters.
//!
//! 2. **[`MetadataApi`]** — an ergonomic facade that bundles an [`Ipfs`] configuration and
//!    delegates to the free functions, mirroring the `MetadataApi` class from the `TypeScript` SDK.
//!
//! Most users will interact through [`MetadataApi`]:
//!
//! ```rust
//! use cow_rs::app_data::{AppDataDoc, MetadataApi};
//!
//! let api = MetadataApi::new();
//! let doc = api.generate_app_data_doc("MyApp");
//! let info = api.get_app_data_info(&doc).unwrap();
//! println!("appData hex : {}", info.app_data_hex);
//! println!("CID         : {}", info.cid);
//! ```

use std::fmt;

use alloy_primitives::B256;
use serde::Deserialize;
use serde_json::json;

use cow_sdk_error::CowError;

use super::{
    cid::{appdata_hex_to_cid, cid_to_appdata_hex, extract_digest},
    hash::{appdata_hex, stringify_deterministic},
    types::{AppDataDoc, Metadata},
    validation::{ValidationError, validate_constraints},
};

/// Default IPFS gateway used when none is provided.
pub const DEFAULT_IPFS_READ_URI: &str = "https://cloudflare-ipfs.com/ipfs";

/// Default IPFS write URI (Pinata).
pub const DEFAULT_IPFS_WRITE_URI: &str = "https://api.pinata.cloud";

// ── Extra types ───────────────────────────────────────────────────────────────

/// Full app-data information derived from an [`AppDataDoc`].
///
/// Bundles the three representations of an order's app-data that are
/// needed at different stages of the order lifecycle:
///
/// - **`cid`** — used to store/retrieve the document on IPFS.
/// - **`app_data_content`** — the canonical JSON whose `keccak256` equals `app_data_hex`. Pin this
///   string on IPFS so solvers can read the metadata.
/// - **`app_data_hex`** — the 32-byte value placed in the on-chain order struct.
///
/// Obtain an instance via [`get_app_data_info`] or [`MetadataApi::get_app_data_info`].
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{AppDataDoc, get_app_data_info};
///
/// let doc = AppDataDoc::new("MyDApp");
/// let info = get_app_data_info(&doc).unwrap();
/// assert!(info.app_data_hex.starts_with("0x"));
/// assert!(info.cid.starts_with('f'));
/// assert!(info.app_data_content.contains("MyDApp"));
/// ```
#[derive(Debug, Clone)]
pub struct AppDataInfo {
    /// IPFS `CIDv1` string for the order's app-data.
    pub cid: String,
    /// Canonical JSON string whose `keccak256` equals [`Self::app_data_hex`].
    pub app_data_content: String,
    /// `0x`-prefixed 32-byte hex used as `appData` in the on-chain order struct.
    pub app_data_hex: String,
}

impl AppDataInfo {
    /// Construct an [`AppDataInfo`] from its three constituent fields.
    ///
    /// Prefer [`get_app_data_info`] to derive all three values from an
    /// [`AppDataDoc`] automatically. Use this constructor only when you
    /// already have the CID, JSON content, and hex hash from an external
    /// source.
    ///
    /// # Parameters
    ///
    /// * `cid` — the IPFS `CIDv1` base16 string.
    /// * `app_data_content` — the canonical JSON string.
    /// * `app_data_hex` — the `0x`-prefixed 32-byte `keccak256` hex.
    ///
    /// # Returns
    ///
    /// A new [`AppDataInfo`] instance.
    #[must_use]
    pub fn new(
        cid: impl Into<String>,
        app_data_content: impl Into<String>,
        app_data_hex: impl Into<String>,
    ) -> Self {
        Self {
            cid: cid.into(),
            app_data_content: app_data_content.into(),
            app_data_hex: app_data_hex.into(),
        }
    }
}

impl fmt::Display for AppDataInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "app-data-info({}, {})", self.cid, self.app_data_hex)
    }
}

/// IPFS connection parameters for upload/fetch operations.
///
/// Configure read/write gateway URIs and optional Pinata API credentials.
/// Pass an instance to [`MetadataApi::with_ipfs`] or directly to
/// [`upload_app_data_to_pinata`].
///
/// # Example
///
/// ```
/// use cow_rs::app_data::Ipfs;
///
/// let ipfs = Ipfs::default()
///     .with_read_uri("https://my-gateway.io/ipfs")
///     .with_pinata("my-api-key", "my-api-secret");
/// assert_eq!(ipfs.read_uri.as_deref(), Some("https://my-gateway.io/ipfs"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Ipfs {
    /// IPFS read gateway URI (defaults to [`DEFAULT_IPFS_READ_URI`]).
    pub read_uri: Option<String>,
    /// IPFS write gateway URI (defaults to [`DEFAULT_IPFS_WRITE_URI`]).
    pub write_uri: Option<String>,
    /// Pinata API key for authenticated uploads.
    pub pinata_api_key: Option<String>,
    /// Pinata API secret for authenticated uploads.
    pub pinata_api_secret: Option<String>,
}

/// Result of validating an [`AppDataDoc`] against its schema.
///
/// Contains both human-readable error strings (for logging / display) and
/// typed [`ValidationError`] values (for programmatic inspection). An empty
/// [`typed_errors`](Self::typed_errors) list means the document is valid.
///
/// Obtain an instance via [`validate_app_data_doc`] or
/// [`MetadataApi::validate_app_data_doc`].
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{AppDataDoc, validate_app_data_doc};
///
/// let result = validate_app_data_doc(&AppDataDoc::new("OK"));
/// assert!(result.is_valid());
/// assert!(!result.has_errors());
/// assert_eq!(result.error_count(), 0);
/// ```
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the document is valid (no errors found).
    pub success: bool,
    /// Human-readable validation errors (empty when `success` is true).
    ///
    /// Kept as `Vec<String>` for backwards compatibility with callers that
    /// only inspect the string messages; typed errors are in [`Self::typed_errors`].
    pub errors: Vec<String>,
    /// Structured, typed constraint violations (empty when `success` is true).
    pub typed_errors: Vec<ValidationError>,
}

impl ValidationResult {
    /// Construct a [`ValidationResult`] from a success flag and
    /// human-readable error list.
    ///
    /// The `typed_errors` field is initialised to an empty `Vec`. Callers
    /// typically use [`validate_app_data_doc`] instead, which populates
    /// both the string errors and typed errors automatically.
    ///
    /// # Parameters
    ///
    /// * `success` — whether the document is valid.
    /// * `errors` — human-readable error messages.
    ///
    /// # Returns
    ///
    /// A new [`ValidationResult`] with an empty `typed_errors` list.
    #[must_use]
    pub const fn new(success: bool, errors: Vec<String>) -> Self {
        Self { success, errors, typed_errors: Vec::new() }
    }

    /// Returns `true` when validation succeeded (no errors).
    ///
    /// Equivalent to checking `typed_errors.is_empty()`, but stored as a
    /// precomputed flag for convenience.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.success
    }

    /// Returns `true` when at least one constraint violation was found.
    ///
    /// The inverse of [`is_valid`](Self::is_valid).
    #[must_use]
    pub const fn has_errors(&self) -> bool {
        !self.typed_errors.is_empty()
    }

    /// Returns the number of typed constraint violations.
    ///
    /// # Returns
    ///
    /// `0` when the document is valid, `> 0` otherwise.
    #[must_use]
    pub const fn error_count(&self) -> usize {
        self.typed_errors.len()
    }

    /// Returns a slice of all typed constraint violations.
    ///
    /// Use this for programmatic inspection of validation errors. Each
    /// [`ValidationError`] variant carries enough context to build a
    /// diagnostic message.
    ///
    /// # Returns
    ///
    /// An empty slice when the document is valid.
    #[must_use]
    pub fn errors_ref(&self) -> &[ValidationError] {
        &self.typed_errors
    }

    /// Returns the first typed constraint violation, if any.
    ///
    /// Useful for quick "fail on first error" workflows.
    ///
    /// # Returns
    ///
    /// `None` when the document is valid, `Some(&error)` otherwise.
    #[must_use]
    pub fn first_error(&self) -> Option<&ValidationError> {
        self.typed_errors.first()
    }
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.success {
            f.write_str("valid")
        } else {
            write!(f, "invalid({} errors)", self.typed_errors.len())
        }
    }
}

impl Ipfs {
    /// Set the IPFS read gateway URI.
    ///
    /// Overrides the default [`DEFAULT_IPFS_READ_URI`] (`cloudflare-ipfs.com`)
    /// for all fetch operations.
    ///
    /// # Parameters
    ///
    /// * `uri` — the base URL of the IPFS read gateway (e.g. `"https://my-gateway.io/ipfs"`).
    ///
    /// # Returns
    ///
    /// `self` with `read_uri` set.
    #[must_use]
    pub fn with_read_uri(mut self, uri: impl Into<String>) -> Self {
        self.read_uri = Some(uri.into());
        self
    }

    /// Set the IPFS write gateway URI.
    ///
    /// Overrides the default [`DEFAULT_IPFS_WRITE_URI`] (`api.pinata.cloud`)
    /// for all upload operations.
    ///
    /// # Parameters
    ///
    /// * `uri` — the base URL of the IPFS write gateway.
    ///
    /// # Returns
    ///
    /// `self` with `write_uri` set.
    #[must_use]
    pub fn with_write_uri(mut self, uri: impl Into<String>) -> Self {
        self.write_uri = Some(uri.into());
        self
    }

    /// Set Pinata API credentials for authenticated uploads.
    ///
    /// Both the API key and secret are required for
    /// [`upload_app_data_to_pinata`] to succeed. Obtain them from the Pinata
    /// dashboard.
    ///
    /// # Parameters
    ///
    /// * `api_key` — your Pinata API key.
    /// * `api_secret` — your Pinata API secret.
    ///
    /// # Returns
    ///
    /// `self` with both `pinata_api_key` and `pinata_api_secret` set.
    #[must_use]
    pub fn with_pinata(
        mut self,
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
    ) -> Self {
        self.pinata_api_key = Some(api_key.into());
        self.pinata_api_secret = Some(api_secret.into());
        self
    }
}

impl fmt::Display for Ipfs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let uri = self.read_uri.as_deref().map_or("default", |s| s);
        write!(f, "ipfs(read={uri})")
    }
}

// ── AppDataInfo helpers ───────────────────────────────────────────────────────

/// Derive the full [`AppDataInfo`] from a document.
///
/// Performs three steps in sequence:
/// 1. Serialise `doc` to deterministic JSON via [`stringify_deterministic`].
/// 2. Compute `keccak256(json_bytes)` to get the `appData` hex.
/// 3. Convert the hex to a `CIDv1` string via [`appdata_hex_to_cid`].
///
/// Mirrors `getAppDataInfo` from the `@cowprotocol/app-data` `TypeScript`
/// package.
///
/// # Parameters
///
/// * `doc` — the [`AppDataDoc`] to derive info from.
///
/// # Returns
///
/// An [`AppDataInfo`] containing the canonical JSON, `0x`-prefixed hex
/// hash, and base16 `CIDv1` string.
///
/// # Errors
///
/// Returns [`CowError::AppData`] on serialisation or CID conversion failure.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{AppDataDoc, get_app_data_info};
///
/// let doc = AppDataDoc::new("CoW Swap");
/// let info = get_app_data_info(&doc)?;
/// assert!(!info.cid.is_empty());
/// assert!(info.app_data_hex.starts_with("0x"));
/// assert!(!info.app_data_content.is_empty());
/// # Ok::<(), cow_rs::error::CowError>(())
/// ```
pub fn get_app_data_info(doc: &AppDataDoc) -> Result<AppDataInfo, CowError> {
    let app_data_content = stringify_deterministic(doc)?;
    let hash: B256 = alloy_primitives::keccak256(app_data_content.as_bytes());
    let app_data_hex = format!("0x{}", alloy_primitives::hex::encode(hash.as_slice()));
    let cid = appdata_hex_to_cid(&app_data_hex)?;
    Ok(AppDataInfo { cid, app_data_content, app_data_hex })
}

/// Derive [`AppDataInfo`] from a pre-serialised app-data JSON string.
///
/// Unlike [`get_app_data_info`], this function does **not** re-serialise
/// the document — it treats `json` as the canonical pre-image and hashes
/// it directly with `keccak256`. Use this when you have a string that was
/// already produced by [`stringify_deterministic`] and must not be
/// re-encoded, or when you received a JSON string from an external source
/// and want to compute its on-chain hash.
///
/// # Parameters
///
/// * `json` — the canonical JSON string to hash.
///
/// # Returns
///
/// An [`AppDataInfo`] where `app_data_content` is `json` verbatim.
///
/// # Errors
///
/// Returns [`CowError::AppData`] on `CID` conversion failure.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{AppDataDoc, MetadataApi, stringify_deterministic};
///
/// let doc = AppDataDoc::new("CoW Swap");
/// let canonical_json = stringify_deterministic(&doc)?;
/// let api = MetadataApi::new();
/// let info = api.get_app_data_info_from_str(&canonical_json)?;
/// assert!(info.app_data_hex.starts_with("0x"));
/// assert_eq!(info.app_data_content, canonical_json);
/// # Ok::<(), cow_rs::error::CowError>(())
/// ```
pub fn get_app_data_info_from_str(json: &str) -> Result<AppDataInfo, CowError> {
    let hash: alloy_primitives::B256 = alloy_primitives::keccak256(json.as_bytes());
    let app_data_hex = format!("0x{}", alloy_primitives::hex::encode(hash.as_slice()));
    let cid = appdata_hex_to_cid(&app_data_hex)?;
    Ok(AppDataInfo { cid, app_data_content: json.to_owned(), app_data_hex })
}

/// Validate an [`AppDataDoc`] against all `CoW` Protocol app-data rules.
///
/// Runs up to three independent checks and merges their results into a
/// single [`ValidationResult`]:
///
/// 1. **Version check** — `version` must be non-empty and parse as semver `x.y.z`.
/// 2. **Business-rule constraints** — `appCode` length, hook address format, `partnerFee`
///    basis-point caps, and similar field-level rules enforced by the private `validation` helper
///    module.
/// 3. **JSON Schema validation** — *(only when the `schema-validation` feature is enabled; on by
///    default for native targets, off by default on wasm)* the serialised document is checked
///    against the bundled upstream schema via the `schema` module, catching structural drift that
///    the hand-written business rules do not cover (missing required fields, unknown properties,
///    regex violations, `anyOf` variants, …).
///
/// Returns a [`ValidationResult`] that lists every violation found. An
/// empty [`ValidationResult::typed_errors`] list means the document is
/// fully valid.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{AppDataDoc, validate_app_data_doc};
///
/// let doc = AppDataDoc::new("CoW Swap");
/// let result = validate_app_data_doc(&doc);
/// assert!(result.is_valid());
/// assert!(!result.has_errors());
/// ```
#[must_use]
pub fn validate_app_data_doc(doc: &AppDataDoc) -> ValidationResult {
    let mut typed_errors: Vec<ValidationError> = Vec::new();

    // ── Version check ──────────────────────────────────────────────────────
    if doc.version.is_empty() {
        typed_errors.push(ValidationError::InvalidVersion("version must not be empty".to_owned()));
    } else {
        // Expect semver format: \d+\.\d+\.\d+
        let parts: Vec<&str> = doc.version.split('.').collect();
        if parts.len() != 3 || parts.iter().any(|p| p.parse::<u32>().is_err()) {
            typed_errors.push(ValidationError::InvalidVersion(format!(
                "version '{}' is not valid semver",
                doc.version
            )));
        }
    }

    // ── Business-rule checks (appCode, hooks, partnerFee, orderClass, …) ──
    validate_constraints(doc, &mut typed_errors);

    // ── Structural JSON Schema check ───────────────────────────────────────
    //
    // Dispatches on `doc.version`: [`super::schema::validate`] selects the
    // bundled schema matching the document's declared version and returns
    // either a list of violations or an `UnsupportedVersion` error. Both
    // outcomes flow into the combined `ValidationResult`.
    #[cfg(feature = "schema-validation")]
    match super::schema::validate(doc) {
        Ok(()) => {}
        Err(super::schema::SchemaError::Violations(violations)) => {
            for v in violations {
                typed_errors
                    .push(ValidationError::SchemaViolation { path: v.path, message: v.message });
            }
        }
        Err(super::schema::SchemaError::UnsupportedVersion { requested, supported }) => {
            typed_errors.push(ValidationError::SchemaViolation {
                path: "/version".to_owned(),
                message: format!(
                    "AppData version `{requested}` is not backed by a bundled schema in \
                     this build (supported: {})",
                    supported.join(", ")
                ),
            });
        }
    }

    // Render string representations once, in sync with the typed list.
    let string_errors: Vec<String> = typed_errors.iter().map(|e| e.to_string()).collect();

    let success = typed_errors.is_empty();
    ValidationResult { success, errors: string_errors, typed_errors }
}

// ── IPFS fetch ────────────────────────────────────────────────────────────────

/// Fetch an [`AppDataDoc`] from IPFS by its `CIDv1`.
///
/// Sends a GET request to `{ipfs_uri}/{cid}` and deserialises the JSON
/// response into an [`AppDataDoc`].
///
/// # Parameters
///
/// * `cid` — the `CIDv1` base16 string identifying the document.
/// * `ipfs_uri` — optional gateway base URL. Defaults to [`DEFAULT_IPFS_READ_URI`] when `None`.
///
/// # Returns
///
/// The deserialised [`AppDataDoc`].
///
/// # Errors
///
/// Returns [`CowError::Http`] or [`CowError::Parse`] on failure.
///
/// # Example
///
/// ```no_run
/// use cow_rs::app_data::fetch_doc_from_cid;
///
/// # async fn example() -> Result<(), cow_rs::error::CowError> {
/// let cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
/// let doc = fetch_doc_from_cid(cid, None).await?;
/// assert!(!doc.version.is_empty());
/// # Ok(())
/// # }
/// ```
pub async fn fetch_doc_from_cid(cid: &str, ipfs_uri: Option<&str>) -> Result<AppDataDoc, CowError> {
    let base = ipfs_uri.map_or(DEFAULT_IPFS_READ_URI, |s| s);
    let url = format!("{base}/{cid}");
    let text = reqwest::get(&url).await?.text().await?;
    serde_json::from_str(&text)
        .map_err(|e| CowError::Parse { field: "app_data_doc", reason: e.to_string() })
}

/// Fetch an [`AppDataDoc`] from IPFS using a hex `appData` value.
///
/// Converts `app_data_hex` to a `CIDv1` via
/// [`appdata_hex_to_cid`], then delegates
/// to [`fetch_doc_from_cid`] for the actual HTTP fetch.
///
/// # Parameters
///
/// * `app_data_hex` — the `0x`-prefixed 32-byte hex value from the on-chain order struct.
/// * `ipfs_uri` — optional gateway base URL (defaults to [`DEFAULT_IPFS_READ_URI`]).
///
/// # Returns
///
/// The deserialised [`AppDataDoc`].
///
/// # Errors
///
/// Returns [`CowError::AppData`], [`CowError::Http`], or [`CowError::Parse`].
///
/// # Example
///
/// ```no_run
/// use cow_rs::app_data::fetch_doc_from_app_data_hex;
///
/// # async fn example() -> Result<(), cow_rs::error::CowError> {
/// let hex = "0x0000000000000000000000000000000000000000000000000000000000000000";
/// let doc = fetch_doc_from_app_data_hex(hex, None).await?;
/// assert!(!doc.version.is_empty());
/// # Ok(())
/// # }
/// ```
pub async fn fetch_doc_from_app_data_hex(
    app_data_hex: &str,
    ipfs_uri: Option<&str>,
) -> Result<AppDataDoc, CowError> {
    let cid = appdata_hex_to_cid(app_data_hex)?;
    fetch_doc_from_cid(&cid, ipfs_uri).await
}

// ── IPFS upload ───────────────────────────────────────────────────────────────

/// Response from the Pinata `pinJSONToIPFS` endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PinataResponse {
    ipfs_hash: String,
}

/// Upload an [`AppDataDoc`] to IPFS via the Pinata pinning service.
///
/// The document is first serialised to deterministic JSON and hashed via
/// [`get_app_data_info`], then uploaded to
/// `{write_uri}/pinning/pinJSONToIPFS` using the provided Pinata API
/// credentials. The canonical JSON is pinned as `pinataContent` and the
/// `keccak256` hex is stored as `pinataMetadata.name`.
///
/// # Parameters
///
/// * `doc` — the [`AppDataDoc`] to upload.
/// * `ipfs` — the [`Ipfs`] configuration containing Pinata credentials and optional gateway URIs.
///
/// # Returns
///
/// The IPFS CID hash string returned by Pinata on success.
///
/// # Errors
///
/// Returns [`CowError::AppData`] when no Pinata credentials are configured,
/// [`CowError::Http`] on transport failure, or [`CowError::Api`] when Pinata
/// returns a non-2xx status code.
///
/// # Example
///
/// ```no_run
/// use cow_rs::app_data::{AppDataDoc, Ipfs, upload_app_data_to_pinata};
///
/// # async fn example() -> Result<(), cow_rs::error::CowError> {
/// let doc = AppDataDoc::new("CoW Swap");
/// let ipfs = Ipfs::default().with_pinata("my-api-key", "my-api-secret");
/// let cid = upload_app_data_to_pinata(&doc, &ipfs).await?;
/// assert!(!cid.is_empty());
/// # Ok(())
/// # }
/// ```
pub async fn upload_app_data_to_pinata(doc: &AppDataDoc, ipfs: &Ipfs) -> Result<String, CowError> {
    let api_key = ipfs
        .pinata_api_key
        .as_deref()
        .ok_or_else(|| CowError::AppData("pinata_api_key is required for IPFS upload".into()))?;
    let api_secret = ipfs
        .pinata_api_secret
        .as_deref()
        .ok_or_else(|| CowError::AppData("pinata_api_secret is required for IPFS upload".into()))?;

    let info = get_app_data_info(doc)?;
    let write_uri = ipfs.write_uri.as_deref().map_or(DEFAULT_IPFS_WRITE_URI, |s| s);
    let url = format!("{write_uri}/pinning/pinJSONToIPFS");

    let content: serde_json::Value = serde_json::from_str(&info.app_data_content)
        .map_err(|e| CowError::AppData(e.to_string()))?;

    let body = json!({
        "pinataContent": content,
        "pinataOptions": { "cidVersion": 1 },
        "pinataMetadata": { "name": info.app_data_hex }
    });

    let resp = reqwest::Client::new()
        .post(&url)
        .header("pinata_api_key", api_key)
        .header("pinata_secret_api_key", api_secret)
        .json(&body)
        .send()
        .await?;

    let status = resp.status().as_u16();
    let text = resp.text().await?;
    if status != 200 {
        return Err(CowError::Api { status, body: text });
    }

    let pinata: PinataResponse =
        serde_json::from_str(&text).map_err(|e| CowError::AppData(e.to_string()))?;
    Ok(pinata.ipfs_hash)
}

// ── Legacy helpers ───────────────────────────────────────────────────────────

/// Internal helper for deriving [`AppDataInfo`] from either a document or a
/// pre-serialised JSON string, using a pluggable CID derivation function.
///
/// This is the Rust equivalent of `_appDataToCidAux` in the `TypeScript` SDK.
#[allow(
    clippy::type_complexity,
    reason = "mirrors the TypeScript SDK's pluggable CID derivation pattern"
)]
fn app_data_to_cid_aux(
    full_app_data: &str,
    derive_cid: fn(&str) -> Result<String, CowError>,
) -> Result<AppDataInfo, CowError> {
    let cid = derive_cid(full_app_data)?;
    let app_data_hex = extract_digest(&cid)?;

    if app_data_hex.is_empty() {
        return Err(CowError::AppData(format!(
            "Could not extract appDataHex from calculated cid {cid}"
        )));
    }

    Ok(AppDataInfo { cid, app_data_content: full_app_data.to_owned(), app_data_hex })
}

/// Internal CID derivation using the legacy `sha2-256` / `dag-pb` method.
///
/// **Note**: The original `TypeScript` SDK used `ipfs-only-hash` with `CIDv0`. This Rust
/// implementation uses `keccak256` as the hash but wraps it in the legacy CID
/// prefix for structural compatibility. True legacy CID reproduction would
/// require an `sha2-256` IPFS chunker which is not included.
///
/// This is the Rust equivalent of `_appDataToCidLegacy` in the `TypeScript` SDK.
#[allow(deprecated, reason = "wraps the deprecated legacy CID function intentionally")]
fn app_data_to_cid_legacy(full_app_data_json: &str) -> Result<String, CowError> {
    let hash = alloy_primitives::keccak256(full_app_data_json.as_bytes());
    let app_data_hex = format!("0x{}", alloy_primitives::hex::encode(hash.as_slice()));
    super::cid::app_data_hex_to_cid_legacy(&app_data_hex)
}

/// Derive [`AppDataInfo`] using the legacy method.
///
/// Uses `JSON.stringify`-equivalent serialisation (plain `serde_json::to_string`) and
/// legacy CID encoding for backwards compatibility.
///
/// # Errors
///
/// Returns [`CowError::AppData`] on serialisation or CID failure.
#[deprecated(
    note = "Use get_app_data_info instead — legacy CID encoding is no longer used by CoW Protocol"
)]
pub fn get_app_data_info_legacy(doc: &AppDataDoc) -> Result<AppDataInfo, CowError> {
    // Legacy mode uses plain JSON.stringify (non-deterministic key order)
    let full_app_data = serde_json::to_string(doc).map_err(|e| CowError::AppData(e.to_string()))?;
    app_data_to_cid_aux(&full_app_data, app_data_to_cid_legacy)
}

/// Internal helper that fetches a document from IPFS via a pluggable
/// hex-to-CID conversion function.
///
/// This is the Rust equivalent of `_fetchDocFromCidAux` in the `TypeScript` SDK.
#[allow(
    clippy::type_complexity,
    reason = "mirrors the TypeScript SDK's pluggable hex-to-CID conversion pattern"
)]
async fn fetch_doc_from_cid_aux(
    hex_to_cid: fn(&str) -> Result<String, CowError>,
    app_data_hex: &str,
    ipfs_uri: Option<&str>,
) -> Result<AppDataDoc, CowError> {
    let cid = hex_to_cid(app_data_hex).map_err(|e| {
        CowError::AppData(format!("Error decoding AppData: appDataHex={app_data_hex}, message={e}"))
    })?;

    if cid.is_empty() {
        return Err(CowError::AppData("Error getting serialized CID".into()));
    }

    fetch_doc_from_cid(&cid, ipfs_uri).await
}

/// Fetch an [`AppDataDoc`] from IPFS using the legacy CID derivation method.
///
/// Converts `app_data_hex` to a CID using the legacy `dag-pb` / `sha2-256`
/// encoding, then fetches the content from IPFS.
///
/// # Errors
///
/// Returns [`CowError::AppData`], [`CowError::Http`], or [`CowError::Parse`].
#[deprecated(
    note = "Use fetch_doc_from_app_data_hex instead — legacy CID encoding is no longer used by CoW Protocol"
)]
#[allow(
    deprecated,
    reason = "this function is itself deprecated and wraps other deprecated functions"
)]
pub async fn fetch_doc_from_app_data_hex_legacy(
    app_data_hex: &str,
    ipfs_uri: Option<&str>,
) -> Result<AppDataDoc, CowError> {
    fetch_doc_from_cid_aux(super::cid::app_data_hex_to_cid_legacy, app_data_hex, ipfs_uri).await
}

/// Upload an [`AppDataDoc`] to IPFS via Pinata using the legacy method.
///
/// The document is pinned to Pinata, and the resulting CID is used to extract
/// the `appData` hex digest.
///
/// # Errors
///
/// Returns [`CowError::AppData`] when credentials are missing or the CID
/// extraction fails, [`CowError::Http`] on transport failure, or
/// [`CowError::Api`] on a non-2xx Pinata response.
#[deprecated(
    note = "Use upload_app_data_to_pinata instead — legacy Pinata pinning relied on implicit encoding"
)]
pub async fn upload_metadata_doc_to_ipfs_legacy(
    doc: &AppDataDoc,
    ipfs: &Ipfs,
) -> Result<IpfsUploadResult, CowError> {
    let cid = upload_app_data_to_pinata_legacy(doc, ipfs).await?;
    let app_data = extract_digest(&cid)?;
    Ok(IpfsUploadResult { app_data, cid })
}

/// Result of uploading metadata to IPFS (legacy).
#[derive(Debug, Clone)]
pub struct IpfsUploadResult {
    /// The `appData` hex digest extracted from the CID.
    pub app_data: String,
    /// The IPFS CID of the pinned content.
    pub cid: String,
}

impl fmt::Display for IpfsUploadResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ipfs-upload(cid={}, appData={})", self.cid, self.app_data)
    }
}

/// Internal legacy Pinata upload (pins with default `CIDv0` encoding).
///
/// This is the Rust equivalent of `_pinJsonInPinataIpfs` in the `TypeScript` SDK.
async fn upload_app_data_to_pinata_legacy(
    doc: &AppDataDoc,
    ipfs: &Ipfs,
) -> Result<String, CowError> {
    let api_key = ipfs
        .pinata_api_key
        .as_deref()
        .ok_or_else(|| CowError::AppData("You need to pass IPFS api credentials.".into()))?;
    let api_secret = ipfs
        .pinata_api_secret
        .as_deref()
        .ok_or_else(|| CowError::AppData("You need to pass IPFS api credentials.".into()))?;

    if api_key.is_empty() || api_secret.is_empty() {
        return Err(CowError::AppData("You need to pass IPFS api credentials.".into()));
    }

    let content: serde_json::Value =
        serde_json::to_value(doc).map_err(|e| CowError::AppData(e.to_string()))?;

    let body = json!({
        "pinataContent": content,
        "pinataMetadata": { "name": "appData" }
    });

    let write_uri = ipfs.write_uri.as_deref().map_or(DEFAULT_IPFS_WRITE_URI, |s| s);
    let url = format!("{write_uri}/pinning/pinJSONToIPFS");

    let resp = reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("pinata_api_key", api_key)
        .header("pinata_secret_api_key", api_secret)
        .json(&body)
        .send()
        .await?;

    let status = resp.status().as_u16();
    let text = resp.text().await?;
    if status != 200 {
        return Err(CowError::Api { status, body: text });
    }

    let pinata: PinataResponse =
        serde_json::from_str(&text).map_err(|e| CowError::AppData(e.to_string()))?;
    Ok(pinata.ipfs_hash)
}

// ── Schema helpers ──────────────────────────────────────────────────────────

/// Known app-data schema versions.
const KNOWN_SCHEMA_VERSIONS: &[&str] = &["0.7.0", "1.3.0"];

/// Import (look up) an app-data schema by version string.
///
/// In the `TypeScript` SDK this dynamically imports a JSON schema file. In
/// Rust, supported schema versions are compiled-in. Returns a placeholder
/// [`AppDataDoc`] with the `version` field set to indicate which schema was
/// requested.
///
/// Currently known versions: `"0.7.0"`, `"1.3.0"`.
///
/// Mirrors `importSchema` from the `@cowprotocol/app-data` `TypeScript`
/// package.
///
/// # Parameters
///
/// * `version` — semver string (e.g. `"1.3.0"`).
///
/// # Returns
///
/// An [`AppDataDoc`] with the requested version and empty metadata.
///
/// # Errors
///
/// Returns [`CowError::AppData`] if `version` is not a valid semver string
/// or is not a known schema version.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::import_schema;
///
/// let doc = import_schema("1.3.0").unwrap();
/// assert_eq!(doc.version, "1.3.0");
///
/// assert!(import_schema("99.0.0").is_err()); // unknown version
/// assert!(import_schema("not-semver").is_err());
/// ```
pub fn import_schema(version: &str) -> Result<AppDataDoc, CowError> {
    // Validate semver format
    let re_parts: Vec<&str> = version.split('.').collect();
    if re_parts.len() != 3 || re_parts.iter().any(|p| p.parse::<u32>().is_err()) {
        return Err(CowError::AppData(format!("AppData version {version} is not a valid version")));
    }

    if !KNOWN_SCHEMA_VERSIONS.contains(&version) {
        return Err(CowError::AppData(format!("AppData version {version} doesn't exist")));
    }

    Ok(AppDataDoc {
        version: version.to_owned(),
        app_code: None,
        environment: None,
        metadata: Metadata::default(),
    })
}

/// Get the app-data schema for a given version.
///
/// Wraps [`import_schema`] and converts errors to [`CowError::AppData`].
///
/// Mirrors `getAppDataSchema` from the `@cowprotocol/app-data` `TypeScript`
/// package.
///
/// # Parameters
///
/// * `version` — semver string (e.g. `"1.3.0"`).
///
/// # Returns
///
/// An [`AppDataDoc`] placeholder with the requested version.
///
/// # Errors
///
/// Returns [`CowError::AppData`] when the version doesn't exist or is not
/// valid semver.
pub fn get_app_data_schema(version: &str) -> Result<AppDataDoc, CowError> {
    import_schema(version).map_err(|e| CowError::AppData(format!("{e}")))
}

// ── MetadataApi ───────────────────────────────────────────────────────────────

/// High-level facade mirroring `MetadataApi` from the `TypeScript` SDK.
///
/// All operations are available as free functions in this module;
/// `MetadataApi` groups them under a single type that carries an optional
/// [`Ipfs`] configuration, so callers do not have to thread IPFS settings
/// through every call.
///
/// # Typical workflow
///
/// ```rust
/// use cow_rs::app_data::{Ipfs, MetadataApi};
///
/// // 1. Create the API (with optional IPFS config).
/// let api = MetadataApi::new();
///
/// // 2. Build an app-data document.
/// let doc = api.generate_app_data_doc("MyDApp");
///
/// // 3. Validate it.
/// let result = api.validate_app_data_doc(&doc);
/// assert!(result.is_valid());
///
/// // 4. Derive the hash + CID.
/// let info = api.get_app_data_info(&doc).unwrap();
/// assert!(info.app_data_hex.starts_with("0x"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct MetadataApi {
    /// Optional IPFS configuration.
    pub ipfs: Ipfs,
}

impl MetadataApi {
    /// Create a new [`MetadataApi`] with default IPFS settings.
    ///
    /// Uses [`DEFAULT_IPFS_READ_URI`] for fetching and
    /// [`DEFAULT_IPFS_WRITE_URI`] for uploads. No Pinata credentials are
    /// configured — set them via [`with_ipfs`](Self::with_ipfs) if you need
    /// upload capability.
    ///
    /// # Returns
    ///
    /// A new [`MetadataApi`] with default [`Ipfs`] configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a [`MetadataApi`] with custom IPFS configuration.
    ///
    /// # Parameters
    ///
    /// * `ipfs` — the [`Ipfs`] settings (gateway URIs, Pinata credentials).
    ///
    /// # Returns
    ///
    /// A new [`MetadataApi`] using the given configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::{Ipfs, MetadataApi};
    ///
    /// let api = MetadataApi::with_ipfs(
    ///     Ipfs::default().with_read_uri("https://my-gateway.io/ipfs").with_pinata("key", "secret"),
    /// );
    /// ```
    #[must_use]
    pub const fn with_ipfs(ipfs: Ipfs) -> Self {
        Self { ipfs }
    }

    /// Generate a minimal [`AppDataDoc`] for `app_code`.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::MetadataApi;
    ///
    /// let api = MetadataApi::new();
    /// let doc = api.generate_app_data_doc("CoW Swap");
    /// assert_eq!(doc.app_code.as_deref(), Some("CoW Swap"));
    /// ```
    #[must_use]
    pub fn generate_app_data_doc(&self, app_code: impl Into<String>) -> AppDataDoc {
        AppDataDoc::new(app_code)
    }

    /// Validate an [`AppDataDoc`].
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::{AppDataDoc, MetadataApi};
    ///
    /// let api = MetadataApi::new();
    /// let doc = AppDataDoc::new("CoW Swap");
    /// let result = api.validate_app_data_doc(&doc);
    /// assert!(result.is_valid());
    /// ```
    #[must_use]
    pub fn validate_app_data_doc(&self, doc: &AppDataDoc) -> ValidationResult {
        validate_app_data_doc(doc)
    }

    /// Compute the `keccak256` hash of `doc` as a [`B256`].
    ///
    /// Delegates to [`appdata_hex`]. The document
    /// is serialised to deterministic JSON before hashing.
    ///
    /// # Parameters
    ///
    /// * `doc` — the [`AppDataDoc`] to hash.
    ///
    /// # Returns
    ///
    /// A 32-byte [`B256`] digest.
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::AppData`] on serialisation failure.
    pub fn appdata_hex(&self, doc: &AppDataDoc) -> Result<B256, CowError> {
        appdata_hex(doc)
    }

    /// Derive the full [`AppDataInfo`] (JSON content, hex hash, CID) from
    /// `doc`.
    ///
    /// Delegates to [`get_app_data_info`]. This is the most common method
    /// for obtaining everything needed to submit an order and pin data on
    /// IPFS.
    ///
    /// # Parameters
    ///
    /// * `doc` — the [`AppDataDoc`] to process.
    ///
    /// # Returns
    ///
    /// An [`AppDataInfo`] with canonical JSON, hex hash, and CID.
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::AppData`].
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::{AppDataDoc, MetadataApi};
    ///
    /// let api = MetadataApi::new();
    /// let doc = AppDataDoc::new("CoW Swap");
    /// let info = api.get_app_data_info(&doc)?;
    /// assert!(info.app_data_hex.starts_with("0x"));
    /// # Ok::<(), cow_rs::error::CowError>(())
    /// ```
    pub fn get_app_data_info(&self, doc: &AppDataDoc) -> Result<AppDataInfo, CowError> {
        get_app_data_info(doc)
    }

    /// Derive [`AppDataInfo`] from a pre-serialised JSON string.
    ///
    /// Hashes `json` directly without re-serialising, preserving the exact
    /// byte sequence as the canonical `keccak256` pre-image.
    ///
    /// # Parameters
    ///
    /// * `json` — the canonical JSON string.
    ///
    /// # Returns
    ///
    /// An [`AppDataInfo`] where `app_data_content` is `json` verbatim.
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::AppData`].
    pub fn get_app_data_info_from_str(&self, json: &str) -> Result<AppDataInfo, CowError> {
        get_app_data_info_from_str(json)
    }

    /// Convert `app_data_hex` to a `CIDv1` base16 string.
    ///
    /// Delegates to
    /// [`appdata_hex_to_cid`].
    ///
    /// # Parameters
    ///
    /// * `app_data_hex` — the `appData` hex value, with or without `0x`.
    ///
    /// # Returns
    ///
    /// A base16 `CIDv1` string (prefix `f`).
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::AppData`].
    pub fn app_data_hex_to_cid(&self, app_data_hex: &str) -> Result<String, CowError> {
        appdata_hex_to_cid(app_data_hex)
    }

    /// Extract the `appData` hex digest from a `CIDv1` string.
    ///
    /// Delegates to
    /// [`cid_to_appdata_hex`].
    ///
    /// # Parameters
    ///
    /// * `cid` — a base16 multibase CID string.
    ///
    /// # Returns
    ///
    /// A `0x`-prefixed hex string of the 32-byte digest.
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::AppData`].
    pub fn cid_to_app_data_hex(&self, cid: &str) -> Result<String, CowError> {
        cid_to_appdata_hex(cid)
    }

    /// Fetch an [`AppDataDoc`] from IPFS by `CIDv1`.
    ///
    /// Uses the configured `ipfs.read_uri` or [`DEFAULT_IPFS_READ_URI`]
    /// when no custom gateway is set.
    ///
    /// # Parameters
    ///
    /// * `cid` — the `CIDv1` base16 string identifying the document on IPFS.
    ///
    /// # Returns
    ///
    /// The deserialised [`AppDataDoc`].
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::Http`] on network failure or
    /// [`CowError::Parse`] if the response is not valid JSON.
    pub async fn fetch_doc_from_cid(&self, cid: &str) -> Result<AppDataDoc, CowError> {
        let uri = self.ipfs.read_uri.as_deref();
        fetch_doc_from_cid(cid, uri).await
    }

    /// Fetch an [`AppDataDoc`] from IPFS by `appData` hex value.
    ///
    /// Converts `app_data_hex` to a `CIDv1`, then fetches the document
    /// from the configured IPFS gateway.
    ///
    /// # Parameters
    ///
    /// * `app_data_hex` — the `0x`-prefixed 32-byte hex value.
    ///
    /// # Returns
    ///
    /// The deserialised [`AppDataDoc`].
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::AppData`], [`CowError::Http`], or
    /// [`CowError::Parse`].
    pub async fn fetch_doc_from_app_data_hex(
        &self,
        app_data_hex: &str,
    ) -> Result<AppDataDoc, CowError> {
        let uri = self.ipfs.read_uri.as_deref();
        fetch_doc_from_app_data_hex(app_data_hex, uri).await
    }

    /// Upload `doc` to IPFS via the Pinata pinning service.
    ///
    /// The document is serialised to deterministic JSON, hashed, and
    /// pinned to Pinata. Requires [`Ipfs::pinata_api_key`] and
    /// [`Ipfs::pinata_api_secret`] to be set on the configured [`Ipfs`]
    /// instance.
    ///
    /// # Parameters
    ///
    /// * `doc` — the [`AppDataDoc`] to upload.
    ///
    /// # Returns
    ///
    /// The IPFS `CIDv1` hash string of the pinned content.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] when credentials are missing,
    /// [`CowError::Http`] on transport failure, or [`CowError::Api`] on a
    /// non-2xx Pinata response.
    pub async fn upload_app_data(&self, doc: &AppDataDoc) -> Result<String, CowError> {
        upload_app_data_to_pinata(doc, &self.ipfs).await
    }

    /// Derive [`AppDataInfo`] using the legacy CID encoding method.
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::AppData`].
    #[deprecated(note = "Use get_app_data_info instead")]
    #[allow(
        deprecated,
        reason = "this method is itself deprecated and delegates to a deprecated function"
    )]
    pub fn get_app_data_info_legacy(&self, doc: &AppDataDoc) -> Result<AppDataInfo, CowError> {
        get_app_data_info_legacy(doc)
    }

    /// Fetch an [`AppDataDoc`] from IPFS using the legacy CID derivation.
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::AppData`], [`CowError::Http`], or [`CowError::Parse`].
    #[deprecated(note = "Use fetch_doc_from_app_data_hex instead")]
    #[allow(
        deprecated,
        reason = "this method is itself deprecated and delegates to a deprecated function"
    )]
    pub async fn fetch_doc_from_app_data_hex_legacy(
        &self,
        app_data_hex: &str,
    ) -> Result<AppDataDoc, CowError> {
        let uri = self.ipfs.read_uri.as_deref();
        fetch_doc_from_app_data_hex_legacy(app_data_hex, uri).await
    }

    /// Upload `doc` to IPFS via Pinata using the legacy method.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`], [`CowError::Http`], or [`CowError::Api`].
    #[deprecated(note = "Use upload_app_data instead")]
    #[allow(
        deprecated,
        reason = "this method is itself deprecated and delegates to a deprecated function"
    )]
    pub async fn upload_metadata_doc_to_ipfs_legacy(
        &self,
        doc: &AppDataDoc,
    ) -> Result<IpfsUploadResult, CowError> {
        upload_metadata_doc_to_ipfs_legacy(doc, &self.ipfs).await
    }

    /// Get the app-data schema for a given version.
    ///
    /// Delegates to [`get_app_data_schema`]. Currently known versions:
    /// `"0.7.0"`, `"1.3.0"`.
    ///
    /// # Parameters
    ///
    /// * `version` — semver string (e.g. `"1.3.0"`).
    ///
    /// # Returns
    ///
    /// An [`AppDataDoc`] placeholder with the requested version.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] when the version doesn't exist.
    pub fn get_app_data_schema(&self, version: &str) -> Result<AppDataDoc, CowError> {
        get_app_data_schema(version)
    }

    /// Import a schema by version string.
    ///
    /// Delegates to [`import_schema`].
    ///
    /// # Parameters
    ///
    /// * `version` — semver string.
    ///
    /// # Returns
    ///
    /// An [`AppDataDoc`] placeholder with the requested version.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] if the version is invalid or unknown.
    pub fn import_schema(&self, version: &str) -> Result<AppDataDoc, CowError> {
        import_schema(version)
    }

    /// Convert `app_data_hex` to a CID using the legacy method.
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::AppData`].
    #[deprecated(note = "Use app_data_hex_to_cid instead")]
    #[allow(
        deprecated,
        reason = "this method is itself deprecated and delegates to a deprecated function"
    )]
    pub fn app_data_hex_to_cid_legacy(&self, app_data_hex: &str) -> Result<String, CowError> {
        super::cid::app_data_hex_to_cid_legacy(app_data_hex)
    }

    /// Parse a CID string into its constituent [`CidComponents`](super::cid::CidComponents).
    ///
    /// Delegates to [`parse_cid`](super::cid::parse_cid). Only base16
    /// CIDs (prefix `f` or `F`) are supported.
    ///
    /// # Parameters
    ///
    /// * `ipfs_hash` — a multibase-encoded CID string.
    ///
    /// # Returns
    ///
    /// A [`CidComponents`](super::cid::CidComponents) with version, codec,
    /// hash function, hash length, and raw digest.
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::AppData`].
    pub fn parse_cid(&self, ipfs_hash: &str) -> Result<super::cid::CidComponents, CowError> {
        super::cid::parse_cid(ipfs_hash)
    }

    /// Decode raw CID bytes into their constituent [`CidComponents`](super::cid::CidComponents).
    ///
    /// Delegates to [`decode_cid`](super::cid::decode_cid).
    ///
    /// # Parameters
    ///
    /// * `bytes` — raw CID bytes (`[version, codec, hash_fn, hash_len, ...digest]`).
    ///
    /// # Returns
    ///
    /// A [`CidComponents`](super::cid::CidComponents) with the parsed fields.
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::AppData`] if the slice is too short.
    pub fn decode_cid(&self, bytes: &[u8]) -> Result<super::cid::CidComponents, CowError> {
        super::cid::decode_cid(bytes)
    }

    /// Extract the multihash digest from a CID string as `0x`-prefixed hex.
    ///
    /// Delegates to [`extract_digest`].
    ///
    /// # Parameters
    ///
    /// * `cid` — a base16 multibase CID string.
    ///
    /// # Returns
    ///
    /// A `0x`-prefixed hex string of the raw digest bytes.
    ///
    /// # Errors
    ///
    /// Propagates [`CowError::AppData`].
    pub fn extract_digest(&self, cid: &str) -> Result<String, CowError> {
        super::cid::extract_digest(cid)
    }
}

impl fmt::Display for MetadataApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("metadata-api")
    }
}
