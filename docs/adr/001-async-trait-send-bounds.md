# ADR-001: Async trait Send bounds

## Status

Accepted

## Context

The `cow-rs` SDK must run on two fundamentally different async runtimes:

1. **Native targets** (Linux, macOS, Windows) typically use `tokio` with a
   multi-threaded work-stealing scheduler. Futures that cross an `.await`
   point must be `Send` so the runtime can move them between OS threads.

2. **WebAssembly targets** (`wasm32-unknown-unknown`) run inside a browser's
   single-threaded event loop. There is no thread pool, and many browser
   types (`JsValue`, `web_sys::*`) are `!Send`. Requiring `Send` on
   futures would make it impossible to hold these types across `.await`.

Rust's `async fn` in traits is not yet stable in a way that lets us
toggle Send bounds per target at the language level. The `async_trait`
procedural macro, however, supports a `(?Send)` mode that erases the
`Send` requirement from the generated `Pin<Box<dyn Future>>` return type.

## Decision

Every async trait in the SDK uses conditional compilation to select the
appropriate `async_trait` variant:

```rust
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait OrderbookClient: Send + Sync {
    // ...
}
```

The pattern is applied uniformly to all async traits (`OrderbookClient`,
`CowSigner`, `RpcProvider`, `IpfsClient`) and to every `impl` block for
those traits.

The `Send + Sync` super-trait bound on the trait definition itself is
kept on all targets. On wasm32, `Send` and `Sync` are automatically
implemented for all types (because there is only one thread), so the
bound is trivially satisfied and does not restrict implementors. On
native targets the bound ensures that trait objects can be shared across
threads (e.g. wrapped in `Arc<dyn OrderbookClient>`).

## Consequences

**Benefits:**

- A single crate compiles for both native and wasm targets without
  feature flags or separate trait hierarchies.
- Implementors on native targets get the full multi-threaded safety
  guarantees they expect from `tokio::spawn`.
- Implementors on wasm targets can freely hold `!Send` browser types
  across `.await` points.
- Mock implementations in tests do not need to artificially satisfy
  `Send` when running under `wasm-pack test`.

**Trade-offs:**

- Every async trait definition and every `impl` block requires the
  two-line `cfg_attr` boilerplate. Forgetting one line leads to a
  compile error on the other target, which is caught by CI but can be
  confusing during local development.
- The `async_trait` crate adds one heap allocation per method call (the
  `Box<dyn Future>`). This is negligible for the SDK's use case (HTTP
  round-trips dominate latency) but would matter in a hot loop.
- When Rust stabilises `async fn` in traits with native Send-bound
  control, the `async_trait` dependency can be removed. The migration
  will touch every trait and impl but is mechanical.
