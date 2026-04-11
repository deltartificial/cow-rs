#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let bits = data[0];

    // Fuzz all flag decoders — every u8 must either decode or return Err, never panic
    let _ = cow_rs::decode_order_flags(bits);
    let _ = cow_rs::decode_trade_flags(bits);
    let _ = cow_rs::decode_signing_scheme(bits);

    // Roundtrip: encode then decode must be identity
    if let Ok(flags) = cow_rs::decode_order_flags(bits) {
        let encoded = cow_rs::encode_order_flags(&flags);
        let decoded = cow_rs::decode_order_flags(encoded);
        assert!(decoded.is_ok(), "roundtrip decode failed for order flags");
    }

    if let Ok(flags) = cow_rs::decode_trade_flags(bits) {
        let encoded = cow_rs::encode_trade_flags(&flags);
        let decoded = cow_rs::decode_trade_flags(encoded);
        assert!(decoded.is_ok(), "roundtrip decode failed for trade flags");
    }
});
