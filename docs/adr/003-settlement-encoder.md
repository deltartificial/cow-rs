# ADR-003: Standalone settlement encoder

## Status

Accepted

## Context

A CoW Protocol settlement transaction is a single `settle()` call on the
`GPv2Settlement` contract. The calldata encodes three interleaved data
structures:

1. **Token registry** -- a sorted list of unique token addresses. Sell
   and buy amounts in each trade reference tokens by index into this
   registry, not by address.
2. **Clearing prices** -- one `uint256` per registered token, forming the
   uniform clearing price vector that the protocol enforces.
3. **Trades** -- each trade references a pre-signed order, encodes the
   executed amount, and carries an optional fee discount.
4. **Interactions** -- arbitrary contract calls executed at three stages:
   _pre-settlement_ (before token transfers), _intra-settlement_ (between
   transfers), and _post-settlement_ (after transfers).

Early code scattered this encoding across helper functions in different
modules. Building a valid settlement required the caller to manually:

- Maintain a token-to-index mapping.
- Ensure clearing prices aligned with the registry.
- Sort interactions into the correct stage.
- ABI-encode the final `settle(tokens, clearingPrices, trades,
  interactions)` calldata.

This was error-prone. Forgetting to register a token, or placing an
interaction in the wrong stage, produced silent encoding bugs that only
surfaced on-chain as reverts.

## Decision

We built a standalone `SettlementEncoder` struct in
`src/settlement/encoder.rs` that owns the full encoding lifecycle:

```rust
let mut encoder = SettlementEncoder::new();
encoder.add_trade(order, signature, clearing_price)?;
encoder.add_interaction(InteractionStage::Pre, target, calldata, value);
let calldata = encoder.encode();
```

The encoder maintains internal state:

- A `TokenRegistry` that assigns deterministic indices.
- A clearing-price vector that grows in lockstep with the registry.
- Three interaction buckets (`Pre`, `Intra`, `Post`).
- A list of `EncodedTrade` values ready for ABI packing.

`encode()` produces the final ABI-encoded `settle()` calldata in one
pass. The caller never manipulates raw indices or byte offsets.

The design mirrors the `SettlementEncoder` class from the TypeScript SDK
(`@cowprotocol/contracts`), ensuring structural parity for conformance
testing.

## Consequences

**Benefits:**

- Single point of responsibility for settlement encoding. Impossible to
  forget a token registration or misalign a clearing price.
- The three interaction stages are enforced by an enum
  (`InteractionStage`), eliminating a class of ordering bugs.
- Conformance tests can compare encoded output byte-for-byte against the
  TypeScript SDK's encoder, catching drift early.
- The encoder is stateless beyond the current settlement being built --
  no global mutable state, no singletons.

**Trade-offs:**

- The encoder allocates heap memory for the token registry, price
  vector, and interaction lists. For the expected settlement sizes
  (tens of trades, a handful of interactions) this is negligible.
- Callers must use the encoder's `add_trade` / `add_interaction` API
  rather than constructing raw calldata. This is intentional -- raw
  construction was the source of bugs -- but it means the encoder must
  expose enough surface area for all valid settlement shapes.
- The encoder does not validate economic invariants (e.g. surplus,
  price consistency). It is a structural encoder, not a solver.
  Economic checks remain the responsibility of the solver or the
  settlement contract's on-chain validation.
