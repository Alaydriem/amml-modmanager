use std::hint::black_box;

use amm_bench::fixtures::IconPng;
use amm_core::services::icon::IconDecoder;
use criterion::{Criterion, criterion_group, criterion_main};

const SIDE: u32 = 256;

fn icon_decode(c: &mut Criterion) {
    let png = IconPng::square(SIDE);
    assert!(IconDecoder::decode(&png).is_ok(), "the fixture PNG must decode, or the bench times the rejection path");
    c.bench_function("icon_decode_256", |b| b.iter(|| black_box(IconDecoder::decode(black_box(&png)))));
}

criterion_group!(benches, icon_decode);
criterion_main!(benches);
