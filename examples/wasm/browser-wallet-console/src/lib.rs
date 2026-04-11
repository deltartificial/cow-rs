//! Browser wallet console demo for the cow-rs WASM bindings.
//!
//! Provides a `BrowserWalletConsole` struct exported to JS with mock wallet
//! capabilities (deterministic signing with a hardcoded test key), injected
//! wallet detection/signing via JS callbacks, sample data generators, and
//! chain info utilities.
//!
//! # Build
//!
//! ```bash
//! cd examples/wasm/browser-wallet-console
//! wasm-pack build --target web
//! ```
//!
//! Then serve `index.html` with any HTTP server.

use wasm_bindgen::prelude::*;

// Well-known Hardhat account #0 private key (testnet only, never holds real funds).
const HARDHAT_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
// Derived address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
const HARDHAT_ADDRESS: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

// ── Helpers ────────────────────────────────────────────────────────────────

/// Log to the browser console.
fn log(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}

/// Pretty-print a JSON string with indentation.
fn pretty_json(s: &str) -> String {
    serde_json::from_str::<serde_json::Value>(s)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| s.to_owned())
}

/// Extract a human-readable error message from a `JsValue`.
fn fmt_err(e: JsValue) -> String {
    format!(
        "Error: {}",
        e.as_string()
            .or_else(|| js_sys::JSON::stringify(&e).ok().and_then(|s| s.as_string()))
            .unwrap_or_else(|| "(unknown error)".to_owned())
    )
}

// ── BrowserWalletConsole ───────────────────────────────────────────────────

/// A browser wallet console combining mock and injected wallet operations
/// with the CoW Protocol SDK.
#[wasm_bindgen]
pub struct BrowserWalletConsole {
    chain_id: u32,
}

#[wasm_bindgen]
impl BrowserWalletConsole {
    // ── Constructor ────────────────────────────────────────────────────

    /// Create a new console instance for the given chain ID.
    ///
    /// Defaults to Sepolia (11155111) if 0 is passed.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(chain_id: u32) -> Self {
        let chain_id = if chain_id == 0 { 11_155_111 } else { chain_id };
        log(&format!("BrowserWalletConsole created for chain {chain_id}"));
        Self { chain_id }
    }

    // ── Mock Wallet ────────────────────────────────────────────────────

    /// Simulate a wallet connection by returning mock session info.
    ///
    /// Uses the well-known Hardhat account #0 address.
    #[wasm_bindgen(js_name = "mockConnect")]
    #[must_use]
    pub fn mock_connect(&self) -> String {
        let info = serde_json::json!({
            "connected": true,
            "address": HARDHAT_ADDRESS,
            "chainId": self.chain_id,
            "wallet": "mock (Hardhat #0)",
            "note": "This is a deterministic test key. Never send real funds to this address."
        });
        let out = serde_json::to_string_pretty(&info).unwrap_or_default();
        log(&format!("[mock] Connected: {out}"));
        out
    }

    /// Sign an arbitrary message with the hardcoded test key using EIP-191
    /// personal sign (keccak256 of the prefixed message).
    ///
    /// Returns a JSON object with the message, message hash, and signature.
    #[wasm_bindgen(js_name = "mockSignMessage")]
    pub async fn mock_sign_message(&self, message: &str) -> Result<String, JsValue> {
        use alloy_signer::Signer as _;
        use alloy_signer_local::PrivateKeySigner;

        // Compute the Ethereum signed message hash:
        // keccak256("\x19Ethereum Signed Message:\n" + len + message)
        let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
        let mut data = Vec::with_capacity(prefix.len() + message.len());
        data.extend_from_slice(prefix.as_bytes());
        data.extend_from_slice(message.as_bytes());
        let hash = alloy_primitives::keccak256(&data);

        // Sign with the hardcoded key
        let signer: PrivateKeySigner = HARDHAT_KEY
            .parse()
            .map_err(|e: <PrivateKeySigner as core::str::FromStr>::Err| {
                JsValue::from_str(&e.to_string())
            })?;
        let sig = signer
            .sign_hash(&hash)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let result = serde_json::json!({
            "message": message,
            "messageHash": format!("0x{}", alloy_primitives::hex::encode(hash.as_slice())),
            "signature": format!("0x{}", alloy_primitives::hex::encode(sig.as_bytes())),
            "signer": HARDHAT_ADDRESS,
        });
        let out = serde_json::to_string_pretty(&result)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        log(&format!("[mock] Signed message: {out}"));
        Ok(out)
    }

    /// Sign an order with the hardcoded test key using EIP-712.
    ///
    /// `order_json` has the standard CoW Protocol order fields.
    /// Returns a JSON object with signature, signing scheme, order hash, and UID.
    #[wasm_bindgen(js_name = "mockSignOrder")]
    pub async fn mock_sign_order(&self, order_json: &str) -> Result<String, JsValue> {
        log(&format!("[mock] Signing order on chain {}...", self.chain_id));

        // Get order hash first
        let order_hash = cow_rs::wasm::wasm_order_hash(order_json)?;

        // Sign the order
        let sign_result =
            cow_rs::wasm::wasm_sign_order(order_json, self.chain_id, HARDHAT_KEY, "eip712").await?;
        let sign_val: serde_json::Value =
            serde_json::from_str(&sign_result).map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Compute order UID
        let uid = cow_rs::wasm::wasm_compute_order_uid(self.chain_id, order_json, HARDHAT_ADDRESS)?;

        let result = serde_json::json!({
            "orderHash": order_hash,
            "orderUid": uid,
            "signature": sign_val["signature"],
            "signingScheme": sign_val["signingScheme"],
            "signer": HARDHAT_ADDRESS,
            "chainId": self.chain_id,
        });
        let out = serde_json::to_string_pretty(&result)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        log(&format!("[mock] Order signed: {out}"));
        Ok(out)
    }

    /// Get a quote from the real CoW Protocol API.
    ///
    /// `trade_json` is a JSON object with `sellToken`, `buyToken`, `kind`,
    /// `sellAmountBeforeFee` or `buyAmountAfterFee`, etc.
    /// `env` is `"prod"` or `"staging"`.
    #[wasm_bindgen(js_name = "mockGetQuote")]
    pub async fn mock_get_quote(&self, trade_json: &str, env: &str) -> Result<String, JsValue> {
        log(&format!(
            "[mock] Getting quote on chain {} ({env})...",
            self.chain_id
        ));
        let result = cow_rs::wasm::wasm_get_quote(self.chain_id, env, trade_json).await?;
        let out = pretty_json(&result);
        log(&format!("[mock] Quote received: {out}"));
        Ok(out)
    }

    /// Execute a full mock trade flow: quote, sign, and mock submit.
    ///
    /// `trade_json` is a quote request JSON. `env` is `"prod"` or `"staging"`.
    /// `app_code` is an application identifier string.
    ///
    /// This does NOT actually submit the order. It fetches a real quote,
    /// signs it with the mock key, and returns the would-be submission payload.
    #[wasm_bindgen(js_name = "mockFullTradeFlow")]
    pub async fn mock_full_trade_flow(
        &self,
        trade_json: &str,
        env: &str,
        app_code: &str,
    ) -> Result<String, JsValue> {
        log(&format!(
            "[mock] Starting full trade flow on chain {} ({env})...",
            self.chain_id
        ));

        // Step 1: Get quote
        log("[mock] Step 1/3: Fetching quote...");
        let quote_result =
            cow_rs::wasm::wasm_get_quote(self.chain_id, env, trade_json).await?;
        let quote_val: serde_json::Value = serde_json::from_str(&quote_result)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Step 2: Build order from quote and sign it
        log("[mock] Step 2/3: Signing order...");
        let quote_obj = &quote_val["quote"];
        let order_json = serde_json::json!({
            "sellToken": quote_obj["sellToken"],
            "buyToken": quote_obj["buyToken"],
            "receiver": HARDHAT_ADDRESS,
            "sellAmount": quote_obj["sellAmount"],
            "buyAmount": quote_obj["buyAmount"],
            "validTo": quote_obj["validTo"],
            "appData": quote_obj["appData"],
            "feeAmount": quote_obj["feeAmount"],
            "kind": quote_obj["kind"],
            "partiallyFillable": false,
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20",
        });
        let order_str = serde_json::to_string(&order_json)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let sign_result =
            cow_rs::wasm::wasm_sign_order(&order_str, self.chain_id, HARDHAT_KEY, "eip712")
                .await?;
        let sign_val: serde_json::Value = serde_json::from_str(&sign_result)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Compute order hash and UID
        let order_hash = cow_rs::wasm::wasm_order_hash(&order_str)?;
        let order_uid =
            cow_rs::wasm::wasm_compute_order_uid(self.chain_id, &order_str, HARDHAT_ADDRESS)?;

        // Step 3: Build mock submission payload (do NOT actually submit)
        log("[mock] Step 3/3: Building submission payload (not actually submitting)...");
        let result = serde_json::json!({
            "status": "mock_complete",
            "note": "Order was NOT submitted. This is a mock flow demonstration.",
            "steps": {
                "quote": quote_val,
                "signedOrder": {
                    "order": order_json,
                    "orderHash": order_hash,
                    "orderUid": order_uid,
                    "signature": sign_val["signature"],
                    "signingScheme": sign_val["signingScheme"],
                    "signer": HARDHAT_ADDRESS,
                },
                "wouldSubmitTo": cow_rs::wasm::wasm_api_base_url(self.chain_id, env)
                    .unwrap_or_else(|_| "unknown".to_owned()),
            },
            "appCode": app_code,
            "chainId": self.chain_id,
        });
        let out = serde_json::to_string_pretty(&result)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        log(&format!("[mock] Full trade flow complete: {out}"));
        Ok(out)
    }

    // ── Injected Wallet ────────────────────────────────────────────────

    /// Detect whether `window.ethereum` is present in the browser.
    ///
    /// Returns a JSON object with detection results.
    #[wasm_bindgen(js_name = "injectedDetect")]
    #[must_use]
    pub fn injected_detect(&self) -> String {
        let global = js_sys::global();
        let has_window = js_sys::Reflect::get(&global, &JsValue::from_str("window"))
            .ok()
            .map_or(false, |w| !w.is_undefined() && !w.is_null());

        let has_ethereum = if has_window {
            let window = js_sys::Reflect::get(&global, &JsValue::from_str("window"))
                .unwrap_or(JsValue::UNDEFINED);
            js_sys::Reflect::get(&window, &JsValue::from_str("ethereum"))
                .ok()
                .map_or(false, |eth| !eth.is_undefined() && !eth.is_null())
        } else {
            false
        };

        let is_metamask = if has_ethereum {
            let window = js_sys::Reflect::get(&global, &JsValue::from_str("window"))
                .unwrap_or(JsValue::UNDEFINED);
            let ethereum = js_sys::Reflect::get(&window, &JsValue::from_str("ethereum"))
                .unwrap_or(JsValue::UNDEFINED);
            js_sys::Reflect::get(&ethereum, &JsValue::from_str("isMetaMask"))
                .ok()
                .map_or(false, |v| v.as_bool().unwrap_or(false))
        } else {
            false
        };

        let result = serde_json::json!({
            "hasWindow": has_window,
            "hasEthereum": has_ethereum,
            "isMetaMask": is_metamask,
            "status": if has_ethereum { "Injected wallet detected" } else { "No injected wallet found" },
        });
        let out = serde_json::to_string_pretty(&result).unwrap_or_default();
        log(&format!("[injected] Detection: {out}"));
        out
    }

    /// Sign an order using an external JS signing function.
    ///
    /// The `signer_fn` is a JS function that receives the EIP-712 signing digest
    /// (a `0x`-prefixed hex string) and must return a Promise resolving to the
    /// signature (a `0x`-prefixed 65-byte hex string).
    ///
    /// Returns a JSON object with the order hash, digest, and signature.
    #[wasm_bindgen(js_name = "injectedSignOrder")]
    pub async fn injected_sign_order(
        &self,
        order_json: &str,
        signer_fn: &js_sys::Function,
    ) -> Result<String, JsValue> {
        log(&format!(
            "[injected] Signing order on chain {}...",
            self.chain_id
        ));

        // Compute the EIP-712 signing digest
        let domain_sep = cow_rs::wasm::wasm_domain_separator(self.chain_id);
        let order_hash = cow_rs::wasm::wasm_order_hash(order_json)?;
        let digest = cow_rs::wasm::wasm_signing_digest(&domain_sep, &order_hash)?;

        // Call the JS signing function with the digest
        let digest_js = JsValue::from_str(&digest);
        let promise = signer_fn
            .call1(&JsValue::NULL, &digest_js)
            .map_err(|e| JsValue::from_str(&fmt_err(e)))?;
        let future = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise));
        let signature = future.await?;
        let sig_str = signature
            .as_string()
            .ok_or_else(|| JsValue::from_str("signer_fn must return a hex string signature"))?;

        let result = serde_json::json!({
            "orderHash": order_hash,
            "domainSeparator": domain_sep,
            "signingDigest": digest,
            "signature": sig_str,
            "signingScheme": "eip712",
            "chainId": self.chain_id,
        });
        let out = serde_json::to_string_pretty(&result)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        log(&format!("[injected] Order signed: {out}"));
        Ok(out)
    }

    // ── Sample Data Generators ─────────────────────────────────────────

    /// Return a sample trade/quote request JSON for the current chain.
    ///
    /// Uses well-known token addresses for each supported chain.
    #[wasm_bindgen(js_name = "sampleTradeJson")]
    #[must_use]
    pub fn sample_trade_json(&self) -> String {
        let (sell_token, buy_token, amount, sell_decimals, buy_decimals) = match self.chain_id {
            // Mainnet: WETH -> USDC
            1 => (
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                "1000000000000000000",
                18u8,
                6u8,
            ),
            // Gnosis Chain: WXDAI -> USDC
            100 => (
                "0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d",
                "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83",
                "1000000000000000000",
                18,
                6,
            ),
            // Arbitrum: WETH -> USDC
            42161 => (
                "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
                "0xaf88d065e77c8cC2239327C5EDb3A432268e5831",
                "1000000000000000000",
                18,
                6,
            ),
            // Base: WETH -> USDC
            8453 => (
                "0x4200000000000000000000000000000000000006",
                "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
                "1000000000000000000",
                18,
                6,
            ),
            // Sepolia: WETH -> COW (test tokens)
            11155111 => (
                "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14",
                "0x0625aFB445C3B6B7B929342a04A22599fd5dBB59",
                "100000000000000000",
                18,
                18,
            ),
            // Polygon: WPOL -> USDC
            137 => (
                "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270",
                "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
                "1000000000000000000",
                18,
                6,
            ),
            // Default: Sepolia
            _ => (
                "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14",
                "0x0625aFB445C3B6B7B929342a04A22599fd5dBB59",
                "100000000000000000",
                18,
                18,
            ),
        };

        let trade = serde_json::json!({
            "sellToken": sell_token,
            "buyToken": buy_token,
            "from": HARDHAT_ADDRESS,
            "kind": "sell",
            "sellAmountBeforeFee": amount,
            "sellTokenDecimals": sell_decimals,
            "buyTokenDecimals": buy_decimals,
        });
        serde_json::to_string_pretty(&trade).unwrap_or_default()
    }

    /// Return a sample unsigned order JSON for the current chain.
    ///
    /// Uses well-known token addresses and a far-future `validTo` timestamp.
    #[wasm_bindgen(js_name = "sampleOrderJson")]
    #[must_use]
    pub fn sample_order_json(&self) -> String {
        let (sell_token, buy_token) = match self.chain_id {
            1 => (
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
            ),
            100 => (
                "0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d",
                "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83",
            ),
            42161 => (
                "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
                "0xaf88d065e77c8cC2239327C5EDb3A432268e5831",
            ),
            8453 => (
                "0x4200000000000000000000000000000000000006",
                "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
            ),
            11155111 => (
                "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14",
                "0x0625aFB445C3B6B7B929342a04A22599fd5dBB59",
            ),
            137 => (
                "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270",
                "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
            ),
            _ => (
                "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14",
                "0x0625aFB445C3B6B7B929342a04A22599fd5dBB59",
            ),
        };

        let order = serde_json::json!({
            "sellToken": sell_token,
            "buyToken": buy_token,
            "receiver": HARDHAT_ADDRESS,
            "sellAmount": "1000000000000000000",
            "buyAmount": "1000000000",
            "validTo": 1999999999u64,
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "feeAmount": "0",
            "kind": "sell",
            "partiallyFillable": false,
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20"
        });
        serde_json::to_string_pretty(&order).unwrap_or_default()
    }

    // ── Utility ────────────────────────────────────────────────────────

    /// Return chain metadata (name, contracts, API URL) for the given chain ID.
    #[wasm_bindgen(js_name = "chainInfoJson")]
    #[must_use]
    pub fn chain_info_json(&self) -> String {
        let settlement = cow_rs::wasm::wasm_settlement_contract(self.chain_id)
            .unwrap_or_else(|e| fmt_err(e));
        let relayer =
            cow_rs::wasm::wasm_vault_relayer(self.chain_id).unwrap_or_else(|e| fmt_err(e));
        let api_prod =
            cow_rs::wasm::wasm_api_base_url(self.chain_id, "prod").unwrap_or_else(|e| fmt_err(e));
        let api_staging = cow_rs::wasm::wasm_api_base_url(self.chain_id, "staging")
            .unwrap_or_else(|e| fmt_err(e));
        let domain_sep = cow_rs::wasm::wasm_domain_separator(self.chain_id);

        let chain_name = match self.chain_id {
            1 => "Ethereum Mainnet",
            100 => "Gnosis Chain",
            42161 => "Arbitrum One",
            8453 => "Base",
            11155111 => "Sepolia (testnet)",
            137 => "Polygon",
            59144 => "Linea",
            57073 => "Ink",
            _ => "Unknown",
        };

        let is_testnet = self.chain_id == 11_155_111;

        let info = serde_json::json!({
            "chainId": self.chain_id,
            "name": chain_name,
            "isTestnet": is_testnet,
            "contracts": {
                "settlement": settlement,
                "vaultRelayer": relayer,
            },
            "api": {
                "prod": api_prod,
                "staging": api_staging,
            },
            "domainSeparator": domain_sep,
        });
        serde_json::to_string_pretty(&info).unwrap_or_default()
    }

    /// Return all supported chains with metadata.
    #[wasm_bindgen(js_name = "supportedChainsJson")]
    #[must_use]
    pub fn supported_chains_json() -> String {
        let chain_ids_str = cow_rs::wasm::wasm_supported_chain_ids();
        let chain_ids: Vec<u32> =
            serde_json::from_str(&chain_ids_str).unwrap_or_default();

        let chains: Vec<serde_json::Value> = chain_ids
            .iter()
            .map(|&id| {
                let name = match id {
                    1 => "Ethereum Mainnet",
                    100 => "Gnosis Chain",
                    42161 => "Arbitrum One",
                    8453 => "Base",
                    11155111 => "Sepolia (testnet)",
                    137 => "Polygon",
                    59144 => "Linea",
                    57073 => "Ink",
                    _ => "Unknown",
                };
                let api = cow_rs::wasm::wasm_api_base_url(id, "prod")
                    .unwrap_or_else(|e| fmt_err(e));
                serde_json::json!({
                    "chainId": id,
                    "name": name,
                    "isTestnet": id == 11_155_111,
                    "apiUrl": api,
                })
            })
            .collect();

        serde_json::to_string_pretty(&chains).unwrap_or_else(|_| "[]".to_owned())
    }
}

/// Entry point called automatically on WASM module init.
#[wasm_bindgen(start)]
pub fn main() {
    log("cow-rs browser-wallet-console initialized");
}
