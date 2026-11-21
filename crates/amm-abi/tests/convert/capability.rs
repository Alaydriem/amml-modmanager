use amm_abi::types::{AmmCapability, AmmStatus};
use amm_core::structs::descriptor::Capability;

#[test]
fn every_manager_lease_has_a_byte_and_reads_back() {
    for capability in Capability::MANAGER {
        let abi = AmmCapability::try_from(capability).expect("a manager lease has an ABI value");
        assert_eq!(AmmCapability::try_from(abi as u8).map(Capability::from), Ok(capability));
    }
}

#[test]
fn input_has_no_byte_and_its_old_byte_is_rejected() {
    assert_eq!(AmmCapability::try_from(Capability::Input), Err(AmmStatus::InvalidArgument));
    assert_eq!(AmmCapability::try_from(1), Err(AmmStatus::InvalidArgument));
    assert_eq!(AmmCapability::try_from(6), Err(AmmStatus::InvalidArgument));
    assert_eq!(AmmCapability::try_from(255), Err(AmmStatus::InvalidArgument));
}
