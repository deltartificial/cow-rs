#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // U256 deserialization from arbitrary decimal strings
        let result: Result<alloy_primitives::U256, _> = serde_json::from_str(&format!("\"{s}\""));
        if let Ok(val) = result {
            // Roundtrip: serialize back and verify it doesn't panic
            let serialized = serde_json::to_string(&val);
            assert!(serialized.is_ok());
        }

        // JSON with BigInt replacer — arbitrary JSON input
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(s) {
            let _ = cow_rs::common::serialize::json_with_bigint_replacer(&val);
        }
    }
});
