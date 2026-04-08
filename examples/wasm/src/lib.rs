//! Browser demo for the cow-rs WASM bindings.
//!
//! Provides thin UI wrappers around every [`cow_rs::wasm`] export, rendering
//! results into a `<pre id="output">` element and logging to the browser
//! console.
//!
//! # Build
//!
//! ```bash
//! cd examples/wasm
//! wasm-pack build --target web
//! ```
//!
//! Then serve `index.html` with any HTTP server.

use wasm_bindgen::prelude::*;

// ── Helpers ────────────────────────────────────────────────────────────────

/// Log a message to the browser console via `console.log`.
fn log(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}

/// Return the DOM [`Document`](web_sys::Document).
///
/// # Panics
///
/// Panics (via `unwrap_throw`) if called outside a browser context.
fn document() -> web_sys::Document {
    web_sys::window().and_then(|w| w.document()).unwrap_throw()
}

/// Write `text` into the `<pre id="output">` element.
///
/// Does nothing if the element is missing from the DOM.
fn set_output(text: &str) {
    if let Some(el) = document().get_element_by_id("output") {
        el.set_text_content(Some(text));
    }
}

/// Pretty-print a JSON string with indentation.
///
/// Returns the original string unchanged if it is not valid JSON.
fn pretty_json(s: &str) -> String {
    serde_json::from_str::<serde_json::Value>(s)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| s.to_owned())
}

/// Extract a human-readable error message from a [`JsValue`].
///
/// Returns `"Error: <message>"` or `"Error: "` if the value is not a string.
fn fmt_err(e: JsValue) -> String {
    format!("Error: {}", e.as_string().unwrap_or_default())
}

// ── Configuration ──────────────────────────────────────────────────────────

/// List all chain IDs supported by the CoW Protocol SDK.
///
/// Delegates to [`cow_rs::wasm::wasm_supported_chain_ids`].
///
/// # Output
///
/// A JSON array of chain ID numbers, e.g. `[1, 100, 11155111, ...]`.
#[wasm_bindgen(js_name = "demoSupportedChains")]
pub fn demo_supported_chains() {
    let chains = cow_rs::wasm::wasm_supported_chain_ids();
    let out = format!("Supported chain IDs:\n{}", pretty_json(&chains));
    log(&out);
    set_output(&out);
}

/// Compute the EIP-712 domain separator for a given chain.
///
/// Delegates to [`cow_rs::wasm::wasm_domain_separator`].
///
/// # Arguments
///
/// * `chain_id` — Numeric chain ID (e.g. `1` for Ethereum mainnet).
///
/// # Output
///
/// A `0x`-prefixed 32-byte hex string.
#[wasm_bindgen(js_name = "demoDomainSeparator")]
pub fn demo_domain_separator(chain_id: u32) {
    let sep = cow_rs::wasm::wasm_domain_separator(chain_id);
    let out = format!("Domain separator (chain {chain_id}):\n{sep}");
    log(&out);
    set_output(&out);
}

/// Look up contract addresses and API URL for a chain.
///
/// Delegates to [`cow_rs::wasm::wasm_settlement_contract`],
/// [`cow_rs::wasm::wasm_vault_relayer`], and
/// [`cow_rs::wasm::wasm_api_base_url`].
///
/// # Arguments
///
/// * `chain_id` — Numeric chain ID.
///
/// # Output
///
/// Settlement address, Vault Relayer address, and API base URL (prod).
#[wasm_bindgen(js_name = "demoContractAddresses")]
pub fn demo_contract_addresses(chain_id: u32) {
    let settlement = cow_rs::wasm::wasm_settlement_contract(chain_id);
    let relayer = cow_rs::wasm::wasm_vault_relayer(chain_id);
    let api = cow_rs::wasm::wasm_api_base_url(chain_id, "prod");

    let out = format!(
        "Chain {chain_id} contracts:\n  Settlement: {}\n  Vault Relayer: {}\n  API URL: {}",
        settlement.unwrap_or_else(|e| fmt_err(e)),
        relayer.unwrap_or_else(|e| fmt_err(e)),
        api.unwrap_or_else(|e| fmt_err(e)),
    );
    log(&out);
    set_output(&out);
}

// ── App Data ───────────────────────────────────────────────────────────────

/// Compute the keccak256 app-data hash from an `AppDataDoc` JSON string.
///
/// Delegates to [`cow_rs::wasm::wasm_appdata_hex`].
///
/// # Arguments
///
/// * `doc_json` — JSON string matching the [`AppDataDoc`] schema,
///   e.g. `{"version":"1.3.0","appCode":"my-app","metadata":{}}`.
///
/// # Output
///
/// A `0x`-prefixed 32-byte hex string (the keccak256 hash).
#[wasm_bindgen(js_name = "demoAppDataHash")]
pub fn demo_app_data_hash(doc_json: &str) {
    match cow_rs::wasm::wasm_appdata_hex(doc_json) {
        Ok(hex) => {
            let out = format!("App-data hash:\n{hex}");
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Derive full app-data info: CID, canonical content, and hex hash.
///
/// Delegates to [`cow_rs::wasm::wasm_get_app_data_info`].
///
/// # Arguments
///
/// * `doc_json` — JSON string matching the [`AppDataDoc`] schema.
///
/// # Output
///
/// A JSON object: `{ "cid": "f...", "appDataContent": "...", "appDataHex": "0x..." }`.
#[wasm_bindgen(js_name = "demoAppDataInfo")]
pub fn demo_app_data_info(doc_json: &str) {
    match cow_rs::wasm::wasm_get_app_data_info(doc_json) {
        Ok(info) => {
            let out = format!("App-data info:\n{}", pretty_json(&info));
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Validate an `AppDataDoc` against CoW Protocol schema rules.
///
/// Delegates to [`cow_rs::wasm::wasm_validate_app_data_doc`].
///
/// # Arguments
///
/// * `doc_json` — JSON string matching the [`AppDataDoc`] schema.
///
/// # Output
///
/// A JSON object: `{ "success": true/false, "errors": [...] }`.
#[wasm_bindgen(js_name = "demoValidateAppData")]
pub fn demo_validate_app_data(doc_json: &str) {
    match cow_rs::wasm::wasm_validate_app_data_doc(doc_json) {
        Ok(result) => {
            let out = format!("Validation result:\n{}", pretty_json(&result));
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Serialize an `AppDataDoc` to canonical deterministic JSON (sorted keys).
///
/// Delegates to [`cow_rs::wasm::wasm_stringify_deterministic`].
///
/// # Arguments
///
/// * `doc_json` — JSON string matching the [`AppDataDoc`] schema.
///
/// # Output
///
/// The canonical JSON string with keys sorted alphabetically.
#[wasm_bindgen(js_name = "demoDeterministicJson")]
pub fn demo_deterministic_json(doc_json: &str) {
    match cow_rs::wasm::wasm_stringify_deterministic(doc_json) {
        Ok(canonical) => {
            let out = format!("Deterministic JSON:\n{}", pretty_json(&canonical));
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

// ── CID Conversion ─────────────────────────────────────────────────────────

/// Convert an `appDataHex` (keccak256 hash) to a CIDv1 base16 string.
///
/// Delegates to [`cow_rs::wasm::wasm_appdata_hex_to_cid`].
///
/// # Arguments
///
/// * `app_data_hex` — `0x`-prefixed 32-byte hex string.
///
/// # Output
///
/// A CIDv1 base16 string, e.g. `f01551b20...`.
#[wasm_bindgen(js_name = "demoHexToCid")]
pub fn demo_hex_to_cid(app_data_hex: &str) {
    match cow_rs::wasm::wasm_appdata_hex_to_cid(app_data_hex) {
        Ok(cid) => {
            let out = format!("CID from hex:\n{cid}");
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Extract the `appDataHex` from a CIDv1 base16 string.
///
/// Delegates to [`cow_rs::wasm::wasm_cid_to_appdata_hex`].
///
/// # Arguments
///
/// * `cid` — CIDv1 base16 string, e.g. `f01551b20...`.
///
/// # Output
///
/// A `0x`-prefixed 32-byte hex string.
#[wasm_bindgen(js_name = "demoCidToHex")]
pub fn demo_cid_to_hex(cid: &str) {
    match cow_rs::wasm::wasm_cid_to_appdata_hex(cid) {
        Ok(hex) => {
            let out = format!("App-data hex from CID:\n{hex}");
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

// ── Order Hashing ──────────────────────────────────────────────────────────

/// Compute the EIP-712 struct hash for a CoW Protocol order.
///
/// Delegates to [`cow_rs::wasm::wasm_order_hash`].
///
/// # Arguments
///
/// * `order_json` — JSON string with fields: `sellToken`, `buyToken`,
///   `receiver`, `sellAmount`, `buyAmount`, `validTo`, `appData`,
///   `feeAmount`, `kind`, `partiallyFillable`, `sellTokenBalance`,
///   `buyTokenBalance`.
///
/// # Output
///
/// A `0x`-prefixed 32-byte hex string.
#[wasm_bindgen(js_name = "demoOrderHash")]
pub fn demo_order_hash(order_json: &str) {
    match cow_rs::wasm::wasm_order_hash(order_json) {
        Ok(hash) => {
            let out = format!("Order hash:\n{hash}");
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Compute the 56-byte order UID for a CoW Protocol order.
///
/// Delegates to [`cow_rs::wasm::wasm_compute_order_uid`].
///
/// # Arguments
///
/// * `chain_id` — Numeric chain ID.
/// * `order_json` — Order JSON string (same format as [`demo_order_hash`]).
/// * `owner` — `0x`-prefixed Ethereum address of the order owner.
///
/// # Output
///
/// A `0x`-prefixed 112-character hex string (56 bytes).
#[wasm_bindgen(js_name = "demoOrderUid")]
pub fn demo_order_uid(chain_id: u32, order_json: &str, owner: &str) {
    match cow_rs::wasm::wasm_compute_order_uid(chain_id, order_json, owner) {
        Ok(uid) => {
            let out = format!("Order UID:\n{uid}");
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Compute the full EIP-712 signing digest for an order on a given chain.
///
/// Computes `keccak256("\x19\x01" || domainSeparator || orderHash)` by
/// chaining [`cow_rs::wasm::wasm_domain_separator`],
/// [`cow_rs::wasm::wasm_order_hash`], and
/// [`cow_rs::wasm::wasm_signing_digest`].
///
/// # Arguments
///
/// * `chain_id` — Numeric chain ID.
/// * `order_json` — Order JSON string (same format as [`demo_order_hash`]).
///
/// # Output
///
/// The domain separator, order hash, and final signing digest — all as
/// `0x`-prefixed 32-byte hex strings.
#[wasm_bindgen(js_name = "demoSigningDigest")]
pub fn demo_signing_digest(chain_id: u32, order_json: &str) {
    let domain_sep = cow_rs::wasm::wasm_domain_separator(chain_id);
    let order_hash = match cow_rs::wasm::wasm_order_hash(order_json) {
        Ok(h) => h,
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
            return;
        }
    };
    match cow_rs::wasm::wasm_signing_digest(&domain_sep, &order_hash) {
        Ok(digest) => {
            let out = format!(
                "Domain separator: {domain_sep}\nOrder hash: {order_hash}\n\nSigning digest:\n{digest}"
            );
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

// ── Order Signing ──────────────────────────────────────────────────────────

/// Sign a CoW Protocol order with a private key.
///
/// Delegates to [`cow_rs::wasm::wasm_sign_order`].
///
/// # Arguments
///
/// * `order_json` — Order JSON string (same format as [`demo_order_hash`]).
/// * `chain_id` — Numeric chain ID.
/// * `private_key` — `0x`-prefixed 32-byte hex private key.
///   **Never use a real key — testnet only.**
/// * `scheme` — `"eip712"` or `"ethsign"`.
///
/// # Output
///
/// A JSON object: `{ "signature": "0x...", "signingScheme": "eip712" }`.
#[wasm_bindgen(js_name = "demoSignOrder")]
pub async fn demo_sign_order(order_json: &str, chain_id: u32, private_key: &str, scheme: &str) {
    set_output("Signing order...");
    match cow_rs::wasm::wasm_sign_order(order_json, chain_id, private_key, scheme).await {
        Ok(result) => {
            let out = format!("Signed order:\n{}", pretty_json(&result));
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

// ── Live API — OrderBook ───────────────────────────────────────────────────

/// Fetch a price quote from the CoW Protocol orderbook.
///
/// Delegates to [`cow_rs::wasm::wasm_get_quote`] (async, hits the network).
///
/// # Arguments
///
/// * `chain_id` — Numeric chain ID.
/// * `request_json` — JSON string matching the `OrderQuoteRequest` schema:
///   `sellToken`, `buyToken`, `kind`, `sellAmountBeforeFee` or
///   `buyAmountAfterFee`, `from`, etc.
///
/// # Output
///
/// The full quote response as a pretty-printed JSON string.
#[wasm_bindgen(js_name = "demoGetQuote")]
pub async fn demo_get_quote(chain_id: u32, request_json: &str) {
    set_output("Fetching quote...");
    match cow_rs::wasm::wasm_get_quote(chain_id, "prod", request_json).await {
        Ok(resp) => {
            let out = format!("Quote response:\n{}", pretty_json(&resp));
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Fetch aggregate protocol totals from the CoW Protocol subgraph.
///
/// Delegates to [`cow_rs::wasm::wasm_get_subgraph_totals`] (async, hits the
/// network).
///
/// # Arguments
///
/// * `chain_id` — Numeric chain ID.
///
/// # Output
///
/// A JSON object with total volumes, trades, fees, etc.
#[wasm_bindgen(js_name = "demoSubgraphTotals")]
pub async fn demo_subgraph_totals(chain_id: u32) {
    set_output("Fetching subgraph totals...");
    match cow_rs::wasm::wasm_get_subgraph_totals(chain_id, "prod").await {
        Ok(resp) => {
            let out = format!("Subgraph totals:\n{}", pretty_json(&resp));
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Fetch a single order by its UID from the CoW Protocol orderbook.
///
/// Delegates to [`cow_rs::wasm::wasm_get_order`] (async, hits the network).
///
/// # Arguments
///
/// * `chain_id` — Numeric chain ID.
/// * `order_uid` — The `0x`-prefixed 112-character order UID.
///
/// # Output
///
/// The full order as a pretty-printed JSON string.
#[wasm_bindgen(js_name = "demoGetOrder")]
pub async fn demo_get_order(chain_id: u32, order_uid: &str) {
    set_output("Fetching order...");
    match cow_rs::wasm::wasm_get_order(chain_id, "prod", order_uid).await {
        Ok(resp) => {
            let out = format!("Order:\n{}", pretty_json(&resp));
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Fetch trades associated with an order UID.
///
/// Delegates to [`cow_rs::wasm::wasm_get_trades`] (async, hits the network).
///
/// # Arguments
///
/// * `chain_id` — Numeric chain ID.
/// * `order_uid` — The `0x`-prefixed 112-character order UID.
///
/// # Output
///
/// A JSON array of trade objects.
#[wasm_bindgen(js_name = "demoGetTrades")]
pub async fn demo_get_trades(chain_id: u32, order_uid: &str) {
    set_output("Fetching trades...");
    match cow_rs::wasm::wasm_get_trades(chain_id, "prod", order_uid).await {
        Ok(resp) => {
            let out = format!("Trades:\n{}", pretty_json(&resp));
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Fetch all orders for an owner address.
///
/// Delegates to [`cow_rs::wasm::wasm_get_orders_by_owner`] (async, hits the
/// network).
///
/// # Arguments
///
/// * `chain_id` — Numeric chain ID.
/// * `owner` — `0x`-prefixed Ethereum address.
///
/// # Output
///
/// A JSON array of order objects.
#[wasm_bindgen(js_name = "demoGetOrdersByOwner")]
pub async fn demo_get_orders_by_owner(chain_id: u32, owner: &str) {
    set_output("Fetching orders...");
    match cow_rs::wasm::wasm_get_orders_by_owner(chain_id, "prod", owner).await {
        Ok(resp) => {
            let out = format!("Orders:\n{}", pretty_json(&resp));
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Get the native token price for a given ERC-20 token.
///
/// Delegates to [`cow_rs::wasm::wasm_get_native_price`] (async, hits the
/// network).
///
/// # Arguments
///
/// * `chain_id` — Numeric chain ID.
/// * `token` — `0x`-prefixed ERC-20 token address.
///
/// # Output
///
/// The price as a decimal string (native token units per one token).
#[wasm_bindgen(js_name = "demoGetNativePrice")]
pub async fn demo_get_native_price(chain_id: u32, token: &str) {
    set_output("Fetching native price...");
    match cow_rs::wasm::wasm_get_native_price(chain_id, "prod", token).await {
        Ok(price) => {
            let out = format!("Native price:\n{price}");
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

// ── IPFS Fetch ─────────────────────────────────────────────────────────────

/// Fetch an `AppDataDoc` from IPFS by its CIDv1.
///
/// Delegates to [`cow_rs::wasm::wasm_fetch_doc_from_cid`] (async, hits the
/// network). Uses the default IPFS gateway.
///
/// # Arguments
///
/// * `cid` — CIDv1 base16 string, e.g. `f01551b20...`.
///
/// # Output
///
/// The `AppDataDoc` as a pretty-printed JSON string.
#[wasm_bindgen(js_name = "demoFetchDocFromCid")]
pub async fn demo_fetch_doc_from_cid(cid: &str) {
    set_output("Fetching from IPFS...");
    match cow_rs::wasm::wasm_fetch_doc_from_cid(cid, None).await {
        Ok(doc) => {
            let out = format!("AppDataDoc from IPFS:\n{}", pretty_json(&doc));
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Fetch an `AppDataDoc` from IPFS using an `appData` hex value.
///
/// Converts the hex to a CID internally, then fetches from IPFS.
/// Delegates to [`cow_rs::wasm::wasm_fetch_doc_from_app_data_hex`] (async,
/// hits the network).
///
/// # Arguments
///
/// * `app_data_hex` — `0x`-prefixed 32-byte hex string.
///
/// # Output
///
/// The `AppDataDoc` as a pretty-printed JSON string.
#[wasm_bindgen(js_name = "demoFetchDocFromHex")]
pub async fn demo_fetch_doc_from_hex(app_data_hex: &str) {
    set_output("Fetching from IPFS...");
    match cow_rs::wasm::wasm_fetch_doc_from_app_data_hex(app_data_hex, None).await {
        Ok(doc) => {
            let out = format!("AppDataDoc from IPFS:\n{}", pretty_json(&doc));
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

// ── Trading SDK (full flow) ────────────────────────────────────────────────

/// Execute a complete swap order: fetch quote, sign, and submit.
///
/// Delegates to [`cow_rs::wasm::wasm_post_swap_order`] (async, hits the
/// network). This submits a **real order** to the CoW Protocol API.
///
/// # Arguments
///
/// * `chain_id` — Numeric chain ID.
/// * `app_code` — Application identifier string, e.g. `"cow-rs-wasm-demo"`.
/// * `private_key` — `0x`-prefixed 32-byte hex private key.
///   **Never use a real key — testnet only.**
/// * `params_json` — JSON string with swap parameters:
///   ```json
///   {
///     "kind": "sell",
///     "sellToken": "0x...",
///     "sellTokenDecimals": 18,
///     "buyToken": "0x...",
///     "buyTokenDecimals": 6,
///     "amount": "1000000000000000000",
///     "slippageBps": 50
///   }
///   ```
///
/// # Output
///
/// A JSON object: `{ "orderId": "...", "signingScheme": "eip712",
/// "signature": "0x..." }`.
#[wasm_bindgen(js_name = "demoPostSwapOrder")]
pub async fn demo_post_swap_order(
    chain_id: u32,
    app_code: &str,
    private_key: &str,
    params_json: &str,
) {
    set_output("Posting swap order (quote → sign → submit)...");
    match cow_rs::wasm::wasm_post_swap_order(chain_id, "prod", app_code, private_key, params_json)
        .await
    {
        Ok(result) => {
            let out = format!("Swap order result:\n{}", pretty_json(&result));
            log(&out);
            set_output(&out);
        }
        Err(e) => {
            let msg = fmt_err(e);
            log(&msg);
            set_output(&msg);
        }
    }
}

/// Entry point — called automatically by `wasm-bindgen` on module init.
#[wasm_bindgen(start)]
pub fn main() {
    log("cow-rs WASM demo initialized");
}
