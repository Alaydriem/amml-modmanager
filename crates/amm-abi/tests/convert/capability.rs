use amm_abi::types::{AmmCapability, AmmStatus};
use amm_core::structs::descriptor::Capability;

#[test]
fn capability_byte_out_of_range_is_rejected() {
    for (byte, capability) in Capability::ALL.into_iter().enumerate() {
        let abi = AmmCapability::try_from(byte as u8);
        assert_eq!(abi.map(Capability::from), Ok(capability));
    }
    assert_eq!(AmmCapability::try_from(Capability::ALL.len() as u8), Err(AmmStatus::InvalidArgument));
    assert_eq!(AmmCapability::try_from(255), Err(AmmStatus::InvalidArgument));
}
