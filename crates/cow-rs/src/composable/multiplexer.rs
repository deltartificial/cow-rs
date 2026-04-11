//! [`Multiplexer`] — manages a set of conditional orders under a single Merkle root.

use std::fmt;

use alloy_primitives::{B256, keccak256};
use serde::{Deserialize, Serialize};

use crate::error::CowError;

use super::{
    order_id,
    types::{ConditionalOrderParams, ProofLocation},
};

/// Merkle inclusion proof for a single conditional order.
#[derive(Debug, Clone)]
pub struct OrderProof {
    /// Unique identifier of the order.
    pub order_id: B256,
    /// Sibling hashes from leaf to root (`OpenZeppelin` `MerkleTree` format).
    pub proof: Vec<B256>,
    /// The params needed to reconstruct the leaf.
    pub params: ConditionalOrderParams,
}

impl OrderProof {
    /// Construct an [`OrderProof`] from its constituent fields.
    ///
    /// # Arguments
    ///
    /// * `order_id` — unique identifier (`keccak256` of ABI-encoded params) for the order.
    /// * `proof` — sibling hashes from leaf to root in `OpenZeppelin` `MerkleTree` format.
    /// * `params` — the [`ConditionalOrderParams`] that define the order.
    ///
    /// # Returns
    ///
    /// A new [`OrderProof`] bundling the id, proof, and params together.
    #[must_use]
    pub const fn new(order_id: B256, proof: Vec<B256>, params: ConditionalOrderParams) -> Self {
        Self { order_id, proof, params }
    }

    /// Returns the number of Merkle proof siblings.
    ///
    /// # Returns
    ///
    /// The length of the `proof` vector, i.e. the number of sibling hashes
    /// needed to verify membership against the Merkle root.
    #[must_use]
    pub const fn proof_len(&self) -> usize {
        self.proof.len()
    }
}

/// Proof and params bundled for watchtower export.
#[derive(Debug, Clone)]
pub struct ProofWithParams {
    /// The order's inclusion proof.
    pub proof: Vec<B256>,
    /// The conditional order params.
    pub params: ConditionalOrderParams,
}

impl ProofWithParams {
    /// Construct a [`ProofWithParams`] bundle.
    ///
    /// # Arguments
    ///
    /// * `proof` — Merkle inclusion proof (sibling hashes from leaf to root).
    /// * `params` — the [`ConditionalOrderParams`] for the order.
    ///
    /// # Returns
    ///
    /// A new [`ProofWithParams`] ready for watchtower export or on-chain verification.
    #[must_use]
    pub const fn new(proof: Vec<B256>, params: ConditionalOrderParams) -> Self {
        Self { proof, params }
    }

    /// Returns the number of Merkle proof siblings.
    ///
    /// # Returns
    ///
    /// The length of the `proof` vector, i.e. the number of sibling hashes
    /// needed to verify membership against the Merkle root.
    #[must_use]
    pub const fn proof_len(&self) -> usize {
        self.proof.len()
    }
}

impl fmt::Display for OrderProof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "order-proof({:#x}, {} siblings)", self.order_id, self.proof.len())
    }
}

impl fmt::Display for ProofWithParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "proof-with-params({} siblings, handler={:#x})",
            self.proof.len(),
            self.params.handler
        )
    }
}

// ── JSON serialisation helpers ────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct MultiplexerJson {
    proof_location: u8,
    orders: Vec<ParamsJson>,
}

#[derive(Serialize, Deserialize)]
struct ParamsJson {
    handler: String,
    salt: String,
    static_input: String,
}

/// Watchtower export format: array of `{ proof, params }` objects.
#[derive(Deserialize)]
struct WatchtowerEntry {
    proof: Vec<String>,
    params: WatchtowerParams,
}

/// Params in watchtower camelCase format.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WatchtowerParams {
    handler: String,
    salt: String,
    static_input: String,
}

impl From<&ConditionalOrderParams> for ParamsJson {
    fn from(p: &ConditionalOrderParams) -> Self {
        Self {
            handler: format!("{:?}", p.handler),
            salt: format!("0x{}", alloy_primitives::hex::encode(p.salt.as_slice())),
            static_input: format!("0x{}", alloy_primitives::hex::encode(&p.static_input)),
        }
    }
}

impl TryFrom<ParamsJson> for ConditionalOrderParams {
    type Error = CowError;
    fn try_from(j: ParamsJson) -> Result<Self, CowError> {
        let handler = j
            .handler
            .parse()
            .map_err(|e: alloy_primitives::hex::FromHexError| CowError::AppData(e.to_string()))?;
        let salt_hex = j.salt.strip_prefix("0x").map_or(j.salt.as_str(), |s| s);
        let salt_bytes = alloy_primitives::hex::decode(salt_hex)
            .map_err(|e| CowError::AppData(format!("salt: {e}")))?;
        let mut salt = [0u8; 32];
        salt.copy_from_slice(&salt_bytes);
        let input_hex = j.static_input.strip_prefix("0x").map_or(j.static_input.as_str(), |s| s);
        let static_input = alloy_primitives::hex::decode(input_hex)
            .map_err(|e| CowError::AppData(format!("static_input: {e}")))?;
        Ok(Self { handler, salt: B256::new(salt), static_input })
    }
}

// ── Multiplexer ───────────────────────────────────────────────────────────────

/// Manages a set of conditional orders and computes their Merkle root.
///
/// The Merkle tree follows the `OpenZeppelin` `MerkleTree` standard used by
/// the `ComposableCow` contract:
///
/// - Leaf   = `keccak256(keccak256(abi.encode(params)))`
/// - Node   = `keccak256(min(left, right) ++ max(left, right))`
/// - Root is verified on-chain by `ComposableCow::setRoot`.
#[derive(Debug, Clone, Default)]
pub struct Multiplexer {
    orders: Vec<ConditionalOrderParams>,
    proof_location: ProofLocation,
}

impl Multiplexer {
    /// Create an empty multiplexer with the given proof location.
    ///
    /// # Arguments
    ///
    /// * `proof_location` — where the Merkle proofs will be stored or published (e.g.
    ///   [`ProofLocation::Emitted`], [`ProofLocation::Ipfs`]).
    ///
    /// # Returns
    ///
    /// A new, empty [`Multiplexer`] configured with the specified proof location.
    #[must_use]
    pub const fn new(proof_location: ProofLocation) -> Self {
        Self { orders: Vec::new(), proof_location }
    }

    /// Add a conditional order to the managed set.
    ///
    /// The order is appended to the end; its position index can be used with
    /// [`proof`](Self::proof) or [`get_by_index`](Self::get_by_index).
    ///
    /// # Arguments
    ///
    /// * `params` — the [`ConditionalOrderParams`] describing the order to add.
    pub fn add(&mut self, params: ConditionalOrderParams) {
        self.orders.push(params);
    }

    /// Remove the first conditional order whose [`order_id`] matches `id`.
    ///
    /// If no order matches, this is a no-op.
    ///
    /// # Arguments
    ///
    /// * `id` — the `keccak256`-based order identifier to match against.
    pub fn remove(&mut self, id: B256) {
        self.orders.retain(|p| order_id(p) != id);
    }

    /// Update the order at `index` with new params.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] if `index` is out of range.
    pub fn update(&mut self, index: usize, params: ConditionalOrderParams) -> Result<(), CowError> {
        if index >= self.orders.len() {
            return Err(CowError::AppData(format!(
                "index {index} out of range (len {})",
                self.orders.len()
            )));
        }
        self.orders[index] = params;
        Ok(())
    }

    /// Retrieve the order at `index`.
    ///
    /// # Arguments
    ///
    /// * `index` — zero-based position of the order in the managed set.
    ///
    /// # Returns
    ///
    /// `Some(&ConditionalOrderParams)` if the index is valid, or `None` if out of range.
    #[must_use]
    pub fn get_by_index(&self, index: usize) -> Option<&ConditionalOrderParams> {
        self.orders.get(index)
    }

    /// Retrieve the first order matching `id`.
    ///
    /// # Arguments
    ///
    /// * `id` — the `keccak256`-based order identifier to search for.
    ///
    /// # Returns
    ///
    /// `Some(&ConditionalOrderParams)` for the first order whose computed
    /// [`order_id`] equals `id`, or `None` if no order matches.
    #[must_use]
    pub fn get_by_id(&self, id: B256) -> Option<&ConditionalOrderParams> {
        self.orders.iter().find(|p| order_id(p) == id)
    }

    /// Number of orders currently managed.
    ///
    /// # Returns
    ///
    /// The count of conditional orders in the managed set.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.orders.len()
    }

    /// True if no orders are managed.
    ///
    /// # Returns
    ///
    /// `true` when the multiplexer contains zero orders, `false` otherwise.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    /// Compute the Merkle root of all managed orders.
    ///
    /// The root is computed using the `OpenZeppelin` `MerkleTree` algorithm:
    /// each leaf is `keccak256(keccak256(abi.encode(params)))` and internal
    /// nodes are `keccak256(min(left, right) ++ max(left, right))`.
    ///
    /// Returns `None` if there are no orders.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] if ABI encoding of any order fails.
    pub fn root(&self) -> Result<Option<B256>, CowError> {
        if self.orders.is_empty() {
            return Ok(None);
        }
        let leaves: Vec<B256> = self.orders.iter().map(leaf_hash).collect();
        Ok(Some(merkle_root(&leaves)))
    }

    /// Generate a Merkle inclusion proof for the order at position `index`.
    ///
    /// Returns the sibling hashes needed to verify membership against the root
    /// computed by [`root`](Self::root).
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] if `index` is out of range.
    pub fn proof(&self, index: usize) -> Result<OrderProof, CowError> {
        if index >= self.orders.len() {
            return Err(CowError::AppData(format!(
                "index {index} out of range (len {})",
                self.orders.len()
            )));
        }
        let leaves: Vec<B256> = self.orders.iter().map(leaf_hash).collect();
        Ok(OrderProof {
            order_id: order_id(&self.orders[index]),
            proof: generate_proof(&leaves, index),
            params: self.orders[index].clone(),
        })
    }

    /// Export all orders with their Merkle proofs — useful for watchtower services.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] if proof generation fails.
    pub fn dump_proofs_and_params(&self) -> Result<Vec<ProofWithParams>, CowError> {
        (0..self.orders.len())
            .map(|i| {
                let op = self.proof(i)?;
                Ok(ProofWithParams { proof: op.proof, params: op.params })
            })
            .collect()
    }

    /// Iterate over the order IDs of all managed conditional orders.
    ///
    /// Each ID is the `keccak256` hash of the ABI-encoded [`ConditionalOrderParams`]
    /// as computed by [`order_id`].
    ///
    /// # Returns
    ///
    /// An iterator yielding the [`B256`] identifier for each managed order.
    pub fn order_ids(&self) -> impl Iterator<Item = alloy_primitives::B256> + '_ {
        self.orders.iter().map(order_id)
    }

    /// Iterate over all managed conditional orders.
    ///
    /// # Returns
    ///
    /// An iterator yielding shared references to each [`ConditionalOrderParams`]
    /// in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &ConditionalOrderParams> {
        self.orders.iter()
    }

    /// View all managed orders as a slice.
    ///
    /// # Returns
    ///
    /// A borrowed slice of all [`ConditionalOrderParams`] in insertion order.
    #[must_use]
    pub fn as_slice(&self) -> &[ConditionalOrderParams] {
        &self.orders
    }

    /// Remove all managed orders.
    pub fn clear(&mut self) {
        self.orders.clear();
    }

    /// The configured proof location.
    ///
    /// # Returns
    ///
    /// The [`ProofLocation`] variant that was set at construction or via
    /// [`with_proof_location`](Self::with_proof_location).
    #[must_use]
    pub const fn proof_location(&self) -> ProofLocation {
        self.proof_location
    }

    /// Override the proof location and return `self` (builder style).
    ///
    /// # Arguments
    ///
    /// * `location` — the new [`ProofLocation`] to use.
    ///
    /// # Returns
    ///
    /// The same [`Multiplexer`] with its proof location updated, enabling
    /// builder-style chaining.
    #[must_use]
    pub const fn with_proof_location(mut self, location: ProofLocation) -> Self {
        self.proof_location = location;
        self
    }

    /// Consume the multiplexer and return the managed orders as a `Vec`.
    ///
    /// # Returns
    ///
    /// A `Vec<ConditionalOrderParams>` containing all orders that were managed
    /// by this multiplexer, in insertion order.
    #[must_use]
    pub fn into_vec(self) -> Vec<ConditionalOrderParams> {
        self.orders
    }

    /// Serialise the multiplexer to a JSON string.
    ///
    /// The output is a JSON object with `proof_location` (integer) and `orders`
    /// (array of `{ handler, salt, static_input }` hex strings).  Deserialise
    /// with [`Multiplexer::from_json`].
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] if serialisation fails.
    pub fn to_json(&self) -> Result<String, CowError> {
        let j = MultiplexerJson {
            proof_location: self.proof_location as u8,
            orders: self.orders.iter().map(ParamsJson::from).collect(),
        };
        serde_json::to_string(&j).map_err(|e| CowError::AppData(e.to_string()))
    }

    /// Decode a watchtower proof array from the JSON format used by the `CoW` Protocol
    /// watchtower service.
    ///
    /// The input must be a JSON array of `{ "proof": ["0x...", ...], "params": { "handler":
    /// "0x...", "salt": "0x...", "staticInput": "0x..." } }` objects.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] on parse or hex-decode failure.
    pub fn decode_proofs_from_json(json: &str) -> Result<Vec<ProofWithParams>, CowError> {
        let entries: Vec<WatchtowerEntry> =
            serde_json::from_str(json).map_err(|e| CowError::AppData(e.to_string()))?;
        entries
            .into_iter()
            .map(|entry| {
                let proof = entry
                    .proof
                    .iter()
                    .map(|s| {
                        let hex = s.strip_prefix("0x").map_or(s.as_str(), |h| h);
                        let bytes = alloy_primitives::hex::decode(hex)
                            .map_err(|e| CowError::AppData(format!("proof hash: {e}")))?;
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&bytes);
                        Ok(B256::new(arr))
                    })
                    .collect::<Result<Vec<_>, CowError>>()?;
                let p = entry.params;
                let handler =
                    p.handler.parse().map_err(|e: alloy_primitives::hex::FromHexError| {
                        CowError::AppData(e.to_string())
                    })?;
                let salt_hex = p.salt.strip_prefix("0x").map_or(p.salt.as_str(), |s| s);
                let salt_bytes = alloy_primitives::hex::decode(salt_hex)
                    .map_err(|e| CowError::AppData(format!("salt: {e}")))?;
                let mut salt = [0u8; 32];
                salt.copy_from_slice(&salt_bytes);
                let input_hex =
                    p.static_input.strip_prefix("0x").map_or(p.static_input.as_str(), |s| s);
                let static_input = alloy_primitives::hex::decode(input_hex)
                    .map_err(|e| CowError::AppData(format!("staticInput: {e}")))?;
                let params =
                    ConditionalOrderParams { handler, salt: B256::new(salt), static_input };
                Ok(ProofWithParams { proof, params })
            })
            .collect()
    }

    /// Deserialise a [`Multiplexer`] from a JSON string produced by [`Multiplexer::to_json`].
    ///
    /// # Errors
    ///
    /// Returns [`CowError::AppData`] on parse or decode failure.
    pub fn from_json(json: &str) -> Result<Self, CowError> {
        let j: MultiplexerJson =
            serde_json::from_str(json).map_err(|e| CowError::AppData(e.to_string()))?;
        let proof_location = match j.proof_location {
            0 => ProofLocation::Private,
            1 => ProofLocation::Emitted,
            2 => ProofLocation::Swarm,
            3 => ProofLocation::Waku,
            4 => ProofLocation::Reserved,
            5 => ProofLocation::Ipfs,
            n => {
                return Err(CowError::AppData(format!("unknown ProofLocation: {n}")));
            }
        };
        let orders = j
            .orders
            .into_iter()
            .map(ConditionalOrderParams::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { orders, proof_location })
    }
}
impl fmt::Display for Multiplexer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "multiplexer({} orders, {})", self.orders.len(), self.proof_location)
    }
}

// ── Merkle tree (OpenZeppelin standard) ───────────────────────────────────────

/// Double-hash a leaf per the `OpenZeppelin` `MerkleTree` convention.
///
/// # Arguments
///
/// * `params` — the conditional order parameters whose ABI encoding is hashed.
///
/// # Returns
///
/// `keccak256(order_id(params))`, where `order_id` is itself
/// `keccak256(abi.encode(params))`, producing the double-hash leaf expected
/// by the `OpenZeppelin` Merkle tree.
fn leaf_hash(params: &ConditionalOrderParams) -> B256 {
    keccak256(order_id(params))
}

/// Compute the Merkle root from pre-hashed leaves using sorted pair hashing.
///
/// # Arguments
///
/// * `leaves` — non-empty slice of double-hashed leaf values.
///
/// # Returns
///
/// The single `B256` root hash produced by iteratively combining pairs of
/// nodes with [`hash_pair`] until one node remains. An odd trailing node is
/// promoted unchanged.
fn merkle_root(leaves: &[B256]) -> B256 {
    if leaves.len() == 1 {
        return leaves[0];
    }
    let mut layer = leaves.to_vec();
    while layer.len() > 1 {
        let mut next = Vec::with_capacity(layer.len().div_ceil(2));
        let mut i = 0;
        while i < layer.len() {
            if i + 1 < layer.len() {
                next.push(hash_pair(layer[i], layer[i + 1]));
            } else {
                next.push(layer[i]);
            }
            i += 2;
        }
        layer = next;
    }
    layer[0]
}

/// Hash a sorted pair of nodes for the Merkle tree.
///
/// # Arguments
///
/// * `a` — first node hash.
/// * `b` — second node hash.
///
/// # Returns
///
/// `keccak256(min(a, b) ++ max(a, b))` — the canonical sorted-pair hash
/// used by the `OpenZeppelin` `MerkleTree` implementation.
fn hash_pair(a: B256, b: B256) -> B256 {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let mut buf = [0u8; 64];
    buf[..32].copy_from_slice(lo.as_slice());
    buf[32..].copy_from_slice(hi.as_slice());
    keccak256(buf)
}

/// Generate a Merkle inclusion proof for the leaf at `index`.
///
/// # Arguments
///
/// * `leaves` — the full set of double-hashed leaf values.
/// * `index` — zero-based position of the target leaf within `leaves`.
///
/// # Returns
///
/// A `Vec<B256>` of sibling hashes ordered from leaf level to root level.
/// Together with the leaf itself, these siblings are sufficient to
/// reconstruct and verify the Merkle root.
fn generate_proof(leaves: &[B256], mut index: usize) -> Vec<B256> {
    let mut proof = Vec::new();
    let mut layer = leaves.to_vec();
    while layer.len() > 1 {
        let sibling = if index.is_multiple_of(2) {
            (index + 1 < layer.len()).then(|| layer[index + 1])
        } else {
            Some(layer[index - 1])
        };
        if let Some(s) = sibling {
            proof.push(s);
        }
        let mut next = Vec::with_capacity(layer.len().div_ceil(2));
        let mut i = 0;
        while i < layer.len() {
            if i + 1 < layer.len() {
                next.push(hash_pair(layer[i], layer[i + 1]));
            } else {
                next.push(layer[i]);
            }
            i += 2;
        }
        layer = next;
        index /= 2;
    }
    proof
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;

    use super::*;

    fn make_params(salt_byte: u8) -> ConditionalOrderParams {
        ConditionalOrderParams {
            handler: Address::ZERO,
            salt: B256::new([salt_byte; 32]),
            static_input: vec![salt_byte; 4],
        }
    }

    #[test]
    fn decode_proofs_from_json_roundtrip() {
        // Build a multiplexer, export proofs, serialise to watchtower JSON, decode back.
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(0xaa));
        mux.add(make_params(0xbb));

        let proofs = mux.dump_proofs_and_params().unwrap();

        // Serialise to watchtower JSON format manually.
        let json_entries: Vec<serde_json::Value> = proofs
            .iter()
            .map(|p| {
                let proof_arr: Vec<String> = p
                    .proof
                    .iter()
                    .map(|h| format!("0x{}", alloy_primitives::hex::encode(h.as_slice())))
                    .collect();
                serde_json::json!({
                    "proof": proof_arr,
                    "params": {
                        "handler": format!("{:#x}", p.params.handler),
                        "salt": format!("0x{}", alloy_primitives::hex::encode(p.params.salt.as_slice())),
                        "staticInput": format!("0x{}", alloy_primitives::hex::encode(&p.params.static_input)),
                    }
                })
            })
            .collect();
        let json = serde_json::to_string(&json_entries).unwrap();

        let decoded = Multiplexer::decode_proofs_from_json(&json).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].params.salt, proofs[0].params.salt);
        assert_eq!(decoded[1].params.static_input, proofs[1].params.static_input);
    }

    #[test]
    fn decode_proofs_from_json_invalid_returns_error() {
        let result = Multiplexer::decode_proofs_from_json("not json");
        assert!(result.is_err());
    }

    #[test]
    fn multiplexer_root_single_order() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(1));
        let root = mux.root().unwrap();
        assert!(root.is_some());
    }

    #[test]
    fn multiplexer_root_empty() {
        let mux = Multiplexer::new(ProofLocation::Private);
        assert!(mux.root().unwrap().is_none());
    }

    // ── add / remove / update ────────────────────────────────────────────

    #[test]
    fn add_increases_len() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        assert!(mux.is_empty());
        mux.add(make_params(1));
        assert_eq!(mux.len(), 1);
        mux.add(make_params(2));
        assert_eq!(mux.len(), 2);
    }

    #[test]
    fn remove_by_id() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        let p = make_params(0xaa);
        let id = order_id(&p);
        mux.add(p);
        mux.add(make_params(0xbb));
        assert_eq!(mux.len(), 2);
        mux.remove(id);
        assert_eq!(mux.len(), 1);
    }

    #[test]
    fn remove_nonexistent_is_noop() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(1));
        mux.remove(B256::ZERO);
        assert_eq!(mux.len(), 1);
    }

    #[test]
    fn update_in_range() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(1));
        mux.add(make_params(2));
        let new_params = make_params(99);
        mux.update(1, new_params.clone()).unwrap();
        assert_eq!(mux.get_by_index(1).unwrap().salt, new_params.salt);
    }

    #[test]
    fn update_out_of_range() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(1));
        assert!(mux.update(5, make_params(2)).is_err());
    }

    // ── get_by_index / get_by_id ─────────────────────────────────────────

    #[test]
    fn get_by_index_valid() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        let p = make_params(0xcc);
        mux.add(p.clone());
        let got = mux.get_by_index(0).unwrap();
        assert_eq!(got.salt, p.salt);
    }

    #[test]
    fn get_by_index_out_of_range() {
        let mux = Multiplexer::new(ProofLocation::Private);
        assert!(mux.get_by_index(0).is_none());
    }

    #[test]
    fn get_by_id_found() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        let p = make_params(0xdd);
        let id = order_id(&p);
        mux.add(p.clone());
        let got = mux.get_by_id(id).unwrap();
        assert_eq!(got.salt, p.salt);
    }

    #[test]
    fn get_by_id_not_found() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(1));
        assert!(mux.get_by_id(B256::ZERO).is_none());
    }

    // ── root / proof ─────────────────────────────────────────────────────

    #[test]
    fn root_changes_when_order_added() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(1));
        let root1 = mux.root().unwrap().unwrap();
        mux.add(make_params(2));
        let root2 = mux.root().unwrap().unwrap();
        assert_ne!(root1, root2);
    }

    #[test]
    fn root_two_orders() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(0xaa));
        mux.add(make_params(0xbb));
        let root = mux.root().unwrap();
        assert!(root.is_some());
    }

    #[test]
    fn proof_valid_index() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(0xaa));
        mux.add(make_params(0xbb));
        let proof = mux.proof(0).unwrap();
        assert!(!proof.proof.is_empty());
        assert_eq!(proof.params.salt, make_params(0xaa).salt);
    }

    #[test]
    fn proof_out_of_range() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(1));
        assert!(mux.proof(5).is_err());
    }

    // ── dump_proofs_and_params ───────────────────────────────────────────

    #[test]
    fn dump_proofs_and_params_returns_all() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(0xaa));
        mux.add(make_params(0xbb));
        mux.add(make_params(0xcc));
        let proofs = mux.dump_proofs_and_params().unwrap();
        assert_eq!(proofs.len(), 3);
    }

    // ── to_json / from_json roundtrip ────────────────────────────────────

    #[test]
    fn to_json_from_json_roundtrip() {
        let mut mux = Multiplexer::new(ProofLocation::Ipfs);
        mux.add(make_params(0x11));
        mux.add(make_params(0x22));

        let json = mux.to_json().unwrap();
        let restored = Multiplexer::from_json(&json).unwrap();

        assert_eq!(restored.len(), 2);
        assert_eq!(restored.proof_location(), ProofLocation::Ipfs);
        assert_eq!(restored.get_by_index(0).unwrap().salt, make_params(0x11).salt);
        assert_eq!(restored.get_by_index(1).unwrap().salt, make_params(0x22).salt);
    }

    #[test]
    fn from_json_invalid() {
        assert!(Multiplexer::from_json("not json").is_err());
    }

    #[test]
    fn from_json_unknown_proof_location() {
        let json = r#"{"proof_location": 99, "orders": []}"#;
        assert!(Multiplexer::from_json(json).is_err());
    }

    // ── miscellaneous ────────────────────────────────────────────────────

    #[test]
    fn clear_empties_multiplexer() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(1));
        mux.add(make_params(2));
        mux.clear();
        assert!(mux.is_empty());
    }

    #[test]
    fn order_ids_iterator() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(0xaa));
        mux.add(make_params(0xbb));
        let ids: Vec<_> = mux.order_ids().collect();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], order_id(&make_params(0xaa)));
    }

    #[test]
    fn iter_and_as_slice() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(1));
        mux.add(make_params(2));
        assert_eq!(mux.iter().count(), 2);
        assert_eq!(mux.as_slice().len(), 2);
    }

    #[test]
    fn into_vec_returns_orders() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(1));
        let v = mux.into_vec();
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn with_proof_location_builder() {
        let mux = Multiplexer::new(ProofLocation::Private).with_proof_location(ProofLocation::Swarm);
        assert_eq!(mux.proof_location(), ProofLocation::Swarm);
    }

    #[test]
    fn display_multiplexer() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(1));
        let s = format!("{mux}");
        assert!(s.contains("1 orders"));
    }

    #[test]
    fn display_order_proof() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(0xaa));
        mux.add(make_params(0xbb));
        let proof = mux.proof(0).unwrap();
        let s = format!("{proof}");
        assert!(s.contains("order-proof"));
    }

    #[test]
    fn display_proof_with_params() {
        let mut mux = Multiplexer::new(ProofLocation::Private);
        mux.add(make_params(0xaa));
        mux.add(make_params(0xbb));
        let proofs = mux.dump_proofs_and_params().unwrap();
        let s = format!("{}", proofs[0]);
        assert!(s.contains("proof-with-params"));
    }

    #[test]
    fn order_proof_new_and_proof_len() {
        let op = OrderProof::new(B256::ZERO, vec![B256::ZERO, B256::ZERO], make_params(1));
        assert_eq!(op.proof_len(), 2);
    }

    #[test]
    fn proof_with_params_new_and_proof_len() {
        let pwp = ProofWithParams::new(vec![B256::ZERO], make_params(1));
        assert_eq!(pwp.proof_len(), 1);
    }
}
