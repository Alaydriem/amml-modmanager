use amm_core::structs::frame::{FrameInputData, LayerKind};

#[test]
fn the_layer_bit_of_a_frame_call_names_the_layer_it_draws() {
    let hud = LayerKind::from_bits(FrameInputData::LAYER_HUD);
    let overlay = LayerKind::from_bits(FrameInputData::LAYER_OVERLAY);
    let both = LayerKind::from_bits(FrameInputData::LAYER_HUD | FrameInputData::LAYER_OVERLAY);

    assert_eq!((hud, overlay), (LayerKind::Hud, LayerKind::Overlay));
    assert_eq!(both, LayerKind::Overlay, "a 1.0 host sends both bits for one mesh, drawn at its highest kind");
}
