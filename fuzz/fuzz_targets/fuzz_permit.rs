#![no_main]

use alloy_primitives::{Address, U256};
use cow_rs::permit::{permit_digest, permit_domain_separator, permit_type_hash};
use cow_rs::PermitInfo;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // permit_type_hash is constant — must never panic
    let _ = permit_type_hash();

    if let Ok(s) = std::str::from_utf8(data) {
        // Split fuzzed string into name/version for domain separator
        if let Some((name, version)) = s.split_once('\0') {
            let _ = permit_domain_separator(name, version, 1, Address::ZERO);
        }
    }

    // Permit digest with arbitrary domain separator bytes
    if data.len() >= 32 {
        let mut domain = [0u8; 32];
        domain.copy_from_slice(&data[..32]);

        let info = PermitInfo::new(Address::ZERO, Address::ZERO, Address::ZERO, U256::ZERO);
        let _ = permit_digest(domain.into(), &info);
    }
});
