/// A registered mod. A slot reused after unregister gets a new generation.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmmHandle {
    pub slot: u16,
    pub generation: u16,
}
