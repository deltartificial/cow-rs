//! App-data document types for `CoW` Protocol order metadata.
//!
//! This module defines the Rust types that mirror the `CoW` Protocol's
//! app-data JSON schema (currently v1.14.0). Every type serialises to /
//! deserialises from `camelCase` JSON via `serde`, matching the on-chain
//! format exactly.
//!
//! # Type overview
//!
//! | Type | Role |
//! |---|---|
//! | [`AppDataDoc`] | Root document — version, app code, metadata |
//! | [`Metadata`] | Container for all optional metadata fields |
//! | [`OrderClassKind`] | `market` / `limit` / `liquidity` / `twap` |
//! | [`CowHook`] | Pre- or post-settlement interaction hook |
//! | [`PartnerFee`] | Single or multi-entry partner fee policy |
//! | [`Quote`] | Slippage tolerance embedded in the order |
//! | [`Referrer`] | Partner referral tracking code |
//! | [`Utm`] | UTM campaign tracking parameters |
//! | [`Bridging`] | Cross-chain bridge metadata |
//! | [`Flashloan`] | Flash loan execution metadata |

use std::fmt;

use serde::{Deserialize, Serialize};

/// Latest app-data schema version this crate targets.
pub const LATEST_APP_DATA_VERSION: &str = "1.14.0";

/// Latest version of the quote metadata schema.
pub const LATEST_QUOTE_METADATA_VERSION: &str = "1.1.0";

/// Latest version of the referrer metadata schema.
pub const LATEST_REFERRER_METADATA_VERSION: &str = "1.0.0";

/// Latest version of the order class metadata schema.
pub const LATEST_ORDER_CLASS_METADATA_VERSION: &str = "0.3.0";

/// Latest version of the UTM metadata schema.
pub const LATEST_UTM_METADATA_VERSION: &str = "0.3.0";

/// Latest version of the hooks metadata schema.
pub const LATEST_HOOKS_METADATA_VERSION: &str = "0.2.0";

/// Latest version of the signer metadata schema.
pub const LATEST_SIGNER_METADATA_VERSION: &str = "0.1.0";

/// Latest version of the widget metadata schema.
pub const LATEST_WIDGET_METADATA_VERSION: &str = "0.1.0";

/// Latest version of the partner fee metadata schema.
pub const LATEST_PARTNER_FEE_METADATA_VERSION: &str = "1.0.0";

/// Latest version of the replaced order metadata schema.
pub const LATEST_REPLACED_ORDER_METADATA_VERSION: &str = "0.1.0";

/// Latest version of the wrappers metadata schema.
pub const LATEST_WRAPPERS_METADATA_VERSION: &str = "0.2.0";

/// Latest version of the user consents metadata schema.
pub const LATEST_USER_CONSENTS_METADATA_VERSION: &str = "0.1.0";

/// Root document for `CoW` Protocol order app-data (schema v1.14.0).
///
/// Every `CoW` Protocol order carries a 32-byte `appData` field that commits
/// to a JSON document describing the order's intent, referral, hooks, and
/// more. `AppDataDoc` is the Rust representation of that JSON document.
///
/// Serialise this struct to canonical JSON with
/// [`appdata_hex`](super::hash::appdata_hex) to obtain the `keccak256` hash
/// used on-chain, or use [`get_app_data_info`](super::ipfs::get_app_data_info)
/// to derive the hash, CID, and canonical JSON in one call.
///
/// Use the builder methods (`with_*`) to attach optional metadata:
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{AppDataDoc, OrderClassKind, Quote, Referrer};
///
/// let doc = AppDataDoc::new("MyDApp")
///     .with_environment("production")
///     .with_referrer(Referrer::new("partner-code"))
///     .with_order_class(OrderClassKind::Limit);
///
/// assert_eq!(doc.app_code.as_deref(), Some("MyDApp"));
/// assert_eq!(doc.environment.as_deref(), Some("production"));
/// assert!(doc.metadata.has_referrer());
/// assert!(doc.metadata.has_order_class());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDataDoc {
    /// Schema version, e.g. `"1.14.0"`.
    pub version: String,
    /// Application identifier, e.g. `"CoW Swap"` or your app name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_code: Option<String>,
    /// Deployment environment, e.g. `"production"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Structured metadata attached to the order.
    pub metadata: Metadata,
}

impl AppDataDoc {
    /// Create a minimal [`AppDataDoc`] with the given `app_code` and no extra
    /// metadata.
    ///
    /// Sets [`version`](Self::version) to [`LATEST_APP_DATA_VERSION`],
    /// `app_code` to the provided value, and [`metadata`](Self::metadata) to
    /// its `Default` (all fields `None`).
    ///
    /// # Parameters
    ///
    /// * `app_code` — application identifier (e.g. `"CoW Swap"`, `"MyDApp"`). Must be ≤ 50
    ///   characters to pass validation.
    ///
    /// # Returns
    ///
    /// A new [`AppDataDoc`] ready to be hashed or further customised with the
    /// `with_*` builder methods.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::AppDataDoc;
    ///
    /// let doc = AppDataDoc::new("MyDApp");
    /// assert_eq!(doc.app_code.as_deref(), Some("MyDApp"));
    /// assert_eq!(doc.version, "1.14.0");
    /// assert!(!doc.metadata.has_referrer());
    /// ```
    #[must_use]
    pub fn new(app_code: impl Into<String>) -> Self {
        Self {
            version: LATEST_APP_DATA_VERSION.to_owned(),
            app_code: Some(app_code.into()),
            environment: None,
            metadata: Metadata::default(),
        }
    }

    /// Set the deployment environment (e.g. `"production"`, `"staging"`).
    ///
    /// The environment string is included in the canonical JSON and therefore
    /// affects the `keccak256` hash. Use it to distinguish orders from
    /// different deployment stages.
    ///
    /// # Parameters
    ///
    /// * `env` — free-form environment label.
    ///
    /// # Returns
    ///
    /// `self` with [`environment`](Self::environment) set.
    #[must_use]
    pub fn with_environment(mut self, env: impl Into<String>) -> Self {
        self.environment = Some(env.into());
        self
    }

    /// Attach a [`Referrer`] code for partner attribution.
    ///
    /// The referral code is embedded in the order's app-data so the protocol
    /// can attribute volume to integration partners.
    ///
    /// # Parameters
    ///
    /// * `referrer` — the [`Referrer`] containing the partner code.
    ///
    /// # Returns
    ///
    /// `self` with [`metadata.referrer`](Metadata::referrer) set.
    #[must_use]
    pub fn with_referrer(mut self, referrer: Referrer) -> Self {
        self.metadata.referrer = Some(referrer);
        self
    }

    /// Attach [`Utm`] campaign tracking parameters.
    ///
    /// UTM parameters (source, medium, campaign, content, term) let analytics
    /// pipelines attribute order volume to marketing campaigns.
    ///
    /// # Parameters
    ///
    /// * `utm` — the [`Utm`] tracking parameters.
    ///
    /// # Returns
    ///
    /// `self` with [`metadata.utm`](Metadata::utm) set.
    #[must_use]
    pub fn with_utm(mut self, utm: Utm) -> Self {
        self.metadata.utm = Some(utm);
        self
    }

    /// Attach pre- and/or post-settlement interaction hooks.
    ///
    /// Hooks are arbitrary contract calls the settlement contract executes
    /// before (`pre`) or after (`post`) the trade. See [`CowHook`] for
    /// details on individual hook entries.
    ///
    /// # Parameters
    ///
    /// * `hooks` — the [`OrderInteractionHooks`] containing pre/post lists.
    ///
    /// # Returns
    ///
    /// `self` with [`metadata.hooks`](Metadata::hooks) set.
    #[must_use]
    pub fn with_hooks(mut self, hooks: OrderInteractionHooks) -> Self {
        self.metadata.hooks = Some(hooks);
        self
    }

    /// Attach a [`PartnerFee`] policy to this order.
    ///
    /// Partner fees are charged by integration partners as a percentage of
    /// the trade. Each fee entry specifies a basis-point rate and a recipient
    /// address.
    ///
    /// # Parameters
    ///
    /// * `fee` — the [`PartnerFee`] (single or multi-entry).
    ///
    /// # Returns
    ///
    /// `self` with [`metadata.partner_fee`](Metadata::partner_fee) set.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::{AppDataDoc, PartnerFee, PartnerFeeEntry};
    ///
    /// let doc = AppDataDoc::new("MyDApp")
    ///     .with_partner_fee(PartnerFee::single(PartnerFeeEntry::volume(50, "0xRecipient")));
    /// assert!(doc.metadata.has_partner_fee());
    /// ```
    #[must_use]
    pub fn with_partner_fee(mut self, fee: PartnerFee) -> Self {
        self.metadata.partner_fee = Some(fee);
        self
    }

    /// Mark this order as replacing a previously submitted order.
    ///
    /// The protocol uses this to link replacement orders for analytics and
    /// to avoid double-fills.
    ///
    /// # Parameters
    ///
    /// * `uid` — the `0x`-prefixed order UID of the order being replaced (56 bytes = `0x` + 112 hex
    ///   chars).
    ///
    /// # Returns
    ///
    /// `self` with [`metadata.replaced_order`](Metadata::replaced_order) set.
    #[must_use]
    pub fn with_replaced_order(mut self, uid: impl Into<String>) -> Self {
        self.metadata.replaced_order = Some(ReplacedOrder { uid: uid.into() });
        self
    }

    /// Attach the signer address for `EIP-1271` or other smart-contract
    /// signers.
    ///
    /// When the order is signed by a smart contract (not an EOA), this field
    /// records the contract address that will validate the signature on-chain.
    ///
    /// # Parameters
    ///
    /// * `signer` — the `0x`-prefixed Ethereum address of the signing contract.
    ///
    /// # Returns
    ///
    /// `self` with [`metadata.signer`](Metadata::signer) set.
    #[must_use]
    pub fn with_signer(mut self, signer: impl Into<String>) -> Self {
        self.metadata.signer = Some(signer.into());
        self
    }

    /// Set the order class kind (`market`, `limit`, `liquidity`, or `twap`).
    ///
    /// Solvers and the protocol UI use this to decide execution strategy and
    /// display. See [`OrderClassKind`] for the available variants.
    ///
    /// # Parameters
    ///
    /// * `kind` — the [`OrderClassKind`] variant.
    ///
    /// # Returns
    ///
    /// `self` with [`metadata.order_class`](Metadata::order_class) set.
    #[must_use]
    pub const fn with_order_class(mut self, kind: OrderClassKind) -> Self {
        self.metadata.order_class = Some(OrderClass { order_class: kind });
        self
    }

    /// Attach cross-chain [`Bridging`] metadata.
    ///
    /// Records which bridge provider was used, the destination chain, and the
    /// destination token address so solvers and analytics can trace
    /// cross-chain flows.
    ///
    /// # Parameters
    ///
    /// * `bridging` — the [`Bridging`] record.
    ///
    /// # Returns
    ///
    /// `self` with [`metadata.bridging`](Metadata::bridging) set.
    #[must_use]
    pub fn with_bridging(mut self, bridging: Bridging) -> Self {
        self.metadata.bridging = Some(bridging);
        self
    }

    /// Attach [`Flashloan`] execution metadata.
    ///
    /// Records the flash-loan parameters (amount, provider, token, adapter,
    /// receiver) so the settlement contract and solvers can reconstruct the
    /// flash-loan flow.
    ///
    /// # Parameters
    ///
    /// * `flashloan` — the [`Flashloan`] record.
    ///
    /// # Returns
    ///
    /// `self` with [`metadata.flashloan`](Metadata::flashloan) set.
    #[must_use]
    pub fn with_flashloan(mut self, flashloan: Flashloan) -> Self {
        self.metadata.flashloan = Some(flashloan);
        self
    }

    /// Attach token [`WrapperEntry`] records.
    ///
    /// Wrapper entries describe token wrapping/unwrapping operations applied
    /// during order execution (e.g. WETH ↔ ETH).
    ///
    /// # Parameters
    ///
    /// * `wrappers` — list of [`WrapperEntry`] records.
    ///
    /// # Returns
    ///
    /// `self` with [`metadata.wrappers`](Metadata::wrappers) set.
    #[must_use]
    pub fn with_wrappers(mut self, wrappers: Vec<WrapperEntry>) -> Self {
        self.metadata.wrappers = Some(wrappers);
        self
    }

    /// Attach [`UserConsent`] records for terms of service acceptance.
    ///
    /// # Parameters
    ///
    /// * `consents` — list of [`UserConsent`] records.
    ///
    /// # Returns
    ///
    /// `self` with [`metadata.user_consents`](Metadata::user_consents) set.
    #[must_use]
    pub fn with_user_consents(mut self, consents: Vec<UserConsent>) -> Self {
        self.metadata.user_consents = Some(consents);
        self
    }
}

impl fmt::Display for AppDataDoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = self.app_code.as_deref().map_or("none", |s| s);
        write!(f, "app-data(v{}, code={})", self.version, code)
    }
}
/// Metadata container — all fields are optional.
///
/// Each field corresponds to a section of the `CoW` Protocol app-data
/// schema. Fields are serialised only when `Some` (via
/// `#[serde(skip_serializing_if = "Option::is_none")]`), keeping the JSON
/// compact.
///
/// Use the builder methods (`with_*`) to populate fields, or the `has_*`
/// predicates to check which fields are set.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{Metadata, Quote, Referrer};
///
/// let meta =
///     Metadata::default().with_referrer(Referrer::new("ref-code")).with_quote(Quote::new(50));
///
/// assert!(meta.has_referrer());
/// assert!(meta.has_quote());
/// assert!(!meta.has_hooks());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    /// Referrer code for partner attribution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referrer: Option<Referrer>,
    /// UTM tracking parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utm: Option<Utm>,
    /// Quote-level slippage settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<Quote>,
    /// Classification of the order intent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_class: Option<OrderClass>,
    /// Pre- and post-interaction hooks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<OrderInteractionHooks>,
    /// Widget metadata when the order originates from a widget integration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widget: Option<Widget>,
    /// Protocol fee charged by an integration partner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partner_fee: Option<PartnerFee>,
    /// UID of a previous order that this order replaces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replaced_order: Option<ReplacedOrder>,
    /// Signer wallet address (for `EIP-1271` or other non-EOA signers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer: Option<String>,
    /// Cross-chain bridging metadata (if the order used a bridge).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridging: Option<Bridging>,
    /// Flash loan metadata (if the order used a flash loan).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flashloan: Option<Flashloan>,
    /// Token wrapper entries applied during execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrappers: Option<Vec<WrapperEntry>>,
    /// User consent records attached to this order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_consents: Option<Vec<UserConsent>>,
}

impl Metadata {
    /// Set the [`Referrer`] tracking code for partner attribution.
    ///
    /// # Parameters
    ///
    /// * `referrer` — the [`Referrer`] containing the partner code.
    ///
    /// # Returns
    ///
    /// `self` with `referrer` set.
    #[must_use]
    pub fn with_referrer(mut self, referrer: Referrer) -> Self {
        self.referrer = Some(referrer);
        self
    }

    /// Set the [`Utm`] campaign tracking parameters.
    ///
    /// # Parameters
    ///
    /// * `utm` — the [`Utm`] parameters (source, medium, campaign, …).
    ///
    /// # Returns
    ///
    /// `self` with `utm` set.
    #[must_use]
    pub fn with_utm(mut self, utm: Utm) -> Self {
        self.utm = Some(utm);
        self
    }

    /// Set the quote-level slippage settings.
    ///
    /// # Parameters
    ///
    /// * `quote` — the [`Quote`] containing the slippage tolerance in basis points and optional
    ///   smart-slippage flag.
    ///
    /// # Returns
    ///
    /// `self` with `quote` set.
    #[must_use]
    pub const fn with_quote(mut self, quote: Quote) -> Self {
        self.quote = Some(quote);
        self
    }

    /// Set the order class classification.
    ///
    /// # Parameters
    ///
    /// * `order_class` — the [`OrderClass`] wrapping an [`OrderClassKind`].
    ///
    /// # Returns
    ///
    /// `self` with `order_class` set.
    #[must_use]
    pub const fn with_order_class(mut self, order_class: OrderClass) -> Self {
        self.order_class = Some(order_class);
        self
    }

    /// Set the pre- and post-settlement interaction hooks.
    ///
    /// # Parameters
    ///
    /// * `hooks` — the [`OrderInteractionHooks`] containing pre/post lists.
    ///
    /// # Returns
    ///
    /// `self` with `hooks` set.
    #[must_use]
    pub fn with_hooks(mut self, hooks: OrderInteractionHooks) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Set the widget integration metadata.
    ///
    /// # Parameters
    ///
    /// * `widget` — the [`Widget`] identifying the widget host.
    ///
    /// # Returns
    ///
    /// `self` with `widget` set.
    #[must_use]
    pub fn with_widget(mut self, widget: Widget) -> Self {
        self.widget = Some(widget);
        self
    }

    /// Set the partner fee policy.
    ///
    /// # Parameters
    ///
    /// * `fee` — the [`PartnerFee`] (single or multi-entry).
    ///
    /// # Returns
    ///
    /// `self` with `partner_fee` set.
    #[must_use]
    pub fn with_partner_fee(mut self, fee: PartnerFee) -> Self {
        self.partner_fee = Some(fee);
        self
    }

    /// Set the replaced-order reference.
    ///
    /// # Parameters
    ///
    /// * `order` — the [`ReplacedOrder`] containing the UID of the order being superseded.
    ///
    /// # Returns
    ///
    /// `self` with `replaced_order` set.
    #[must_use]
    pub fn with_replaced_order(mut self, order: ReplacedOrder) -> Self {
        self.replaced_order = Some(order);
        self
    }

    /// Set the signer address override for smart-contract wallets.
    ///
    /// # Parameters
    ///
    /// * `signer` — `0x`-prefixed Ethereum address of the signing contract.
    ///
    /// # Returns
    ///
    /// `self` with `signer` set.
    #[must_use]
    pub fn with_signer(mut self, signer: impl Into<String>) -> Self {
        self.signer = Some(signer.into());
        self
    }

    /// Set the cross-chain [`Bridging`] metadata.
    ///
    /// # Parameters
    ///
    /// * `bridging` — the [`Bridging`] record.
    ///
    /// # Returns
    ///
    /// `self` with `bridging` set.
    #[must_use]
    pub fn with_bridging(mut self, bridging: Bridging) -> Self {
        self.bridging = Some(bridging);
        self
    }

    /// Set the [`Flashloan`] execution metadata.
    ///
    /// # Parameters
    ///
    /// * `flashloan` — the [`Flashloan`] record.
    ///
    /// # Returns
    ///
    /// `self` with `flashloan` set.
    #[must_use]
    pub fn with_flashloan(mut self, flashloan: Flashloan) -> Self {
        self.flashloan = Some(flashloan);
        self
    }

    /// Set the token [`WrapperEntry`] records.
    ///
    /// # Parameters
    ///
    /// * `wrappers` — list of wrapper entries applied during execution.
    ///
    /// # Returns
    ///
    /// `self` with `wrappers` set.
    #[must_use]
    pub fn with_wrappers(mut self, wrappers: Vec<WrapperEntry>) -> Self {
        self.wrappers = Some(wrappers);
        self
    }

    /// Set the [`UserConsent`] records for terms of service acceptance.
    ///
    /// # Parameters
    ///
    /// * `consents` — list of consent records.
    ///
    /// # Returns
    ///
    /// `self` with `user_consents` set.
    #[must_use]
    pub fn with_user_consents(mut self, consents: Vec<UserConsent>) -> Self {
        self.user_consents = Some(consents);
        self
    }

    /// Returns `true` if a referrer tracking code is set.
    #[must_use]
    pub const fn has_referrer(&self) -> bool {
        self.referrer.is_some()
    }

    /// Returns `true` if `UTM` campaign parameters are set.
    #[must_use]
    pub const fn has_utm(&self) -> bool {
        self.utm.is_some()
    }

    /// Returns `true` if quote-level slippage settings are set.
    #[must_use]
    pub const fn has_quote(&self) -> bool {
        self.quote.is_some()
    }

    /// Returns `true` if an order class classification is set.
    #[must_use]
    pub const fn has_order_class(&self) -> bool {
        self.order_class.is_some()
    }

    /// Returns `true` if pre/post interaction hooks are set.
    #[must_use]
    pub const fn has_hooks(&self) -> bool {
        self.hooks.is_some()
    }

    /// Returns `true` if widget integration metadata is set.
    #[must_use]
    pub const fn has_widget(&self) -> bool {
        self.widget.is_some()
    }

    /// Returns `true` if a partner fee is set.
    #[must_use]
    pub const fn has_partner_fee(&self) -> bool {
        self.partner_fee.is_some()
    }

    /// Returns `true` if a replaced-order reference is set.
    #[must_use]
    pub const fn has_replaced_order(&self) -> bool {
        self.replaced_order.is_some()
    }

    /// Returns `true` if a signer address override is set.
    #[must_use]
    pub const fn has_signer(&self) -> bool {
        self.signer.is_some()
    }

    /// Returns `true` if cross-chain bridging metadata is set.
    #[must_use]
    pub const fn has_bridging(&self) -> bool {
        self.bridging.is_some()
    }

    /// Returns `true` if flash loan metadata is set.
    #[must_use]
    pub const fn has_flashloan(&self) -> bool {
        self.flashloan.is_some()
    }

    /// Returns `true` if at least one token wrapper entry is set.
    #[must_use]
    pub fn has_wrappers(&self) -> bool {
        self.wrappers.as_ref().is_some_and(|v| !v.is_empty())
    }

    /// Returns `true` if at least one user consent record is set.
    #[must_use]
    pub fn has_user_consents(&self) -> bool {
        self.user_consents.as_ref().is_some_and(|v| !v.is_empty())
    }
}

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("metadata")
    }
}

/// Partner referral tracking code.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Referrer {
    /// Opaque referral code string.
    pub code: String,
}

impl Referrer {
    /// Construct a new [`Referrer`] with the given partner referral code.
    ///
    /// The referral code is an opaque string assigned by the `CoW` Protocol
    /// to integration partners. It is embedded in the order's app-data JSON
    /// so the protocol can attribute volume.
    ///
    /// # Parameters
    ///
    /// * `code` — the partner referral code string.
    ///
    /// # Returns
    ///
    /// A new [`Referrer`] instance.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::Referrer;
    ///
    /// let r = Referrer::new("partner-42");
    /// assert_eq!(r.code, "partner-42");
    /// ```
    #[must_use]
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}

impl fmt::Display for Referrer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "referrer({})", self.code)
    }
}

/// UTM campaign tracking parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Utm {
    /// UTM source parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utm_source: Option<String>,
    /// UTM medium parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utm_medium: Option<String>,
    /// UTM campaign parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utm_campaign: Option<String>,
    /// UTM content parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utm_content: Option<String>,
    /// UTM keyword / term parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utm_term: Option<String>,
}

impl Utm {
    /// Construct a [`Utm`] with all fields `None`.
    ///
    /// Use the `with_*` builder methods to populate individual UTM
    /// parameters. Only non-`None` fields are serialised into the JSON.
    ///
    /// # Returns
    ///
    /// An empty [`Utm`] instance.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::Utm;
    ///
    /// let utm = Utm::new().with_source("twitter").with_campaign("launch-2025");
    /// assert!(utm.has_source());
    /// assert!(utm.has_campaign());
    /// assert!(!utm.has_medium());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the `utm_source` parameter (e.g. `"twitter"`, `"google"`).
    ///
    /// # Parameters
    ///
    /// * `source` — the traffic source identifier.
    ///
    /// # Returns
    ///
    /// `self` with `utm_source` set.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.utm_source = Some(source.into());
        self
    }

    /// Set the `utm_medium` parameter (e.g. `"cpc"`, `"email"`).
    ///
    /// # Parameters
    ///
    /// * `medium` — the marketing medium identifier.
    ///
    /// # Returns
    ///
    /// `self` with `utm_medium` set.
    #[must_use]
    pub fn with_medium(mut self, medium: impl Into<String>) -> Self {
        self.utm_medium = Some(medium.into());
        self
    }

    /// Set the `utm_campaign` parameter (e.g. `"launch-2025"`).
    ///
    /// # Parameters
    ///
    /// * `campaign` — the campaign name.
    ///
    /// # Returns
    ///
    /// `self` with `utm_campaign` set.
    #[must_use]
    pub fn with_campaign(mut self, campaign: impl Into<String>) -> Self {
        self.utm_campaign = Some(campaign.into());
        self
    }

    /// Set the `utm_content` parameter for A/B testing or ad variants.
    ///
    /// # Parameters
    ///
    /// * `content` — the content variant identifier.
    ///
    /// # Returns
    ///
    /// `self` with `utm_content` set.
    #[must_use]
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.utm_content = Some(content.into());
        self
    }

    /// Set the `utm_term` parameter for paid search keywords.
    ///
    /// # Parameters
    ///
    /// * `term` — the search keyword or term.
    ///
    /// # Returns
    ///
    /// `self` with `utm_term` set.
    #[must_use]
    pub fn with_term(mut self, term: impl Into<String>) -> Self {
        self.utm_term = Some(term.into());
        self
    }

    /// Returns `true` if the `utm_source` parameter is set.
    #[must_use]
    pub const fn has_source(&self) -> bool {
        self.utm_source.is_some()
    }

    /// Returns `true` if the `utm_medium` parameter is set.
    #[must_use]
    pub const fn has_medium(&self) -> bool {
        self.utm_medium.is_some()
    }

    /// Returns `true` if the `utm_campaign` parameter is set.
    #[must_use]
    pub const fn has_campaign(&self) -> bool {
        self.utm_campaign.is_some()
    }

    /// Returns `true` if the `utm_content` parameter is set.
    #[must_use]
    pub const fn has_content(&self) -> bool {
        self.utm_content.is_some()
    }

    /// Returns `true` if the `utm_term` parameter is set.
    #[must_use]
    pub const fn has_term(&self) -> bool {
        self.utm_term.is_some()
    }
}

impl fmt::Display for Utm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let src = self.utm_source.as_deref().map_or("none", |s| s);
        write!(f, "utm(source={src})")
    }
}

/// Quote-level slippage settings embedded in app-data.
///
/// Records the slippage tolerance the user chose when placing the order, so
/// solvers and analytics can reconstruct the original intent.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::Quote;
///
/// // 0.5 % slippage with smart slippage enabled
/// let quote = Quote::new(50).with_smart_slippage();
/// assert_eq!(quote.slippage_bips, 50);
/// assert_eq!(quote.smart_slippage, Some(true));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// Slippage tolerance in basis points (e.g. `50` = 0.5 %).
    pub slippage_bips: u32,
    /// Whether smart (dynamic per-trade) slippage is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smart_slippage: Option<bool>,
}

impl Quote {
    /// Construct a [`Quote`] with the given slippage tolerance.
    ///
    /// # Parameters
    ///
    /// * `slippage_bips` — slippage tolerance in basis points. `50` = 0.5 %, `100` = 1 %, `10_000`
    ///   = 100 %.
    ///
    /// # Returns
    ///
    /// A new [`Quote`] with `smart_slippage` set to `None` (disabled).
    /// Chain [`with_smart_slippage`](Self::with_smart_slippage) to enable it.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::Quote;
    ///
    /// let q = Quote::new(50);
    /// assert_eq!(q.slippage_bips, 50);
    /// assert_eq!(q.smart_slippage, None);
    /// ```
    #[must_use]
    pub const fn new(slippage_bips: u32) -> Self {
        Self { slippage_bips, smart_slippage: None }
    }

    /// Enable dynamic (smart) slippage adjustment.
    ///
    /// When enabled, the protocol may adjust the slippage tolerance
    /// per-trade based on market conditions rather than using a fixed value.
    ///
    /// # Returns
    ///
    /// `self` with `smart_slippage` set to `Some(true)`.
    #[must_use]
    pub const fn with_smart_slippage(mut self) -> Self {
        self.smart_slippage = Some(true);
        self
    }
}

impl fmt::Display for Quote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "quote({}bips)", self.slippage_bips)
    }
}

/// High-level classification of the order's intent.
///
/// Solvers and the protocol UI use this to decide execution strategy and
/// display. The variant is serialised as a `camelCase` string in the JSON
/// document (e.g. `"market"`, `"twap"`).
///
/// # Example
///
/// ```
/// use cow_rs::app_data::OrderClassKind;
///
/// let kind = OrderClassKind::Limit;
/// assert_eq!(kind.as_str(), "limit");
/// assert!(kind.is_limit());
/// assert!(!kind.is_market());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderClassKind {
    /// Standard market order.
    Market,
    /// Limit order with a price constraint.
    Limit,
    /// Programmatic liquidity order.
    Liquidity,
    /// Time-Weighted Average Price order.
    Twap,
}

impl OrderClassKind {
    /// Returns the camelCase string used by the `CoW` Protocol schema.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::Limit => "limit",
            Self::Liquidity => "liquidity",
            Self::Twap => "twap",
        }
    }

    /// Returns `true` if this is a market order class.
    #[must_use]
    pub const fn is_market(self) -> bool {
        matches!(self, Self::Market)
    }

    /// Returns `true` if this is a limit order class.
    #[must_use]
    pub const fn is_limit(self) -> bool {
        matches!(self, Self::Limit)
    }

    /// Returns `true` if this is a liquidity order class.
    #[must_use]
    pub const fn is_liquidity(self) -> bool {
        matches!(self, Self::Liquidity)
    }

    /// Returns `true` if this is a `TWAP` order class.
    #[must_use]
    pub const fn is_twap(self) -> bool {
        matches!(self, Self::Twap)
    }
}

impl fmt::Display for OrderClassKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for OrderClassKind {
    type Error = crate::CowError;

    /// Parse an [`OrderClassKind`] from the `CoW` Protocol schema string.
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "market" => Ok(Self::Market),
            "limit" => Ok(Self::Limit),
            "liquidity" => Ok(Self::Liquidity),
            "twap" => Ok(Self::Twap),
            other => Err(crate::CowError::Parse {
                field: "OrderClassKind",
                reason: format!("unknown value: {other}"),
            }),
        }
    }
}

/// Wrapper for [`OrderClassKind`] as it appears in the metadata schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderClass {
    /// Order classification kind.
    pub order_class: OrderClassKind,
}

impl OrderClass {
    /// Construct an [`OrderClass`] from an [`OrderClassKind`].
    ///
    /// This is a thin wrapper — [`OrderClass`] exists because the JSON
    /// schema nests the classification under `{ "orderClass": "market" }`
    /// rather than using the enum value directly.
    ///
    /// # Parameters
    ///
    /// * `order_class` — the [`OrderClassKind`] variant.
    ///
    /// # Returns
    ///
    /// A new [`OrderClass`] wrapping the given kind.
    #[must_use]
    pub const fn new(order_class: OrderClassKind) -> Self {
        Self { order_class }
    }
}

impl fmt::Display for OrderClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.order_class, f)
    }
}

/// Pre- and post-settlement interaction hooks.
///
/// Contains optional lists of [`CowHook`] entries that the settlement
/// contract will execute before (`pre`) and after (`post`) the trade.
/// When both lists are empty, the field is typically omitted from the JSON.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{CowHook, OrderInteractionHooks};
///
/// let pre_hook =
///     CowHook::new("0x1234567890abcdef1234567890abcdef12345678", "0xabcdef00", "50000");
/// let hooks = OrderInteractionHooks::new(vec![pre_hook], vec![]);
/// assert!(hooks.has_pre());
/// assert!(!hooks.has_post());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInteractionHooks {
    /// Hook schema version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Hooks executed before the settlement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre: Option<Vec<CowHook>>,
    /// Hooks executed after the settlement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Vec<CowHook>>,
}

impl OrderInteractionHooks {
    /// Create hooks with the given pre- and post-execution lists.
    ///
    /// Empty vectors are stored as `None` (omitted from JSON) rather than
    /// as empty arrays, matching the `TypeScript` SDK's behaviour.
    ///
    /// # Parameters
    ///
    /// * `pre` — hooks to execute **before** the settlement trade.
    /// * `post` — hooks to execute **after** the settlement trade.
    ///
    /// # Returns
    ///
    /// A new [`OrderInteractionHooks`] with `version` set to `None`.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::{CowHook, OrderInteractionHooks};
    ///
    /// let pre =
    ///     vec![CowHook::new("0x1234567890abcdef1234567890abcdef12345678", "0x095ea7b3", "50000")];
    /// let hooks = OrderInteractionHooks::new(pre, vec![]);
    /// assert!(hooks.has_pre());
    /// assert!(!hooks.has_post());
    /// ```
    #[must_use]
    pub fn new(pre: Vec<CowHook>, post: Vec<CowHook>) -> Self {
        Self {
            version: None,
            pre: if pre.is_empty() { None } else { Some(pre) },
            post: if post.is_empty() { None } else { Some(post) },
        }
    }

    /// Override the hook schema version.
    ///
    /// # Parameters
    ///
    /// * `version` — the hook schema version string (e.g. `"0.2.0"`).
    ///
    /// # Returns
    ///
    /// `self` with `version` set.
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Returns `true` if at least one pre-settlement hook is set.
    #[must_use]
    pub fn has_pre(&self) -> bool {
        self.pre.as_ref().is_some_and(|v| !v.is_empty())
    }

    /// Returns `true` if at least one post-settlement hook is set.
    #[must_use]
    pub fn has_post(&self) -> bool {
        self.post.as_ref().is_some_and(|v| !v.is_empty())
    }
}

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
/// use cow_rs::app_data::CowHook;
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
    ///
    /// # Parameters
    ///
    /// * `target` — the contract address to call. Must be a valid Ethereum address (`0x` + 40 hex
    ///   chars) to pass validation.
    /// * `call_data` — ABI-encoded function selector and arguments (`0x`-prefixed hex string).
    /// * `gas_limit` — maximum gas this hook may consume, as a decimal string (e.g. `"100000"`).
    ///   Must parse as `u64` to pass validation.
    ///
    /// # Returns
    ///
    /// A new [`CowHook`] with `dapp_id` set to `None`.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::CowHook;
    ///
    /// let hook = CowHook::new("0x1234567890abcdef1234567890abcdef12345678", "0x095ea7b3", "50000");
    /// assert_eq!(hook.gas_limit, "50000");
    /// assert!(!hook.has_dapp_id());
    /// ```
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
    ///
    /// The dApp ID is an opaque string identifying the application that
    /// registered this hook, useful for analytics and debugging.
    ///
    /// # Parameters
    ///
    /// * `dapp_id` — the dApp identifier string.
    ///
    /// # Returns
    ///
    /// `self` with `dapp_id` set.
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
/// Widget integration metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Widget {
    /// App code of the widget host.
    pub app_code: String,
    /// Deployment environment of the widget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
}

impl Widget {
    /// Construct a new [`Widget`] for the given app code.
    ///
    /// Used when an order originates from an embedded widget integration.
    /// The `app_code` identifies the widget host application.
    ///
    /// # Parameters
    ///
    /// * `app_code` — the widget host's application identifier.
    ///
    /// # Returns
    ///
    /// A new [`Widget`] with `environment` set to `None`.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::Widget;
    ///
    /// let w = Widget::new("WidgetHost").with_environment("production");
    /// assert_eq!(w.app_code, "WidgetHost");
    /// assert!(w.has_environment());
    /// ```
    #[must_use]
    pub fn new(app_code: impl Into<String>) -> Self {
        Self { app_code: app_code.into(), environment: None }
    }

    /// Attach a deployment environment string (e.g. `"production"`).
    ///
    /// # Parameters
    ///
    /// * `env` — free-form environment label.
    ///
    /// # Returns
    ///
    /// `self` with `environment` set.
    #[must_use]
    pub fn with_environment(mut self, env: impl Into<String>) -> Self {
        self.environment = Some(env.into());
        self
    }

    /// Returns `true` if a deployment environment string is set.
    #[must_use]
    pub const fn has_environment(&self) -> bool {
        self.environment.is_some()
    }
}

impl fmt::Display for Widget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "widget({})", self.app_code)
    }
}

/// A single partner fee policy entry (schema v1.14.0).
///
/// Exactly one of `volume_bps`, `surplus_bps`, or `price_improvement_bps`
/// should be set; the other two should be `None`. Use the named
/// constructors [`volume`](Self::volume), [`surplus`](Self::surplus), or
/// [`price_improvement`](Self::price_improvement) to enforce this invariant.
///
/// All basis-point values must be ≤ 10 000 (= 100 %). Values above that
/// threshold will be flagged by [`validate_app_data_doc`](super::ipfs::validate_app_data_doc).
///
/// # Example
///
/// ```
/// use cow_rs::app_data::PartnerFeeEntry;
///
/// // 0.5 % volume-based fee
/// let fee = PartnerFeeEntry::volume(50, "0xRecipientAddress");
/// assert_eq!(fee.volume_bps(), Some(50));
/// assert_eq!(fee.surplus_bps(), None);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnerFeeEntry {
    /// Volume-based fee in basis points of the sell amount.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_bps: Option<u32>,
    /// Surplus-based fee in basis points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surplus_bps: Option<u32>,
    /// Price-improvement fee in basis points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_improvement_bps: Option<u32>,
    /// Volume cap in basis points (required for surplus/price-improvement variants).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_volume_bps: Option<u32>,
    /// Address that receives the fee.
    pub recipient: String,
}

impl PartnerFeeEntry {
    /// Construct a volume-based fee entry.
    ///
    /// The fee is charged as a percentage of the sell amount. This is the
    /// most common fee model for integration partners.
    ///
    /// # Parameters
    ///
    /// * `volume_bps` — fee rate in basis points (e.g. `50` = 0.5 %). Must be ≤ 10 000 to pass
    ///   validation.
    /// * `recipient` — the `0x`-prefixed Ethereum address that receives the fee.
    ///
    /// # Returns
    ///
    /// A new [`PartnerFeeEntry`] with only `volume_bps` set.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::PartnerFeeEntry;
    ///
    /// let fee = PartnerFeeEntry::volume(50, "0xRecipient");
    /// assert_eq!(fee.volume_bps(), Some(50));
    /// assert_eq!(fee.surplus_bps(), None);
    /// assert_eq!(fee.max_volume_bps(), None);
    /// ```
    #[must_use]
    pub fn volume(volume_bps: u32, recipient: impl Into<String>) -> Self {
        Self {
            volume_bps: Some(volume_bps),
            surplus_bps: None,
            price_improvement_bps: None,
            max_volume_bps: None,
            recipient: recipient.into(),
        }
    }

    /// Construct a surplus-based fee entry.
    ///
    /// The fee is charged as a percentage of the surplus (the difference
    /// between the execution price and the limit price). A `max_volume_bps`
    /// cap is required to bound the fee as a percentage of the sell amount.
    ///
    /// # Parameters
    ///
    /// * `surplus_bps` — fee rate in basis points on the surplus.
    /// * `max_volume_bps` — cap on the fee as a percentage of sell amount.
    /// * `recipient` — the `0x`-prefixed Ethereum address that receives the fee.
    ///
    /// # Returns
    ///
    /// A new [`PartnerFeeEntry`] with `surplus_bps` and `max_volume_bps` set.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::PartnerFeeEntry;
    ///
    /// let fee = PartnerFeeEntry::surplus(30, 100, "0xRecipient");
    /// assert_eq!(fee.surplus_bps(), Some(30));
    /// assert_eq!(fee.max_volume_bps(), Some(100));
    /// ```
    #[must_use]
    pub fn surplus(surplus_bps: u32, max_volume_bps: u32, recipient: impl Into<String>) -> Self {
        Self {
            volume_bps: None,
            surplus_bps: Some(surplus_bps),
            price_improvement_bps: None,
            max_volume_bps: Some(max_volume_bps),
            recipient: recipient.into(),
        }
    }

    /// Construct a price-improvement fee entry.
    ///
    /// The fee is charged as a percentage of the price improvement the
    /// solver achieved. A `max_volume_bps` cap is required to bound the
    /// fee as a percentage of the sell amount.
    ///
    /// # Parameters
    ///
    /// * `price_improvement_bps` — fee rate in basis points on the price improvement.
    /// * `max_volume_bps` — cap on the fee as a percentage of sell amount.
    /// * `recipient` — the `0x`-prefixed Ethereum address that receives the fee.
    ///
    /// # Returns
    ///
    /// A new [`PartnerFeeEntry`] with `price_improvement_bps` and
    /// `max_volume_bps` set.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::PartnerFeeEntry;
    ///
    /// let fee = PartnerFeeEntry::price_improvement(20, 80, "0xRecipient");
    /// assert_eq!(fee.price_improvement_bps(), Some(20));
    /// assert_eq!(fee.max_volume_bps(), Some(80));
    /// assert_eq!(fee.volume_bps(), None);
    /// ```
    #[must_use]
    pub fn price_improvement(
        price_improvement_bps: u32,
        max_volume_bps: u32,
        recipient: impl Into<String>,
    ) -> Self {
        Self {
            volume_bps: None,
            surplus_bps: None,
            price_improvement_bps: Some(price_improvement_bps),
            max_volume_bps: Some(max_volume_bps),
            recipient: recipient.into(),
        }
    }

    /// Extract the volume fee in basis points, if present.
    ///
    /// # Returns
    ///
    /// `Some(bps)` if this is a volume-based fee entry, `None` otherwise.
    #[must_use]
    pub const fn volume_bps(&self) -> Option<u32> {
        self.volume_bps
    }

    /// Extract the surplus fee in basis points, if present.
    ///
    /// # Returns
    ///
    /// `Some(bps)` if this is a surplus-based fee entry, `None` otherwise.
    #[must_use]
    pub const fn surplus_bps(&self) -> Option<u32> {
        self.surplus_bps
    }

    /// Extract the price-improvement fee in basis points, if present.
    ///
    /// # Returns
    ///
    /// `Some(bps)` if this is a price-improvement fee entry, `None` otherwise.
    #[must_use]
    pub const fn price_improvement_bps(&self) -> Option<u32> {
        self.price_improvement_bps
    }

    /// Extract the max-volume cap in basis points, if present.
    ///
    /// # Returns
    ///
    /// `Some(bps)` for surplus/price-improvement entries, `None` for volume entries.
    #[must_use]
    pub const fn max_volume_bps(&self) -> Option<u32> {
        self.max_volume_bps
    }
}

impl fmt::Display for PartnerFeeEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(bps) = self.volume_bps {
            write!(f, "volume-fee({}bps, {})", bps, self.recipient)
        } else if let Some(bps) = self.surplus_bps {
            write!(f, "surplus-fee({}bps, {})", bps, self.recipient)
        } else if let Some(bps) = self.price_improvement_bps {
            write!(f, "price-improvement-fee({}bps, {})", bps, self.recipient)
        } else {
            write!(f, "fee({})", self.recipient)
        }
    }
}
/// Partner fee attached to a `CoW` Protocol order (schema v1.14.0).
///
/// Can be a single [`PartnerFeeEntry`] or a list of entries. The most common
/// form is a single volume-based entry: `PartnerFee::single(PartnerFeeEntry::volume(50, "0x..."))`.
///
/// Use [`get_partner_fee_bps`] to extract the first `volumeBps` value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PartnerFee {
    /// A single fee policy.
    Single(PartnerFeeEntry),
    /// A list of fee policies (one per fee type).
    Multiple(Vec<PartnerFeeEntry>),
}

impl PartnerFee {
    /// Convenience constructor for the common single-entry case.
    ///
    /// Most integrations use a single fee policy. Wrap a
    /// [`PartnerFeeEntry`] in [`PartnerFee::Single`] for ergonomic use.
    ///
    /// # Parameters
    ///
    /// * `entry` — the fee policy entry (use [`PartnerFeeEntry::volume`],
    ///   [`PartnerFeeEntry::surplus`], or [`PartnerFeeEntry::price_improvement`] to create one).
    ///
    /// # Returns
    ///
    /// A [`PartnerFee::Single`] wrapping the given entry.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::{PartnerFee, PartnerFeeEntry};
    ///
    /// let fee = PartnerFee::single(PartnerFeeEntry::volume(50, "0xAddr"));
    /// assert!(fee.is_single());
    /// assert_eq!(fee.count(), 1);
    /// ```
    #[must_use]
    pub const fn single(entry: PartnerFeeEntry) -> Self {
        Self::Single(entry)
    }

    /// Iterate over all fee entries.
    ///
    /// Returns a single-element iterator for [`Single`](Self::Single), or
    /// iterates the full vector for [`Multiple`](Self::Multiple).
    ///
    /// # Returns
    ///
    /// An iterator yielding `&PartnerFeeEntry` references.
    pub fn entries(&self) -> impl Iterator<Item = &PartnerFeeEntry> {
        match self {
            Self::Single(e) => std::slice::from_ref(e).iter(),
            Self::Multiple(v) => v.iter(),
        }
    }

    /// Returns `true` if this is a single-entry partner fee.
    #[must_use]
    pub const fn is_single(&self) -> bool {
        matches!(self, Self::Single(_))
    }

    /// Returns `true` if this is a multi-entry partner fee.
    #[must_use]
    pub const fn is_multiple(&self) -> bool {
        matches!(self, Self::Multiple(_))
    }

    /// Returns the number of fee entries: `1` for [`Single`](Self::Single),
    /// or the vector length for [`Multiple`](Self::Multiple).
    ///
    /// ```
    /// use cow_rs::app_data::{PartnerFee, PartnerFeeEntry};
    ///
    /// let fee = PartnerFee::single(PartnerFeeEntry::volume(50, "0x1234"));
    /// assert_eq!(fee.count(), 1);
    ///
    /// let multi = PartnerFee::Multiple(vec![
    ///     PartnerFeeEntry::volume(50, "0x1234"),
    ///     PartnerFeeEntry::surplus(30, 100, "0x5678"),
    /// ]);
    /// assert_eq!(multi.count(), 2);
    /// ```
    #[must_use]
    pub const fn count(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Multiple(v) => v.len(),
        }
    }
}

impl fmt::Display for PartnerFee {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(e) => fmt::Display::fmt(e, f),
            Self::Multiple(v) => write!(f, "fees({})", v.len()),
        }
    }
}
/// Extract the first `volumeBps` value from an optional [`PartnerFee`].
///
/// Iterates over the fee entries and returns the first
/// [`PartnerFeeEntry::volume_bps`] that is `Some`. Returns `None` if `fee`
/// is `None` or no entry has a volume-based fee.
///
/// Mirrors `getPartnerFeeBps` from the `@cowprotocol/app-data` `TypeScript`
/// package.
///
/// # Parameters
///
/// * `fee` — optional reference to a [`PartnerFee`].
///
/// # Returns
///
/// The first volume-based fee in basis points, or `None`.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::{PartnerFee, PartnerFeeEntry, get_partner_fee_bps};
///
/// let fee = PartnerFee::single(PartnerFeeEntry::volume(50, "0x1234"));
/// assert_eq!(get_partner_fee_bps(Some(&fee)), Some(50));
/// assert_eq!(get_partner_fee_bps(None), None);
/// ```
#[must_use]
pub fn get_partner_fee_bps(fee: Option<&PartnerFee>) -> Option<u32> {
    fee?.entries().find_map(PartnerFeeEntry::volume_bps)
}

/// Links this order to a previously submitted order it supersedes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacedOrder {
    /// UID of the order being replaced.
    pub uid: String,
}

impl ReplacedOrder {
    /// Construct a [`ReplacedOrder`] reference from the given order UID.
    ///
    /// # Parameters
    ///
    /// * `uid` — the `0x`-prefixed order UID of the order being replaced. Must be 56 bytes (`0x` +
    ///   112 hex chars) to pass validation.
    ///
    /// # Returns
    ///
    /// A new [`ReplacedOrder`] instance.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::ReplacedOrder;
    ///
    /// let uid = format!("0x{}", "ab".repeat(56)); // 0x + 112 hex chars
    /// let ro = ReplacedOrder::new(&uid);
    /// assert_eq!(ro.uid.len(), 114);
    /// ```
    #[must_use]
    pub fn new(uid: impl Into<String>) -> Self {
        Self { uid: uid.into() }
    }
}

impl fmt::Display for ReplacedOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "replaced({})", self.uid)
    }
}

/// Cross-chain bridging metadata.
///
/// Embedded in [`Metadata`] when an order was placed via a cross-chain
/// bridge (e.g. Across, Bungee). Records the bridge provider, destination
/// chain, destination token, and optional quote/attestation data so solvers
/// and analytics can trace cross-chain flows.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::types::Bridging;
///
/// let bridging = Bridging::new("across", "42161", "0xTokenOnArbitrum").with_quote_id("quote-123");
/// assert!(bridging.has_quote_id());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bridging {
    /// Bridge provider identifier.
    pub provider: String,
    /// Destination chain ID (as a decimal string).
    pub destination_chain_id: String,
    /// Destination token contract address.
    pub destination_token_address: String,
    /// Bridge quote identifier, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_id: Option<String>,
    /// Bridge quote signature bytes, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_signature: Option<String>,
    /// Bridge attestation signature bytes, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation_signature: Option<String>,
    /// Opaque bridge quote body, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_body: Option<String>,
}

impl Bridging {
    /// Construct a [`Bridging`] record with the three required fields.
    ///
    /// Optional fields (quote ID, signatures, quote body) can be attached
    /// afterwards via the `with_*` builder methods.
    ///
    /// # Parameters
    ///
    /// * `provider` — bridge provider identifier (e.g. `"across"`, `"bungee"`).
    /// * `destination_chain_id` — target chain ID as a decimal string (e.g. `"42161"` for Arbitrum
    ///   One).
    /// * `destination_token_address` — `0x`-prefixed contract address of the token on the
    ///   destination chain.
    ///
    /// # Returns
    ///
    /// A new [`Bridging`] with all optional fields set to `None`.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::types::Bridging;
    ///
    /// let b = Bridging::new("across", "42161", "0xTokenOnArbitrum");
    /// assert_eq!(b.provider, "across");
    /// assert!(!b.has_quote_id());
    /// ```
    #[must_use]
    pub fn new(
        provider: impl Into<String>,
        destination_chain_id: impl Into<String>,
        destination_token_address: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            destination_chain_id: destination_chain_id.into(),
            destination_token_address: destination_token_address.into(),
            quote_id: None,
            quote_signature: None,
            attestation_signature: None,
            quote_body: None,
        }
    }

    /// Attach a bridge quote identifier.
    ///
    /// # Parameters
    ///
    /// * `id` — the quote identifier returned by the bridge provider.
    ///
    /// # Returns
    ///
    /// `self` with `quote_id` set.
    #[must_use]
    pub fn with_quote_id(mut self, id: impl Into<String>) -> Self {
        self.quote_id = Some(id.into());
        self
    }

    /// Attach a bridge quote signature.
    ///
    /// # Parameters
    ///
    /// * `sig` — the hex-encoded signature bytes from the bridge provider.
    ///
    /// # Returns
    ///
    /// `self` with `quote_signature` set.
    #[must_use]
    pub fn with_quote_signature(mut self, sig: impl Into<String>) -> Self {
        self.quote_signature = Some(sig.into());
        self
    }

    /// Attach an attestation signature.
    ///
    /// # Parameters
    ///
    /// * `sig` — the hex-encoded attestation signature bytes.
    ///
    /// # Returns
    ///
    /// `self` with `attestation_signature` set.
    #[must_use]
    pub fn with_attestation_signature(mut self, sig: impl Into<String>) -> Self {
        self.attestation_signature = Some(sig.into());
        self
    }

    /// Attach an opaque bridge quote body.
    ///
    /// # Parameters
    ///
    /// * `body` — the raw quote body string from the bridge provider.
    ///
    /// # Returns
    ///
    /// `self` with `quote_body` set.
    #[must_use]
    pub fn with_quote_body(mut self, body: impl Into<String>) -> Self {
        self.quote_body = Some(body.into());
        self
    }

    /// Returns `true` if a bridge quote identifier is set.
    #[must_use]
    pub const fn has_quote_id(&self) -> bool {
        self.quote_id.is_some()
    }

    /// Returns `true` if a bridge quote signature is set.
    #[must_use]
    pub const fn has_quote_signature(&self) -> bool {
        self.quote_signature.is_some()
    }

    /// Returns `true` if an attestation signature is set.
    #[must_use]
    pub const fn has_attestation_signature(&self) -> bool {
        self.attestation_signature.is_some()
    }

    /// Returns `true` if an opaque quote body is set.
    #[must_use]
    pub const fn has_quote_body(&self) -> bool {
        self.quote_body.is_some()
    }
}

impl fmt::Display for Bridging {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bridge({}, chain={})", self.provider, self.destination_chain_id)
    }
}

/// Flash loan metadata.
///
/// Embedded in [`Metadata`] when the order uses a flash loan for execution.
/// Records the loan amount, liquidity provider, protocol adapter, receiver,
/// and token address so the settlement contract and solvers can reconstruct
/// the flash-loan flow.
///
/// # Example
///
/// ```
/// use cow_rs::app_data::types::Flashloan;
///
/// let fl = Flashloan::new(
///     "1000000000000000000", // 1 ETH in wei
///     "0xLiquidityProvider",
///     "0xTokenAddress",
/// )
/// .with_protocol_adapter("0xAdapterAddress")
/// .with_receiver("0xReceiverAddress");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Flashloan {
    /// Loan amount in token atoms (decimal string).
    pub loan_amount: String,
    /// Address of the liquidity provider.
    pub liquidity_provider_address: String,
    /// Address of the protocol adapter contract.
    pub protocol_adapter_address: String,
    /// Address that receives the flash loan proceeds.
    pub receiver_address: String,
    /// Address of the token being flash-loaned.
    pub token_address: String,
}

impl Flashloan {
    /// Construct a [`Flashloan`] record with the core required fields.
    ///
    /// `protocol_adapter_address` and `receiver_address` default to empty
    /// strings; set them via [`with_protocol_adapter`](Self::with_protocol_adapter)
    /// and [`with_receiver`](Self::with_receiver).
    ///
    /// # Parameters
    ///
    /// * `loan_amount` — the flash-loan amount in token atoms (decimal string, e.g.
    ///   `"1000000000000000000"` for 1 ETH).
    /// * `liquidity_provider_address` — `0x`-prefixed address of the liquidity pool providing the
    ///   flash loan.
    /// * `token_address` — `0x`-prefixed address of the token being flash-loaned.
    ///
    /// # Returns
    ///
    /// A new [`Flashloan`] with adapter and receiver addresses empty.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::types::Flashloan;
    ///
    /// let fl = Flashloan::new("1000000000000000000", "0xPool", "0xToken")
    ///     .with_protocol_adapter("0xAdapter")
    ///     .with_receiver("0xReceiver");
    /// assert_eq!(fl.loan_amount, "1000000000000000000");
    /// assert_eq!(fl.protocol_adapter_address, "0xAdapter");
    /// ```
    #[must_use]
    pub fn new(
        loan_amount: impl Into<String>,
        liquidity_provider_address: impl Into<String>,
        token_address: impl Into<String>,
    ) -> Self {
        Self {
            loan_amount: loan_amount.into(),
            liquidity_provider_address: liquidity_provider_address.into(),
            protocol_adapter_address: String::new(),
            receiver_address: String::new(),
            token_address: token_address.into(),
        }
    }

    /// Set the protocol adapter contract address.
    ///
    /// The adapter contract mediates between the settlement contract and the
    /// flash-loan liquidity provider.
    ///
    /// # Parameters
    ///
    /// * `address` — `0x`-prefixed Ethereum address.
    ///
    /// # Returns
    ///
    /// `self` with `protocol_adapter_address` set.
    #[must_use]
    pub fn with_protocol_adapter(mut self, address: impl Into<String>) -> Self {
        self.protocol_adapter_address = address.into();
        self
    }

    /// Set the receiver address for flash loan proceeds.
    ///
    /// # Parameters
    ///
    /// * `address` — `0x`-prefixed Ethereum address that receives the borrowed tokens.
    ///
    /// # Returns
    ///
    /// `self` with `receiver_address` set.
    #[must_use]
    pub fn with_receiver(mut self, address: impl Into<String>) -> Self {
        self.receiver_address = address.into();
        self
    }
}

impl fmt::Display for Flashloan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "flashloan({}, amount={})", self.token_address, self.loan_amount)
    }
}

/// A single token wrapper entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WrapperEntry {
    /// Address of the wrapper contract.
    pub wrapper_address: String,
    /// Optional wrapper-specific data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrapper_data: Option<String>,
    /// Whether this wrapper can be omitted if not needed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_omittable: Option<bool>,
}

impl WrapperEntry {
    /// Construct a [`WrapperEntry`] with just the wrapper contract address.
    ///
    /// Wrapper entries describe token wrapping/unwrapping operations applied
    /// during order execution (e.g. WETH ↔ ETH).
    ///
    /// # Parameters
    ///
    /// * `wrapper_address` — `0x`-prefixed address of the wrapper contract.
    ///
    /// # Returns
    ///
    /// A new [`WrapperEntry`] with `wrapper_data` and `is_omittable` unset.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::types::WrapperEntry;
    ///
    /// let w = WrapperEntry::new("0xWrapperContract").with_is_omittable(true);
    /// assert!(w.is_omittable());
    /// ```
    #[must_use]
    pub fn new(wrapper_address: impl Into<String>) -> Self {
        Self { wrapper_address: wrapper_address.into(), wrapper_data: None, is_omittable: None }
    }

    /// Attach wrapper-specific data (e.g. ABI-encoded parameters).
    ///
    /// # Parameters
    ///
    /// * `data` — opaque data string specific to the wrapper contract.
    ///
    /// # Returns
    ///
    /// `self` with `wrapper_data` set.
    #[must_use]
    pub fn with_wrapper_data(mut self, data: impl Into<String>) -> Self {
        self.wrapper_data = Some(data.into());
        self
    }

    /// Mark this wrapper as omittable when not needed.
    ///
    /// When `true`, the settlement contract may skip this wrapper if the
    /// wrapping/unwrapping step is unnecessary for the specific execution
    /// path.
    ///
    /// # Parameters
    ///
    /// * `omittable` — whether the wrapper can be skipped.
    ///
    /// # Returns
    ///
    /// `self` with `is_omittable` set.
    #[must_use]
    pub const fn with_is_omittable(mut self, omittable: bool) -> Self {
        self.is_omittable = Some(omittable);
        self
    }

    /// Returns `true` if wrapper-specific data is attached.
    #[must_use]
    pub const fn has_wrapper_data(&self) -> bool {
        self.wrapper_data.is_some()
    }

    /// Returns `true` if the omittable flag is explicitly set.
    #[must_use]
    pub const fn has_is_omittable(&self) -> bool {
        self.is_omittable.is_some()
    }

    /// Returns `true` if this wrapper is explicitly marked as omittable.
    #[must_use]
    pub const fn is_omittable(&self) -> bool {
        matches!(self.is_omittable, Some(true))
    }
}

impl fmt::Display for WrapperEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "wrapper({})", self.wrapper_address)
    }
}

/// User acceptance record for terms of service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserConsent {
    /// Identifier or URL of the accepted terms.
    pub terms: String,
    /// ISO 8601 date when the terms were accepted.
    pub accepted_date: String,
}

impl UserConsent {
    /// Construct a [`UserConsent`] record for terms-of-service acceptance.
    ///
    /// # Parameters
    ///
    /// * `terms` — identifier or URL of the accepted terms of service.
    /// * `accepted_date` — ISO 8601 date string when the terms were accepted (e.g. `"2025-04-07"`).
    ///
    /// # Returns
    ///
    /// A new [`UserConsent`] instance.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::app_data::types::UserConsent;
    ///
    /// let consent = UserConsent::new("https://cow.fi/tos", "2025-04-07");
    /// assert_eq!(consent.terms, "https://cow.fi/tos");
    /// ```
    #[must_use]
    pub fn new(terms: impl Into<String>, accepted_date: impl Into<String>) -> Self {
        Self { terms: terms.into(), accepted_date: accepted_date.into() }
    }
}

impl fmt::Display for UserConsent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "consent({}, {})", self.terms, self.accepted_date)
    }
}
