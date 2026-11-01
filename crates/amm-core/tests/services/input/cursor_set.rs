use amm_core::services::input::CursorSet;
use amm_core::structs::registry::ModIndex;

#[test]
fn cursor_is_free_while_any_mod_asks_or_the_panel_is_open() {
    let mut cursor = CursorSet::new();
    assert!(!cursor.cursor_free(false));
    assert!(cursor.cursor_free(true));

    cursor.request(ModIndex(1), true);
    cursor.request(ModIndex(4), true);
    cursor.request(ModIndex(1), false);

    assert!(cursor.cursor_free(false));

    cursor.request(ModIndex(4), false);

    assert!(!cursor.cursor_free(false));
    assert!(cursor.cursor_free(true));
}

#[test]
fn removing_the_last_requester_ends_cursor_mode() {
    let mut cursor = CursorSet::new();
    cursor.request(ModIndex(2), true);
    cursor.request(ModIndex(5), true);

    cursor.remove(ModIndex(2));
    assert!(cursor.cursor_free(false));

    cursor.remove(ModIndex(5));
    assert!(!cursor.cursor_free(false));

    cursor.remove(ModIndex(5));
    assert!(!cursor.cursor_free(false));
}
