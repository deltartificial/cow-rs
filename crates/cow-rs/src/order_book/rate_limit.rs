//! Client-side throttling and retry policy for the orderbook HTTP client.
//!
//! Mirrors the defaults of the upstream `TypeScript` SDK's
//! `packages/order-book/src/request.ts`:
//!
//! * **Rate limit** — 5 requests per second, per instance, enforced via a shared token bucket.
//!   Matches `DEFAULT_LIMITER_OPTIONS = { tokensPerInterval: 5, interval: 'second' }`.
//! * **Retry policy** — up to 10 attempts with exponential backoff, no jitter, unbounded maximum
//!   delay (we cap at 30 s in practice), retrying on the same HTTP status codes as the `TypeScript`
//!   SDK: `[408, 425, 429, 500, 502, 503, 504]`.
//!
//! # Target-specific behaviour
//!
//! `tokio::time::sleep` is only available when the `native` feature is
//! enabled (which pulls in the `tokio` runtime). When `native` is
//! disabled (e.g. on wasm or bare-features builds) the sleep calls
//! become no-ops — the rate limiter never blocks, retries happen
//! back-to-back without a delay — and any concrete timing behaviour is
//! out of scope until a compatible timer backend is wired in.
//!
//! # Construction
//!
//! [`RateLimiter`] is cloned cheaply (internally an `Arc<Mutex<_>>`) so a
//! single instance can be shared across clones of
//! [`super::api::OrderBookApi`], matching the behaviour of
//! `this.rateLimiter` on a `TypeScript` `OrderBookApi` whose instance is
//! reused by every request.

use std::{sync::Arc, time::Duration};

#[cfg(feature = "native")]
#[allow(
    clippy::disallowed_types,
    reason = "std::sync::Mutex is intentional here: the critical section is \
              microseconds and a poisoned mutex is actually the right failure \
              mode for a misbehaving caller — parking_lot would silently \
              continue with corrupted bucket state"
)]
use std::sync::Mutex;
#[cfg(feature = "native")]
use tokio::time::Instant;

// ── Rate limiter ─────────────────────────────────────────────────────────────

/// A shared token-bucket rate limiter.
///
/// Refills continuously at `rate` tokens per second up to a maximum of
/// `capacity` tokens. [`RateLimiter::acquire`] blocks asynchronously
/// until at least one token is available, then decrements the bucket.
///
/// Cloning a `RateLimiter` shares the same bucket — every clone throttles
/// against the same budget. Create a new instance with [`RateLimiter::new`]
/// if you want an independent budget.
///
/// # Example
///
/// ```
/// use cow_rs::order_book::RateLimiter;
///
/// // Match the upstream default: 5 requests per second, burst of 5.
/// let limiter = RateLimiter::new(5.0, 5.0);
/// ```
#[derive(Debug, Clone)]
pub struct RateLimiter {
    #[cfg(feature = "native")]
    #[allow(
        clippy::disallowed_types,
        reason = "std::sync::Mutex is chosen deliberately — see the import comment"
    )]
    state: Arc<Mutex<BucketState>>,
    /// Sustained refill rate in tokens per second.
    rate: f64,
    /// Maximum bucket capacity (burst allowance).
    capacity: f64,
}

#[cfg(feature = "native")]
#[derive(Debug)]
struct BucketState {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    /// Construct a new rate limiter with `rate` tokens per second and a
    /// maximum burst of `capacity` tokens. The bucket starts full.
    ///
    /// # Panics
    ///
    /// Panics if `rate` or `capacity` is negative, zero, or not finite.
    #[must_use]
    #[allow(
        clippy::panic,
        reason = "configuration error that must be surfaced loudly at construction"
    )]
    pub fn new(rate: f64, capacity: f64) -> Self {
        assert!(
            rate.is_finite() && rate > 0.0,
            "RateLimiter rate must be a finite positive number (got {rate})"
        );
        assert!(
            capacity.is_finite() && capacity > 0.0,
            "RateLimiter capacity must be a finite positive number (got {capacity})"
        );
        #[cfg(feature = "native")]
        #[allow(
            clippy::disallowed_types,
            reason = "std::sync::Mutex is chosen deliberately — see the import comment"
        )]
        {
            Self {
                state: Arc::new(Mutex::new(BucketState {
                    tokens: capacity,
                    last_refill: Instant::now(),
                })),
                rate,
                capacity,
            }
        }
        #[cfg(not(feature = "native"))]
        {
            Self { rate, capacity }
        }
    }

    /// Construct a limiter matching the upstream `TypeScript` SDK's
    /// defaults: 5 requests per second, burst of 5.
    #[must_use]
    pub fn default_orderbook() -> Self {
        Self::new(5.0, 5.0)
    }

    /// Return the sustained refill rate in tokens per second.
    #[must_use]
    pub const fn rate(&self) -> f64 {
        self.rate
    }

    /// Return the maximum burst capacity in tokens.
    #[must_use]
    pub const fn capacity(&self) -> f64 {
        self.capacity
    }

    /// Wait until at least one token is available and consume it.
    ///
    /// When the `native` feature is enabled this sleeps with
    /// [`tokio::time::sleep`] for the time required to refill a single
    /// token. Without `native` this returns immediately because no async
    /// timer backend is wired in — the bucket state is not consulted.
    #[allow(
        clippy::unused_async,
        reason = "wasm path is intentionally synchronous so the API stays unchanged"
    )]
    pub async fn acquire(&self) {
        #[cfg(feature = "native")]
        {
            loop {
                let wait = {
                    #[allow(
                        clippy::expect_used,
                        reason = "poisoned mutex is unrecoverable — surface it immediately"
                    )]
                    let mut state = self.state.lock().expect("rate limiter mutex poisoned");
                    let now = Instant::now();
                    let elapsed = now.duration_since(state.last_refill).as_secs_f64();
                    state.tokens = elapsed.mul_add(self.rate, state.tokens).min(self.capacity);
                    state.last_refill = now;
                    if state.tokens >= 1.0 {
                        state.tokens -= 1.0;
                        return;
                    }
                    let missing = 1.0 - state.tokens;
                    Duration::from_secs_f64(missing / self.rate)
                };
                tokio::time::sleep(wait).await;
            }
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::default_orderbook()
    }
}

// ── Retry policy ─────────────────────────────────────────────────────────────

/// HTTP status codes that the upstream `TypeScript` SDK retries on. Mirrors
/// `STATUS_CODES_TO_RETRY` in `packages/order-book/src/request.ts`.
pub const DEFAULT_RETRY_STATUS_CODES: &[u16] = &[
    408, // Request Timeout
    425, // Too Early
    429, // Too Many Requests
    500, // Internal Server Error
    502, // Bad Gateway
    503, // Service Unavailable
    504, // Gateway Timeout
];

/// Exponential-backoff retry policy for transient HTTP failures.
///
/// Matches the upstream `TypeScript` SDK's `DEFAULT_BACKOFF_OPTIONS`:
/// 10 attempts total, retry on any of
/// [`DEFAULT_RETRY_STATUS_CODES`] and on any transport-level
/// `reqwest::Error` other than a body-parse failure. The delay between
/// attempt *N* and attempt *N + 1* is `initial_delay * 2^N`, capped at
/// `max_delay`. No jitter is added.
///
/// Cloning a `RetryPolicy` is cheap — it is a plain data struct.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Total number of attempts (including the first, pre-retry one).
    /// A value of `1` disables retries; `0` is clamped to `1` at use site.
    pub max_attempts: u32,
    /// Delay before the second attempt; doubles for every subsequent retry.
    pub initial_delay: Duration,
    /// Upper bound on the delay between retries.
    pub max_delay: Duration,
    /// HTTP status codes that trigger a retry. Non-listed codes fail fast.
    pub retry_status_codes: &'static [u16],
}

impl RetryPolicy {
    /// The upstream-compatible default: 10 attempts, 100 ms initial
    /// delay, 30 s max delay, [`DEFAULT_RETRY_STATUS_CODES`].
    #[must_use]
    pub const fn default_orderbook() -> Self {
        Self {
            max_attempts: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            retry_status_codes: DEFAULT_RETRY_STATUS_CODES,
        }
    }

    /// Disable retries entirely (`max_attempts = 1`).
    #[must_use]
    pub const fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            initial_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
            retry_status_codes: &[],
        }
    }

    /// Return the delay to wait before the `attempt`-th retry (0-indexed:
    /// `attempt = 0` is the delay before the *second* request).
    ///
    /// Uses saturating arithmetic — a large `attempt` index clamps to
    /// [`Self::max_delay`] rather than overflowing.
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let factor = 2u64.saturating_pow(attempt);
        let nanos = self.initial_delay.as_nanos().saturating_mul(u128::from(factor));
        let capped = nanos.min(self.max_delay.as_nanos());
        // Values above u64::MAX nanos (~584 years) cannot occur under the
        // upstream defaults; clamp to max_delay as a safety net.
        u64::try_from(capped).map_or(self.max_delay, Duration::from_nanos)
    }

    /// Return `true` if an HTTP status code triggers a retry under this
    /// policy.
    #[must_use]
    pub fn should_retry_status(&self, status: u16) -> bool {
        self.retry_status_codes.contains(&status)
    }

    /// Return `true` if a `reqwest` transport error should trigger a
    /// retry — every error other than a body-parse failure (timeouts,
    /// connect errors, DNS failures, TLS errors, etc.).
    #[must_use]
    pub fn should_retry_error(&self, err: &reqwest::Error) -> bool {
        !err.is_decode()
    }

    /// Sleep for `delay` on native targets; no-op on wasm.
    #[allow(clippy::unused_async, reason = "wasm path is intentionally synchronous")]
    pub(crate) async fn wait(&self, delay: Duration) {
        #[cfg(feature = "native")]
        tokio::time::sleep(delay).await;
        #[cfg(not(feature = "native"))]
        let _ = delay;
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::default_orderbook()
    }
}

// Silence an unused-import warning when `native` is disabled and `Arc` is not referenced.
#[cfg(not(feature = "native"))]
const _: () = {
    let _: Option<Arc<()>> = None;
};

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(all(test, feature = "native"))]
#[allow(
    clippy::tests_outside_test_module,
    clippy::let_underscore_must_use,
    reason = "the compound `cfg(all(test, feature = \"native\"))` gate confuses the \
              test-module lint, and `let _ =` is the idiomatic form for \
              `#[should_panic]` expression discards"
)]
mod tests {
    use super::*;

    #[test]
    fn retry_policy_default_matches_upstream() {
        let p = RetryPolicy::default_orderbook();
        assert_eq!(p.max_attempts, 10);
        assert_eq!(p.initial_delay, Duration::from_millis(100));
        assert_eq!(p.max_delay, Duration::from_secs(30));
        assert_eq!(p.retry_status_codes, DEFAULT_RETRY_STATUS_CODES);
    }

    #[test]
    fn retry_policy_delay_doubles_and_caps() {
        let p = RetryPolicy::default_orderbook();
        assert_eq!(p.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(p.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(p.delay_for_attempt(2), Duration::from_millis(400));
        assert_eq!(p.delay_for_attempt(3), Duration::from_millis(800));
        // Capped at 30s (which is 30_000ms). 100ms * 2^9 = 51_200ms > 30s.
        assert_eq!(p.delay_for_attempt(9), Duration::from_secs(30));
        // Saturating: a huge attempt index still returns max_delay, never panics.
        assert_eq!(p.delay_for_attempt(1_000), Duration::from_secs(30));
    }

    #[test]
    fn retry_policy_should_retry_status_matches_upstream() {
        let p = RetryPolicy::default_orderbook();
        for code in [408_u16, 425, 429, 500, 502, 503, 504] {
            assert!(p.should_retry_status(code), "{code} should retry");
        }
        for code in [200_u16, 201, 204, 400, 401, 403, 404, 422] {
            assert!(!p.should_retry_status(code), "{code} must not retry");
        }
    }

    #[test]
    fn retry_policy_no_retry_disables_everything() {
        let p = RetryPolicy::no_retry();
        assert_eq!(p.max_attempts, 1);
        assert!(!p.should_retry_status(500));
    }

    #[test]
    fn rate_limiter_accessors() {
        let limiter = RateLimiter::new(5.0, 10.0);
        assert!((limiter.rate() - 5.0).abs() < f64::EPSILON);
        assert!((limiter.capacity() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    #[should_panic(expected = "rate must be a finite positive number")]
    fn rate_limiter_rejects_zero_rate() {
        let _ = RateLimiter::new(0.0, 5.0);
    }

    #[test]
    #[should_panic(expected = "capacity must be a finite positive number")]
    fn rate_limiter_rejects_negative_capacity() {
        let _ = RateLimiter::new(5.0, -1.0);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn rate_limiter_consumes_initial_burst_immediately() {
        // With a capacity of 3 the first 3 calls must not sleep.
        let limiter = RateLimiter::new(5.0, 3.0);
        let start = tokio::time::Instant::now();
        limiter.acquire().await;
        limiter.acquire().await;
        limiter.acquire().await;
        assert!(
            start.elapsed() < Duration::from_millis(1),
            "initial burst should be instantaneous"
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn rate_limiter_throttles_after_burst() {
        // Capacity 1, rate 5/s -> second call waits ~200 ms.
        let limiter = RateLimiter::new(5.0, 1.0);
        limiter.acquire().await; // drains the bucket
        let start = tokio::time::Instant::now();
        limiter.acquire().await;
        let waited = start.elapsed();
        assert!(
            waited >= Duration::from_millis(200),
            "second acquire should wait for a refill (got {waited:?})"
        );
        assert!(
            waited < Duration::from_millis(500),
            "second acquire should wait roughly 1 / rate seconds (got {waited:?})"
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn rate_limiter_shared_state_across_clones() {
        let a = RateLimiter::new(5.0, 1.0);
        let b = a.clone();
        a.acquire().await; // drains the single token
        let start = tokio::time::Instant::now();
        b.acquire().await; // forced to wait because it shares the bucket
        assert!(
            start.elapsed() >= Duration::from_millis(200),
            "cloned limiter should share the bucket"
        );
    }

    #[test]
    fn rate_limiter_default_matches_default_orderbook() {
        let a = RateLimiter::default();
        let b = RateLimiter::default_orderbook();
        assert!((a.rate() - b.rate()).abs() < f64::EPSILON);
        assert!((a.capacity() - b.capacity()).abs() < f64::EPSILON);
    }

    #[test]
    fn retry_policy_default_trait() {
        let a = RetryPolicy::default();
        let b = RetryPolicy::default_orderbook();
        assert_eq!(a.max_attempts, b.max_attempts);
        assert_eq!(a.initial_delay, b.initial_delay);
        assert_eq!(a.max_delay, b.max_delay);
    }

    #[test]
    #[should_panic(expected = "rate must be a finite positive number")]
    fn rate_limiter_rejects_nan_rate() {
        let _ = RateLimiter::new(f64::NAN, 5.0);
    }

    #[test]
    #[should_panic(expected = "capacity must be a finite positive number")]
    fn rate_limiter_rejects_inf_capacity() {
        let _ = RateLimiter::new(5.0, f64::INFINITY);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn retry_policy_wait_sleeps() {
        let p = RetryPolicy::default_orderbook();
        let start = tokio::time::Instant::now();
        p.wait(Duration::from_millis(100)).await;
        assert!(start.elapsed() >= Duration::from_millis(100));
    }
}
