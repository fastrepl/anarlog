use super::denoiser::Denoiser;
use super::model::{BLOCK_SHIFT, BLOCK_SIZE};
use approx::assert_abs_diff_eq;
use hypr_audio_snapshot::{SpectralConfig, Tolerances};
use std::path::PathBuf;

fn pcm_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
        .collect()
}

fn output_path(prefix: &str, mode: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("outputs")
        .join(format!("{prefix}_{mode}.wav"))
}

fn snapshot_path(prefix: &str, mode: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("snapshots")
        .join(format!("{prefix}_{mode}.json"))
}

#[test]
fn test_denoise_english_1() {
    let config = SpectralConfig {
        fft_size: BLOCK_SIZE,
        hop_size: BLOCK_SHIFT,
        sample_rate: 16000.0,
    };
    let tolerances = Tolerances::default();

    let samples = pcm_bytes_to_f32(hypr_data::english_1::AUDIO);

    let batch_result = {
        let mut denoiser = Denoiser::new().unwrap();
        let result = denoiser.process(&samples).unwrap();
        assert!(result.iter().all(|&x| x.is_finite()));
        result
    };

    let streaming_result = {
        let mut denoiser = Denoiser::new().unwrap();
        let mut streaming_result = Vec::new();

        let chunk_size = BLOCK_SIZE * 2;
        let mut processed = 0;

        while processed < samples.len() {
            let end = (processed + chunk_size).min(samples.len());
            let chunk = &samples[processed..end];

            let chunk_result = denoiser.process_streaming(chunk).unwrap();
            streaming_result.extend(chunk_result);

            processed = end;
        }

        assert!(streaming_result.iter().all(|&x| x.is_finite()));
        streaming_result
    };

    let batch_snap = hypr_audio_snapshot::assert_or_update(
        &batch_result,
        &output_path("english_1", "batch"),
        &snapshot_path("english_1", "batch"),
        "english_1 batch",
        &config,
        &tolerances,
    );

    let streaming_snap = hypr_audio_snapshot::assert_or_update(
        &streaming_result,
        &output_path("english_1", "streaming"),
        &snapshot_path("english_1", "streaming"),
        "english_1 streaming",
        &config,
        &tolerances,
    );

    assert_abs_diff_eq!(
        batch_snap.rms_energy,
        streaming_snap.rms_energy,
        epsilon = 0.05,
    );
    assert_abs_diff_eq!(
        batch_snap.spectral_centroid,
        streaming_snap.spectral_centroid,
        epsilon = 300.0,
    );
}
