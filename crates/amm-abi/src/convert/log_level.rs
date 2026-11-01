use crate::types::{AmmLogLevel, AmmStatus};

impl TryFrom<u8> for AmmLogLevel {
    type Error = AmmStatus;

    fn try_from(value: u8) -> Result<Self, AmmStatus> {
        match value {
            0 => Ok(AmmLogLevel::Error),
            1 => Ok(AmmLogLevel::Warn),
            2 => Ok(AmmLogLevel::Info),
            3 => Ok(AmmLogLevel::Debug),
            4 => Ok(AmmLogLevel::Trace),
            _ => Err(AmmStatus::InvalidArgument),
        }
    }
}
