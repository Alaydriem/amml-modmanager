use std::sync::{Arc, Mutex};

use amm_abi::types::AmmCapability;
use amml_api::amml_abi::FramePresent;
use amml_api::{FramePresentExt, HookOrder};
use amml_modmanager::services::ManagerRuntime;

use crate::support::{ConsumerCallbacks, Descriptors, ManagerRig, WarpSurface};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

#[test]
fn no_surface_skips_the_frame_without_a_draw() {
    let rig = ManagerRig::new();
    rig.start();
    let host = ManagerRuntime::get().unwrap().overlay();

    rig.present(ManagerRig::no_surface());

    assert_eq!(host.frames(), 0);
    assert_eq!(host.skipped(), 1);
    assert_eq!(rig.log_count("info overlay bound"), 0);
}

#[test]
fn mod_layers_draw_in_normal_and_the_panel_in_last() {
    let rig = ManagerRig::new();
    let surface = Arc::new(WarpSurface::new(WIDTH, HEIGHT));
    // what a listener between the manager's two bands sees: the consumer's frame calls and the pixels drawn
    let between: Arc<Mutex<Vec<(u32, usize)>>> = Arc::new(Mutex::new(Vec::new()));
    let (seen, target) = (Arc::clone(&between), Arc::clone(&surface));
    let probe = rig
        .fake
        .ctx()
        .events()
        .listen::<FramePresent>(HookOrder::READ_AFTER_WRITERS, move |event, _| {
            assert!(event.surface().is_some());
            seen.lock().unwrap().push((ConsumerCallbacks::frames(), target.drawn_pixels()));
        })
        .unwrap();
    rig.start();
    let consumer = rig.register(&Descriptors::overlay_and_size("consumer"));
    rig.set_consumer_callbacks(consumer);
    assert!(rig.acquire(consumer, AmmCapability::Overlay));
    rig.toggle_panel();
    surface.clear();

    rig.present(surface.surface(1));

    // the consumer's layer was composed and drawn before the probe, the panel only after it
    assert_eq!(*between.lock().unwrap(), vec![(1, 0)]);
    assert!(surface.drawn_pixels() > 0, "the panel is drawn in the last band");
    drop(probe);
}

#[test]
fn surface_lost_calls_textures_lost_on_consumers_once() {
    let rig = ManagerRig::new();
    rig.start();
    let consumer = rig.register(&Descriptors::overlay_and_size("consumer"));
    rig.set_consumer_callbacks(consumer);
    assert!(rig.acquire(consumer, AmmCapability::Overlay));
    let surface = WarpSurface::new(WIDTH, HEIGHT);
    rig.present(surface.surface(1));
    assert_eq!(ConsumerCallbacks::frames(), 1);

    rig.lose_surface(1);
    rig.lose_surface(1);

    assert_eq!(ConsumerCallbacks::textures_lost_calls(), 1);
}

#[test]
fn each_bind_logs_overlay_bound_once() {
    let rig = ManagerRig::new();
    rig.start();
    let surface = WarpSurface::new(WIDTH, HEIGHT);

    rig.present(surface.surface(1));
    rig.present(surface.surface(1));
    let first_bind = rig.log_count("info overlay bound width=1280 height=720 wrapped=false");
    rig.lose_surface(1);
    rig.device_reset(2);
    rig.present(surface.surface(2));

    assert_eq!(first_bind, 1);
    assert_eq!(rig.log_count("info overlay bound width=1280 height=720 wrapped=false"), 2);
    assert_eq!(rig.log_count("info render device reset epoch=2"), 1);
    assert_eq!(ManagerRuntime::get().unwrap().overlay().frames(), 3);
}

#[test]
fn a_present_and_its_pace_report_run_without_a_size_forced() {
    let rig = ManagerRig::new();
    rig.start();
    let surface = WarpSurface::new(WIDTH, HEIGHT);

    let present = rig.present(surface.surface(1));
    let presented = rig.presented();

    assert!(!present.panicked);
    assert!(!presented.panicked);
    assert!(rig.fake.window_requests().is_empty());
}
