use amm_abi::types::AmmCapability;
use amm_ffi::services::HostCell;
use amml_api::amml_abi::{AmmlCursorShape, AmmlMouseButton, AmmlPointerKind, WindowChar};
use amml_api::testing::FakeGame;

use crate::support::{Descriptors, ManagerRig, WarpSurface};

fn host_pointer() -> (f32, f32) {
    HostCell::global().with(|runtime| Ok(runtime.pointer())).unwrap()
}

fn close_rect() -> [f32; 4] {
    HostCell::global().with(|runtime| Ok(runtime.panel_close_rect())).unwrap().expect("the panel placed its close")
}

#[test]
fn a_cancelled_window_button_down_reaches_the_panel() {
    let rig = ManagerRig::new();
    rig.start();
    let surface = WarpSurface::new(1280, 720);
    rig.toggle_panel();
    rig.present(surface.surface(1));
    let [min_x, min_y, max_x, max_y] = close_rect();
    let at = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);

    let moved = rig.pointer(AmmlPointerKind::MOVE, AmmlMouseButton::LEFT, false, at, at);
    rig.present(surface.surface(1));
    let pressed = rig.pointer(AmmlPointerKind::BUTTON, AmmlMouseButton::LEFT, true, at, at);
    rig.present(surface.surface(1));
    rig.pointer(AmmlPointerKind::BUTTON, AmmlMouseButton::LEFT, false, at, at);
    rig.present(surface.surface(1));

    assert!(moved.cancelled);
    assert!(pressed.cancelled);
    // the panel saw the click on its close button and closed
    assert_eq!(rig.panel_open(), 0);
    rig.tick();
    assert_eq!(rig.log_count("info panel action action=Close"), 1);
}

#[test]
fn pointer_uses_buffer_coordinates_while_a_size_is_forced() {
    let rig = ManagerRig::new();
    rig.start();
    let consumer = rig.register(&Descriptors::overlay_and_size("consumer"));
    assert!(rig.acquire(consumer, AmmCapability::RenderSize));
    assert_eq!(rig.force_size(consumer, Some((640, 360))), amm_abi::types::AmmStatus::Ok);

    rig.pointer(AmmlPointerKind::MOVE, AmmlMouseButton::LEFT, false, (640.0, 360.0), (320.0, 180.0));

    assert_eq!(host_pointer(), (320.0, 180.0));
}

#[test]
fn a_button_passes_to_the_game_while_the_panel_is_closed() {
    let rig = ManagerRig::new();
    rig.start();

    let cancelled = rig.click((100.0, 100.0));

    assert!(!cancelled);
    assert_eq!(host_pointer(), (100.0, 100.0));
}

#[test]
fn a_character_goes_to_the_open_panel_and_not_the_game() {
    let rig = ManagerRig::new();
    rig.start();
    let mut typed = WindowChar { header: FakeGame::header::<WindowChar>(), ch: u32::from('q') };
    let closed = rig.fake.raise(&mut typed);

    rig.toggle_panel();
    let open = rig.fake.raise(&mut typed);

    assert!(!closed.cancelled);
    assert!(open.cancelled);
}

#[test]
fn focus_loss_is_held_while_the_panel_is_open_and_replayed_after() {
    let rig = ManagerRig::new();
    rig.start();
    rig.toggle_panel();

    let held = rig.focus(false, false);
    rig.tick();
    let while_open = rig.fake.window_requests();
    rig.toggle_panel();
    rig.tick();
    let replayed = rig.focus(false, true);
    let closed = rig.focus(false, false);

    assert!(held.cancelled);
    assert!(while_open.is_empty());
    assert_eq!(rig.fake.window_requests(), vec!["replay_focus_loss".to_string()]);
    assert!(!replayed.cancelled);
    assert!(!closed.cancelled);
}

#[test]
fn a_focus_gain_forgets_the_held_loss() {
    let rig = ManagerRig::new();
    rig.start();
    rig.toggle_panel();
    assert!(rig.focus(false, false).cancelled);

    rig.focus(true, false);
    rig.toggle_panel();
    rig.tick();

    assert!(rig.fake.window_requests().is_empty());
}

#[test]
fn cursor_shape_is_set_over_the_panel() {
    let rig = ManagerRig::new();
    rig.start();
    let closed = rig.cursor();

    rig.toggle_panel();
    let open = rig.cursor();
    rig.cursor();
    rig.toggle_panel();
    let after = rig.cursor();

    assert_eq!(closed, AmmlCursorShape::NONE);
    assert!(open == AmmlCursorShape::ARROW || open == AmmlCursorShape::HAND, "{open:?}");
    assert_eq!(after, AmmlCursorShape::NONE);
    assert_eq!(rig.log_count("info cursor shape shape="), 1);
}

#[test]
fn a_held_focus_loss_is_replayed_when_the_manager_is_disabled_with_the_panel_open() {
    let rig = ManagerRig::new();
    rig.start();
    rig.toggle_panel();
    assert!(rig.focus(false, false).cancelled);

    rig.fake.disable::<amml_modmanager::ManagerMod>();

    assert_eq!(rig.fake.window_requests(), vec!["replay_focus_loss".to_string()]);
}
