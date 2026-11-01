use crate::errors::{DescriptorError, RegistryError, StoreError};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManagerError {
    #[error(transparent)]
    Descriptor(#[from] DescriptorError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error("no mod is registered in that slot")]
    UnknownMod,
    #[error("the mod does not hold what the call needs")]
    NotPermitted,
    #[error("setting {path} is not in the schema, or the value does not fit it")]
    Setting { path: String },
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("a menu entry needs a 1 to 24 character label, a 1 to 3 character mark and at least one screen")]
    MenuEntry,
    #[error("the mod or the manager already has the most menu entries it may hold")]
    MenuEntryLimit,
    #[error("a forced render size needs both sides 1 to 16384")]
    RenderSizeOutOfRange,
    #[error("a virtual-key code is 255 at most, and a watch holds 32 keys at most")]
    InvalidArgument,
    #[error("the mod manager is shutting down, so it forces no render size")]
    RenderSizeClosed,
}
