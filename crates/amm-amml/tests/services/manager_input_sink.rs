use amm_abi::types::AmmCursorShape;
use amm_overlay::InputSink;
use amml_modmanager::services::{ManagerInputSink, ManagerState};

#[test]
fn the_cursor_is_the_one_of_the_last_host_frame() {
    let state = ManagerState::leaked();
    let sink = ManagerInputSink::new(state);
    assert_eq!(sink.cursor(), (false, AmmCursorShape::Default));

    state.frame().publish(state.latch(), true, AmmCursorShape::Text, false);

    assert_eq!(sink.cursor(), (true, AmmCursorShape::Text));
}
