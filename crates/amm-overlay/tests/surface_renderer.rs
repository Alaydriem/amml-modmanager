use amm_overlay::DrawReport;

use super::support::WarpTarget;
use super::support::log_capture::LogCapture;
use super::support::ref_count::RefCount;
use super::support::rig_outcome::RigOutcome;
use super::support::surface_rig::SurfaceRig;
use super::support::test_host::TestHost;

const BLACK: [u8; 4] = [0, 0, 0, 255];
const MADE: &str = "overlay device objects made";

fn drawn(outcome: RigOutcome) -> DrawReport {
    match outcome {
        RigOutcome::Drawn(report) => report,
        other => panic!("expected a draw, got {other:?}"),
    }
}

#[test]
fn draw_makes_its_objects_once_per_device_epoch() {
    LogCapture::install();
    let rig = SurfaceRig::new();
    let target = WarpTarget::new();
    target.clear(BLACK);
    assert_eq!(rig.renderer().live_objects(), 0);

    let first = drawn(rig.draw(target.surface(1)));
    let made = rig.renderer().live_objects();
    let second = drawn(rig.draw(target.surface(1)));

    assert_eq!(first.drawn, 1);
    assert_eq!(second.drawn, 1);
    assert!(made > 0);
    assert_eq!(rig.renderer().live_objects(), made);
    assert_eq!(LogCapture::count(MADE), 1);
    assert_eq!(rig.host.textures_lost(), 0);
    assert_eq!(target.pixel(32, 32), TestHost::RED);
}

#[test]
fn surface_lost_releases_every_device_object() {
    let rig = SurfaceRig::new();
    let target = WarpTarget::new();
    drawn(rig.draw(target.surface(1)));
    assert!(rig.renderer().live_objects() > 0);

    rig.lose(1);

    assert_eq!(rig.renderer().live_objects(), 0);
    assert_eq!(rig.host.textures_lost(), 1);
    // a second loss with nothing held drops nothing the manager must send again
    rig.lose(1);
    assert_eq!(rig.host.textures_lost(), 1);
}

#[test]
fn a_new_device_epoch_rebuilds_the_objects() {
    LogCapture::install();
    let rig = SurfaceRig::new();
    let first = WarpTarget::new();
    drawn(rig.draw(first.surface(1)));
    rig.lose(1);
    let second = WarpTarget::new();
    second.clear(BLACK);

    let report = drawn(rig.draw(second.surface(2)));

    assert!(rig.renderer().live_objects() > 0);
    assert_eq!(LogCapture::count(MADE), 2);
    assert_eq!(report.drawn, 1);
    assert_eq!(report.skipped(), 0);
    assert!(!report.geometry_dropped);
    assert_eq!(second.pixel(32, 32), TestHost::RED);
}

#[test]
fn a_new_device_epoch_without_a_loss_drops_the_old_objects_first() {
    let rig = SurfaceRig::new();
    let first = WarpTarget::new();
    drawn(rig.draw(first.surface(1)));
    let second = WarpTarget::new();
    second.clear(BLACK);

    let report = drawn(rig.draw(second.surface(2)));

    assert_eq!(rig.host.textures_lost(), 1);
    assert_eq!(report.drawn, 1);
    assert_eq!(second.pixel(32, 32), TestHost::RED);
}

#[test]
fn draw_does_not_keep_a_back_buffer_reference() {
    let rig = SurfaceRig::new();
    let target = WarpTarget::new();
    let view_before = RefCount::of(&target.view);
    let buffer_before = RefCount::of(&target.texture);

    drawn(rig.draw(target.surface(1)));
    drawn(rig.draw(target.surface(1)));

    assert_eq!(RefCount::of(&target.view), view_before);
    assert_eq!(RefCount::of(&target.texture), buffer_before);
}

#[test]
fn game_picture_copy_needs_the_same_size_and_format() {
    let rig = SurfaceRig::new();
    let target = WarpTarget::new();
    let mut smaller = target.surface(1);
    smaller.width = WarpTarget::SIZE / 2;
    let mut unknown_format = target.surface(1);
    unknown_format.format = 0;

    let mismatched = rig.refresh_picture(smaller);
    let made = rig.renderer().live_objects();
    let unknown = rig.refresh_picture(unknown_format);

    assert!(matches!(mismatched, RigOutcome::Picture(Err(_))), "{mismatched:?}");
    assert!(matches!(unknown, RigOutcome::Picture(Err(_))), "{unknown:?}");
    // the unknown format made no texture
    assert_eq!(rig.renderer().live_objects(), made);
    // no panic: the mod still draws
    assert_eq!(drawn(rig.draw(target.surface(1))).drawn, 1);
}

#[test]
fn a_refused_picture_copy_leaves_its_commands_undrawn() {
    let rig = SurfaceRig::new();
    let target = WarpTarget::new();
    target.clear(BLACK);
    rig.host.set_picture(true);

    let report = drawn(rig.draw(target.surface(1)));

    assert_eq!(report.drawn, 0);
    assert_eq!(report.skipped_texture, 1);
    assert_eq!(target.pixel(32, 32), BLACK);
}

#[test]
fn a_frame_before_the_game_is_ready_makes_nothing() {
    let rig = SurfaceRig::new();
    let target = WarpTarget::new();
    rig.host.set_ready(false);

    let report = drawn(rig.draw(target.surface(1)));

    assert_eq!(report, DrawReport::default());
    assert_eq!(rig.renderer().live_objects(), 0);
    assert_eq!(rig.host.frames(), 0);
    assert_eq!(rig.host.skipped(), 1);
}

#[test]
fn the_manager_layers_wait_for_the_manager_pass() {
    let rig = SurfaceRig::new();
    let target = WarpTarget::new();
    target.clear(BLACK);
    rig.host.set_split(true);

    let mods = drawn(rig.draw_mods(target.surface(1)));
    let after_mods = (target.pixel(16, 32), target.pixel(48, 32));
    let manager = drawn(rig.draw_manager(target.surface(1)));

    assert_eq!(mods.drawn, 1);
    assert_eq!(after_mods, (TestHost::RED, BLACK));
    assert_eq!(manager.drawn, 1);
    assert_eq!((target.pixel(16, 32), target.pixel(48, 32)), (TestHost::RED, TestHost::BLUE));
    assert_eq!(rig.host.frames(), 1);
}

#[test]
fn a_manager_pass_without_a_mod_pass_draws_nothing() {
    let rig = SurfaceRig::new();
    let target = WarpTarget::new();
    target.clear(BLACK);
    rig.host.set_split(true);
    drawn(rig.draw_mods(target.surface(1)));
    drawn(rig.draw_manager(target.surface(1)));
    target.clear(BLACK);

    // the frame was drawn once; a second manager pass in the same present has nothing to draw
    let again = drawn(rig.draw_manager(target.surface(1)));
    rig.lose(1);
    let after_loss = drawn(rig.draw_manager(target.surface(1)));

    assert_eq!(again, DrawReport::default());
    assert_eq!(after_loss, DrawReport::default());
    assert_eq!(target.pixel(48, 32), BLACK);
    assert_eq!(rig.host.frames(), 1);
}

#[test]
fn a_host_without_a_frame_skips_it() {
    let rig = SurfaceRig::new();
    let target = WarpTarget::new();
    target.clear(BLACK);
    rig.host.set_no_frame(true);

    let report = drawn(rig.draw(target.surface(1)));

    assert_eq!(report, DrawReport::default());
    assert_eq!(rig.host.skipped(), 1);
    assert_eq!(target.pixel(32, 32), BLACK);
}

#[test]
fn a_frame_with_no_layer_never_asks_for_the_surface() {
    let target = WarpTarget::new();
    let rig = SurfaceRig::on_demand(target.surface(1));
    target.clear(BLACK);
    rig.host.set_empty(true);

    let report = drawn(rig.draw(SurfaceRig::waiting(target.surface(1))));
    drawn(rig.draw_mods(SurfaceRig::waiting(target.surface(1))));
    drawn(rig.draw_manager(SurfaceRig::waiting(target.surface(1))));

    assert_eq!(report, DrawReport::default());
    assert_eq!(rig.fake.prepare_surface_calls(), 0);
    assert_eq!(rig.host.frames(), 2);
    assert_eq!(rig.host.skipped(), 0);
    assert!(rig.renderer().live_objects() > 0);
    assert_eq!(target.pixel(32, 32), BLACK);
}

#[test]
fn a_frame_with_a_layer_asks_for_the_surface_once_and_draws() {
    let target = WarpTarget::new();
    let rig = SurfaceRig::on_demand(target.surface(1));
    target.clear(BLACK);

    let report = drawn(rig.draw(SurfaceRig::waiting(target.surface(1))));

    assert_eq!(report.drawn, 1);
    assert_eq!(rig.fake.prepare_surface_calls(), 1);
    assert_eq!(target.pixel(32, 32), TestHost::RED);
}

#[test]
fn a_frame_with_only_a_manager_layer_asks_for_the_surface_in_the_manager_pass_only() {
    let target = WarpTarget::new();
    let rig = SurfaceRig::on_demand(target.surface(1));
    rig.host.set_manager_only(true);

    drawn(rig.draw_mods(SurfaceRig::waiting(target.surface(1))));
    let after_mods = rig.fake.prepare_surface_calls();
    let manager = drawn(rig.draw_manager(SurfaceRig::waiting(target.surface(1))));

    assert_eq!(after_mods, 0);
    assert_eq!(manager.drawn, 1);
    assert_eq!(rig.fake.prepare_surface_calls(), 1);
    assert_eq!(target.pixel(48, 32), TestHost::BLUE);
}

#[test]
fn a_frame_whose_layers_draw_nothing_never_asks_for_the_surface() {
    let target = WarpTarget::new();
    let rig = SurfaceRig::on_demand(target.surface(1));
    target.clear(BLACK);
    rig.host.set_blank_layer(true);

    let report = drawn(rig.draw(SurfaceRig::waiting(target.surface(1))));

    assert_eq!(report, DrawReport::default());
    assert_eq!(rig.fake.prepare_surface_calls(), 0);
    assert_eq!(target.pixel(32, 32), BLACK);
}
