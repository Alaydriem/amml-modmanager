use amm_abi::types::AmmHandle;

use crate::services::{Boundary, HostCell};

/// Table `key_down`. False for a stale handle and for a mod without the `Input` lease.
pub extern "C" fn amm_key_down(handle: AmmHandle, vk: u32) -> bool {
    Boundary::guard(|| HostCell::global().with(|runtime| runtime.key_down(handle, vk))).unwrap_or(false)
}

/// Table `panel_open`. 0 for a stale handle, no host, or a call from inside a frame callback, where the host is busy.
pub extern "C" fn amm_panel_open(handle: AmmHandle) -> u8 {
    u8::from(Boundary::guard(|| HostCell::global().with(|runtime| runtime.panel_open(handle))).unwrap_or(false))
}
