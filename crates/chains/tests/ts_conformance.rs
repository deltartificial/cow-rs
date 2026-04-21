#![allow(
    clippy::allow_attributes_without_reason,
    clippy::tests_outside_test_module,
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::unwrap_used,
    clippy::expect_used
)]
//! Conformance tests asserting Rust chain-id constants stay aligned
//! with the upstream TypeScript SDK (`cow-sdk`).
//!
//! These tests hardcode the canonical TS values and fail loudly if the
//! Rust workspace drifts. Regenerate by auditing the relevant TS file
//! against `cow-sdk@<ref>` and updating both sides in the same PR.

use cow_chains::SupportedChainId;

/// `SupportedChainId::Plasma = 9_745`.
///
/// TS side: `cow-sdk/packages/config/src/chains/types.ts::PLASMA_ID = 9745`
/// (verified against `cow-sdk@a5207e0d`, 2026-04-16). Also cross-checks
/// with `viem/src/chains/definitions/plasma.ts::id = 9745`.
///
/// If this test fails, either:
/// - TS bumped the Plasma chain-id → update `SupportedChainId::Plasma` to match
/// - Rust drifted → update this expected value alongside the Rust change
///
/// Silent divergence would route Plasma bridges to the wrong URLs
/// without any user-visible error.
#[test]
fn plasma_chain_id_matches_ts_viem_constant() {
    const TS_PLASMA_ID: u64 = 9_745;
    assert_eq!(
        SupportedChainId::Plasma as u64,
        TS_PLASMA_ID,
        "Rust SupportedChainId::Plasma ({}) diverged from TS PLASMA_ID ({}). See \
         `cow-sdk/packages/config/src/chains/types.ts` to confirm the TS side.",
        SupportedChainId::Plasma as u64,
        TS_PLASMA_ID,
    );
}

/// Spot-check the well-known EVM chain ids — defensive against a
/// refactor that accidentally renames a variant and shuffles the
/// discriminant.
#[test]
fn well_known_evm_chain_ids_unchanged() {
    for (chain, expected) in [
        (SupportedChainId::Mainnet, 1_u64),
        (SupportedChainId::GnosisChain, 100),
        (SupportedChainId::ArbitrumOne, 42_161),
        (SupportedChainId::Base, 8_453),
        (SupportedChainId::Polygon, 137),
        (SupportedChainId::Avalanche, 43_114),
        (SupportedChainId::BnbChain, 56),
        (SupportedChainId::Linea, 59_144),
        (SupportedChainId::Ink, 57_073),
        (SupportedChainId::Sepolia, 11_155_111),
    ] {
        assert_eq!(chain as u64, expected, "{chain:?} discriminant drift");
    }
}
