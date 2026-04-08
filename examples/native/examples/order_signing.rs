//! # Order Signing & UID Computation
//!
//! Demonstrates low-level EIP-712 order signing and order UID computation.
//! This is useful when you need:
//!
//! - Custom signing flows (hardware wallets, multi-sig, EIP-1271)
//! - Offline signature verification
//! - Order UID pre-computation before submission
//!
//! ## Usage
//!
//! ```sh
//! cargo run --example order_signing
//! ```
//!
//! No private key env var needed — uses a deterministic test key.

use alloy_primitives::{Address, B256, U256};
use alloy_signer_local::PrivateKeySigner;
use cow_rs::{
    EcdsaSigningScheme, OrderKind, SupportedChainId, TokenBalance, UnsignedOrder,
    build_order_typed_data, compute_order_uid, domain_separator, order_hash, sign_order,
    signing_digest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chain_id = SupportedChainId::Mainnet as u64;

    // ── 1. Create a test signer ──────────────────────────────────────────────
    //
    // Hardhat account #0 — deterministic, never use with real funds.
    let signer: PrivateKeySigner =
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".parse()?;
    let owner = signer.address();
    println!("Signer: {owner}");
    println!();

    // ── 2. Build an unsigned order ───────────────────────────────────────────
    //
    // This is the raw EIP-712 struct that gets hashed and signed.
    let order = UnsignedOrder {
        sell_token: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?, // WETH
        buy_token: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse()?,  // USDC
        receiver: owner,
        sell_amount: U256::from(1_000_000_000_000_000_000_u64), // 1 WETH
        buy_amount: U256::from(2_500_000_000_u64),              // 2500 USDC (6 dec)
        valid_to: 1_800_000_000,
        app_data: B256::ZERO,
        fee_amount: U256::ZERO,
        kind: OrderKind::Sell,
        partially_fillable: false,
        sell_token_balance: TokenBalance::Erc20,
        buy_token_balance: TokenBalance::Erc20,
    };

    // ── 3. EIP-712 components ────────────────────────────────────────────────
    //
    // The domain separator is chain-specific and includes the settlement
    // contract address.  The struct hash is computed from the order fields.
    let domain_sep = domain_separator(chain_id);
    let struct_hash = order_hash(&order);
    let digest = signing_digest(domain_sep, struct_hash);

    println!("=== EIP-712 Components ===");
    println!("  Domain separator: 0x{}", alloy_primitives::hex::encode(domain_sep));
    println!("  Struct hash:      0x{}", alloy_primitives::hex::encode(struct_hash));
    println!("  Signing digest:   0x{}", alloy_primitives::hex::encode(digest));
    println!();

    // ── 4. Build typed data envelope ─────────────────────────────────────────
    //
    // The typed data structure is what hardware wallets display for review.
    let typed_data = build_order_typed_data(order.clone(), chain_id);
    println!("=== Typed Data ===");
    println!("  Primary type: {}", typed_data.primary_type);
    println!("  Domain:       {:?}", typed_data.domain);
    println!();

    // ── 5. Sign the order (EIP-712) ──────────────────────────────────────────
    let sig = sign_order(&order, chain_id, &signer, EcdsaSigningScheme::Eip712).await?;
    println!("=== Signature (EIP-712) ===");
    println!("  Signature: {}", sig.signature);
    println!("  Scheme:    {:?}", sig.signing_scheme);
    println!();

    // ── 6. Sign with EthSign (EIP-191) ───────────────────────────────────────
    //
    // Alternative signing scheme for wallets that don't support EIP-712.
    let eth_sig = sign_order(&order, chain_id, &signer, EcdsaSigningScheme::EthSign).await?;
    println!("=== Signature (EthSign / EIP-191) ===");
    println!("  Signature: {}", eth_sig.signature);
    println!("  Scheme:    {:?}", eth_sig.signing_scheme);
    println!();

    // ── 7. Compute the order UID ─────────────────────────────────────────────
    //
    // The UID is a 56-byte identifier: hash(order) ++ owner ++ validTo.
    // It uniquely identifies this order on this chain.
    let uid = compute_order_uid(chain_id, &order, owner);
    println!("=== Order UID ===");
    println!("  {uid}");
    println!("  Length: {} bytes", (uid.len() - 2) / 2);

    // Verify UID is deterministic — same inputs always produce same UID.
    let uid2 = compute_order_uid(chain_id, &order, owner);
    assert_eq!(uid, uid2, "UID must be deterministic");
    println!("  (determinism OK)");

    // Different owner produces different UID.
    let other_uid = compute_order_uid(chain_id, &order, Address::ZERO);
    assert_ne!(uid, other_uid, "Different owners must produce different UIDs");
    println!("  (uniqueness OK)");

    Ok(())
}
