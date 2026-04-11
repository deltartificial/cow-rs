#![no_main]

use alloy_primitives::Address;
use cow_rs::weiroll::{
    create_weiroll_contract, create_weiroll_library, WeirollCommand, WeirollCommandFlags,
    WeirollPlanner,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Command packing with arbitrary bytes
    if data.len() >= 32 {
        let flags = data[0];
        let value = data[1];
        let gas = u16::from_le_bytes([data[2], data[3]]);
        let mut target_bytes = [0u8; 20];
        target_bytes.copy_from_slice(&data[4..24]);
        let target = Address::from(target_bytes);
        let selector: [u8; 4] = [data[24], data[25], data[26], data[27]];
        let in_out: [u8; 4] = [data[28], data[29], data[30], data[31]];

        let cmd = WeirollCommand {
            flags,
            value,
            gas,
            target,
            selector,
            in_out,
        };
        let packed = cmd.pack();
        assert_eq!(packed.len(), 32);
    }

    // Contract/library creation with arbitrary ABI bytes
    let addr = Address::ZERO;
    let _ = create_weiroll_contract(addr, data.to_vec(), None);
    let _ = create_weiroll_contract(addr, data.to_vec(), Some(WeirollCommandFlags::DelegateCall));
    let _ = create_weiroll_library(addr, data.to_vec());

    // Planner operations
    let mut planner = WeirollPlanner::new();
    let _ = planner.add_state_slot(data.to_vec());
    let _ = planner.plan();
});
