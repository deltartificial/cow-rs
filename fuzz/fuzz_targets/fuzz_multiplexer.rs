#![no_main]

use cow_rs::composable::{Multiplexer, ProofLocation};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // JSON deserialization — must not panic on malformed input
        let _ = Multiplexer::from_json(s);
        let _ = Multiplexer::decode_proofs_from_json(s);

        // Roundtrip: build a multiplexer, serialize, deserialize
        let mux = Multiplexer::new(ProofLocation::Emitted);
        if let Ok(json) = mux.to_json() {
            let decoded = Multiplexer::from_json(&json);
            assert!(decoded.is_ok(), "roundtrip from_json failed");
        }
    }
});
