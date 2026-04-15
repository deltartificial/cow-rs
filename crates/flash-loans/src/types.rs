//! Flash loan provider types and parameter structs.
//!
//! Defines the [`FlashLoanProvider`] enum (Balancer, `MakerDAO`, Aave V3) and
//! the [`FlashLoanParams`] struct that bundles all parameters needed to build
//! a flash loan pre-interaction hook.

use alloy_primitives::{Address, U256, address};

/// Supported flash loan providers.
///
/// Each variant corresponds to a `DeFi` protocol that offers flash loans.
/// Use [`contract_address`](Self::contract_address) to look up the
/// provider's contract on a given chain, and
/// [`is_supported_on`](Self::is_supported_on) to check availability.
///
/// # Example
///
/// ```
/// use cow_rs::flash_loans::FlashLoanProvider;
///
/// let provider = FlashLoanProvider::Balancer;
/// assert_eq!(provider.name(), "Balancer");
/// assert!(provider.is_supported_on(1)); // Ethereum mainnet
/// assert!(!provider.is_supported_on(42161)); // not on Arbitrum
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashLoanProvider {
    /// Balancer vault flash loans (available on Ethereum mainnet and Gnosis).
    Balancer,
    /// `MakerDAO` flash loans via `DssFlash` (Ethereum mainnet only).
    MakerDao,
    /// Aave V3 flash loans (available on Ethereum mainnet and Gnosis).
    AaveV3,
}

impl FlashLoanProvider {
    /// Returns the canonical human-readable name for this provider.
    ///
    /// # Returns
    ///
    /// `"Balancer"`, `"MakerDAO"`, or `"Aave V3"`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Balancer => "Balancer",
            Self::MakerDao => "MakerDAO",
            Self::AaveV3 => "Aave V3",
        }
    }

    /// Returns the flash loan contract address for this provider on
    /// `chain_id`, or `None` if the provider is not deployed on that chain.
    ///
    /// # Parameters
    ///
    /// * `chain_id` — the numeric EIP-155 chain ID.
    ///
    /// # Returns
    ///
    /// `Some(address)` if the provider has a contract on `chain_id`,
    /// `None` otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use cow_rs::flash_loans::FlashLoanProvider;
    ///
    /// let addr = FlashLoanProvider::Balancer.contract_address(1);
    /// assert!(addr.is_some());
    ///
    /// let none = FlashLoanProvider::MakerDao.contract_address(999);
    /// assert!(none.is_none());
    /// ```
    #[must_use]
    pub const fn contract_address(self, chain_id: u64) -> Option<Address> {
        match (self, chain_id) {
            (Self::Balancer, 1 | 100) => Some(address!("BA12222222228d8Ba445958a75a0704d566BF2C8")),
            (Self::AaveV3, 1) => Some(address!("87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2")),
            (Self::AaveV3, 100) => Some(address!("b50201558B00496A145fE76f7424749556E326D8")),
            _ => None,
        }
    }

    /// Returns `true` if this provider is available on `chain_id`.
    ///
    /// # Parameters
    ///
    /// * `chain_id` — the numeric EIP-155 chain ID.
    ///
    /// # Returns
    ///
    /// `true` if [`contract_address`](Self::contract_address) returns `Some`.
    #[must_use]
    pub const fn is_supported_on(self, chain_id: u64) -> bool {
        self.contract_address(chain_id).is_some()
    }
}

/// Parameters for a flash loan pre-interaction hook.
///
/// Bundles the provider, token, amount, and chain ID needed by
/// [`FlashLoanSdk::build_flash_loan_hook`](super::FlashLoanSdk::build_flash_loan_hook)
/// to generate the calldata for a flash loan pre-interaction.
///
/// # Example
///
/// ```
/// use alloy_primitives::{Address, U256};
/// use cow_rs::flash_loans::{FlashLoanParams, FlashLoanProvider};
///
/// let params = FlashLoanParams::new(
///     FlashLoanProvider::Balancer,
///     Address::ZERO,
///     U256::from(1_000_000u64),
///     1, // Ethereum mainnet
/// );
/// assert_eq!(params.provider_name(), "Balancer");
/// assert!(params.is_provider_supported());
/// ```
#[derive(Debug, Clone)]
pub struct FlashLoanParams {
    /// The flash loan provider to use.
    pub provider: FlashLoanProvider,
    /// The ERC-20 token to borrow.
    pub token: Address,
    /// Amount to borrow in token atoms (e.g. `1_000_000` for 1 USDC).
    pub amount: U256,
    /// EIP-155 chain ID where the flash loan will execute.
    pub chain_id: u64,
}

impl FlashLoanParams {
    /// Construct a new [`FlashLoanParams`].
    ///
    /// # Parameters
    ///
    /// * `provider` — which flash loan protocol to use.
    /// * `token` — the ERC-20 [`Address`] of the token to borrow.
    /// * `amount` — borrow amount in token atoms.
    /// * `chain_id` — the EIP-155 chain ID where the loan executes.
    ///
    /// # Returns
    ///
    /// A new [`FlashLoanParams`] instance.
    #[must_use]
    pub const fn new(
        provider: FlashLoanProvider,
        token: Address,
        amount: U256,
        chain_id: u64,
    ) -> Self {
        Self { provider, token, amount, chain_id }
    }

    /// The human-readable name of the provider (e.g. `"Balancer"`).
    ///
    /// Delegates to [`FlashLoanProvider::name`].
    ///
    /// # Returns
    ///
    /// A static string such as `"Balancer"`, `"MakerDAO"`, or `"Aave V3"`.
    #[must_use]
    pub const fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    /// Returns `true` if the provider is deployed on [`Self::chain_id`].
    ///
    /// Delegates to [`FlashLoanProvider::is_supported_on`].
    ///
    /// # Returns
    ///
    /// `true` if the provider has a contract on this params' chain.
    #[must_use]
    pub const fn is_provider_supported(&self) -> bool {
        self.provider.is_supported_on(self.chain_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_loan_provider_name() {
        assert_eq!(FlashLoanProvider::Balancer.name(), "Balancer");
        assert_eq!(FlashLoanProvider::MakerDao.name(), "MakerDAO");
        assert_eq!(FlashLoanProvider::AaveV3.name(), "Aave V3");
    }

    #[test]
    fn flash_loan_provider_contract_address_balancer() {
        assert!(FlashLoanProvider::Balancer.contract_address(1).is_some());
        assert!(FlashLoanProvider::Balancer.contract_address(100).is_some());
        assert!(FlashLoanProvider::Balancer.contract_address(42161).is_none());
    }

    #[test]
    fn flash_loan_provider_contract_address_maker() {
        // MakerDAO has no contract addresses in the current implementation
        assert!(FlashLoanProvider::MakerDao.contract_address(1).is_none());
        assert!(FlashLoanProvider::MakerDao.contract_address(999).is_none());
    }

    #[test]
    fn flash_loan_provider_contract_address_aave() {
        assert!(FlashLoanProvider::AaveV3.contract_address(1).is_some());
        assert!(FlashLoanProvider::AaveV3.contract_address(100).is_some());
        assert!(FlashLoanProvider::AaveV3.contract_address(42161).is_none());
    }

    #[test]
    fn flash_loan_provider_is_supported_on() {
        assert!(FlashLoanProvider::Balancer.is_supported_on(1));
        assert!(!FlashLoanProvider::Balancer.is_supported_on(42161));
    }

    #[test]
    fn flash_loan_params_new() {
        let params = FlashLoanParams::new(
            FlashLoanProvider::Balancer,
            Address::ZERO,
            U256::from(1_000_000u64),
            1,
        );
        assert_eq!(params.provider_name(), "Balancer");
        assert!(params.is_provider_supported());
    }

    #[test]
    fn flash_loan_params_unsupported_chain() {
        let params = FlashLoanParams::new(
            FlashLoanProvider::Balancer,
            Address::ZERO,
            U256::from(1_000_000u64),
            999,
        );
        assert!(!params.is_provider_supported());
    }
}
