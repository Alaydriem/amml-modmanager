use amm_core::config::ResolutionBook;
use amm_core::services::descriptor::DescriptorChecker;
use amm_core::services::event::EventHub;
use amm_core::services::input::{ChordMatcher, InputRouter};
use amm_core::services::lease::LeaseArbiter;
use amm_core::services::registry::ModRegistry;
use amm_core::structs::descriptor::{Capability, Chord, Claim, ClaimMode, Descriptor};
use amm_core::structs::event::KeybindPhase;
use amm_core::structs::input::{InputEvent, Modifiers, RouteContext, RouteDecision};
use amm_core::structs::registry::{ModIndex, ModMask};
use amm_core::structs::toast::ToastKind;

const FREECAM: &str = include_str!("../../../fixtures/four_mods/freecam.hcl");
const DEJAVU: &str = include_str!("../../../fixtures/four_mods/dejavu.hcl");
const MINIMAP: &str = include_str!("../../../fixtures/four_mods/minimap.hcl");

const NO_MODS: Modifiers = Modifiers { ctrl: false, shift: false, alt: false };
const CTRL: Modifiers = Modifiers { ctrl: true, shift: false, alt: false };
const CTRL_SHIFT: Modifiers = Modifiers { ctrl: true, shift: true, alt: false };
const TOGGLE_CHORD: Chord = Chord { key: 0x4D, ctrl: true, shift: true, alt: false, f3: false };

const VK_CONTROL: u32 = 0x11;
const VK_ESCAPE: u32 = 0x1B;
const VK_F1: u32 = 0x70;
const VK_F3: u32 = 0x72;
const KEY_E: u32 = 0x45;
const KEY_G: u32 = 0x47;
const KEY_L: u32 = 0x4C;
const KEY_M: u32 = 0x4D;
const KEY_P: u32 = 0x50;
const KEY_W: u32 = 0x57;
const KEY_Z: u32 = 0x5A;
const VK_DELETE: u32 = 0x2E;

struct Rig {
    registry: ModRegistry,
    book: ResolutionBook,
    hub: EventHub,
    arbiter: LeaseArbiter,
    matcher: ChordMatcher,
    router: InputRouter,
    panel_open: bool,
    keyboard_owner: Option<ModIndex>,
    pointer_owner: Option<ModIndex>,
    over_game_viewport: bool,
}

impl Rig {
    fn new(descriptors: Vec<Descriptor>) -> (Rig, Vec<ModIndex>) {
        let mut registry = ModRegistry::new(16);
        let ids: Vec<ModIndex> =
            descriptors.into_iter().filter_map(|descriptor| registry.register(descriptor, true).ok()).collect();
        let book = ResolutionBook::new();
        let mut arbiter = LeaseArbiter::new();
        arbiter.refresh_blocked(&registry, &book);
        let mut chords = Vec::new();
        for registered in registry.iter() {
            for (position, keybind) in registered.descriptor.keybinds.iter().enumerate() {
                chords.push((registered.index, position as u16, keybind.default_chord));
            }
        }
        let mut matcher = ChordMatcher::new(256);
        matcher.rebuild(&chords);
        let rig = Rig {
            registry,
            book,
            hub: EventHub::new(16, 256),
            arbiter,
            matcher,
            router: InputRouter::new(16),
            panel_open: false,
            keyboard_owner: None,
            pointer_owner: None,
            over_game_viewport: false,
        };
        (rig, ids)
    }

    fn acquire(&mut self, who: ModIndex, capability: Capability) {
        self.arbiter.acquire(&self.registry, &self.book, who, capability, &mut self.hub);
    }

    fn action(&self, who: ModIndex, name: &str) -> u16 {
        let keybinds =
            self.registry.get(who).map(|registered| registered.descriptor.keybinds.as_slice()).unwrap_or(&[]);
        let position = keybinds.iter().position(|keybind| keybind.action == name);
        position.unwrap_or_else(|| panic!("no keybind {name}")) as u16
    }

    // Replaces the matcher with exactly these binds, so a test can bind any key to any declared action.
    fn rebind(&mut self, binds: &[(ModIndex, &str, Chord)]) {
        let chords: Vec<(ModIndex, u16, Chord)> =
            binds.iter().map(|(who, name, chord)| (*who, self.action(*who, name), *chord)).collect();
        self.matcher.rebuild(&chords);
    }

    fn route(&mut self, event: InputEvent) -> RouteDecision {
        let ctx = RouteContext {
            panel_open: self.panel_open,
            toggle_chord: TOGGLE_CHORD,
            pointer_owner: self.pointer_owner,
            keyboard_owner: self.keyboard_owner,
            over_menu_button: false,
            over_game_viewport: self.over_game_viewport,
            registry: &self.registry,
            arbiter: &self.arbiter,
            book: &self.book,
        };
        self.router.route(&event, &ctx, &self.matcher)
    }
}

fn parse(text: &str) -> Descriptor {
    DescriptorChecker::new(128, 256).parse(text).unwrap_or_else(|error| panic!("fixture does not parse: {error}"))
}

fn four_mods() -> (Rig, ModIndex, ModIndex, ModIndex) {
    let (rig, ids) = Rig::new(vec![parse(FREECAM), parse(DEJAVU), parse(MINIMAP)]);
    (rig, ids[0], ids[1], ids[2])
}

fn editor() -> Descriptor {
    input_mod("editor", ClaimMode::Exclusive, false, false)
}

fn input_mod(id: &str, mode: ClaimMode, pass_pointer_motion: bool, block_player_turn: bool) -> Descriptor {
    Descriptor {
        namespace: "test".into(),
        name: id.into(),
        display_name: "Editor".into(),
        version: "0.1.0".into(),
        author: "test".into(),
        config_path: "config.hcl".into(),
        texture_budget_mb: None,
        texture_budget_count: None,
        claims: vec![Claim {
            capability: Capability::Input,
            mode,
            reason: "test".into(),
            hide_vanilla_hud: false,
            block_player_turn,
            pass_pointer_motion,
        }],
        keybinds: Vec::new(),
        input_passthrough: Vec::new(),
        settings: Vec::new(),
    }
}

fn key(vk: u32, down: bool, mods: Modifiers) -> InputEvent {
    InputEvent::Key { vk, down, repeat: false, mods }
}

fn repeat(vk: u32, mods: Modifiers) -> InputEvent {
    InputEvent::Key { vk, down: true, repeat: true, mods }
}

fn mask(ids: &[ModIndex]) -> ModMask {
    let mut mask = ModMask::default();
    for id in ids {
        mask.insert(*id);
    }
    mask
}

#[test]
fn panel_open_takes_every_event_and_consumes_it() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.acquire(freecam, Capability::Input);
    rig.panel_open = true;
    let events = [
        key(KEY_W, true, NO_MODS),
        key(KEY_G, true, NO_MODS),
        key(KEY_P, true, CTRL),
        InputEvent::PointerMove { x: 4.0, y: 5.0 },
        InputEvent::PointerButton { button: 0, down: true, x: 4.0, y: 5.0 },
        InputEvent::PointerDelta { dx: 1.0, dy: 1.0 },
        InputEvent::Wheel { delta: 1.0 },
        InputEvent::Char(u32::from('a')),
    ];

    for event in events {
        let decision = rig.route(event);

        assert!(decision.to_panel, "{event:?}");
        assert!(decision.consume_from_game, "{event:?}");
        assert!(!decision.toggle_panel, "{event:?}");
        assert_eq!(decision.keybind, None, "{event:?}");
        assert!(decision.raw_targets.is_empty(), "{event:?}");
        assert_eq!(decision.blocked, None, "{event:?}");
    }
}

#[test]
fn toggle_chord_toggles_the_panel_even_when_a_mod_holds_input() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.acquire(freecam, Capability::Input);

    let opened = rig.route(key(KEY_M, true, CTRL_SHIFT));

    assert!(opened.toggle_panel);
    assert!(opened.consume_from_game);
    assert!(opened.raw_targets.is_empty());
    assert_eq!(opened.keybind, None);

    rig.route(key(KEY_M, false, CTRL_SHIFT));
    rig.panel_open = true;
    let closed = rig.route(key(KEY_M, true, CTRL_SHIFT));

    assert!(closed.toggle_panel);
    assert!(closed.consume_from_game);
}

#[test]
fn global_toggle_bind_reaches_its_mod_while_freecam_holds_shared_input() {
    let (mut rig, freecam, _, minimap) = four_mods();
    rig.acquire(freecam, Capability::Input);

    let world_map = rig.route(key(KEY_M, true, NO_MODS));

    let toggle_world_map = rig.action(minimap, "toggle_world_map");
    assert_eq!(world_map.keybind, Some((minimap, toggle_world_map, KeybindPhase::Pressed)));
    assert!(world_map.consume_from_game);
    assert!(world_map.raw_targets.is_empty());

    let released = rig.route(key(KEY_M, false, NO_MODS));
    assert_eq!(released.keybind, Some((minimap, toggle_world_map, KeybindPhase::Released)));
    assert!(released.raw_targets.is_empty());

    let toggle_input = rig.route(key(KEY_E, true, CTRL));
    assert_eq!(toggle_input.keybind, Some((freecam, rig.action(freecam, "toggle_input"), KeybindPhase::Pressed)));
}

#[test]
fn while_held_bind_is_ignored_until_the_mod_holds_its_capability() {
    let (mut rig, freecam, _, _) = four_mods();

    let ignored = rig.route(key(KEY_W, true, NO_MODS));
    rig.route(key(KEY_W, false, NO_MODS));

    assert_eq!(ignored.keybind, None);
    assert_eq!(ignored.blocked, None);
    assert!(!ignored.consume_from_game);
    assert!(ignored.raw_targets.is_empty());

    rig.acquire(freecam, Capability::Input);
    let forward = rig.route(key(KEY_W, true, NO_MODS));
    let held = rig.route(repeat(KEY_W, NO_MODS));
    let fast = rig.route(key(VK_CONTROL, true, CTRL));

    let fly_forward = rig.action(freecam, "fly_forward");
    assert_eq!(forward.keybind, Some((freecam, fly_forward, KeybindPhase::Pressed)));
    assert!(forward.consume_from_game);
    assert_eq!(held.keybind, Some((freecam, fly_forward, KeybindPhase::Repeat)));
    assert_eq!(fast.keybind, Some((freecam, rig.action(freecam, "fly_fast"), KeybindPhase::Pressed)));
}

#[test]
fn bind_blocked_by_exclusive_camera_is_consumed_and_names_the_holder() {
    let (mut rig, freecam, dejavu, _) = four_mods();
    rig.acquire(dejavu, Capability::Camera);

    let decision = rig.route(key(KEY_P, true, CTRL));

    let toggle = rig.action(freecam, "toggle");
    assert_eq!(decision.keybind, None);
    assert_eq!(
        decision.blocked,
        Some(ToastKind::Blocked { capability: Capability::Camera, holder: dejavu, mod_index: freecam, action: toggle })
    );
    assert!(decision.consume_from_game);
    assert!(decision.raw_targets.is_empty());

    let held = rig.route(repeat(KEY_P, CTRL));
    let released = rig.route(key(KEY_P, false, CTRL));
    assert_eq!(held.blocked, None);
    assert!(held.consume_from_game);
    assert_eq!(released.keybind, None);
    assert!(released.consume_from_game);
}

#[test]
fn raw_key_goes_to_every_input_holder_and_is_consumed() {
    let (mut rig, freecam, _, minimap) = four_mods();
    rig.acquire(freecam, Capability::Input);
    rig.acquire(minimap, Capability::Input);

    let down = rig.route(key(KEY_G, true, NO_MODS));
    let look = rig.route(InputEvent::PointerDelta { dx: 3.0, dy: -2.0 });
    let up = rig.route(key(KEY_G, false, NO_MODS));

    assert_eq!(down.raw_targets, mask(&[freecam, minimap]));
    assert!(down.consume_from_game);
    assert_eq!(down.keybind, None);
    assert_eq!(look.raw_targets, mask(&[freecam, minimap]));
    assert!(look.consume_from_game);
    assert_eq!(up.raw_targets, mask(&[freecam, minimap]));
    assert!(up.consume_from_game);
}

#[test]
fn passthrough_key_reaches_the_game_only_if_every_holder_lists_it() {
    let second = Descriptor { input_passthrough: vec![VK_ESCAPE], ..parse(MINIMAP) };
    let (mut rig, ids) = Rig::new(vec![parse(FREECAM), second]);
    let (freecam, minimap) = (ids[0], ids[1]);
    rig.acquire(freecam, Capability::Input);
    rig.acquire(minimap, Capability::Input);

    let f1 = rig.route(key(VK_F1, true, NO_MODS));
    let escape = rig.route(key(VK_ESCAPE, true, NO_MODS));
    let escape_up = rig.route(key(VK_ESCAPE, false, NO_MODS));

    assert_eq!(f1.raw_targets, mask(&[freecam, minimap]));
    assert!(f1.consume_from_game);
    assert_eq!(escape.raw_targets, mask(&[freecam, minimap]));
    assert!(!escape.consume_from_game);
    assert!(!escape_up.consume_from_game);

    let (mut alone, ids) = Rig::new(vec![parse(FREECAM)]);
    alone.acquire(ids[0], Capability::Input);
    assert!(!alone.route(key(VK_F1, true, NO_MODS)).consume_from_game);
}

#[test]
fn blocked_input_holder_gets_no_raw_events() {
    let (mut rig, ids) = Rig::new(vec![parse(FREECAM), editor()]);
    let (freecam, editor) = (ids[0], ids[1]);
    rig.acquire(freecam, Capability::Input);
    rig.acquire(editor, Capability::Input);

    let down = rig.route(key(KEY_G, true, NO_MODS));
    let button = rig.route(InputEvent::PointerButton { button: 1, down: true, x: 0.0, y: 0.0 });
    let wheel = rig.route(InputEvent::Wheel { delta: -1.0 });

    assert_eq!(down.raw_targets, mask(&[editor]));
    assert!(down.consume_from_game);
    assert_eq!(button.raw_targets, mask(&[editor]));
    assert_eq!(wheel.raw_targets, mask(&[editor]));
}

#[test]
fn key_up_reaches_the_mod_that_got_the_key_down_after_it_is_suspended() {
    let (mut rig, ids) = Rig::new(vec![parse(FREECAM), editor()]);
    let (freecam, editor) = (ids[0], ids[1]);
    rig.acquire(freecam, Capability::Input);
    let raw_down = rig.route(key(KEY_G, true, NO_MODS));
    let bind_down = rig.route(key(KEY_W, true, NO_MODS));
    assert_eq!(raw_down.raw_targets, mask(&[freecam]));

    rig.acquire(editor, Capability::Input);
    assert!(!rig.arbiter.holds(freecam, Capability::Input));
    let raw_up = rig.route(key(KEY_G, false, NO_MODS));
    let bind_up = rig.route(key(KEY_W, false, NO_MODS));

    let fly_forward = rig.action(freecam, "fly_forward");
    assert_eq!(bind_down.keybind, Some((freecam, fly_forward, KeybindPhase::Pressed)));
    assert_eq!(raw_up.raw_targets, mask(&[freecam]));
    assert!(raw_up.consume_from_game);
    assert_eq!(bind_up.keybind, Some((freecam, fly_forward, KeybindPhase::Released)));
    assert!(bind_up.raw_targets.is_empty());

    let next = rig.route(key(KEY_G, true, NO_MODS));
    assert_eq!(next.raw_targets, mask(&[editor]));
}

#[test]
fn keys_pressed_while_the_panel_is_open_never_reach_the_holder() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.acquire(freecam, Capability::Input);
    rig.panel_open = true;
    rig.route(key(KEY_G, true, NO_MODS));
    rig.route(key(KEY_W, true, NO_MODS));

    rig.panel_open = false;
    let raw_repeat = rig.route(repeat(KEY_G, NO_MODS));
    let bind_repeat = rig.route(repeat(KEY_W, NO_MODS));
    let raw_up = rig.route(key(KEY_G, false, NO_MODS));
    let bind_up = rig.route(key(KEY_W, false, NO_MODS));

    for decision in [raw_repeat, bind_repeat, raw_up, bind_up] {
        assert!(decision.raw_targets.is_empty());
        assert_eq!(decision.keybind, None);
        assert!(decision.consume_from_game);
        assert!(!decision.to_panel);
    }

    let fresh = rig.route(key(KEY_G, true, NO_MODS));
    assert_eq!(fresh.raw_targets, mask(&[freecam]));
}

struct Chords;

impl Chords {
    fn plain(key: u32) -> Chord {
        Chord { key, ctrl: false, shift: false, alt: false, f3: false }
    }

    fn ctrl(key: u32) -> Chord {
        Chord { key, ctrl: true, shift: false, alt: false, f3: false }
    }

    fn f3(key: u32) -> Chord {
        Chord { key, ctrl: false, shift: false, alt: false, f3: true }
    }
}

#[test]
fn a_letter_bound_by_the_text_owner_goes_to_its_field() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.rebind(&[(freecam, "toggle", Chords::plain(KEY_P))]);
    rig.keyboard_owner = Some(freecam);

    let down = rig.route(key(KEY_P, true, NO_MODS));
    let held = rig.route(repeat(KEY_P, NO_MODS));
    let up = rig.route(key(KEY_P, false, NO_MODS));

    for decision in [down, held, up] {
        assert_eq!(decision.raw_targets, mask(&[freecam]));
        assert_eq!(decision.keybind, None);
        assert!(decision.consume_from_game);
    }
}

#[test]
fn a_chord_bound_by_the_text_owner_goes_to_its_field() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.rebind(&[(freecam, "toggle", Chords::ctrl(KEY_Z)), (freecam, "clear_path", Chords::plain(VK_DELETE))]);
    rig.keyboard_owner = Some(freecam);

    let undo = rig.route(key(KEY_Z, true, CTRL));
    let delete = rig.route(key(VK_DELETE, true, NO_MODS));

    for decision in [undo, delete] {
        assert_eq!(decision.raw_targets, mask(&[freecam]));
        assert_eq!(decision.keybind, None);
        assert!(decision.consume_from_game);
    }
}

#[test]
fn a_keybind_of_another_mod_does_not_run_while_a_field_has_focus() {
    let (mut rig, _, dejavu, _) = four_mods();
    rig.keyboard_owner = Some(dejavu);

    let decision = rig.route(key(KEY_E, true, CTRL));

    assert_eq!(decision.keybind, None);
    assert_eq!(decision.raw_targets, mask(&[dejavu]));
    assert!(decision.consume_from_game);
}

#[test]
fn a_blocked_keybind_shows_no_toast_while_a_field_has_focus() {
    let (mut rig, _, dejavu, minimap) = four_mods();
    rig.acquire(dejavu, Capability::Camera);
    rig.keyboard_owner = Some(minimap);

    let decision = rig.route(key(KEY_P, true, CTRL));

    assert_eq!(decision.blocked, None);
    assert_eq!(decision.keybind, None);
    assert_eq!(decision.raw_targets, mask(&[minimap]));
}

#[test]
fn the_text_owner_gets_the_key_before_an_input_holder() {
    let (mut rig, freecam, dejavu, _) = four_mods();
    rig.acquire(freecam, Capability::Input);
    rig.keyboard_owner = Some(dejavu);

    let down = rig.route(key(KEY_G, true, NO_MODS));
    let up = rig.route(key(KEY_G, false, NO_MODS));

    assert_eq!(down.raw_targets, mask(&[dejavu]));
    assert_eq!(up.raw_targets, mask(&[dejavu]));
}

#[test]
fn the_manager_toggle_chord_still_opens_the_panel_while_a_field_has_focus() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.keyboard_owner = Some(freecam);

    let decision = rig.route(key(KEY_M, true, CTRL_SHIFT));

    assert!(decision.toggle_panel);
    assert!(decision.consume_from_game);
    assert!(decision.raw_targets.is_empty());
}

#[test]
fn the_open_panel_still_gets_the_key_while_a_field_has_focus() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.keyboard_owner = Some(freecam);
    rig.panel_open = true;

    let decision = rig.route(key(KEY_G, true, NO_MODS));

    assert!(decision.to_panel);
    assert!(decision.raw_targets.is_empty());
}

#[test]
fn a_keybind_pressed_before_the_field_got_focus_is_released_to_its_keybind() {
    let (mut rig, freecam, _, minimap) = four_mods();
    let toggle = rig.action(freecam, "toggle");
    let down = rig.route(key(KEY_P, true, CTRL));
    rig.keyboard_owner = Some(minimap);

    let up = rig.route(key(KEY_P, false, CTRL));

    assert_eq!(down.keybind, Some((freecam, toggle, KeybindPhase::Pressed)));
    assert_eq!(up.keybind, Some((freecam, toggle, KeybindPhase::Released)));
    assert!(up.raw_targets.is_empty());
}

#[test]
fn a_key_pressed_in_a_field_is_released_to_the_field_after_focus_goes() {
    let (mut rig, _, dejavu, _) = four_mods();
    rig.keyboard_owner = Some(dejavu);
    rig.route(key(KEY_G, true, NO_MODS));
    rig.keyboard_owner = None;

    let up = rig.route(key(KEY_G, false, NO_MODS));

    assert_eq!(up.raw_targets, mask(&[dejavu]));
    assert!(up.consume_from_game);
}

#[test]
fn keybinds_run_again_when_no_field_has_focus() {
    let (mut rig, freecam, dejavu, _) = four_mods();
    rig.keyboard_owner = Some(dejavu);
    let typed = rig.route(key(KEY_E, true, CTRL));
    rig.route(key(KEY_E, false, CTRL));
    rig.keyboard_owner = None;

    let after = rig.route(key(KEY_E, true, CTRL));

    assert_eq!(typed.keybind, None);
    assert_eq!(after.keybind, Some((freecam, rig.action(freecam, "toggle_input"), KeybindPhase::Pressed)));
}

#[test]
fn a_click_in_the_game_viewport_reaches_the_game_and_the_layer_owner() {
    let (mut rig, ids) = Rig::new(vec![parse(DEJAVU)]);
    rig.pointer_owner = Some(ids[0]);
    let click = InputEvent::PointerButton { button: 0, down: true, x: 300.0, y: 200.0 };

    let outside = rig.route(click);
    rig.over_game_viewport = true;
    let inside = rig.route(click);

    assert!(outside.consume_from_game);
    assert!(!inside.consume_from_game);
    assert!(inside.raw_targets.contains(ids[0]));
}

const MOTION: InputEvent = InputEvent::PointerDelta { dx: 3.0, dy: -2.0 };

#[test]
fn motion_is_consumed_for_an_input_holder_by_default() {
    let (mut rig, ids) = Rig::new(vec![input_mod("plain", ClaimMode::Shared, false, false)]);
    rig.acquire(ids[0], Capability::Input);

    let decision = rig.route(MOTION);

    assert!(decision.consume_from_game);
    assert_eq!(decision.raw_targets, mask(&[ids[0]]));
}

#[test]
fn motion_reaches_the_game_when_every_holder_passes_it() {
    let (mut rig, ids) = Rig::new(vec![input_mod("look", ClaimMode::Shared, true, false)]);
    rig.acquire(ids[0], Capability::Input);

    let decision = rig.route(MOTION);

    assert!(!decision.consume_from_game);
    assert_eq!(decision.raw_targets, mask(&[ids[0]]));
    assert!(!decision.to_panel);
}

#[test]
fn motion_is_consumed_when_any_holder_does_not_pass_it() {
    let (mut rig, ids) = Rig::new(vec![
        input_mod("look", ClaimMode::Shared, true, false),
        input_mod("plain", ClaimMode::Shared, false, false),
    ]);
    rig.acquire(ids[0], Capability::Input);
    rig.acquire(ids[1], Capability::Input);

    let decision = rig.route(MOTION);

    assert!(decision.consume_from_game);
    assert_eq!(decision.raw_targets, mask(&[ids[0], ids[1]]));
}

#[test]
fn wheel_and_buttons_stay_consumed_with_motion_passthrough() {
    let (mut rig, ids) = Rig::new(vec![input_mod("look", ClaimMode::Shared, true, false)]);
    rig.acquire(ids[0], Capability::Input);

    let wheel = rig.route(InputEvent::Wheel { delta: 1.0 });
    let button = rig.route(InputEvent::PointerButton { button: 1, down: true, x: 0.0, y: 0.0 });
    let moved = rig.route(InputEvent::PointerMove { x: 1.0, y: 1.0 });

    assert!(wheel.consume_from_game);
    assert_eq!(wheel.raw_targets, mask(&[ids[0]]));
    assert!(button.consume_from_game);
    assert_eq!(button.raw_targets, mask(&[ids[0]]));
    assert!(!moved.consume_from_game);
}

#[test]
fn the_panel_still_takes_motion() {
    let (mut rig, ids) = Rig::new(vec![input_mod("look", ClaimMode::Shared, true, false)]);
    rig.acquire(ids[0], Capability::Input);
    rig.panel_open = true;

    let decision = rig.route(MOTION);

    assert!(decision.to_panel);
    assert!(decision.consume_from_game);
    assert!(decision.raw_targets.is_empty());
}

#[test]
fn motion_passthrough_does_not_depend_on_block_player_turn() {
    let (mut rig, ids) = Rig::new(vec![input_mod("look", ClaimMode::Shared, true, true)]);
    rig.acquire(ids[0], Capability::Input);

    let decision = rig.route(MOTION);

    assert!(!decision.consume_from_game);
    assert_eq!(decision.raw_targets, mask(&[ids[0]]));
}

#[test]
fn dejavu_exclusive_input_alone_decides_motion() {
    let (mut rig, ids) = Rig::new(vec![
        input_mod("shared", ClaimMode::Shared, false, false),
        input_mod("dejavu", ClaimMode::Exclusive, true, false),
    ]);
    let (shared, dejavu) = (ids[0], ids[1]);
    rig.acquire(shared, Capability::Input);
    assert!(rig.route(MOTION).consume_from_game);

    rig.acquire(dejavu, Capability::Input);
    assert!(!rig.arbiter.holds(shared, Capability::Input));
    let decision = rig.route(MOTION);

    assert!(!decision.consume_from_game);
    assert_eq!(decision.raw_targets, mask(&[dejavu]));
}

fn assert_swallowed(decision: RouteDecision) {
    assert!(decision.consume_from_game, "{decision:?}");
    assert_eq!(decision.keybind, None, "{decision:?}");
    assert!(decision.raw_targets.is_empty(), "{decision:?}");
    assert!(!decision.to_panel, "{decision:?}");
    assert!(!decision.toggle_panel, "{decision:?}");
}

#[test]
fn f3_alone_reaches_nothing() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.rebind(&[(freecam, "toggle", Chords::f3(KEY_G))]);

    assert_swallowed(rig.route(key(VK_F3, true, NO_MODS)));
    assert_swallowed(rig.route(key(VK_F3, false, NO_MODS)));
}

#[test]
fn f3_then_keys_fire_each_f3_bind() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.rebind(&[(freecam, "toggle", Chords::f3(KEY_G)), (freecam, "clear_path", Chords::f3(KEY_L))]);
    let toggle = rig.action(freecam, "toggle");
    let clear_path = rig.action(freecam, "clear_path");

    let f3_down = rig.route(key(VK_F3, true, NO_MODS));
    let g_down = rig.route(key(KEY_G, true, NO_MODS));
    let g_up = rig.route(key(KEY_G, false, NO_MODS));
    let l_down = rig.route(key(KEY_L, true, NO_MODS));
    let l_up = rig.route(key(KEY_L, false, NO_MODS));
    let f3_up = rig.route(key(VK_F3, false, NO_MODS));

    assert_eq!(g_down.keybind, Some((freecam, toggle, KeybindPhase::Pressed)));
    assert_eq!(g_up.keybind, Some((freecam, toggle, KeybindPhase::Released)));
    assert_eq!(l_down.keybind, Some((freecam, clear_path, KeybindPhase::Pressed)));
    assert_eq!(l_up.keybind, Some((freecam, clear_path, KeybindPhase::Released)));
    for decision in [f3_down, g_down, g_up, l_down, l_up, f3_up] {
        assert!(decision.consume_from_game, "{decision:?}");
    }
}

#[test]
fn a_key_without_an_f3_bind_is_swallowed_while_f3_is_held() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.rebind(&[(freecam, "toggle", Chords::plain(KEY_G))]);
    rig.acquire(freecam, Capability::Input);

    rig.route(key(VK_F3, true, NO_MODS));

    assert_swallowed(rig.route(key(KEY_W, true, NO_MODS)));
    assert_swallowed(rig.route(repeat(KEY_W, NO_MODS)));
    assert_swallowed(rig.route(key(KEY_W, false, NO_MODS)));
    assert_swallowed(rig.route(key(KEY_G, true, NO_MODS)));
}

#[test]
fn plain_g_still_binds_after_f3_is_released() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.rebind(&[(freecam, "toggle", Chords::plain(KEY_G))]);
    let toggle = rig.action(freecam, "toggle");

    rig.route(key(VK_F3, true, NO_MODS));
    rig.route(key(VK_F3, false, NO_MODS));
    let g_down = rig.route(key(KEY_G, true, NO_MODS));

    assert_eq!(g_down.keybind, Some((freecam, toggle, KeybindPhase::Pressed)));
}

#[test]
fn f3_released_first_still_releases_the_bind() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.rebind(&[(freecam, "toggle", Chords::f3(KEY_G))]);
    let toggle = rig.action(freecam, "toggle");

    rig.route(key(VK_F3, true, NO_MODS));
    rig.route(key(KEY_G, true, NO_MODS));
    rig.route(key(VK_F3, false, NO_MODS));
    let g_up = rig.route(key(KEY_G, false, NO_MODS));

    assert_eq!(g_up.keybind, Some((freecam, toggle, KeybindPhase::Released)));
    assert!(g_up.consume_from_game);
}

#[test]
fn focus_lost_forgets_a_held_f3() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.rebind(&[(freecam, "toggle", Chords::plain(KEY_G))]);
    let toggle = rig.action(freecam, "toggle");

    rig.route(key(VK_F3, true, NO_MODS));
    rig.route(InputEvent::FocusLost);
    let g_down = rig.route(key(KEY_G, true, NO_MODS));

    assert_eq!(g_down.keybind, Some((freecam, toggle, KeybindPhase::Pressed)));
}

#[test]
fn text_owner_gets_f3_and_the_next_key_raw() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.rebind(&[(freecam, "toggle", Chords::f3(KEY_G))]);
    rig.keyboard_owner = Some(freecam);

    let f3_down = rig.route(key(VK_F3, true, NO_MODS));
    let g_down = rig.route(key(KEY_G, true, NO_MODS));

    for decision in [f3_down, g_down] {
        assert_eq!(decision.raw_targets, mask(&[freecam]));
        assert_eq!(decision.keybind, None);
    }
}

#[test]
fn f3_repeats_are_swallowed_and_g_still_binds() {
    let (mut rig, freecam, _, _) = four_mods();
    rig.rebind(&[(freecam, "toggle", Chords::f3(KEY_G))]);
    let toggle = rig.action(freecam, "toggle");

    rig.route(key(VK_F3, true, NO_MODS));
    assert_swallowed(rig.route(repeat(VK_F3, NO_MODS)));
    assert_swallowed(rig.route(repeat(VK_F3, NO_MODS)));
    let g_down = rig.route(key(KEY_G, true, NO_MODS));
    let g_held = rig.route(repeat(KEY_G, NO_MODS));

    assert_eq!(g_down.keybind, Some((freecam, toggle, KeybindPhase::Pressed)));
    assert_eq!(g_held.keybind, Some((freecam, toggle, KeybindPhase::Repeat)));
}

#[test]
fn toggle_chord_with_f3_held_does_not_toggle() {
    let (mut rig, _, _, _) = four_mods();

    rig.route(key(VK_F3, true, NO_MODS));

    assert_swallowed(rig.route(key(KEY_M, true, CTRL_SHIFT)));
}

#[test]
fn an_exclusive_input_holder_gets_f3_and_keys_held_with_it_raw() {
    let mut replay = editor();
    replay.input_passthrough = vec![VK_F3];
    let (mut rig, ids) = Rig::new(vec![parse(FREECAM), replay]);
    let (freecam, replay) = (ids[0], ids[1]);
    rig.rebind(&[(freecam, "toggle", Chords::f3(KEY_G))]);
    rig.acquire(replay, Capability::Input);

    let f3_down = rig.route(key(VK_F3, true, NO_MODS));
    let g_down = rig.route(key(KEY_G, true, NO_MODS));
    let g_up = rig.route(key(KEY_G, false, NO_MODS));
    let f3_up = rig.route(key(VK_F3, false, NO_MODS));

    for decision in [f3_down, g_down, g_up, f3_up] {
        assert_eq!(decision.raw_targets, mask(&[replay]), "{decision:?}");
        assert_eq!(decision.keybind, None, "{decision:?}");
    }
    assert!(!f3_down.consume_from_game);
    assert!(g_down.consume_from_game);
}
