//! Constants for the NEAR Intents bridge provider.
//!
//! `ATTESTATOR_ADDRESS`, `ATTESTATION_PREFIX_CONST`, and
//! `ATTESTION_VERSION_BYTE` are re-exported from
//! [`cow_primitives`](https://docs.rs/cow-primitives) since they are
//! workspace-wide constants — kept in one place so other crates can
//! consume them without depending on `cow-bridging`.

/// dApp identifier embedded in the order's `appData.metadata.bridging.providerId`.
///
/// Must match the value used by the `TypeScript` SDK byte-for-byte —
/// downstream tooling looks up providers by this exact string.
pub const NEAR_INTENTS_HOOK_DAPP_ID: &str = "cow-sdk://bridging/providers/near-intents";

/// Default base URL for the NEAR Intents (`1click.chaindefuser.com`) API.
pub const NEAR_INTENTS_BASE_URL: &str = "https://1click.chaindefuser.com";

/// Default per-request timeout for `/v0/quote` (15 s).
///
/// Quotes do onchain-ish work and can be slow under load; keep the
/// ceiling higher than the rest of the surface.
pub const NEAR_INTENTS_QUOTE_TIMEOUT_MS: u64 = 15_000;

/// Default per-request timeout for `/v0/tokens` and `/v0/execution-status` (5 s).
pub const NEAR_INTENTS_DEFAULT_TIMEOUT_MS: u64 = 5_000;

/// Default per-request timeout for `/v0/attestation` (10 s).
pub const NEAR_INTENTS_ATTESTATION_TIMEOUT_MS: u64 = 10_000;
