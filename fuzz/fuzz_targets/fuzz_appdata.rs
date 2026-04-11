#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // AppData hex to CID conversion
        let _ = cow_rs::appdata_hex_to_cid(s);

        // CID to AppData hex conversion
        let _ = cow_rs::cid_to_appdata_hex(s);

        // CID assertion (both args from fuzzed input)
        if data.len() >= 2 {
            let mid = data.len() / 2;
            if let (Ok(a), Ok(b)) =
                (std::str::from_utf8(&data[..mid]), std::str::from_utf8(&data[mid..]))
            {
                let _ = cow_rs::assert_cid(a, b);
            }
        }

        // JSON app-data document building
        let _ = cow_rs::build_order_app_data(s);
    }
});
