//! NEAR Intents bridge provider — HTTP client + wire types.
//!
//! The provider talks to `1click.chaindefuser.com` (aka the Defuse
//! "one-click" API) to bridge an EVM asset into non-EVM destinations
//! (SOL, BTC) or between EVM chains. The bridge is initiated by a
//! plain ERC-20 transfer to a `depositAddress` the API allocates —
//! there is no on-chain post-hook, so NEAR Intents is a
//! [`ReceiverAccountBridgeProvider`](crate::provider::ReceiverAccountBridgeProvider).
//!
//! This module scope (PR #9 of the NEAR bridging plan) ships:
//! - [`types`]: serde types for the four API endpoints.
//! - [`api::NearIntentsApi`]: reqwest HTTP client.
//! - constants: base URL, timeouts, dApp ID.
//!
//! The `NearIntentsBridgeProvider` itself (attestation verification,
//! `BridgeProvider` impl) lands in PR #10.

/// Constants for the NEAR Intents bridge provider.
///
/// `const_` is the module path because `const` is a keyword; callers
/// should prefer the re-exports below.
#[path = "const.rs"]
pub mod const_;

pub mod api;
pub mod types;

pub use api::NearIntentsApi;
pub use const_::{
    NEAR_INTENTS_ATTESTATION_TIMEOUT_MS, NEAR_INTENTS_BASE_URL, NEAR_INTENTS_DEFAULT_TIMEOUT_MS,
    NEAR_INTENTS_HOOK_DAPP_ID, NEAR_INTENTS_QUOTE_TIMEOUT_MS,
};
pub use types::{
    DefuseBlockchain, DefuseToken, NearAppFee, NearAttestationRequest, NearAttestationResponse,
    NearChainTxHash, NearDepositMode, NearDepositType, NearExecutionStatus,
    NearExecutionStatusResponse, NearQuote, NearQuoteRequest, NearQuoteResponse, NearRecipientType,
    NearRefundType, NearSwapDetails, NearSwapType,
};
