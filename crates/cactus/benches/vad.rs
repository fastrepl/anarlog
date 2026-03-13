use std::hint::black_box;

use cactus::{Model, VadOptions};
use criterion::{Criterion, criterion_group, criterion_main};

fn vad_model() -> Model {
    let path =
        std::env::var("CACTUS_STT_MODEL").unwrap_or_else(|_| "/tmp/whisper-final/vad".into());
    Model::new(&path).unwrap()
}

fn bench_vad_pcm(c: &mut Criterion) {
    let model = vad_model();
    let options = VadOptions::default();
    let pcm = hypr_data::english_1::AUDIO;

    c.bench_function("vad_pcm english_1", |b| {
        b.iter(|| model.vad_pcm(black_box(pcm), black_box(&options)).unwrap())
    });
}

fn bench_vad_file(c: &mut Criterion) {
    let model = vad_model();
    let options = VadOptions::default();
    let path = hypr_data::english_1::AUDIO_PATH;

    c.bench_function("vad_file english_1", |b| {
        b.iter(|| {
            model
                .vad_file(black_box(path), black_box(&options))
                .unwrap()
        })
    });
}

criterion_group!(benches, bench_vad_pcm, bench_vad_file);
criterion_main!(benches);
