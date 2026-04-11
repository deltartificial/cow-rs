#![no_main]

use cow_rs::bridging::bungee::{
    decode_bungee_bridge_tx_data, get_bungee_bridge_from_display_name,
    is_valid_bungee_events_response, is_valid_quote_response,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Bridge tx data decoding — hex string parsing
        let _ = decode_bungee_bridge_tx_data(s);

        // Display name lookup
        let _ = get_bungee_bridge_from_display_name(s);

        // JSON response validation
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(s) {
            let _ = is_valid_quote_response(&val);
            let _ = is_valid_bungee_events_response(&val);
        }
    }
});
