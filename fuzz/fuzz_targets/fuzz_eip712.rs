#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Order UID extraction — parses hex-encoded UID into components
        let _ = cow_rs::order_signing::extract_order_uid_params(s);

        // Single cancellation hash
        if let Ok(hash) = cow_rs::order_signing::hash_order_cancellation(1, s) {
            // Hash must be deterministic
            if let Ok(hash2) = cow_rs::order_signing::hash_order_cancellation(1, s) {
                assert_eq!(hash, hash2);
            }
        }

        // Batch cancellation hash
        let uids: Vec<&str> = s.split(',').collect();
        let _ = cow_rs::order_signing::hash_order_cancellations(1, &uids);
        let _ = cow_rs::order_signing::cancellations_hash(&uids);
    }

    // Domain separator with arbitrary chain IDs
    if data.len() >= 8 {
        let chain_id = u64::from_le_bytes(data[..8].try_into().unwrap_or([0; 8]));
        let _ = cow_rs::order_signing::domain_separator(chain_id);
    }
});
