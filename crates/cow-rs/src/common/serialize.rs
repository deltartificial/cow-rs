//! Custom serde serialisation helpers — ported from `utils/serialize.ts`.
//!
//! The `TypeScript` SDK provides a `jsonWithBigintReplacer` that converts
//! `BigInt` values to their decimal string representation when serialising
//! to JSON. In Rust, [`U256`] from `alloy_primitives` is the equivalent type.
//!
//! # Key items
//!
//! | Item | Purpose |
//! |---|---|
//! | [`u256_to_dec_string`] | Serde serialise function: `U256` → decimal string |
//! | [`U256DecString`] | Newtype wrapper for quick `serde_json::to_string` |
//! | [`json_with_bigint_replacer`] | Serialize any `T: Serialize` to JSON string |
//! | [`u256_string`] | Serde module for `#[serde(with = "…")]` round-trip |

use alloy_primitives::U256;
use serde::Serializer;

/// Serialize a [`U256`] as a decimal string.
///
/// This mirrors the `TypeScript` `jsonWithBigintReplacer` which converts
/// `BigInt` values to strings during `JSON.stringify`.
///
/// Attach to struct fields with
/// `#[serde(serialize_with = "u256_to_dec_string")]`, or call directly
/// from a custom `Serialize` implementation.
///
/// # Parameters
///
/// * `value` — the [`U256`] value to serialize.
/// * `serializer` — the serde [`Serializer`].
///
/// # Returns
///
/// The serializer's `Ok` type on success.
///
/// # Example
///
/// ```
/// use alloy_primitives::U256;
/// use cow_rs::common::serialize::u256_to_dec_string;
///
/// let val = U256::from(123_456_789u64);
/// let json = serde_json::to_string(&cow_rs::common::serialize::U256DecString(val)).unwrap();
/// assert_eq!(json, "\"123456789\"");
/// ```
#[allow(clippy::type_complexity, reason = "serde Serializer trait requires this signature")]
pub fn u256_to_dec_string<S: Serializer>(value: &U256, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

/// Wrapper type that serialises a [`U256`] as a decimal string.
///
/// Useful when you need a quick `serde_json::to_string` of a single
/// [`U256`] value without defining a full struct.
///
/// # Example
///
/// ```
/// use alloy_primitives::U256;
/// use cow_rs::common::serialize::U256DecString;
///
/// let json = serde_json::to_string(&U256DecString(U256::from(42u64))).unwrap();
/// assert_eq!(json, "\"42\"");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct U256DecString(pub U256);

impl serde::Serialize for U256DecString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

/// Convert a value containing `U256` fields into a JSON string, with all
/// `U256` values serialised as decimal strings.
///
/// This is the Rust equivalent of
/// `JSON.stringify(obj, jsonWithBigintReplacer)` from the `TypeScript` SDK.
///
/// **Note:** For `U256` fields to be serialised as strings, they must use
/// `#[serde(serialize_with = "u256_to_dec_string")]` or the
/// [`u256_string`] module. This function is a thin wrapper around
/// `serde_json::to_string`.
///
/// # Parameters
///
/// * `value` — any `Serialize`-able value.
///
/// # Returns
///
/// A JSON string.
///
/// # Errors
///
/// Returns a [`serde_json::Error`] if serialisation fails.
pub fn json_with_bigint_replacer<T: serde::Serialize>(
    value: &T,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(value)
}

/// Serde module for `U256` ↔ decimal string round-trip.
///
/// Attach to struct fields with
/// `#[serde(with = "cow_rs::common::serialize::u256_string")]` to
/// serialise `U256` as `"123456789"` and deserialise back.
///
/// # Example
///
/// ```
/// use alloy_primitives::U256;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// struct Wrapper {
///     #[serde(with = "cow_rs::common::serialize::u256_string")]
///     value: U256,
/// }
///
/// let w = Wrapper { value: U256::from(999u64) };
/// let json = serde_json::to_string(&w).unwrap();
/// assert!(json.contains("\"999\""));
/// let back: Wrapper = serde_json::from_str(&json).unwrap();
/// assert_eq!(back, w);
/// ```
pub mod u256_string {
    use alloy_primitives::U256;
    use serde::{Deserialize, Deserializer, Serializer};

    /// Serialize a `U256` as a decimal string.
    ///
    /// # Parameters
    ///
    /// * `value` — the [`U256`] to serialize.
    /// * `serializer` — the serde [`Serializer`].
    ///
    /// # Returns
    ///
    /// The serializer's `Ok` type on success.
    #[allow(clippy::type_complexity, reason = "serde Serializer trait requires this signature")]
    pub fn serialize<S: Serializer>(value: &U256, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    /// Deserialize a `U256` from a decimal string.
    ///
    /// # Parameters
    ///
    /// * `deserializer` — the serde [`Deserializer`].
    ///
    /// # Returns
    ///
    /// A [`U256`] parsed from the string.
    ///
    /// # Errors
    ///
    /// Returns a deserialisation error if the string is not a valid `U256`.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<U256, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<U256>().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u256_dec_string_serialises_correctly() {
        let w = U256DecString(U256::from(42u64));
        let json = serde_json::to_string(&w).unwrap();
        assert_eq!(json, "\"42\"");
    }

    #[test]
    fn u256_dec_string_large_value() {
        let w = U256DecString(U256::MAX);
        let json = serde_json::to_string(&w).unwrap();
        // MAX U256 is 2^256 - 1
        assert!(json.starts_with('"'));
        assert!(json.ends_with('"'));
        assert!(json.len() > 10);
    }

    #[test]
    fn u256_string_module_roundtrip() {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Wrapper {
            #[serde(with = "u256_string")]
            value: U256,
        }

        let w = Wrapper { value: U256::from(999u64) };
        let json = serde_json::to_string(&w).unwrap();
        assert!(json.contains("\"999\""));
        let back: Wrapper = serde_json::from_str(&json).unwrap();
        assert_eq!(back, w);
    }

    #[test]
    fn json_with_bigint_replacer_simple_struct() {
        use serde::Serialize;

        #[derive(Serialize)]
        struct Simple {
            name: String,
            count: u32,
        }

        let s = Simple { name: "test".into(), count: 42 };
        let json = json_with_bigint_replacer(&s).unwrap();
        assert!(json.contains("\"test\""));
        assert!(json.contains("42"));
    }

    #[test]
    fn u256_dec_string_zero() {
        let w = U256DecString(U256::ZERO);
        let json = serde_json::to_string(&w).unwrap();
        assert_eq!(json, "\"0\"");
    }

    #[test]
    fn u256_string_module_roundtrip_zero() {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Wrapper {
            #[serde(with = "u256_string")]
            value: U256,
        }

        let w = Wrapper { value: U256::ZERO };
        let json = serde_json::to_string(&w).unwrap();
        assert!(json.contains("\"0\""));
        let back: Wrapper = serde_json::from_str(&json).unwrap();
        assert_eq!(back, w);
    }

    #[test]
    fn u256_string_module_roundtrip_max() {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Wrapper {
            #[serde(with = "u256_string")]
            value: U256,
        }

        let w = Wrapper { value: U256::MAX };
        let json = serde_json::to_string(&w).unwrap();
        let back: Wrapper = serde_json::from_str(&json).unwrap();
        assert_eq!(back, w);
    }

    #[test]
    fn u256_string_deserialize_invalid_returns_error() {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(with = "u256_string")]
            #[allow(dead_code, reason = "field is tested via deserialization failure")]
            value: U256,
        }

        let result = serde_json::from_str::<Wrapper>(r#"{"value":"not-a-number"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn u256_to_dec_string_via_serialize_with() {
        use serde::Serialize;

        #[derive(Serialize)]
        struct Wrapper {
            #[serde(serialize_with = "u256_to_dec_string")]
            value: U256,
        }

        let w = Wrapper { value: U256::from(7777u64) };
        let json = serde_json::to_string(&w).unwrap();
        assert!(json.contains("\"7777\""));
    }

    #[test]
    fn json_with_bigint_replacer_with_u256_field() {
        use serde::Serialize;

        #[derive(Serialize)]
        struct WithU256 {
            #[serde(serialize_with = "u256_to_dec_string")]
            amount: U256,
        }

        let val = WithU256 { amount: U256::from(123u64) };
        let json = json_with_bigint_replacer(&val).unwrap();
        assert!(json.contains("\"123\""));
    }
}
