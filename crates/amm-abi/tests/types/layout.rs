use std::mem::{align_of, offset_of, size_of};

use amm_abi::types::{
    AMM_ABI_MAJOR, AMM_ABI_MINOR, AmmAcquireOutcome, AmmAcquireResult, AmmCallbacks, AmmCapability, AmmCoreTable,
    AmmCursorShape, AmmDrawCmd, AmmEvent, AmmEventKind, AmmFrameInput, AmmFrameOutput, AmmHandle, AmmKeybindsTable,
    AmmLayersTable, AmmLeasesTable, AmmLogLevel, AmmMeshView, AmmRenderSize, AmmRenderSizeState, AmmSamplerKind,
    AmmSettingsTable, AmmStatus, AmmTextureDelta, AmmUiEvent, AmmUiEventKind, AmmVertex,
};
use amm_core::structs::frame::{CursorShape, UiEvent, UiEventKind};
use amm_core::structs::mesh::{DrawCmd, SamplerKind, TextureDelta, Vertex};
use amm_core::traits::AbiSized;
use amml_api::Provided;

const HEADER: usize = 8;
const SLOT: usize = 8;

#[test]
fn every_table_starts_with_size_and_version() {
    assert_eq!((offset_of!(AmmCoreTable, size), offset_of!(AmmCoreTable, version)), (0, 4));
    assert_eq!((offset_of!(AmmSettingsTable, size), offset_of!(AmmSettingsTable, version)), (0, 4));
    assert_eq!((offset_of!(AmmKeybindsTable, size), offset_of!(AmmKeybindsTable, version)), (0, 4));
    assert_eq!((offset_of!(AmmLayersTable, size), offset_of!(AmmLayersTable, version)), (0, 4));
    assert_eq!((offset_of!(AmmLeasesTable, size), offset_of!(AmmLeasesTable, version)), (0, 4));
}

#[test]
fn table_sizes_are_pinned() {
    assert_eq!(size_of::<AmmCoreTable>(), HEADER + 7 * SLOT);
    assert_eq!(size_of::<AmmSettingsTable>(), HEADER + 2 * SLOT);
    assert_eq!(size_of::<AmmKeybindsTable>(), HEADER + 2 * SLOT);
    assert_eq!(size_of::<AmmLayersTable>(), HEADER + 7 * SLOT);
    assert_eq!(size_of::<AmmLeasesTable>(), HEADER + 4 * SLOT);
}

/// The slot order of each table is the contract a consumer built at another time reads: a slot keeps its offset for
/// good, and a new one only goes at the end.
#[test]
fn table_slot_offsets_are_stable() {
    let core = [
        offset_of!(AmmCoreTable, register),
        offset_of!(AmmCoreTable, unregister),
        offset_of!(AmmCoreTable, poll_events),
        offset_of!(AmmCoreTable, locale),
        offset_of!(AmmCoreTable, panel_open),
        offset_of!(AmmCoreTable, register_menu_entry),
        offset_of!(AmmCoreTable, set_icon),
    ];
    let layers = [
        offset_of!(AmmLayersTable, set_callbacks),
        offset_of!(AmmLayersTable, texture_budget),
        offset_of!(AmmLayersTable, set_cursor_mode),
        offset_of!(AmmLayersTable, game_picture),
        offset_of!(AmmLayersTable, set_game_viewport),
        offset_of!(AmmLayersTable, set_render_size),
        offset_of!(AmmLayersTable, render_size),
    ];
    let leases = [
        offset_of!(AmmLeasesTable, acquire),
        offset_of!(AmmLeasesTable, release),
        offset_of!(AmmLeasesTable, holder),
        offset_of!(AmmLeasesTable, held_by_other),
    ];
    let settings = [offset_of!(AmmSettingsTable, config_document), offset_of!(AmmSettingsTable, set_setting)];
    let keybinds = [offset_of!(AmmKeybindsTable, chord_text), offset_of!(AmmKeybindsTable, bind_down)];
    let expected = |slots: usize| (0..slots).map(|slot| HEADER + slot * SLOT).collect::<Vec<_>>();
    assert_eq!(core.to_vec(), expected(7));
    assert_eq!(layers.to_vec(), expected(7));
    assert_eq!(leases.to_vec(), expected(4));
    assert_eq!(settings.to_vec(), expected(2));
    assert_eq!(keybinds.to_vec(), expected(2));
}

#[test]
fn the_tables_are_provided_under_their_names_at_the_abi_version() {
    assert_eq!(
        [
            AmmCoreTable::NAME,
            AmmSettingsTable::NAME,
            AmmKeybindsTable::NAME,
            AmmLayersTable::NAME,
            AmmLeasesTable::NAME
        ],
        [
            "alaydriem.modmanager.core",
            "alaydriem.modmanager.settings",
            "alaydriem.modmanager.keybinds",
            "alaydriem.modmanager.layers",
            "alaydriem.modmanager.leases",
        ]
    );
    let versions = [
        (AmmCoreTable::MAJOR, AmmCoreTable::MINOR),
        (AmmSettingsTable::MAJOR, AmmSettingsTable::MINOR),
        (AmmKeybindsTable::MAJOR, AmmKeybindsTable::MINOR),
        (AmmLayersTable::MAJOR, AmmLayersTable::MINOR),
        (AmmLeasesTable::MAJOR, AmmLeasesTable::MINOR),
    ];
    assert_eq!(versions, [(3, 0); 5]);
    assert_eq!((AMM_ABI_MAJOR, AMM_ABI_MINOR), (3, 0));
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
        AmmStatus::NotReady,
        AmmStatus::Disabled,
    ];
    assert_eq!(status.map(|value| value as i32), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
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
        AmmEventKind::MenuEntryClicked,
        AmmEventKind::LocaleChanged,
    ];
    assert_eq!(event.map(|value| value as u8), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 16, 17]);
    let capability = [
        AmmCapability::Camera,
        AmmCapability::Hud,
        AmmCapability::Overlay,
        AmmCapability::WorldDraw,
        AmmCapability::RenderSize,
    ];
    assert_eq!(capability.map(|value| value as u8), [0, 2, 3, 4, 5]);
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

/// Each sized struct starts with its size, its blank value says so, and the fields every 3.x version has fit in it.
#[test]
fn sized_structs_start_with_size_and_base_fits() {
    assert_eq!(offset_of!(AmmCallbacks, size), 0);
    assert_eq!(offset_of!(AmmFrameInput, size), 0);
    assert_eq!(offset_of!(AmmFrameOutput, size), 0);
    assert_eq!(offset_of!(AmmRenderSizeState, size), 0);
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

/// A sized struct ends with its last field, not with padding: a field added by a later minor version then always
/// grows `size_of`, so the `size` a side declares tells which fields it has.
#[test]
fn sized_structs_end_without_tail_padding() {
    assert_eq!(offset_of!(AmmCallbacks, textures_lost) + 8, size_of::<AmmCallbacks>());
    assert_eq!(offset_of!(AmmFrameInput, reserved) + 6, size_of::<AmmFrameInput>());
    assert_eq!(offset_of!(AmmFrameOutput, reserved) + 4, size_of::<AmmFrameOutput>());
    assert_eq!(offset_of!(AmmRenderSizeState, format) + 4, size_of::<AmmRenderSizeState>());
    assert_eq!(AmmFrameInput::blank().reserved, [0; 6]);
    assert_eq!(AmmFrameOutput::blank().reserved, [0; 4]);
}
