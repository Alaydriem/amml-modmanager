use amm_bench::fixtures::EightMods;

#[test]
fn eight_mods_register_and_a_bound_key_reaches_a_keybind() {
    let mut mods = EightMods::new();

    let pressed = mods.route_key(EightMods::VK_B, true, 0);
    let released = mods.route_key(EightMods::VK_B, false, 1);

    assert_eq!(mods.mod_count(), 8);
    assert!(pressed.keybind.is_some(), "Ctrl+B is a freecam keybind and WorldDraw is shared");
    assert!(released.keybind.is_some());
}

#[test]
fn an_unbound_key_reaches_no_keybind() {
    let mut mods = EightMods::new();

    let decision = mods.route_key(EightMods::VK_Q, true, 0);

    assert!(decision.keybind.is_none());
}

#[test]
fn releasing_and_reacquiring_the_overlay_changes_the_layer_count() {
    let mut mods = EightMods::new();
    let before = mods.snapshot().layers.len();

    let released = mods.change_then_snapshot(0).layers.len();
    let acquired = mods.change_then_snapshot(1).layers.len();

    assert_eq!((released + 1, acquired), (before, before));
}
