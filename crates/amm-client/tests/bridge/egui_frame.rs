use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use amm_abi::types::{
    AmmCursorShape, AmmDrawCmd, AmmFrameInput, AmmStatus, AmmTextureDelta, AmmUiEvent, AmmUiEventKind, AmmVertex,
};
use amm_client::bridge::{ClipboardBackend, EguiFrame, FrameTrampoline};
use amm_client::link::FrameHandler;
use amm_core::structs::frame::{FrameInputData, LayerKind};
use amm_core::structs::mesh::DrawCmd;
use amml_ui::font::FontFace;
use egui::{FontFamily, Rect, pos2, vec2};

use crate::support::{
    ClipboardHold, ClipboardPass, ClipboardProbeHandler, ContextProbeHandler, FakeTable, HoldingHandler,
    LayerProbeHandler, LayerSeen, OverlayPanickingHandler, RectHandler,
};

fn layer_input(layer: u8, events: &[AmmUiEvent]) -> AmmFrameInput {
    AmmFrameInput {
        size: size_of::<AmmFrameInput>() as u32,
        reserved: [0; 6],
        width: 640,
        height: 480,
        pixels_per_point: 1.0,
        time_secs: 0.0,
        events: if events.is_empty() { std::ptr::null() } else { events.as_ptr() },
        event_count: events.len(),
        allowed_layers: layer,
        input_flags: 7,
    }
}

fn view_slice<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if len == 0 || ptr.is_null() {
        return &[];
    }
    // SAFETY: the view points at the frame's own buffers, which live until that layer's next run
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

// The managed texture ids one layer call uploads, frees and draws with, and its count of full uploads.
fn run_layer<H: FrameHandler>(frame: &mut EguiFrame<H>, layer: u8) -> (Vec<u64>, usize) {
    let mut out = FakeTable::empty_output();
    // SAFETY: the input has no events, so it points at nothing
    let status = unsafe { frame.run(&layer_input(layer, &[]), &mut out) };
    assert_eq!(status, AmmStatus::Ok);
    let view = out.mesh;
    let deltas: &[AmmTextureDelta] = view_slice(view.deltas, view.delta_count);
    let cmds: &[AmmDrawCmd] = view_slice(view.cmds, view.cmd_count);
    let freed: &[u64] = view_slice(view.freed, view.freed_count);
    let mut ids: Vec<u64> = deltas.iter().map(|d| d.id).collect();
    ids.extend(freed);
    ids.extend(cmds.iter().map(|c| c.texture).filter(|t| t & DrawCmd::USER_TEXTURE_FLAG == 0));
    let full = deltas.iter().filter(|d| d.full != 0).count();
    (ids, full)
}

fn rect_frame() -> EguiFrame<RectHandler> {
    EguiFrame::new(RectHandler { rect: Rect::from_min_size(pos2(10.0, 20.0), vec2(30.0, 40.0)) })
}

#[test]
fn frame_output_points_at_physical_pixel_vertices() {
    let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(30.0, 40.0));
    let mut frame = EguiFrame::new(RectHandler { rect });
    let input = AmmFrameInput {
        size: size_of::<AmmFrameInput>() as u32,
        reserved: [0; 6],
        width: 640,
        height: 480,
        pixels_per_point: 2.0,
        time_secs: 0.0,
        events: std::ptr::null(),
        event_count: 0,
        allowed_layers: 3,
        input_flags: 7,
    };
    let mut out = FakeTable::empty_output();

    // SAFETY: the input has no events, so it points at nothing
    let status = unsafe { frame.run(&input, &mut out) };

    assert_eq!(status, AmmStatus::Ok);
    assert!(out.mesh.vertex_count > 0);
    // SAFETY: the view points at the frame's own buffers, which live until its next run
    let vertices: &[AmmVertex] = unsafe { std::slice::from_raw_parts(out.mesh.vertices, out.mesh.vertex_count) };
    let red: Vec<[f32; 2]> = vertices.iter().filter(|v| v.color == [255, 0, 0, 255]).map(|v| v.pos).collect();
    assert!(!red.is_empty());
    let min_x = red.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
    let min_y = red.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    let max_x = red.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
    let max_y = red.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    assert!((min_x - 20.0).abs() <= 1.0 && (min_y - 40.0).abs() <= 1.0, "min ({min_x}, {min_y})");
    assert!((max_x - 80.0).abs() <= 1.0 && (max_y - 120.0).abs() <= 1.0, "max ({max_x}, {max_y})");
    assert_eq!(out.layer_rect, [20.0, 40.0, 80.0, 120.0]);
    assert_eq!(out.cursor, AmmCursorShape::Hand as u8);
    assert!(out.wants_pointer <= 1 && out.wants_keyboard <= 1);
    assert!(out.mesh.delta_count > 0, "the first pass uploads the font atlas");
}

#[test]
fn a_layer_context_has_the_amml_ui_faces_only() {
    let handler = ContextProbeHandler::default();
    let families = Arc::clone(&handler.families);
    let mut frame = EguiFrame::new(handler);

    run_layer(&mut frame, FrameInputData::LAYER_HUD);

    let families = families.lock().map(|families| families.clone()).unwrap_or_default();
    assert!(families.contains(&FontFace::Semibold.family()), "{families:?}");
    assert!(
        !families.contains(&FontFamily::Name("ui-semibold".into())),
        "a font family from before amml-ui is installed"
    );
}

#[test]
fn new_context_runs_once_per_context_and_again_after_textures_lost() {
    let handler = ContextProbeHandler::default();
    let count = Arc::clone(&handler.new_contexts);
    let mut frame = EguiFrame::new(handler);
    let seen = |count: &Arc<Mutex<usize>>| count.lock().map(|count| *count).unwrap_or_default();

    run_layer(&mut frame, FrameInputData::LAYER_HUD);
    let first = seen(&count);
    run_layer(&mut frame, FrameInputData::LAYER_HUD);
    let steady = seen(&count);
    frame.textures_lost();
    run_layer(&mut frame, FrameInputData::LAYER_HUD);

    assert_eq!((first, steady, seen(&count)), (1, 1, 2));
}

#[test]
fn two_layers_of_one_client_never_send_the_same_texture_id() {
    let mut frame = rect_frame();

    let (hud, hud_full) = run_layer(&mut frame, FrameInputData::LAYER_HUD);
    let (overlay, overlay_full) = run_layer(&mut frame, FrameInputData::LAYER_OVERLAY);

    assert!(hud_full > 0 && overlay_full > 0, "each layer context uploads its own font atlas");
    let hud: HashSet<u64> = hud.into_iter().collect();
    let overlay: HashSet<u64> = overlay.into_iter().collect();
    assert!(!hud.is_empty() && !overlay.is_empty());
    assert!(hud.is_disjoint(&overlay), "hud {hud:?} overlay {overlay:?}");
}

#[test]
fn textures_lost_resends_the_font_atlas_of_both_layers() {
    let mut frame = rect_frame();
    let (hud_first, _) = run_layer(&mut frame, FrameInputData::LAYER_HUD);
    let (overlay_first, _) = run_layer(&mut frame, FrameInputData::LAYER_OVERLAY);
    let (_, hud_steady) = run_layer(&mut frame, FrameInputData::LAYER_HUD);
    let (_, overlay_steady) = run_layer(&mut frame, FrameInputData::LAYER_OVERLAY);

    frame.textures_lost();
    let (hud_again, hud_full) = run_layer(&mut frame, FrameInputData::LAYER_HUD);
    let (overlay_again, overlay_full) = run_layer(&mut frame, FrameInputData::LAYER_OVERLAY);

    assert_eq!((hud_steady, overlay_steady), (0, 0), "a steady frame uploads nothing in full");
    assert!(hud_full > 0 && overlay_full > 0, "both layers send their font atlas again");
    assert_eq!(hud_again[0], hud_first[0], "the hud atlas comes back under its old id");
    assert_eq!(overlay_again[0], overlay_first[0], "the overlay atlas comes back under its old id");
}

#[test]
fn raw_input_reaches_only_the_layer_that_has_the_pointer() {
    let (handler, seen) = LayerProbeHandler::new();
    let mut frame = EguiFrame::new(handler);
    let event = |kind, down| AmmUiEvent { kind, x: 100.0, y: 60.0, key: 0, modifiers: 0, down, ch: 0 };
    let routed = [event(AmmUiEventKind::PointerMove, false), event(AmmUiEventKind::PointerButton, true)];
    let mut out = FakeTable::empty_output();

    // SAFETY: the events live for the call and the count matches them
    let hud = unsafe { frame.run(&layer_input(FrameInputData::LAYER_HUD, &routed), &mut out) };
    // SAFETY: the input has no events, so it points at nothing
    let overlay = unsafe { frame.run(&layer_input(FrameInputData::LAYER_OVERLAY, &[]), &mut out) };

    assert_eq!((hud, overlay), (AmmStatus::Ok, AmmStatus::Ok));
    let seen = seen.borrow();
    let (hud, overlay) = (seen[LayerKind::Hud.slot()], seen[LayerKind::Overlay.slot()]);
    assert_eq!((hud.passes, overlay.passes), (1, 1));
    assert_eq!(hud.pointer, Some(pos2(100.0, 60.0)));
    assert!(hud.primary_down);
    assert_eq!(overlay, LayerSeen { pointer: None, primary_down: false, passes: 1 });
}

#[test]
fn a_panic_in_one_layer_resets_only_that_layer() {
    let (handler, armed) = OverlayPanickingHandler::new();
    let user = FrameTrampoline::<OverlayPanickingHandler>::into_user(EguiFrame::new(handler));
    let frame = FrameTrampoline::<OverlayPanickingHandler>::callbacks(user).frame;
    let Some(frame) = frame else { panic!("no frame callback") };
    let full_uploads = |layer: u8| {
        let mut out = FakeTable::empty_output();
        let input = layer_input(layer, &[]);
        // SAFETY: user is the live box from into_user, input has no events and out lives for the call
        let status = unsafe { frame(user, &input, &mut out) };
        let deltas: &[AmmTextureDelta] = view_slice(out.mesh.deltas, out.mesh.delta_count);
        (status, deltas.iter().filter(|d| d.full != 0).count())
    };
    full_uploads(FrameInputData::LAYER_HUD);
    full_uploads(FrameInputData::LAYER_OVERLAY);
    assert_eq!(full_uploads(FrameInputData::LAYER_HUD), (AmmStatus::Ok as i32, 0));
    assert_eq!(full_uploads(FrameInputData::LAYER_OVERLAY), (AmmStatus::Ok as i32, 0));

    armed.set(true);
    let (status, _) = full_uploads(FrameInputData::LAYER_OVERLAY);
    armed.set(false);
    assert_eq!(status, AmmStatus::Panic as i32);

    let (hud_status, hud_full) = full_uploads(FrameInputData::LAYER_HUD);
    let (overlay_status, overlay_full) = full_uploads(FrameInputData::LAYER_OVERLAY);
    // SAFETY: user is the box from into_user, freed once with no call in flight
    unsafe { FrameTrampoline::<OverlayPanickingHandler>::free(user) };
    assert_eq!((hud_status, overlay_status), (AmmStatus::Ok as i32, AmmStatus::Ok as i32));
    assert_eq!(hud_full, 0, "the hud layer keeps its font atlas");
    assert!(overlay_full > 0, "the overlay layer sends its font atlas again");
}

#[test]
fn dropping_the_frame_joins_the_worker() {
    let memory = Arc::new(Mutex::new(String::new()));
    let (probe, action, text) = ClipboardProbeHandler::new("first");
    let handler = HoldingHandler::new(probe, ClipboardHold::new(&memory));
    let mut frame = EguiFrame::with_clipboard(handler, ClipboardBackend::Blocking(Arc::clone(&memory)));
    ClipboardPass::focus(&mut frame, &action);
    ClipboardPass::select_all(&mut frame);
    ClipboardPass::ctrl_c(&mut frame);
    // the worker took the first copy and waits inside it for the lock the handler holds
    assert!(ClipboardPass::wait_for_empty_queue(frame.clipboard()));
    text.replace("second".to_owned());
    ClipboardPass::run(&mut frame, &[]);
    ClipboardPass::select_all(&mut frame);
    ClipboardPass::ctrl_c(&mut frame);
    assert_eq!(frame.clipboard().pending(), 1, "the second copy waits in the queue");

    drop(frame);

    // the first copy ended when the handler let go of the lock; the waiting one never ran
    assert_eq!(memory.lock().map(|text| text.clone()).unwrap_or_default(), "first");
}
