use amm_core::services::input::CornerRow;
use amm_core::structs::frame::{FrameInputData, MenuScreen, UiEvent, UiEventKind};
use amm_core::structs::menu::{MenuEntryId, MenuScreens};
use amm_core::structs::mesh::MeshBuffers;
use amm_ui::model::{PanelAction, PanelModel};
use amm_ui::runtime::PanelRuntime;
use amml_ui::font::FontSource;
use amml_ui::widget::CornerButton;

use crate::support::{FourMods, MINIMAP};

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

fn pointer(kind: UiEventKind, (x, y): (f32, f32), down: bool) -> UiEvent {
    UiEvent { kind, x, y, key: 0, modifiers: 0, down, ch: 0 }
}

// the row's rect after one pass, then the actions of a click at `x` (from the row's right edge) on its middle line
fn click_from_right(mods: &FourMods, from_right: f32) -> ([f32; 4], Vec<PanelAction>) {
    let model = PanelModel::new(&mods.manager, "1.4.1", 0);
    let mut runtime = PanelRuntime::new(FontSource::BuiltIn, "");
    let mut out = MeshBuffers::new();
    runtime.run(&input_with(0.0, &[]), &model, &mut out);
    let rect = runtime.menu_button_rect().unwrap_or_else(|| panic!("the row is placed"));
    let at = (rect[2] - from_right, (rect[1] + rect[3]) / 2.0);
    runtime.run(&input_with(0.1, &[pointer(UiEventKind::PointerMove, at, false)]), &model, &mut out);
    runtime.run(&input_with(0.2, &[pointer(UiEventKind::PointerButton, at, true)]), &model, &mut out);
    let actions = runtime.run(&input_with(0.3, &[pointer(UiEventKind::PointerButton, at, false)]), &model, &mut out);
    (rect, actions.to_vec())
}

#[test]
fn the_layout_height_is_the_button_height() {
    // amm-core lays the row out without amml-ui; the button it lays out must fill its slot
    assert_eq!(CornerRow::HEIGHT, CornerButton::HEIGHT);
}

#[test]
fn mods_is_square_icon_with_no_entries() {
    let mut mods = FourMods::new(MINIMAP);
    mods.manager.set_menu_screen(MenuScreen::Start);

    let (rect, actions) = click_from_right(&mods, CornerButton::HEIGHT / 2.0);

    assert_eq!(rect, [1851.0, 987.0, 1896.0, 1032.0]);
    assert_eq!(actions, [PanelAction::Open]);
}

#[test]
fn mods_is_square_icon_when_entries_exist() {
    let mut mods = FourMods::new(MINIMAP);
    let dejavu = mods.dejavu;
    let _ = mods.manager.register_menu_entry(dejavu, "Dejavu", "DV", MenuScreens::START);
    mods.manager.set_menu_screen(MenuScreen::Start);

    let (rect, actions) = click_from_right(&mods, CornerButton::HEIGHT / 2.0);

    assert!(rect[0] < 1851.0 - CornerRow::GAP - 50.0, "the row reaches left of Mods: {rect:?}");
    assert_eq!(rect[2], 1896.0);
    assert_eq!(actions, [PanelAction::Open]);
}

#[test]
fn entry_click_returns_entry_target() {
    let mut mods = FourMods::new(MINIMAP);
    let dejavu = mods.dejavu;
    let _ = mods.manager.register_menu_entry(dejavu, "Dejavu", "DV", MenuScreens::START);
    mods.manager.set_menu_screen(MenuScreen::Start);

    let (_, actions) = click_from_right(&mods, CornerButton::HEIGHT + CornerRow::GAP + 30.0);

    assert_eq!(actions, [PanelAction::MenuEntry { owner: dejavu, id: MenuEntryId(1) }]);
}

#[test]
fn a_start_only_entry_is_not_on_the_pause_screen() {
    let mut mods = FourMods::new(MINIMAP);
    let dejavu = mods.dejavu;
    let _ = mods.manager.register_menu_entry(dejavu, "Dejavu", "DV", MenuScreens::START);
    mods.manager.set_menu_screen(MenuScreen::Pause);

    let (rect, _) = click_from_right(&mods, CornerButton::HEIGHT / 2.0);

    assert_eq!(rect, [1851.0, 987.0, 1896.0, 1032.0]);
}
