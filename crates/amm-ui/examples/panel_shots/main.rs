// Writes the manager panel screenshots into a folder, or with --view <n> opens shot n for a look.
// --locale <code> draws them in that language (en_XA for the pseudo-locale).
mod game_backdrop;
mod scene;
mod shot;
mod shot_runner;
mod shots_app;

use scene::Scene;
use shot_runner::ShotRunner;
use shots_app::ShotsApp;

fn main() -> eframe::Result {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let locale = match args.iter().position(|arg| arg == "--locale") {
        Some(position) if position + 1 < args.len() => {
            let code = args.remove(position + 1);
            args.remove(position);
            code
        }
        _ => String::new(),
    };
    let (runner, first) = match (args.first().map(String::as_str), args.get(1)) {
        (Some("--view"), Some(number)) => (None, number.parse::<usize>().unwrap_or(1).saturating_sub(1)),
        (Some(out_dir), _) if out_dir != "--view" => (Some(ShotRunner::new(out_dir.into())), 0),
        _ => {
            eprintln!("usage: panel_shots <out dir> | --view <shot number> [--locale <code>]");
            std::process::exit(2);
        }
    };
    let scene = match Scene::new() {
        Ok(scene) => scene,
        Err(error) => {
            eprintln!("could not build the scene: {error}");
            std::process::exit(1);
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_resizable(false)
            .with_title("Alaydriem Mod Manager panel shots"),
        ..Default::default()
    };
    eframe::run_native(
        "amm-ui-panel-shots",
        options,
        Box::new(move |cc| Ok(Box::new(ShotsApp::new(&cc.egui_ctx, scene, runner, first, &locale)))),
    )
}
