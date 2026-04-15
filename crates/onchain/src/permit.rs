//! On-chain `EIP-2612` read methods for [`OnchainReader`].

use alloy_primitives::{Address, U256};
use cow_erc20::{build_eip2612_nonces_calldata, build_eip2612_version_calldata};
use cow_errors::CowError;

use crate::{OnchainReader, decode_string, decode_u256};

/// Aggregate on-chain token information gathered in a single round of concurrent `eth_call`
/// requests.
///
/// Contains the owner's balance, allowance, permit nonce, token decimals, and
/// `EIP-2612` domain version. Returned by [`OnchainReader::read_token_permit_info`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnchainTokenInfo {
    /// Current token balance of the queried owner.
    pub balance: U256,
    /// Current allowance granted by owner to spender.
    pub allowance: U256,
    /// Current `EIP-2612` permit nonce for owner.
    pub nonce: U256,
    /// Token decimals.
    pub decimals: u8,
    /// `EIP-2612` domain version string (commonly `"1"`).
    pub version: String,
}

impl OnchainTokenInfo {
    /// Returns `true` if the owner holds a non-zero token balance.
    ///
    /// # Returns
    ///
    /// `true` when `balance > 0`, `false` otherwise.
    #[must_use]
    pub fn has_balance(&self) -> bool {
        !self.balance.is_zero()
    }

    /// Returns `true` if the current allowance is greater than or equal to `amount`.
    ///
    /// # Arguments
    ///
    /// * `amount` - The [`U256`] transfer amount to check against the stored allowance.
    ///
    /// # Returns
    ///
    /// `true` when `self.allowance >= amount`, `false` otherwise.
    #[must_use]
    pub fn allowance_covers(&self, amount: U256) -> bool {
        self.allowance >= amount
    }
}

impl OnchainReader {
    /// Read the current `EIP-2612` permit nonce for `owner` on the contract at `token`.
    ///
    /// The nonce must be included in `permit()` signatures to prevent replay attacks.
    /// Executes `nonces(address)` via `eth_call` against block `"latest"`.
    ///
    /// # Arguments
    ///
    /// * `token` - The [`Address`] of the `EIP-2612`-compatible token contract.
    /// * `owner` - The [`Address`] whose permit nonce to query.
    ///
    /// # Returns
    ///
    /// The current permit nonce as a [`U256`]. This value must be passed when
    /// constructing a `permit()` signature.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] on RPC failure or [`CowError::Parse`] if the
    /// response cannot be decoded as a `uint256`.
    pub async fn eip2612_nonce(&self, token: Address, owner: Address) -> Result<U256, CowError> {
        let cd = build_eip2612_nonces_calldata(owner);
        let ret = self.eth_call(token, &cd).await?;
        decode_u256(&ret)
    }

    /// Read the `EIP-2612` domain separator version string from the contract at `token`.
    ///
    /// The version is used when constructing `EIP-712` typed-data signatures for permits.
    /// Executes `version()` via `eth_call` against block `"latest"` and decodes the
    /// ABI-encoded dynamic `string`.
    ///
    /// # Arguments
    ///
    /// * `token` - The [`Address`] of the `EIP-2612`-compatible token contract.
    ///
    /// # Returns
    ///
    /// The domain version as a [`String`] (commonly `"1"`).
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] on RPC failure or [`CowError::Parse`] if the
    /// response cannot be decoded as an ABI `string`.
    pub async fn eip2612_version(&self, token: Address) -> Result<String, CowError> {
        let cd = build_eip2612_version_calldata();
        let ret = self.eth_call(token, &cd).await?;
        decode_string(&ret)
    }

    /// Fetch `balance`, `allowance`, `nonce`, `decimals`, and `version` in five
    /// concurrent `eth_call` requests.
    ///
    /// Uses [`futures::try_join!`] so all five calls are issued simultaneously; the
    /// future resolves to an error as soon as any single call fails.
    ///
    /// # Arguments
    ///
    /// * `token` - The [`Address`] of the `ERC-20` / `EIP-2612` token contract.
    /// * `owner` - The [`Address`] whose balance, allowance, and nonce to query.
    /// * `spender` - The [`Address`] to check the allowance against.
    ///
    /// # Returns
    ///
    /// An [`OnchainTokenInfo`] struct containing the owner's balance, the
    /// allowance granted to `spender`, the permit nonce, token decimals, and
    /// the `EIP-2612` domain version string.
    ///
    /// # Errors
    ///
    /// Returns the first [`CowError`] encountered across the five calls.
    pub async fn read_token_permit_info(
        &self,
        token: Address,
        owner: Address,
        spender: Address,
    ) -> Result<OnchainTokenInfo, CowError> {
        let (balance, allowance, nonce, decimals, version) = futures::try_join!(
            self.erc20_balance(token, owner),
            self.erc20_allowance(token, owner, spender),
            self.eip2612_nonce(token, owner),
            self.erc20_decimals(token),
            self.eip2612_version(token),
        )?;

        Ok(OnchainTokenInfo { balance, allowance, nonce, decimals, version })
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::address;

    use super::*;

    fn token() -> Address {
        address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
    }

    fn owner() -> Address {
        address!("1111111111111111111111111111111111111111")
    }

    #[test]
    fn nonces_calldata_len() {
        let cd = build_eip2612_nonces_calldata(owner());
        assert_eq!(cd.len(), 36);
    }

    #[test]
    fn version_calldata_len() {
        let cd = build_eip2612_version_calldata();
        assert_eq!(cd.len(), 4);
    }

    #[test]
    fn nonces_selector_correct() {
        let cd = build_eip2612_nonces_calldata(owner());
        let sel = &alloy_primitives::keccak256(b"nonces(address)")[..4];
        assert_eq!(&cd[..4], sel);
    }

    #[test]
    fn version_selector_correct() {
        let cd = build_eip2612_version_calldata();
        let sel = &alloy_primitives::keccak256(b"version()")[..4];
        assert_eq!(&*cd, sel);
    }

    #[test]
    fn nonces_encodes_owner_address() {
        let cd = build_eip2612_nonces_calldata(owner());
        // The last 20 bytes of the 32-byte address word should match owner bytes
        let owner_bytes = owner();
        assert_eq!(&cd[16..36], owner_bytes.as_slice());
    }

    #[test]
    fn onchain_token_info_has_balance() {
        let info = OnchainTokenInfo {
            balance: U256::from(100u64),
            allowance: U256::ZERO,
            nonce: U256::ZERO,
            decimals: 6,
            version: "1".into(),
        };
        assert!(info.has_balance());
    }

    #[test]
    fn onchain_token_info_no_balance() {
        let info = OnchainTokenInfo {
            balance: U256::ZERO,
            allowance: U256::ZERO,
            nonce: U256::ZERO,
            decimals: 6,
            version: "1".into(),
        };
        assert!(!info.has_balance());
    }

    #[test]
    fn onchain_token_info_allowance_covers() {
        let info = OnchainTokenInfo {
            balance: U256::ZERO,
            allowance: U256::from(1000u64),
            nonce: U256::ZERO,
            decimals: 6,
            version: "1".into(),
        };
        assert!(info.allowance_covers(U256::from(999u64)));
        assert!(info.allowance_covers(U256::from(1000u64)));
        assert!(!info.allowance_covers(U256::from(1001u64)));
    }

    #[test]
    fn token_address_non_zero() {
        assert_ne!(token(), Address::ZERO);
    }

    #[test]
    fn onchain_token_info_debug_and_clone() {
        let info = OnchainTokenInfo {
            balance: U256::from(100u64),
            allowance: U256::from(50u64),
            nonce: U256::from(3u64),
            decimals: 18,
            version: "2".into(),
        };
        let cloned = info.clone();
        assert_eq!(info, cloned);
        let debug = format!("{info:?}");
        assert!(debug.contains("balance"));
    }

    #[test]
    fn onchain_token_info_allowance_exact_zero() {
        let info = OnchainTokenInfo {
            balance: U256::ZERO,
            allowance: U256::ZERO,
            nonce: U256::ZERO,
            decimals: 6,
            version: "1".into(),
        };
        assert!(info.allowance_covers(U256::ZERO));
        assert!(!info.allowance_covers(U256::from(1u64)));
    }

    #[test]
    fn nonces_calldata_address_padded() {
        let cd = build_eip2612_nonces_calldata(Address::ZERO);
        // First 4 bytes are selector, next 12 bytes should be zero-padding
        assert!(cd[4..16].iter().all(|&b| b == 0));
    }
}
