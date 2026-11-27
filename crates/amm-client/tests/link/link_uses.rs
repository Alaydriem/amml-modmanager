use amm_client::link::LinkUses;
use amm_core::errors::LinkError;
use amml_api::amml_abi::{
    InjectedInputReleased, InputLeaseLost, InputLeaseResumed, InputLeaseSuspended, ModsLoaded, ProviderGone,
    ProviderReady, ScreenshotTaken, ScreenshotWritten,
};
use amml_api::{Area, Uses};

const FULL: Uses = amml_api::amml_uses! {
    feature "manager" {
        required ProviderReady;
        required ProviderGone;
        required area Area::INPUT_LEASE;
        optional ModsLoaded;
        optional InputLeaseSuspended;
        optional InputLeaseResumed;
        optional InputLeaseLost;
        optional InjectedInputReleased;
        optional ScreenshotTaken;
        optional ScreenshotWritten;
    }
};

const NO_GONE: Uses = amml_api::amml_uses! {
    feature "manager" {
        required ProviderReady;
        required area Area::INPUT_LEASE;
        optional ModsLoaded;
        optional InputLeaseSuspended;
        optional InputLeaseResumed;
        optional InputLeaseLost;
        optional InjectedInputReleased;
        optional ScreenshotTaken;
        optional ScreenshotWritten;
    }
};

const READY_OPTIONAL: Uses = amml_api::amml_uses! {
    feature "manager" {
        optional ProviderReady;
        required ProviderGone;
        required area Area::INPUT_LEASE;
        optional ModsLoaded;
        optional InputLeaseSuspended;
        optional InputLeaseResumed;
        optional InputLeaseLost;
        optional InjectedInputReleased;
        optional ScreenshotTaken;
        optional ScreenshotWritten;
    }
};

const NO_WRITTEN: Uses = amml_api::amml_uses! {
    feature "manager" {
        required ProviderReady;
        required ProviderGone;
        required area Area::INPUT_LEASE;
        optional ModsLoaded;
        optional InputLeaseSuspended;
        optional InputLeaseResumed;
        optional InputLeaseLost;
        optional InjectedInputReleased;
        optional ScreenshotTaken;
    }
};

#[test]
fn link_uses_names_a_missing_row() {
    assert_eq!(LinkUses::check(&FULL), Ok(()));
    assert_eq!(LinkUses::check(&NO_GONE), Err(LinkError::MissingUse("registry.provider_gone".to_owned())));
    assert_eq!(LinkUses::check(&NO_WRITTEN), Err(LinkError::MissingUse("overlay.screenshot_written".to_owned())));
}

#[test]
fn a_required_row_written_optional_is_missing() {
    assert_eq!(LinkUses::check(&READY_OPTIONAL), Err(LinkError::MissingUse("registry.provider_ready".to_owned())));
}

const LEASE_OPTIONAL: Uses = amml_api::amml_uses! {
    feature "manager" {
        required ProviderReady;
        required ProviderGone;
        optional area Area::INPUT_LEASE;
        optional ModsLoaded;
        optional InputLeaseSuspended;
        optional InputLeaseResumed;
        optional InputLeaseLost;
        optional InjectedInputReleased;
        optional ScreenshotTaken;
        optional ScreenshotWritten;
    }
};

#[test]
fn a_mod_that_claims_no_input_may_list_the_lease_area_as_optional() {
    assert_eq!(LinkUses::check(&LEASE_OPTIONAL), Ok(()));
}
