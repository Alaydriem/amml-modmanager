use crate::structs::clash::ClashKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clash {
    pub kind: ClashKind,
    // the operator stored a resolution for this clash
    pub reviewed: bool,
}
