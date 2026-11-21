use amm_abi::types::{AmmCapability, AmmStatus};
use amml_api::amml_abi::{AmmlMouseAction, AmmlMouseButton, AmmlPointerKind, MouseInput};
use amml_api::testing::FakeGame;

use crate::support::{Descriptors, ManagerRig, WarpSurface};

fn feed(rig: &ManagerRig, x: i32, y: i32, injected: bool) -> (i32, i32) {
    let mut header = FakeGame::header::<MouseInput>();
    header.injected = u8::from(injected);
    let mut event = MouseInput {
        header,
        action: AmmlMouseAction::MOVE,
        button: AmmlMouseButton::LEFT,
        down: 0,
        _reserved: 0,
        x,
        y,
        dx: 1,
        dy: 1,
        notches: 0.0,
    };
    rig.fake.raise(&mut event);
    (event.x, event.y)
}

#[test]
fn the_game_mouse_feed_gets_buffer_pixels_while_a_size_is_forced() {
    let rig = ManagerRig::new();
    rig.start();
    let consumer = rig.register(&Descriptors::overlay_and_size("consumer"));
    assert!(rig.acquire(consumer, AmmCapability::RenderSize));
    assert_eq!(rig.force_size(consumer, Some((640, 360))), AmmStatus::Ok);
    rig.present(WarpSurface::new(640, 360).surface(1));
    rig.pointer(AmmlPointerKind::MOVE, AmmlMouseButton::LEFT, false, (640.0, 360.0), (320.0, 180.0));

    let real = feed(&rig, 640, 360, false);
    let injected = feed(&rig, 640, 360, true);

    assert_eq!(real, (320, 180));
    assert_eq!(injected, (640, 360));
}

#[test]
fn the_game_mouse_feed_is_left_alone_with_nothing_forced_and_no_viewport() {
    let rig = ManagerRig::new();
    rig.start();
    rig.present(WarpSurface::new(1280, 720).surface(1));
    rig.pointer(AmmlPointerKind::MOVE, AmmlMouseButton::LEFT, false, (640.0, 360.0), (640.0, 360.0));

    assert_eq!(feed(&rig, 641, 361, false), (641, 361));
}
