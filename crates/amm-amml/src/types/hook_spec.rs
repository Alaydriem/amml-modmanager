use std::ffi::c_void;
use std::sync::atomic::AtomicPtr;

use super::HookGroup;

/// One game hook: the entry of its target, the detour written for that function's signature, the detour's
/// original slot, the group disable removes it with, and the feature it serves. A hook that is not required is
/// skipped when its entry is missing.
#[derive(Clone, Copy)]
pub struct HookSpec {
    pub entry: &'static str,
    pub detour: *const c_void,
    pub original: &'static AtomicPtr<c_void>,
    pub group: HookGroup,
    pub required: bool,
    pub feature: &'static str,
}
