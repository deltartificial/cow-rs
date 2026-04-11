#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // TWAP decoding — expects ABI-encoded data, must not panic on arbitrary bytes
    let _ = cow_rs::decode_twap_static_input(data);
    let _ = cow_rs::decode_twap_struct(data);

    // StopLoss decoding
    let _ = cow_rs::decode_stop_loss_static_input(data);

    // GoodAfterTime decoding
    let _ = cow_rs::decode_gat_static_input(data);

    // Hex-string parameter decoding
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(params) = cow_rs::decode_params(s) {
            // Roundtrip: encode then decode must not panic
            let hex = cow_rs::encode_params(&params);
            let _ = cow_rs::decode_params(&hex);
        }
    }
});
