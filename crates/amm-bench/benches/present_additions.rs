use std::hint::black_box;

use amm_bench::fixtures::BenchHost;
use criterion::{Criterion, criterion_group, criterion_main};

// The per-Present work of the manager: the host frame a `SurfaceRenderer` asks for, idle and while a render size is
// forced (the locked path).

const FRAME_STEP_MS: u64 = 16;
const UHD_WIDTH: u32 = 3840;
const UHD_HEIGHT: u32 = 2160;

fn host_frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("host_frame");
    {
        let host = BenchHost::new();
        let mut now_ms = 0;
        group.bench_function("idle_3840x2160", |b| {
            b.iter(|| {
                now_ms += FRAME_STEP_MS;
                black_box(host.frame_at(now_ms, UHD_WIDTH, UHD_HEIGHT))
            })
        });
    }
    {
        let host = BenchHost::with_forced_size(UHD_WIDTH, UHD_HEIGHT);
        let mut now_ms = 0;
        group.bench_function("forced_size_3840x2160", |b| {
            b.iter(|| {
                now_ms += FRAME_STEP_MS;
                black_box(host.frame_at(now_ms, UHD_WIDTH, UHD_HEIGHT))
            })
        });
    }
    group.finish();
}

criterion_group!(benches, host_frame);
criterion_main!(benches);
