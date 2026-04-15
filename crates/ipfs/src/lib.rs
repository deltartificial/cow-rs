//! `cow-sdk-ipfs` — Layer 4 IPFS client for the `CoW` Protocol SDK.
//!
//! **Placeholder**: the IPFS fetch/upload helpers currently live inside
//! [`cow_sdk_app_data::ipfs`] for convenience. They will migrate into this
//! crate once the `IpfsClient` trait and Pinata adapter are finalised so
//! that `cow-sdk-app-data` can drop its `reqwest` dependency and become a
//! true L2 domain crate.

#![deny(unsafe_code)]
#![warn(missing_docs)]
