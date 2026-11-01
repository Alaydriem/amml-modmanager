use amm_core::structs::descriptor::Chord;
use amm_core::structs::input::Modifiers;
use amm_core::structs::registry::ModIndex;
use amm_ui::model::{PanelAction, PanelController, PanelPage, RebindTarget};

const ESCAPE: u32 = 0x1B;
const CONTROL: u32 = 0x11;
const LEFT_CONTROL: u32 = 0xA2;
const LEFT_SHIFT: u32 = 0xA0;
const KEY_P: u32 = 0x50;
const KEY_F9: u32 = 0x78;
const KEY_G: u32 = 0x47;
const F3: u32 = 0x72;
const NO_MODS: Modifiers = Modifiers { ctrl: false, shift: false, alt: false };
const CTRL: Modifiers = Modifiers { ctrl: true, shift: false, alt: false };
const CTRL_SHIFT: Modifiers = Modifiers { ctrl: true, shift: true, alt: false };
const FREECAM: ModIndex = ModIndex(1);

fn bind(mod_index: ModIndex, action: u16) -> RebindTarget {
    RebindTarget::Mod { mod_index, action }
}

#[test]
fn close_closes_at_once_and_drops_a_waiting_rebind() {
    let mut controller = PanelController::new();
    controller.begin_rebind(bind(FREECAM, 0));

    let result = controller.on_action(&PanelAction::Close);

    assert_eq!(result, Some(PanelAction::Close));
    assert_eq!(controller.rebinding(), None);
}

#[test]
fn escape_cancels_a_rebind() {
    let mut controller = PanelController::new();
    controller.begin_rebind(bind(FREECAM, 0));

    let result = controller.on_key(ESCAPE, true, NO_MODS);

    assert_eq!(result, None);
    assert_eq!(controller.rebinding(), None);
}

#[test]
fn the_next_key_down_with_modifiers_is_the_new_chord() {
    let mut controller = PanelController::new();
    controller.begin_rebind(bind(FREECAM, 3));

    let modifier = controller.on_key(LEFT_CONTROL, true, CTRL);
    let result = controller.on_key(KEY_P, true, CTRL);

    let chord = Chord { key: KEY_P, ctrl: true, shift: false, alt: false, f3: false };
    assert_eq!(modifier, None);
    assert_eq!(result, Some(PanelAction::Rebind { mod_index: FREECAM, action: 3, chord }));
    assert_eq!(controller.rebinding(), None);
}

#[test]
fn a_rebind_of_the_panel_toggle_gives_the_toggle_action() {
    let mut controller = PanelController::new();
    controller.begin_rebind(RebindTarget::ManagerToggle);

    controller.on_key(LEFT_CONTROL, true, CTRL);
    controller.on_key(LEFT_SHIFT, true, CTRL_SHIFT);
    let result = controller.on_key(KEY_F9, true, CTRL_SHIFT);

    let chord = Chord { key: KEY_F9, ctrl: true, shift: true, alt: false, f3: false };
    assert_eq!(result, Some(PanelAction::RebindToggle(chord)));
}

#[test]
fn a_modifier_pressed_and_released_alone_is_its_own_chord() {
    let mut controller = PanelController::new();
    controller.begin_rebind(bind(FREECAM, 11));

    controller.on_key(LEFT_CONTROL, true, CTRL);
    let result = controller.on_key(LEFT_CONTROL, false, NO_MODS);

    let chord = Chord { key: CONTROL, ctrl: false, shift: false, alt: false, f3: false };
    assert_eq!(result, Some(PanelAction::Rebind { mod_index: FREECAM, action: 11, chord }));
}

#[test]
fn keys_without_a_rebind_produce_no_action() {
    let mut controller = PanelController::new();

    let result = controller.on_key(KEY_P, true, CTRL);

    assert_eq!(result, None);
}

#[test]
fn showing_a_page_drops_a_waiting_rebind() {
    let mut controller = PanelController::new();
    controller.begin_rebind(bind(FREECAM, 0));

    controller.show_page(PanelPage::Keybinds);

    assert_eq!(controller.page(), Some(PanelPage::Keybinds));
    assert_eq!(controller.rebinding(), None);
}

#[test]
fn reset_drops_a_rebind() {
    let mut controller = PanelController::new();
    controller.begin_rebind(bind(FREECAM, 0));

    controller.reset();

    assert_eq!(controller.rebinding(), None);
    assert_eq!(controller.on_action(&PanelAction::Close), Some(PanelAction::Close));
}

const LEFT_ALT: u32 = 0xA4;
const KEY_X: u32 = 0x58;
const SHIFT: Modifiers = Modifiers { ctrl: false, shift: true, alt: false };

#[test]
fn rebind_commits_ctrl_shift_x_not_shift() {
    let mut controller = PanelController::new();
    controller.begin_rebind(bind(FREECAM, 2));

    let ctrl = controller.on_key(LEFT_CONTROL, true, CTRL);
    let shift = controller.on_key(LEFT_SHIFT, true, CTRL_SHIFT);
    let shift_up = controller.on_key(LEFT_SHIFT, false, CTRL);
    let shift_again = controller.on_key(LEFT_SHIFT, true, CTRL_SHIFT);
    let x = controller.on_key(KEY_X, true, CTRL_SHIFT);

    let chord = Chord { key: KEY_X, ctrl: true, shift: true, alt: false, f3: false };
    assert_eq!((ctrl, shift, shift_up, shift_again), (None, None, None, None));
    assert_eq!(x, Some(PanelAction::Rebind { mod_index: FREECAM, action: 2, chord }));
}

#[test]
fn rebind_commits_a_modifier_alone_only_on_its_release() {
    let mut mixed = PanelController::new();
    mixed.begin_rebind(bind(FREECAM, 0));
    let mut alone = PanelController::new();
    alone.begin_rebind(bind(FREECAM, 0));

    mixed.on_key(LEFT_CONTROL, true, CTRL);
    mixed.on_key(LEFT_SHIFT, true, CTRL_SHIFT);
    let after_mixed = [mixed.on_key(LEFT_SHIFT, false, CTRL), mixed.on_key(LEFT_CONTROL, false, NO_MODS)];
    let down = alone.on_key(LEFT_ALT, true, Modifiers { alt: true, ..NO_MODS });
    let up = alone.on_key(LEFT_ALT, false, NO_MODS);

    assert_eq!(after_mixed, [None, None]);
    assert!(mixed.rebinding().is_some(), "a modifier released after another key does not commit");
    assert_eq!(down, None);
    let chord = Chord { key: 0x12, ctrl: false, shift: false, alt: false, f3: false };
    assert_eq!(up, Some(PanelAction::Rebind { mod_index: FREECAM, action: 0, chord }));
}

#[test]
fn rebind_preview_shows_held_modifiers() {
    let mut controller = PanelController::new();
    controller.begin_rebind(bind(FREECAM, 0));
    let before = controller.rebind_preview();

    controller.on_key(LEFT_SHIFT, true, SHIFT);
    controller.on_key(LEFT_CONTROL, true, CTRL_SHIFT);
    let held = controller.rebind_preview();
    controller.on_key(LEFT_CONTROL, false, SHIFT);
    let shift_only = controller.rebind_preview();

    assert_eq!(before, None);
    assert_eq!(held.as_deref(), Some("Ctrl+Shift+\u{2026}"));
    assert_eq!(shift_only.as_deref(), Some("Shift+\u{2026}"));
}

fn f3_chord(key: u32, ctrl: bool) -> Chord {
    Chord { key, ctrl, shift: false, alt: false, f3: true }
}

#[test]
fn f3_then_a_key_is_an_f3_chord() {
    let mut controller = PanelController::new();
    controller.begin_rebind(bind(FREECAM, 3));

    let held = controller.on_key(F3, true, NO_MODS);
    let result = controller.on_key(KEY_G, true, NO_MODS);

    assert_eq!(held, None);
    assert_eq!(result, Some(PanelAction::Rebind { mod_index: FREECAM, action: 3, chord: f3_chord(KEY_G, false) }));
}

#[test]
fn ctrl_f3_then_a_key_keeps_both() {
    let mut controller = PanelController::new();
    controller.begin_rebind(bind(FREECAM, 3));

    controller.on_key(LEFT_CONTROL, true, CTRL);
    controller.on_key(F3, true, CTRL);
    let result = controller.on_key(KEY_G, true, CTRL);

    assert_eq!(result, Some(PanelAction::Rebind { mod_index: FREECAM, action: 3, chord: f3_chord(KEY_G, true) }));
}

#[test]
fn f3_alone_does_not_end_a_rebind() {
    let mut controller = PanelController::new();
    controller.begin_rebind(bind(FREECAM, 3));

    let down = controller.on_key(F3, true, NO_MODS);
    let preview = controller.rebind_preview();
    let up = controller.on_key(F3, false, NO_MODS);

    assert_eq!(down, None);
    assert_eq!(up, None);
    assert_eq!(preview, Some("F3+\u{2026}".into()));
    assert_eq!(controller.rebind_preview(), None);
    assert_eq!(controller.rebinding(), Some(bind(FREECAM, 3)));
}

#[test]
fn rebind_preview_orders_f3_after_alt() {
    let mut controller = PanelController::new();
    controller.begin_rebind(bind(FREECAM, 3));

    controller.on_key(LEFT_CONTROL, true, CTRL);
    controller.on_key(F3, true, CTRL);

    assert_eq!(controller.rebind_preview(), Some("Ctrl+F3+\u{2026}".into()));
}
