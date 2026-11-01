//! The steady-state frame path of the host, counted with a global allocator that counts the frame thread only, so
//! the host's own background threads (I/O, log writers) cannot add to a measurement. This target is its own binary,
//! so the allocator does not touch any other test.
#![allow(unsafe_code)]

mod zero_alloc_support;

use amm_bench::fixtures::{BenchHost, EguiProbe, PanelRig};
use zero_alloc_support::ThreadAlloc;

#[global_allocator]
static GLOBAL: ThreadAlloc = ThreadAlloc;

const CLIENTS: usize = 4;
const TWO_LAYER_CLIENTS: usize = 2;
const VERTICES_PER_CLIENT: usize = 10_000;
const WARMUP_FRAMES: u64 = 16;
const MEASURED_FRAMES: u64 = 1_000;
const FRAME_STEP_MS: u64 = 16;

#[test]
fn four_clients_of_ten_thousand_vertices_allocate_nothing_in_a_steady_frame() {
    let host = BenchHost::with_overlay_clients(CLIENTS, VERTICES_PER_CLIENT);
    let mut now_ms = 0;
    for _ in 0..WARMUP_FRAMES {
        now_ms += FRAME_STEP_MS;
        host.frame(now_ms);
    }

    let start = ThreadAlloc::current();
    let mut layers = 0;
    for _ in 0..MEASURED_FRAMES {
        now_ms += FRAME_STEP_MS;
        layers = host.frame(now_ms).layer_count;
    }
    let change = ThreadAlloc::since(start);

    assert_eq!(layers as usize, CLIENTS);
    assert_eq!(host.vertices_per_frame(), CLIENTS * VERTICES_PER_CLIENT);
    assert_eq!(
        (change.allocations, change.reallocations, change.bytes_allocated),
        (0, 0, 0),
        "{MEASURED_FRAMES} steady frames allocated"
    );
}

#[test]
fn two_clients_with_hud_and_overlay_layers_allocate_nothing_in_a_steady_frame() {
    let host = BenchHost::with_two_layer_clients(TWO_LAYER_CLIENTS, VERTICES_PER_CLIENT);
    let mut now_ms = 0;
    for _ in 0..WARMUP_FRAMES {
        now_ms += FRAME_STEP_MS;
        host.frame(now_ms);
    }

    let start = ThreadAlloc::current();
    let mut layers = 0;
    for _ in 0..MEASURED_FRAMES {
        now_ms += FRAME_STEP_MS;
        layers = host.frame(now_ms).layer_count;
    }
    let change = ThreadAlloc::since(start);

    assert_eq!(layers as usize, 2 * TWO_LAYER_CLIENTS);
    assert_eq!(
        (change.allocations, change.reallocations, change.bytes_allocated),
        (0, 0, 0),
        "{MEASURED_FRAMES} steady two-layer frames allocated"
    );
}

#[test]
fn one_egui_client_pass_allocation_count_is_reported() {
    let probe = EguiProbe::new();
    let mut now_ms = 0;
    for _ in 0..WARMUP_FRAMES {
        now_ms += FRAME_STEP_MS;
        probe.host().frame(now_ms);
    }

    let start = ThreadAlloc::current();
    for _ in 0..MEASURED_FRAMES {
        now_ms += FRAME_STEP_MS;
        probe.host().frame(now_ms);
    }
    let change = ThreadAlloc::since(start);

    let per_frame = change.allocations as f64 / MEASURED_FRAMES as f64;
    eprintln!(
        "egui pass, trivial handler: {} allocations, {} reallocations, {} bytes over {MEASURED_FRAMES} frames \
         ({per_frame:.1} allocations per frame)",
        change.allocations, change.reallocations, change.bytes_allocated
    );
    assert!(change.allocations > 0, "the egui client pass is expected to allocate, the probe measured nothing");
}

#[test]
fn a_world_draw_client_without_a_frame_callback_allocates_nothing_in_a_steady_frame() {
    let host = BenchHost::with_world_draw_client_without_frame();
    let mut now_ms = 0;
    for _ in 0..WARMUP_FRAMES {
        now_ms += FRAME_STEP_MS;
        host.frame(now_ms);
    }

    let start = ThreadAlloc::current();
    let mut layers = u32::MAX;
    for _ in 0..MEASURED_FRAMES {
        now_ms += FRAME_STEP_MS;
        layers = host.frame(now_ms).layer_count;
    }
    let change = ThreadAlloc::since(start);

    assert_eq!(layers, 0);
    assert_eq!(
        (change.allocations, change.reallocations, change.bytes_allocated),
        (0, 0, 0),
        "{MEASURED_FRAMES} steady frames without a frame callback allocated"
    );
}

const PANEL_FRAMES: u64 = 200;
// Allocations a steady frame adds for four mod icons. A shape on another texture id starts its own egui mesh with
// its own vectors; the icon brush is shared, not built per frame. Measured: 274 with four icons, 264 without.
const ICON_FRAME_ALLOCATIONS: usize = 10;

// The allocations of each of `PANEL_FRAMES` passes of `rig`, after the warm-up.
struct PanelCounts;

impl PanelCounts {
    fn per_frame(rig: &mut PanelRig) -> Vec<usize> {
        rig.run(WARMUP_FRAMES);
        let mut counts = Vec::with_capacity(PANEL_FRAMES as usize);
        for _ in 0..PANEL_FRAMES {
            let start = ThreadAlloc::current();
            rig.run(1);
            counts.push(ThreadAlloc::since(start).allocations);
        }
        counts
    }
}

#[test]
fn panel_frames_with_steady_icons_allocate_a_constant_amount_per_frame() {
    let mut without = PanelRig::new();
    let mut with = PanelRig::new();
    with.set_icons();

    let a = PanelCounts::per_frame(&mut without);
    let b = PanelCounts::per_frame(&mut with);

    eprintln!("panel allocations per frame: without icons {}, with icons {}", a[0], b[0]);
    // The icon textures are synced once; a steady frame draws them with the same work every pass, so the count
    // per frame does not move and does not grow with the frames run.
    assert!(a.iter().all(|count| *count == a[0]), "panel frames without icons vary: {a:?}");
    assert!(b.iter().all(|count| *count == b[0]), "panel frames with icons vary: {b:?}");
    assert!(
        b[0] <= a[0] + ICON_FRAME_ALLOCATIONS,
        "four icons cost more allocations per frame than allowed: {} with icons, {} without, {ICON_FRAME_ALLOCATIONS} allowed",
        b[0],
        a[0]
    );
}
