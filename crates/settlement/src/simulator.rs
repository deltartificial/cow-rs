//! Trade simulation for estimating gas costs and detecting reverts.
//!
//! Provides [`TradeSimulator`] for simulating settlement execution against
//! an Ethereum node via JSON-RPC, and [`SimulationResult`] for inspecting
//! the outcome.

use std::fmt;

use alloy_primitives::Address;
use cow_chains::{chain::SupportedChainId, contracts::settlement_contract};
use cow_errors::CowError;

use super::encoder::SettlementEncoder;

/// Result of simulating a settlement transaction via `eth_call`.
///
/// Contains the success status, estimated gas usage, and raw return data
/// from the simulated call.
///
/// # Example
///
/// ```
/// use cow_settlement::simulator::SimulationResult;
///
/// let result = SimulationResult::new(true, 150_000, vec![]);
/// assert!(result.is_success());
/// assert!(!result.is_revert());
/// assert_eq!(result.gas_used, 150_000);
/// ```
#[derive(Debug, Clone)]
pub struct SimulationResult {
    /// Whether the simulation completed without reverting.
    pub success: bool,
    /// Estimated gas consumed by the simulated transaction.
    pub gas_used: u64,
    /// Raw bytes returned by the simulated call.
    pub return_data: Vec<u8>,
}

impl SimulationResult {
    /// Create a new simulation result.
    ///
    /// # Arguments
    ///
    /// * `success` - Whether the simulation succeeded.
    /// * `gas_used` - Estimated gas consumed.
    /// * `return_data` - Raw return bytes from the call.
    ///
    /// # Returns
    ///
    /// A new [`SimulationResult`].
    #[must_use]
    pub const fn new(success: bool, gas_used: u64, return_data: Vec<u8>) -> Self {
        Self { success, gas_used, return_data }
    }

    /// Check whether the simulation succeeded (did not revert).
    ///
    /// # Returns
    ///
    /// `true` if the simulated transaction completed without reverting.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.success
    }

    /// Check whether the simulation reverted.
    ///
    /// # Returns
    ///
    /// `true` if the simulated transaction reverted.
    #[must_use]
    pub const fn is_revert(&self) -> bool {
        !self.success
    }
}

impl fmt::Display for SimulationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.success {
            write!(f, "Success (gas: {})", self.gas_used)
        } else {
            write!(f, "Revert (gas: {}, data: {} bytes)", self.gas_used, self.return_data.len())
        }
    }
}

/// Simulates settlement execution to estimate gas costs and detect reverts.
///
/// Wraps a `reqwest::Client` targeting a JSON-RPC endpoint and the canonical
/// `GPv2Settlement` contract on a specific chain.
///
/// # Example
///
/// ```rust
/// use cow_chains::SupportedChainId;
/// use cow_settlement::simulator::TradeSimulator;
///
/// let sim = TradeSimulator::new("https://rpc.sepolia.org", SupportedChainId::Sepolia);
/// ```
#[derive(Debug, Clone)]
pub struct TradeSimulator {
    /// The JSON-RPC endpoint URL.
    rpc_url: String,
    /// HTTP client for making RPC requests.
    client: reqwest::Client,
    /// The settlement contract address on the target chain.
    settlement: Address,
}

impl TradeSimulator {
    /// Build a `reqwest::Client` with platform-appropriate settings.
    ///
    /// # Returns
    ///
    /// A configured [`reqwest::Client`] with a 30-second timeout on native targets,
    /// or a default client on WASM targets.
    #[allow(clippy::shadow_reuse, reason = "builder pattern chains naturally shadow")]
    fn build_client() -> reqwest::Client {
        let builder = reqwest::Client::builder();
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.timeout(std::time::Duration::from_secs(30));
        builder.build().unwrap_or_default()
    }

    /// Create a new trade simulator for the given chain.
    ///
    /// Uses the canonical `GPv2Settlement` contract address for `chain`.
    ///
    /// # Arguments
    ///
    /// * `rpc_url` - The JSON-RPC endpoint URL.
    /// * `chain` - The target [`SupportedChainId`].
    ///
    /// # Returns
    ///
    /// A new [`TradeSimulator`] configured for the specified chain.
    #[must_use]
    pub fn new(rpc_url: impl Into<String>, chain: SupportedChainId) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            client: Self::build_client(),
            settlement: settlement_contract(chain),
        }
    }

    /// Return the settlement contract address this simulator targets.
    ///
    /// # Returns
    ///
    /// The settlement contract [`Address`].
    #[must_use]
    pub const fn settlement_address(&self) -> Address {
        self.settlement
    }

    /// Return the RPC URL this simulator is configured to use.
    ///
    /// # Returns
    ///
    /// A reference to the RPC URL string.
    #[must_use]
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Estimate the gas cost of executing calldata against the settlement contract.
    ///
    /// Sends an `eth_estimateGas` JSON-RPC request with the provided calldata
    /// targeting the settlement contract.
    ///
    /// # Arguments
    ///
    /// * `calldata` - The ABI-encoded calldata to estimate gas for.
    ///
    /// # Returns
    ///
    /// The estimated gas as `u64`.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] if the RPC request fails or the node returns
    /// an error (e.g., the transaction would revert).
    pub async fn estimate_gas(&self, calldata: &[u8]) -> Result<u64, CowError> {
        let to_hex = format!("{:#x}", self.settlement);
        let data_hex = format!("0x{}", alloy_primitives::hex::encode(calldata));

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method":  "eth_estimateGas",
            "params":  [{"to": to_hex, "data": data_hex}],
            "id":      1u32
        });

        let resp = self.client.post(&self.rpc_url).json(&body).send().await?;

        if !resp.status().is_success() {
            let code = i64::from(resp.status().as_u16());
            let msg = resp.text().await.unwrap_or_else(|_e| String::new());
            return Err(CowError::Rpc { code, message: msg });
        }

        let rpc: RpcResponse = resp.json().await?;

        if let Some(err) = rpc.error {
            return Err(CowError::Rpc { code: err.code, message: err.message });
        }

        let hex_str = rpc
            .result
            .ok_or_else(|| CowError::Rpc { code: -1, message: "missing result field".into() })?;

        parse_hex_u64(&hex_str)
    }

    /// Simulate executing calldata against the settlement contract via `eth_call`.
    ///
    /// Unlike [`estimate_gas`](Self::estimate_gas), this method does not fail on
    /// reverts — instead it returns a [`SimulationResult`] indicating whether the
    /// call succeeded or reverted.
    ///
    /// # Arguments
    ///
    /// * `calldata` - The ABI-encoded calldata to simulate.
    ///
    /// # Returns
    ///
    /// A [`SimulationResult`] with success status, gas estimate, and return data.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] only on transport-level failures (HTTP errors).
    /// Execution reverts are captured in the returned [`SimulationResult`].
    pub async fn simulate(&self, calldata: &[u8]) -> Result<SimulationResult, CowError> {
        let to_hex = format!("{:#x}", self.settlement);
        let data_hex = format!("0x{}", alloy_primitives::hex::encode(calldata));

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method":  "eth_call",
            "params":  [{"to": to_hex, "data": data_hex}, "latest"],
            "id":      1u32
        });

        let resp = self.client.post(&self.rpc_url).json(&body).send().await?;

        if !resp.status().is_success() {
            let code = i64::from(resp.status().as_u16());
            let msg = resp.text().await.unwrap_or_else(|_e| String::new());
            return Err(CowError::Rpc { code, message: msg });
        }

        let rpc: RpcResponse = resp.json().await?;

        if let Some(err) = rpc.error {
            // Execution revert — capture as a failed simulation rather than
            // propagating as an error.
            return Ok(SimulationResult::new(false, 0, err.message.into_bytes()));
        }

        let hex_str = rpc
            .result
            .ok_or_else(|| CowError::Rpc { code: -1, message: "missing result field".into() })?;

        let return_data = decode_hex_result(&hex_str)?;

        // Attempt a gas estimate for successful simulations.
        let gas_used = self.estimate_gas(calldata).await.unwrap_or_default();

        Ok(SimulationResult::new(true, gas_used, return_data))
    }

    /// Convenience method: encode a settlement and estimate its gas cost.
    ///
    /// Combines [`SettlementEncoder::encode_settlement`] with
    /// [`estimate_gas`](Self::estimate_gas).
    ///
    /// # Arguments
    ///
    /// * `encoder` - The [`SettlementEncoder`] containing the settlement to estimate.
    ///
    /// # Returns
    ///
    /// The estimated gas as `u64`.
    ///
    /// # Errors
    ///
    /// Returns [`CowError::Rpc`] if the RPC request fails or the settlement
    /// would revert.
    pub async fn estimate_settlement(&self, encoder: &SettlementEncoder) -> Result<u64, CowError> {
        let calldata = encoder.encode_settlement();
        self.estimate_gas(&calldata).await
    }
}

// ── JSON-RPC response types (private) ────────────────────────────────────────

#[derive(serde::Deserialize)]
struct RpcResponse {
    result: Option<String>,
    error: Option<RpcError>,
}

#[derive(serde::Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

// ── Private helpers ──────────────────────────────────────────────────────────

/// Parse a `0x`-prefixed hex string as a `u64`.
fn parse_hex_u64(hex_str: &str) -> Result<u64, CowError> {
    let clean = hex_str.trim_start_matches("0x");
    u64::from_str_radix(clean, 16)
        .map_err(|e| CowError::Parse { field: "gas_estimate", reason: format!("invalid hex: {e}") })
}

/// Decode a `0x`-prefixed hex result string into bytes.
fn decode_hex_result(hex_str: &str) -> Result<Vec<u8>, CowError> {
    let clean = hex_str.trim_start_matches("0x");
    alloy_primitives::hex::decode(clean)
        .map_err(|e| CowError::Rpc { code: -1, message: format!("hex decode: {e}") })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cow_chains::contracts::SETTLEMENT_CONTRACT;

    // ── SimulationResult tests ───────────────────────────────────────────

    #[test]
    fn simulation_result_new() {
        let result = SimulationResult::new(true, 100_000, vec![0xab, 0xcd]);
        assert!(result.success);
        assert_eq!(result.gas_used, 100_000);
        assert_eq!(result.return_data, vec![0xab, 0xcd]);
    }

    #[test]
    fn simulation_result_is_success() {
        let success = SimulationResult::new(true, 50_000, vec![]);
        assert!(success.is_success());
        assert!(!success.is_revert());
    }

    #[test]
    fn simulation_result_is_revert() {
        let revert = SimulationResult::new(false, 0, vec![0xff]);
        assert!(!revert.is_success());
        assert!(revert.is_revert());
    }

    #[test]
    fn simulation_result_display_success() {
        let result = SimulationResult::new(true, 150_000, vec![]);
        assert_eq!(format!("{result}"), "Success (gas: 150000)");
    }

    #[test]
    fn simulation_result_display_revert() {
        let result = SimulationResult::new(false, 21_000, vec![0xde, 0xad]);
        assert_eq!(format!("{result}"), "Revert (gas: 21000, data: 2 bytes)");
    }

    #[test]
    fn simulation_result_clone() {
        let result = SimulationResult::new(true, 42, vec![1, 2, 3]);
        let cloned = result.clone();
        assert_eq!(cloned.success, result.success);
        assert_eq!(cloned.gas_used, result.gas_used);
        assert_eq!(cloned.return_data, result.return_data);
    }

    // ── TradeSimulator construction tests ────────────────────────────────

    #[test]
    fn trade_simulator_new_mainnet() {
        let sim = TradeSimulator::new("https://eth.example.com", SupportedChainId::Mainnet);
        assert_eq!(sim.settlement_address(), SETTLEMENT_CONTRACT);
        assert_eq!(sim.rpc_url(), "https://eth.example.com");
    }

    #[test]
    fn trade_simulator_new_sepolia() {
        let sim = TradeSimulator::new("https://sepolia.example.com", SupportedChainId::Sepolia);
        assert_eq!(sim.settlement_address(), settlement_contract(SupportedChainId::Sepolia));
        assert_eq!(sim.rpc_url(), "https://sepolia.example.com");
    }

    #[test]
    fn trade_simulator_new_gnosis() {
        let sim = TradeSimulator::new("https://gnosis.example.com", SupportedChainId::GnosisChain);
        assert_eq!(sim.settlement_address(), settlement_contract(SupportedChainId::GnosisChain));
    }

    #[test]
    fn trade_simulator_new_arbitrum() {
        let sim = TradeSimulator::new("https://arb.example.com", SupportedChainId::ArbitrumOne);
        assert_eq!(sim.settlement_address(), settlement_contract(SupportedChainId::ArbitrumOne));
    }

    #[test]
    fn trade_simulator_clone() {
        let sim = TradeSimulator::new("https://example.com", SupportedChainId::Mainnet);
        let cloned = sim.clone();
        assert_eq!(cloned.settlement_address(), sim.settlement_address());
        assert_eq!(cloned.rpc_url(), sim.rpc_url());
    }

    // ── Helper tests ────────────────────────────────────────────────────

    #[test]
    fn parse_hex_u64_valid() {
        assert_eq!(parse_hex_u64("0x5208").unwrap(), 21_000);
    }

    #[test]
    fn parse_hex_u64_no_prefix() {
        assert_eq!(parse_hex_u64("ff").unwrap(), 255);
    }

    #[test]
    fn parse_hex_u64_invalid() {
        assert!(parse_hex_u64("0xZZZZ").is_err());
    }

    #[test]
    fn decode_hex_result_valid() {
        let bytes = decode_hex_result("0xdeadbeef").unwrap();
        assert_eq!(bytes, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn decode_hex_result_empty() {
        let bytes = decode_hex_result("0x").unwrap();
        assert!(bytes.is_empty());
    }
}
