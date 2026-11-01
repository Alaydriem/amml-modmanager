use amm_abi::types::{AmmLogLevel, AmmStatus};

#[test]
fn log_level_byte_out_of_range_is_rejected() {
    assert_eq!(AmmLogLevel::try_from(0), Ok(AmmLogLevel::Error));
    assert_eq!(AmmLogLevel::try_from(4), Ok(AmmLogLevel::Trace));
    assert_eq!(AmmLogLevel::try_from(5), Err(AmmStatus::InvalidArgument));
    assert_eq!(AmmLogLevel::try_from(255), Err(AmmStatus::InvalidArgument));
}
