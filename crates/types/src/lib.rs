//! `cow-types` — Layer 1 protocol enums and shared types for the `CoW` Protocol SDK.
//!
//! This crate defines the protocol-level enums used across the workspace:
//!
//! | Enum | Purpose |
//! |---|---|
//! | [`OrderKind`] | `Sell` / `Buy` direction |
//! | [`TokenBalance`] | `Erc20` / `External` / `Internal` balance source |
//! | [`SigningScheme`] | `Eip712` / `EthSign` / `Eip1271` / `PreSign` |
//! | [`EcdsaSigningScheme`] | ECDSA-only subset (`Eip712` / `EthSign`) |
//! | [`PriceQuality`] | `Fast` / `Optimal` / `Verified` quote hint |
//!
//! Numeric constants (`ZERO`, `ONE`, `MAX_UINT256`, ...) live in
//! [`cow-primitives`](https://docs.rs/cow-primitives).

#![deny(unsafe_code)]
#![warn(missing_docs)]

use std::fmt;

use cow_errors::CowError;
use serde::{Deserialize, Serialize};

// ── Shared protocol types pushed down from domain crates ────────────────────
//
// Types in this section used to live in higher-layer crates (app-data, order-
// book) but were referenced from multiple L2 siblings and so had to be pushed
// down to L1 to avoid cross-sibling dependencies.

/// A single `CoW` Protocol pre- or post-settlement interaction hook.
///
/// Hooks are arbitrary contract calls that the `CoW` settlement contract
/// executes before (`pre`) or after (`post`) the trade. Common use cases
/// include token approvals, NFT transfers, and flash-loan repayments.
///
/// # Fields
///
/// * `target` — the contract address to call (`0x`-prefixed, 20 bytes).
/// * `call_data` — ABI-encoded function selector + arguments (`0x`-prefixed).
/// * `gas_limit` — maximum gas the hook may consume (decimal string).
/// * `dapp_id` — optional identifier for the dApp that registered the hook.
///
/// # Example
///
/// ```
/// use cow_types::CowHook;
///
/// let hook = CowHook::new("0x1234567890abcdef1234567890abcdef12345678", "0xabcdef00", "100000")
///     .with_dapp_id("my-dapp");
///
/// assert!(hook.has_dapp_id());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CowHook {
    /// Target contract address (checksummed hex with `0x` prefix).
    pub target: String,
    /// ABI-encoded call data (hex with `0x` prefix).
    pub call_data: String,
    /// Maximum gas this hook may consume (decimal string, e.g. `"100000"`).
    pub gas_limit: String,
    /// Optional dApp identifier for the hook's origin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dapp_id: Option<String>,
}

impl CowHook {
    /// Construct a new [`CowHook`] without a dApp identifier.
    #[must_use]
    pub fn new(
        target: impl Into<String>,
        call_data: impl Into<String>,
        gas_limit: impl Into<String>,
    ) -> Self {
        Self {
            target: target.into(),
            call_data: call_data.into(),
            gas_limit: gas_limit.into(),
            dapp_id: None,
        }
    }

    /// Attach a dApp identifier to this hook.
    #[must_use]
    pub fn with_dapp_id(mut self, dapp_id: impl Into<String>) -> Self {
        self.dapp_id = Some(dapp_id.into());
        self
    }

    /// Returns `true` if a dApp identifier is set on this hook.
    #[must_use]
    pub const fn has_dapp_id(&self) -> bool {
        self.dapp_id.is_some()
    }
}

impl fmt::Display for CowHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hook(target={}, gas={})", self.target, self.gas_limit)
    }
}

/// On-chain placement metadata for orders submitted directly on-chain
/// (as opposed to the off-chain API).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnchainOrderData {
    /// The address that created the on-chain order (may differ from `owner` for
    /// `EthFlow` orders where the contract is the technical owner).
    pub sender: alloy_primitives::Address,
    /// Non-`None` when the orderbook rejected the order due to a placement error.
    pub placement_error: Option<String>,
}

impl OnchainOrderData {
    /// Construct an [`OnchainOrderData`] record.
    #[must_use]
    pub const fn new(sender: alloy_primitives::Address) -> Self {
        Self { sender, placement_error: None }
    }

    /// Returns `true` if a placement error was reported for this on-chain order.
    #[must_use]
    pub const fn has_placement_error(&self) -> bool {
        self.placement_error.is_some()
    }
}

impl fmt::Display for OnchainOrderData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "onchain(sender={:#x})", self.sender)
    }
}

/// Whether to sell an exact input amount or buy an exact output amount.
///
/// Used in every order and quote request to specify the trade direction.
/// Serialises to `"sell"` or `"buy"` in JSON.
///
/// # Example
///
/// ```
/// use cow_types::OrderKind;
///
/// let kind = OrderKind::Sell;
/// assert_eq!(kind.as_str(), "sell");
/// assert!(kind.is_sell());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderKind {
    /// Sell an exact input amount; receive at least `buyAmount`.
    Sell,
    /// Buy an exact output amount; spend at most `sellAmount`.
    Buy,
}

impl OrderKind {
    /// Returns the lowercase string used by the `CoW` Protocol API.
    ///
    /// # Returns
    ///
    /// `"sell"` for [`Sell`](Self::Sell), `"buy"` for [`Buy`](Self::Buy).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sell => "sell",
            Self::Buy => "buy",
        }
    }

    /// Returns `true` if this is a sell order.
    ///
    /// ```
    /// use cow_types::OrderKind;
    ///
    /// assert!(OrderKind::Sell.is_sell());
    /// assert!(!OrderKind::Buy.is_sell());
    /// ```
    #[must_use]
    pub const fn is_sell(self) -> bool {
        matches!(self, Self::Sell)
    }

    /// Returns `true` if this is a buy order.
    ///
    /// ```
    /// use cow_types::OrderKind;
    ///
    /// assert!(OrderKind::Buy.is_buy());
    /// assert!(!OrderKind::Sell.is_buy());
    /// ```
    #[must_use]
    pub const fn is_buy(self) -> bool {
        matches!(self, Self::Buy)
    }
}

impl fmt::Display for OrderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The ERC-20 balance source/destination for `sellToken` and `buyToken`.
///
/// Controls whether the `CoW` settlement contract transfers tokens via
/// standard ERC-20 `transferFrom` or via the Balancer Vault's internal
/// balance system.
///
/// # Example
///
/// ```
/// use cow_types::TokenBalance;
///
/// let balance = TokenBalance::Erc20;
/// assert_eq!(balance.as_str(), "erc20");
/// assert!(balance.is_erc20());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TokenBalance {
    /// Standard ERC-20 transfer (default).
    #[default]
    Erc20,
    /// Balancer Vault internal balance (sell side only).
    External,
    /// Balancer Vault internal balance (buy side only).
    Internal,
}

impl TokenBalance {
    /// Returns the lowercase string used by the `CoW` Protocol API.
    ///
    /// # Returns
    ///
    /// `"erc20"`, `"external"`, or `"internal"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Erc20 => "erc20",
            Self::External => "external",
            Self::Internal => "internal",
        }
    }

    /// Compute `keccak256(self.as_str())` for EIP-712 struct hashing.
    ///
    /// The EIP-712 order struct encodes the token balance kind as
    /// `keccak256(bytes("erc20"))` (or `"external"` / `"internal"`).
    ///
    /// # Returns
    ///
    /// A 32-byte [`B256`](alloy_primitives::B256) hash of the variant
    /// string.
    #[must_use]
    pub fn eip712_hash(self) -> alloy_primitives::B256 {
        alloy_primitives::keccak256(self.as_str().as_bytes())
    }

    /// Returns `true` if the standard ERC-20 transfer mode is used.
    ///
    /// This is the default for most orders.
    #[must_use]
    pub const fn is_erc20(self) -> bool {
        matches!(self, Self::Erc20)
    }

    /// Returns `true` if the Balancer Vault external balance is used
    /// (sell side only).
    #[must_use]
    pub const fn is_external(self) -> bool {
        matches!(self, Self::External)
    }

    /// Returns `true` if the Balancer Vault internal balance is used
    /// (buy side only).
    #[must_use]
    pub const fn is_internal(self) -> bool {
        matches!(self, Self::Internal)
    }
}

impl fmt::Display for TokenBalance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Signing scheme for a `CoW` Protocol order.
///
/// Determines how the order signature is verified:
///
/// - **`Eip712`** — standard EIP-712 typed-data signature (most wallets).
/// - **`EthSign`** — legacy `eth_sign` with EIP-191 prefix.
/// - **`Eip1271`** — smart-contract signature via `isValidSignature`.
/// - **`PreSign`** — on-chain pre-approval via `setPreSignature`.
///
/// # Example
///
/// ```
/// use cow_types::SigningScheme;
///
/// let scheme = SigningScheme::Eip712;
/// assert_eq!(scheme.as_str(), "eip712");
/// assert!(scheme.is_eip712());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SigningScheme {
    /// Standard EIP-712 typed-data signature.
    Eip712,
    /// Legacy `eth_sign` (EIP-191) signature.
    EthSign,
    /// EIP-1271 smart-contract signature.
    Eip1271,
    /// On-chain pre-signature via `setPreSignature`.
    PreSign,
}

impl SigningScheme {
    /// Returns the lowercase string used by the `CoW` Protocol API.
    ///
    /// # Returns
    ///
    /// `"eip712"`, `"ethsign"`, `"eip1271"`, or `"presign"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eip712 => "eip712",
            Self::EthSign => "ethsign",
            Self::Eip1271 => "eip1271",
            Self::PreSign => "presign",
        }
    }

    /// Returns `true` if the EIP-712 typed-data signing scheme is used.
    ///
    /// This is the most common scheme for EOA wallets.
    #[must_use]
    pub const fn is_eip712(self) -> bool {
        matches!(self, Self::Eip712)
    }

    /// Returns `true` if the legacy EIP-191 (`eth_sign`) scheme is used.
    ///
    /// Some older wallets or hardware signers only support this method.
    #[must_use]
    pub const fn is_eth_sign(self) -> bool {
        matches!(self, Self::EthSign)
    }

    /// Returns `true` if the EIP-1271 smart-contract signature scheme is
    /// used.
    ///
    /// The signature is verified on-chain by calling `isValidSignature`
    /// on the signing contract.
    #[must_use]
    pub const fn is_eip1271(self) -> bool {
        matches!(self, Self::Eip1271)
    }

    /// Returns `true` if the on-chain pre-sign scheme is used.
    ///
    /// The order owner calls `setPreSignature` on the settlement contract
    /// before the order can be filled.
    #[must_use]
    pub const fn is_presign(self) -> bool {
        matches!(self, Self::PreSign)
    }
}

impl fmt::Display for SigningScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// ECDSA-only signing schemes (EIP-712 or EIP-191).
///
/// A subset of [`SigningScheme`] limited to the two schemes that produce
/// standard ECDSA signatures. Use [`into_signing_scheme`](Self::into_signing_scheme)
/// to widen to the full enum when needed.
///
/// # Example
///
/// ```
/// use cow_types::{EcdsaSigningScheme, SigningScheme};
///
/// let ecdsa = EcdsaSigningScheme::Eip712;
/// let full: SigningScheme = ecdsa.into();
/// assert_eq!(full, SigningScheme::Eip712);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EcdsaSigningScheme {
    /// Standard EIP-712 typed-data signature (default).
    #[default]
    Eip712,
    /// Legacy `eth_sign` (EIP-191) signature.
    EthSign,
}

impl EcdsaSigningScheme {
    /// Widen to the full [`SigningScheme`] enum.
    ///
    /// # Returns
    ///
    /// [`SigningScheme::Eip712`] or [`SigningScheme::EthSign`].
    #[must_use]
    pub const fn into_signing_scheme(self) -> SigningScheme {
        match self {
            Self::Eip712 => SigningScheme::Eip712,
            Self::EthSign => SigningScheme::EthSign,
        }
    }

    /// Returns the lowercase string used by the `CoW` Protocol API.
    ///
    /// # Returns
    ///
    /// `"eip712"` or `"ethsign"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eip712 => "eip712",
            Self::EthSign => "ethsign",
        }
    }

    /// Returns `true` if the EIP-712 typed-data scheme is selected.
    #[must_use]
    pub const fn is_eip712(self) -> bool {
        matches!(self, Self::Eip712)
    }

    /// Returns `true` if the legacy EIP-191 (`eth_sign`) scheme is
    /// selected.
    #[must_use]
    pub const fn is_eth_sign(self) -> bool {
        matches!(self, Self::EthSign)
    }
}

impl fmt::Display for EcdsaSigningScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<EcdsaSigningScheme> for SigningScheme {
    /// Widen an ECDSA-only scheme to the full [`SigningScheme`] enum.
    ///
    /// This is the Rust equivalent of `SIGN_SCHEME_MAP` from the
    /// `TypeScript` SDK.
    fn from(s: EcdsaSigningScheme) -> Self {
        s.into_signing_scheme()
    }
}

/// Quote price-quality hint passed to the orderbook.
///
/// Controls the trade-off between response speed and price accuracy when
/// requesting a quote via `POST /api/v1/quote`.
///
/// # Example
///
/// ```
/// use cow_types::PriceQuality;
///
/// let quality = PriceQuality::Optimal;
/// assert_eq!(quality.as_str(), "optimal");
/// assert!(quality.is_optimal());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PriceQuality {
    /// Fast estimate — may be slightly stale.
    Fast,
    /// Optimal price — runs the full solver pipeline.
    #[default]
    Optimal,
    /// Like optimal but includes on-chain simulation to verify executability.
    Verified,
}

impl PriceQuality {
    /// Returns the lowercase string used by the `CoW` Protocol API.
    ///
    /// # Returns
    ///
    /// `"fast"`, `"optimal"`, or `"verified"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Optimal => "optimal",
            Self::Verified => "verified",
        }
    }

    /// Returns `true` if the fast (potentially stale) price quality is
    /// selected.
    #[must_use]
    pub const fn is_fast(self) -> bool {
        matches!(self, Self::Fast)
    }

    /// Returns `true` if the optimal (full solver pipeline) price quality
    /// is selected. This is the default.
    #[must_use]
    pub const fn is_optimal(self) -> bool {
        matches!(self, Self::Optimal)
    }

    /// Returns `true` if the verified (on-chain simulation) price quality
    /// is selected.
    #[must_use]
    pub const fn is_verified(self) -> bool {
        matches!(self, Self::Verified)
    }
}

impl fmt::Display for PriceQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for OrderKind {
    type Error = CowError;

    /// Parse a `CoW` Protocol order kind from its API string.
    ///
    /// Accepts `"sell"` or `"buy"`.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "sell" => Ok(Self::Sell),
            "buy" => Ok(Self::Buy),
            other => Err(CowError::Parse {
                field: "OrderKind",
                reason: format!("unknown value: {other}"),
            }),
        }
    }
}

impl TryFrom<&str> for TokenBalance {
    type Error = CowError;

    /// Parse a `CoW` Protocol token balance kind from its API string.
    ///
    /// Accepts `"erc20"`, `"external"`, or `"internal"`.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "erc20" => Ok(Self::Erc20),
            "external" => Ok(Self::External),
            "internal" => Ok(Self::Internal),
            other => Err(CowError::Parse {
                field: "TokenBalance",
                reason: format!("unknown value: {other}"),
            }),
        }
    }
}

impl TryFrom<&str> for SigningScheme {
    type Error = CowError;

    /// Parse a `CoW` Protocol signing scheme from its API string.
    ///
    /// Accepts `"eip712"`, `"ethsign"`, `"eip1271"`, or `"presign"`.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "eip712" => Ok(Self::Eip712),
            "ethsign" => Ok(Self::EthSign),
            "eip1271" => Ok(Self::Eip1271),
            "presign" => Ok(Self::PreSign),
            other => Err(CowError::Parse {
                field: "SigningScheme",
                reason: format!("unknown value: {other}"),
            }),
        }
    }
}

impl TryFrom<&str> for EcdsaSigningScheme {
    type Error = CowError;

    /// Parse a `CoW` Protocol ECDSA signing scheme from its API string.
    ///
    /// Accepts `"eip712"` or `"ethsign"`.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "eip712" => Ok(Self::Eip712),
            "ethsign" => Ok(Self::EthSign),
            other => Err(CowError::Parse {
                field: "EcdsaSigningScheme",
                reason: format!("unknown value: {other}"),
            }),
        }
    }
}

impl TryFrom<&str> for PriceQuality {
    type Error = CowError;

    /// Parse a `CoW` Protocol price quality hint from its API string.
    ///
    /// Accepts `"fast"`, `"optimal"`, or `"verified"`.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "fast" => Ok(Self::Fast),
            "optimal" => Ok(Self::Optimal),
            "verified" => Ok(Self::Verified),
            other => Err(CowError::Parse {
                field: "PriceQuality",
                reason: format!("unknown value: {other}"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── OrderKind ────────────────────────────────────────────────────────

    #[test]
    fn order_kind_as_str() {
        assert_eq!(OrderKind::Sell.as_str(), "sell");
        assert_eq!(OrderKind::Buy.as_str(), "buy");
    }

    #[test]
    fn order_kind_predicates() {
        assert!(OrderKind::Sell.is_sell());
        assert!(!OrderKind::Sell.is_buy());
        assert!(OrderKind::Buy.is_buy());
        assert!(!OrderKind::Buy.is_sell());
    }

    #[test]
    fn order_kind_display() {
        assert_eq!(format!("{}", OrderKind::Sell), "sell");
        assert_eq!(format!("{}", OrderKind::Buy), "buy");
    }

    #[test]
    fn order_kind_roundtrip() {
        for kind in [OrderKind::Sell, OrderKind::Buy] {
            let parsed = OrderKind::try_from(kind.as_str()).unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn order_kind_invalid() {
        assert!(OrderKind::try_from("invalid").is_err());
        assert!(OrderKind::try_from("").is_err());
        assert!(OrderKind::try_from("SELL").is_err());
    }

    #[test]
    fn order_kind_serde_roundtrip() {
        let json = serde_json::to_string(&OrderKind::Sell).unwrap();
        assert_eq!(json, "\"sell\"");
        let back: OrderKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, OrderKind::Sell);
    }

    // ── TokenBalance ────────────────────────────────────────────────────

    #[test]
    fn token_balance_as_str() {
        assert_eq!(TokenBalance::Erc20.as_str(), "erc20");
        assert_eq!(TokenBalance::External.as_str(), "external");
        assert_eq!(TokenBalance::Internal.as_str(), "internal");
    }

    #[test]
    fn token_balance_predicates() {
        assert!(TokenBalance::Erc20.is_erc20());
        assert!(!TokenBalance::Erc20.is_external());
        assert!(!TokenBalance::Erc20.is_internal());
        assert!(TokenBalance::External.is_external());
        assert!(TokenBalance::Internal.is_internal());
    }

    #[test]
    fn token_balance_default() {
        assert_eq!(TokenBalance::default(), TokenBalance::Erc20);
    }

    #[test]
    fn token_balance_roundtrip() {
        for bal in [TokenBalance::Erc20, TokenBalance::External, TokenBalance::Internal] {
            let parsed = TokenBalance::try_from(bal.as_str()).unwrap();
            assert_eq!(parsed, bal);
        }
    }

    #[test]
    fn token_balance_invalid() {
        assert!(TokenBalance::try_from("ERC20").is_err());
        assert!(TokenBalance::try_from("").is_err());
    }

    #[test]
    fn token_balance_eip712_hash_deterministic() {
        let h1 = TokenBalance::Erc20.eip712_hash();
        let h2 = TokenBalance::Erc20.eip712_hash();
        assert_eq!(h1, h2);
        // Different variants produce different hashes
        assert_ne!(TokenBalance::Erc20.eip712_hash(), TokenBalance::External.eip712_hash());
        assert_ne!(TokenBalance::External.eip712_hash(), TokenBalance::Internal.eip712_hash());
    }

    #[test]
    fn token_balance_display() {
        assert_eq!(format!("{}", TokenBalance::External), "external");
    }

    // ── SigningScheme ────────────────────────────────────────────────────

    #[test]
    fn signing_scheme_as_str() {
        assert_eq!(SigningScheme::Eip712.as_str(), "eip712");
        assert_eq!(SigningScheme::EthSign.as_str(), "ethsign");
        assert_eq!(SigningScheme::Eip1271.as_str(), "eip1271");
        assert_eq!(SigningScheme::PreSign.as_str(), "presign");
    }

    #[test]
    fn signing_scheme_predicates() {
        assert!(SigningScheme::Eip712.is_eip712());
        assert!(SigningScheme::EthSign.is_eth_sign());
        assert!(SigningScheme::Eip1271.is_eip1271());
        assert!(SigningScheme::PreSign.is_presign());
        assert!(!SigningScheme::Eip712.is_presign());
    }

    #[test]
    fn signing_scheme_roundtrip() {
        for s in [
            SigningScheme::Eip712,
            SigningScheme::EthSign,
            SigningScheme::Eip1271,
            SigningScheme::PreSign,
        ] {
            assert_eq!(SigningScheme::try_from(s.as_str()).unwrap(), s);
        }
    }

    #[test]
    fn signing_scheme_invalid() {
        assert!(SigningScheme::try_from("eip-712").is_err());
        assert!(SigningScheme::try_from("").is_err());
    }

    #[test]
    fn signing_scheme_display() {
        assert_eq!(format!("{}", SigningScheme::PreSign), "presign");
    }

    // ── EcdsaSigningScheme ──────────────────────────────────────────────

    #[test]
    fn ecdsa_scheme_default() {
        assert_eq!(EcdsaSigningScheme::default(), EcdsaSigningScheme::Eip712);
    }

    #[test]
    fn ecdsa_scheme_into_signing_scheme() {
        assert_eq!(EcdsaSigningScheme::Eip712.into_signing_scheme(), SigningScheme::Eip712);
        assert_eq!(EcdsaSigningScheme::EthSign.into_signing_scheme(), SigningScheme::EthSign);
    }

    #[test]
    fn ecdsa_scheme_from_conversion() {
        let full: SigningScheme = EcdsaSigningScheme::EthSign.into();
        assert_eq!(full, SigningScheme::EthSign);
    }

    #[test]
    fn ecdsa_scheme_predicates() {
        assert!(EcdsaSigningScheme::Eip712.is_eip712());
        assert!(!EcdsaSigningScheme::Eip712.is_eth_sign());
        assert!(EcdsaSigningScheme::EthSign.is_eth_sign());
    }

    #[test]
    fn ecdsa_scheme_roundtrip() {
        for s in [EcdsaSigningScheme::Eip712, EcdsaSigningScheme::EthSign] {
            assert_eq!(EcdsaSigningScheme::try_from(s.as_str()).unwrap(), s);
        }
    }

    #[test]
    fn ecdsa_scheme_invalid() {
        assert!(EcdsaSigningScheme::try_from("eip1271").is_err());
    }

    #[test]
    fn ecdsa_scheme_display() {
        assert_eq!(format!("{}", EcdsaSigningScheme::EthSign), "ethsign");
    }

    // ── PriceQuality ────────────────────────────────────────────────────

    #[test]
    fn price_quality_default() {
        assert_eq!(PriceQuality::default(), PriceQuality::Optimal);
    }

    #[test]
    fn price_quality_as_str() {
        assert_eq!(PriceQuality::Fast.as_str(), "fast");
        assert_eq!(PriceQuality::Optimal.as_str(), "optimal");
        assert_eq!(PriceQuality::Verified.as_str(), "verified");
    }

    #[test]
    fn price_quality_predicates() {
        assert!(PriceQuality::Fast.is_fast());
        assert!(PriceQuality::Optimal.is_optimal());
        assert!(PriceQuality::Verified.is_verified());
        assert!(!PriceQuality::Fast.is_optimal());
    }

    #[test]
    fn price_quality_roundtrip() {
        for q in [PriceQuality::Fast, PriceQuality::Optimal, PriceQuality::Verified] {
            assert_eq!(PriceQuality::try_from(q.as_str()).unwrap(), q);
        }
    }

    #[test]
    fn price_quality_invalid() {
        assert!(PriceQuality::try_from("slow").is_err());
    }

    #[test]
    fn price_quality_display() {
        assert_eq!(format!("{}", PriceQuality::Verified), "verified");
    }
}
