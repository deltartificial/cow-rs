//! Bridging utility functions — token adaptation, fee math, provider resolution.

use foldhash::{HashMap, HashSet};

use alloy_primitives::{Address, U256};

use crate::{
    app_data::CowHook,
    bridging::types::{BridgeError, MultiQuoteResult},
    config::SupportedChainId,
};

use super::sdk::HOOK_DAPP_BRIDGE_PROVIDER_PREFIX;

// ── Fee / basis-point math ────────────────────────────────────────────────────

/// 100% expressed in the Across percentage format (1e18).
const PCT_100_PERCENT: u128 = 10u128.pow(18);

/// Apply a basis-point adjustment to an amount.
///
/// `result = amount * (10_000 - bps) / 10_000`
///
/// # Example
///
/// ```
/// use alloy_primitives::U256;
/// use cow_rs::bridging::utils::apply_bps;
///
/// let amount = U256::from(10_000u64);
/// assert_eq!(apply_bps(amount, 50), U256::from(9_950u64));
/// ```
#[must_use]
pub fn apply_bps(amount: U256, bps: u32) -> U256 {
    (amount * U256::from(10_000u32 - bps)) / U256::from(10_000u32)
}

/// Apply a percentage fee (in Across 1e18 format) to an amount.
///
/// `result = amount * (1e18 - pct) / 1e18`
///
/// # Errors
///
/// Returns [`BridgeError::QuoteError`] if `pct` exceeds 100% or is negative.
pub fn apply_pct_fee(amount: U256, pct: u128) -> Result<U256, BridgeError> {
    if pct > PCT_100_PERCENT {
        return Err(BridgeError::QuoteError("fee cannot exceed 100%".to_owned()));
    }

    let factor = U256::from(PCT_100_PERCENT - pct);
    Ok((amount * factor) / U256::from(PCT_100_PERCENT))
}

/// Convert an Across percentage (1e18 = 100%) to basis points.
///
/// # Errors
///
/// Returns [`BridgeError::QuoteError`] if `pct` exceeds 100%.
pub fn pct_to_bps(pct: u128) -> Result<u32, BridgeError> {
    if pct > PCT_100_PERCENT {
        return Err(BridgeError::QuoteError("fee cannot exceed 100%".to_owned()));
    }

    // pct * 10_000 / 1e18 — fits in u32 because max is 10_000.
    let bps = (pct * 10_000) / PCT_100_PERCENT;
    Ok(bps as u32)
}

/// Reverse-calculate the fee in basis points from a fee amount and total amount.
///
/// `feeBps = round(feeAmount * 10_000 / amount)`
///
/// # Errors
///
/// Returns [`BridgeError::QuoteError`] if `amount` is zero or `fee_amount > amount`.
pub fn calculate_fee_bps(fee_amount: U256, amount: U256) -> Result<u32, BridgeError> {
    if amount.is_zero() {
        return Err(BridgeError::QuoteError("denominator is zero".to_owned()));
    }
    if fee_amount > amount {
        return Err(BridgeError::QuoteError("fee amount is greater than amount".to_owned()));
    }

    // (feeAmount * 10_000 + amount / 2) / amount — rounded.
    let numerator = fee_amount * U256::from(10_000u32) + amount / U256::from(2u32);
    let bps = numerator / amount;

    // Safe to convert: max value is 10_000.
    Ok(bps.to::<u64>() as u32)
}

/// Calculate a deadline timestamp from a duration in seconds.
///
/// `deadline = now + duration_secs`
///
/// Uses saturating addition to avoid overflow — if the sum would exceed
/// `u64::MAX`, the result is clamped to `u64::MAX`.
///
/// # Arguments
///
/// * `now_secs` — Current Unix timestamp in seconds.
/// * `duration_secs` — Number of seconds until the deadline expires.
///
/// # Returns
///
/// The deadline as a Unix timestamp in seconds.
///
/// # Example
///
/// ```
/// use cow_rs::bridging::utils::calculate_deadline;
///
/// let now = 1_700_000_000;
/// let deadline = calculate_deadline(now, 300);
/// assert_eq!(deadline, 1_700_000_300);
/// ```
#[must_use]
pub const fn calculate_deadline(now_secs: u64, duration_secs: u64) -> u64 {
    now_secs.saturating_add(duration_secs)
}

// ── Hook helpers ──────────────────────────────────────────────────────────────

/// Compare two [`CowHook`] instances for equality.
///
/// Two hooks are equal if their `call_data`, `gas_limit`, and `target` match.
#[must_use]
pub fn are_hooks_equal(hook_a: &CowHook, hook_b: &CowHook) -> bool {
    hook_a.call_data == hook_b.call_data &&
        hook_a.gas_limit == hook_b.gas_limit &&
        hook_a.target == hook_b.target
}

/// Create a mock [`CowHook`] for gas cost estimation.
///
/// The returned hook has minimal calldata and a zero target, suitable for
/// inclusion in an app-data document to estimate gas costs before the real
/// hook is available.
#[must_use]
pub fn hook_mock_for_cost_estimation(gas_limit: u64) -> CowHook {
    CowHook {
        call_data: "0x00".to_owned(),
        gas_limit: gas_limit.to_string(),
        target: "0x0000000000000000000000000000000000000000".to_owned(),
        dapp_id: Some(HOOK_DAPP_BRIDGE_PROVIDER_PREFIX.to_owned()),
    }
}

/// Extract post-hooks from a full app-data JSON string.
///
/// Returns an empty vector if the app-data is invalid or has no post-hooks.
#[must_use]
pub fn get_post_hooks(full_app_data: &str) -> Vec<CowHook> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(full_app_data) else {
        return vec![];
    };

    // Check for the standard app-data structure: { version, metadata: { hooks: { post: [...] } } }
    value
        .get("metadata")
        .and_then(|m| m.get("hooks"))
        .and_then(|h| h.get("post"))
        .and_then(|p| serde_json::from_value::<Vec<CowHook>>(p.clone()).ok())
        .unwrap_or_default()
}

/// Hash a bridge quote for caching purposes.
///
/// Produces a deterministic string key from the provider dApp ID, chain IDs,
/// and token address.
#[must_use]
pub fn hash_quote(
    dapp_id: &str,
    buy_chain_id: u64,
    sell_chain_id: u64,
    token_address: &str,
) -> String {
    format!("{dapp_id}-{buy_chain_id}-{sell_chain_id}-{}", token_address.to_lowercase())
}

// ── Token utilities ───────────────────────────────────────────────────────────

/// Priority stablecoins (USDC/USDT) per chain for intermediate token selection.
///
/// Builds a registry of well-known stablecoin addresses keyed by chain ID.
/// This registry is used by [`determine_intermediate_token`] to prefer
/// stablecoins when choosing an intermediate bridging token.
///
/// # Returns
///
/// A map from chain ID to the set of priority stablecoin [`Address`]es on
/// that chain. Covered chains include Mainnet, BNB, Gnosis, Polygon, Base,
/// Arbitrum One, Avalanche, Linea, and Sepolia.
#[must_use]
pub fn priority_stablecoin_tokens() -> HashMap<u64, HashSet<Address>> {
    let mut map = HashMap::default();

    let mut insert_chain = |chain_id: u64, addrs: &[&str]| {
        let set: HashSet<Address> =
            addrs.iter().filter_map(|a| a.parse::<Address>().ok()).collect();
        map.insert(chain_id, set);
    };

    // Mainnet
    insert_chain(
        SupportedChainId::Mainnet.as_u64(),
        &[
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", // USDC
            "0xdac17f958d2ee523a2206206994597c13d831ec7", // USDT
        ],
    );

    // BNB
    insert_chain(
        SupportedChainId::BnbChain.as_u64(),
        &[
            "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d", // USDC
            "0x55d398326f99059ff775485246999027b3197955", // USDT
        ],
    );

    // Gnosis
    insert_chain(
        SupportedChainId::GnosisChain.as_u64(),
        &[
            "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83", // USDC
            "0x2a22f9c3b484c3629090feed35f17ff8f88f76f0", // USDC.e
            "0x4ecaba5870353805a9f068101a40e0f32ed605c6", // USDT
        ],
    );

    // Polygon
    insert_chain(
        SupportedChainId::Polygon.as_u64(),
        &[
            "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359", // USDC
            "0xc2132d05d31c914a87c6611c10748aeb04b58e8f", // USDT
        ],
    );

    // Base
    insert_chain(
        SupportedChainId::Base.as_u64(),
        &[
            "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913", // USDC
            "0xfde4c96c8593536e31f229ea8f37b2ada2699bb2", // USDT
        ],
    );

    // Arbitrum One
    insert_chain(
        SupportedChainId::ArbitrumOne.as_u64(),
        &[
            "0xaf88d065e77c8cc2239327c5edb3a432268e5831", // USDC
            "0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9", // USDT
        ],
    );

    // Avalanche
    insert_chain(
        SupportedChainId::Avalanche.as_u64(),
        &[
            "0xb97ef9ef8734c71904d8002f8b6bc66dd9c48a6e", // USDC
            "0x9702230a8ea53601f5cd2dc00fdbc13d4df4a8c7", // USDT
        ],
    );

    // Linea
    insert_chain(
        SupportedChainId::Linea.as_u64(),
        &[
            "0x176211869ca2b568f2a7d4ee941e073a821ee1ff", // USDC
            "0xa219439258ca9da29e9cc4ce5596924745e12b93", // USDT
        ],
    );

    // Sepolia
    insert_chain(
        SupportedChainId::Sepolia.as_u64(),
        &[
            "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238", // USDC
        ],
    );

    map
}

/// Check whether a token is a priority stablecoin on the given chain.
///
/// Looks up `token_address` in the registry returned by
/// [`priority_stablecoin_tokens`].
///
/// # Arguments
///
/// * `chain_id` — Numeric chain ID (e.g. `1` for Mainnet, `42161` for Arbitrum).
/// * `token_address` — The token contract address to check.
///
/// # Returns
///
/// `true` if `token_address` is a known priority stablecoin (USDC/USDT)
/// on the given chain, `false` otherwise.
#[must_use]
pub fn is_stablecoin_priority_token(chain_id: u64, token_address: Address) -> bool {
    let registry = priority_stablecoin_tokens();
    registry.get(&chain_id).is_some_and(|set| set.contains(&token_address))
}

/// Check whether a token address is in a set of correlated tokens.
///
/// Correlated tokens are assets whose prices move together (e.g. WETH and
/// stETH). This information is used by [`determine_intermediate_token`] to
/// assign medium priority to correlated candidates.
///
/// # Arguments
///
/// * `token_address` — The token contract address to look up.
/// * `correlated_tokens` — Set of addresses considered correlated to the sell token.
///
/// # Returns
///
/// `true` if `token_address` is present in `correlated_tokens`.
#[must_use]
pub fn is_correlated_token(token_address: Address, correlated_tokens: &HashSet<Address>) -> bool {
    correlated_tokens.contains(&token_address)
}

/// Token priority levels used when selecting the best intermediate token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum TokenPriority {
    /// Fallback priority for unrecognized tokens.
    Lowest = 1,
    /// Native/wrapped native token.
    Low = 2,
    /// Tokens in a correlated-tokens list.
    Medium = 3,
    /// USDC/USDT from the hardcoded registry.
    High = 4,
    /// Same as sell token (highest priority).
    Highest = 5,
}

/// Determine the best intermediate token from a list of candidates.
///
/// Uses a priority-based algorithm:
/// 1. Same as sell token (highest)
/// 2. USDC/USDT from the hardcoded registry
/// 3. Tokens in the CMS correlated-tokens set
/// 4. Blockchain native token
/// 5. Other tokens (lowest)
///
/// # Errors
///
/// Returns [`BridgeError::NoIntermediateTokens`] if `candidates` is empty.
pub fn determine_intermediate_token(
    source_chain_id: u64,
    source_token_address: Address,
    candidates: &[Address],
    correlated_tokens: &HashSet<Address>,
    allow_intermediate_eq_sell: bool,
) -> Result<Address, BridgeError> {
    if candidates.is_empty() {
        return Err(BridgeError::NoIntermediateTokens);
    }

    if candidates.len() == 1 {
        return Ok(candidates[0]);
    }

    let native = crate::config::NATIVE_CURRENCY_ADDRESS;

    let filtered: Vec<Address> = if allow_intermediate_eq_sell {
        candidates.to_vec()
    } else {
        candidates.iter().copied().filter(|a| *a != source_token_address).collect()
    };

    if filtered.is_empty() {
        return Err(BridgeError::NoIntermediateTokens);
    }

    let is_sell_native = source_token_address == native;

    let mut scored: Vec<(Address, TokenPriority)> = filtered
        .iter()
        .map(|&addr| {
            let is_native = addr == native;

            if addr == source_token_address && !(is_sell_native && is_native) {
                return (addr, TokenPriority::Highest);
            }
            if is_stablecoin_priority_token(source_chain_id, addr) {
                return (addr, TokenPriority::High);
            }
            if is_correlated_token(addr, correlated_tokens) {
                return (addr, TokenPriority::Medium);
            }
            if is_native && !is_sell_native {
                return (addr, TokenPriority::Low);
            }
            (addr, TokenPriority::Lowest)
        })
        .collect();

    // Sort by priority descending (stable sort preserves original order for ties).
    scored.sort_unstable_by_key(|item| std::cmp::Reverse(item.1));

    scored.first().map(|(addr, _)| *addr).ok_or(BridgeError::NoIntermediateTokens)
}

// ── Query-string builder ──────────────────────────────────────────────────────

/// A key-value pair for building URL query strings.
///
/// The key is a parameter name, and the value is a list of strings that will be
/// joined with commas.
pub type QueryParam = (String, Vec<String>);

/// Convert key-value pairs to a URL query string.
///
/// Array values are joined with commas, mirroring the Bungee API convention.
///
/// # Example
///
/// ```
/// use cow_rs::bridging::utils::object_to_search_params;
///
/// let params = vec![
///     ("userAddress".to_owned(), vec!["0x123".to_owned()]),
///     ("includeBridges".to_owned(), vec!["across".to_owned(), "cctp".to_owned()]),
/// ];
/// let qs = object_to_search_params(&params);
/// assert!(qs.contains("userAddress=0x123"));
/// assert!(
///     qs.contains("includeBridges=across%2Ccctp") || qs.contains("includeBridges=across,cctp")
/// );
/// ```
#[must_use]
pub fn object_to_search_params(params: &[QueryParam]) -> String {
    let pairs: Vec<String> = params
        .iter()
        .map(|(key, values)| {
            let value = values.join(",");
            format!("{key}={value}")
        })
        .collect();
    pairs.join("&")
}

// ── Provider resolution ───────────────────────────────────────────────────────

/// Validate that a cross-chain request involves different chains.
///
/// # Errors
///
/// Returns [`BridgeError::SameChain`] if sell and buy chain IDs are equal.
pub const fn validate_cross_chain_request(
    sell_chain_id: u64,
    buy_chain_id: u64,
) -> Result<(), BridgeError> {
    if sell_chain_id == buy_chain_id {
        return Err(BridgeError::SameChain);
    }
    Ok(())
}

/// Find the bridge provider dApp ID from app-data JSON.
///
/// Looks first at `metadata.bridging.providerId`, then falls back to
/// scanning post-hooks for one matching the bridge provider prefix.
#[must_use]
pub fn find_bridge_provider_dapp_id(full_app_data: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(full_app_data).ok()?;

    // First check metadata.bridging.providerId
    if let Some(provider_id) = value
        .get("metadata")
        .and_then(|m| m.get("bridging"))
        .and_then(|b| b.get("providerId"))
        .and_then(|p| p.as_str())
    {
        return Some(provider_id.to_owned());
    }

    // Fall back to scanning post-hooks
    let post_hooks = get_post_hooks(full_app_data);
    post_hooks
        .into_iter()
        .find_map(|hook| hook.dapp_id.filter(|id| id.starts_with(HOOK_DAPP_BRIDGE_PROVIDER_PREFIX)))
}

// ── Quote comparison ──────────────────────────────────────────────────────────

/// Returns `true` if `candidate` is a better quote than `current_best`.
///
/// A quote is better if it has a higher buy amount after slippage, or if
/// `current_best` has no successful quote.
#[must_use]
pub fn is_better_quote(
    candidate: &MultiQuoteResult,
    current_best: Option<&MultiQuoteResult>,
) -> bool {
    let Some(best) = current_best else {
        return candidate.quote.is_some();
    };

    match (&candidate.quote, &best.quote) {
        (Some(c), Some(b)) => c.after_slippage.buy_amount > b.after_slippage.buy_amount,
        (Some(_), None) => true,
        _ => false,
    }
}

/// Returns `true` if `candidate` has a more informative error than `current_best`.
///
/// Higher-priority errors (e.g. "sell amount too small") are preferred over
/// generic errors so the user sees the most actionable message.
#[must_use]
pub const fn is_better_error(
    candidate: Option<&BridgeError>,
    current_best: Option<&BridgeError>,
) -> bool {
    match (candidate, current_best) {
        (Some(c), Some(b)) => {
            crate::bridging::types::bridge_error_priority(c) >
                crate::bridging::types::bridge_error_priority(b)
        }
        (Some(_), None) => true,
        _ => false,
    }
}

/// Fill timeout results for providers that did not respond in time.
///
/// Iterates over `provider_dapp_ids` and, for each index where `results`
/// has no quote and no error (or has no entry at all), inserts a
/// [`MultiQuoteResult`] with a `"provider request timed out"` error.
///
/// # Arguments
///
/// * `results` — Mutable vector of quote results, indexed in the same order as `provider_dapp_ids`.
///   Entries that already contain a quote or an error are left unchanged.
/// * `provider_dapp_ids` — Ordered list of provider dApp IDs that were queried. Any provider
///   without a corresponding result is treated as having timed out.
pub fn fill_timeout_results(results: &mut Vec<MultiQuoteResult>, provider_dapp_ids: &[String]) {
    for (i, dapp_id) in provider_dapp_ids.iter().enumerate() {
        if i >= results.len() || results[i].quote.is_none() && results[i].error.is_none() {
            if i < results.len() {
                results[i] = MultiQuoteResult {
                    provider_dapp_id: dapp_id.clone(),
                    quote: None,
                    error: Some("provider request timed out".to_owned()),
                };
            } else {
                results.push(MultiQuoteResult {
                    provider_dapp_id: dapp_id.clone(),
                    quote: None,
                    error: Some("provider request timed out".to_owned()),
                });
            }
        }
    }
}

// ── Gas estimation ────────────────────────────────────────────────────────────

/// Default gas cost for the bridge hook itself.
pub const COW_SHED_PROXY_CREATION_GAS: u64 = 360_000;

/// Estimate gas limit for a bridge hook.
///
/// If the proxy is not deployed (`proxy_deployed` is `false`), adds the proxy
/// creation gas on top. Extra gas can be added via `extra_gas` and
/// `extra_gas_proxy_creation`.
#[must_use]
pub fn get_gas_limit_estimation_for_hook(
    proxy_deployed: bool,
    extra_gas: Option<u64>,
    extra_gas_proxy_creation: Option<u64>,
) -> u64 {
    use super::sdk::DEFAULT_GAS_COST_FOR_HOOK_ESTIMATION;

    if proxy_deployed {
        DEFAULT_GAS_COST_FOR_HOOK_ESTIMATION + extra_gas.map_or(0, |v| v)
    } else {
        let base = DEFAULT_GAS_COST_FOR_HOOK_ESTIMATION + COW_SHED_PROXY_CREATION_GAS;
        base + extra_gas_proxy_creation.map_or(0, |v| v)
    }
}

// ── API validation helpers ────────────────────────────────────────────────────

/// Returns `true` if the HTTP status code indicates an infrastructure error
/// (5xx or 429 rate-limit).
#[must_use]
pub const fn is_infrastructure_error(status: u16) -> bool {
    status >= 500 || status == 429
}

/// Returns `true` if the error message is likely a network/fetch error.
///
/// Performs a case-sensitive substring check for `"fetch"`, `"network"`, or
/// `"dns"` — the typical keywords found in client-side connectivity errors.
///
/// # Arguments
///
/// * `error` — The error message string to inspect.
///
/// # Returns
///
/// `true` if the message contains any of the network-related keywords,
/// `false` otherwise.
#[must_use]
pub fn is_client_fetch_error(error: &str) -> bool {
    error.contains("fetch") || error.contains("network") || error.contains("dns")
}

// ── App-data validation ──────────────────────────────────────────────────────

/// Returns `true` if `json_str` looks like a valid app-data document.
///
/// Checks that the JSON object contains `version` and `metadata` fields,
/// mirroring the `TypeScript` `isAppDoc` helper.
#[must_use]
pub fn is_app_doc(json_str: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return false;
    };
    value.is_object() && value.get("version").is_some() && value.get("metadata").is_some()
}

// ── Provider resolution ──────────────────────────────────────────────────────

/// Resolve which providers to query from a list of dApp IDs.
///
/// When `dapp_ids` is `None`, all `available_dapp_ids` are returned.
/// Otherwise, each requested dApp ID is looked up in `available_dapp_ids`
/// and an error is returned if any ID is missing.
///
/// This mirrors the `TypeScript` `resolveProvidersToQuery` function, adapted
/// for Rust where provider objects live behind the bridge provider trait.
///
/// # Errors
///
/// Returns [`BridgeError::ProviderNotFound`] if a requested dApp ID is not
/// in the available set.
pub fn resolve_providers_to_query<'a>(
    dapp_ids: Option<&[String]>,
    available_dapp_ids: &'a [String],
) -> Result<Vec<&'a str>, BridgeError> {
    let Some(requested) = dapp_ids else {
        return Ok(available_dapp_ids.iter().map(String::as_str).collect());
    };

    requested
        .iter()
        .map(|id| {
            available_dapp_ids
                .iter()
                .find(|avail| avail.as_str() == id.as_str())
                .map(String::as_str)
                .ok_or_else(|| BridgeError::ProviderNotFound { dapp_id: id.clone() })
        })
        .collect()
}

// ── Alias ────────────────────────────────────────────────────────────────────

/// Alias for [`find_bridge_provider_dapp_id`] matching the `TypeScript` name
/// `findBridgeProviderFromHook`.
///
/// Extracts the bridge provider dApp ID from app-data JSON, either from
/// `metadata.bridging.providerId` or from scanning post-hooks.
pub use find_bridge_provider_dapp_id as find_bridge_provider_from_hook;

/// Alias for [`hook_mock_for_cost_estimation`] matching the `TypeScript` name
/// `getHookMockForCostEstimation`.
pub use hook_mock_for_cost_estimation as get_hook_mock_for_cost_estimation;

// ── Token adaptation ────────────────────────────────────────────────────────

use super::across::{across_token_mapping, get_token_address, get_token_symbol};

/// Adapt a token from one chain to another by matching its symbol in the Across
/// token mapping.
///
/// Given a token address on `source_chain`, looks up its symbol, then resolves
/// the corresponding address on `target_chain`. Returns `None` if either chain
/// is not configured or the token symbol has no mapping on the target chain.
///
/// # Example
///
/// ```
/// use alloy_primitives::Address;
/// use cow_rs::bridging::utils::adapt_token;
///
/// // USDC on Mainnet → USDC on Arbitrum
/// let mainnet_usdc: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
/// let result = adapt_token(mainnet_usdc, 1, 42161);
/// assert!(result.is_some());
/// ```
#[must_use]
pub fn adapt_token(
    token_address: Address,
    source_chain_id: u64,
    target_chain_id: u64,
) -> Option<Address> {
    let mapping = across_token_mapping();
    let source_config = mapping.get(&source_chain_id)?;
    let symbol = get_token_symbol(token_address, source_config)?;
    let target_config = mapping.get(&target_chain_id)?;
    get_token_address(&symbol, target_config)
}

/// Batch-adapt tokens from one chain to another using the Across token mapping.
///
/// For each token in `tokens`, attempts to find the equivalent address on
/// `target_chain_id`. Tokens that have no mapping are silently dropped.
///
/// # Example
///
/// ```
/// use alloy_primitives::Address;
/// use cow_rs::bridging::utils::adapt_tokens;
///
/// let mainnet_usdc: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
/// let results = adapt_tokens(&[mainnet_usdc], 1, 42161);
/// assert_eq!(results.len(), 1);
/// ```
#[must_use]
pub fn adapt_tokens(
    tokens: &[Address],
    source_chain_id: u64,
    target_chain_id: u64,
) -> Vec<Address> {
    tokens.iter().filter_map(|&addr| adapt_token(addr, source_chain_id, target_chain_id)).collect()
}
