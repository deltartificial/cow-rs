#![no_main]

use cow_rs::order_signing::{decode_eip1271_signature_data, decode_signature_owner};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // EIP-1271 signature decoding — parses (owner ++ signature) from raw bytes
    let _ = decode_eip1271_signature_data(data);

    // Signature owner extraction
    let _ = decode_signature_owner(data);
});
