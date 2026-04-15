//! `wasm-bindgen` exports for browser and Node.js usage.
//!
//! Enabled by the `wasm` feature flag. Provides JS-callable wrappers around
//! core SDK functions: EIP-712 hashing, order signing, app-data utilities,
//! CID conversion, and configuration lookups.
//!
//! All complex types are passed as JSON strings and returned as JSON strings
//! or `JsValue` objects.

use cow_sdk_app_data::{
    cid::{appdata_hex_to_cid, cid_to_appdata_hex},
    hash::{appdata_hex, stringify_deterministic},
    ipfs::{get_app_data_info, validate_app_data_doc},
    types::AppDataDoc,
};
use cow_sdk_chains::{
    chain::SupportedChainId,
    contracts::{settlement_contract, vault_relayer},
};
use cow_sdk_signing::{
    eip712::{domain_separator, order_hash, signing_digest},
    types::UnsignedOrder,
    utils::{compute_order_uid, sign_order},
};
use cow_sdk_types::{EcdsaSigningScheme, OrderKind, TokenBalance};
use wasm_bindgen::prelude::*;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Convert any `Display`-able error into a `JsValue` string for WASM interop.
fn to_js_err(e: impl core::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// Parse a JSON string into an [`UnsignedOrder`].
///
/// Expected JSON fields: `sellToken`, `buyToken`, `receiver`, `sellAmount`,
/// `buyAmount`, `validTo`, `appData`, `feeAmount`, `kind`,
/// `partiallyFillable`, `sellTokenBalance`, `buyTokenBalance`.
/// Missing optional fields fall back to sensible defaults.
fn parse_order(json: &str) -> Result<UnsignedOrder, JsValue> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(to_js_err)?;

    let parse_addr = |key: &str| -> Result<alloy_primitives::Address, JsValue> {
        v.get(key)
            .and_then(|s| s.as_str())
            .ok_or_else(|| to_js_err(format!("missing field: {key}")))?
            .parse()
            .map_err(to_js_err)
    };

    let parse_u256 = |key: &str| -> Result<alloy_primitives::U256, JsValue> {
        let s = v
            .get(key)
            .and_then(|s| s.as_str())
            .ok_or_else(|| to_js_err(format!("missing field: {key}")))?;
        s.parse().map_err(to_js_err)
    };

    let kind_str = v.get("kind").and_then(|s| s.as_str()).unwrap_or_else(|| "sell");
    let kind = match kind_str {
        "buy" => OrderKind::Buy,
        _ => OrderKind::Sell,
    };

    let sell_token_balance_str =
        v.get("sellTokenBalance").and_then(|s| s.as_str()).unwrap_or_else(|| "erc20");
    let sell_token_balance = match sell_token_balance_str {
        "external" => TokenBalance::External,
        "internal" => TokenBalance::Internal,
        _ => TokenBalance::Erc20,
    };

    let buy_token_balance_str =
        v.get("buyTokenBalance").and_then(|s| s.as_str()).unwrap_or_else(|| "erc20");
    let buy_token_balance = match buy_token_balance_str {
        "external" => TokenBalance::External,
        "internal" => TokenBalance::Internal,
        _ => TokenBalance::Erc20,
    };

    let valid_to = v.get("validTo").and_then(|n| n.as_u64()).unwrap_or_else(|| 0) as u32;

    let app_data_str = v
        .get("appData")
        .and_then(|s| s.as_str())
        .unwrap_or_else(|| "0x0000000000000000000000000000000000000000000000000000000000000000");
    let app_data: alloy_primitives::B256 = app_data_str.parse().map_err(to_js_err)?;

    let partially_fillable =
        v.get("partiallyFillable").and_then(|b| b.as_bool()).unwrap_or_else(|| false);

    Ok(UnsignedOrder {
        sell_token: parse_addr("sellToken")?,
        buy_token: parse_addr("buyToken")?,
        receiver: parse_addr("receiver").unwrap_or_else(|_| alloy_primitives::Address::ZERO),
        sell_amount: parse_u256("sellAmount")?,
        buy_amount: parse_u256("buyAmount")?,
        valid_to,
        app_data,
        fee_amount: parse_u256("feeAmount").unwrap_or_else(|_| alloy_primitives::U256::ZERO),
        kind,
        partially_fillable,
        sell_token_balance,
        buy_token_balance,
    })
}

/// Format a [`B256`](alloy_primitives::B256) as a `0x`-prefixed lowercase hex string.
fn hex_b256(b: alloy_primitives::B256) -> String {
    format!("0x{}", alloy_primitives::hex::encode(b.as_slice()))
}

// ── EIP-712 Hashing ──────────────────────────────────────────────────────────

/// Compute the EIP-712 domain separator for `CoW` Protocol on the given chain.
///
/// Returns a `0x`-prefixed 32-byte hex string.
#[wasm_bindgen(js_name = "domainSeparator")]
#[must_use]
pub fn wasm_domain_separator(chain_id: u32) -> String {
    hex_b256(domain_separator(u64::from(chain_id)))
}

/// Compute the EIP-712 struct hash for an order.
///
/// `order_json` is a JSON string with fields: `sellToken`, `buyToken`,
/// `receiver`, `sellAmount`, `buyAmount`, `validTo`, `appData`, `feeAmount`,
/// `kind`, `partiallyFillable`, `sellTokenBalance`, `buyTokenBalance`.
///
/// Returns a `0x`-prefixed 32-byte hex string.
#[wasm_bindgen(js_name = "orderHash")]
pub fn wasm_order_hash(order_json: &str) -> Result<String, JsValue> {
    let order = parse_order(order_json)?;
    Ok(hex_b256(order_hash(&order)))
}

/// Compute the EIP-712 signing digest: `keccak256("\x19\x01" || domainSep || orderHash)`.
///
/// Both arguments are `0x`-prefixed 32-byte hex strings.
///
/// Returns a `0x`-prefixed 32-byte hex string.
#[wasm_bindgen(js_name = "signingDigest")]
pub fn wasm_signing_digest(domain_sep_hex: &str, order_hash_hex: &str) -> Result<String, JsValue> {
    let ds: alloy_primitives::B256 = domain_sep_hex.parse().map_err(to_js_err)?;
    let oh: alloy_primitives::B256 = order_hash_hex.parse().map_err(to_js_err)?;
    Ok(hex_b256(signing_digest(ds, oh)))
}

/// Compute the 56-byte order UID for a `CoW` Protocol order.
///
/// Returns a `0x`-prefixed 112-character hex string.
#[wasm_bindgen(js_name = "computeOrderUid")]
pub fn wasm_compute_order_uid(
    chain_id: u32,
    order_json: &str,
    owner: &str,
) -> Result<String, JsValue> {
    let order = parse_order(order_json)?;
    let owner_addr: alloy_primitives::Address = owner.parse().map_err(to_js_err)?;
    Ok(compute_order_uid(u64::from(chain_id), &order, owner_addr))
}

// ── Order Signing ────────────────────────────────────────────────────────────

/// Sign a `CoW` Protocol order with a private key.
///
/// `private_key` is a `0x`-prefixed 32-byte hex string.
/// `scheme` is `"eip712"` or `"ethsign"`.
///
/// Returns a JSON string: `{ "signature": "0x...", "signingScheme": "eip712" }`.
#[wasm_bindgen(js_name = "signOrder")]
pub async fn wasm_sign_order(
    order_json: &str,
    chain_id: u32,
    private_key: &str,
    scheme: &str,
) -> Result<String, JsValue> {
    let order = parse_order(order_json)?;
    let signer: alloy_signer_local::PrivateKeySigner = private_key.parse().map_err(to_js_err)?;
    let ecdsa_scheme = match scheme {
        "ethsign" => EcdsaSigningScheme::EthSign,
        _ => EcdsaSigningScheme::Eip712,
    };
    let result =
        sign_order(&order, u64::from(chain_id), &signer, ecdsa_scheme).await.map_err(to_js_err)?;
    let json = serde_json::json!({
        "signature": result.signature,
        "signingScheme": result.signing_scheme.as_str(),
    });
    serde_json::to_string(&json).map_err(to_js_err)
}

// ── Browser Wallet Signing (EIP-1193) ────────────────────────────────────────

/// Sign a `CoW` Protocol order using a browser wallet via `EIP-1193`.
///
/// Instead of a private key, this function accepts a `JavaScript` callback
/// (`signer_fn`) that receives the EIP-712 signing digest and returns a
/// `Promise<string>` with the `0x`-prefixed hex signature. This allows
/// `MetaMask` or any `EIP-1193` wallet to sign without exposing private keys.
///
/// # Arguments
///
/// * `order_json` — Order JSON string (same format as [`wasm_sign_order`]).
/// * `chain_id` — Numeric chain ID.
/// * `signer_fn` — A `JavaScript` function: `(digest: string) => Promise<string>`.
///
/// # Returns
///
/// A JSON string: `{ "signature": "0x...", "signingScheme": "eip712",
/// "orderHash": "0x...", "domainSeparator": "0x...", "signingDigest": "0x..." }`.
///
/// # `JavaScript` Usage
///
/// ```javascript
/// const signerFn = async (digest) => {
///   return await window.ethereum.request({
///     method: 'personal_sign',
///     params: [digest, account],
///   });
/// };
/// const result = await signOrderWithBrowserWallet(orderJson, chainId, signerFn);
/// ```
#[wasm_bindgen(js_name = "signOrderWithBrowserWallet")]
pub async fn wasm_sign_order_with_browser_wallet(
    order_json: &str,
    chain_id: u32,
    signer_fn: &js_sys::Function,
) -> Result<String, JsValue> {
    let order = parse_order(order_json)?;
    let chain = u64::from(chain_id);

    let domain_sep = domain_separator(chain);
    let o_hash = order_hash(&order);
    let digest = signing_digest(domain_sep, o_hash);

    let domain_sep_hex = hex_b256(domain_sep);
    let order_hash_hex = hex_b256(o_hash);
    let digest_hex = hex_b256(digest);

    // Call the JS signer function with the digest
    let promise =
        signer_fn.call1(&JsValue::NULL, &JsValue::from_str(&digest_hex)).map_err(|e| {
            to_js_err(format!("signer_fn call failed: {}", e.as_string().unwrap_or_default()))
        })?;

    // Await the Promise
    let future = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise));
    let signature = future.await.map_err(|e| {
        to_js_err(format!("signer rejected: {}", e.as_string().unwrap_or_default()))
    })?;

    let sig_str = signature
        .as_string()
        .ok_or_else(|| to_js_err("signer_fn must return a hex string signature"))?;

    let json = serde_json::json!({
        "signature": sig_str,
        "signingScheme": "eip712",
        "orderHash": order_hash_hex,
        "domainSeparator": domain_sep_hex,
        "signingDigest": digest_hex,
    });
    serde_json::to_string(&json).map_err(to_js_err)
}

// ── App-Data ─────────────────────────────────────────────────────────────────

/// Compute the keccak256 app-data hash from an `AppDataDoc` JSON string.
///
/// Returns a `0x`-prefixed 32-byte hex string.
#[wasm_bindgen(js_name = "appdataHex")]
pub fn wasm_appdata_hex(doc_json: &str) -> Result<String, JsValue> {
    let doc: AppDataDoc = serde_json::from_str(doc_json).map_err(to_js_err)?;
    let hash = appdata_hex(&doc).map_err(to_js_err)?;
    Ok(hex_b256(hash))
}

/// Serialise an `AppDataDoc` to canonical JSON with sorted keys.
#[wasm_bindgen(js_name = "stringifyDeterministic")]
pub fn wasm_stringify_deterministic(doc_json: &str) -> Result<String, JsValue> {
    let doc: AppDataDoc = serde_json::from_str(doc_json).map_err(to_js_err)?;
    stringify_deterministic(&doc).map_err(to_js_err)
}

/// Derive full app-data info (CID, content, hex) from an `AppDataDoc` JSON string.
///
/// Returns a JSON string: `{ "cid": "f...", "appDataContent": "...", "appDataHex": "0x..." }`.
#[wasm_bindgen(js_name = "getAppDataInfo")]
pub fn wasm_get_app_data_info(doc_json: &str) -> Result<String, JsValue> {
    let doc: AppDataDoc = serde_json::from_str(doc_json).map_err(to_js_err)?;
    let info = get_app_data_info(&doc).map_err(to_js_err)?;
    let json = serde_json::json!({
        "cid": info.cid,
        "appDataContent": info.app_data_content,
        "appDataHex": info.app_data_hex,
    });
    serde_json::to_string(&json).map_err(to_js_err)
}

/// Validate an `AppDataDoc` JSON string against `CoW` Protocol schema rules.
///
/// Returns a JSON string: `{ "success": bool, "errors": [...] }`.
#[wasm_bindgen(js_name = "validateAppDataDoc")]
pub fn wasm_validate_app_data_doc(doc_json: &str) -> Result<String, JsValue> {
    let doc: AppDataDoc = serde_json::from_str(doc_json).map_err(to_js_err)?;
    let result = validate_app_data_doc(&doc);
    let json = serde_json::json!({
        "success": result.success,
        "errors": result.errors,
    });
    serde_json::to_string(&json).map_err(to_js_err)
}

// ── CID Conversion ───────────────────────────────────────────────────────────

/// Convert an `appDataHex` (32-byte keccak256) to a `CIDv1` base16 string.
#[wasm_bindgen(js_name = "appdataHexToCid")]
pub fn wasm_appdata_hex_to_cid(app_data_hex: &str) -> Result<String, JsValue> {
    appdata_hex_to_cid(app_data_hex).map_err(to_js_err)
}

/// Extract the `appData` hex from a `CIDv1` base16 string.
///
/// Returns a `0x`-prefixed 32-byte hex string.
#[wasm_bindgen(js_name = "cidToAppdataHex")]
pub fn wasm_cid_to_appdata_hex(cid: &str) -> Result<String, JsValue> {
    cid_to_appdata_hex(cid).map_err(to_js_err)
}

// ── Configuration ────────────────────────────────────────────────────────────

/// Get the `GPv2Settlement` contract address for a chain ID.
///
/// Returns a `0x`-prefixed address string.
#[wasm_bindgen(js_name = "settlementContract")]
pub fn wasm_settlement_contract(chain_id: u32) -> Result<String, JsValue> {
    let chain = SupportedChainId::try_from_u64(u64::from(chain_id))
        .ok_or_else(|| to_js_err(format!("unsupported chain ID: {chain_id}")))?;
    Ok(format!("{:#x}", settlement_contract(chain)))
}

/// Get the `GPv2VaultRelayer` address for a chain ID.
///
/// Returns a `0x`-prefixed address string.
#[wasm_bindgen(js_name = "vaultRelayer")]
pub fn wasm_vault_relayer(chain_id: u32) -> Result<String, JsValue> {
    let chain = SupportedChainId::try_from_u64(u64::from(chain_id))
        .ok_or_else(|| to_js_err(format!("unsupported chain ID: {chain_id}")))?;
    Ok(format!("{:#x}", vault_relayer(chain)))
}

/// Get the API base URL for a given chain ID and environment.
///
/// `env` is `"prod"` (default) or `"staging"`.
///
/// Returns the base URL string, or an error if the chain is unsupported.
#[wasm_bindgen(js_name = "apiBaseUrl")]
pub fn wasm_api_base_url(chain_id: u32, env: &str) -> Result<String, JsValue> {
    let chain = SupportedChainId::try_from_u64(u64::from(chain_id))
        .ok_or_else(|| to_js_err(format!("unsupported chain ID: {chain_id}")))?;
    let environment = match env {
        "staging" => cow_sdk_chains::Env::Staging,
        _ => cow_sdk_chains::Env::Prod,
    };
    Ok(cow_sdk_chains::chain::api_base_url(chain, environment).to_owned())
}

/// List all supported chain IDs.
///
/// Returns a JSON array of chain ID numbers.
#[wasm_bindgen(js_name = "supportedChainIds")]
#[must_use]
pub fn wasm_supported_chain_ids() -> String {
    let ids: Vec<u64> = SupportedChainId::all().iter().map(|c| c.as_u64()).collect();
    serde_json::to_string(&ids).unwrap_or_else(|_| "[]".to_owned())
}

// ── OrderBook API (async HTTP) ───────────────────────────────────────────────

/// Parse a `chain_id` / `env` pair into typed values.
///
/// `env` accepts `"staging"` or defaults to `"prod"`.
#[allow(clippy::type_complexity, reason = "tuple return matches domain parse requirements")]
fn parse_chain_env(
    chain_id: u32,
    env: &str,
) -> Result<(SupportedChainId, cow_sdk_chains::Env), JsValue> {
    let chain = SupportedChainId::try_from_u64(u64::from(chain_id))
        .ok_or_else(|| to_js_err(format!("unsupported chain ID: {chain_id}")))?;
    let environment = match env {
        "staging" => cow_sdk_chains::Env::Staging,
        _ => cow_sdk_chains::Env::Prod,
    };
    Ok((chain, environment))
}

/// Fetch a price quote from the `CoW` Protocol orderbook.
///
/// `request_json` is a JSON string matching the `OrderQuoteRequest` schema
/// (fields: `sellToken`, `buyToken`, `kind`, `sellAmountBeforeFee` or
/// `buyAmountAfterFee`, `from`, `appData`, etc.).
///
/// Returns the full quote response as a JSON string.
#[wasm_bindgen(js_name = "getQuote")]
pub async fn wasm_get_quote(
    chain_id: u32,
    env: &str,
    request_json: &str,
) -> Result<String, JsValue> {
    let (chain, environment) = parse_chain_env(chain_id, env)?;
    let api = cow_sdk_orderbook::OrderBookApi::new(chain, environment);
    let req: cow_sdk_orderbook::OrderQuoteRequest =
        serde_json::from_str(request_json).map_err(to_js_err)?;
    let resp = api.get_quote(&req).await.map_err(to_js_err)?;
    serde_json::to_string(&resp).map_err(to_js_err)
}

/// Submit a signed order to the `CoW` Protocol orderbook.
///
/// `order_creation_json` is a JSON string matching the `OrderCreation` schema.
///
/// Returns the order UID as a JSON string.
#[wasm_bindgen(js_name = "sendOrder")]
pub async fn wasm_send_order(
    chain_id: u32,
    env: &str,
    order_creation_json: &str,
) -> Result<String, JsValue> {
    let (chain, environment) = parse_chain_env(chain_id, env)?;
    let api = cow_sdk_orderbook::OrderBookApi::new(chain, environment);
    let creation: cow_sdk_orderbook::OrderCreation =
        serde_json::from_str(order_creation_json).map_err(to_js_err)?;
    let uid = api.send_order(&creation).await.map_err(to_js_err)?;
    serde_json::to_string(&uid).map_err(to_js_err)
}

/// Fetch an order by its UID from the `CoW` Protocol orderbook.
///
/// Returns the full order as a JSON string.
#[wasm_bindgen(js_name = "getOrder")]
pub async fn wasm_get_order(chain_id: u32, env: &str, order_uid: &str) -> Result<String, JsValue> {
    let (chain, environment) = parse_chain_env(chain_id, env)?;
    let api = cow_sdk_orderbook::OrderBookApi::new(chain, environment);
    let order = api.get_order(order_uid).await.map_err(to_js_err)?;
    serde_json::to_string(&order).map_err(to_js_err)
}

/// Fetch trades for an order UID from the `CoW` Protocol orderbook.
///
/// Returns a JSON array of trades.
#[wasm_bindgen(js_name = "getTrades")]
pub async fn wasm_get_trades(chain_id: u32, env: &str, order_uid: &str) -> Result<String, JsValue> {
    let (chain, environment) = parse_chain_env(chain_id, env)?;
    let api = cow_sdk_orderbook::OrderBookApi::new(chain, environment);
    let trades = api.get_trades(Some(order_uid), None).await.map_err(to_js_err)?;
    serde_json::to_string(&trades).map_err(to_js_err)
}

/// Fetch all orders for an account from the `CoW` Protocol orderbook.
///
/// Returns a JSON array of orders.
#[wasm_bindgen(js_name = "getOrdersByOwner")]
pub async fn wasm_get_orders_by_owner(
    chain_id: u32,
    env: &str,
    owner: &str,
) -> Result<String, JsValue> {
    let (chain, environment) = parse_chain_env(chain_id, env)?;
    let api = cow_sdk_orderbook::OrderBookApi::new(chain, environment);
    let owner_addr: alloy_primitives::Address = owner.parse().map_err(to_js_err)?;
    let orders = api.get_orders_for_account(owner_addr, None).await.map_err(to_js_err)?;
    serde_json::to_string(&orders).map_err(to_js_err)
}

/// Get the native token price for a token from the `CoW` Protocol orderbook.
///
/// Returns the price as a JSON string.
#[wasm_bindgen(js_name = "getNativePrice")]
pub async fn wasm_get_native_price(
    chain_id: u32,
    env: &str,
    token: &str,
) -> Result<String, JsValue> {
    let (chain, environment) = parse_chain_env(chain_id, env)?;
    let api = cow_sdk_orderbook::OrderBookApi::new(chain, environment);
    let token_addr: alloy_primitives::Address = token.parse().map_err(to_js_err)?;
    let price = api.get_native_price(token_addr).await.map_err(to_js_err)?;
    Ok(price.to_string())
}

// ── Subgraph API (async HTTP) ────────────────────────────────────────────────

/// Fetch aggregate protocol totals from the `CoW` Protocol subgraph.
///
/// Returns a JSON string with total volumes, trades, fees, etc.
#[wasm_bindgen(js_name = "getSubgraphTotals")]
pub async fn wasm_get_subgraph_totals(chain_id: u32, env: &str) -> Result<String, JsValue> {
    let (chain, environment) = parse_chain_env(chain_id, env)?;
    let api = cow_sdk_subgraph::SubgraphApi::new(chain, environment).map_err(to_js_err)?;
    let totals = api.get_totals().await.map_err(to_js_err)?;
    serde_json::to_string(&totals).map_err(to_js_err)
}

// ── IPFS / App-Data Fetch (async HTTP) ───────────────────────────────────────

/// Fetch an `AppDataDoc` from IPFS by its `CIDv1`.
///
/// Returns the document as a JSON string.
#[wasm_bindgen(js_name = "fetchDocFromCid")]
pub async fn wasm_fetch_doc_from_cid(
    cid: &str,
    ipfs_uri: Option<String>,
) -> Result<String, JsValue> {
    let doc =
        cow_sdk_app_data::fetch_doc_from_cid(cid, ipfs_uri.as_deref()).await.map_err(to_js_err)?;
    serde_json::to_string(&doc).map_err(to_js_err)
}

/// Fetch an `AppDataDoc` from IPFS using a hex `appData` value.
///
/// Returns the document as a JSON string.
#[wasm_bindgen(js_name = "fetchDocFromAppDataHex")]
pub async fn wasm_fetch_doc_from_app_data_hex(
    app_data_hex: &str,
    ipfs_uri: Option<String>,
) -> Result<String, JsValue> {
    let doc = cow_sdk_app_data::fetch_doc_from_app_data_hex(app_data_hex, ipfs_uri.as_deref())
        .await
        .map_err(to_js_err)?;
    serde_json::to_string(&doc).map_err(to_js_err)
}

// ── TradingSdk (async HTTP) ──────────────────────────────────────────────────

/// Create a `TradingSdk`, fetch a quote, sign and submit a swap order.
///
/// `params_json`: `{ "kind": "sell"|"buy", "sellToken": "0x...",
/// "sellTokenDecimals": 18, "buyToken": "0x...", "buyTokenDecimals": 6,
/// "amount": "1000000", "slippageBps": 50 }`
///
/// Returns a JSON string with the order ID, signature, and signed order.
#[wasm_bindgen(js_name = "postSwapOrder")]
pub async fn wasm_post_swap_order(
    chain_id: u32,
    env: &str,
    app_code: &str,
    private_key: &str,
    params_json: &str,
) -> Result<String, JsValue> {
    let (chain, environment) = parse_chain_env(chain_id, env)?;
    let config = match environment {
        cow_sdk_chains::Env::Staging => cow_sdk_trading::TradingSdkConfig::staging(chain, app_code),
        _ => cow_sdk_trading::TradingSdkConfig::prod(chain, app_code),
    };
    let sdk = cow_sdk_trading::TradingSdk::new(config, private_key).map_err(to_js_err)?;

    let v: serde_json::Value = serde_json::from_str(params_json).map_err(to_js_err)?;

    let parse_addr = |key: &str| -> Result<alloy_primitives::Address, JsValue> {
        v.get(key)
            .and_then(|s| s.as_str())
            .ok_or_else(|| to_js_err(format!("missing field: {key}")))?
            .parse()
            .map_err(to_js_err)
    };

    let kind_str = v.get("kind").and_then(|s| s.as_str()).unwrap_or_else(|| "sell");
    let kind = match kind_str {
        "buy" => OrderKind::Buy,
        _ => OrderKind::Sell,
    };

    let amount_str = v
        .get("amount")
        .and_then(|s| s.as_str())
        .ok_or_else(|| to_js_err("missing field: amount"))?;
    let amount: alloy_primitives::U256 = amount_str.parse().map_err(to_js_err)?;

    let params = cow_sdk_trading::TradeParameters {
        kind,
        sell_token: parse_addr("sellToken")?,
        sell_token_decimals: v
            .get("sellTokenDecimals")
            .and_then(|n| n.as_u64())
            .unwrap_or_else(|| 18) as u8,
        buy_token: parse_addr("buyToken")?,
        buy_token_decimals: v.get("buyTokenDecimals").and_then(|n| n.as_u64()).unwrap_or_else(|| 18)
            as u8,
        amount,
        slippage_bps: v.get("slippageBps").and_then(|n| n.as_u64()).map(|n| n as u32),
        receiver: None,
        valid_for: None,
        valid_to: None,
        partially_fillable: None,
        partner_fee: None,
    };

    let result = sdk.post_swap_order(params).await.map_err(to_js_err)?;
    let json = serde_json::json!({
        "orderId": result.order_id,
        "signingScheme": result.signing_scheme.as_str(),
        "signature": result.signature,
    });
    serde_json::to_string(&json).map_err(to_js_err)
}
