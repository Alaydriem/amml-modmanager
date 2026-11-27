use amm_ffi::services::HostCell;
use amml_api::Area;
use amml_api::Size;
use amml_api::testing::{FakeGame, FakeWindow};
use amml_modmanager::services::PanelLayoutLog;
use amml_modmanager::types::PanelLayout;

use crate::support::{ManagerRig, WarpSurface};

#[test]
fn panel_layout_line_uses_client_pixels() {
    let fake = FakeGame::new();
    fake.offer_area(Area::WINDOW_READ);
    fake.set_window(FakeWindow {
        client: Size { width: 1280, height: 720 },
        buffer: Size { width: 640, height: 360 },
        ..FakeWindow::default()
    });
    let ctx = fake.ctx();
    let log = PanelLayoutLog::new();
    let layout = PanelLayout { close: [600.0, 10.0, 620.0, 30.0], search: Some([100.0, 50.0, 300.0, 70.0]) };

    log.on_open(&ctx.log(), &layout, &ctx.window());
    log.on_open(&ctx.log(), &layout, &ctx.window());
    log.on_close();
    log.on_open(&ctx.log(), &PanelLayout { search: None, ..layout }, &ctx.window());

    assert_eq!(
        fake.logs(),
        vec![
            "info panel layout close=1200,20,40,40 search=200,100,400,40".to_string(),
            "info panel layout close=1200,20,40,40 search=none".to_string(),
        ]
    );
}

#[test]
fn a_window_with_no_size_gives_buffer_pixels() {
    let fake = FakeGame::new();
    let ctx = fake.ctx();
    let layout = PanelLayout { close: [600.0, 10.0, 620.0, 30.0], search: None };

    PanelLayoutLog::new().on_open(&ctx.log(), &layout, &ctx.window());

    assert_eq!(fake.logs(), vec!["info panel layout close=600,10,20,20 search=none".to_string()]);
}

#[test]
fn the_manager_logs_the_layout_once_per_open_after_the_panel_is_drawn() {
    let rig = ManagerRig::new();
    rig.start();
    let surface = WarpSurface::new(1280, 720);
    rig.toggle_panel();
    rig.tick();
    let before_draw = rig.log_count("info panel layout");

    rig.present(surface.surface(1));
    rig.tick();
    rig.tick();
    let [min_x, min_y, max_x, max_y] =
        HostCell::global().with(|runtime| Ok(runtime.panel_close_rect())).unwrap().unwrap();
    rig.toggle_panel();
    rig.tick();
    rig.toggle_panel();
    rig.present(surface.surface(1));
    rig.tick();

    assert_eq!(before_draw, 0);
    let expected = format!(
        "info panel layout close={},{},{},{} search=none",
        min_x.round(),
        min_y.round(),
        (max_x - min_x).round(),
        (max_y - min_y).round()
    );
    assert_eq!(rig.log_line("info panel layout"), Some(expected));
    assert_eq!(rig.log_count("info panel layout"), 2);
    assert_eq!(rig.log_count("info panel opened by=keys"), 2);
    assert_eq!(rig.log_count("info panel closed by=keys"), 1);
}
