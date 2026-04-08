//! Error type for the `CoW` Protocol SDK.
//!
//! [`CowError`] is the unified error type used across all modules.
//! Every fallible function in the SDK returns `Result<T, CowError>`.
//!
//! # Variants
//!
//! | Variant | When |
//! |---|---|
//! | [`UnknownAsset`](CowError::UnknownAsset) | Asset symbol not in [`TokenRegistry`](crate::config::TokenRegistry) |
//! | [`Api`](CowError::Api) | Orderbook/subgraph returned non-2xx |
//! | [`Http`](CowError::Http) | Network transport failure |
//! | [`Signing`](CowError::Signing) | ECDSA / EIP-712 signing failure |
//! | [`Parse`](CowError::Parse) | Field parsing / deserialisation error |
//! | [`AppData`](CowError::AppData) | App-data encoding / hashing failure |
//! | [`Rpc`](CowError::Rpc) | JSON-RPC error from an Ethereum node |
//! | [`Unsupported`](CowError::Unsupported) | Feature not available on chain/config |
//! | [`Config`](CowError::Config) | SDK configuration error |
//! | [`ZeroQuantity`](CowError::ZeroQuantity) | Trade amount is zero |

/// Errors that can occur when interacting with the `CoW` Protocol SDK.
///
/// This is the unified error type returned by every fallible function in
/// the crate. Each variant carries enough context to produce a useful
/// diagnostic message via its [`Display`](std::fmt::Display)
/// implementation.
#[derive(Debug, thiserror::Error)]
pub enum CowError {
    /// The asset symbol is not in the [`TokenRegistry`](crate::config::TokenRegistry).
    #[error("unknown asset: {0}")]
    UnknownAsset(String),

    /// The `CoW` Protocol API returned a non-2xx response.
    #[error("cow api error {status}: {body}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Response body text.
        body: String,
    },

    /// An HTTP transport error from `reqwest`.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// EIP-712 signing failed.
    #[error("signing error: {0}")]
    Signing(String),

    /// The signal quantity is zero — nothing to trade.
    #[error("signal quantity is zero")]
    ZeroQuantity,

    /// A required field in a quote or order response could not be parsed.
    #[error("parse error for field '{field}': {reason}")]
    Parse {
        /// Field name that failed to parse.
        field: &'static str,
        /// Reason for the parse failure.
        reason: String,
    },

    /// App-data encoding or hashing failed.
    #[error("app-data error: {0}")]
    AppData(String),

    /// A JSON-RPC error returned by an Ethereum node.
    #[error("rpc error {code}: {message}")]
    Rpc {
        /// JSON-RPC error code (e.g., `-32602` for invalid params).
        code: i64,
        /// Human-readable error description from the node.
        message: String,
    },

    /// A feature or provider is not supported on the current chain or configuration.
    #[error("unsupported: {message}")]
    Unsupported {
        /// Human-readable description of what is not supported.
        message: String,
    },

    /// SDK configuration error (e.g. missing global adapter).
    #[error("config error: {0}")]
    Config(String),
}
