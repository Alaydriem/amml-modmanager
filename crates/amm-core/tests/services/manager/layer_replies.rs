use amm_core::services::manager::LayerReplies;
use amm_core::structs::frame::{CursorShape, FrameInputData, FrameReplyData, LayerKind};
use amm_core::structs::registry::ModIndex;
use amm_core::structs::snapshot::LayerEntry;

const MOD: ModIndex = ModIndex(0);

fn hud(mod_index: ModIndex) -> LayerEntry {
    LayerEntry { mod_index, kind: LayerKind::Hud, allowed: FrameInputData::LAYER_HUD }
}

fn overlay(mod_index: ModIndex) -> LayerEntry {
    LayerEntry { mod_index, kind: LayerKind::Overlay, allowed: FrameInputData::LAYER_OVERLAY }
}

fn reply(wants_pointer: bool, wants_keyboard: bool) -> FrameReplyData {
    FrameReplyData { cursor: CursorShape::Default, wants_pointer, wants_keyboard, layer_rect: [0.0, 0.0, 100.0, 100.0] }
}

#[test]
fn the_key_layer_falls_back_to_the_overlay_layer_then_the_hud_layer() {
    let replies = LayerReplies::new(4);

    let both = replies.key_layer(&[hud(MOD), overlay(MOD)], MOD);
    let hud_only = replies.key_layer(&[hud(MOD)], MOD);
    let none = replies.key_layer(&[hud(ModIndex(1))], MOD);

    assert_eq!((both, hud_only, none), (Some(LayerKind::Overlay), Some(LayerKind::Hud), None));
}

#[test]
fn the_key_layer_is_the_layer_that_wants_the_keyboard_and_the_overlay_layer_when_both_do() {
    let mut replies = LayerReplies::new(4);
    let layers = [hud(MOD), overlay(MOD)];
    replies.record(MOD, LayerKind::Hud, reply(false, true));
    replies.record(MOD, LayerKind::Overlay, reply(false, false));

    let hud_wants = replies.key_layer(&layers, MOD);
    replies.record(MOD, LayerKind::Overlay, reply(false, true));
    let both_want = replies.key_layer(&layers, MOD);

    assert_eq!((hud_wants, both_want), (Some(LayerKind::Hud), Some(LayerKind::Overlay)));
    assert_eq!(replies.keyboard_owner(&layers), Some(MOD));
}

#[test]
fn retain_forgets_the_reply_of_a_layer_that_is_no_longer_drawn() {
    let mut replies = LayerReplies::new(4);
    replies.record(MOD, LayerKind::Hud, reply(true, true));

    replies.retain(&[overlay(MOD)]);

    let layers = [hud(MOD), overlay(MOD)];
    assert_eq!(replies.pointer_layer(&layers, (50.0, 50.0)), None);
    assert_eq!(replies.keyboard_owner(&layers), None);
}

fn reply_with_rect(layer_rect: [f32; 4]) -> FrameReplyData {
    FrameReplyData { cursor: CursorShape::Default, wants_pointer: true, wants_keyboard: false, layer_rect }
}

#[test]
fn an_overlay_layer_covers_the_screen_within_one_pixel_and_a_small_one_does_not() {
    let mut replies = LayerReplies::new(4);
    let layers = [hud(MOD), overlay(MOD)];
    replies.record(MOD, LayerKind::Overlay, reply_with_rect([0.0, 0.0, 1919.5, 1080.0]));
    let full = replies.overlay_covers(&layers, (1920, 1080));
    replies.record(MOD, LayerKind::Overlay, reply_with_rect([0.0, 0.0, 300.0, 1080.0]));
    let rail = replies.overlay_covers(&layers, (1920, 1080));

    assert_eq!((full, rail), (true, false));
}

#[test]
fn a_hud_layer_never_covers_the_screen() {
    let mut replies = LayerReplies::new(4);
    replies.record(MOD, LayerKind::Hud, reply_with_rect([0.0, 0.0, 1920.0, 1080.0]));

    assert!(!replies.overlay_covers(&[hud(MOD)], (1920, 1080)));
}
