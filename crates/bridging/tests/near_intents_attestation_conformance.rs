#![allow(
    clippy::allow_attributes_without_reason,
    clippy::tests_outside_test_module,
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::unwrap_used,
    clippy::expect_used
)]
//! Attestation conformance vectors for the NEAR Intents bridge provider.
//!
//! These tests pin down the **byte-exact** output of
//! [`cow_bridging::near_intents::util::hash_quote_payload`] and the
//! 57-byte signed-message layout
//! (`prefix ‖ version ‖ depositAddress ‖ quoteHash`) against a golden
//! fixture. If the Rust serialisation ever drifts from the TS SDK's
//! `hashQuote`, the JSON canonicalisation or the message assembly will
//! break these assertions.
//!
//! The fixture is a deterministic Rust-generated reference vector — NOT
//! a live API capture. A live capture should use an `#[ignore]`-gated
//! test; see
//! `live_attestation_fixture_round_trips_against_attestator_address`
//! below for the hook.

use alloy_primitives::{Address, B256, keccak256};
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use cow_bridging::near_intents::{
    types::{
        NearDepositMode, NearDepositType, NearQuote, NearQuoteRequest, NearRecipientType,
        NearRefundType, NearSwapType,
    },
    util::{
        ATTESTATION_PREFIX_BYTES, ATTESTATION_VERSION_BYTES, hash_quote_payload,
        recover_attestation,
    },
};

const TEST_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const TEST_SIGNER_ADDRESS_HEX: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

// ── Fixed inputs (the golden vector) ──────────────────────────────────────

fn fixture_quote() -> NearQuote {
    NearQuote {
        deposit_address: "0xdead00000000000000000000000000000000beef".into(),
        amount_in: "1000000".into(),
        amount_in_formatted: "1.0".into(),
        amount_in_usd: "1.0".into(),
        min_amount_in: "1000000".into(),
        amount_out: "1000000".into(),
        amount_out_formatted: "1.0".into(),
        amount_out_usd: "1.0".into(),
        min_amount_out: "999500".into(),
        time_estimate: 120,
        deadline: "2099-01-01T00:00:00.000Z".into(),
        time_when_inactive: "2099-01-01T01:00:00.000Z".into(),
    }
}

fn fixture_quote_request() -> NearQuoteRequest {
    NearQuoteRequest {
        dry: false,
        swap_type: NearSwapType::ExactInput,
        deposit_mode: NearDepositMode::Simple,
        slippage_tolerance: 50,
        origin_asset: "nep141:0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".into(),
        deposit_type: NearDepositType::OriginChain,
        destination_asset: "nep141:0x0000000000000000000000000000000000000000".into(),
        amount: "1000000".into(),
        refund_to: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266".into(),
        refund_type: NearRefundType::OriginChain,
        recipient: "bc1q000000000000000000000000000000000000".into(),
        recipient_type: NearRecipientType::DestinationChain,
        deadline: "2099-01-01T00:00:00.000Z".into(),
        app_fees: None,
        quote_waiting_time_ms: None,
        referral: None,
        virtual_chain_recipient: None,
        virtual_chain_refund_recipient: None,
        custom_recipient_msg: None,
        session_id: None,
        connected_wallets: None,
    }
}

const FIXTURE_TIMESTAMP: &str = "2025-09-05T12:00:40.000Z";

/// Fixed deposit address the fixture attests to.
fn fixture_deposit_address() -> Address {
    "0xdead00000000000000000000000000000000beef".parse().unwrap()
}

// ── Golden expected outputs ───────────────────────────────────────────────

/// SHA-256 of the canonical-JSON serialisation of the fixture.
///
/// Captured from a first run of [`hash_quote_payload`] — if this value
/// ever changes, either:
/// - The JSON canonicalisation logic regressed (look at `canonicalise_value`) — this PR would catch
///   it.
/// - The TS SDK's `hashQuote` producesa different byte string — capture a live fixture and update
///   both sides.
const EXPECTED_QUOTE_HASH_HEX: &str =
    "0x2f9197654a6f43c8a2d71d34320397303fb6fe1620fe23c824601ba72ee345ea";

/// Canonical-JSON serialisation (sorted keys, no whitespace, `ensureAscii=false`).
const EXPECTED_CANONICAL_JSON: &str = concat!(
    r#"{"amount":"1000000","amountIn":"1000000","amountInFormatted":"1.0","#,
    r#""amountInUsd":"1.0","amountOut":"1000000","amountOutFormatted":"1.0","#,
    r#""amountOutUsd":"1.0","deadline":"2099-01-01T00:00:00.000Z","#,
    r#""depositMode":"SIMPLE","depositType":"ORIGIN_CHAIN","#,
    r#""destinationAsset":"nep141:0x0000000000000000000000000000000000000000","#,
    r#""dry":false,"minAmountIn":"1000000","minAmountOut":"999500","#,
    r#""originAsset":"nep141:0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48","#,
    r#""recipient":"bc1q000000000000000000000000000000000000","#,
    r#""recipientType":"DESTINATION_CHAIN","#,
    r#""refundTo":"0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266","#,
    r#""refundType":"ORIGIN_CHAIN","slippageTolerance":50,"#,
    r#""swapType":"EXACT_INPUT","timestamp":"2025-09-05T12:00:40.000Z"}"#,
);

// ── Tests ─────────────────────────────────────────────────────────────────

#[test]
fn canonical_json_matches_golden_fixture() {
    let (_, canonical) =
        hash_quote_payload(&fixture_quote(), &fixture_quote_request(), FIXTURE_TIMESTAMP).unwrap();
    assert_eq!(
        canonical, EXPECTED_CANONICAL_JSON,
        "canonical JSON drift — either canonicalise_value changed key ordering / escaping, or \
         the quote payload shape changed. If intentional, regenerate the golden fixture."
    );
}

#[test]
fn quote_hash_matches_golden_fixture() {
    let (hash, _) =
        hash_quote_payload(&fixture_quote(), &fixture_quote_request(), FIXTURE_TIMESTAMP).unwrap();
    let expected: B256 = EXPECTED_QUOTE_HASH_HEX.parse().unwrap();
    assert_eq!(hash, expected, "SHA-256(canonical_json) drift");
}

#[test]
fn signed_message_layout_is_57_bytes_prefix_version_addr_hash() {
    let (quote_hash, _) =
        hash_quote_payload(&fixture_quote(), &fixture_quote_request(), FIXTURE_TIMESTAMP).unwrap();
    let deposit_address = fixture_deposit_address();

    let mut message = Vec::with_capacity(57);
    message.extend_from_slice(&ATTESTATION_PREFIX_BYTES);
    message.extend_from_slice(&ATTESTATION_VERSION_BYTES);
    message.extend_from_slice(deposit_address.as_slice());
    message.extend_from_slice(quote_hash.as_slice());

    assert_eq!(message.len(), 57);
    // Slice offsets are what the TS attestation contract also consumes.
    assert_eq!(&message[0..4], &ATTESTATION_PREFIX_BYTES);
    assert_eq!(&message[4..5], &ATTESTATION_VERSION_BYTES);
    assert_eq!(&message[5..25], deposit_address.as_slice());
    assert_eq!(&message[25..57], quote_hash.as_slice());
}

#[test]
fn recover_attestation_round_trips_against_local_signer() {
    use std::str::FromStr;
    let (quote_hash, _) =
        hash_quote_payload(&fixture_quote(), &fixture_quote_request(), FIXTURE_TIMESTAMP).unwrap();
    let deposit_address = fixture_deposit_address();

    let mut message = Vec::with_capacity(57);
    message.extend_from_slice(&ATTESTATION_PREFIX_BYTES);
    message.extend_from_slice(&ATTESTATION_VERSION_BYTES);
    message.extend_from_slice(deposit_address.as_slice());
    message.extend_from_slice(quote_hash.as_slice());
    let digest = keccak256(&message);

    let signer = PrivateKeySigner::from_str(TEST_KEY).unwrap();
    let sig = signer.sign_hash_sync(&digest).unwrap();
    let sig_hex = format!("0x{}", hex_encode(&sig.as_bytes()));
    let expected_signer: Address = TEST_SIGNER_ADDRESS_HEX.parse().unwrap();
    assert_eq!(signer.address(), expected_signer);

    let recovered = recover_attestation(deposit_address, quote_hash, &sig_hex).unwrap();
    assert_eq!(recovered, expected_signer);
}

#[test]
fn recover_attestation_rejects_one_byte_signature_mutation() {
    use std::str::FromStr;
    let (quote_hash, _) =
        hash_quote_payload(&fixture_quote(), &fixture_quote_request(), FIXTURE_TIMESTAMP).unwrap();
    let deposit_address = fixture_deposit_address();

    let mut message = Vec::with_capacity(57);
    message.extend_from_slice(&ATTESTATION_PREFIX_BYTES);
    message.extend_from_slice(&ATTESTATION_VERSION_BYTES);
    message.extend_from_slice(deposit_address.as_slice());
    message.extend_from_slice(quote_hash.as_slice());
    let digest = keccak256(&message);

    let signer = PrivateKeySigner::from_str(TEST_KEY).unwrap();
    let sig = signer.sign_hash_sync(&digest).unwrap();
    let mut sig_bytes = sig.as_bytes();
    // Flip one bit in `r` — recover either fails or returns a different address.
    sig_bytes[0] ^= 0x01;
    let sig_hex = format!("0x{}", hex_encode(&sig_bytes));

    // A 1-byte `r` mutation either fails to recover or recovers a
    // different signer — both are acceptable rejection paths.
    if let Ok(addr) = recover_attestation(deposit_address, quote_hash, &sig_hex) {
        assert_ne!(
            addr,
            signer.address(),
            "1-byte r-mutation must not still recover the same signer"
        );
    }
}

#[test]
fn recover_attestation_rejects_one_byte_deposit_address_mutation() {
    use std::str::FromStr;
    let (quote_hash, _) =
        hash_quote_payload(&fixture_quote(), &fixture_quote_request(), FIXTURE_TIMESTAMP).unwrap();
    let deposit_address = fixture_deposit_address();

    let mut message = Vec::with_capacity(57);
    message.extend_from_slice(&ATTESTATION_PREFIX_BYTES);
    message.extend_from_slice(&ATTESTATION_VERSION_BYTES);
    message.extend_from_slice(deposit_address.as_slice());
    message.extend_from_slice(quote_hash.as_slice());
    let digest = keccak256(&message);

    let signer = PrivateKeySigner::from_str(TEST_KEY).unwrap();
    let sig = signer.sign_hash_sync(&digest).unwrap();
    let sig_hex = format!("0x{}", hex_encode(&sig.as_bytes()));

    // Recover with a *different* deposit address — the recovered
    // signer cannot match.
    let mut mutated = deposit_address.into_array();
    mutated[0] ^= 0x01;
    let mutated_addr = Address::from_slice(&mutated);
    let recovered = recover_attestation(mutated_addr, quote_hash, &sig_hex).unwrap();
    assert_ne!(
        recovered,
        signer.address(),
        "flipping 1 byte of depositAddress must make recovery yield a different signer"
    );
}

#[test]
fn recover_attestation_rejects_one_byte_quote_hash_mutation() {
    use std::str::FromStr;
    let (quote_hash, _) =
        hash_quote_payload(&fixture_quote(), &fixture_quote_request(), FIXTURE_TIMESTAMP).unwrap();
    let deposit_address = fixture_deposit_address();

    let mut message = Vec::with_capacity(57);
    message.extend_from_slice(&ATTESTATION_PREFIX_BYTES);
    message.extend_from_slice(&ATTESTATION_VERSION_BYTES);
    message.extend_from_slice(deposit_address.as_slice());
    message.extend_from_slice(quote_hash.as_slice());
    let digest = keccak256(&message);

    let signer = PrivateKeySigner::from_str(TEST_KEY).unwrap();
    let sig = signer.sign_hash_sync(&digest).unwrap();
    let sig_hex = format!("0x{}", hex_encode(&sig.as_bytes()));

    let mut mutated_bytes: [u8; 32] = quote_hash.into();
    mutated_bytes[0] ^= 0x01;
    let recovered =
        recover_attestation(deposit_address, B256::from(mutated_bytes), &sig_hex).unwrap();
    assert_ne!(
        recovered,
        signer.address(),
        "flipping 1 byte of quote_hash must make recovery yield a different signer"
    );
}

/// Placeholder for a future live-capture test — gated on `#[ignore]` so
/// the CI lane stays deterministic. Populate once the TS SDK's
/// `captureAttestation.ts` script is wired up and committed a real
/// fixture under `tests/fixtures/near_intents/`.
#[test]
#[ignore = "requires live NEAR Intents API capture fixture"]
fn live_attestation_fixture_round_trips_against_attestator_address() {
    // TODO: read `tests/fixtures/near_intents/attestation_conformance.json`
    // once a live capture exists, then:
    //   let recovered = recover_attestation(dep_addr, quote_hash, &signature).unwrap();
    //   assert_eq!(recovered, cow_primitives::ATTESTATOR_ADDRESS);
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        write!(&mut s, "{b:02x}").unwrap();
    }
    s
}
