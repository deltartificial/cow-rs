//! Browser wallet integration via `EIP-1193`.
//!
//! Provides [`BrowserWallet`] which wraps a `JavaScript` signing function
//! and implements [`CowSigner`](cow_signing::CowSigner) for use with
//! [`TradingSdk`](crate::trading::TradingSdk).
//!
//! The [`BrowserWallet`] struct and its [`CowSigner`](cow_signing::CowSigner) implementation
//! are only available when the `wasm` feature is enabled.
//!
//! The [`CowSigner`](cow_signing::CowSigner) trait implementation on [`BrowserWallet`] is
//! further gated to `target_arch = "wasm32"` because it requires `JsFuture` which
//! is not `Send`. On native hosts the struct and utility functions are
//! still available for type-checking and documentation.
//!
//! Platform-independent types ([`WalletSession`], [`WalletEvent`],
//! [`MockBrowserWallet`], [`SignRequest`], [`SignRequestKind`]) are always
//! available for testing and type-checking on any target.

#[allow(
    clippy::disallowed_types,
    reason = "only used for interior mutability in MockBrowserWallet; no async or cross-thread contention"
)]
use std::sync::Mutex;

use alloy_primitives::{Address, B256, keccak256};
use cow_errors::CowError;
#[cfg(target_arch = "wasm32")]
use cow_signing::CowSigner;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

// ── WalletSession ───────────────────────────────────────────────────────────

/// Active wallet connection state.
///
/// Represents a live session between the SDK and a browser wallet.
/// Created when [`BrowserWallet::connect`] (wasm) or
/// [`MockBrowserWallet::connect`] (testing) is called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletSession {
    /// The Ethereum address of the connected account.
    pub address: Address,
    /// The chain ID the wallet is connected to.
    pub chain_id: u64,
    /// Unix timestamp (seconds) when the session was established.
    pub connected_at: u64,
}

impl WalletSession {
    /// Create a new wallet session.
    #[must_use]
    pub const fn new(address: Address, chain_id: u64, connected_at: u64) -> Self {
        Self { address, chain_id, connected_at }
    }

    /// Check whether this session has expired given the current time and a TTL.
    ///
    /// Returns `true` if `now >= connected_at + ttl_secs`, meaning the session
    /// has lived longer than the allowed time-to-live.
    #[must_use]
    pub const fn is_expired(&self, now: u64, ttl_secs: u64) -> bool {
        now >= self.connected_at.saturating_add(ttl_secs)
    }
}

impl core::fmt::Display for WalletSession {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "WalletSession({:#x} on chain {} since {})",
            self.address, self.chain_id, self.connected_at
        )
    }
}

// ── WalletEvent ─────────────────────────────────────────────────────────────

/// Events emitted by an `EIP-1193` provider.
///
/// These mirror the standard events defined in
/// [EIP-1193](https://eips.ethereum.org/EIPS/eip-1193#events):
///
/// - `accountsChanged` — the user switched accounts
/// - `chainChanged` — the user switched networks
/// - `connect` — the provider established a connection
/// - `disconnect` — the provider lost its connection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletEvent {
    /// The list of available accounts changed.
    AccountsChanged(Vec<Address>),
    /// The active chain was switched to the given chain ID.
    ChainChanged(u64),
    /// The provider connected to the given chain.
    Connect {
        /// The chain ID that was connected to.
        chain_id: u64,
    },
    /// The provider disconnected with an error.
    Disconnect {
        /// The `EIP-1193` error code.
        code: u32,
        /// A human-readable disconnect reason.
        message: String,
    },
}

impl WalletEvent {
    /// Returns `true` if this is an [`AccountsChanged`](WalletEvent::AccountsChanged) event.
    #[must_use]
    pub const fn is_accounts_changed(&self) -> bool {
        matches!(self, Self::AccountsChanged(_))
    }

    /// Returns `true` if this is a [`ChainChanged`](WalletEvent::ChainChanged) event.
    #[must_use]
    pub const fn is_chain_changed(&self) -> bool {
        matches!(self, Self::ChainChanged(_))
    }

    /// Returns `true` if this is a [`Connect`](WalletEvent::Connect) event.
    #[must_use]
    pub const fn is_connect(&self) -> bool {
        matches!(self, Self::Connect { .. })
    }

    /// Returns `true` if this is a [`Disconnect`](WalletEvent::Disconnect) event.
    #[must_use]
    pub const fn is_disconnect(&self) -> bool {
        matches!(self, Self::Disconnect { .. })
    }
}

impl core::fmt::Display for WalletEvent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AccountsChanged(addrs) => {
                write!(f, "AccountsChanged({} account(s))", addrs.len())
            }
            Self::ChainChanged(id) => write!(f, "ChainChanged({id})"),
            Self::Connect { chain_id } => write!(f, "Connect(chain_id={chain_id})"),
            Self::Disconnect { code, message } => {
                write!(f, "Disconnect(code={code}, message={message})")
            }
        }
    }
}

// ── SignRequest / SignRequestKind ────────────────────────────────────────────

/// The kind of signing operation requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignRequestKind {
    /// An `EIP-712` typed-data signing request.
    TypedData,
    /// A raw message (`EIP-191` personal-sign) request.
    Message,
}

/// A recorded signing request made to a [`MockBrowserWallet`].
///
/// Captures the kind of signing operation, the raw payload bytes, and the
/// timestamp at which the request was made.
#[derive(Debug, Clone)]
pub struct SignRequest {
    /// Whether this was a typed-data or message signing request.
    pub kind: SignRequestKind,
    /// The raw bytes that were submitted for signing.
    pub data: Vec<u8>,
    /// Unix timestamp (seconds) when the request was recorded.
    pub timestamp: u64,
}

// ── BrowserWallet ────────────────────────────────────────────────────────────

/// A browser wallet signer that delegates to a `JavaScript` `EIP-1193` provider.
///
/// Instead of holding a private key, this struct holds a JS callback function
/// that is called with the `EIP-712` digest and returns a signature via the
/// browser wallet (`MetaMask`, etc.).
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
/// `BrowserWallet` implements [`CowSigner`](cow_signing::CowSigner) on
/// `wasm32` targets, so it can be used anywhere the SDK expects a signer
/// (e.g., `TradingSdk` internals).
#[cfg(feature = "wasm")]
pub struct BrowserWallet {
    /// The Ethereum address associated with this browser wallet.
    address: Address,
    /// A `JavaScript` function `(hex_string) => Promise<string>` that signs
    /// via the browser wallet's `EIP-1193` provider.
    signer_fn: js_sys::Function,
    /// The currently active session, if any.
    session: Option<WalletSession>,
    /// The chain ID this wallet is connected to.
    chain_id: u64,
}

#[cfg(feature = "wasm")]
impl core::fmt::Debug for BrowserWallet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BrowserWallet")
            .field("address", &self.address)
            .field("signer_fn", &"<js_sys::Function>")
            .field("chain_id", &self.chain_id)
            .field("session", &self.session)
            .finish()
    }
}

// SAFETY: `js_sys::Function` is inherently single-threaded in WebAssembly.
// These impls are required so that `BrowserWallet` satisfies the `Send + Sync`
// bounds on `CowSigner` when type-checked on a native (non-wasm32) host.
// In practice this struct is only constructed and used inside a WASM runtime
// where there is a single thread.
#[cfg(feature = "wasm")]
#[allow(unsafe_code, reason = "js_sys types are single-threaded in WASM; Send/Sync are no-ops")]
unsafe impl Send for BrowserWallet {}
#[cfg(feature = "wasm")]
#[allow(unsafe_code, reason = "js_sys types are single-threaded in WASM; Send/Sync are no-ops")]
unsafe impl Sync for BrowserWallet {}

#[cfg(feature = "wasm")]
impl BrowserWallet {
    /// Create a new [`BrowserWallet`] from a parsed [`Address`] and a JS signing function.
    ///
    /// The `signer_fn` should accept a `0x`-prefixed hex string (the digest)
    /// and return a `Promise<string>` resolving to the `0x`-prefixed signature.
    ///
    /// The chain ID defaults to `1` (Ethereum mainnet). Use [`with_chain_id`](Self::with_chain_id)
    /// to override.
    #[must_use]
    pub fn new(address: Address, signer_fn: js_sys::Function) -> Self {
        Self { address, signer_fn, session: None, chain_id: 1 }
    }

    /// Set the chain ID for this wallet, consuming and returning `self`.
    ///
    /// This is a builder-style method for use during construction.
    #[must_use]
    pub const fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = chain_id;
        self
    }

    /// Return the Ethereum address associated with this wallet.
    #[must_use]
    pub const fn address(&self) -> Address {
        self.address
    }

    /// Return a reference to the inner JS signing function.
    #[must_use]
    pub const fn signer_fn(&self) -> &js_sys::Function {
        &self.signer_fn
    }

    /// Return the active session, if any.
    #[must_use]
    pub const fn session(&self) -> Option<&WalletSession> {
        self.session.as_ref()
    }

    /// Return `true` if a session is currently active.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.session.is_some()
    }

    /// Return the chain ID this wallet is configured for.
    #[must_use]
    pub const fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Establish a new session with the current address and chain ID.
    ///
    /// The `now` parameter is the current Unix timestamp in seconds.
    /// Returns a reference to the newly created session.
    #[allow(
        clippy::missing_const_for_fn,
        reason = "Option::as_ref and expect are not const-compatible"
    )]
    pub fn connect(&mut self, now: u64) -> &WalletSession {
        let session = WalletSession::new(self.address, self.chain_id, now);
        self.session = Some(session);
        // SAFETY: we just assigned `Some` above, so unwrap is safe.
        #[allow(
            clippy::expect_used,
            reason = "infallible: we just assigned Some on the previous line"
        )]
        self.session.as_ref().expect("session was just set")
    }

    /// Clear the active session, disconnecting the wallet.
    pub const fn disconnect(&mut self) {
        self.session = None;
    }

    /// Switch to a new chain ID and update the session if one is active.
    ///
    /// If a session is active, its `chain_id` is updated in place.
    pub const fn switch_chain(&mut self, chain_id: u64) {
        self.chain_id = chain_id;
        if let Some(ref mut session) = self.session {
            session.chain_id = chain_id;
        }
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

/// Compute the `EIP-712` signing digest: `keccak256("\x19\x01" || domain_separator ||
/// struct_hash)`.
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
    let stripped = hex_str.strip_prefix("0x").unwrap_or_else(|| hex_str);
    alloy_primitives::hex::decode(stripped)
        .map_err(|e| CowError::Signing(format!("invalid hex signature: {e}")))
}

// ── Utility: detect injected wallet ──────────────────────────────────────────

/// Check whether an injected `window.ethereum` provider exists.
///
/// Returns `true` if `window.ethereum` is defined (i.e., a browser wallet
/// such as `MetaMask` is installed), `false` otherwise.
///
/// This is safe to call in non-browser WASM environments; it will simply
/// return `false` if `window` or `window.ethereum` is not present.
#[cfg(feature = "wasm")]
#[must_use]
pub fn detect_injected_wallet() -> bool {
    let global = js_sys::global();
    let window = js_sys::Reflect::get(&global, &JsValue::from_str("window"))
        .unwrap_or_else(|_| JsValue::UNDEFINED);
    if window.is_undefined() || window.is_null() {
        return false;
    }
    let ethereum = js_sys::Reflect::get(&window, &JsValue::from_str("ethereum"))
        .unwrap_or_else(|_| JsValue::UNDEFINED);
    !ethereum.is_undefined() && !ethereum.is_null()
}

/// Request account addresses from an `EIP-1193` provider via `eth_requestAccounts`.
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

/// Request the wallet to switch to a different Ethereum chain.
///
/// Calls `wallet_switchEthereumChain` on the `EIP-1193` provider. The
/// `chain_id` is encoded as a `0x`-prefixed hex string per the spec.
///
/// # Arguments
///
/// * `ethereum` - A reference to the `window.ethereum` `JsValue`.
/// * `chain_id` - The target chain ID to switch to.
///
/// # Errors
///
/// Returns [`CowError::Signing`] if the RPC call fails or the user rejects
/// the chain switch.
#[cfg(target_arch = "wasm32")]
pub async fn request_switch_chain(ethereum: &JsValue, chain_id: u64) -> Result<(), CowError> {
    let method = JsValue::from_str("wallet_switchEthereumChain");
    let chain_hex = format!("0x{chain_id:x}");

    let chain_param = js_sys::Object::new();
    js_sys::Reflect::set(
        &chain_param,
        &JsValue::from_str("chainId"),
        &JsValue::from_str(&chain_hex),
    )
    .map_err(|e| {
        CowError::Signing(format!("failed to set chainId: {}", e.as_string().unwrap_or_default()))
    })?;

    let params = js_sys::Array::new();
    params.push(&chain_param);

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
            "wallet_switchEthereumChain call failed: {}",
            e.as_string().unwrap_or_default()
        ))
    })?;

    let future = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise));
    future.await.map_err(|e| {
        CowError::Signing(format!(
            "wallet_switchEthereumChain rejected: {}",
            e.as_string().unwrap_or_default()
        ))
    })?;

    Ok(())
}

// ── wasm_bindgen JS exports ──────────────────────────────────────────────────

/// Create a [`BrowserWallet`] from a hex address string and a JS signing function.
///
/// This is the primary constructor exposed to `JavaScript` consumers.
///
/// # Arguments
///
/// * `address` - A `0x`-prefixed Ethereum address string.
/// * `signer_fn` - A `JavaScript` function `(digest: string) => Promise<string>`.
///
/// # Errors
///
/// Returns a `JsValue` error string if the address cannot be parsed.
#[cfg(feature = "wasm")]
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

/// Detect whether a browser wallet (e.g., `MetaMask`) is available.
///
/// Returns `true` if `window.ethereum` exists.
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "detectBrowserWallet")]
#[must_use]
pub fn detect_js() -> bool {
    detect_injected_wallet()
}

/// JavaScript-facing wrapper around [`BrowserWallet`].
///
/// Exposed via `wasm_bindgen` so `JavaScript` code can hold and interact
/// with a `BrowserWallet` instance.
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "BrowserWallet")]
pub struct JsBrowserWallet {
    /// The underlying [`BrowserWallet`] instance.
    inner: BrowserWallet,
}

#[cfg(feature = "wasm")]
impl core::fmt::Debug for JsBrowserWallet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("JsBrowserWallet").field("inner", &self.inner).finish()
    }
}

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_class = "BrowserWallet")]
impl JsBrowserWallet {
    /// Return the wallet's Ethereum address as a `0x`-prefixed hex string.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn address(&self) -> String {
        format!("{:#x}", self.inner.address)
    }
}

// ── MockBrowserWallet ───────────────────────────────────────────────────────

/// Stateful mock browser wallet for testing.
///
/// Tracks all signing requests and can be configured to fail. This type
/// is available on **all** platforms (not just `wasm32`) so it can be used
/// in native unit and integration tests.
///
/// Implements [`CowSigner`](cow_signing::CowSigner) so it can be injected
/// wherever the SDK expects a signer.
///
/// # Example
///
/// ```rust
/// use alloy_primitives::Address;
/// use cow_browser_wallet::wallet::MockBrowserWallet;
///
/// let mut mock = MockBrowserWallet::new(Address::ZERO, 1);
/// mock.connect();
/// assert!(mock.is_connected());
/// ```
#[derive(Debug)]
#[allow(
    clippy::disallowed_types,
    reason = "std::sync::Mutex is adequate for this test-only mock; no async or contention"
)]
pub struct MockBrowserWallet {
    /// The Ethereum address this mock wallet represents.
    address: Address,
    /// The chain ID the mock wallet is connected to.
    chain_id: u64,
    /// Whether the mock wallet is currently connected.
    connected: bool,
    /// All signing requests that have been made to this mock.
    ///
    /// Uses [`Mutex`] for interior mutability so that the
    /// [`CowSigner`](cow_signing::CowSigner) trait methods (which take
    /// `&self`) can record requests without requiring `&mut self`.
    sign_requests: Mutex<Vec<SignRequest>>,
    /// When `true`, all signing operations will return an error.
    should_fail: bool,
    /// Events that have been emitted by this mock wallet.
    events: Vec<WalletEvent>,
}

impl MockBrowserWallet {
    /// Create a new mock browser wallet with the given address and chain ID.
    ///
    /// The wallet starts in a disconnected state with no recorded events
    /// or signing requests.
    #[must_use]
    #[allow(
        clippy::disallowed_types,
        reason = "std::sync::Mutex is adequate for this test-only mock"
    )]
    #[allow(clippy::missing_const_for_fn, reason = "Mutex::new is not const-stable")]
    pub fn new(address: Address, chain_id: u64) -> Self {
        Self {
            address,
            chain_id,
            connected: false,
            sign_requests: Mutex::new(Vec::new()),
            should_fail: false,
            events: Vec::new(),
        }
    }

    /// Simulate connecting the wallet.
    ///
    /// Sets the connected state to `true` and pushes a
    /// [`WalletEvent::Connect`] event.
    pub fn connect(&mut self) {
        self.connected = true;
        self.events.push(WalletEvent::Connect { chain_id: self.chain_id });
    }

    /// Simulate disconnecting the wallet.
    ///
    /// Sets the connected state to `false` and pushes a
    /// [`WalletEvent::Disconnect`] event.
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.events
            .push(WalletEvent::Disconnect { code: 4900, message: "disconnected".to_owned() });
    }

    /// Simulate switching to a different chain.
    ///
    /// Updates the chain ID and pushes a [`WalletEvent::ChainChanged`] event.
    pub fn switch_chain(&mut self, chain_id: u64) {
        self.chain_id = chain_id;
        self.events.push(WalletEvent::ChainChanged(chain_id));
    }

    /// Configure whether signing operations should fail.
    ///
    /// When set to `true`,
    /// [`CowSigner::sign_typed_data`](cow_signing::CowSigner::sign_typed_data)
    /// and [`CowSigner::sign_message`](cow_signing::CowSigner::sign_message) will return
    /// [`CowError::Signing`].
    pub const fn set_should_fail(&mut self, fail: bool) {
        self.should_fail = fail;
    }

    /// Return the number of signing requests that have been recorded.
    #[must_use]
    #[allow(
        clippy::expect_used,
        reason = "Mutex is never poisoned in single-threaded mock context"
    )]
    pub fn sign_request_count(&self) -> usize {
        self.sign_requests.lock().expect("sign_requests lock").len()
    }

    /// Return a clone of the most recent signing request, if any.
    #[must_use]
    #[allow(
        clippy::expect_used,
        reason = "Mutex is never poisoned in single-threaded mock context"
    )]
    pub fn last_sign_request(&self) -> Option<SignRequest> {
        self.sign_requests.lock().expect("sign_requests lock").last().cloned()
    }

    /// Return a slice of all events emitted by this mock wallet.
    #[must_use]
    pub fn events(&self) -> &[WalletEvent] {
        &self.events
    }

    /// Clear all recorded events.
    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    /// Return `true` if the mock wallet is currently connected.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.connected
    }

    /// Return the chain ID the mock wallet is connected to.
    #[must_use]
    pub const fn chain_id(&self) -> u64 {
        self.chain_id
    }
}

// The dummy signature is 65 zero bytes, matching a typical ECDSA signature length.
const MOCK_SIGNATURE: [u8; 65] = [0u8; 65];

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl cow_signing::CowSigner for MockBrowserWallet {
    fn address(&self) -> Address {
        self.address
    }

    async fn sign_typed_data(
        &self,
        domain_separator: B256,
        struct_hash: B256,
    ) -> Result<Vec<u8>, CowError> {
        if self.should_fail {
            return Err(CowError::Signing("mock wallet configured to fail".to_owned()));
        }
        // Build the payload from the two hashes for logging purposes.
        let mut data = Vec::with_capacity(64);
        data.extend_from_slice(domain_separator.as_ref());
        data.extend_from_slice(struct_hash.as_ref());

        #[allow(
            clippy::expect_used,
            reason = "Mutex is never poisoned in single-threaded mock context"
        )]
        self.sign_requests.lock().expect("sign_requests lock").push(SignRequest {
            kind: SignRequestKind::TypedData,
            data,
            timestamp: 0,
        });

        Ok(MOCK_SIGNATURE.to_vec())
    }

    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, CowError> {
        if self.should_fail {
            return Err(CowError::Signing("mock wallet configured to fail".to_owned()));
        }

        #[allow(
            clippy::expect_used,
            reason = "Mutex is never poisoned in single-threaded mock context"
        )]
        self.sign_requests.lock().expect("sign_requests lock").push(SignRequest {
            kind: SignRequestKind::Message,
            data: message.to_vec(),
            timestamp: 0,
        });

        Ok(MOCK_SIGNATURE.to_vec())
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

    /// Helper: a second test address for multi-account scenarios.
    fn test_address_2() -> Address {
        "0x1111111111111111111111111111111111111111".parse().expect("valid address")
    }

    // ── parse_hex_signature tests ───────────────────────────────────────

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

    // ── EIP-712 digest tests ────────────────────────────────────────────

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
        let hex = format!("{addr:#x}");
        let parsed: Address = hex.parse().expect("roundtrip parse");
        assert_eq!(addr, parsed);
    }

    #[test]
    fn cow_signer_trait_is_object_safe() {
        // Verify that CowSigner can be used behind a trait object.
        // This is a compile-time check only.
        fn _assert_object_safe(_: &dyn cow_signing::CowSigner) {}
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

    // ── WalletSession tests ─────────────────────────────────────────────

    #[test]
    fn wallet_session_new() {
        let addr = test_address();
        let session = WalletSession::new(addr, 1, 1_000_000);
        assert_eq!(session.address, addr);
        assert_eq!(session.chain_id, 1);
        assert_eq!(session.connected_at, 1_000_000);
    }

    #[test]
    fn wallet_session_is_expired_false_within_ttl() {
        let session = WalletSession::new(test_address(), 1, 1000);
        // 1000 + 3600 = 4600, so at t=4599 it should NOT be expired.
        assert!(!session.is_expired(4599, 3600));
    }

    #[test]
    fn wallet_session_is_expired_true_at_boundary() {
        let session = WalletSession::new(test_address(), 1, 1000);
        // At exactly connected_at + ttl the session is expired.
        assert!(session.is_expired(4600, 3600));
    }

    #[test]
    fn wallet_session_is_expired_true_past_ttl() {
        let session = WalletSession::new(test_address(), 1, 1000);
        assert!(session.is_expired(9999, 3600));
    }

    #[test]
    fn wallet_session_display() {
        let session = WalletSession::new(test_address(), 42, 1_700_000_000);
        let display = format!("{session}");
        assert!(display.contains("chain 42"), "display should contain chain ID");
        assert!(display.contains("1700000000"), "display should contain timestamp");
    }

    // ── WalletEvent tests ───────────────────────────────────────────────

    #[test]
    fn wallet_event_accounts_changed() {
        let event = WalletEvent::AccountsChanged(vec![test_address()]);
        assert!(event.is_accounts_changed());
        assert!(!event.is_chain_changed());
        assert!(!event.is_connect());
        assert!(!event.is_disconnect());
    }

    #[test]
    fn wallet_event_chain_changed() {
        let event = WalletEvent::ChainChanged(137);
        assert!(!event.is_accounts_changed());
        assert!(event.is_chain_changed());
        assert!(!event.is_connect());
        assert!(!event.is_disconnect());
    }

    #[test]
    fn wallet_event_connect() {
        let event = WalletEvent::Connect { chain_id: 1 };
        assert!(!event.is_accounts_changed());
        assert!(!event.is_chain_changed());
        assert!(event.is_connect());
        assert!(!event.is_disconnect());
    }

    #[test]
    fn wallet_event_disconnect() {
        let event = WalletEvent::Disconnect { code: 4900, message: "connection lost".to_owned() };
        assert!(!event.is_accounts_changed());
        assert!(!event.is_chain_changed());
        assert!(!event.is_connect());
        assert!(event.is_disconnect());
    }

    #[test]
    fn wallet_event_display_accounts_changed() {
        let event = WalletEvent::AccountsChanged(vec![test_address(), test_address_2()]);
        let display = format!("{event}");
        assert!(display.contains("2 account(s)"), "display should show account count");
    }

    #[test]
    fn wallet_event_display_chain_changed() {
        let display = format!("{}", WalletEvent::ChainChanged(42161));
        assert!(display.contains("42161"));
    }

    #[test]
    fn wallet_event_display_connect() {
        let display = format!("{}", WalletEvent::Connect { chain_id: 10 });
        assert!(display.contains("10"));
    }

    #[test]
    fn wallet_event_display_disconnect() {
        let display =
            format!("{}", WalletEvent::Disconnect { code: 4900, message: "bye".to_owned() });
        assert!(display.contains("4900"));
        assert!(display.contains("bye"));
    }

    // ── MockBrowserWallet tests ─────────────────────────────────────────

    #[test]
    fn mock_wallet_new_defaults() {
        let mock = MockBrowserWallet::new(test_address(), 1);
        assert!(!mock.is_connected());
        assert_eq!(mock.chain_id(), 1);
        assert_eq!(mock.sign_request_count(), 0);
        assert!(mock.events().is_empty());
    }

    #[test]
    fn mock_wallet_connect_disconnect() {
        let mut mock = MockBrowserWallet::new(test_address(), 1);
        mock.connect();
        assert!(mock.is_connected());
        assert_eq!(mock.events().len(), 1);
        assert!(mock.events()[0].is_connect());

        mock.disconnect();
        assert!(!mock.is_connected());
        assert_eq!(mock.events().len(), 2);
        assert!(mock.events()[1].is_disconnect());
    }

    #[test]
    fn mock_wallet_switch_chain() {
        let mut mock = MockBrowserWallet::new(test_address(), 1);
        mock.switch_chain(137);
        assert_eq!(mock.chain_id(), 137);
        assert_eq!(mock.events().len(), 1);
        assert!(mock.events()[0].is_chain_changed());
        assert_eq!(mock.events()[0], WalletEvent::ChainChanged(137));
    }

    #[test]
    fn mock_wallet_clear_events() {
        let mut mock = MockBrowserWallet::new(test_address(), 1);
        mock.connect();
        mock.disconnect();
        assert_eq!(mock.events().len(), 2);
        mock.clear_events();
        assert!(mock.events().is_empty());
    }

    #[tokio::test]
    async fn mock_wallet_sign_typed_data_success() {
        let mock = MockBrowserWallet::new(test_address(), 1);
        let result = cow_signing::CowSigner::sign_typed_data(&mock, B256::ZERO, B256::ZERO).await;
        assert!(result.is_ok());
        let sig = result.expect("signing should succeed");
        assert_eq!(sig.len(), 65);
        assert_eq!(mock.sign_request_count(), 1);
        let req = mock.last_sign_request().expect("should have a request");
        assert_eq!(req.kind, SignRequestKind::TypedData);
        assert_eq!(req.data.len(), 64);
    }

    #[tokio::test]
    async fn mock_wallet_sign_typed_data_failure() {
        let mut mock = MockBrowserWallet::new(test_address(), 1);
        mock.set_should_fail(true);
        let result = cow_signing::CowSigner::sign_typed_data(&mock, B256::ZERO, B256::ZERO).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("mock wallet configured to fail"));
    }

    #[tokio::test]
    async fn mock_wallet_sign_message_success() {
        let mock = MockBrowserWallet::new(test_address(), 1);
        let message = b"hello world";
        let result = cow_signing::CowSigner::sign_message(&mock, message).await;
        assert!(result.is_ok());
        let sig = result.expect("signing should succeed");
        assert_eq!(sig.len(), 65);
        assert_eq!(mock.sign_request_count(), 1);
        let req = mock.last_sign_request().expect("should have a request");
        assert_eq!(req.kind, SignRequestKind::Message);
        assert_eq!(req.data, message);
    }

    #[tokio::test]
    async fn mock_wallet_sign_message_failure() {
        let mut mock = MockBrowserWallet::new(test_address(), 1);
        mock.set_should_fail(true);
        let result = cow_signing::CowSigner::sign_message(&mock, b"test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn mock_wallet_multiple_sign_requests() {
        let mock = MockBrowserWallet::new(test_address(), 1);
        cow_signing::CowSigner::sign_typed_data(&mock, B256::ZERO, B256::ZERO).await.unwrap();
        cow_signing::CowSigner::sign_message(&mock, b"msg1").await.unwrap();
        cow_signing::CowSigner::sign_message(&mock, b"msg2").await.unwrap();
        assert_eq!(mock.sign_request_count(), 3);
    }

    #[tokio::test]
    async fn mock_wallet_cow_signer_address() {
        let addr = test_address();
        let mock = MockBrowserWallet::new(addr, 1);
        assert_eq!(cow_signing::CowSigner::address(&mock), addr);
    }

    #[test]
    fn mock_wallet_is_send_and_sync() {
        fn _assert_send<T: Send>() {}
        fn _assert_sync<T: Sync>() {}
        _assert_send::<MockBrowserWallet>();
        _assert_sync::<MockBrowserWallet>();
    }

    #[test]
    fn mock_wallet_implements_cow_signer() {
        fn _assert_cow_signer<T: cow_signing::CowSigner>() {}
        _assert_cow_signer::<MockBrowserWallet>();
    }

    #[test]
    fn sign_request_kind_equality() {
        assert_eq!(SignRequestKind::TypedData, SignRequestKind::TypedData);
        assert_eq!(SignRequestKind::Message, SignRequestKind::Message);
        assert_ne!(SignRequestKind::TypedData, SignRequestKind::Message);
    }

    #[test]
    fn mock_wallet_event_sequence() {
        let mut mock = MockBrowserWallet::new(test_address(), 1);
        mock.connect();
        mock.switch_chain(42);
        mock.disconnect();
        assert_eq!(mock.events().len(), 3);
        assert!(mock.events()[0].is_connect());
        assert!(mock.events()[1].is_chain_changed());
        assert!(mock.events()[2].is_disconnect());
    }

    #[test]
    fn wallet_session_equality() {
        let s1 = WalletSession::new(test_address(), 1, 1000);
        let s2 = WalletSession::new(test_address(), 1, 1000);
        let s3 = WalletSession::new(test_address(), 2, 1000);
        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }
}
