use std::time::Duration;

use amm_bench::fixtures::CaptureRig;
use criterion::{Criterion, criterion_group, criterion_main};

fn capture_buffer(c: &mut Criterion) {
    let mut rig = CaptureRig::new();
    let mut group = c.benchmark_group("capture_buffer");
    group.sample_size(30);
    group.bench_function("buffer_copy_done_3840x2160", |b| {
        b.iter_custom(|iterations| (0..iterations).map(|_| rig.take_one()).sum::<Duration>())
    });
    group.finish();
}

criterion_group!(benches, capture_buffer);
criterion_main!(benches);
