use amm_core::services::egui_bridge::KeyMap;
use egui::Key;

#[test]
fn safe_key_map_does_not_map_space_or_enter() {
    for vk in [0x20, 0x0D, 0x09, 0x25, 0x26, 0x27, 0x28] {
        assert_eq!(KeyMap::to_egui_safe(vk), None, "vk {vk:#x}");
    }
    assert_eq!(KeyMap::to_egui(0x20), Some(Key::Space));
    assert_eq!(KeyMap::to_egui(0x0D), Some(Key::Enter));
    assert_eq!(KeyMap::to_egui_safe(0x41), Some(Key::A));
}
