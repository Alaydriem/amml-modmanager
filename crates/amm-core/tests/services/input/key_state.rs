use amm_core::services::input::KeyState;
use amm_core::structs::input::{InputEvent, Modifiers};

const NO_MODS: Modifiers = Modifiers { ctrl: false, shift: false, alt: false };
const KEY_W: u32 = 0x57;
const KEY_G: u32 = 0x47;
const VK_LBUTTON: u32 = 0x01;

fn key(vk: u32, down: bool) -> InputEvent {
    InputEvent::Key { vk, down, repeat: false, mods: NO_MODS }
}

#[test]
fn key_state_follows_downs_and_ups_of_keys_and_buttons() {
    let mut keys = KeyState::new();

    keys.apply(&key(KEY_W, true));
    keys.apply(&key(KEY_G, true));
    keys.apply(&key(KEY_G, false));
    keys.apply(&InputEvent::PointerButton { button: 0, down: true, x: 1.0, y: 2.0 });
    keys.apply(&key(0x1_0000, true));

    assert!(keys.is_down(KEY_W));
    assert!(!keys.is_down(KEY_G));
    assert!(keys.is_down(VK_LBUTTON));
    assert!(!keys.is_down(0x1_0000));
}

#[test]
fn focus_lost_clears_every_key() {
    let mut keys = KeyState::new();
    keys.apply(&key(KEY_W, true));
    keys.apply(&key(KEY_G, true));
    keys.apply(&InputEvent::PointerButton { button: 1, down: true, x: 0.0, y: 0.0 });

    keys.apply(&InputEvent::FocusLost);

    assert!(!keys.is_down(KEY_W));
    assert!(!keys.is_down(KEY_G));
    assert!(!keys.is_down(0x02));

    keys.apply(&key(KEY_W, true));
    keys.clear();
    assert!(!keys.is_down(KEY_W));
}
