use super::AmmAcquireOutcome;

/// The answer to `acquire`. `holder` is the mod index of the current holder when `outcome` is `Denied`.
/// `0xFF` means no holder.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmmAcquireResult {
    pub outcome: AmmAcquireOutcome,
    pub holder: u8,
}

impl AmmAcquireResult {
    /// The `holder` value that names no mod.
    pub const NO_HOLDER: u8 = 0xFF;
}
