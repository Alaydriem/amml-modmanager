use std::hint::black_box;

use amm_bench::fixtures::EguiProbe;
use criterion::{Criterion, criterion_group, criterion_main};

// One host frame with one mod linked through amm-client: the frame callback, its input read and its output write
// across the boundary, and one small egui pass.

const FRAME_STEP_MS: u64 = 16;
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

fn client_frame(c: &mut Criterion) {
    let probe = EguiProbe::new();
    let mut now_ms = 0;
    let mut group = c.benchmark_group("client_frame");
    group.bench_function("one_overlay_layer_1920x1080", |b| {
        b.iter(|| {
            now_ms += FRAME_STEP_MS;
            black_box(probe.host().frame_at(now_ms, WIDTH, HEIGHT))
        })
    });
    group.finish();
}

criterion_group!(benches, client_frame);
criterion_main!(benches);
