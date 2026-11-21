use amm_abi::types::{AmmCapability, AmmHandle, AmmStatus};

use crate::support::{Descriptors, ManagerRig, WarpSurface};

fn forcing(rig: &ManagerRig) -> AmmHandle {
    rig.start();
    let consumer = rig.register(&Descriptors::overlay_and_size("consumer"));
    assert!(rig.acquire(consumer, AmmCapability::RenderSize));
    assert_eq!(rig.force_size(consumer, Some((640, 360))), AmmStatus::Ok);
    consumer
}

#[test]
fn render_size_writes_resize_and_window_size() {
    let rig = ManagerRig::new();
    let consumer = forcing(&rig);
    let surface = WarpSurface::new(640, 360);

    let resized = rig.resize(1280, 720);
    let sized = rig.window_size(1280, 720, false);
    rig.resized(true, 640, 360);
    rig.present(surface.surface(1));
    let while_forced = rig.fake.window_requests();
    assert_eq!(rig.force_size(consumer, None), AmmStatus::Ok);
    rig.present(surface.surface(1));

    assert_eq!(resized, (640, 360));
    assert_eq!(sized, (640, 360));
    assert_eq!(while_forced, vec!["request_size 640x360".to_string()]);
    assert_eq!(rig.fake.window_requests().last().map(String::as_str), Some("request_size none"));
    assert_eq!(rig.log_count("info resized ok=true width=640 height=360"), 1);
    assert_eq!(rig.log_count("info render size requested width=640 height=360"), 1);
    assert_eq!(rig.log_count("info render size given back"), 1);
    // nothing forced: a resize and a size message keep what the game asked
    assert_eq!(rig.resize(1280, 720), (1280, 720));
    assert_eq!(rig.window_size(1280, 720, false), (1280, 720));
}

#[test]
fn injected_window_size_is_not_written_again() {
    let rig = ManagerRig::new();
    forcing(&rig);

    let posted = rig.window_size(1280, 720, true);

    assert_eq!(posted, (1280, 720));
    assert!(rig.fake.window_requests().is_empty());
}

#[test]
fn a_window_change_sends_the_forced_size_again() {
    let rig = ManagerRig::new();
    forcing(&rig);
    let surface = WarpSurface::new(640, 360);
    rig.present(surface.surface(1));

    rig.present(surface.surface(1));
    let without_change = rig.fake.window_requests().len();
    rig.window_changed();
    rig.present(surface.surface(1));

    assert_eq!(without_change, 1);
    assert_eq!(rig.fake.window_requests(), vec!["request_size 640x360".to_string(); 2]);
    // the resend of the same size is not logged again
    assert_eq!(rig.log_count("info render size requested"), 1);
}
