#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz raw byte CID decoding
    let _ = cow_rs::decode_cid(data);

    // Fuzz string-based CID functions
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = cow_rs::parse_cid(s);
        let _ = cow_rs::extract_digest(s);
        let _ = cow_rs::cid_to_appdata_hex(s);

        // Roundtrip: if hex→cid succeeds, cid→hex should not panic
        if let Ok(cid) = cow_rs::appdata_hex_to_cid(s) {
            let _ = cow_rs::cid_to_appdata_hex(&cid);
        }
    }
});
