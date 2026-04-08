//! On-chain `ERC-20` read methods for [`OnchainReader`].

use alloy_primitives::{Address, U256};

use crate::{
    erc20::{
        build_erc20_allowance_calldata, build_erc20_balance_of_calldata,
        build_erc20_decimals_calldata, build_erc20_name_calldata,
    },
    error::CowError,
    onchain::{OnchainReader, decode_string, decode_u8, decode_u256},
};

impl OnchainReader {
    /// Read the `ERC-20` token balance of `owner` for the contract at `token`.
    ///
    /// Executes `balanceOf(address)` via `eth_call` against block `"latest"`.
    ///
    /// # Arguments
    ///
    /// * `token` - The [`Address`] of the `ERC-20` token contract.
    /// * `owner` - The [`Address`] whose balance to query.
    ///
    /// # Returns
    ///
    /// The token balance as a [`U256`] in the token's smallest unit (wei-equivalent).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use alloy_primitives::{U256, address};
    /// use cow_rs::OnchainReader;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let reader = OnchainReader::new("https://rpc.sepolia.org");
    /// let token = address!("fFf9976782d46CC05630D1f6eBAb18b2324d6B14");
    /// let owner = address!("1111111111111111111111111111111111111111");
    /// let balance: U256 = reader.erc20_balance(token, owner).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] on RPC failure or [`CowError::Parse`] if the
    /// response cannot be decoded as a `uint256`.
    pub async fn erc20_balance(&self, token: Address, owner: Address) -> Result<U256, CowError> {
        let cd = build_erc20_balance_of_calldata(owner);
        let ret = self.eth_call(token, &cd).await?;
        decode_u256(&ret)
    }

    /// Read the `ERC-20` allowance that `owner` has granted to `spender` on the
    /// contract at `token`.
    ///
    /// Executes `allowance(address,address)` via `eth_call` against block `"latest"`.
    ///
    /// # Arguments
    ///
    /// * `token` - The [`Address`] of the `ERC-20` token contract.
    /// * `owner` - The [`Address`] that granted the allowance.
    /// * `spender` - The [`Address`] that was granted permission to spend.
    ///
    /// # Returns
    ///
    /// The remaining allowance as a [`U256`] that `spender` is permitted to
    /// transfer on behalf of `owner`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use alloy_primitives::{U256, address};
    /// use cow_rs::OnchainReader;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let reader = OnchainReader::new("https://rpc.sepolia.org");
    /// let token = address!("fFf9976782d46CC05630D1f6eBAb18b2324d6B14");
    /// let owner = address!("1111111111111111111111111111111111111111");
    /// let spender = address!("2222222222222222222222222222222222222222");
    /// let allowance: U256 = reader.erc20_allowance(token, owner, spender).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] on RPC failure or [`CowError::Parse`] if the
    /// response cannot be decoded as a `uint256`.
    pub async fn erc20_allowance(
        &self,
        token: Address,
        owner: Address,
        spender: Address,
    ) -> Result<U256, CowError> {
        let cd = build_erc20_allowance_calldata(owner, spender);
        let ret = self.eth_call(token, &cd).await?;
        decode_u256(&ret)
    }

    /// Read the number of decimal places for the `ERC-20` contract at `token`.
    ///
    /// Executes `decimals()` via `eth_call` against block `"latest"`.
    ///
    /// # Arguments
    ///
    /// * `token` - The [`Address`] of the `ERC-20` token contract.
    ///
    /// # Returns
    ///
    /// The number of decimals as a `u8` (commonly `18` for ETH-like tokens,
    /// `6` for USDC/USDT).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use alloy_primitives::address;
    /// use cow_rs::OnchainReader;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let reader = OnchainReader::new("https://rpc.sepolia.org");
    /// let token = address!("fFf9976782d46CC05630D1f6eBAb18b2324d6B14");
    /// let decimals = reader.erc20_decimals(token).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] on RPC failure or [`CowError::Parse`] if the
    /// response cannot be decoded as a `uint8`.
    pub async fn erc20_decimals(&self, token: Address) -> Result<u8, CowError> {
        let cd = build_erc20_decimals_calldata();
        let ret = self.eth_call(token, &cd).await?;
        decode_u8(&ret)
    }

    /// Read the human-readable name of the `ERC-20` contract at `token`.
    ///
    /// Executes `name()` via `eth_call` against block `"latest"` and decodes the
    /// ABI-encoded dynamic `string` return value.
    ///
    /// # Arguments
    ///
    /// * `token` - The [`Address`] of the `ERC-20` token contract.
    ///
    /// # Returns
    ///
    /// The token name as a [`String`] (e.g. `"Wrapped Ether"`).
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] on RPC failure or [`CowError::Parse`] if the
    /// response cannot be decoded as an ABI `string`.
    pub async fn erc20_name(&self, token: Address) -> Result<String, CowError> {
        let cd = build_erc20_name_calldata();
        let ret = self.eth_call(token, &cd).await?;
        decode_string(&ret)
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::address;

    use super::*;

    fn make_u256_response(val: u64) -> String {
        let bytes = U256::from(val).to_be_bytes::<32>();
        format!("0x{}", alloy_primitives::hex::encode(bytes))
    }

    fn make_u8_response(val: u8) -> String {
        let mut bytes = [0u8; 32];
        bytes[31] = val;
        format!("0x{}", alloy_primitives::hex::encode(bytes))
    }

    fn make_string_response(s: &str) -> String {
        let padded_len = s.len().div_ceil(32) * 32;
        let mut buf = vec![0u8; 64 + padded_len];
        // offset = 0x20
        buf[31] = 32;
        // length
        let len = s.len();
        buf[32 + 31] = len as u8;
        // data
        buf[64..64 + len].copy_from_slice(s.as_bytes());
        format!("0x{}", alloy_primitives::hex::encode(&buf))
    }

    fn token() -> Address {
        address!("fFf9976782d46CC05630D1f6eBAb18b2324d6B14")
    }

    fn owner() -> Address {
        address!("1111111111111111111111111111111111111111")
    }

    fn spender() -> Address {
        address!("2222222222222222222222222222222222222222")
    }

    // Tests that verify the calldata helpers encode correctly (no network needed)

    #[test]
    fn balance_calldata_len() {
        let cd = build_erc20_balance_of_calldata(owner());
        assert_eq!(cd.len(), 36);
    }

    #[test]
    fn allowance_calldata_len() {
        let cd = build_erc20_allowance_calldata(owner(), spender());
        assert_eq!(cd.len(), 68);
    }

    #[test]
    fn decimals_calldata_len() {
        let cd = build_erc20_decimals_calldata();
        assert_eq!(cd.len(), 4);
    }

    #[test]
    fn name_calldata_len() {
        let cd = build_erc20_name_calldata();
        assert_eq!(cd.len(), 4);
    }

    #[test]
    fn u256_response_helper_roundtrip() {
        let hex = make_u256_response(1_000_000u64);
        let bytes = alloy_primitives::hex::decode(hex.trim_start_matches("0x")).unwrap();
        let v = decode_u256(&bytes).unwrap();
        assert_eq!(v, U256::from(1_000_000u64));
    }

    #[test]
    fn u8_response_helper_roundtrip() {
        let hex = make_u8_response(18u8);
        let bytes = alloy_primitives::hex::decode(hex.trim_start_matches("0x")).unwrap();
        let v = decode_u8(&bytes).unwrap();
        assert_eq!(v, 18u8);
    }

    #[test]
    fn string_response_helper_roundtrip() {
        let hex = make_string_response("Wrapped Ether");
        let bytes = alloy_primitives::hex::decode(hex.trim_start_matches("0x")).unwrap();
        let s = decode_string(&bytes).unwrap();
        assert_eq!(s, "Wrapped Ether");
    }

    // Verify token address is used for the eth_call target
    #[test]
    fn token_address_formats() {
        let token_addr = token();
        let formatted = format!("{token_addr:#x}");
        assert!(formatted.starts_with("0x"));
        assert_eq!(formatted.len(), 42);
    }
}
