use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use denoise::onnx::Denoiser;

fn pcm_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
        .collect()
}

fn bench_denoise_initialization(c: &mut Criterion) {
    c.bench_function("denoise_initialization", |b| {
        b.iter(|| black_box(Denoiser::new().unwrap()))
    });
}

fn bench_denoise_process(c: &mut Criterion) {
    let samples = pcm_bytes_to_f32(hypr_data::english_1::AUDIO);
    let mut denoiser = Denoiser::new().unwrap();

    c.bench_function("denoise_process_full", |b| {
        b.iter(|| black_box(denoiser.process(black_box(&samples)).unwrap()))
    });
}

fn bench_denoise_process_chunks(c: &mut Criterion) {
    let samples = pcm_bytes_to_f32(hypr_data::english_1::AUDIO);
    let mut denoiser = Denoiser::new().unwrap();

    let chunk_sizes = [1024, 4096, 16384];

    for &chunk_size in &chunk_sizes {
        let chunk = &samples[..chunk_size.min(samples.len())];

        c.bench_function(&format!("denoise_process_chunk_{}", chunk_size), |b| {
            b.iter(|| black_box(denoiser.process(black_box(chunk)).unwrap()))
        });
    }
}

fn bench_denoise_throughput(c: &mut Criterion) {
    let samples = pcm_bytes_to_f32(hypr_data::english_1::AUDIO);
    let mut denoiser = Denoiser::new().unwrap();

    let mut group = c.benchmark_group("denoise_throughput");
    group.throughput(criterion::Throughput::Elements(samples.len() as u64));

    group.bench_function("samples_per_second", |b| {
        b.iter(|| black_box(denoiser.process(black_box(&samples)).unwrap()))
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_denoise_initialization,
    bench_denoise_process,
    bench_denoise_process_chunks,
    bench_denoise_throughput
}
criterion_main!(benches);
