# ADR-002: Trait injection for testability

## Status

Accepted

## Context

The `cow-rs` SDK interacts with three external systems at runtime:

1. **CoW Protocol orderbook** -- an HTTP REST API for quoting, submitting,
   and querying orders.
2. **ECDSA signing** -- private-key operations that produce EIP-712 or
   EIP-191 signatures.
3. **Ethereum JSON-RPC** -- read-only `eth_call` and `eth_getStorageAt`
   calls for on-chain state.

Early prototypes used concrete types directly (`OrderBookApi`,
`PrivateKeySigner`, `OnchainReader`). This worked for production code
but made unit testing painful: every test that exercised `TradingSdk`
logic needed either a live network connection or an elaborate HTTP mock
server. Integration tests were slow, flaky, and hard to run in CI
without secrets.

We considered two alternatives:

- **Concrete types only** (status quo). Simpler API surface, no trait
  boilerplate, but poor testability and no way for downstream crates to
  swap implementations.
- **Trait objects behind `dyn` dispatch**. Enables mocking and
  alternative implementations at the cost of one vtable indirection per
  call and the need to define trait interfaces.

## Decision

We introduced three injectable traits in `src/traits.rs`:

| Trait             | Concrete impl      | Purpose                   |
| ----------------- | ------------------ | ------------------------- |
| `OrderbookClient` | `OrderBookApi`     | Orderbook HTTP operations |
| `CowSigner`       | `PrivateKeySigner` | ECDSA signing             |
| `RpcProvider`     | `OnchainReader`    | JSON-RPC reads            |
| `IpfsClient`      | `Ipfs`             | IPFS fetch and upload     |

Each trait mirrors the subset of methods that `TradingSdk` (or other
high-level orchestrators) actually calls. Blanket `impl` blocks on the
concrete types delegate to their existing async methods, so production
code paths are unchanged.

`TradingSdkConfig` exposes builder methods for injecting custom
implementations:

```rust
let config = TradingSdkConfig::prod(SupportedChainId::Sepolia, "test")
    .with_orderbook_client(Arc::new(my_mock));
```

Internally, the SDK stores each dependency as `Arc<dyn Trait>`. The
`with_*` methods accept anything that implements the trait and wrap it in
an `Arc`.

## Consequences

**Benefits:**

- Unit tests inject lightweight mock structs that return canned data.
  No network, no secrets, deterministic and fast.
- Downstream crates can provide their own implementations (e.g. a
  caching orderbook proxy, an HSM-backed signer, a batch-optimised RPC
  provider).
- The concrete types remain usable directly -- the traits are additive,
  not a breaking change.
- All traits are object-safe (`dyn`-compatible), so they can be stored
  in `Arc<dyn T>` and passed across thread boundaries.

**Trade-offs:**

- Each trait must be kept in sync with the concrete type's public API.
  Adding a new method to `OrderBookApi` requires adding it to
  `OrderbookClient` (if the SDK uses it internally).
- One level of dynamic dispatch per call. Negligible compared to
  network latency, but visible in profiles of purely local test runs.
- The `async_trait` attribute is required on every trait and impl (see
  ADR-001), adding boilerplate.
- Mock structs in tests carry canned data fields, which must be updated
  when response types change. This is intentional -- it forces tests to
  stay aligned with the real API surface.
