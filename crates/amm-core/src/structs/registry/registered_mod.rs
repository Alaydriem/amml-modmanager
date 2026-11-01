use crate::structs::descriptor::Descriptor;
use crate::structs::registry::ModIndex;

#[derive(Debug, Clone, PartialEq)]
pub struct RegisteredMod {
    pub index: ModIndex,
    pub descriptor: Descriptor,
    pub enabled: bool,
    pub failed: bool,
}
