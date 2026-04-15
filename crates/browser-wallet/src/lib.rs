//! `cow-sdk-browser-wallet` — EIP-1193 browser wallet adapter and WASM bindings.
//!
//! This is an **orthogonal** crate in the workspace: it does not belong to
//! any of the architecture layers L0..L6 because it adapts the native SDK
//! surface to a browser/WASM environment.
//!
//! # Submodules
//!
//! - [`wallet`] — [`BrowserWallet`](wallet::BrowserWallet) adapter around an EIP-1193 provider,
//!   plus [`MockBrowserWallet`](wallet::MockBrowserWallet) for testing.
//! - [`wasm`] — `wasm-bindgen` exports for browser/Node.js usage (enabled via the `wasm` feature
//!   flag).

#![warn(missing_docs)]
#![cfg_attr(
    feature = "wasm",
    allow(unsafe_code, reason = "wasm-bindgen macro generates unsafe glue code")
)]
#![cfg_attr(not(feature = "wasm"), deny(unsafe_code))]

pub mod wallet;

#[cfg(feature = "wasm")]
#[allow(unsafe_code, reason = "wasm-bindgen macro generates unsafe glue code")]
pub mod wasm;
