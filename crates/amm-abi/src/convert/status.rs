use amm_core::errors::{CaptureError, DescriptorError, IconError, LinkError, ManagerError, RegistryError, StoreError};

use crate::types::AmmStatus;

impl From<&DescriptorError> for AmmStatus {
    fn from(_: &DescriptorError) -> Self {
        AmmStatus::InvalidDescriptor
    }
}

impl From<&RegistryError> for AmmStatus {
    fn from(value: &RegistryError) -> Self {
        match value {
            RegistryError::TooManyMods => AmmStatus::TooManyMods,
            RegistryError::DuplicateId(_) => AmmStatus::DuplicateId,
        }
    }
}

impl From<&ManagerError> for AmmStatus {
    fn from(value: &ManagerError) -> Self {
        match value {
            ManagerError::Descriptor(error) => AmmStatus::from(error),
            ManagerError::Registry(error) => AmmStatus::from(error),
            ManagerError::UnknownMod => AmmStatus::UnknownHandle,
            ManagerError::NotPermitted => AmmStatus::NotPermitted,
            ManagerError::Setting { .. } => AmmStatus::InvalidArgument,
            ManagerError::Store(StoreError::QueueFull) => AmmStatus::QueueFull,
            ManagerError::Store(_) => AmmStatus::Unsupported,
            ManagerError::MenuEntry => AmmStatus::InvalidArgument,
            ManagerError::MenuEntryLimit => AmmStatus::QueueFull,
            ManagerError::RenderSizeOutOfRange => AmmStatus::InvalidArgument,
            ManagerError::RenderSizeClosed => AmmStatus::NotPermitted,
            ManagerError::InvalidArgument => AmmStatus::InvalidArgument,
        }
    }
}

impl From<&CaptureError> for AmmStatus {
    fn from(value: &CaptureError) -> Self {
        match value {
            CaptureError::UnknownMod => AmmStatus::UnknownHandle,
            CaptureError::NotPermitted => AmmStatus::NotPermitted,
            CaptureError::InvalidPath => AmmStatus::InvalidArgument,
            CaptureError::Busy => AmmStatus::Busy,
        }
    }
}

impl From<&IconError> for AmmStatus {
    fn from(_: &IconError) -> Self {
        AmmStatus::InvalidArgument
    }
}

impl From<AmmStatus> for LinkError {
    fn from(value: AmmStatus) -> Self {
        match value {
            AmmStatus::ManagerMissing => LinkError::ManagerMissing,
            other => LinkError::Status(other as i32),
        }
    }
}

impl TryFrom<i32> for AmmStatus {
    type Error = i32;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(AmmStatus::Ok),
            1 => Ok(AmmStatus::NullPointer),
            2 => Ok(AmmStatus::InvalidArgument),
            3 => Ok(AmmStatus::InvalidDescriptor),
            4 => Ok(AmmStatus::DuplicateId),
            5 => Ok(AmmStatus::TooManyMods),
            6 => Ok(AmmStatus::UnknownHandle),
            7 => Ok(AmmStatus::NotPermitted),
            8 => Ok(AmmStatus::QueueFull),
            9 => Ok(AmmStatus::Panic),
            10 => Ok(AmmStatus::Unsupported),
            11 => Ok(AmmStatus::ManagerMissing),
            12 => Ok(AmmStatus::Busy),
            other => Err(other),
        }
    }
}
