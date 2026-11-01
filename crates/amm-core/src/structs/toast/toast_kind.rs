use crate::structs::descriptor::Capability;
use crate::structs::registry::ModIndex;

/// What a toast says. Plain data: the panel builds the text when it draws the toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Denied { capability: Capability, holder: ModIndex, requester: ModIndex },
    Blocked { capability: Capability, holder: ModIndex, mod_index: ModIndex, action: u16 },
    FrameDisabled(ModIndex),
    SaveFailed,
}

impl ToastKind {
    /// The (capability, mod) pair that limits how often this kind is shown. Kinds without a pair are never limited.
    pub fn rate_key(&self) -> Option<(Capability, ModIndex)> {
        match self {
            ToastKind::Denied { capability, requester, .. } => Some((*capability, *requester)),
            ToastKind::Blocked { capability, mod_index, .. } => Some((*capability, *mod_index)),
            ToastKind::FrameDisabled(_) | ToastKind::SaveFailed => None,
        }
    }
}
