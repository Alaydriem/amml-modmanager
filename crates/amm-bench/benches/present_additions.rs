use std::hint::black_box;
use std::time::Duration;

use amm_bench::fixtures::{BenchHost, ClientRectProbe, StreamRig};
use criterion::{Criterion, criterion_group, criterion_main};

// The per-Present work the video render added: the client size read, the host frame while a render size is forced
// (the locked path), and the copy of one 4K capture stream frame.

const FRAME_STEP_MS: u64 = 16;
const UHD_WIDTH: u32 = 3840;
const UHD_HEIGHT: u32 = 2160;

fn client_rect(c: &mut Criterion) {
    let probe = ClientRectProbe::new();
    let mut group = c.benchmark_group("present_input");
    group.bench_function("get_client_rect", |b| b.iter(|| black_box(probe.read())));
    group.finish();
}

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

fn stream_copy(c: &mut Criterion) {
    let mut rig = StreamRig::new();
    let mut group = c.benchmark_group("capture_stream");
    group.sample_size(30);
    group.bench_function("buffer_copy_done_3840x2160", |b| {
        b.iter_custom(|iterations| (0..iterations).map(|_| rig.take_one()).sum::<Duration>())
    });
    group.finish();
}

criterion_group!(benches, client_rect, host_frame, stream_copy);
criterion_main!(benches);
