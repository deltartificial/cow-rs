//! ABI-encoded calldata builders for the `ComposableCow` contract.
//!
//! Function signatures:
//! - `create((address,bytes32,bytes),bytes32[2][],bytes)`
//! - `createWithContext((address,bytes32,bytes),address,bytes,bool)`
//! - `remove(bytes32)`

use alloy_primitives::{Address, B256, keccak256};

use super::types::{ConditionalOrderParams, ProofStruct};

// ── Function selectors ────────────────────────────────────────────────────────

/// Compute the 4-byte selector from a Solidity function signature.
///
/// # Arguments
///
/// * `sig` - A Solidity function signature string (e.g. `"remove(bytes32)"`). The full canonical
///   form including parameter types is required for a correct `keccak256` hash.
///
/// # Returns
///
/// The first 4 bytes of the `keccak256` hash of `sig`, which is the standard
/// Solidity function selector.
fn selector(sig: &str) -> [u8; 4] {
    let h = keccak256(sig.as_bytes());
    [h[0], h[1], h[2], h[3]]
}

// ── createWithContext ─────────────────────────────────────────────────────────

/// Encode calldata for `ComposableCow::createWithContext`.
///
/// Used when a `TWAP` order has `start_time = AtMiningTime` (`t0 = 0`).
/// Pass [`super::types::CURRENT_BLOCK_TIMESTAMP_FACTORY_ADDRESS`] as `factory`
/// and `&[]` as `factory_data` for standard TWAP orders.
///
/// Encodes:
/// ```text
/// createWithContext(
///   (address handler, bytes32 salt, bytes staticInput),
///   address factory,          // IValueFactory address
///   bytes   factoryData,      // context factory data (empty for timestamp factory)
///   bool    dispatch,         // whether to emit ConditionalOrderCreated log
/// )
/// ```
///
/// # Arguments
///
/// * `params` - The conditional order parameters (handler address, salt, and ABI-encoded static
///   input).
/// * `factory` - The `IValueFactory` contract address. For standard TWAP orders with `start_time =
///   AtMiningTime`, pass [`super::types::CURRENT_BLOCK_TIMESTAMP_FACTORY_ADDRESS`].
/// * `factory_data` - Opaque bytes forwarded to the factory. Pass `&[]` for the timestamp factory.
/// * `dispatch` - When `true`, the contract emits a `ConditionalOrderCreated` event so watchtower
///   services can index the order.
///
/// # Returns
///
/// A `Vec<u8>` containing the full ABI-encoded calldata (4-byte selector
/// followed by the encoded arguments), ready to be submitted as a transaction's
/// `data` field.
#[must_use]
pub fn create_with_context_calldata(
    params: &ConditionalOrderParams,
    factory: Address,
    factory_data: &[u8],
    dispatch: bool,
) -> Vec<u8> {
    let sel = selector("createWithContext((address,bytes32,bytes),address,bytes,bool)");

    let static_input = &params.static_input;
    let static_input_padded_len = padded32(static_input.len());
    // tuple: (address[32], bytes32[32], offset[32], len[32], data[padded])
    let tuple_size = 3 * 32 + 32 + static_input_padded_len;

    let factory_data_padded_len = padded32(factory_data.len());
    // factory_data encoding: length[32] + data[padded]
    let factory_data_enc_size = 32 + factory_data_padded_len;

    // Top-level args: tuple(dynamic), address(static), bytes(dynamic), bool(static)
    // ABI head = 4 slots (tuple offset, address inline, bytes offset, bool inline)
    let offset_tuple: u64 = 4 * 32;
    let offset_factory_data: u64 = offset_tuple + tuple_size as u64;

    let total = 4 + 4 * 32 + tuple_size + factory_data_enc_size;
    let mut buf = Vec::with_capacity(total);
    buf.extend_from_slice(&sel);

    // Head slots
    buf.extend_from_slice(&u256_be(offset_tuple)); // offset to tuple
    buf.extend_from_slice(&pad_address(factory.as_slice())); // factory address (static)
    buf.extend_from_slice(&u256_be(offset_factory_data)); // offset to factoryData
    buf.extend_from_slice(&u256_be(u64::from(dispatch))); // bool dispatch

    // Encode tuple (address, bytes32, bytes)
    buf.extend_from_slice(&pad_address(params.handler.as_slice()));
    buf.extend_from_slice(params.salt.as_slice());
    buf.extend_from_slice(&u256_be(96u64)); // offset to staticInput within tuple = 3*32
    buf.extend_from_slice(&u256_be(static_input.len() as u64));
    buf.extend_from_slice(static_input);
    pad_to(&mut buf, static_input.len());

    // Encode factory_data: bytes
    buf.extend_from_slice(&u256_be(factory_data.len() as u64));
    buf.extend_from_slice(factory_data);
    pad_to(&mut buf, factory_data.len());

    buf
}

// ── setRoot ───────────────────────────────────────────────────────────────────

/// Encode calldata for `ComposableCow::setRoot`.
///
/// Sets the Merkle root for a multiplexed set of conditional orders.
///
/// ```text
/// setRoot(
///   bytes32                        root,
///   (uint256 location, bytes data) proof,
///   (address handler, bytes32 salt, bytes staticInput) params,
/// )
/// ```
///
/// The `proof` argument describes where watchtower services can find the full
/// Merkle proof.  For on-chain emission use [`super::types::ProofLocation::Emitted`]
/// together with the actual proof bytes; for private orders pass
/// [`super::types::ProofLocation::Private`] and `data = &[]`.
///
/// # Arguments
///
/// * `root` - The 32-byte Merkle root hash that commits to the full set of conditional orders.
/// * `proof` - A [`ProofStruct`] describing where the Merkle proof data is stored and the proof
///   bytes themselves.
/// * `params` - The conditional order parameters (handler, salt, static input) for the order being
///   registered under this root.
///
/// # Returns
///
/// A `Vec<u8>` containing the full ABI-encoded calldata (4-byte selector
/// followed by the encoded arguments), ready to be submitted as a transaction's
/// `data` field.
#[must_use]
pub fn set_root_calldata(
    root: B256,
    proof: &ProofStruct,
    params: &ConditionalOrderParams,
) -> Vec<u8> {
    let sel = selector("setRoot(bytes32,(uint256,bytes),(address,bytes32,bytes))");

    let proof_enc = encode_proof_struct(proof);
    let params_enc = encode_params_tuple(params);

    // Top-level head: root (inline), proof offset, params offset.
    // Offsets are relative to the start of the args section (after selector).
    // Head = 3 slots = 96 bytes; root uses slot 0 inline.
    let proof_offset: u64 = 3 * 32;
    let params_offset: u64 = proof_offset + proof_enc.len() as u64;

    let mut buf = Vec::with_capacity(4 + 3 * 32 + proof_enc.len() + params_enc.len());
    buf.extend_from_slice(&sel);
    buf.extend_from_slice(root.as_slice());
    buf.extend_from_slice(&u256_be(proof_offset));
    buf.extend_from_slice(&u256_be(params_offset));
    buf.extend_from_slice(&proof_enc);
    buf.extend_from_slice(&params_enc);
    buf
}

/// Encode calldata for `ComposableCow::setRootWithContext`.
///
/// Like [`set_root_calldata`] but also supplies a value factory that writes
/// the mining-time timestamp into the `ComposableCow` cabinet, enabling
/// `AtMiningTime` `TWAP` orders.
///
/// ```text
/// setRootWithContext(
///   bytes32                        root,
///   (uint256 location, bytes data) proof,
///   (address handler, bytes32 salt, bytes staticInput) params,
///   address                        factory,
///   bytes                          factoryData,
/// )
/// ```
///
/// # Arguments
///
/// * `root` - The 32-byte Merkle root hash that commits to the full set of conditional orders.
/// * `proof` - A [`ProofStruct`] describing where the Merkle proof data is stored and the proof
///   bytes themselves.
/// * `params` - The conditional order parameters (handler, salt, static input) for the order being
///   registered under this root.
/// * `factory` - The `IValueFactory` contract address (typically
///   [`super::types::CURRENT_BLOCK_TIMESTAMP_FACTORY_ADDRESS`]).
/// * `factory_data` - Opaque bytes forwarded to the factory. Pass `&[]` for the timestamp factory.
///
/// # Returns
///
/// A `Vec<u8>` containing the full ABI-encoded calldata (4-byte selector
/// followed by the encoded arguments), ready to be submitted as a transaction's
/// `data` field.
#[must_use]
pub fn set_root_with_context_calldata(
    root: B256,
    proof: &ProofStruct,
    params: &ConditionalOrderParams,
    factory: Address,
    factory_data: &[u8],
) -> Vec<u8> {
    let sel = selector(
        "setRootWithContext(bytes32,(uint256,bytes),(address,bytes32,bytes),address,bytes)",
    );

    let proof_enc = encode_proof_struct(proof);
    let params_enc = encode_params_tuple(params);

    let factory_data_padded_len = padded32(factory_data.len());
    let factory_data_enc_size = 32 + factory_data_padded_len; // length word + data

    // Head: root (inline), proof offset, params offset, factory (inline), factory_data offset.
    // 5 slots = 160 bytes.
    let proof_offset: u64 = 5 * 32;
    let params_offset: u64 = proof_offset + proof_enc.len() as u64;
    let factory_data_offset: u64 = params_offset + params_enc.len() as u64;

    let mut buf =
        Vec::with_capacity(4 + 5 * 32 + proof_enc.len() + params_enc.len() + factory_data_enc_size);
    buf.extend_from_slice(&sel);
    buf.extend_from_slice(root.as_slice());
    buf.extend_from_slice(&u256_be(proof_offset));
    buf.extend_from_slice(&u256_be(params_offset));
    buf.extend_from_slice(&pad_address(factory.as_slice()));
    buf.extend_from_slice(&u256_be(factory_data_offset));
    buf.extend_from_slice(&proof_enc);
    buf.extend_from_slice(&params_enc);
    // factory_data: bytes
    buf.extend_from_slice(&u256_be(factory_data.len() as u64));
    buf.extend_from_slice(factory_data);
    pad_to(&mut buf, factory_data.len());
    buf
}

// ── remove ────────────────────────────────────────────────────────────────────

/// Encode calldata for `ComposableCow::remove(bytes32 id)`.
///
/// Returns 36 bytes: 4-byte selector + 32-byte order ID.
///
/// # Arguments
///
/// * `order_id` - The 32-byte identifier of the conditional order to remove, as returned by
///   `ComposableCow.hash()`.
///
/// # Returns
///
/// A 36-byte `Vec<u8>` containing the 4-byte function selector followed by the
/// 32-byte order ID.
#[must_use]
pub fn remove_calldata(order_id: B256) -> Vec<u8> {
    let sel = selector("remove(bytes32)");
    let mut buf = Vec::with_capacity(36);
    buf.extend_from_slice(&sel);
    buf.extend_from_slice(order_id.as_slice());
    buf
}

// ── create ────────────────────────────────────────────────────────────────────

/// Encode calldata for `ComposableCow::create`.
///
/// Encodes:
/// ```text
/// create(
///   (address handler, bytes32 salt, bytes staticInput),
///   bytes32[2][] proof,      // empty for single orders
///   bytes offchainInput      // empty unless handler uses it
/// )
/// ```
///
/// `proof` is the Merkle sibling-hash array — pass an empty slice for single
/// (non-multiplexed) orders. `offchain_input` is handler-specific; pass `&[]`.
///
/// # Arguments
///
/// * `params` - The conditional order parameters (handler address, salt, and ABI-encoded static
///   input).
/// * `proof` - A slice of `[B256; 2]` Merkle sibling-hash pairs. Pass an empty slice (`&[]`) for
///   single (non-multiplexed) orders.
/// * `offchain_input` - Handler-specific off-chain input bytes. Pass `&[]` unless the handler
///   requires additional data.
///
/// # Returns
///
/// A `Vec<u8>` containing the full ABI-encoded calldata (4-byte selector
/// followed by the encoded arguments), ready to be submitted as a transaction's
/// `data` field.
#[must_use]
pub fn create_calldata(
    params: &ConditionalOrderParams,
    proof: &[[B256; 2]],
    offchain_input: &[u8],
) -> Vec<u8> {
    let sel = selector("create((address,bytes32,bytes),bytes32[2][],bytes)");

    // ABI encoding of (tuple, bytes32[2][], bytes) with 3 dynamic top-level args.
    // Head section (3 * 32 bytes): offsets to each arg.
    // Then each arg's encoding follows.

    // Encode the params tuple: (address, bytes32, bytes)
    // The tuple itself is dynamic because `bytes` is dynamic.
    //   slot 0: offset to handler (address = static, but tuple is treated as its own ABI struct)
    // Actually for a tuple with a dynamic member, encode inline:
    //   address  handler  — left-padded 32 bytes (static)
    //   bytes32  salt     — 32 bytes (static)
    //   uint256  offset   — offset to staticInput bytes (relative to tuple start) = 3*32 = 96
    //   uint256  length   — len(staticInput)
    //   bytes    data     — staticInput padded to 32-byte boundary
    let static_input = &params.static_input;
    let static_input_padded_len = padded32(static_input.len());

    let tuple_size = 3 * 32 + 32 + static_input_padded_len;

    // Encode proof: bytes32[2][] — dynamic array of fixed-size pairs
    let proof_size = 32 + proof.len() * 64; // length word + data

    // Encode offchain_input: bytes — dynamic
    let offchain_padded_len = padded32(offchain_input.len());
    let offchain_enc_size = 32 + offchain_padded_len;

    // Top-level offsets (relative to start of args section):
    let offset_tuple: u64 = 3 * 32; // right after the 3 head slots
    let offset_proof: u64 = offset_tuple + tuple_size as u64;
    let offset_offchain: u64 = offset_proof + proof_size as u64;

    let total = 4 + 3 * 32 + tuple_size + proof_size + offchain_enc_size;
    let mut buf = Vec::with_capacity(total);
    buf.extend_from_slice(&sel);

    // Head: 3 offset words
    buf.extend_from_slice(&u256_be(offset_tuple));
    buf.extend_from_slice(&u256_be(offset_proof));
    buf.extend_from_slice(&u256_be(offset_offchain));

    // Encode tuple (address, bytes32, bytes)
    buf.extend_from_slice(&pad_address(params.handler.as_slice()));
    buf.extend_from_slice(params.salt.as_slice());
    buf.extend_from_slice(&u256_be(96u64)); // offset to staticInput within tuple = 3*32
    buf.extend_from_slice(&u256_be(static_input.len() as u64));
    buf.extend_from_slice(static_input);
    pad_to(&mut buf, static_input.len());

    // Encode proof: bytes32[2][]
    buf.extend_from_slice(&u256_be(proof.len() as u64));
    for pair in proof {
        buf.extend_from_slice(pair[0].as_slice());
        buf.extend_from_slice(pair[1].as_slice());
    }

    // Encode offchain_input: bytes
    buf.extend_from_slice(&u256_be(offchain_input.len() as u64));
    buf.extend_from_slice(offchain_input);
    pad_to(&mut buf, offchain_input.len());

    buf
}

// ── Shared tuple encoders ─────────────────────────────────────────────────────

/// ABI-encode a `(uint256 location, bytes data)` proof struct.
///
/// # Arguments
///
/// * `proof` - A [`ProofStruct`] containing the proof location discriminant and the associated
///   proof bytes.
///
/// # Returns
///
/// A `Vec<u8>` with the ABI-encoded tuple: `location` as a 32-byte word
/// followed by the dynamic `bytes data` encoding (offset word, length word,
/// and zero-padded payload).
fn encode_proof_struct(proof: &ProofStruct) -> Vec<u8> {
    let data_padded_len = padded32(proof.data.len());
    // head: location (32) + offset-to-data (32) = 64 bytes; then data encoding
    let mut out = Vec::with_capacity(64 + 32 + data_padded_len);
    out.extend_from_slice(&u256_be(proof.location as u64));
    out.extend_from_slice(&u256_be(64u64)); // offset to data = 2 head slots = 64
    out.extend_from_slice(&u256_be(proof.data.len() as u64));
    out.extend_from_slice(&proof.data);
    pad_to(&mut out, proof.data.len());
    out
}

/// ABI-encode a `(address handler, bytes32 salt, bytes staticInput)` params tuple.
///
/// # Arguments
///
/// * `params` - The [`ConditionalOrderParams`] to encode, containing the handler address, a 32-byte
///   salt, and the variable-length static input.
///
/// # Returns
///
/// A `Vec<u8>` with the ABI-encoded tuple: left-padded handler address,
/// salt word, dynamic offset to `staticInput`, length word, and zero-padded
/// static input payload.
fn encode_params_tuple(params: &ConditionalOrderParams) -> Vec<u8> {
    let si = &params.static_input;
    let si_padded_len = padded32(si.len());
    let mut out = Vec::with_capacity(3 * 32 + 32 + si_padded_len);
    out.extend_from_slice(&pad_address(params.handler.as_slice()));
    out.extend_from_slice(params.salt.as_slice());
    out.extend_from_slice(&u256_be(96u64)); // offset to staticInput = 3 head slots = 96
    out.extend_from_slice(&u256_be(si.len() as u64));
    out.extend_from_slice(si);
    pad_to(&mut out, si.len());
    out
}

// ── ABI helpers ───────────────────────────────────────────────────────────────

/// Left-pad an address (or shorter slice) to 32 bytes.
///
/// # Arguments
///
/// * `bytes` - A 20-byte Ethereum address slice (or any slice up to 20 bytes).
///
/// # Returns
///
/// A `[u8; 32]` array with the input right-aligned (left-padded with zeroes),
/// matching ABI encoding of `address` types.
fn pad_address(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(bytes);
    out
}

/// Encode a `u64` as a 32-byte big-endian ABI word.
///
/// # Arguments
///
/// * `v` - The value to encode. It is placed in the lowest 8 bytes of the 32-byte word
///   (big-endian), with the upper 24 bytes zeroed.
///
/// # Returns
///
/// A `[u8; 32]` big-endian ABI word representing `v` as a `uint256`.
fn u256_be(v: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&v.to_be_bytes());
    out
}

/// Round `n` up to the next multiple of 32.
///
/// # Arguments
///
/// * `n` - The byte length to round up.
///
/// # Returns
///
/// The smallest value >= `n` that is a multiple of 32. Returns `n` unchanged
/// if it is already a multiple of 32.
const fn padded32(n: usize) -> usize {
    if n.is_multiple_of(32) { n } else { n + (32 - n % 32) }
}

/// Zero-pad `buf` to the next 32-byte boundary after `written` bytes.
///
/// # Arguments
///
/// * `buf` - The buffer to extend with zero bytes.
/// * `written` - The number of data bytes most recently appended to `buf`. If `written` is not a
///   multiple of 32, enough zero bytes are appended to reach the next 32-byte boundary. No bytes
///   are added when `written` is already aligned.
fn pad_to(buf: &mut Vec<u8>, written: usize) {
    let rem = written % 32;
    if rem != 0 {
        buf.resize(buf.len() + (32 - rem), 0);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::composable::types::ProofLocation;

    use super::*;

    fn dummy_params() -> ConditionalOrderParams {
        ConditionalOrderParams {
            handler: Address::ZERO,
            salt: B256::ZERO,
            static_input: vec![0xaau8; 32],
        }
    }

    #[test]
    fn set_root_calldata_has_correct_selector() {
        let expected_sel = {
            let h = keccak256(b"setRoot(bytes32,(uint256,bytes),(address,bytes32,bytes))");
            [h[0], h[1], h[2], h[3]]
        };
        let proof = ProofStruct { location: ProofLocation::Private, data: vec![] };
        let cd = set_root_calldata(B256::ZERO, &proof, &dummy_params());
        assert_eq!(&cd[..4], expected_sel);
        // 4 (sel) + 3*32 (head: root, proof_off, params_off) + proof_enc (96) + params_enc (160)
        assert_eq!(cd.len(), 4 + 3 * 32 + 96 + 160);
    }

    #[test]
    fn set_root_with_context_calldata_has_correct_selector() {
        let expected_sel = {
            let h = keccak256(
                b"setRootWithContext(bytes32,(uint256,bytes),(address,bytes32,bytes),address,bytes)",
            );
            [h[0], h[1], h[2], h[3]]
        };
        let proof = ProofStruct { location: ProofLocation::Private, data: vec![] };
        let cd =
            set_root_with_context_calldata(B256::ZERO, &proof, &dummy_params(), Address::ZERO, &[]);
        assert_eq!(&cd[..4], expected_sel);
        // 4 (sel) + 5*32 (head) + proof_enc (96) + params_enc (160) + factory_data_enc (32)
        assert_eq!(cd.len(), 4 + 5 * 32 + 96 + 160 + 32);
    }

    #[test]
    fn set_root_calldata_embeds_location() {
        let proof = ProofStruct { location: ProofLocation::Ipfs, data: vec![0x01, 0x02] };
        let cd = set_root_calldata(B256::ZERO, &proof, &dummy_params());
        // After selector (4) + root (32) + proof_offset (32) + params_offset (32) = 100 bytes
        // The proof tuple starts at offset proof_offset = 3*32 = 96 bytes after selector,
        // i.e. at byte position 4 + 96 = 100.
        // Slot 0 of proof tuple = ProofLocation::Ipfs as u64 = 5
        let loc_slot = &cd[100..132];
        assert_eq!(loc_slot[31], 5u8); // location = Ipfs = 5
    }

    // ── create_with_context_calldata ─────────────────────────────────────

    #[test]
    fn create_with_context_calldata_has_correct_selector() {
        let expected_sel = {
            let h = keccak256(b"createWithContext((address,bytes32,bytes),address,bytes,bool)");
            [h[0], h[1], h[2], h[3]]
        };
        let cd = create_with_context_calldata(&dummy_params(), Address::ZERO, &[], true);
        assert_eq!(&cd[..4], expected_sel);
    }

    #[test]
    fn create_with_context_calldata_dispatch_true() {
        let cd = create_with_context_calldata(&dummy_params(), Address::ZERO, &[], true);
        // dispatch is the 4th head slot (index 3), starting at byte 4 + 3*32 = 100
        assert_eq!(cd[4 + 3 * 32 + 31], 1u8);
    }

    #[test]
    fn create_with_context_calldata_dispatch_false() {
        let cd = create_with_context_calldata(&dummy_params(), Address::ZERO, &[], false);
        assert_eq!(cd[4 + 3 * 32 + 31], 0u8);
    }

    #[test]
    fn create_with_context_calldata_with_factory_data() {
        let factory_data = vec![0xffu8; 40];
        let cd = create_with_context_calldata(
            &dummy_params(),
            Address::repeat_byte(0x11),
            &factory_data,
            false,
        );
        // Factory data should appear in the calldata
        assert!(cd.windows(40).any(|w| w == &*factory_data));
    }

    #[test]
    fn create_with_context_calldata_length_aligned() {
        let cd = create_with_context_calldata(&dummy_params(), Address::ZERO, &[0xab; 5], true);
        // Total length after selector must be 32-byte aligned
        assert_eq!((cd.len() - 4) % 32, 0);
    }

    // ── remove_calldata ──────────────────────────────────────────────────

    #[test]
    fn remove_calldata_has_correct_selector() {
        let expected_sel = {
            let h = keccak256(b"remove(bytes32)");
            [h[0], h[1], h[2], h[3]]
        };
        let cd = remove_calldata(B256::ZERO);
        assert_eq!(&cd[..4], expected_sel);
    }

    #[test]
    fn remove_calldata_has_correct_length() {
        let cd = remove_calldata(B256::ZERO);
        assert_eq!(cd.len(), 36); // 4 selector + 32 order_id
    }

    #[test]
    fn remove_calldata_embeds_order_id() {
        let id = B256::new([0xabu8; 32]);
        let cd = remove_calldata(id);
        assert_eq!(&cd[4..36], id.as_slice());
    }

    // ── create_calldata ──────────────────────────────────────────────────

    #[test]
    fn create_calldata_has_correct_selector() {
        let expected_sel = {
            let h = keccak256(b"create((address,bytes32,bytes),bytes32[2][],bytes)");
            [h[0], h[1], h[2], h[3]]
        };
        let cd = create_calldata(&dummy_params(), &[], &[]);
        assert_eq!(&cd[..4], expected_sel);
    }

    #[test]
    fn create_calldata_empty_proof_and_offchain() {
        let cd = create_calldata(&dummy_params(), &[], &[]);
        // Length after selector must be 32-byte aligned
        assert_eq!((cd.len() - 4) % 32, 0);
    }

    #[test]
    fn create_calldata_with_proof_pairs() {
        let pair = [B256::new([0x11u8; 32]), B256::new([0x22u8; 32])];
        let cd = create_calldata(&dummy_params(), &[pair], &[]);
        // The proof pair data should appear somewhere in the buffer
        assert!(cd.windows(32).any(|w| w == pair[0].as_slice()));
        assert!(cd.windows(32).any(|w| w == pair[1].as_slice()));
    }

    #[test]
    fn create_calldata_with_offchain_input() {
        let offchain = vec![0xddu8; 17];
        let cd = create_calldata(&dummy_params(), &[], &offchain);
        assert!(cd.windows(17).any(|w| w == &*offchain));
        assert_eq!((cd.len() - 4) % 32, 0);
    }

    // ── ABI helpers ──────────────────────────────────────────────────────

    #[test]
    fn padded32_multiples() {
        assert_eq!(padded32(0), 0);
        assert_eq!(padded32(1), 32);
        assert_eq!(padded32(31), 32);
        assert_eq!(padded32(32), 32);
        assert_eq!(padded32(33), 64);
        assert_eq!(padded32(64), 64);
    }

    #[test]
    fn u256_be_encodes_correctly() {
        let w = u256_be(1);
        assert_eq!(w[31], 1);
        assert_eq!(w[..31], [0u8; 31]);

        let w = u256_be(256);
        assert_eq!(w[30], 1);
        assert_eq!(w[31], 0);
    }

    #[test]
    fn pad_address_correct() {
        let addr = Address::repeat_byte(0xff);
        let padded = pad_address(addr.as_slice());
        assert_eq!(&padded[..12], &[0u8; 12]);
        assert_eq!(&padded[12..], addr.as_slice());
    }

    #[test]
    fn pad_to_aligns_buffer() {
        let mut buf = vec![0u8; 10];
        pad_to(&mut buf, 10);
        assert_eq!(buf.len(), 32); // 10 + 22 padding

        let mut buf2 = vec![0u8; 32];
        pad_to(&mut buf2, 32);
        assert_eq!(buf2.len(), 32); // already aligned, no change
    }

    #[test]
    fn selector_is_deterministic() {
        let s1 = selector("remove(bytes32)");
        let s2 = selector("remove(bytes32)");
        assert_eq!(s1, s2);
    }

    // ── set_root_with_context with non-empty factory data ────────────────

    #[test]
    fn set_root_with_context_calldata_with_factory_data() {
        let proof = ProofStruct { location: ProofLocation::Emitted, data: vec![0xcc; 10] };
        let factory_data = vec![0xffu8; 20];
        let cd = set_root_with_context_calldata(
            B256::new([0x01u8; 32]),
            &proof,
            &dummy_params(),
            Address::repeat_byte(0x22),
            &factory_data,
        );
        assert_eq!((cd.len() - 4) % 32, 0);
        // Factory data should appear in the buffer
        assert!(cd.windows(20).any(|w| w == &*factory_data));
    }
}
