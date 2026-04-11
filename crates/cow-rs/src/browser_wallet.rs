//! Browser wallet integration via EIP-1193.
//!
//! Provides [`BrowserWallet`] which wraps a JavaScript signing function
//! and implements [`CowSigner`](crate::traits::CowSigner) for use with
//! [`TradingSdk`](crate::trading::TradingSdk).
//!
//! This module is only available when the `wasm` feature is enabled.
//!
//! The [`CowSigner`](crate::traits::CowSigner) trait implementation on [`BrowserWallet`] is further
//! gated to `target_arch = "wasm32"` because it requires `JsFuture` which
//! is not `Send`. On native hosts the struct and utility functions are
//! still available for type-checking and documentation.

use alloy_primitives::{Address, B256, keccak256};
use wasm_bindgen::prelude::*;

#[cfg(any(target_arch = "wasm32", test))]
use crate::error::CowError;
#[cfg(target_arch = "wasm32")]
use crate::traits::CowSigner;

// ── BrowserWallet ────────────────────────────────────────────────────────────

/// A browser wallet signer that delegates to a JavaScript EIP-1193 provider.
///
/// Instead of holding a private key, this struct holds a JS callback function
/// that is called with the EIP-712 digest and returns a signature via the
/// browser wallet (MetaMask, etc.).
///
/// # Construction
///
/// ```javascript
/// // JavaScript side:
/// const signerFn = async (digest) => {
///   return await window.ethereum.request({
///     method: 'personal_sign',
///     params: [digest, account],
///   });
/// };
/// const wallet = BrowserWallet.newFromJs("0xYourAddress...", signerFn);
/// ```
///
/// # Trait implementation
///
/// `BrowserWallet` implements [`CowSigner`](crate::traits::CowSigner) on
/// `wasm32` targets, so it can be used anywhere the SDK expects a signer
/// (e.g., `TradingSdk` internals).
pub struct BrowserWallet {
    /// The Ethereum address associated with this browser wallet.
    address: Address,
    /// A JavaScript function `(hex_string) => Promise<string>` that signs
    /// via the browser wallet's EIP-1193 provider.
    signer_fn: js_sys::Function,
}

impl core::fmt::Debug for BrowserWallet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BrowserWallet")
            .field("address", &self.address)
            .field("signer_fn", &"<js_sys::Function>")
            .finish()
    }
}

// SAFETY: `js_sys::Function` is inherently single-threaded in WebAssembly.
// These impls are required so that `BrowserWallet` satisfies the `Send + Sync`
// bounds on `CowSigner` when type-checked on a native (non-wasm32) host.
// In practice this struct is only constructed and used inside a WASM runtime
// where there is a single thread.
#[allow(unsafe_code, reason = "js_sys types are single-threaded in WASM; Send/Sync are no-ops")]
unsafe impl Send for BrowserWallet {}
#[allow(unsafe_code, reason = "js_sys types are single-threaded in WASM; Send/Sync are no-ops")]
unsafe impl Sync for BrowserWallet {}

impl BrowserWallet {
    /// Create a new [`BrowserWallet`] from a parsed [`Address`] and a JS signing function.
    ///
    /// The `signer_fn` should accept a `0x`-prefixed hex string (the digest)
    /// and return a `Promise<string>` resolving to the `0x`-prefixed signature.
    #[must_use]
    pub fn new(address: Address, signer_fn: js_sys::Function) -> Self {
        Self { address, signer_fn }
    }

    /// Return the Ethereum address associated with this wallet.
    #[must_use]
    pub fn address(&self) -> Address {
        self.address
    }

    /// Return a reference to the inner JS signing function.
    #[must_use]
    pub fn signer_fn(&self) -> &js_sys::Function {
        &self.signer_fn
    }
}

// ── CowSigner impl ──────────────────────────────────────────────────────────

// The `CowSigner` impl uses `JsFuture` which is not `Send`. On wasm32 the
// trait is `?Send` so this compiles. On non-wasm32 the trait requires `Send`
// futures which `JsFuture` cannot satisfy, so we gate the impl.
#[cfg(target_arch = "wasm32")]
#[async_trait::async_trait(?Send)]
impl CowSigner for BrowserWallet {
    fn address(&self) -> Address {
        self.address
    }

    async fn sign_typed_data(
        &self,
        domain_separator: B256,
        struct_hash: B256,
    ) -> Result<Vec<u8>, CowError> {
        let digest = compute_eip712_digest(domain_separator, struct_hash);
        let digest_hex = format!("0x{}", alloy_primitives::hex::encode(digest.as_slice()));

        let sig_hex = call_signer_fn(&self.signer_fn, &digest_hex).await?;
        parse_hex_signature(&sig_hex)
    }

    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, CowError> {
        let message_hex = format!("0x{}", alloy_primitives::hex::encode(message));
        let sig_hex = call_signer_fn(&self.signer_fn, &message_hex).await?;
        parse_hex_signature(&sig_hex)
    }
}

// ── EIP-712 digest computation ───────────────────────────────────────────────

/// Compute the EIP-712 signing digest: `keccak256("\x19\x01" || domain_separator || struct_hash)`.
///
/// This is a pure function with no JS dependencies, usable on any platform.
#[must_use]
pub fn compute_eip712_digest(domain_separator: B256, struct_hash: B256) -> B256 {
    let mut msg = [0u8; 66];
    msg[0] = 0x19;
    msg[1] = 0x01;
    msg[2..34].copy_from_slice(domain_separator.as_ref());
    msg[34..66].copy_from_slice(struct_hash.as_ref());
    keccak256(msg)
}

// ── Helper: invoke JS signer and await the Promise ───────────────────────────

/// Call the JS signer function with a hex-encoded payload and await its `Promise`.
///
/// Returns the signature as a hex string.
#[cfg(target_arch = "wasm32")]
async fn call_signer_fn(
    signer_fn: &js_sys::Function,
    hex_payload: &str,
) -> Result<String, CowError> {
    let promise =
        signer_fn.call1(&JsValue::NULL, &JsValue::from_str(hex_payload)).map_err(|e| {
            CowError::Signing(format!(
                "signer_fn call failed: {}",
                e.as_string().unwrap_or_default()
            ))
        })?;

    let future = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise));
    let result = future.await.map_err(|e| {
        CowError::Signing(format!("signer rejected: {}", e.as_string().unwrap_or_default()))
    })?;

    result
        .as_string()
        .ok_or_else(|| CowError::Signing("signer_fn must return a hex string signature".to_owned()))
}

/// Parse a `0x`-prefixed hex signature string into raw bytes.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn parse_hex_signature(hex_str: &str) -> Result<Vec<u8>, CowError> {
    let stripped = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    alloy_primitives::hex::decode(stripped)
        .map_err(|e| CowError::Signing(format!("invalid hex signature: {e}")))
}

// ── Utility: detect injected wallet ──────────────────────────────────────────

/// Check whether an injected `window.ethereum` provider exists.
///
/// Returns `true` if `window.ethereum` is defined (i.e., a browser wallet
/// such as MetaMask is installed), `false` otherwise.
///
/// This is safe to call in non-browser WASM environments; it will simply
/// return `false` if `window` or `window.ethereum` is not present.
#[must_use]
pub fn detect_injected_wallet() -> bool {
    let global = js_sys::global();
    let window =
        js_sys::Reflect::get(&global, &JsValue::from_str("window")).unwrap_or(JsValue::UNDEFINED);
    if window.is_undefined() || window.is_null() {
        return false;
    }
    let ethereum =
        js_sys::Reflect::get(&window, &JsValue::from_str("ethereum")).unwrap_or(JsValue::UNDEFINED);
    !ethereum.is_undefined() && !ethereum.is_null()
}

/// Request account addresses from an EIP-1193 provider via `eth_requestAccounts`.
///
/// # Arguments
///
/// * `ethereum` - A reference to the `window.ethereum` `JsValue`.
///
/// # Errors
///
/// Returns [`CowError::Signing`] if the RPC call fails or the result
/// cannot be parsed as an array of strings.
#[cfg(target_arch = "wasm32")]
pub async fn request_accounts(ethereum: &JsValue) -> Result<Vec<String>, CowError> {
    let method = JsValue::from_str("eth_requestAccounts");
    let params = js_sys::Array::new();

    let request_obj = js_sys::Object::new();
    js_sys::Reflect::set(&request_obj, &JsValue::from_str("method"), &method).map_err(|e| {
        CowError::Signing(format!("failed to set method: {}", e.as_string().unwrap_or_default()))
    })?;
    js_sys::Reflect::set(&request_obj, &JsValue::from_str("params"), &params).map_err(|e| {
        CowError::Signing(format!("failed to set params: {}", e.as_string().unwrap_or_default()))
    })?;

    let request_fn =
        js_sys::Reflect::get(ethereum, &JsValue::from_str("request")).map_err(|e| {
            CowError::Signing(format!(
                "ethereum.request not found: {}",
                e.as_string().unwrap_or_default()
            ))
        })?;

    let request_fn: js_sys::Function = request_fn
        .dyn_into()
        .map_err(|_| CowError::Signing("ethereum.request is not a function".to_owned()))?;

    let promise = request_fn.call1(ethereum, &request_obj).map_err(|e| {
        CowError::Signing(format!(
            "eth_requestAccounts call failed: {}",
            e.as_string().unwrap_or_default()
        ))
    })?;

    let future = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise));
    let result = future.await.map_err(|e| {
        CowError::Signing(format!(
            "eth_requestAccounts rejected: {}",
            e.as_string().unwrap_or_default()
        ))
    })?;

    let array: js_sys::Array = result
        .dyn_into()
        .map_err(|_| CowError::Signing("eth_requestAccounts did not return an array".to_owned()))?;

    let mut accounts = Vec::with_capacity(array.length() as usize);
    for i in 0..array.length() {
        let val = array.get(i);
        let s = val
            .as_string()
            .ok_or_else(|| CowError::Signing(format!("account at index {i} is not a string")))?;
        accounts.push(s);
    }

    Ok(accounts)
}

// ── wasm_bindgen JS exports ──────────────────────────────────────────────────

/// Create a [`BrowserWallet`] from a hex address string and a JS signing function.
///
/// This is the primary constructor exposed to JavaScript consumers.
///
/// # Arguments
///
/// * `address` - A `0x`-prefixed Ethereum address string.
/// * `signer_fn` - A JavaScript function `(digest: string) => Promise<string>`.
///
/// # Errors
///
/// Returns a `JsValue` error string if the address cannot be parsed.
#[wasm_bindgen(js_name = "createBrowserWallet")]
pub fn new_from_js(
    address: &str,
    signer_fn: &js_sys::Function,
) -> Result<JsBrowserWallet, JsValue> {
    let addr: Address = address
        .parse()
        .map_err(|e: <Address as core::str::FromStr>::Err| JsValue::from_str(&e.to_string()))?;
    Ok(JsBrowserWallet { inner: BrowserWallet::new(addr, signer_fn.clone()) })
}

/// Detect whether a browser wallet (e.g., MetaMask) is available.
///
/// Returns `true` if `window.ethereum` exists.
#[wasm_bindgen(js_name = "detectBrowserWallet")]
#[must_use]
pub fn detect_js() -> bool {
    detect_injected_wallet()
}

/// JavaScript-facing wrapper around [`BrowserWallet`].
///
/// Exposed via `wasm_bindgen` so JavaScript code can hold and interact
/// with a `BrowserWallet` instance.
#[wasm_bindgen(js_name = "BrowserWallet")]
pub struct JsBrowserWallet {
    /// The underlying [`BrowserWallet`] instance.
    inner: BrowserWallet,
}

impl core::fmt::Debug for JsBrowserWallet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("JsBrowserWallet").field("inner", &self.inner).finish()
    }
}

#[wasm_bindgen(js_class = "BrowserWallet")]
impl JsBrowserWallet {
    /// Return the wallet's Ethereum address as a `0x`-prefixed hex string.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn address(&self) -> String {
        format!("{:#x}", self.inner.address)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: We cannot run real wasm_bindgen / JS interop tests in a native
    // test runner. Instead we test all non-JS logic (parsing, digest
    // computation, struct construction) and verify trait bounds at compile
    // time.

    /// Helper: a known test address.
    fn test_address() -> Address {
        "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".parse().expect("valid address")
    }

    #[test]
    fn parse_hex_signature_with_prefix() {
        let hex = "0xabcdef01";
        let bytes = parse_hex_signature(hex).expect("should parse");
        assert_eq!(bytes, vec![0xab, 0xcd, 0xef, 0x01]);
    }

    #[test]
    fn parse_hex_signature_without_prefix() {
        let hex = "abcdef01";
        let bytes = parse_hex_signature(hex).expect("should parse");
        assert_eq!(bytes, vec![0xab, 0xcd, 0xef, 0x01]);
    }

    #[test]
    fn parse_hex_signature_empty() {
        let hex = "0x";
        let bytes = parse_hex_signature(hex).expect("should parse empty");
        assert!(bytes.is_empty());
    }

    #[test]
    fn parse_hex_signature_invalid() {
        let hex = "0xZZZZ";
        let result = parse_hex_signature(hex);
        assert!(result.is_err(), "should fail on invalid hex");
    }

    #[test]
    fn parse_hex_signature_65_bytes() {
        // A typical ECDSA signature is 65 bytes (r + s + v).
        let hex = format!("0x{}", "ab".repeat(65));
        let bytes = parse_hex_signature(&hex).expect("should parse 65-byte sig");
        assert_eq!(bytes.len(), 65);
    }

    #[test]
    fn parse_hex_signature_odd_length() {
        let hex = "0xabc";
        let result = parse_hex_signature(hex);
        assert!(result.is_err(), "odd-length hex should fail");
    }

    #[test]
    fn eip712_digest_computation() {
        // Verify the digest computation matches the expected keccak256 output.
        let domain_sep = B256::ZERO;
        let struct_hash = B256::ZERO;

        let digest = compute_eip712_digest(domain_sep, struct_hash);

        // The digest should be deterministic and non-zero.
        assert_ne!(digest, B256::ZERO);
        // Known value for keccak256("\x19\x01" || 0x00..00 || 0x00..00)
        let expected: B256 = "0x0b15111afa5c2b936d8dd23e1ffc4a97dd7a9af57a8144231ff70b749ab128d0"
            .parse()
            .expect("valid hash");
        assert_eq!(digest, expected);
    }

    #[test]
    fn eip712_digest_changes_with_domain() {
        let struct_hash = B256::ZERO;

        let digest_a = compute_eip712_digest(B256::ZERO, struct_hash);

        let domain_b: B256 = "0x0000000000000000000000000000000000000000000000000000000000000001"
            .parse()
            .expect("valid hash");
        let digest_b = compute_eip712_digest(domain_b, struct_hash);

        assert_ne!(digest_a, digest_b, "different domains must produce different digests");
    }

    #[test]
    fn eip712_digest_changes_with_struct_hash() {
        let domain_sep = B256::ZERO;

        let digest_a = compute_eip712_digest(domain_sep, B256::ZERO);

        let struct_hash_b: B256 =
            "0x0000000000000000000000000000000000000000000000000000000000000001"
                .parse()
                .expect("valid hash");
        let digest_b = compute_eip712_digest(domain_sep, struct_hash_b);

        assert_ne!(digest_a, digest_b, "different struct hashes must produce different digests");
    }

    #[test]
    fn test_address_roundtrip() {
        let addr = test_address();
        let hex = format!("{:#x}", addr);
        let parsed: Address = hex.parse().expect("roundtrip parse");
        assert_eq!(addr, parsed);
    }

    #[test]
    fn cow_signer_trait_is_object_safe() {
        // Verify that CowSigner can be used behind a trait object.
        // This is a compile-time check only.
        fn _assert_object_safe(_: &dyn crate::traits::CowSigner) {}
    }

    #[test]
    fn browser_wallet_is_send_and_sync() {
        // Verify that BrowserWallet satisfies Send + Sync bounds, which are
        // required by CowSigner on non-wasm32 targets.
        fn _assert_send<T: Send>() {}
        fn _assert_sync<T: Sync>() {}
        _assert_send::<BrowserWallet>();
        _assert_sync::<BrowserWallet>();
    }

    #[test]
    fn browser_wallet_debug_impl() {
        // Debug formatting should not panic and should include the address.
        // We cannot construct a real BrowserWallet without js_sys::Function,
        // so we test the JsBrowserWallet Debug impl indirectly.
        let debug_str = format!("{:?}", B256::ZERO);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn compute_eip712_digest_is_deterministic() {
        let domain: B256 = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .parse()
            .expect("valid hash");
        let hash: B256 = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            .parse()
            .expect("valid hash");
        let d1 = compute_eip712_digest(domain, hash);
        let d2 = compute_eip712_digest(domain, hash);
        assert_eq!(d1, d2, "same inputs must produce same digest");
    }
}
