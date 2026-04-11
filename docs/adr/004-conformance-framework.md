# ADR-004: Fixture-based conformance framework

## Status

Accepted

## Context

`cow-rs` is a Rust port of the upstream TypeScript SDK
(`@cowprotocol/cow-sdk` and related packages). The two implementations
must produce identical outputs for the same inputs across several
domains:

- **EIP-712 hashing** -- domain separators, order struct hashes, signing
  digests, and order UIDs.
- **App-data encoding** -- deterministic JSON serialisation, keccak256
  hashes, and CIDv1 derivation.
- **Order flag encoding** -- bitfield packing of order kind, fill mode,
  and token-balance variants.
- **Settlement encoding** -- ABI-encoded `settle()` calldata for known
  trade/interaction configurations.

Ensuring byte-level parity is critical: a single-bit divergence in an
order hash means the Rust SDK produces orders that the protocol rejects.

We considered three approaches:

1. **Manual golden-value tests**. Hard-code expected hashes in Rust
   tests. Simple but brittle -- values must be re-derived by hand
   whenever the TypeScript SDK changes behaviour.
2. **Live cross-SDK tests**. Spawn the TypeScript SDK in a child
   process, feed it the same inputs, and compare. Accurate but slow,
   requires Node.js in CI, and introduces a flaky network dependency.
3. **Fixture files generated from the TypeScript SDK**. Run a one-time
   generation script against a pinned TypeScript SDK commit, write the
   inputs and expected outputs to JSON/YAML fixture files, and load them
   in Rust `#[test]` functions.

## Decision

We adopted approach (3): fixture-based conformance tests.

**Fixture generation** is performed by a maintainer CLI (not run in CI)
that imports the TypeScript SDK at a pinned commit, exercises each
function under test, and writes structured fixture files to
`crates/cow-rs/tests/fixtures/`.

**Source pinning** is tracked in `source-lock.yaml` at the workspace
root. This file records the upstream TypeScript SDK repository URL,
commit hash, and the date the fixtures were last regenerated. CI does
not regenerate fixtures -- it only runs the Rust tests against the
checked-in fixtures. A maintainer updates `source-lock.yaml` and
regenerates fixtures when the upstream SDK releases a new version.

**Test structure** follows a data-driven pattern:

```rust
#[test]
fn conformance_domain_separator() {
    let fixtures: Vec<DomainSeparatorFixture> =
        load_fixtures("domain_separator.json");
    for f in &fixtures {
        let got = domain_separator(f.chain_id);
        assert_eq!(got, f.expected, "chain_id={}", f.chain_id);
    }
}
```

Each fixture file contains an array of test cases. The Rust test
iterates over them, exercising the function under test and comparing the
output to the expected value from the TypeScript SDK.

## Consequences

**Benefits:**

- Byte-level parity with the TypeScript SDK is verified on every CI run
  without needing Node.js, network access, or upstream API credentials.
- Adding a new conformance check is mechanical: add a fixture file and a
  short Rust test that loads it.
- `source-lock.yaml` makes the upstream dependency explicit and
  auditable. Reviewers can see exactly which TypeScript SDK commit the
  fixtures were generated from.
- Fixture files serve as documentation: they show concrete
  input/output pairs for every function, which helps new contributors
  understand expected behaviour.

**Trade-offs:**

- Fixtures can become stale. If the upstream TypeScript SDK changes
  behaviour and `source-lock.yaml` is not updated, the Rust tests will
  still pass against the old expectations. This is mitigated by a CI
  job that warns when `source-lock.yaml` is older than a configurable
  threshold.
- The maintainer CLI must be kept in sync with the fixture file schemas.
  Schema drift (e.g. adding a new field to a fixture type) requires
  updating both the generator and the Rust deserialiser.
- Fixture files add binary weight to the repository. For the current
  scope (dozens of test cases, each a few hundred bytes of JSON) this is
  negligible, but it could grow if we add fuzz-corpus-style fixtures.
- The regeneration step is manual and requires a working Node.js
  environment with the TypeScript SDK installed. This is intentional --
  automated regeneration in CI would mask upstream breakages instead of
  surfacing them for review.
