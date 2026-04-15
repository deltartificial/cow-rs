//! Weiroll script types, command encoding, and contract reference factories.
//!
//! This module defines the low-level types for Weiroll scripts:
//!
//! | Type | Purpose |
//! |---|---|
//! | [`WeirollCommand`] | A single 32-byte packed instruction |
//! | [`WeirollScript`] | Finalised script (commands + state slots) |
//! | [`WeirollCommandFlags`] | Call-type flags (`CALL`, `DELEGATECALL`, `STATICCALL`) |
//! | [`WeirollContractRef`] | Contract address + ABI + default call flags |
//!
//! Factory functions:
//!
//! | Function | Creates |
//! |---|---|
//! | [`create_weiroll_contract`] | `CALL`-mode contract ref |
//! | [`create_weiroll_library`] | `DELEGATECALL`-mode library ref |
//! | [`create_weiroll_delegate_call`] | Full `execute(...)` [`EvmCall`] from a planner callback |

use alloy_primitives::{Address, U256, address};

use cow_chains::chains::EvmCall;

/// Canonical Weiroll VM contract address.
pub const WEIROLL_CONTRACT_ADDRESS: Address = address!("9585c3062Df1C247d5E373Cfca9167F7dC2b5963");

/// A single command in a Weiroll script (32-byte packed encoding).
///
/// Each command encodes a contract call instruction that the Weiroll VM
/// executes sequentially. The 32-byte packed layout is:
///
/// ```text
/// ┌──────────┬───────┬────────┬────────────────┬──────────┬─────────┐
/// │ flags(1) │ val(1)│ gas(2) │  target (20)   │ sel (4)  │ i/o (4) │
/// └──────────┴───────┴────────┴────────────────┴──────────┴─────────┘
/// ```
///
/// Use [`pack`](Self::pack) to serialise into the 32-byte wire format.
#[derive(Debug, Clone)]
pub struct WeirollCommand {
    /// Command flags byte — see [`WeirollCommandFlags`] for the call-type
    /// bits and modifier constants.
    pub flags: u8,
    /// Value byte — used with [`WeirollCommandFlags::CallWithValue`] to
    /// index the state slot containing the ETH value to send.
    pub value: u8,
    /// Gas limit for this call (big-endian, 2 bytes). `0` means unlimited.
    pub gas: u16,
    /// Target contract address (20 bytes).
    pub target: Address,
    /// Solidity function selector (first 4 bytes of `keccak256(signature)`).
    pub selector: [u8; 4],
    /// Input/output slot mapping (4 bytes) — each nibble or byte indexes a
    /// state slot that provides an argument or receives the return value.
    pub in_out: [u8; 4],
}

impl WeirollCommand {
    /// Pack this command into a 32-byte word for on-chain execution.
    ///
    /// Layout: `[flags(1)] [value(1)] [gas(2)] [target(20)] [selector(4)] [inout(4)]`
    ///
    /// # Returns
    ///
    /// A `[u8; 32]` containing the packed command.
    ///
    /// # Example
    ///
    /// ```
    /// use alloy_primitives::Address;
    /// use cow_weiroll::WeirollCommand;
    ///
    /// let cmd = WeirollCommand {
    ///     flags: 0x01,
    ///     value: 0x00,
    ///     gas: 21_000,
    ///     target: Address::ZERO,
    ///     selector: [0xde, 0xad, 0xbe, 0xef],
    ///     in_out: [0x00; 4],
    /// };
    /// let packed = cmd.pack();
    /// assert_eq!(packed.len(), 32);
    /// assert_eq!(packed[0], 0x01); // flags
    /// ```
    #[must_use]
    pub fn pack(&self) -> [u8; 32] {
        let mut word = [0u8; 32];
        word[0] = self.flags;
        word[1] = self.value;
        word[2..4].copy_from_slice(&self.gas.to_be_bytes());
        word[4..24].copy_from_slice(self.target.as_slice());
        word[24..28].copy_from_slice(&self.selector);
        word[28..32].copy_from_slice(&self.in_out);
        word
    }

    /// Return the flags byte.
    ///
    /// # Returns
    ///
    /// The raw `u8` flags value for this command.
    #[must_use]
    pub const fn flags_ref(&self) -> u8 {
        self.flags
    }

    /// Return a reference to the target address.
    ///
    /// # Returns
    ///
    /// A reference to the target contract [`Address`].
    #[must_use]
    pub const fn target_ref(&self) -> &Address {
        &self.target
    }

    /// Return a reference to the function selector.
    ///
    /// # Returns
    ///
    /// A reference to the 4-byte function selector.
    #[must_use]
    pub const fn selector_ref(&self) -> &[u8; 4] {
        &self.selector
    }
}

/// A complete Weiroll script ready for on-chain execution.
///
/// Produced by [`WeirollPlanner::plan`](super::WeirollPlanner::plan). The
/// `commands` and `state` fields map directly to the two arguments of the
/// Weiroll executor's `execute(bytes32[],bytes[])` function.
///
/// # Example
///
/// ```
/// use cow_weiroll::WeirollPlanner;
///
/// let planner = WeirollPlanner::new();
/// let script = planner.plan();
/// assert!(script.is_empty());
/// assert_eq!(script.command_count(), 0);
/// assert_eq!(script.state_slot_count(), 0);
/// ```
#[derive(Debug, Clone)]
pub struct WeirollScript {
    /// Packed 32-byte command words (one per instruction).
    pub commands: Vec<[u8; 32]>,
    /// ABI-encoded state slots (arguments and return-value buffers).
    pub state: Vec<Vec<u8>>,
}

impl WeirollScript {
    /// Number of commands in this script.
    ///
    /// # Returns
    ///
    /// The length of the [`commands`](Self::commands) vector.
    #[must_use]
    pub const fn command_count(&self) -> usize {
        self.commands.len()
    }

    /// Number of state slots in this script.
    ///
    /// # Returns
    ///
    /// The length of the [`state`](Self::state) vector.
    #[must_use]
    pub const fn state_slot_count(&self) -> usize {
        self.state.len()
    }

    /// Returns `true` if the script contains no commands.
    ///
    /// An empty script is a no-op when executed on-chain.
    ///
    /// # Returns
    ///
    /// `true` when the [`commands`](Self::commands) vector is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

/// Flags that modify the execution mode of a Weiroll command.
///
/// These correspond to the EVM opcodes used when the Weiroll executor
/// invokes each command's target contract.  The call-type variants live in
/// the lower 2 bits; combine them with the associated `CALLTYPE_MASK`,
/// `EXTENDED_COMMAND`, and `TUPLE_RETURN` constants using bitwise
/// operations.
///
/// # Example
///
/// ```
/// use cow_weiroll::WeirollCommandFlags;
///
/// let flags = WeirollCommandFlags::Call;
/// assert_eq!(flags as u8, 0x01);
/// assert_eq!(flags as u8 & WeirollCommandFlags::CALLTYPE_MASK, WeirollCommandFlags::Call as u8,);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WeirollCommandFlags {
    /// Execute via `DELEGATECALL` opcode (library calls).
    DelegateCall = 0x00,
    /// Execute via `CALL` opcode (standard external calls).
    Call = 0x01,
    /// Execute via `STATICCALL` opcode (read-only calls).
    StaticCall = 0x02,
    /// Execute via `CALL` with an explicit value transfer; the first
    /// argument is interpreted as the ETH value to send.
    CallWithValue = 0x03,
}

impl WeirollCommandFlags {
    /// Bitmask that isolates the call-type bits from other flag bits.
    pub const CALLTYPE_MASK: u8 = 0x03;
    /// Marks an extended command that uses an additional 32-byte word
    /// for argument slot indices (internal use).
    pub const EXTENDED_COMMAND: u8 = 0x40;
    /// Signals that the return value should be ABI-wrapped as `bytes`
    /// so that multi-return functions can be captured (internal use).
    pub const TUPLE_RETURN: u8 = 0x80;
}

/// The default Weiroll executor contract address deployed across supported
/// chains.
pub const WEIROLL_ADDRESS: &str = "0x9585c3062Df1C247d5E373Cfca9167F7dC2b5963";

/// A Weiroll-compatible contract reference with a default call mode.
///
/// Pairs a contract address and its ABI with the [`WeirollCommandFlags`]
/// that should be used when the Weiroll executor invokes this contract.
/// Create via [`create_weiroll_contract`] (for `CALL`) or
/// [`create_weiroll_library`] (for `DELEGATECALL`).
///
/// Mirrors `WeirollContract` from the `TypeScript` SDK.
///
/// # Example
///
/// ```
/// use alloy_primitives::Address;
/// use cow_weiroll::{WeirollCommandFlags, WeirollContractRef};
///
/// let contract = WeirollContractRef {
///     address: Address::ZERO,
///     abi: vec![],
///     command_flags: WeirollCommandFlags::Call,
/// };
/// assert_eq!(contract.command_flags, WeirollCommandFlags::Call);
/// ```
#[derive(Debug, Clone)]
pub struct WeirollContractRef {
    /// The on-chain address of the contract.
    pub address: Address,
    /// The JSON-ABI of the contract (raw bytes or string representation).
    pub abi: Vec<u8>,
    /// The default call flags to apply when executing this contract's
    /// functions through Weiroll.
    pub command_flags: WeirollCommandFlags,
}

/// Create a [`WeirollContractRef`] for a standard `CALL` contract.
///
/// All function invocations through the returned reference will default to
/// [`WeirollCommandFlags::Call`] unless overridden via the optional
/// `command_flags` argument.
///
/// Mirrors `createWeirollContract` from the `TypeScript` SDK.
///
/// # Parameters
///
/// * `address` — the on-chain contract [`Address`].
/// * `abi` — the contract's JSON-ABI as raw bytes (pass `vec![]` if not needed).
/// * `command_flags` — optional override for the default call mode. When `None`, defaults to
///   [`WeirollCommandFlags::Call`].
///
/// # Returns
///
/// A [`WeirollContractRef`] with the specified (or default) call flags.
///
/// # Example
///
/// ```
/// use alloy_primitives::Address;
/// use cow_weiroll::{WeirollCommandFlags, create_weiroll_contract};
///
/// let contract = create_weiroll_contract(Address::ZERO, vec![], None);
/// assert_eq!(contract.command_flags, WeirollCommandFlags::Call);
///
/// let static_contract =
///     create_weiroll_contract(Address::ZERO, vec![], Some(WeirollCommandFlags::StaticCall));
/// assert_eq!(static_contract.command_flags, WeirollCommandFlags::StaticCall);
/// ```
#[must_use]
pub fn create_weiroll_contract(
    address: Address,
    abi: Vec<u8>,
    command_flags: Option<WeirollCommandFlags>,
) -> WeirollContractRef {
    WeirollContractRef {
        address,
        abi,
        command_flags: command_flags.map_or(WeirollCommandFlags::Call, |v| v),
    }
}

/// Create a [`WeirollContractRef`] for a Weiroll library
/// (`DELEGATECALL`).
///
/// Library contracts are executed in the context of the Weiroll executor,
/// so their storage writes affect the executor's state. This is the
/// expected mode for helper libraries specifically written for Weiroll.
///
/// Mirrors `createWeirollLibrary` from the `TypeScript` SDK.
///
/// # Parameters
///
/// * `address` — the on-chain library [`Address`].
/// * `abi` — the library's JSON-ABI as raw bytes.
///
/// # Returns
///
/// A [`WeirollContractRef`] with
/// [`WeirollCommandFlags::DelegateCall`].
///
/// # Example
///
/// ```
/// use alloy_primitives::Address;
/// use cow_weiroll::{WeirollCommandFlags, create_weiroll_library};
///
/// let library = create_weiroll_library(Address::ZERO, vec![]);
/// assert_eq!(library.command_flags, WeirollCommandFlags::DelegateCall);
/// ```
#[must_use]
pub const fn create_weiroll_library(address: Address, abi: Vec<u8>) -> WeirollContractRef {
    WeirollContractRef { address, abi, command_flags: WeirollCommandFlags::DelegateCall }
}

/// Build a Weiroll delegate-call [`EvmCall`] by running a planner
/// callback.
///
/// Creates a fresh [`WeirollPlanner`](super::WeirollPlanner), passes it to
/// the caller-supplied closure so commands and state slots can be added,
/// then finalises the plan and ABI-encodes the resulting
/// `execute(bytes32[],bytes[])` calldata targeting the canonical
/// [`WEIROLL_CONTRACT_ADDRESS`].
///
/// Mirrors `createWeirollDelegateCall` from the `TypeScript` SDK.
///
/// # Parameters
///
/// * `add_to_planner` — a closure that receives `&mut WeirollPlanner` and populates it with
///   commands and state slots.
///
/// # Returns
///
/// An [`EvmCall`] with `to` set to the Weiroll executor, `data` set to
/// the ABI-encoded `execute(...)` calldata, and `value` set to zero.
///
/// # Example
///
/// ```
/// use alloy_primitives::Address;
/// use cow_weiroll::{WEIROLL_ADDRESS, WeirollCommand, create_weiroll_delegate_call};
///
/// let evm_call = create_weiroll_delegate_call(|planner| {
///     planner.add_command(WeirollCommand {
///         flags: 0,
///         value: 0,
///         gas: 0,
///         target: Address::ZERO,
///         selector: [0; 4],
///         in_out: [0; 4],
///     });
/// });
/// assert_eq!(evm_call.to, WEIROLL_ADDRESS.parse::<Address>().unwrap());
/// assert_eq!(evm_call.value, alloy_primitives::U256::ZERO);
/// ```
///
/// [`WEIROLL_ADDRESS`]: cow_weiroll::WEIROLL_ADDRESS
#[must_use]
pub fn create_weiroll_delegate_call(
    add_to_planner: impl FnOnce(&mut super::WeirollPlanner),
) -> EvmCall {
    let mut planner = super::WeirollPlanner::new();
    add_to_planner(&mut planner);
    let script = planner.plan();

    let calldata = abi_encode_execute(&script.commands, &script.state);

    EvmCall { to: WEIROLL_CONTRACT_ADDRESS, data: calldata, value: U256::ZERO }
}

/// ABI-encode an `execute(bytes32[], bytes[])` call.
///
/// Performs manual Solidity ABI encoding without requiring `alloy-sol-types`.
fn abi_encode_execute(commands: &[[u8; 32]], state: &[Vec<u8>]) -> Vec<u8> {
    // Function selector: keccak256("execute(bytes32[],bytes[])") = 0xde792be1
    let selector: [u8; 4] = [0xde, 0x79, 0x2b, 0xe1];

    // Head: selector + offset(commands) + offset(state)
    // commands array starts at offset 64 (2 * 32)
    // state array starts after commands: 64 + 32 + commands.len() * 32
    let commands_offset: usize = 64;
    let commands_data_len = 32 + commands.len() * 32; // length + elements
    let state_offset: usize = commands_offset + commands_data_len;

    let mut buf = Vec::with_capacity(4 + state_offset + 256);
    buf.extend_from_slice(&selector);

    // Offset to commands array
    buf.extend_from_slice(&pad_u256(commands_offset));
    // Offset to state array
    buf.extend_from_slice(&pad_u256(state_offset));

    // Commands array: length + elements
    buf.extend_from_slice(&pad_u256(commands.len()));
    for cmd in commands {
        buf.extend_from_slice(cmd);
    }

    // State array (dynamic): length + offsets + data
    buf.extend_from_slice(&pad_u256(state.len()));

    // Calculate offsets for each bytes element (relative to start of data area)
    let data_area_start = state.len() * 32;
    let mut current_offset = data_area_start;
    for slot in state {
        buf.extend_from_slice(&pad_u256(current_offset));
        // Each element: 32 bytes length + ceil(len/32)*32 bytes data
        current_offset += 32 + slot.len().div_ceil(32) * 32;
    }

    // Encode each bytes element
    for slot in state {
        buf.extend_from_slice(&pad_u256(slot.len()));
        buf.extend_from_slice(slot);
        // Pad to 32-byte boundary
        let padding = (32 - (slot.len() % 32)) % 32;
        buf.extend(std::iter::repeat_n(0u8, padding));
    }

    buf
}

/// Left-pad a `usize` into a 32-byte big-endian word.
fn pad_u256(value: usize) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[24..32].copy_from_slice(&(value as u64).to_be_bytes());
    word
}

/// Apply a mutation to a value and return it — a porting convenience for
/// builder-style field assignments.
///
/// In the `TypeScript` SDK, `defineReadOnly` uses `Object.defineProperty`
/// to freeze a property as non-writable. Rust achieves immutability by
/// default through its ownership system. This function exists as a porting
/// convenience — it takes ownership of `object`, applies `setter`, and
/// returns the modified value.
///
/// # Parameters
///
/// * `object` — the value to mutate (consumed by move).
/// * `setter` — a closure that receives `&mut T` and applies the desired field assignment.
///
/// # Returns
///
/// The modified `object`.
///
/// # Example
///
/// ```
/// use cow_weiroll::define_read_only;
///
/// #[derive(Debug, PartialEq)]
/// struct Config {
///     name: String,
///     value: u32,
/// }
///
/// let cfg = Config { name: String::new(), value: 0 };
/// let cfg = define_read_only(cfg, |c| c.name = "example".into());
/// let cfg = define_read_only(cfg, |c| c.value = 42);
/// assert_eq!(cfg.name, "example");
/// assert_eq!(cfg.value, 42);
/// ```
#[must_use]
pub fn define_read_only<T>(mut object: T, setter: impl FnOnce(&mut T)) -> T {
    setter(&mut object);
    object
}

/// Look up a value by key in a static registry of `(key, value)` pairs.
///
/// In the `TypeScript` SDK, `getStatic` walks up the prototype chain
/// (up to 32 levels) looking for a property on the constructor. In Rust
/// there is no prototype chain; this function linearly searches a slice
/// and returns a clone of the first matching value.
///
/// # Parameters
///
/// * `entries` — a slice of `(&str, T)` pairs to search.
/// * `key` — the key to look up.
///
/// # Returns
///
/// `Some(value.clone())` if found, `None` otherwise.
///
/// # Example
///
/// ```
/// use cow_weiroll::get_static;
///
/// let registry: &[(&str, i32)] = &[("version", 1), ("max_depth", 32)];
///
/// assert_eq!(get_static(registry, "version"), Some(1));
/// assert_eq!(get_static(registry, "missing"), None);
/// ```
#[must_use]
pub fn get_static<T: Clone>(entries: &[(&str, T)], key: &str) -> Option<T> {
    entries.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── WeirollCommand::pack ─────────────────────────────────────────────────

    #[test]
    fn pack_encodes_all_fields() {
        let cmd = WeirollCommand {
            flags: 0x01,
            value: 0xFF,
            gas: 21_000,
            target: Address::ZERO,
            selector: [0xde, 0xad, 0xbe, 0xef],
            in_out: [0x01, 0x02, 0x03, 0x04],
        };
        let packed = cmd.pack();
        assert_eq!(packed.len(), 32);
        assert_eq!(packed[0], 0x01);
        assert_eq!(packed[1], 0xFF);
        assert_eq!(&packed[2..4], &21_000u16.to_be_bytes());
        assert_eq!(&packed[4..24], Address::ZERO.as_slice());
        assert_eq!(&packed[24..28], &[0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(&packed[28..32], &[0x01, 0x02, 0x03, 0x04]);
    }

    // ── WeirollCommand accessors ─────────────────────────────────────────────

    #[test]
    fn flags_ref_returns_flags() {
        let cmd = WeirollCommand {
            flags: 0x42,
            value: 0,
            gas: 0,
            target: Address::ZERO,
            selector: [0; 4],
            in_out: [0; 4],
        };
        assert_eq!(cmd.flags_ref(), 0x42);
    }

    #[test]
    fn target_ref_returns_address() {
        let addr: Address = "0x1111111111111111111111111111111111111111".parse().unwrap();
        let cmd = WeirollCommand {
            flags: 0,
            value: 0,
            gas: 0,
            target: addr,
            selector: [0; 4],
            in_out: [0; 4],
        };
        assert_eq!(*cmd.target_ref(), addr);
    }

    #[test]
    fn selector_ref_returns_selector() {
        let cmd = WeirollCommand {
            flags: 0,
            value: 0,
            gas: 0,
            target: Address::ZERO,
            selector: [0xaa, 0xbb, 0xcc, 0xdd],
            in_out: [0; 4],
        };
        assert_eq!(*cmd.selector_ref(), [0xaa, 0xbb, 0xcc, 0xdd]);
    }

    // ── WeirollScript ────────────────────────────────────────────────────────

    #[test]
    fn empty_script() {
        let script = WeirollScript { commands: vec![], state: vec![] };
        assert!(script.is_empty());
        assert_eq!(script.command_count(), 0);
        assert_eq!(script.state_slot_count(), 0);
    }

    #[test]
    fn non_empty_script() {
        let script =
            WeirollScript { commands: vec![[0u8; 32], [1u8; 32]], state: vec![vec![0xab]] };
        assert!(!script.is_empty());
        assert_eq!(script.command_count(), 2);
        assert_eq!(script.state_slot_count(), 1);
    }

    // ── WeirollCommandFlags constants ────────────────────────────────────────

    #[test]
    fn command_flags_values() {
        assert_eq!(WeirollCommandFlags::DelegateCall as u8, 0x00);
        assert_eq!(WeirollCommandFlags::Call as u8, 0x01);
        assert_eq!(WeirollCommandFlags::StaticCall as u8, 0x02);
        assert_eq!(WeirollCommandFlags::CallWithValue as u8, 0x03);
    }

    #[test]
    fn calltype_mask_isolates_call_type() {
        assert_eq!(
            WeirollCommandFlags::Call as u8 & WeirollCommandFlags::CALLTYPE_MASK,
            WeirollCommandFlags::Call as u8
        );
    }

    #[test]
    fn extended_command_and_tuple_return_bits() {
        assert_eq!(WeirollCommandFlags::EXTENDED_COMMAND, 0x40);
        assert_eq!(WeirollCommandFlags::TUPLE_RETURN, 0x80);
    }

    // ── create_weiroll_contract ──────────────────────────────────────────────

    #[test]
    fn create_contract_default_flags() {
        let c = create_weiroll_contract(Address::ZERO, vec![], None);
        assert_eq!(c.command_flags, WeirollCommandFlags::Call);
    }

    #[test]
    fn create_contract_custom_flags() {
        let c =
            create_weiroll_contract(Address::ZERO, vec![], Some(WeirollCommandFlags::StaticCall));
        assert_eq!(c.command_flags, WeirollCommandFlags::StaticCall);
    }

    // ── create_weiroll_library ───────────────────────────────────────────────

    #[test]
    fn create_library_uses_delegatecall() {
        let lib = create_weiroll_library(Address::ZERO, vec![]);
        assert_eq!(lib.command_flags, WeirollCommandFlags::DelegateCall);
    }

    // ── create_weiroll_delegate_call ─────────────────────────────────────────

    #[test]
    fn delegate_call_produces_valid_evm_call() {
        let evm_call = create_weiroll_delegate_call(|planner| {
            planner.add_command(WeirollCommand {
                flags: 0,
                value: 0,
                gas: 0,
                target: Address::ZERO,
                selector: [0; 4],
                in_out: [0; 4],
            });
        });
        assert_eq!(evm_call.to, WEIROLL_CONTRACT_ADDRESS);
        assert_eq!(evm_call.value, U256::ZERO);
        // Selector is 0xde792be1
        assert_eq!(&evm_call.data[..4], &[0xde, 0x79, 0x2b, 0xe1]);
    }

    #[test]
    fn delegate_call_empty_planner() {
        let evm_call = create_weiroll_delegate_call(|_| {});
        assert_eq!(evm_call.to, WEIROLL_CONTRACT_ADDRESS);
        assert!(!evm_call.data.is_empty());
    }

    // ── define_read_only ─────────────────────────────────────────────────────

    #[test]
    fn define_read_only_applies_mutation() {
        let val = define_read_only(42u32, |v| *v = 100);
        assert_eq!(val, 100);
    }

    #[test]
    fn define_read_only_with_struct() {
        #[derive(Default)]
        struct S {
            x: i32,
        }
        let s = define_read_only(S::default(), |s| s.x = 7);
        assert_eq!(s.x, 7);
    }

    // ── get_static ───────────────────────────────────────────────────────────

    #[test]
    fn get_static_found() {
        let entries: &[(&str, i32)] = &[("a", 1), ("b", 2)];
        assert_eq!(get_static(entries, "b"), Some(2));
    }

    #[test]
    fn get_static_not_found() {
        let entries: &[(&str, i32)] = &[("a", 1)];
        assert_eq!(get_static(entries, "z"), None);
    }

    #[test]
    fn get_static_empty_entries() {
        let entries: &[(&str, i32)] = &[];
        assert_eq!(get_static(entries, "a"), None);
    }

    // ── WEIROLL_ADDRESS constant ─────────────────────────────────────────────

    #[test]
    fn weiroll_address_matches_constant() {
        let parsed: Address = WEIROLL_ADDRESS.parse().unwrap();
        assert_eq!(parsed, WEIROLL_CONTRACT_ADDRESS);
    }
}
