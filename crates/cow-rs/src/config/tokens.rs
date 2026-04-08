//! Token constants and per-chain wrapped-native-currency info.
//!
//! Defines sentinel addresses for native currencies (ETH, SOL, BTC) and the
//! [`TokenInfo`] struct used to describe tokens throughout the SDK.
//!
//! # Key items
//!
//! | Item | Purpose |
//! |---|---|
//! | [`NATIVE_CURRENCY_ADDRESS`] | `0xEeee…EeEe` sentinel for native currency |
//! | [`TokenInfo`] | Minimal token metadata (address, decimals, symbol) |
//! | [`wrapped_native_currency`] | Per-chain wrapped token lookup (WETH, WXDAI, …) |
//! | [`get_wrapped_token_for_chain`] | Same as above, returns `Option` |

use std::fmt;

use alloy_primitives::Address;

use super::chain::SupportedChainId;

/// The sentinel address used to represent the native currency (ETH, xDAI, AVAX, …).
///
/// `0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE`
pub const NATIVE_CURRENCY_ADDRESS: Address = Address::new([0xee; 20]);

/// The standard address used to represent native currency on EVM chains.
///
/// Alias for [`NATIVE_CURRENCY_ADDRESS`].
/// `0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE`
pub const EVM_NATIVE_CURRENCY_ADDRESS: Address = NATIVE_CURRENCY_ADDRESS;

/// Sentinel address for SOL native currency.
///
/// There is no standard address for SOL, so the default program address is used.
pub const SOL_NATIVE_CURRENCY_ADDRESS: &str = "11111111111111111111111111111111";

/// Sentinel address for BTC native currency.
///
/// The Bitcoin genesis address is used as a token address placeholder.
pub const BTC_CURRENCY_ADDRESS: &str = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";

/// Minimal token metadata used for order size calculations.
///
/// Stores the on-chain address, decimal precision, ticker symbol, and
/// optional human-readable name and logo URL. Used throughout the SDK to
/// convert between human-readable amounts and token atoms, and to identify
/// tokens in logs and error messages.
///
/// # Example
///
/// ```
/// use alloy_primitives::Address;
/// use cow_rs::TokenInfo;
///
/// let usdc = TokenInfo::new(Address::ZERO, 6, "USDC").with_name("USD Coin");
/// assert_eq!(usdc.decimals_multiplier(), 1_000_000u128);
/// assert!(usdc.has_name());
/// assert!(!usdc.has_logo_url());
/// ```
#[derive(Debug, Clone, Copy)]
pub struct TokenInfo {
    /// On-chain ERC-20 address.
    pub address: Address,
    /// Number of decimal places.
    pub decimals: u8,
    /// Short ticker symbol (e.g. `"WETH"`).
    pub symbol: &'static str,
    /// Human-readable token name (e.g. `"Wrapped Ether"`).
    ///
    /// `None` when the name is not known at compile time.
    pub name: Option<&'static str>,
    /// URL to the token's logo image.
    ///
    /// `None` when no logo URL is available.
    pub logo_url: Option<&'static str>,
}

impl TokenInfo {
    /// Construct a minimal [`TokenInfo`] with no name or logo.
    ///
    /// # Parameters
    ///
    /// * `address` — the on-chain ERC-20 contract [`Address`].
    /// * `decimals` — number of decimal places (e.g. `18` for WETH, `6` for USDC).
    /// * `symbol` — short ticker symbol (e.g. `"WETH"`).
    ///
    /// # Returns
    ///
    /// A new [`TokenInfo`] with `name` and `logo_url` set to `None`.
    #[must_use]
    pub const fn new(address: Address, decimals: u8, symbol: &'static str) -> Self {
        Self { address, decimals, symbol, name: None, logo_url: None }
    }

    /// Attach a human-readable name (e.g. `"Wrapped Ether"`).
    ///
    /// # Parameters
    ///
    /// * `name` — the token's full name.
    ///
    /// # Returns
    ///
    /// `self` with `name` set.
    #[must_use]
    pub const fn with_name(mut self, name: &'static str) -> Self {
        self.name = Some(name);
        self
    }

    /// Attach a logo URL for display in UIs.
    ///
    /// # Parameters
    ///
    /// * `url` — URL pointing to the token's logo image.
    ///
    /// # Returns
    ///
    /// `self` with `logo_url` set.
    #[must_use]
    pub const fn with_logo_url(mut self, url: &'static str) -> Self {
        self.logo_url = Some(url);
        self
    }

    /// Returns `true` if this token has an associated human-readable name.
    ///
    /// # Returns
    ///
    /// `true` when the `name` field is `Some`.
    #[must_use]
    pub const fn has_name(&self) -> bool {
        self.name.is_some()
    }

    /// Returns `true` if this token has an associated logo URL.
    ///
    /// # Returns
    ///
    /// `true` when the `logo_url` field is `Some`.
    #[must_use]
    pub const fn has_logo_url(&self) -> bool {
        self.logo_url.is_some()
    }

    /// Returns `true` if this token represents the native currency sentinel address.
    ///
    /// The sentinel `0xEeee...EeEe` is used by `CoW` Protocol to denote ETH,
    /// xDAI, MATIC, or the native currency of any chain.
    ///
    /// # Returns
    ///
    /// `true` when the token address equals [`NATIVE_CURRENCY_ADDRESS`].
    ///
    /// ```
    /// use cow_rs::{NATIVE_CURRENCY_ADDRESS, TokenInfo};
    ///
    /// let native = TokenInfo::new(NATIVE_CURRENCY_ADDRESS, 18, "ETH");
    /// assert!(native.is_native_currency());
    ///
    /// let weth = cow_rs::config::wrapped_native_currency(cow_rs::SupportedChainId::Mainnet);
    /// assert!(!weth.is_native_currency());
    /// ```
    #[must_use]
    pub fn is_native_currency(&self) -> bool {
        self.address == NATIVE_CURRENCY_ADDRESS
    }

    /// Returns `10^decimals` as a `u128` multiplier.
    ///
    /// Useful for converting between human-readable amounts and token atoms.
    ///
    /// # Returns
    ///
    /// `10^decimals` as a `u128`.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::TokenInfo;
    ///
    /// let usdc = TokenInfo::new(Address::ZERO, 6, "USDC");
    /// assert_eq!(usdc.decimals_multiplier(), 1_000_000u128);
    ///
    /// let weth = TokenInfo::new(Address::ZERO, 18, "WETH");
    /// assert_eq!(weth.decimals_multiplier(), 1_000_000_000_000_000_000u128);
    /// ```
    #[must_use]
    pub fn decimals_multiplier(&self) -> u128 {
        10u128.pow(u32::from(self.decimals))
    }

    /// Returns the decimal count as a `u64`.
    ///
    /// # Returns
    ///
    /// The `decimals` field widened to `u64`.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_rs::TokenInfo;
    ///
    /// let usdc = TokenInfo::new(Address::ZERO, 6, "USDC");
    /// assert_eq!(usdc.decimals_u64(), 6u64);
    /// ```
    #[must_use]
    pub const fn decimals_u64(&self) -> u64 {
        self.decimals as u64
    }

    /// Returns `true` if this token has no fractional precision (zero decimals).
    ///
    /// # Returns
    ///
    /// `true` when `decimals == 0`.
    #[must_use]
    pub const fn is_zero_decimals(&self) -> bool {
        self.decimals == 0
    }
}

impl fmt::Display for TokenInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:#x})", self.symbol, self.address)
    }
}

impl From<TokenInfo> for Address {
    fn from(t: TokenInfo) -> Self {
        t.address
    }
}

impl From<TokenInfo> for u8 {
    /// Extract the decimal count from a [`TokenInfo`].
    fn from(t: TokenInfo) -> Self {
        t.decimals
    }
}

/// Return the wrapped native currency for `chain`, or `None` if unknown.
///
/// This is the Rust equivalent of `getWrappedTokenForChain` from the `TypeScript` SDK.
/// Since all [`SupportedChainId`] variants have a known wrapped token, this will
/// always return `Some` for valid chain IDs.
///
/// # Arguments
///
/// * `chain` — the [`SupportedChainId`] to look up.
///
/// # Returns
///
/// `Some(token_info)` for any supported chain (currently always `Some`).
///
/// # Example
///
/// ```rust
/// use cow_rs::{SupportedChainId, config::get_wrapped_token_for_chain};
///
/// let token = get_wrapped_token_for_chain(SupportedChainId::Mainnet);
/// assert!(token.is_some());
/// assert_eq!(token.unwrap().symbol, "WETH");
/// ```
#[must_use]
pub const fn get_wrapped_token_for_chain(chain: SupportedChainId) -> Option<TokenInfo> {
    Some(wrapped_native_currency(chain))
}

/// Return the wrapped native currency [`TokenInfo`] for `chain`.
///
/// Each supported chain has a canonical wrapped token: WETH on Ethereum,
/// WXDAI on Gnosis, WPOL on Polygon, etc.
///
/// # Parameters
///
/// * `chain` — the [`SupportedChainId`] to look up.
///
/// # Returns
///
/// A [`TokenInfo`] with the wrapped token's address, decimals, symbol,
/// and name.
///
/// # Example
///
/// ```
/// use cow_rs::{SupportedChainId, config::wrapped_native_currency};
///
/// let weth = wrapped_native_currency(SupportedChainId::Mainnet);
/// assert_eq!(weth.symbol, "WETH");
/// assert_eq!(weth.decimals, 18);
/// ```
#[must_use]
pub const fn wrapped_native_currency(chain: SupportedChainId) -> TokenInfo {
    match chain {
        SupportedChainId::Mainnet => TokenInfo {
            address: address("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            decimals: 18,
            symbol: "WETH",
            name: Some("Wrapped Ether"),
            logo_url: None,
        },
        SupportedChainId::GnosisChain => TokenInfo {
            address: address("e91d153e0b41518a2ce8dd3d7944fa863463a97d"),
            decimals: 18,
            symbol: "WXDAI",
            name: Some("Wrapped XDAI"),
            logo_url: None,
        },
        SupportedChainId::ArbitrumOne => TokenInfo {
            address: address("82af49447d8a07e3bd95bd0d56f35241523fbab1"),
            decimals: 18,
            symbol: "WETH",
            name: Some("Wrapped Ether"),
            logo_url: None,
        },
        // Base and Ink are both OP Stack chains sharing the canonical WETH address.
        SupportedChainId::Base | SupportedChainId::Ink => TokenInfo {
            address: address("4200000000000000000000000000000000000006"),
            decimals: 18,
            symbol: "WETH",
            name: Some("Wrapped Ether"),
            logo_url: None,
        },
        SupportedChainId::Sepolia => TokenInfo {
            address: address("fff9976782d46cc05630d1f6ebab18b2324d6b14"),
            decimals: 18,
            symbol: "WETH",
            name: Some("Wrapped Ether"),
            logo_url: None,
        },
        SupportedChainId::Polygon => TokenInfo {
            address: address("0d500b1d8e8ef31e21c99d1db9a6444d3adf1270"),
            decimals: 18,
            symbol: "WPOL",
            name: Some("Wrapped POL"),
            logo_url: None,
        },
        SupportedChainId::Avalanche => TokenInfo {
            address: address("b31f66aa3c1e785363f0875a1b74e27b85fd66c7"),
            decimals: 18,
            symbol: "WAVAX",
            name: Some("Wrapped AVAX"),
            logo_url: None,
        },
        SupportedChainId::BnbChain => TokenInfo {
            address: address("bb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c"),
            decimals: 18,
            symbol: "WBNB",
            name: Some("Wrapped BNB"),
            logo_url: None,
        },
        SupportedChainId::Linea => TokenInfo {
            address: address("e5d7c2a44ffddf6b295a15c148167daaaf5cf34e"),
            decimals: 18,
            symbol: "WETH",
            name: Some("Wrapped Ether"),
            logo_url: None,
        },
        // GHO is the native gas token on Lens; WGHO is its ERC-20 wrapper.
        SupportedChainId::Lens => TokenInfo {
            address: address("6bdc36e20d267ff0dd6097799f82e78907105e2f"),
            decimals: 18,
            symbol: "WGHO",
            name: Some("Wrapped GHO"),
            logo_url: None,
        },
        SupportedChainId::Plasma => TokenInfo {
            address: address("6100e367285b01f48d07953803a2d8dca5d19873"),
            decimals: 18,
            symbol: "WXPL",
            name: Some("Wrapped XPL"),
            logo_url: None,
        },
    }
}

/// Parse a lowercase hex address literal (without `0x` prefix) into [`Address`].
const fn address(hex: &str) -> Address {
    // Safe: all callers are validated lowercase hex strings of exactly 40 chars.
    let bytes = hex.as_bytes();
    let mut out = [0u8; 20];
    let mut i = 0;
    while i < 20 {
        out[i] = nibble(bytes[i * 2]) << 4 | nibble(bytes[i * 2 + 1]);
        i += 1;
    }
    Address::new(out)
}

/// Convert a hex ASCII byte to its 4-bit value.
const fn nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        _ => 0,
    }
}
