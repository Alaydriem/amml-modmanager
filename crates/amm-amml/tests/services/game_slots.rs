use amml_modmanager::services::GameSlots;
use amml_modmanager::types::GameSlot;
use amml_sdk::SdkError;

use crate::support::fake_hook_host::FakeHookHost;

// every entry, valued by its position
fn entries_without(missing: &[GameSlot]) -> Vec<(&'static str, u64)> {
    GameSlot::ALL
        .iter()
        .enumerate()
        .filter(|(_, slot)| !missing.contains(slot))
        .map(|(index, slot)| (slot.entry(), 100 + index as u64))
        .collect()
}

#[test]
fn a_missing_get_options_logs_one_line_and_only_hide_hud_is_off() {
    let host = FakeHookHost::new(&entries_without(&[GameSlot::Options]), &[]);
    let slots = GameSlots::new();

    slots.resolve(&host).unwrap();

    let warnings = host.warnings.lock().unwrap().clone();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].starts_with("ClientInstance.getOptions") && warnings[0].ends_with("hide HUD is off"));
    assert_eq!(slots.get(GameSlot::Options), None);
    assert!(!slots.all(&[GameSlot::Options, GameSlot::SetHideGui, GameSlot::GetHideHud]));
    assert_eq!(slots.get(GameSlot::TopScreenName), Some(105));
}

#[test]
fn a_missing_required_slot_fails_and_keeps_nothing() {
    let host = FakeHookHost::new(&entries_without(&[GameSlot::ShowingMenu]), &[]);
    let slots = GameSlots::new();

    let result = slots.resolve(&host);

    assert!(matches!(result, Err(SdkError::MissingEntry(id)) if id == "ClientInstance.isShowingMenu"));
    assert_eq!(slots.get(GameSlot::ReleaseMouse), None);
}

#[test]
fn every_optional_slot_names_its_feature() {
    let optional = [GameSlot::TopScreenName, GameSlot::Options, GameSlot::SetHideGui, GameSlot::GetHideHud];
    let host = FakeHookHost::new(&entries_without(&optional), &[]);

    GameSlots::new().resolve(&host).unwrap();

    assert_eq!(host.warnings.lock().unwrap().len(), optional.len());
}
