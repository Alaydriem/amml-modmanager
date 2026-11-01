use std::hint::black_box;

use amm_bench::fixtures::{BenchHost, ChordTable, EightMods};
use criterion::{Criterion, criterion_group, criterion_main};

const CLIENTS: usize = 4;
const VERTICES_PER_CLIENT: usize = 10_000;
const FRAME_STEP_MS: u64 = 16;
// A time inside the toast lifetime, so the one toast stays on screen for the whole run.
const TOAST_NOW_MS: u64 = 1_000;

fn route_event(c: &mut Criterion) {
    let mut mods = EightMods::new();
    let mut group = c.benchmark_group("route_event");
    let mut down = true;
    group.bench_function("bound_key", |b| {
        b.iter(|| {
            down = !down;
            black_box(mods.route_key(black_box(EightMods::VK_B), down, 0))
        })
    });
    group.bench_function("unbound_key", |b| {
        b.iter(|| {
            down = !down;
            black_box(mods.route_key(black_box(EightMods::VK_Q), down, 0))
        })
    });
    group.finish();
}

fn frame_snapshot(c: &mut Criterion) {
    let mut mods = EightMods::new();
    let mut group = c.benchmark_group("frame_snapshot");
    group.bench_function("read", |b| b.iter(|| black_box(mods.snapshot())));
    group.bench_function("publish_then_read", |b| b.iter(|| black_box(mods.publish_then_snapshot())));
    group.bench_function("change_then_read", |b| b.iter(|| black_box(mods.change_then_snapshot(0))));
    group.finish();
}

fn merge_meshes(c: &mut Criterion) {
    let host = BenchHost::with_overlay_clients(CLIENTS, VERTICES_PER_CLIENT);
    let mut now_ms = 0;
    let mut group = c.benchmark_group("merge_meshes");
    group.bench_function("4_clients_x_10k_vertices", |b| {
        b.iter(|| {
            now_ms += FRAME_STEP_MS;
            black_box(host.frame(now_ms))
        })
    });
    group.finish();
}

fn panel_closed(c: &mut Criterion) {
    let host = BenchHost::with_denied_camera();
    let mut group = c.benchmark_group("panel_closed");
    group.bench_function("one_toast", |b| b.iter(|| black_box(host.frame(black_box(TOAST_NOW_MS)))));
    group.finish();
}

fn match_chord(c: &mut Criterion) {
    let table = ChordTable::new();
    let mut group = c.benchmark_group("match_chord");
    group.bench_function("hit_64_binds", |b| b.iter(|| black_box(table.count(black_box(table.hit())))));
    group.bench_function("miss_64_binds", |b| b.iter(|| black_box(table.count(black_box(table.miss())))));
    group.finish();
}

criterion_group!(benches, route_event, frame_snapshot, merge_meshes, panel_closed, match_chord);
criterion_main!(benches);
