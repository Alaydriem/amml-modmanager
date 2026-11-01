use amm_core::structs::frame::FrameInputData;
use amm_core::structs::mesh::MeshBuffers;
use amm_ui::model::{PanelAction, PanelModel, PanelTab};
use amm_ui::runtime::PanelRuntime;
use amm_ui::views::PanelLayout;
use amml_ui::font::FontSource;
use egui::{Pos2, Rect, vec2};

use crate::support::{DEMO, FourMods, MINIMAP_SETTINGS, PanelHarness};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const DEMO_HELP: &str = "Also registers alaydriem-demo-b, which claims the camera exclusively, to show a suspension, a denial and a key clash";

fn input(time_secs: f64) -> FrameInputData<'static> {
    FrameInputData {
        width: WIDTH,
        height: HEIGHT,
        pixels_per_point: 1.0,
        time_secs,
        events: &[],
        allowed_layers: FrameInputData::LAYER_OVERLAY,
        input_flags: FrameInputData::ALLOW_POINTER | FrameInputData::ALLOW_KEYBOARD | FrameInputData::ALLOW_NAV_KEYS,
    }
}

// The right edge of the pane's content: text drawn past it crosses the panel edge.
fn content_right() -> f32 {
    PanelLayout::pane(Rect::from_min_size(Pos2::ZERO, vec2(WIDTH as f32, HEIGHT as f32))).right()
}

// The right-most glyph vertex: glyphs sample the font atlas, fills sample the white pixel at uv (0, 0).
fn rightmost_glyph(out: &MeshBuffers) -> f32 {
    out.vertices.iter().filter(|vertex| vertex.uv != [0.0, 0.0]).map(|vertex| vertex.pos[0]).fold(0.0, f32::max)
}

// the panel on the minimap's General tab, with its full settings schema
fn minimap_general(mods: FourMods) -> PanelHarness {
    let minimap = mods.minimap;
    let mut harness = PanelHarness::new(mods, "");
    harness.controller.on_action(&PanelAction::Select(Some(minimap)));
    harness.controller.on_action(&PanelAction::SelectTab(PanelTab::General));
    harness.run();
    harness
}

#[test]
fn no_glyph_past_content_edge() {
    let long_help = format!("{DEMO_HELP}, and then some more words so it runs well past any card width at this size");
    let descriptor = DEMO.replace(DEMO_HELP, &long_help);
    let mut mods = FourMods::new(&descriptor);
    mods.manager.set_panel_open(true);
    let model = PanelModel::new(&mods.manager, "1.4.1", 0);
    let mut runtime = PanelRuntime::new(FontSource::BuiltIn, "");
    runtime.controller_mut().on_action(&PanelAction::Select(Some(mods.minimap)));
    runtime.controller_mut().on_action(&PanelAction::SelectTab(PanelTab::General));
    let mut out = MeshBuffers::new();

    for frame in 0..3 {
        runtime.run(&input(f64::from(frame) / 60.0), &model, &mut out);
    }

    let right = rightmost_glyph(&out);
    assert!(right <= content_right(), "a glyph ends at {right}, past the content edge {}", content_right());
}

#[test]
fn typed_value_saves_a_draft() {
    let mods = FourMods::new(MINIMAP_SETTINGS);
    let minimap = mods.minimap;
    let mut harness = minimap_general(mods);
    let zoom = harness.text_rect("Zoom").unwrap_or_else(|| panic!("the Zoom row is painted"));
    // the zoom's number is the right-most number on the row
    let number = harness
        .texts()
        .into_iter()
        .filter(|(text, rect)| {
            text.parse::<f64>().is_ok() && rect.bottom() > zoom.top() && rect.top() < zoom.bottom() + 40.0
        })
        .max_by(|a, b| a.1.right().total_cmp(&b.1.right()))
        .unwrap_or_else(|| panic!("the zoom value is painted"));

    harness.click(number.1.center());
    harness.type_and_enter("2.5");

    let edit = PanelAction::Edit { mod_index: minimap, path: "map.zoom".to_owned(), value: hcl::Value::from(2.5) };
    assert!(harness.actions.contains(&edit), "no zoom edit in {:?}", harness.actions);
}

#[test]
fn changed_in_game_note_shows_the_value() {
    let mut mods = FourMods::new(MINIMAP_SETTINGS);
    let minimap = mods.minimap;
    mods.manager
        .edit_draft(minimap, "map.zoom", hcl::Value::from(2.0), 0)
        .unwrap_or_else(|error| panic!("the zoom edits: {error}"));
    mods.manager.set_setting(minimap, "map.zoom", "3.0", 0).unwrap_or_else(|error| panic!("the game sets it: {error}"));

    let harness = minimap_general(mods);

    assert!(harness.text_rect("Changed in game to 3.00x").is_some(), "no note in {:?}", harness.texts());
}
