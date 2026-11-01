use amm_abi::types::{AmmHandle, AmmStatus};
use amm_core::structs::menu::MenuScreens;

use crate::services::{Boundary, HostCell};

/// Table `register_menu_entry`.
///
/// # Safety
///
/// `label` and `mark` are null or hold their lengths in readable bytes for the call; `out_id` is null or writable
/// for the call.
pub unsafe extern "C" fn amm_register_menu_entry(
    handle: AmmHandle,
    label: *const u8,
    label_len: usize,
    mark: *const u8,
    mark_len: usize,
    screens: u8,
    out_id: *mut u16,
) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        let out_id = Boundary::writable(out_id)?;
        // SAFETY: the caller gives label_len readable bytes at label for the call
        let label = unsafe { Boundary::text(label, label_len) }?;
        // SAFETY: the caller gives mark_len readable bytes at mark for the call
        let mark = unsafe { Boundary::text(mark, mark_len) }?;
        let id = HostCell::global()
            .with(|runtime| runtime.register_menu_entry(handle, label, mark, MenuScreens::from_bits(screens)))?;
        // SAFETY: out_id is non-null and the caller gives it writable for the call
        unsafe { out_id.write(id) };
        Ok(())
    }))
}
