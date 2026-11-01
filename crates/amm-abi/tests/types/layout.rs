use std::mem::{align_of, offset_of, size_of};

use amm_abi::types::{
    AmmAcquireOutcome, AmmAcquireResult, AmmApi, AmmCallbacks, AmmCapability, AmmCaptureStreamFault,
    AmmCaptureStreamFrame, AmmCaptureStreamRequest, AmmCursorShape, AmmDrawCmd, AmmEvent, AmmEventKind, AmmFrameInput,
    AmmFrameOutput, AmmHandle, AmmLogLevel, AmmMeshView, AmmRenderSize, AmmRenderSizeState, AmmSamplerKind,
    AmmScreenshotLayers, AmmStatus, AmmTextureDelta, AmmUiEvent, AmmUiEventKind, AmmVertex,
};
use amm_core::structs::frame::{CursorShape, UiEvent, UiEventKind};
use amm_core::structs::mesh::{DrawCmd, SamplerKind, TextureDelta, Vertex};
use amm_core::traits::AbiSized;

#[test]
fn api_table_field_offsets_are_stable() {
    assert_eq!(offset_of!(AmmApi, size), 0);
    assert_eq!(offset_of!(AmmApi, abi_major), 4);
    assert_eq!(offset_of!(AmmApi, abi_minor), 8);
    assert_eq!(offset_of!(AmmApi, register), 16);
    assert_eq!(offset_of!(AmmApi, unregister), 16 + 8);
    assert_eq!(offset_of!(AmmApi, config_document), 16 + 2 * 8);
    assert_eq!(offset_of!(AmmApi, acquire), 16 + 3 * 8);
    assert_eq!(offset_of!(AmmApi, release), 16 + 4 * 8);
    assert_eq!(offset_of!(AmmApi, holder), 16 + 5 * 8);
    assert_eq!(offset_of!(AmmApi, poll_events), 16 + 6 * 8);
    assert_eq!(offset_of!(AmmApi, key_down), 16 + 7 * 8);
    assert_eq!(offset_of!(AmmApi, set_setting), 16 + 8 * 8);
    assert_eq!(offset_of!(AmmApi, set_cursor_mode), 16 + 9 * 8);
    assert_eq!(offset_of!(AmmApi, log), 16 + 10 * 8);
    assert_eq!(offset_of!(AmmApi, request_screenshot), 16 + 11 * 8);
    assert_eq!(offset_of!(AmmApi, held_by_other), 16 + 12 * 8);
    assert_eq!(offset_of!(AmmApi, texture_budget), 16 + 13 * 8);
    assert_eq!(offset_of!(AmmApi, request_screenshot_layers), 16 + 14 * 8);
    assert_eq!(offset_of!(AmmApi, register_menu_entry), 16 + 15 * 8);
    assert_eq!(offset_of!(AmmApi, locale), 16 + 16 * 8);
    assert_eq!(offset_of!(AmmApi, set_icon), 16 + 17 * 8);
    assert_eq!(offset_of!(AmmApi, game_picture), 16 + 18 * 8);
    assert_eq!(offset_of!(AmmApi, set_game_viewport), 16 + 19 * 8);
    assert_eq!(offset_of!(AmmApi, set_render_size), 16 + 20 * 8);
    assert_eq!(offset_of!(AmmApi, render_size), 16 + 21 * 8);
    assert_eq!(offset_of!(AmmApi, capture_stream_request), 16 + 22 * 8);
    assert_eq!(offset_of!(AmmApi, capture_stream_take), 16 + 23 * 8);
    assert_eq!(offset_of!(AmmApi, capture_stream_free), 16 + 24 * 8);
    assert_eq!(offset_of!(AmmApi, capture_stream_close), 16 + 25 * 8);
    assert_eq!(offset_of!(AmmApi, present_count), 16 + 26 * 8);
    assert_eq!(offset_of!(AmmApi, capture_stream_lend), 16 + 27 * 8);
    assert_eq!(offset_of!(AmmApi, capture_stream_give_back), 16 + 28 * 8);
    assert_eq!(offset_of!(AmmApi, watch_keys), 16 + 29 * 8);
    assert_eq!(offset_of!(AmmApi, send_key), 16 + 30 * 8);
    assert_eq!(size_of::<AmmApi>(), 16 + 31 * 8);
}

#[test]
fn api_size_covers_send_key() {
    assert!(offset_of!(AmmApi, send_key) > offset_of!(AmmApi, capture_stream_give_back));
    assert!(offset_of!(AmmApi, watch_keys) > offset_of!(AmmApi, capture_stream_give_back));
    assert!(offset_of!(AmmApi, send_key) + size_of::<usize>() <= size_of::<AmmApi>());
}

/// The value of every variant of every enum that crosses the boundary. These bytes are a contract between a mod and a
/// manager built at different times: a variant keeps its value for good, and a new one only goes at the end.
#[test]
fn wire_values_are_stable() {
    let status = [
        AmmStatus::Ok,
        AmmStatus::NullPointer,
        AmmStatus::InvalidArgument,
        AmmStatus::InvalidDescriptor,
        AmmStatus::DuplicateId,
        AmmStatus::TooManyMods,
        AmmStatus::UnknownHandle,
        AmmStatus::NotPermitted,
        AmmStatus::QueueFull,
        AmmStatus::Panic,
        AmmStatus::Unsupported,
        AmmStatus::ManagerMissing,
        AmmStatus::Busy,
    ];
    assert_eq!(status.map(|value| value as i32), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    let event = [
        AmmEventKind::Enabled,
        AmmEventKind::Disabled,
        AmmEventKind::Granted,
        AmmEventKind::Revoked,
        AmmEventKind::Suspended,
        AmmEventKind::Resumed,
        AmmEventKind::CapabilityAvailable,
        AmmEventKind::Keybind,
        AmmEventKind::SettingsChanged,
        AmmEventKind::WorldReloadApply,
        AmmEventKind::Key,
        AmmEventKind::PointerButton,
        AmmEventKind::PointerDelta,
        AmmEventKind::Wheel,
        AmmEventKind::ScreenshotTaken,
        AmmEventKind::ScreenshotWritten,
        AmmEventKind::MenuEntryClicked,
        AmmEventKind::LocaleChanged,
        AmmEventKind::FakeKeysReleased,
    ];
    assert_eq!(event.map(|value| value as u8), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18]);
    let capability = [
        AmmCapability::Camera,
        AmmCapability::Input,
        AmmCapability::Hud,
        AmmCapability::Overlay,
        AmmCapability::WorldDraw,
        AmmCapability::RenderSize,
    ];
    assert_eq!(capability.map(|value| value as u8), [0, 1, 2, 3, 4, 5]);
    let outcome = [AmmAcquireOutcome::Granted, AmmAcquireOutcome::Denied, AmmAcquireOutcome::Preempted];
    assert_eq!(outcome.map(|value| value as u8), [0, 1, 2]);
    let shape = [AmmCursorShape::Default, AmmCursorShape::Arrow, AmmCursorShape::Hand, AmmCursorShape::Text];
    assert_eq!(shape.map(|value| value as u8), [0, 1, 2, 3]);
    assert_eq!([AmmSamplerKind::Linear, AmmSamplerKind::Nearest].map(|value| value as u8), [0, 1]);
    let ui = [
        AmmUiEventKind::PointerMove,
        AmmUiEventKind::PointerButton,
        AmmUiEventKind::Wheel,
        AmmUiEventKind::Key,
        AmmUiEventKind::Char,
        AmmUiEventKind::FocusLost,
    ];
    assert_eq!(ui.map(|value| value as u8), [0, 1, 2, 3, 4, 5]);
    let level = [AmmLogLevel::Error, AmmLogLevel::Warn, AmmLogLevel::Info, AmmLogLevel::Debug, AmmLogLevel::Trace];
    assert_eq!(level.map(|value| value as u8), [0, 1, 2, 3, 4]);
    let layers = [AmmScreenshotLayers::GameOnly, AmmScreenshotLayers::WithModLayers];
    assert_eq!(layers.map(|value| value as u8), [0, 1]);
    let fault = [
        AmmCaptureStreamFault::None,
        AmmCaptureStreamFault::CopyFailed,
        AmmCaptureStreamFault::UnsupportedFormat,
        AmmCaptureStreamFault::TimedOut,
        AmmCaptureStreamFault::DeviceLost,
        AmmCaptureStreamFault::NotPermitted,
        AmmCaptureStreamFault::Resized,
        AmmCaptureStreamFault::Missed,
    ];
    assert_eq!(fault.map(|value| value as u8), [0, 1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn event_layout_is_stable() {
    assert_eq!((size_of::<AmmEvent>(), align_of::<AmmEvent>()), (16, 4));
    assert_eq!(offset_of!(AmmEvent, kind), 0);
    assert_eq!(offset_of!(AmmEvent, capability), 1);
    assert_eq!(offset_of!(AmmEvent, other_mod), 2);
    assert_eq!(offset_of!(AmmEvent, phase), 3);
    assert_eq!(offset_of!(AmmEvent, code), 4);
    assert_eq!(offset_of!(AmmEvent, x), 8);
    assert_eq!(offset_of!(AmmEvent, y), 12);
}

#[test]
fn callbacks_layout_is_stable() {
    assert_eq!((size_of::<AmmCallbacks>(), align_of::<AmmCallbacks>()), (32, 8));
    assert_eq!(offset_of!(AmmCallbacks, size), 0);
    assert_eq!(offset_of!(AmmCallbacks, user), 8);
    assert_eq!(offset_of!(AmmCallbacks, frame), 16);
    assert_eq!(offset_of!(AmmCallbacks, textures_lost), 24);
}

#[test]
fn frame_input_layout_is_stable() {
    assert_eq!((size_of::<AmmFrameInput>(), align_of::<AmmFrameInput>()), (48, 8));
    assert_eq!(offset_of!(AmmFrameInput, size), 0);
    assert_eq!(offset_of!(AmmFrameInput, width), 4);
    assert_eq!(offset_of!(AmmFrameInput, height), 8);
    assert_eq!(offset_of!(AmmFrameInput, pixels_per_point), 12);
    assert_eq!(offset_of!(AmmFrameInput, time_secs), 16);
    assert_eq!(offset_of!(AmmFrameInput, events), 24);
    assert_eq!(offset_of!(AmmFrameInput, event_count), 32);
    assert_eq!(offset_of!(AmmFrameInput, allowed_layers), 40);
    assert_eq!(offset_of!(AmmFrameInput, input_flags), 41);
}

#[test]
fn mesh_view_layout_is_stable() {
    assert_eq!((size_of::<AmmMeshView>(), align_of::<AmmMeshView>()), (96, 8));
    assert_eq!(offset_of!(AmmMeshView, vertices), 0);
    assert_eq!(offset_of!(AmmMeshView, vertex_count), 8);
    assert_eq!(offset_of!(AmmMeshView, indices), 16);
    assert_eq!(offset_of!(AmmMeshView, index_count), 24);
    assert_eq!(offset_of!(AmmMeshView, cmds), 32);
    assert_eq!(offset_of!(AmmMeshView, cmd_count), 40);
    assert_eq!(offset_of!(AmmMeshView, deltas), 48);
    assert_eq!(offset_of!(AmmMeshView, delta_count), 56);
    assert_eq!(offset_of!(AmmMeshView, bytes), 64);
    assert_eq!(offset_of!(AmmMeshView, byte_count), 72);
    assert_eq!(offset_of!(AmmMeshView, freed), 80);
    assert_eq!(offset_of!(AmmMeshView, freed_count), 88);
}

#[test]
fn frame_output_layout_is_stable() {
    assert_eq!((size_of::<AmmFrameOutput>(), align_of::<AmmFrameOutput>()), (128, 8));
    assert_eq!(offset_of!(AmmFrameOutput, size), 0);
    assert_eq!(offset_of!(AmmFrameOutput, mesh), 8);
    assert_eq!(offset_of!(AmmFrameOutput, wants_pointer), 104);
    assert_eq!(offset_of!(AmmFrameOutput, wants_keyboard), 105);
    assert_eq!(offset_of!(AmmFrameOutput, cursor), 106);
    assert_eq!(offset_of!(AmmFrameOutput, layer_rect), 108);
}

/// Each sized struct starts with its size, its blank value says so, and the fields every 2.x version has fit in it.
#[test]
fn sized_structs_start_with_size_and_base_fits() {
    assert_eq!(offset_of!(AmmApi, size), 0);
    assert_eq!(offset_of!(AmmCallbacks, size), 0);
    assert_eq!(offset_of!(AmmFrameInput, size), 0);
    assert_eq!(offset_of!(AmmFrameOutput, size), 0);
    assert_eq!(offset_of!(AmmCaptureStreamRequest, size), 0);
    assert_eq!(offset_of!(AmmCaptureStreamFrame, size), 0);
    assert_eq!(offset_of!(AmmRenderSizeState, size), 0);
    assert_eq!((AmmApi::BASE_SIZE, AmmApi::blank().size as usize), (size_of::<AmmApi>(), size_of::<AmmApi>()));
    assert_eq!(
        (AmmCallbacks::BASE_SIZE, AmmCallbacks::blank().size as usize),
        (size_of::<AmmCallbacks>(), size_of::<AmmCallbacks>())
    );
    assert_eq!(
        (AmmFrameInput::BASE_SIZE, AmmFrameInput::blank().size as usize),
        (size_of::<AmmFrameInput>(), size_of::<AmmFrameInput>())
    );
    assert_eq!(
        (AmmFrameOutput::BASE_SIZE, AmmFrameOutput::blank().size as usize),
        (size_of::<AmmFrameOutput>(), size_of::<AmmFrameOutput>())
    );
    assert_eq!(
        (AmmCaptureStreamRequest::BASE_SIZE, AmmCaptureStreamRequest::blank().size as usize),
        (size_of::<AmmCaptureStreamRequest>(), size_of::<AmmCaptureStreamRequest>())
    );
    assert_eq!(
        (AmmCaptureStreamFrame::BASE_SIZE, AmmCaptureStreamFrame::blank().size as usize),
        (size_of::<AmmCaptureStreamFrame>(), size_of::<AmmCaptureStreamFrame>())
    );
    assert_eq!(
        (AmmRenderSizeState::BASE_SIZE, AmmRenderSizeState::blank().size as usize),
        (size_of::<AmmRenderSizeState>(), size_of::<AmmRenderSizeState>())
    );
}

#[test]
fn small_value_types_layout_is_stable() {
    assert_eq!((size_of::<AmmHandle>(), align_of::<AmmHandle>()), (4, 2));
    assert_eq!(offset_of!(AmmHandle, slot), 0);
    assert_eq!(offset_of!(AmmHandle, generation), 2);
    assert_eq!((size_of::<AmmAcquireResult>(), align_of::<AmmAcquireResult>()), (2, 1));
    assert_eq!(offset_of!(AmmAcquireResult, outcome), 0);
    assert_eq!(offset_of!(AmmAcquireResult, holder), 1);
    assert_eq!((size_of::<AmmRenderSize>(), align_of::<AmmRenderSize>()), (8, 4));
    assert_eq!(offset_of!(AmmRenderSize, width), 0);
    assert_eq!(offset_of!(AmmRenderSize, height), 4);
}

#[test]
fn vertex_layout_is_stable() {
    assert_eq!((size_of::<AmmVertex>(), align_of::<AmmVertex>()), (20, 4));
    assert_eq!(offset_of!(AmmVertex, pos), 0);
    assert_eq!(offset_of!(AmmVertex, uv), 8);
    assert_eq!(offset_of!(AmmVertex, color), 16);
}

#[test]
fn draw_cmd_layout_is_stable() {
    assert_eq!((size_of::<AmmDrawCmd>(), align_of::<AmmDrawCmd>()), (40, 8));
    assert_eq!(offset_of!(AmmDrawCmd, texture), 0);
    assert_eq!(offset_of!(AmmDrawCmd, index_offset), 8);
    assert_eq!(offset_of!(AmmDrawCmd, index_count), 12);
    assert_eq!(offset_of!(AmmDrawCmd, clip), 16);
    assert_eq!(offset_of!(AmmDrawCmd, shader), 32);
    assert_eq!(offset_of!(AmmDrawCmd, sampler), 33);
}

#[test]
fn texture_delta_layout_is_stable() {
    assert_eq!((size_of::<AmmTextureDelta>(), align_of::<AmmTextureDelta>()), (40, 8));
    assert_eq!(offset_of!(AmmTextureDelta, id), 0);
    assert_eq!(offset_of!(AmmTextureDelta, sampler), 8);
    assert_eq!(offset_of!(AmmTextureDelta, full), 9);
    assert_eq!(offset_of!(AmmTextureDelta, x), 12);
    assert_eq!(offset_of!(AmmTextureDelta, y), 16);
    assert_eq!(offset_of!(AmmTextureDelta, width), 20);
    assert_eq!(offset_of!(AmmTextureDelta, height), 24);
    assert_eq!(offset_of!(AmmTextureDelta, byte_offset), 28);
    assert_eq!(offset_of!(AmmTextureDelta, byte_len), 32);
}

#[test]
fn ui_event_layout_is_stable() {
    assert_eq!((size_of::<AmmUiEvent>(), align_of::<AmmUiEvent>()), (24, 4));
    assert_eq!(offset_of!(AmmUiEvent, kind), 0);
    assert_eq!(offset_of!(AmmUiEvent, x), 4);
    assert_eq!(offset_of!(AmmUiEvent, y), 8);
    assert_eq!(offset_of!(AmmUiEvent, key), 12);
    assert_eq!(offset_of!(AmmUiEvent, modifiers), 16);
    assert_eq!(offset_of!(AmmUiEvent, down), 17);
    assert_eq!(offset_of!(AmmUiEvent, ch), 20);
}

#[test]
fn abi_vertex_matches_core_vertex_layout() {
    assert_eq!(size_of::<AmmVertex>(), size_of::<Vertex>());
    assert_eq!(align_of::<AmmVertex>(), align_of::<Vertex>());
}

#[test]
fn abi_draw_cmd_matches_core_draw_cmd_layout() {
    assert_eq!(size_of::<AmmDrawCmd>(), size_of::<DrawCmd>());
    assert_eq!(align_of::<AmmDrawCmd>(), align_of::<DrawCmd>());
    assert_eq!(offset_of!(AmmDrawCmd, clip), offset_of!(DrawCmd, clip));
    assert_eq!(offset_of!(AmmDrawCmd, sampler), offset_of!(DrawCmd, sampler));
}

#[test]
fn abi_texture_delta_matches_core_texture_delta_layout() {
    assert_eq!(size_of::<AmmTextureDelta>(), size_of::<TextureDelta>());
    assert_eq!(align_of::<AmmTextureDelta>(), align_of::<TextureDelta>());
    assert_eq!(offset_of!(AmmTextureDelta, byte_len), offset_of!(TextureDelta, byte_len));
}

#[test]
fn abi_ui_event_matches_core_ui_event_layout() {
    assert_eq!(size_of::<AmmUiEvent>(), size_of::<UiEvent>());
    assert_eq!(align_of::<AmmUiEvent>(), align_of::<UiEvent>());
    assert_eq!(offset_of!(AmmUiEvent, ch), offset_of!(UiEvent, ch));
    assert_eq!(offset_of!(AmmUiEvent, down), offset_of!(UiEvent, down));
}

#[test]
fn abi_draw_cmd_and_ui_event_field_offsets_match_core() {
    assert_eq!(offset_of!(AmmDrawCmd, shader), offset_of!(DrawCmd, shader));
    assert_eq!(offset_of!(AmmDrawCmd, texture), offset_of!(DrawCmd, texture));
    assert_eq!(offset_of!(AmmUiEvent, key), offset_of!(UiEvent, key));
    assert_eq!(offset_of!(AmmUiEvent, modifiers), offset_of!(UiEvent, modifiers));
}

#[test]
fn enum_values_match_their_mirrors() {
    assert_eq!(SamplerKind::Linear as u8, AmmSamplerKind::Linear as u8);
    assert_eq!(SamplerKind::Nearest as u8, AmmSamplerKind::Nearest as u8);

    assert_eq!(UiEventKind::PointerMove as u8, AmmUiEventKind::PointerMove as u8);
    assert_eq!(UiEventKind::PointerButton as u8, AmmUiEventKind::PointerButton as u8);
    assert_eq!(UiEventKind::Wheel as u8, AmmUiEventKind::Wheel as u8);
    assert_eq!(UiEventKind::Key as u8, AmmUiEventKind::Key as u8);
    assert_eq!(UiEventKind::Char as u8, AmmUiEventKind::Char as u8);
    assert_eq!(UiEventKind::FocusLost as u8, AmmUiEventKind::FocusLost as u8);

    assert_eq!(CursorShape::Default as u8, AmmCursorShape::Default as u8);
    assert_eq!(CursorShape::Arrow as u8, AmmCursorShape::Arrow as u8);
    assert_eq!(CursorShape::Hand as u8, AmmCursorShape::Hand as u8);
    assert_eq!(CursorShape::Text as u8, AmmCursorShape::Text as u8);
}

#[test]
fn render_size_state_layout_is_stable() {
    assert_eq!(size_of::<AmmRenderSize>(), 8);
    assert_eq!((size_of::<AmmRenderSizeState>(), align_of::<AmmRenderSizeState>()), (24, 4));
    assert_eq!(offset_of!(AmmRenderSizeState, size), 0);
    assert_eq!(offset_of!(AmmRenderSizeState, wanted), 4);
    assert_eq!(offset_of!(AmmRenderSizeState, actual), 12);
    assert_eq!(offset_of!(AmmRenderSizeState, format), 20);
}

#[test]
fn capture_stream_request_layout_is_stable() {
    assert_eq!((size_of::<AmmCaptureStreamRequest>(), align_of::<AmmCaptureStreamRequest>()), (24, 8));
    assert_eq!(offset_of!(AmmCaptureStreamRequest, size), 0);
    assert_eq!(offset_of!(AmmCaptureStreamRequest, tag), 8);
    assert_eq!(offset_of!(AmmCaptureStreamRequest, present_seq), 16);
}

#[test]
fn capture_stream_frame_layout_is_stable() {
    assert_eq!((size_of::<AmmCaptureStreamFrame>(), align_of::<AmmCaptureStreamFrame>()), (72, 8));
    assert_eq!(offset_of!(AmmCaptureStreamFrame, size), 0);
    assert_eq!(offset_of!(AmmCaptureStreamFrame, tag), 8);
    assert_eq!(offset_of!(AmmCaptureStreamFrame, width), 16);
    assert_eq!(offset_of!(AmmCaptureStreamFrame, height), 20);
    assert_eq!(offset_of!(AmmCaptureStreamFrame, row_pitch), 24);
    assert_eq!(offset_of!(AmmCaptureStreamFrame, format), 28);
    assert_eq!(offset_of!(AmmCaptureStreamFrame, fault), 32);
    assert_eq!(offset_of!(AmmCaptureStreamFrame, pixels), 40);
    assert_eq!(offset_of!(AmmCaptureStreamFrame, pixels_len), 48);
    assert_eq!(offset_of!(AmmCaptureStreamFrame, present_seq), 56);
    assert_eq!(offset_of!(AmmCaptureStreamFrame, lend_id), 64);
}

/// A sized struct ends with its last field, not with padding: a field added by a later minor version then always
/// grows `size_of`, so the `size` a side declares tells which fields it has.
#[test]
fn sized_structs_end_without_tail_padding() {
    assert_eq!(offset_of!(AmmApi, send_key) + 8, size_of::<AmmApi>());
    assert_eq!(offset_of!(AmmCallbacks, textures_lost) + 8, size_of::<AmmCallbacks>());
    assert_eq!(offset_of!(AmmFrameInput, reserved) + 6, size_of::<AmmFrameInput>());
    assert_eq!(offset_of!(AmmFrameOutput, reserved) + 4, size_of::<AmmFrameOutput>());
    assert_eq!(offset_of!(AmmCaptureStreamRequest, present_seq) + 8, size_of::<AmmCaptureStreamRequest>());
    assert_eq!(offset_of!(AmmCaptureStreamFrame, lend_id) + 8, size_of::<AmmCaptureStreamFrame>());
    assert_eq!(offset_of!(AmmRenderSizeState, format) + 4, size_of::<AmmRenderSizeState>());
    assert_eq!(AmmFrameInput::blank().reserved, [0; 6]);
    assert_eq!(AmmFrameOutput::blank().reserved, [0; 4]);
}
