//! Logging toggle for the `CoW` SDK, ported from `utils/log.ts`.
//!
//! Uses a global [`AtomicBool`] flag to gate SDK-level trace output through
//! the `tracing` crate. When disabled (the default), `tracing` events emitted
//! at `info` level or below from `cow_rs` are effectively suppressed unless
//! the caller installs their own subscriber.

use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag that gates SDK trace output.
static LOG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable or disable SDK-level logging.
///
/// When set to `true`, the SDK will emit `tracing` events at `info` level
/// for major operations (quote requests, signing, posting, etc.).
///
/// The flag is stored in a global [`AtomicBool`] — changes take effect
/// immediately and are visible from all threads.
///
/// Mirrors `enableLogging(enabled)` from the `TypeScript` SDK.
///
/// # Parameters
///
/// * `enabled` — `true` to enable SDK logging, `false` to suppress it.
///
/// # Example
///
/// ```
/// use cow_rs::common::logging::enable_logging;
///
/// enable_logging(true);
/// assert!(cow_rs::common::logging::is_logging_enabled());
/// enable_logging(false);
/// assert!(!cow_rs::common::logging::is_logging_enabled());
/// ```
pub fn enable_logging(enabled: bool) {
    LOG_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Returns `true` if SDK logging is currently enabled.
///
/// # Returns
///
/// The current value of the global logging flag.
///
/// # Example
///
/// ```
/// use cow_rs::common::logging::{enable_logging, is_logging_enabled};
///
/// enable_logging(false);
/// assert!(!is_logging_enabled());
/// ```
#[must_use]
pub fn is_logging_enabled() -> bool {
    LOG_ENABLED.load(Ordering::Relaxed)
}

/// Log a message at `info` level if SDK logging is enabled.
///
/// When logging is disabled (the default), this function is a no-op.
/// When enabled, the message is emitted via `tracing::info!` with target
/// `"cow_sdk"`.
///
/// Mirrors the `TypeScript` `log(text)` function which prefixes messages
/// with `[COW SDK]`. In Rust we use the `tracing` crate's structured
/// logging instead.
///
/// # Parameters
///
/// * `text` — the message to log.
///
/// # Example
///
/// ```
/// use cow_rs::common::logging::{enable_logging, sdk_log};
///
/// enable_logging(true);
/// sdk_log("getting quote for WETH → USDC");
/// ```
pub fn sdk_log(text: &str) {
    if is_logging_enabled() {
        tracing::info!(target: "cow_sdk", "{}", text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_logging() {
        enable_logging(false);
        assert!(!is_logging_enabled());
        enable_logging(true);
        assert!(is_logging_enabled());
        enable_logging(false);
        assert!(!is_logging_enabled());
    }

    #[test]
    fn sdk_log_does_not_panic_when_disabled() {
        enable_logging(false);
        sdk_log("this should be silent");
    }

    #[test]
    fn sdk_log_does_not_panic_when_enabled() {
        enable_logging(true);
        sdk_log("this should be visible if a subscriber is installed");
        enable_logging(false);
    }
}
