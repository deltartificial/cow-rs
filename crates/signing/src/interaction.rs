//! Interaction normalization for `CoW` Protocol settlement encoding.
//!
//! Mirrors `normalizeInteraction` and `normalizeInteractions` from the
//! `TypeScript` `contracts-ts` package.

use alloy_primitives::{Address, Bytes, U256};

/// Normalized interaction data for a settlement contract call.
///
/// Corresponds to the `TypeScript` `Interaction` type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interaction {
    /// Address of the smart contract to call.
    pub target: Address,
    /// Call value in wei.
    pub value: U256,
    /// Call data for the interaction.
    pub call_data: Bytes,
}

/// Partially specified interaction, where `value` and `call_data` are optional.
///
/// Corresponds to the `TypeScript` `InteractionLike` type.
#[derive(Debug, Clone)]
pub struct InteractionLike {
    /// Address of the smart contract to call.
    pub target: Address,
    /// Call value in wei (defaults to 0).
    pub value: Option<U256>,
    /// Call data for the interaction (defaults to empty).
    pub call_data: Option<Bytes>,
}

impl InteractionLike {
    /// Create a new interaction with only a target address.
    ///
    /// # Arguments
    ///
    /// * `target` — the smart contract address to call.
    ///
    /// # Returns
    ///
    /// An [`InteractionLike`] with `value` and `call_data` set to `None`.
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_signing::interaction::InteractionLike;
    ///
    /// let interaction = InteractionLike::new(Address::ZERO);
    /// assert!(interaction.value.is_none());
    /// assert!(interaction.call_data.is_none());
    /// ```
    #[must_use]
    pub const fn new(target: Address) -> Self {
        Self { target, value: None, call_data: None }
    }

    /// Set the call value.
    ///
    /// # Arguments
    ///
    /// * `value` — the call value in wei.
    ///
    /// # Returns
    ///
    /// The updated [`InteractionLike`] with `value` set.
    #[must_use]
    pub const fn with_value(mut self, value: U256) -> Self {
        self.value = Some(value);
        self
    }

    /// Set the call data.
    ///
    /// # Arguments
    ///
    /// * `call_data` — the encoded call data bytes.
    ///
    /// # Returns
    ///
    /// The updated [`InteractionLike`] with `call_data` set.
    #[must_use]
    pub fn with_call_data(mut self, call_data: Bytes) -> Self {
        self.call_data = Some(call_data);
        self
    }
}

/// Normalize an interaction by filling in defaults for optional fields.
///
/// - `value` defaults to `0`
/// - `call_data` defaults to empty bytes (`0x`)
///
/// Mirrors `normalizeInteraction` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `interaction` — the partially specified interaction to normalize.
///
/// # Returns
///
/// A fully populated [`Interaction`] with defaults applied for any missing
/// fields.
///
/// ```
/// use alloy_primitives::{Address, U256};
/// use cow_signing::interaction::{InteractionLike, normalize_interaction};
///
/// let like = InteractionLike::new(Address::ZERO);
/// let normalized = normalize_interaction(like);
/// assert_eq!(normalized.value, U256::ZERO);
/// assert!(normalized.call_data.is_empty());
/// ```
#[must_use]
pub fn normalize_interaction(interaction: InteractionLike) -> Interaction {
    Interaction {
        target: interaction.target,
        value: interaction.value.map_or(U256::ZERO, |v| v),
        call_data: interaction.call_data.unwrap_or_default(),
    }
}

/// Normalize a list of interactions by filling in defaults for optional fields.
///
/// Mirrors `normalizeInteractions` from the `TypeScript` SDK.
///
/// # Arguments
///
/// * `interactions` — the list of partially specified interactions.
///
/// # Returns
///
/// A `Vec` of fully populated [`Interaction`] values with defaults applied.
///
/// ```
/// use alloy_primitives::Address;
/// use cow_signing::interaction::{InteractionLike, normalize_interactions};
///
/// let interactions =
///     vec![InteractionLike::new(Address::ZERO), InteractionLike::new(Address::ZERO)];
/// let normalized = normalize_interactions(interactions);
/// assert_eq!(normalized.len(), 2);
/// ```
#[must_use]
pub fn normalize_interactions(interactions: Vec<InteractionLike>) -> Vec<Interaction> {
    interactions.into_iter().map(normalize_interaction).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::bytes;

    #[test]
    fn normalize_defaults() {
        let like = InteractionLike::new(Address::ZERO);
        let norm = normalize_interaction(like);
        assert_eq!(norm.target, Address::ZERO);
        assert_eq!(norm.value, U256::ZERO);
        assert!(norm.call_data.is_empty());
    }

    #[test]
    fn normalize_with_values() {
        let data = bytes!("deadbeef");
        let like = InteractionLike::new(Address::ZERO)
            .with_value(U256::from(42))
            .with_call_data(data.clone());
        let norm = normalize_interaction(like);
        assert_eq!(norm.value, U256::from(42));
        assert_eq!(norm.call_data, data);
    }

    #[test]
    fn normalize_multiple() {
        let interactions = vec![
            InteractionLike::new(Address::ZERO),
            InteractionLike::new(Address::ZERO).with_value(U256::from(1)),
        ];
        let normalized = normalize_interactions(interactions);
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].value, U256::ZERO);
        assert_eq!(normalized[1].value, U256::from(1));
    }
}
