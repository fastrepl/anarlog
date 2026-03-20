use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use fs_sync_core::audio::import_audio_with_max_duration;

const MAX_BENCH_DURATION: Duration = Duration::from_secs(5 * 60);

fn bench_input_path() -> PathBuf {
    std::env::var_os("FS_SYNC_AUDIO_BENCH_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(hypr_data::english_1::AUDIO_M4A_PATH))
}

fn bench_input_label(path: &Path) -> String {
    let base = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("audio")
        .to_string();
    format!("{base}_first_5m")
}

fn bench_import_audio(c: &mut Criterion) {
    let input_path = bench_input_path();
    let input_label = bench_input_label(&input_path);

    let mut group = c.benchmark_group("audio_import");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(12));

    group.bench_function(input_label, |b| {
        b.iter(|| {
            let temp = tempfile::tempdir().unwrap();
            let tmp_path = temp.path().join("audio.mp3.tmp");
            let target_path = temp.path().join("audio.mp3");

            black_box(
                import_audio_with_max_duration(
                    black_box(input_path.as_path()),
                    black_box(tmp_path.as_path()),
                    black_box(target_path.as_path()),
                    Some(MAX_BENCH_DURATION),
                )
                .unwrap(),
            );
        });
    });
    group.finish();
}

criterion_group!(benches, bench_import_audio);
criterion_main!(benches);
