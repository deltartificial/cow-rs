//! High-level executor configuration for `CoW` Protocol swap operations.
//!
//! Provides [`CowSwapConfig`] which bundles all the parameters needed to
//! submit orders (chain, environment, tokens, slippage, TTL) and
//! [`TokenRegistry`] which maps ticker symbols to on-chain addresses and
//! decimal counts.

use std::fmt;

use alloy_primitives::Address;
use foldhash::HashMap;

use super::chain::{Env, SupportedChainId};

/// Internal storage entry: `(token_address, decimals)`.
type TokenEntry = (Address, u8);

/// A registry mapping asset ticker symbols to their ERC-20 token metadata.
///
/// Stores both address and decimal precision so the executor can convert
/// human-readable quantities to token atoms without assuming 18 decimals.
///
/// # Example
///
/// ```
/// use alloy_primitives::Address;
/// use cow_chains::TokenRegistry;
///
/// let mut reg = TokenRegistry::new_with_decimals([
///     ("USDC", Address::ZERO, 6u8),
///     ("WETH", Address::ZERO, 18u8),
/// ]);
/// assert_eq!(reg.get_decimals("USDC"), Some(6));
/// assert!(reg.contains("WETH"));
/// assert!(!reg.contains("DAI"));
///
/// reg.insert("DAI", Address::ZERO);
/// assert!(reg.contains("DAI"));
/// assert_eq!(reg.len(), 3);
/// ```
#[derive(Debug)]
pub struct TokenRegistry {
    inner: HashMap<String, TokenEntry>,
}

impl TokenRegistry {
    /// Create a new registry from `(symbol, address)` pairs.
    ///
    /// All tokens registered this way are assumed to have **18 decimals**.
    /// Use [`new_with_decimals`](Self::new_with_decimals) when tokens have
    /// non-standard decimal counts (e.g. USDC = 6, WBTC = 8).
    ///
    /// # Parameters
    ///
    /// * `entries` — an iterator of `(symbol, address)` pairs.
    ///
    /// # Returns
    ///
    /// A new [`TokenRegistry`] with all entries set to 18 decimals.
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = (impl Into<String>, Address)>) -> Self {
        Self { inner: entries.into_iter().map(|(k, v)| (k.into(), (v, 18))).collect() }
    }

    /// Create a new registry from `(symbol, address, decimals)` tuples.
    ///
    /// Use this when tokens have non-standard decimal counts (e.g. USDC = 6,
    /// WBTC = 8).
    ///
    /// # Parameters
    ///
    /// * `entries` — an iterator of `(symbol, address, decimals)` tuples.
    ///
    /// # Returns
    ///
    /// A new [`TokenRegistry`] with explicit decimal counts per token.
    #[must_use]
    pub fn new_with_decimals(
        entries: impl IntoIterator<Item = (impl Into<String>, Address, u8)>,
    ) -> Self {
        Self { inner: entries.into_iter().map(|(k, v, d)| (k.into(), (v, d))).collect() }
    }

    /// Look up the [`Address`] for a given asset symbol, e.g. `"WETH"`.
    ///
    /// # Arguments
    ///
    /// * `asset` — the ticker symbol to look up.
    ///
    /// # Returns
    ///
    /// `Some(address)` if the symbol is registered, `None` otherwise.
    #[must_use]
    pub fn get(&self, asset: &str) -> Option<Address> {
        self.inner.get(asset).map(|&(addr, _)| addr)
    }

    /// Look up the decimal count for a given asset symbol.
    ///
    /// # Arguments
    ///
    /// * `asset` — the ticker symbol to look up.
    ///
    /// # Returns
    ///
    /// `Some(decimals)` when the symbol is registered, `None` otherwise.
    #[must_use]
    pub fn get_decimals(&self, asset: &str) -> Option<u8> {
        self.inner.get(asset).map(|&(_, decimals)| decimals)
    }

    /// Register a token with 18 decimals (or update an existing entry).
    ///
    /// # Arguments
    ///
    /// * `symbol` — the ticker symbol to register.
    /// * `address` — the ERC-20 contract [`Address`].
    pub fn insert(&mut self, symbol: impl Into<String>, address: Address) {
        self.inner.insert(symbol.into(), (address, 18));
    }

    /// Register a token with explicit decimals (or update an existing entry).
    ///
    /// # Arguments
    ///
    /// * `symbol` — the ticker symbol to register.
    /// * `address` — the ERC-20 contract [`Address`].
    /// * `decimals` — the token's decimal precision.
    pub fn insert_with_decimals(
        &mut self,
        symbol: impl Into<String>,
        address: Address,
        decimals: u8,
    ) {
        self.inner.insert(symbol.into(), (address, decimals));
    }

    /// Returns `true` if `asset` is registered in this registry.
    ///
    /// # Arguments
    ///
    /// * `asset` — the ticker symbol to check.
    ///
    /// # Returns
    ///
    /// `true` when the symbol exists in the registry.
    #[must_use]
    pub fn contains(&self, asset: &str) -> bool {
        self.inner.contains_key(asset)
    }

    /// Returns the number of registered tokens.
    ///
    /// # Returns
    ///
    /// The count of tokens in this registry.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if no tokens are registered.
    ///
    /// # Returns
    ///
    /// `true` when the registry contains zero tokens.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Look up both the address and decimal count for a given asset symbol.
    ///
    /// Returns `Some((address, decimals))` when registered, `None` otherwise.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_chains::TokenRegistry;
    ///
    /// let reg = TokenRegistry::new_with_decimals([("USDC", Address::ZERO, 6u8)]);
    /// assert_eq!(reg.get_entry("USDC"), Some((Address::ZERO, 6)));
    /// assert_eq!(reg.get_entry("WETH"), None);
    /// ```
    #[must_use]
    pub fn get_entry(&self, asset: &str) -> Option<(Address, u8)> {
        self.inner.get(asset).copied()
    }

    /// Remove a token from the registry.
    ///
    /// # Arguments
    ///
    /// * `asset` — the ticker symbol to remove.
    ///
    /// # Returns
    ///
    /// `Some((address, decimals))` if the symbol was registered, `None` otherwise.
    pub fn remove(&mut self, asset: &str) -> Option<(Address, u8)> {
        self.inner.remove(asset)
    }
}

impl fmt::Display for TokenRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "registry({} tokens)", self.inner.len())
    }
}

/// Configuration for the `CoW` Protocol swap executor.
///
/// Bundles all parameters needed to submit orders: target chain,
/// environment, sell token, slippage, TTL, and a [`TokenRegistry`] mapping
/// strategy symbols to on-chain addresses.
///
/// Construct via [`prod`](Self::prod) or [`staging`](Self::staging), then
/// customise with the `with_*` builder methods.
///
/// # Example
///
/// ```
/// use alloy_primitives::Address;
/// use cow_chains::{CowSwapConfig, SupportedChainId, TokenRegistry};
///
/// let empty: Vec<(&str, Address)> = vec![];
/// let config = CowSwapConfig::prod(
///     SupportedChainId::Mainnet,
///     Address::ZERO, // sell token
///     TokenRegistry::new(empty),
///     50,   // 0.5% slippage
///     1800, // 30 min TTL
/// );
/// assert!(config.env.is_prod());
/// assert_eq!(config.slippage_bps, 50);
/// ```
#[derive(Debug)]
pub struct CowSwapConfig {
    /// Target chain.
    pub chain_id: SupportedChainId,
    /// API environment (`Prod` or `Staging`).
    pub env: Env,
    /// The token used as the quote / sell currency (e.g. USDC on Sepolia).
    pub sell_token: Address,
    /// Decimal count for [`Self::sell_token`] (e.g. `6` for USDC, `18` for WETH).
    pub sell_token_decimals: u8,
    /// Registry mapping strategy asset symbols to their on-chain token addresses
    /// and decimal counts.
    pub tokens: TokenRegistry,
    /// Slippage tolerance in basis points (e.g. `50` = 0.5 %).
    pub slippage_bps: u32,
    /// Default order TTL in seconds (e.g. `1800` = 30 min).
    pub order_valid_secs: u32,
    /// Optional override for the buy-token receiver address.
    ///
    /// When `None` the order receiver defaults to the signing wallet address.
    pub receiver: Option<Address>,
}

impl CowSwapConfig {
    /// Convenience constructor defaulting to the production environment.
    ///
    /// [`Self::sell_token_decimals`] defaults to `18`; use
    /// [`with_sell_token_decimals`](Self::with_sell_token_decimals) for
    /// tokens such as USDC (`6`) or WBTC (`8`).
    ///
    /// # Parameters
    ///
    /// * `chain_id` — the target [`SupportedChainId`].
    /// * `sell_token` — the ERC-20 [`Address`] of the sell (quote) currency.
    /// * `tokens` — the [`TokenRegistry`] mapping strategy symbols to tokens.
    /// * `slippage_bps` — slippage tolerance in basis points (e.g. `50` = 0.5 %).
    /// * `order_valid_secs` — order TTL in seconds (e.g. `1800` = 30 min).
    ///
    /// # Returns
    ///
    /// A new [`CowSwapConfig`] targeting [`Env::Prod`] with no custom receiver.
    #[must_use]
    pub const fn prod(
        chain_id: SupportedChainId,
        sell_token: Address,
        tokens: TokenRegistry,
        slippage_bps: u32,
        order_valid_secs: u32,
    ) -> Self {
        Self {
            chain_id,
            env: Env::Prod,
            sell_token,
            sell_token_decimals: 18,
            tokens,
            slippage_bps,
            order_valid_secs,
            receiver: None,
        }
    }

    /// Convenience constructor defaulting to the staging (barn) environment.
    ///
    /// Same parameters as [`prod`](Self::prod) but targets [`Env::Staging`]
    /// (`barn.api.cow.fi`). [`Self::sell_token_decimals`] defaults to `18`.
    ///
    /// # Parameters
    ///
    /// See [`prod`](Self::prod) for parameter descriptions.
    ///
    /// # Returns
    ///
    /// A new [`CowSwapConfig`] targeting [`Env::Staging`].
    #[must_use]
    pub const fn staging(
        chain_id: SupportedChainId,
        sell_token: Address,
        tokens: TokenRegistry,
        slippage_bps: u32,
        order_valid_secs: u32,
    ) -> Self {
        Self {
            chain_id,
            env: Env::Staging,
            sell_token,
            sell_token_decimals: 18,
            tokens,
            slippage_bps,
            order_valid_secs,
            receiver: None,
        }
    }

    /// Override the sell token address.
    ///
    /// # Arguments
    ///
    /// * `token` — the new sell token [`Address`].
    ///
    /// # Returns
    ///
    /// `self` with the updated sell token.
    #[must_use]
    pub const fn with_sell_token(mut self, token: Address) -> Self {
        self.sell_token = token;
        self
    }

    /// Override the chain ID.
    ///
    /// # Arguments
    ///
    /// * `chain_id` — the new target [`SupportedChainId`].
    ///
    /// # Returns
    ///
    /// `self` with the updated chain ID.
    #[must_use]
    pub const fn with_chain_id(mut self, chain_id: SupportedChainId) -> Self {
        self.chain_id = chain_id;
        self
    }

    /// Override the API environment (`Prod` or `Staging`).
    ///
    /// # Arguments
    ///
    /// * `env` — the new [`Env`] value.
    ///
    /// # Returns
    ///
    /// `self` with the updated environment.
    #[must_use]
    pub const fn with_env(mut self, env: Env) -> Self {
        self.env = env;
        self
    }

    /// Override the slippage tolerance in basis points.
    ///
    /// # Arguments
    ///
    /// * `slippage_bps` — the new slippage in basis points (e.g. `50` = 0.5 %).
    ///
    /// # Returns
    ///
    /// `self` with the updated slippage.
    #[must_use]
    pub const fn with_slippage_bps(mut self, slippage_bps: u32) -> Self {
        self.slippage_bps = slippage_bps;
        self
    }

    /// Override the default order TTL in seconds.
    ///
    /// # Arguments
    ///
    /// * `secs` — the new TTL in seconds (e.g. `1800` = 30 min).
    ///
    /// # Returns
    ///
    /// `self` with the updated order TTL.
    #[must_use]
    pub const fn with_order_valid_secs(mut self, secs: u32) -> Self {
        self.order_valid_secs = secs;
        self
    }

    /// Override the decimal count for `sell_token` (defaults to `18`).
    ///
    /// # Arguments
    ///
    /// * `decimals` — the decimal precision of the sell token.
    ///
    /// # Returns
    ///
    /// `self` with the updated decimal count.
    #[must_use]
    pub const fn with_sell_token_decimals(mut self, decimals: u8) -> Self {
        self.sell_token_decimals = decimals;
        self
    }

    /// Override the order receiver address.
    ///
    /// # Arguments
    ///
    /// * `receiver` — the custom receiver [`Address`].
    ///
    /// # Returns
    ///
    /// `self` with the receiver override set.
    #[must_use]
    pub const fn with_receiver(mut self, receiver: Address) -> Self {
        self.receiver = Some(receiver);
        self
    }

    /// Returns `true` if a custom receiver address has been set.
    ///
    /// When `false`, the executor uses the signing wallet address as receiver.
    ///
    /// # Returns
    ///
    /// `true` when a receiver override is present.
    #[must_use]
    pub const fn has_custom_receiver(&self) -> bool {
        self.receiver.is_some()
    }

    /// Return the effective receiver: the override if set, otherwise `default`.
    ///
    /// # Example
    ///
    /// ```
    /// use alloy_primitives::{Address, address};
    /// use cow_chains::{CowSwapConfig, SupportedChainId, TokenRegistry};
    ///
    /// let wallet = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
    /// let empty: Vec<(&str, Address)> = vec![];
    /// let config = CowSwapConfig::prod(
    ///     SupportedChainId::Mainnet,
    ///     Address::ZERO,
    ///     TokenRegistry::new(empty),
    ///     50,
    ///     1800,
    /// );
    /// assert_eq!(config.effective_receiver(wallet), wallet);
    ///
    /// let override_addr = address!("0000000000000000000000000000000000000001");
    /// let with_recv = config.with_receiver(override_addr);
    /// assert_eq!(with_recv.effective_receiver(wallet), override_addr);
    /// ```
    #[must_use]
    pub fn effective_receiver(&self, default: Address) -> Address {
        self.receiver.map_or(default, |r| r)
    }
}

impl fmt::Display for CowSwapConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "config({}, {}, sell={:#x})", self.chain_id, self.env, self.sell_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── TokenRegistry ───────────────────────────────────────────────────

    #[test]
    fn token_registry_new_defaults_to_18_decimals() {
        let reg = TokenRegistry::new([("WETH", Address::ZERO)]);
        assert_eq!(reg.get_decimals("WETH"), Some(18));
    }

    #[test]
    fn token_registry_new_with_decimals() {
        let reg = TokenRegistry::new_with_decimals([("USDC", Address::ZERO, 6u8)]);
        assert_eq!(reg.get_decimals("USDC"), Some(6));
        assert_eq!(reg.get("USDC"), Some(Address::ZERO));
    }

    #[test]
    fn token_registry_insert_and_contains() {
        let mut reg = TokenRegistry::new(std::iter::empty::<(&str, Address)>());
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(!reg.contains("DAI"));

        reg.insert("DAI", Address::ZERO);
        assert!(reg.contains("DAI"));
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
    }

    #[test]
    fn token_registry_insert_with_decimals() {
        let mut reg = TokenRegistry::new(std::iter::empty::<(&str, Address)>());
        reg.insert_with_decimals("WBTC", Address::ZERO, 8);
        assert_eq!(reg.get_decimals("WBTC"), Some(8));
    }

    #[test]
    fn token_registry_get_entry() {
        let reg = TokenRegistry::new_with_decimals([("USDC", Address::ZERO, 6u8)]);
        assert_eq!(reg.get_entry("USDC"), Some((Address::ZERO, 6)));
        assert_eq!(reg.get_entry("NONEXISTENT"), None);
    }

    #[test]
    fn token_registry_remove() {
        let mut reg = TokenRegistry::new([("WETH", Address::ZERO)]);
        assert!(reg.contains("WETH"));
        let removed = reg.remove("WETH");
        assert!(removed.is_some());
        assert!(!reg.contains("WETH"));
        assert!(reg.is_empty());
    }

    #[test]
    fn token_registry_remove_nonexistent() {
        let mut reg = TokenRegistry::new(std::iter::empty::<(&str, Address)>());
        assert!(reg.remove("WETH").is_none());
    }

    #[test]
    fn token_registry_get_missing_returns_none() {
        let reg = TokenRegistry::new(std::iter::empty::<(&str, Address)>());
        assert_eq!(reg.get("WETH"), None);
        assert_eq!(reg.get_decimals("WETH"), None);
    }

    #[test]
    fn token_registry_display() {
        let reg = TokenRegistry::new([("A", Address::ZERO), ("B", Address::ZERO)]);
        let s = format!("{reg}");
        assert!(s.contains("2 tokens"));
    }

    // ── CowSwapConfig ───────────────────────────────────────────────────

    fn empty_registry() -> TokenRegistry {
        TokenRegistry::new(std::iter::empty::<(&str, Address)>())
    }

    #[test]
    fn config_prod_defaults() {
        let cfg = CowSwapConfig::prod(
            SupportedChainId::Mainnet,
            Address::ZERO,
            empty_registry(),
            50,
            1800,
        );
        assert!(cfg.env.is_prod());
        assert_eq!(cfg.slippage_bps, 50);
        assert_eq!(cfg.order_valid_secs, 1800);
        assert_eq!(cfg.sell_token_decimals, 18);
        assert!(!cfg.has_custom_receiver());
    }

    #[test]
    fn config_staging_defaults() {
        let cfg = CowSwapConfig::staging(
            SupportedChainId::Sepolia,
            Address::ZERO,
            empty_registry(),
            100,
            900,
        );
        assert!(cfg.env.is_staging());
        assert_eq!(cfg.slippage_bps, 100);
    }

    #[test]
    fn config_builder_methods() {
        let cfg = CowSwapConfig::prod(
            SupportedChainId::Mainnet,
            Address::ZERO,
            empty_registry(),
            50,
            1800,
        )
        .with_slippage_bps(100)
        .with_order_valid_secs(600)
        .with_sell_token_decimals(6)
        .with_chain_id(SupportedChainId::Sepolia)
        .with_env(Env::Staging);

        assert_eq!(cfg.slippage_bps, 100);
        assert_eq!(cfg.order_valid_secs, 600);
        assert_eq!(cfg.sell_token_decimals, 6);
        assert_eq!(cfg.chain_id, SupportedChainId::Sepolia);
        assert!(cfg.env.is_staging());
    }

    #[test]
    fn config_with_receiver() {
        let recv = Address::new([0x01; 20]);
        let wallet = Address::new([0x02; 20]);
        let cfg = CowSwapConfig::prod(
            SupportedChainId::Mainnet,
            Address::ZERO,
            empty_registry(),
            50,
            1800,
        )
        .with_receiver(recv);
        assert!(cfg.has_custom_receiver());
        assert_eq!(cfg.effective_receiver(wallet), recv);
    }

    #[test]
    fn config_effective_receiver_defaults_to_wallet() {
        let wallet = Address::new([0x02; 20]);
        let cfg = CowSwapConfig::prod(
            SupportedChainId::Mainnet,
            Address::ZERO,
            empty_registry(),
            50,
            1800,
        );
        assert_eq!(cfg.effective_receiver(wallet), wallet);
    }

    #[test]
    fn config_with_sell_token() {
        let token = Address::new([0xaa; 20]);
        let cfg = CowSwapConfig::prod(
            SupportedChainId::Mainnet,
            Address::ZERO,
            empty_registry(),
            50,
            1800,
        )
        .with_sell_token(token);
        assert_eq!(cfg.sell_token, token);
    }

    #[test]
    fn config_display() {
        let cfg = CowSwapConfig::prod(
            SupportedChainId::Mainnet,
            Address::ZERO,
            empty_registry(),
            50,
            1800,
        );
        let s = format!("{cfg}");
        assert!(s.contains("config("));
    }
}
