use amm_core::structs::descriptor::Capability;
use amm_ui::views::ToastsView;
use egui::{Pos2, Rect, vec2};

use crate::support::{FourMods, MINIMAP, PanelHarness};

#[test]
fn toast_title_and_message_are_two_texts() {
    let mut mods = FourMods::new(MINIMAP);
    let (dejavu, freecam) = (mods.dejavu, mods.freecam);
    mods.manager.acquire(dejavu, Capability::Camera, 0).unwrap_or_else(|error| panic!("dejavu acquires: {error}"));
    // freecam's shared claim is denied while DejaVu holds the camera exclusively: a denied toast
    mods.manager.acquire(freecam, Capability::Camera, 0).unwrap_or_else(|error| panic!("freecam asks: {error}"));

    let harness = PanelHarness::new(mods, "");

    let title = harness.text_rect("Camera is held by DejaVu");
    let message = harness.text_rect("Freecam: the request was denied.");
    assert!(title.is_some() && message.is_some(), "the toast's lines in {:?}", harness.texts());
    assert!(!harness.texts().iter().any(|(text, _)| text.contains("DejaVu. ")), "a text holds two sentences");
}

fn screen() -> Rect {
    Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0))
}

#[test]
fn toasts_stack_above_the_menu_button_while_it_shows() {
    let button = Rect::from_min_max(egui::pos2(1744.0, 996.0), egui::pos2(1896.0, 1032.0));

    let (right, bottom) = ToastsView::anchor(screen(), None, Some(button), 84.0);

    assert!(bottom < button.top(), "the stack ends at {bottom}, the button starts at {}", button.top());
    assert_eq!(right, screen().right() - ToastsView::MARGIN);
}

#[test]
fn toasts_sit_in_the_corner_without_the_menu_button() {
    let (_, bottom) = ToastsView::anchor(screen(), None, None, 84.0);

    assert_eq!(bottom, screen().bottom() - ToastsView::MARGIN);
}
