#![no_main]

use cow_rs::common::address::{
    are_addresses_equal, get_address_key, get_btc_address_key, get_evm_address_key,
    get_sol_address_key, get_token_id, is_btc_address, is_evm_address, is_native_token,
    is_solana_address, is_supported_address, is_wrapped_native_token,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Address format validation — must not panic on any input
        let _ = is_evm_address(s);
        let _ = is_btc_address(s);
        let _ = is_solana_address(s);
        let _ = is_supported_address(s);

        // Key derivation
        let _ = get_evm_address_key(s);
        let _ = get_btc_address_key(s);
        let _ = get_sol_address_key(s);
        let _ = get_address_key(s);

        // Token identification
        let _ = get_token_id(1, s);
        let _ = is_native_token(1, s);
        let _ = is_wrapped_native_token(1, s);

        // Comparison with self and None
        let _ = are_addresses_equal(Some(s), Some(s));
        let _ = are_addresses_equal(Some(s), None);
        let _ = are_addresses_equal(None, Some(s));

        // Split input to compare two arbitrary addresses
        if data.len() >= 2 {
            let mid = data.len() / 2;
            if let (Ok(a), Ok(b)) =
                (std::str::from_utf8(&data[..mid]), std::str::from_utf8(&data[mid..]))
            {
                let _ = are_addresses_equal(Some(a), Some(b));
            }
        }
    }
});
