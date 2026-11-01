use amm_core::structs::descriptor::{Capability, Chord};
use amm_core::structs::frame::{FrameInputData, MenuScreen, UiEvent, UiEventKind};
use amm_core::structs::mesh::MeshBuffers;
use amm_ui::model::{PanelAction, PanelModel, PanelPage, PanelTab, RebindTarget};
use amm_ui::runtime::PanelRuntime;
use amml_ui::font::FontSource;

use crate::support::{FourMods, IconFixture, MINIMAP, MINIMAP_SETTINGS};

const VERTEX_BUDGET: usize = 262_144;
const FRAMES_PER_VIEW: u32 = 3;

fn input(time_secs: f64) -> FrameInputData<'static> {
    input_with(time_secs, &[])
}

fn input_with(time_secs: f64, events: &[UiEvent]) -> FrameInputData<'_> {
    FrameInputData {
        width: 1920,
        height: 1080,
        pixels_per_point: 1.0,
        time_secs,
        events,
        allowed_layers: FrameInputData::LAYER_OVERLAY,
        input_flags: FrameInputData::ALLOW_POINTER | FrameInputData::ALLOW_KEYBOARD | FrameInputData::ALLOW_NAV_KEYS,
    }
}

// Every page and tab the panel can show, as the actions that reach it.
fn views(mods: &FourMods) -> Vec<Vec<PanelAction>> {
    let mut views = Vec::new();
    for index in [mods.minimap, mods.freecam, mods.levels, mods.dejavu] {
        for tab in [PanelTab::General, PanelTab::Keybinds, PanelTab::Capabilities] {
            views.push(vec![PanelAction::Select(Some(index)), PanelAction::SelectTab(tab)]);
        }
    }
    views.push(vec![PanelAction::Select(None)]);
    views
}

fn render_every_view(minimap: &str) {
    let mut mods = FourMods::new(minimap);
    mods.manager
        .acquire(mods.dejavu, Capability::Camera, 0)
        .unwrap_or_else(|error| panic!("dejavu acquires the camera: {error}"));
    mods.manager
        .acquire(mods.freecam, Capability::Camera, 0)
        .unwrap_or_else(|error| panic!("freecam asks for the camera: {error}"));
    mods.manager
        .acquire(mods.minimap, Capability::Hud, 0)
        .unwrap_or_else(|error| panic!("minimap acquires the hud: {error}"));
    mods.manager.set_panel_open(true);
    let mut runtime = PanelRuntime::new(FontSource::Embedded, "");
    let mut out = MeshBuffers::new();
    let mut frame = 0.0;
    let mut largest = 0;
    let views = views(&mods);
    let model = PanelModel::new(&mods.manager, "1.4.1", 100);
    let mut render = |runtime: &mut PanelRuntime, out: &mut MeshBuffers| {
        for _ in 0..FRAMES_PER_VIEW {
            frame += 1.0 / 60.0;
            runtime.run(&input(frame), &model, out);
            largest = largest.max(out.vertex_count());
            assert!(out.vertex_count() > 0, "the panel draws something");
        }
    };
    for actions in &views {
        for action in actions {
            runtime.controller_mut().on_action(action);
        }
        render(&mut runtime, &mut out);
    }
    for page in [PanelPage::ManagerSettings, PanelPage::Keybinds] {
        runtime.controller_mut().show_page(page);
        render(&mut runtime, &mut out);
    }
    runtime.controller_mut().on_action(&PanelAction::Close);
    render(&mut runtime, &mut out);
    assert!(largest < VERTEX_BUDGET, "the largest view has {largest} vertices");
}

#[test]
fn each_view_renders_four_mods_within_the_vertex_budget() {
    render_every_view(MINIMAP);
}

#[test]
fn each_view_renders_a_full_settings_schema_within_the_vertex_budget() {
    render_every_view(MINIMAP_SETTINGS);
}

#[test]
fn a_lost_texture_set_sends_the_full_font_atlas_again() {
    let mut mods = FourMods::new(MINIMAP);
    mods.manager.set_panel_open(true);
    let model = PanelModel::new(&mods.manager, "1.4.1", 0);
    let mut runtime = PanelRuntime::new(FontSource::BuiltIn, "");
    let mut out = MeshBuffers::new();
    runtime.run(&input(0.0), &model, &mut out);
    runtime.run(&input(0.1), &model, &mut out);
    let before = out.deltas.iter().filter(|delta| delta.full).count();

    runtime.textures_lost();
    runtime.run(&input(0.2), &model, &mut out);

    assert_eq!(before, 0);
    assert!(out.deltas.iter().any(|delta| delta.full));
}

// The vertices of a frame that shows a denied toast. The toast's words are messages, so a pseudo-locale draws
// more glyphs than English. `prepare` runs on the runtime before the frame.
fn toast_vertices(locale: &str, prepare: impl FnOnce(&mut PanelRuntime)) -> usize {
    let mut mods = FourMods::new(MINIMAP);
    mods.manager
        .acquire(mods.dejavu, Capability::Camera, 0)
        .unwrap_or_else(|error| panic!("dejavu acquires the camera: {error}"));
    mods.manager
        .acquire(mods.freecam, Capability::Camera, 0)
        .unwrap_or_else(|error| panic!("freecam asks for the camera: {error}"));
    let model = PanelModel::new(&mods.manager, "1.4.1", 100);
    let mut runtime = PanelRuntime::new(FontSource::BuiltIn, locale);
    let mut out = MeshBuffers::new();
    runtime.run(&input(0.0), &model, &mut out);
    prepare(&mut runtime);
    runtime.run(&input(0.1), &model, &mut out);
    out.vertex_count()
}

#[test]
fn rebuild_keeps_the_locale() {
    let english = toast_vertices("", |_| {});
    let pseudo = toast_vertices("en_XA", PanelRuntime::textures_lost);

    assert!(pseudo > english, "en_XA after a rebuild drew {pseudo} vertices, English {english}");
}

#[test]
fn set_locale_changes_the_panel() {
    let english = toast_vertices("", |_| {});
    let switched = toast_vertices("", |runtime| runtime.set_locale("en_XA"));
    let back = toast_vertices("en_XA", |runtime| runtime.set_locale(""));

    assert!(switched > english, "set_locale(en_XA) drew {switched} vertices, English {english}");
    assert_eq!(back, english);
}

fn key(vk: u32, down: bool, modifiers: u8) -> UiEvent {
    UiEvent { kind: UiEventKind::Key, x: 0.0, y: 0.0, key: vk, modifiers, down, ch: 0 }
}

#[test]
fn a_key_press_during_a_rebind_becomes_the_rebind_action() {
    let mut mods = FourMods::new(MINIMAP);
    mods.manager.set_panel_open(true);
    let model = PanelModel::new(&mods.manager, "1.4.1", 0);
    let mut runtime = PanelRuntime::new(FontSource::BuiltIn, "");
    let mut out = MeshBuffers::new();
    runtime.controller_mut().begin_rebind(RebindTarget::Mod { mod_index: mods.freecam, action: 2 });
    let events = [key(0xA2, true, UiEvent::MOD_CTRL), key(0x4A, true, UiEvent::MOD_CTRL)];

    let actions = runtime.run(&input_with(0.0, &events), &model, &mut out).to_vec();

    let chord = Chord { key: 0x4A, ctrl: true, shift: false, alt: false, f3: false };
    assert_eq!(actions, vec![PanelAction::Rebind { mod_index: mods.freecam, action: 2, chord }]);
    assert_eq!(runtime.controller_mut().rebinding(), None);
}

#[test]
fn escape_with_a_pending_edit_closes_the_panel_at_once() {
    let mut mods = FourMods::new(MINIMAP_SETTINGS);
    mods.manager.edit_draft(mods.minimap, "map.zoom", hcl::Value::from(3.0), 0).expect("zoom is a schema field");
    mods.manager.set_panel_open(true);
    let model = PanelModel::new(&mods.manager, "1.4.1", 0);
    let mut runtime = PanelRuntime::new(FontSource::BuiltIn, "");
    let mut out = MeshBuffers::new();

    let actions = runtime.run(&input_with(0.0, &[key(0x1B, true, 0)]), &model, &mut out).to_vec();

    assert_eq!(actions, vec![PanelAction::Close]);
}

fn pointer(kind: UiEventKind, (x, y): (f32, f32), down: bool) -> UiEvent {
    UiEvent { kind, x, y, key: 0, modifiers: 0, down, ch: 0 }
}

#[test]
fn the_menu_button_renders_within_the_vertex_budget_while_the_game_shows_its_cursor() {
    let mut mods = FourMods::new(MINIMAP);
    let mut runtime = PanelRuntime::new(FontSource::Embedded, "");
    let mut out = MeshBuffers::new();
    runtime.run(&input(0.0), &PanelModel::new(&mods.manager, "1.4.1", 0), &mut out);
    let in_play = (out.vertex_count(), runtime.menu_button_rect());

    mods.manager.set_menu_screen(MenuScreen::Start);
    runtime.run(&input(0.1), &PanelModel::new(&mods.manager, "1.4.1", 0), &mut out);

    assert_eq!(in_play, (0, None));
    assert!(out.vertex_count() > 0, "the button draws something");
    assert!(out.vertex_count() < VERTEX_BUDGET);
    assert!(runtime.menu_button_rect().is_some());
}

#[test]
fn a_click_on_the_menu_button_asks_to_open_the_panel() {
    let mut mods = FourMods::new(MINIMAP);
    mods.manager.set_menu_screen(MenuScreen::Start);
    let model = PanelModel::new(&mods.manager, "1.4.1", 0);
    let mut runtime = PanelRuntime::new(FontSource::BuiltIn, "");
    let mut out = MeshBuffers::new();
    runtime.run(&input(0.0), &model, &mut out);
    let [min_x, min_y, max_x, max_y] = runtime.menu_button_rect().expect("the button is placed");
    let at = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);

    runtime.run(&input_with(0.1, &[pointer(UiEventKind::PointerMove, at, false)]), &model, &mut out);
    runtime.run(&input_with(0.2, &[pointer(UiEventKind::PointerButton, at, true)]), &model, &mut out);
    let actions = runtime.run(&input_with(0.3, &[pointer(UiEventKind::PointerButton, at, false)]), &model, &mut out);

    assert_eq!(actions.to_vec(), vec![PanelAction::Open]);
}

#[test]
fn textures_lost_reuploads_icons() {
    let mut mods = FourMods::new(MINIMAP);
    mods.manager
        .set_icon(mods.freecam, Some(IconFixture::solid(8, 8, [255, 0, 0, 255])))
        .expect("freecam is registered");
    mods.manager.set_panel_open(true);
    let model = PanelModel::new(&mods.manager, "1.4.1", 0);
    let mut runtime = PanelRuntime::new(FontSource::BuiltIn, "");
    let mut out = MeshBuffers::new();
    runtime.run(&input(0.0), &model, &mut out);
    let uploads = |out: &MeshBuffers| out.deltas.iter().filter(|delta| (delta.width, delta.height) == (8, 8)).count();
    let first = uploads(&out);
    runtime.run(&input(0.1), &model, &mut out);
    let steady = uploads(&out);

    runtime.textures_lost();
    runtime.run(&input(0.2), &model, &mut out);

    assert_eq!(first, 1, "the first pass uploads the icon");
    assert_eq!(steady, 0);
    assert_eq!(uploads(&out), 1, "the icon is uploaded again");
}
