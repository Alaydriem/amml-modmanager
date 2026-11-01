use amm_abi::types::AmmStatus;
use amm_core::errors::{CaptureError, DescriptorError, LinkError, ManagerError, RegistryError};

#[test]
fn registry_errors_map_to_their_own_status() {
    assert_eq!(AmmStatus::from(&RegistryError::TooManyMods), AmmStatus::TooManyMods);
    assert_eq!(AmmStatus::from(&RegistryError::DuplicateId("a".into())), AmmStatus::DuplicateId);
}

#[test]
fn descriptor_errors_map_to_invalid_descriptor() {
    assert_eq!(AmmStatus::from(&DescriptorError::EmptyId), AmmStatus::InvalidDescriptor);
}

#[test]
fn manager_errors_delegate_to_the_inner_mapping() {
    assert_eq!(AmmStatus::from(&ManagerError::Descriptor(DescriptorError::EmptyId)), AmmStatus::InvalidDescriptor);
    assert_eq!(AmmStatus::from(&ManagerError::Registry(RegistryError::TooManyMods)), AmmStatus::TooManyMods);
    assert_eq!(AmmStatus::from(&ManagerError::UnknownMod), AmmStatus::UnknownHandle);
    assert_eq!(AmmStatus::from(&ManagerError::NotPermitted), AmmStatus::NotPermitted);
    assert_eq!(AmmStatus::from(&ManagerError::Setting { path: "a.b".into() }), AmmStatus::InvalidArgument);
}

#[test]
fn a_missing_manager_status_becomes_a_missing_manager_link_error() {
    assert_eq!(LinkError::from(AmmStatus::ManagerMissing), LinkError::ManagerMissing);
}

#[test]
fn any_other_status_keeps_its_code_in_the_link_error() {
    assert_eq!(LinkError::from(AmmStatus::QueueFull), LinkError::Status(8));
    assert_eq!(LinkError::from(AmmStatus::Panic), LinkError::Status(9));
}

#[test]
fn a_status_integer_converts_back_and_an_unknown_one_is_rejected() {
    assert_eq!(AmmStatus::try_from(8), Ok(AmmStatus::QueueFull));
    assert_eq!(AmmStatus::try_from(11), Ok(AmmStatus::ManagerMissing));
    assert_eq!(AmmStatus::try_from(12), Ok(AmmStatus::Busy));
    assert_eq!(AmmStatus::try_from(13), Err(13));
    assert_eq!(AmmStatus::try_from(-1), Err(-1));
}

#[test]
fn capture_errors_map_to_their_status_and_busy_has_its_own() {
    assert_eq!(AmmStatus::from(&CaptureError::UnknownMod), AmmStatus::UnknownHandle);
    assert_eq!(AmmStatus::from(&CaptureError::NotPermitted), AmmStatus::NotPermitted);
    assert_eq!(AmmStatus::from(&CaptureError::InvalidPath), AmmStatus::InvalidArgument);
    assert_eq!(AmmStatus::from(&CaptureError::Busy), AmmStatus::Busy);
    assert_eq!(LinkError::from(AmmStatus::Busy), LinkError::Status(12));
}
