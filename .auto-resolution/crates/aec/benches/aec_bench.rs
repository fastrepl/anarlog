use std::{hint::black_box, path::PathBuf, time::Duration};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use hound::WavReader;

use aec::{AEC, BLOCK_SIZE};

fn load_test_data() -> (Vec<f32>, Vec<f32>) {
    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("inputs");

    let lpb_sample = WavReader::open(data_dir.join("doubletalk_lpb_sample.wav")).unwrap();
    let mic_sample = WavReader::open(data_dir.join("doubletalk_mic_sample.wav")).unwrap();

    let lpb_samples: Vec<f32> = lpb_sample
        .into_samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();

    let mic_samples: Vec<f32> = mic_sample
        .into_samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();

    (mic_samples, lpb_samples)
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&value| value > 0)
        .unwrap_or(default)
}

fn build_long_stream_data(
    mic_base: &[f32],
    lpb_base: &[f32],
    target_samples: usize,
) -> (Vec<f32>, Vec<f32>) {
    let repeats = target_samples.div_ceil(mic_base.len());

    let mut mic = Vec::with_capacity(mic_base.len() * repeats);
    let mut lpb = Vec::with_capacity(lpb_base.len() * repeats);
    for _ in 0..repeats {
        mic.extend_from_slice(mic_base);
        lpb.extend_from_slice(lpb_base);
    }
    mic.truncate(target_samples);
    lpb.truncate(target_samples);

    (mic, lpb)
}

fn bench_aec_streaming_long(c: &mut Criterion) {
    let sample_rate = env_usize("AEC_BENCH_SAMPLE_RATE", 16_000);
    let seconds = env_usize("AEC_BENCH_SECONDS", 60);
    let chunk_size = env_usize("AEC_BENCH_CHUNK_SIZE", BLOCK_SIZE * 4);

    let target_samples = sample_rate * seconds;
    let (mic_base, lpb_base) = load_test_data();
    let (mic_samples, lpb_samples) = build_long_stream_data(&mic_base, &lpb_base, target_samples);

    let mut group = c.benchmark_group("aec_streaming");
    group.throughput(Throughput::Elements(target_samples as u64));
    group.bench_function(
        format!("long_stream_{}s_chunk_{}", seconds, chunk_size),
        |b| {
            let mut aec = AEC::new().unwrap();
            b.iter(|| {
                aec.reset();

                let mut offset = 0usize;
                while offset < mic_samples.len() {
                    let end = (offset + chunk_size).min(mic_samples.len());
                    black_box(
                        aec.process_streaming(
                            black_box(&mic_samples[offset..end]),
                            black_box(&lpb_samples[offset..end]),
                        )
                        .unwrap(),
                    );
                    offset = end;
                }
            });
        },
    );
    group.finish();
}

fn criterion_config() -> Criterion {
    Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(30))
}

criterion_group! {
    name = benches;
    config = criterion_config();
    targets = bench_aec_streaming_long
}
criterion_main!(benches);
