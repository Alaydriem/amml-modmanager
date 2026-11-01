use crate::structs::toast::ToastKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toast {
    pub kind: ToastKind,
    pub created_ms: u64,
}
