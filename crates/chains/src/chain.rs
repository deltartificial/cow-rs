//! Supported chain IDs, API base URLs, and explorer link helpers.
//!
//! This module defines the [`SupportedChainId`] enum (one variant per chain
//! that the `CoW` Protocol orderbook supports), the [`Env`] enum (production
//! vs. staging), and helpers to build API and explorer URLs.
//!
//! # Key items
//!
//! | Item | Purpose |
//! |---|---|
//! | [`SupportedChainId`] | Enum of all chains with EIP-155 discriminants |
//! | [`Env`] | `Prod` / `Staging` orderbook environment |
//! | [`api_base_url`] | Build the orderbook API URL for a chain + env |
//! | [`order_explorer_link`] | Build a `CoW` Protocol Explorer URL for an order |

use serde::{Deserialize, Serialize};

/// Chains supported by the `CoW` Protocol orderbook.
///
/// Each variant's numeric discriminant matches the EIP-155 chain ID, so
/// `SupportedChainId::Mainnet as u64 == 1`. Use [`try_from_u64`](Self::try_from_u64)
/// to convert from a raw chain ID, or [`all`](Self::all) to iterate every
/// supported chain.
///
/// # Example
///
/// ```
/// use cow_chains::SupportedChainId;
///
/// let chain = SupportedChainId::try_from_u64(1).unwrap();
/// assert_eq!(chain, SupportedChainId::Mainnet);
/// assert_eq!(chain.as_u64(), 1);
/// assert_eq!(chain.as_str(), "mainnet");
/// assert!(!chain.is_testnet());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u64)]
pub enum SupportedChainId {
    /// Ethereum mainnet (chain ID 1).
    Mainnet = 1,
    /// Gnosis Chain (chain ID 100).
    GnosisChain = 100,
    /// Arbitrum One (chain ID 42161).
    ArbitrumOne = 42_161,
    /// Base (chain ID 8453).
    Base = 8_453,
    /// Ethereum Sepolia testnet (chain ID 11155111).
    Sepolia = 11_155_111,
    /// Polygon `PoS` (chain ID 137).
    Polygon = 137,
    /// Avalanche C-Chain (chain ID 43114).
    Avalanche = 43_114,
    /// BNB Smart Chain (chain ID 56).
    BnbChain = 56,
    /// Linea (chain ID 59144).
    Linea = 59_144,
    /// Lens Network (chain ID 232).
    Lens = 232,
    /// Plasma (chain ID 9745).
    Plasma = 9_745,
    /// Ink (chain ID 57073).
    Ink = 57_073,
}

impl SupportedChainId {
    /// Return the numeric EIP-155 chain ID.
    ///
    /// # Returns
    ///
    /// The `u64` chain ID (e.g. `1` for Mainnet, `100` for Gnosis Chain).
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self as u64
    }

    /// Try to construct a [`SupportedChainId`] from a raw EIP-155 chain ID.
    ///
    /// # Parameters
    ///
    /// * `chain_id` — the numeric EIP-155 chain ID.
    ///
    /// # Returns
    ///
    /// `Some(variant)` if `chain_id` is supported, `None` otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_chains::SupportedChainId;
    ///
    /// assert_eq!(SupportedChainId::try_from_u64(1), Some(SupportedChainId::Mainnet));
    /// assert_eq!(SupportedChainId::try_from_u64(11155111), Some(SupportedChainId::Sepolia));
    /// assert_eq!(SupportedChainId::try_from_u64(9999), None);
    /// ```
    #[must_use]
    pub const fn try_from_u64(chain_id: u64) -> Option<Self> {
        match chain_id {
            1 => Some(Self::Mainnet),
            100 => Some(Self::GnosisChain),
            42_161 => Some(Self::ArbitrumOne),
            8_453 => Some(Self::Base),
            11_155_111 => Some(Self::Sepolia),
            137 => Some(Self::Polygon),
            43_114 => Some(Self::Avalanche),
            56 => Some(Self::BnbChain),
            59_144 => Some(Self::Linea),
            232 => Some(Self::Lens),
            9_745 => Some(Self::Plasma),
            57_073 => Some(Self::Ink),
            _ => None,
        }
    }

    /// Return a slice of all chains supported by the `CoW` Protocol orderbook.
    ///
    /// # Returns
    ///
    /// A static slice containing every [`SupportedChainId`] variant.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Mainnet,
            Self::GnosisChain,
            Self::ArbitrumOne,
            Self::Base,
            Self::Sepolia,
            Self::Polygon,
            Self::Avalanche,
            Self::BnbChain,
            Self::Linea,
            Self::Lens,
            Self::Plasma,
            Self::Ink,
        ]
    }

    /// Returns the `CoW` Protocol API path segment for this chain.
    ///
    /// Matches the path used in [`api_base_url`], e.g. `"mainnet"`, `"xdai"`,
    /// `"sepolia"`. Useful for constructing API URLs manually.
    ///
    /// # Returns
    ///
    /// A static string suitable for use in API URL paths.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::GnosisChain => "xdai",
            Self::ArbitrumOne => "arbitrum_one",
            Self::Base => "base",
            Self::Sepolia => "sepolia",
            Self::Polygon => "polygon",
            Self::Avalanche => "avalanche",
            Self::BnbChain => "bnb",
            Self::Linea => "linea",
            Self::Lens => "lens",
            Self::Plasma => "plasma",
            Self::Ink => "ink",
        }
    }

    /// Whether this chain is a testnet.
    ///
    /// # Returns
    ///
    /// `true` for [`Sepolia`](Self::Sepolia), `false` for all other chains.
    #[must_use]
    pub const fn is_testnet(self) -> bool {
        matches!(self, Self::Sepolia)
    }

    /// Returns `true` for production chains (i.e. not a testnet).
    ///
    /// This is the logical complement of [`Self::is_testnet`].
    ///
    /// ```
    /// use cow_chains::SupportedChainId;
    /// assert!(SupportedChainId::Mainnet.is_mainnet());
    /// assert!(!SupportedChainId::Sepolia.is_mainnet());
    /// ```
    #[must_use]
    pub const fn is_mainnet(self) -> bool {
        !self.is_testnet()
    }

    /// Returns `true` for layer-2 networks.
    ///
    /// Currently includes Arbitrum One, Base, Linea, Ink, and Polygon.
    ///
    /// ```
    /// use cow_chains::SupportedChainId;
    ///
    /// assert!(SupportedChainId::ArbitrumOne.is_layer2());
    /// assert!(SupportedChainId::Base.is_layer2());
    /// assert!(SupportedChainId::Polygon.is_layer2());
    /// assert!(!SupportedChainId::Mainnet.is_layer2());
    /// assert!(!SupportedChainId::GnosisChain.is_layer2());
    /// ```
    #[must_use]
    pub const fn is_layer2(self) -> bool {
        matches!(self, Self::ArbitrumOne | Self::Base | Self::Linea | Self::Ink | Self::Polygon)
    }

    /// Return the network segment used in `CoW` Protocol Explorer URLs.
    ///
    /// Mainnet uses an empty segment (orders live at the root path).
    ///
    /// # Returns
    ///
    /// A static string for the URL path segment (empty for Mainnet).
    #[must_use]
    pub const fn explorer_network(self) -> &'static str {
        match self {
            Self::Mainnet => "",
            Self::GnosisChain => "gc",
            Self::ArbitrumOne => "arb1",
            Self::Base => "base",
            Self::Sepolia => "sepolia",
            Self::Polygon => "polygon",
            Self::Avalanche => "avalanche",
            Self::BnbChain => "bnb",
            Self::Linea => "linea",
            Self::Lens => "lens",
            Self::Plasma => "plasma",
            Self::Ink => "ink",
        }
    }
}

/// Build a `CoW` Protocol Explorer link for an order.
///
/// Returns a URL pointing to `https://explorer.cow.fi/{network}/orders/{uid}`.
/// Mainnet orders omit the network prefix (orders live at the root path).
///
/// # Parameters
///
/// * `chain` — the [`SupportedChainId`] the order was placed on.
/// * `order_uid` — the order UID string (typically `0x`-prefixed hex).
///
/// # Returns
///
/// A `String` URL pointing to the order on the `CoW` Protocol Explorer.
///
/// # Example
///
/// ```
/// use cow_chains::{SupportedChainId, order_explorer_link};
///
/// let url = order_explorer_link(SupportedChainId::Mainnet, "0xabc123...");
/// assert!(url.starts_with("https://explorer.cow.fi/orders/"));
///
/// let url_sep = order_explorer_link(SupportedChainId::Sepolia, "0xabc123...");
/// assert!(url_sep.starts_with("https://explorer.cow.fi/sepolia/orders/"));
/// ```
#[must_use]
pub fn order_explorer_link(chain: SupportedChainId, order_uid: &str) -> String {
    let net = chain.explorer_network();
    if net.is_empty() {
        format!("https://explorer.cow.fi/orders/{order_uid}")
    } else {
        format!("https://explorer.cow.fi/{net}/orders/{order_uid}")
    }
}

impl std::fmt::Display for SupportedChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Mainnet => "Ethereum",
            Self::GnosisChain => "Gnosis Chain",
            Self::ArbitrumOne => "Arbitrum One",
            Self::Base => "Base",
            Self::Sepolia => "Sepolia",
            Self::Polygon => "Polygon",
            Self::Avalanche => "Avalanche",
            Self::BnbChain => "BNB Smart Chain",
            Self::Linea => "Linea",
            Self::Lens => "Lens",
            Self::Plasma => "Plasma",
            Self::Ink => "Ink",
        };
        f.write_str(name)
    }
}

impl From<SupportedChainId> for u64 {
    fn from(id: SupportedChainId) -> Self {
        id.as_u64()
    }
}

impl TryFrom<u64> for SupportedChainId {
    type Error = u64;

    fn try_from(chain_id: u64) -> Result<Self, Self::Error> {
        Self::try_from_u64(chain_id).ok_or(chain_id)
    }
}

impl TryFrom<&str> for SupportedChainId {
    type Error = cow_errors::CowError;

    /// Parse a [`SupportedChainId`] from the `CoW` Protocol API path segment.
    ///
    /// Accepts the same strings returned by [`SupportedChainId::as_str`]:
    /// `"mainnet"`, `"xdai"`, `"arbitrum_one"`, `"base"`, `"sepolia"`,
    /// `"polygon"`, `"avalanche"`, `"bnb"`, `"linea"`, `"lens"`, `"plasma"`, `"ink"`.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "xdai" => Ok(Self::GnosisChain),
            "arbitrum_one" => Ok(Self::ArbitrumOne),
            "base" => Ok(Self::Base),
            "sepolia" => Ok(Self::Sepolia),
            "polygon" => Ok(Self::Polygon),
            "avalanche" => Ok(Self::Avalanche),
            "bnb" => Ok(Self::BnbChain),
            "linea" => Ok(Self::Linea),
            "lens" => Ok(Self::Lens),
            "plasma" => Ok(Self::Plasma),
            "ink" => Ok(Self::Ink),
            other => Err(cow_errors::CowError::Parse {
                field: "SupportedChainId",
                reason: format!("unknown chain: {other}"),
            }),
        }
    }
}

/// Orderbook API environment.
///
/// The `CoW` Protocol runs two parallel orderbooks:
///
/// - **Prod** (`api.cow.fi`) — the production orderbook used for real trades.
/// - **Staging** (`barn.api.cow.fi`) — the "barn" environment used for testing with real tokens but
///   lower liquidity.
///
/// # Example
///
/// ```
/// use cow_chains::Env;
///
/// let env = Env::Prod;
/// assert!(env.is_prod());
/// assert_eq!(env.as_str(), "prod");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Env {
    /// Production orderbook at `api.cow.fi`.
    #[default]
    Prod,
    /// Staging (barn) orderbook at `barn.api.cow.fi`.
    Staging,
}

impl Env {
    /// Returns the string label for this environment.
    ///
    /// # Returns
    ///
    /// `"prod"` or `"staging"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prod => "prod",
            Self::Staging => "staging",
        }
    }

    /// Returns all supported environments.
    ///
    /// ```
    /// use cow_chains::Env;
    /// assert_eq!(Env::all().len(), 2);
    /// ```
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Prod, Self::Staging]
    }

    /// Returns `true` if this is the production environment.
    ///
    /// # Returns
    ///
    /// `true` for [`Env::Prod`].
    #[must_use]
    pub const fn is_prod(self) -> bool {
        matches!(self, Self::Prod)
    }

    /// Returns `true` if this is the staging (barn) environment.
    ///
    /// # Returns
    ///
    /// `true` for [`Env::Staging`].
    #[must_use]
    pub const fn is_staging(self) -> bool {
        matches!(self, Self::Staging)
    }
}

impl std::fmt::Display for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for Env {
    type Error = cow_errors::CowError;

    /// Parse an [`Env`] from its string label.
    ///
    /// Accepts `"prod"` or `"staging"`.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "prod" => Ok(Self::Prod),
            "staging" => Ok(Self::Staging),
            other => Err(cow_errors::CowError::Parse {
                field: "Env",
                reason: format!("unknown env: {other}"),
            }),
        }
    }
}

/// Return the orderbook API base URL for a chain and environment.
///
/// This is an alias for [`api_base_url`] matching the `apiUrl` name from
/// the `TypeScript` `contracts-ts` package.
///
/// # Parameters
///
/// * `chain` — the target [`SupportedChainId`].
/// * `env` — the [`Env`] (production or staging).
///
/// # Returns
///
/// A static string like `"https://api.cow.fi/mainnet"`.
#[must_use]
pub const fn api_url(chain: SupportedChainId, env: Env) -> &'static str {
    api_base_url(chain, env)
}

/// Return the orderbook API base URL (no trailing slash) for `chain` in
/// `env`.
///
/// Append `/api/v1/<endpoint>` to form a full request URL. For example,
/// `api_base_url(Mainnet, Prod)` returns `"https://api.cow.fi/mainnet"`,
/// so a quote request would go to
/// `"https://api.cow.fi/mainnet/api/v1/quote"`.
///
/// # Parameters
///
/// * `chain` — the target [`SupportedChainId`].
/// * `env` — the [`Env`] (production or staging).
///
/// # Returns
///
/// A `&'static str` base URL.
///
/// # Example
///
/// ```
/// use cow_chains::{Env, SupportedChainId, api_base_url};
///
/// let url = api_base_url(SupportedChainId::Mainnet, Env::Prod);
/// assert_eq!(url, "https://api.cow.fi/mainnet");
///
/// let barn = api_base_url(SupportedChainId::Sepolia, Env::Staging);
/// assert!(barn.contains("barn.api.cow.fi"));
/// ```
#[must_use]
pub const fn api_base_url(chain: SupportedChainId, env: Env) -> &'static str {
    match (chain, env) {
        (SupportedChainId::Mainnet, Env::Prod) => "https://api.cow.fi/mainnet",
        (SupportedChainId::GnosisChain, Env::Prod) => "https://api.cow.fi/xdai",
        (SupportedChainId::ArbitrumOne, Env::Prod) => "https://api.cow.fi/arbitrum_one",
        (SupportedChainId::Base, Env::Prod) => "https://api.cow.fi/base",
        (SupportedChainId::Sepolia, Env::Prod) => "https://api.cow.fi/sepolia",
        (SupportedChainId::Polygon, Env::Prod) => "https://api.cow.fi/polygon",
        (SupportedChainId::Avalanche, Env::Prod) => "https://api.cow.fi/avalanche",
        (SupportedChainId::BnbChain, Env::Prod) => "https://api.cow.fi/bnb",
        (SupportedChainId::Linea, Env::Prod) => "https://api.cow.fi/linea",
        (SupportedChainId::Lens, Env::Prod) => "https://api.cow.fi/lens",
        (SupportedChainId::Plasma, Env::Prod) => "https://api.cow.fi/plasma",
        (SupportedChainId::Ink, Env::Prod) => "https://api.cow.fi/ink",
        (SupportedChainId::Mainnet, Env::Staging) => "https://barn.api.cow.fi/mainnet",
        (SupportedChainId::GnosisChain, Env::Staging) => "https://barn.api.cow.fi/xdai",
        (SupportedChainId::ArbitrumOne, Env::Staging) => "https://barn.api.cow.fi/arbitrum_one",
        (SupportedChainId::Base, Env::Staging) => "https://barn.api.cow.fi/base",
        (SupportedChainId::Sepolia, Env::Staging) => "https://barn.api.cow.fi/sepolia",
        (SupportedChainId::Polygon, Env::Staging) => "https://barn.api.cow.fi/polygon",
        (SupportedChainId::Avalanche, Env::Staging) => "https://barn.api.cow.fi/avalanche",
        (SupportedChainId::BnbChain, Env::Staging) => "https://barn.api.cow.fi/bnb",
        (SupportedChainId::Linea, Env::Staging) => "https://barn.api.cow.fi/linea",
        (SupportedChainId::Lens, Env::Staging) => "https://barn.api.cow.fi/lens",
        (SupportedChainId::Plasma, Env::Staging) => "https://barn.api.cow.fi/plasma",
        (SupportedChainId::Ink, Env::Staging) => "https://barn.api.cow.fi/ink",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SupportedChainId ────────────────────────────────────────────────

    #[test]
    fn all_chains_roundtrip_u64() {
        for &chain in SupportedChainId::all() {
            let id = chain.as_u64();
            assert_eq!(SupportedChainId::try_from_u64(id), Some(chain));
            assert_eq!(u64::from(chain), id);
            assert!(matches!(SupportedChainId::try_from(id), Ok(c) if c == chain));
        }
    }

    #[test]
    fn all_chains_roundtrip_str() {
        for &chain in SupportedChainId::all() {
            let s = chain.as_str();
            assert!(!s.is_empty());
            assert!(matches!(SupportedChainId::try_from(s), Ok(c) if c == chain));
        }
    }

    #[test]
    fn all_chains_have_display() {
        for &chain in SupportedChainId::all() {
            let display = format!("{chain}");
            assert!(!display.is_empty());
        }
    }

    #[test]
    fn all_chains_have_explorer_network() {
        for &chain in SupportedChainId::all() {
            // Mainnet is empty, all others non-empty
            let net = chain.explorer_network();
            if chain == SupportedChainId::Mainnet {
                assert!(net.is_empty());
            } else {
                assert!(!net.is_empty());
            }
        }
    }

    #[test]
    fn unknown_chain_id_returns_none() {
        assert_eq!(SupportedChainId::try_from_u64(9999), None);
        assert!(SupportedChainId::try_from(0u64).is_err());
    }

    #[test]
    fn unknown_chain_str_returns_err() {
        assert!(SupportedChainId::try_from("unknown").is_err());
    }

    #[test]
    fn only_sepolia_is_testnet() {
        for &chain in SupportedChainId::all() {
            if chain == SupportedChainId::Sepolia {
                assert!(chain.is_testnet());
                assert!(!chain.is_mainnet());
            } else {
                assert!(!chain.is_testnet());
                assert!(chain.is_mainnet());
            }
        }
    }

    #[test]
    fn layer2_chains() {
        let l2s = [
            SupportedChainId::ArbitrumOne,
            SupportedChainId::Base,
            SupportedChainId::Linea,
            SupportedChainId::Ink,
            SupportedChainId::Polygon,
        ];
        for &chain in &l2s {
            assert!(chain.is_layer2(), "{chain:?} should be L2");
        }
        assert!(!SupportedChainId::Mainnet.is_layer2());
        assert!(!SupportedChainId::GnosisChain.is_layer2());
    }

    #[test]
    fn all_contains_every_variant() {
        assert_eq!(SupportedChainId::all().len(), 12);
    }

    // ── Env ─────────────────────────────────────────────────────────────

    #[test]
    fn env_default_is_prod() {
        assert_eq!(Env::default(), Env::Prod);
    }

    #[test]
    fn env_predicates() {
        assert!(Env::Prod.is_prod());
        assert!(!Env::Prod.is_staging());
        assert!(Env::Staging.is_staging());
        assert!(!Env::Staging.is_prod());
    }

    #[test]
    fn env_roundtrip_str() {
        for &env in Env::all() {
            assert!(matches!(Env::try_from(env.as_str()), Ok(e) if e == env));
        }
    }

    #[test]
    fn env_all_has_two() {
        assert_eq!(Env::all().len(), 2);
    }

    #[test]
    fn env_display() {
        assert_eq!(format!("{}", Env::Prod), "prod");
        assert_eq!(format!("{}", Env::Staging), "staging");
    }

    #[test]
    fn env_invalid_str() {
        assert!(Env::try_from("production").is_err());
    }

    // ── API URLs ────────────────────────────────────────────────────────

    #[test]
    fn api_base_url_all_chains_all_envs() {
        for &chain in SupportedChainId::all() {
            for &env in Env::all() {
                let url = api_base_url(chain, env);
                assert!(url.starts_with("https://"));
                assert!(url.contains("cow.fi"));
                if env.is_staging() {
                    assert!(url.contains("barn."), "{url} should contain barn for staging");
                }
            }
        }
    }

    #[test]
    fn api_url_is_alias_for_api_base_url() {
        let chain = SupportedChainId::Mainnet;
        assert_eq!(api_url(chain, Env::Prod), api_base_url(chain, Env::Prod));
    }

    // ── Explorer links ──────────────────────────────────────────────────

    #[test]
    fn explorer_link_mainnet_no_network_prefix() {
        let url = order_explorer_link(SupportedChainId::Mainnet, "0xabc");
        assert_eq!(url, "https://explorer.cow.fi/orders/0xabc");
    }

    #[test]
    fn explorer_link_non_mainnet_has_network() {
        let url = order_explorer_link(SupportedChainId::Sepolia, "0xabc");
        assert_eq!(url, "https://explorer.cow.fi/sepolia/orders/0xabc");
    }
}
