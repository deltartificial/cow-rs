//! Settlement encoding and contract interaction layer for `CoW` Protocol.
//!
//! The encoder, simulator, vault and refunds helpers live in the
//! [`cow_settlement`] crate and are re-exported here for backwards
//! compatibility. The [`reader`] submodule stays in `cow-rs` until the
//! `onchain` transport crate is extracted.

pub mod reader;

pub use cow_settlement::{encoder, refunds, simulator, vault};
