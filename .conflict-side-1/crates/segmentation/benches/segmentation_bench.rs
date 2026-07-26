use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use segmentation::Segmenter;

fn pcm_bytes_to_i16(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect()
}

fn bench_segmenter_init(c: &mut Criterion) {
    c.bench_function("segmenter_init", |b| {
        b.iter(|| black_box(Segmenter::new(16000).unwrap()))
    });
}

fn bench_segmenter_process_english_1(c: &mut Criterion) {
    let samples = pcm_bytes_to_i16(hypr_data::english_1::AUDIO);

    c.bench_function("segmenter_process english_1", |b| {
        b.iter_batched(
            || Segmenter::new(16000).unwrap(),
            |mut segmenter| black_box(segmenter.process(black_box(&samples), 16000).unwrap()),
            BatchSize::SmallInput,
        )
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10).noise_threshold(1.0);
    targets = bench_segmenter_init, bench_segmenter_process_english_1
}
criterion_main!(benches);
