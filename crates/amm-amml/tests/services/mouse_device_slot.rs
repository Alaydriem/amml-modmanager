use amml_modmanager::services::MouseDeviceSlot;

use crate::support::fake_hook_host::FakeHookHost;

const DEVICE: u64 = 0x7ff6_2000_0010;

#[test]
fn the_entry_gives_the_mouse_device() {
    let host = FakeHookHost::new(&[(MouseDeviceSlot::ENTRY, DEVICE)], &[]);

    assert_eq!(MouseDeviceSlot::resolve(&host), Some(DEVICE as usize));
    assert!(host.warnings.lock().unwrap().is_empty());
}

#[test]
fn a_missing_entry_leaves_fake_buttons_to_the_first_real_mouse_event_with_one_line() {
    let host = FakeHookHost::new(&[], &[]);

    assert_eq!(MouseDeviceSlot::resolve(&host), None);
    let warnings = host.warnings.lock().unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains(MouseDeviceSlot::ENTRY), "{}", warnings[0]);
}

#[test]
fn an_entry_of_0_gives_no_device() {
    let host = FakeHookHost::new(&[(MouseDeviceSlot::ENTRY, 0)], &[]);

    assert_eq!(MouseDeviceSlot::resolve(&host), None);
    assert_eq!(host.warnings.lock().unwrap().len(), 1);
}

#[test]
fn the_entry_is_in_the_entry_list() {
    let list = include_str!("../../packaging/amml.hcl");

    assert!(list.contains(&format!("\"{}\"", MouseDeviceSlot::ENTRY)));
}
