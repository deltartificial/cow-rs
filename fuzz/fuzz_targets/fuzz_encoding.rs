#![no_main]

use alloy_primitives::{Address, U256};
use cow_rs::flash_loans::FlashLoanSdk;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Flash loan encoding with arbitrary user_data
    let calldata = FlashLoanSdk::encode_balancer_flash_loan(
        Address::ZERO,
        Address::ZERO,
        U256::ZERO,
        data,
    );
    // Encoded calldata must always be valid (no panic)
    assert!(calldata.len() >= 4, "calldata must have at least a selector");

    // CowShed encoding with fuzzed bytes as hook calldata
    // (structured input needed — just ensure no panics on the encoding path)
});
