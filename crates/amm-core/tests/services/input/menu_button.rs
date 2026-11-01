use amm_core::services::input::MenuButton;
use amm_core::structs::frame::MenuScreen;

const RECT: [f32; 4] = [1851.0, 987.0, 1896.0, 1032.0];

fn on_start_screen() -> MenuButton {
    let mut button = MenuButton::new(true);
    button.set_screen(MenuScreen::Start);
    button.place(Some(RECT));
    button
}

#[test]
fn hidden_while_overlay_covers_screen() {
    let button = on_start_screen();

    assert!(!button.visible(false, true));
}

#[test]
fn shown_with_small_overlay() {
    let button = on_start_screen();

    assert!(button.visible(false, false));
}

#[test]
fn rect_is_none_while_covered() {
    let button = on_start_screen();

    assert_eq!(button.rect(false, true), None);
    assert_eq!(button.rect(false, false), Some(RECT));
}

#[test]
fn a_click_on_the_old_rect_while_covered_misses_the_button() {
    let button = on_start_screen();

    assert!(!button.contains(false, true, (1860.0, 1000.0)));
    assert!(button.contains(false, false, (1860.0, 1000.0)));
}
