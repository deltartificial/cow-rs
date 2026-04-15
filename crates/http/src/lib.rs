//! `cow-http` — Layer 3 HTTP transport primitives for the `CoW` Protocol SDK.
//!
//! Provides the shared rate limiter and retry policy used by every L4
//! transport client (`orderbook`, `subgraph`, `ipfs`, ...). Future
//! iterations will add an `HttpTransport` trait so custom backends can
//! plug in without touching the L4 crates.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod rate_limit;

pub use rate_limit::{DEFAULT_RETRY_STATUS_CODES, RateLimiter, RetryPolicy};
