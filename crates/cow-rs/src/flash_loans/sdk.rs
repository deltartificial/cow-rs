//! [`FlashLoanSdk`] — calldata builders and hook helpers for flash loans.
//!
//! Contains the [`FlashLoanSdk`] unit struct with static methods for
//! encoding flash loan calldata and building pre-interaction hooks, plus
//! Aave V3 adapter constants.

use alloy_primitives::{Address, U256, keccak256};

use crate::CowError;

use super::types::{FlashLoanParams, FlashLoanProvider};

// ── Aave flash loan constants ───────────────────────────────────────────────

/// Aave V3 pool address on Ethereum mainnet.
///
/// `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2`
pub const AAVE_POOL_ADDRESS_MAINNET: Address = Address::new([
    0x87, 0x87, 0x0b, 0xca, 0x3f, 0x3f, 0xd6, 0x33, 0x5c, 0x3f, 0x4c, 0xe8, 0x39, 0x2d, 0x69, 0x35,
    0x0b, 0x4f, 0xa4, 0xe2,
]);

/// Aave adapter factory address (same on all supported chains).
///
/// `0x43c658Ea38bBfD897706fDb35e2468ef5D8F6927`
pub const AAVE_ADAPTER_FACTORY: Address = Address::new([
    0x43, 0xc6, 0x58, 0xea, 0x38, 0xbb, 0xfd, 0x89, 0x77, 0x06, 0xfd, 0xb3, 0x5e, 0x24, 0x68, 0xef,
    0x5d, 0x8f, 0x69, 0x27,
]);

/// The 32-byte zero hash constant.
///
/// `0x0000000000000000000000000000000000000000000000000000000000000000`
pub const HASH_ZERO: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";

/// Scale factor for flash loan percentage calculations (Aave `PERCENTAGE_FACTOR` = 10 000).
pub const PERCENT_SCALE: u32 = 10_000;

/// Scale factor for basis-point conversion in flash loan fee calculation.
///
/// Equals `100 * PERCENT_SCALE` = 1 000 000. Matches Aave `PercentageMath.percentMul()`.
pub const BASIS_POINTS_SCALE: u64 = 1_000_000;

/// Half of [`BASIS_POINTS_SCALE`] (500 000), used for rounding in flash loan fee calculations.
pub const HALF_BASIS_POINTS_SCALE: u64 = BASIS_POINTS_SCALE / 2;

/// Default validity period for flash loan orders (10 minutes, in seconds).
pub const DEFAULT_VALIDITY: u32 = 10 * 60;

/// Extra percentage added to gas estimates for safety (10 %).
pub const GAS_ESTIMATION_ADDITION_PERCENT: u32 = 10;

/// EIP-712 domain name for the Aave V3 adapter factory.
pub const ADAPTER_DOMAIN_NAME: &str = "AaveV3AdapterFactory";

/// EIP-712 domain version for the Aave V3 adapter factory.
pub const ADAPTER_DOMAIN_VERSION: &str = "1";

/// Helper for building flash loan pre-interaction hooks.
///
/// `FlashLoanSdk` is a unit struct with static methods — no instances are
/// needed. It provides calldata encoding for supported providers and a
/// convenience method to build a complete
/// [`CowHook`](crate::app_data::CowHook) ready to attach to an order's
/// app-data.
///
/// Currently only **Balancer** flash loan encoding is implemented. MakerDAO
/// and Aave V3 encoding will return
/// [`CowError::Unsupported`](crate::CowError::Unsupported).
#[derive(Debug, Clone, Copy)]
pub struct FlashLoanSdk;

impl FlashLoanSdk {
    /// Return the flash loan provider contract address for `chain_id`, if
    /// supported.
    ///
    /// Delegates to [`FlashLoanProvider::contract_address`].
    ///
    /// # Parameters
    ///
    /// * `provider` — the flash loan protocol.
    /// * `chain_id` — the numeric EIP-155 chain ID.
    ///
    /// # Returns
    ///
    /// `Some(address)` if the provider is deployed on `chain_id`, `None`
    /// otherwise.
    #[must_use]
    pub const fn provider_address(provider: FlashLoanProvider, chain_id: u64) -> Option<Address> {
        provider.contract_address(chain_id)
    }

    /// Encode calldata for
    /// `Balancer.flashLoan(address,address[],uint256[],bytes)`.
    ///
    /// Produces ABI-encoded calldata for a single-token Balancer vault flash
    /// loan. The encoding follows standard Solidity ABI rules: a 128-byte
    /// static head (receiver + 3 dynamic offsets) followed by the tokens
    /// array, amounts array, and padded user data.
    ///
    /// # Parameters
    ///
    /// * `receiver` — the [`Address`] that receives the borrowed tokens and
    ///   must implement `IFlashLoanRecipient`.
    /// * `token` — the ERC-20 [`Address`] of the token to borrow.
    /// * `amount` — the borrow amount as a [`U256`] in token atoms.
    /// * `user_data` — arbitrary bytes forwarded to the receiver's callback.
    ///   Pass `&[]` when no extra data is needed.
    ///
    /// # Returns
    ///
    /// A `Vec<u8>` containing the 4-byte selector + ABI-encoded arguments.
    ///
    /// # Panics
    ///
    /// Never panics — all arithmetic is infallible for a single-token call.
    ///
    /// # Example
    ///
    /// ```
    /// use alloy_primitives::{Address, U256};
    /// use cow_rs::flash_loans::FlashLoanSdk;
    ///
    /// let calldata = FlashLoanSdk::encode_balancer_flash_loan(
    ///     Address::ZERO,
    ///     Address::ZERO,
    ///     U256::from(1_000_000u64),
    ///     &[],
    /// );
    /// // 4 (selector) + 128 (head) + 64 (tokens) + 64 (amounts) + 32 (data len) = 292
    /// assert_eq!(calldata.len(), 292);
    /// ```
    #[must_use]
    pub fn encode_balancer_flash_loan(
        receiver: Address,
        token: Address,
        amount: U256,
        user_data: &[u8],
    ) -> Vec<u8> {
        let sig = b"flashLoan(address,address[],uint256[],bytes)";
        let h = keccak256(sig);
        let sel = [h[0], h[1], h[2], h[3]];

        // Head (4 params × 32 bytes each = 128 bytes):
        //   param 0: receiver (address, static)
        //   param 1: tokens array offset
        //   param 2: amounts array offset
        //   param 3: user_data offset
        //
        // Dynamic sections (in order, after the 128-byte head):
        //   tokens   : length(1) + 1 × address padded
        //   amounts  : length(1) + 1 × uint256
        //   user_data: length    + data padded to 32 bytes

        let data_padded_len = user_data.len().div_ceil(32) * 32;

        // Offsets are relative to the start of the dynamic section (i.e. byte 128 from the
        // selector-stripped head, or byte 132 from the very start of calldata).
        // ABI spec: offsets in the head are relative to the START of the encoding (byte 0 of the
        // head, which is byte 4 in calldata).
        let head_size: u64 = 128; // 4 × 32
        let tokens_offset: u64 = head_size; // immediately after head
        let amounts_offset: u64 = tokens_offset + 64; // tokens: 32(len) + 32(addr) = 64
        let data_offset: u64 = amounts_offset + 64; // amounts: 32(len) + 32(amount) = 64

        /// Encode a `u64` as a 32-byte big-endian ABI word.
        fn u64_word(v: u64) -> [u8; 32] {
            U256::from(v).to_be_bytes()
        }

        /// Left-pad an [`Address`] to a 32-byte ABI word.
        fn abi_addr(a: Address) -> [u8; 32] {
            let mut buf = [0u8; 32];
            buf[12..].copy_from_slice(a.as_slice());
            buf
        }

        let capacity = 4 + 128 + 64 + 64 + 32 + data_padded_len;
        let mut buf = Vec::with_capacity(capacity);

        // Selector
        buf.extend_from_slice(&sel);
        // Head
        buf.extend_from_slice(&abi_addr(receiver));
        buf.extend_from_slice(&u64_word(tokens_offset));
        buf.extend_from_slice(&u64_word(amounts_offset));
        buf.extend_from_slice(&u64_word(data_offset));
        // tokens array: length=1, then token
        buf.extend_from_slice(&u64_word(1));
        buf.extend_from_slice(&abi_addr(token));
        // amounts array: length=1, then amount
        buf.extend_from_slice(&u64_word(1));
        buf.extend_from_slice(&amount.to_be_bytes::<32>());
        // user_data: length, then padded bytes
        buf.extend_from_slice(&u64_word(user_data.len() as u64));
        buf.extend_from_slice(user_data);
        // Pad to 32-byte boundary
        let pad = data_padded_len - user_data.len();
        buf.extend(std::iter::repeat_n(0u8, pad));

        buf
    }

    /// Build a [`CowHook`](crate::app_data::CowHook) that triggers a flash
    /// loan pre-interaction.
    ///
    /// Looks up the provider's contract address on the specified chain,
    /// encodes the flash loan calldata, and wraps everything in a
    /// [`CowHook`](crate::app_data::CowHook) with a default gas limit of
    /// `500_000`.
    ///
    /// # Parameters
    ///
    /// * `params` — the [`FlashLoanParams`] (provider, token, amount, chain).
    /// * `receiver` — the [`Address`] that receives the borrowed tokens and
    ///   implements the flash loan callback.
    /// * `user_data` — arbitrary bytes forwarded to the receiver's callback.
    ///
    /// # Returns
    ///
    /// A [`CowHook`](crate::app_data::CowHook) ready to be attached to an
    /// order's [`OrderInteractionHooks::pre`](crate::app_data::OrderInteractionHooks).
    ///
    /// # Errors
    ///
    /// - [`CowError::Unsupported`] if the provider is not deployed on
    ///   `params.chain_id`.
    /// - [`CowError::Unsupported`] if the provider's calldata encoding is
    ///   not yet implemented (MakerDAO, Aave V3).
    pub fn build_flash_loan_hook(
        params: &FlashLoanParams,
        receiver: Address,
        user_data: &[u8],
    ) -> Result<crate::app_data::CowHook, CowError> {
        let contract = params.provider.contract_address(params.chain_id).ok_or_else(|| {
            CowError::Unsupported {
                message: format!(
                    "{} flash loans not supported on chain {}",
                    params.provider.name(),
                    params.chain_id
                ),
            }
        })?;

        let calldata = match params.provider {
            FlashLoanProvider::Balancer => {
                Self::encode_balancer_flash_loan(receiver, params.token, params.amount, user_data)
            }
            FlashLoanProvider::MakerDao | FlashLoanProvider::AaveV3 => {
                return Err(CowError::Unsupported {
                    message: format!(
                        "{} flash loan encoding not yet implemented",
                        params.provider.name()
                    ),
                });
            }
        };

        Ok(crate::app_data::CowHook {
            target: format!("{contract:#x}"),
            call_data: alloy_primitives::hex::encode(&calldata),
            gas_limit: "500000".to_owned(),
            dapp_id: None,
        })
    }
}
