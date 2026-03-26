use realfft::{RealFftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct AudioSnapshot {
    pub sample_count: usize,
    pub rms_energy: f32,
    pub peak_amplitude: f32,
    pub zero_crossing_rate: f32,
    pub spectral_centroid: f32,
    pub band_energy_low: f32,
    pub band_energy_mid: f32,
    pub band_energy_high: f32,
}

pub struct SpectralConfig {
    pub fft_size: usize,
    pub hop_size: usize,
    pub sample_rate: f32,
}

impl Default for SpectralConfig {
    fn default() -> Self {
        Self {
            fft_size: 512,
            hop_size: 128,
            sample_rate: 16000.0,
        }
    }
}

pub struct Tolerances {
    pub rms_energy_epsilon: f32,
    pub peak_amplitude_epsilon: f32,
    pub zero_crossing_rate_epsilon: f32,
    pub spectral_centroid_epsilon: f32,
    pub band_energy_max_relative: f32,
}

impl Default for Tolerances {
    fn default() -> Self {
        Self {
            rms_energy_epsilon: 1e-3,
            peak_amplitude_epsilon: 1e-3,
            zero_crossing_rate_epsilon: 5e-3,
            spectral_centroid_epsilon: 50.0,
            band_energy_max_relative: 0.05,
        }
    }
}

fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&x| x * x).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

fn compute_peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |max, &x| max.max(x.abs()))
}

fn compute_zero_crossing_rate(samples: &[f32]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    let crossings = samples
        .windows(2)
        .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
        .count();
    crossings as f32 / (samples.len() - 1) as f32
}

fn compute_spectral_metrics(samples: &[f32], config: &SpectralConfig) -> (f32, f32, f32, f32) {
    let block_size = config.fft_size;
    let block_shift = config.hop_size;
    let num_bins = block_size / 2 + 1;
    let sample_rate = config.sample_rate;

    if samples.len() < block_size {
        return (0.0, 0.0, 0.0, 0.0);
    }

    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(block_size);

    let mut avg_magnitude = vec![0.0f32; num_bins];
    let mut num_blocks = 0usize;

    let mut fft_input = vec![0.0f32; block_size];
    let mut fft_output = vec![Complex::new(0.0f32, 0.0f32); num_bins];
    let mut scratch = vec![Complex::new(0.0f32, 0.0f32); fft.get_scratch_len()];

    let mut pos = 0;
    while pos + block_size <= samples.len() {
        fft_input.copy_from_slice(&samples[pos..pos + block_size]);
        fft.process_with_scratch(&mut fft_input, &mut fft_output, &mut scratch)
            .unwrap();

        for (i, c) in fft_output.iter().enumerate() {
            avg_magnitude[i] += c.norm();
        }
        num_blocks += 1;
        pos += block_shift;
    }

    if num_blocks == 0 {
        return (0.0, 0.0, 0.0, 0.0);
    }

    for m in avg_magnitude.iter_mut() {
        *m /= num_blocks as f32;
    }

    let freq_per_bin = sample_rate / block_size as f32;

    let total_energy: f32 = avg_magnitude.iter().map(|&m| m * m).sum();
    let centroid = if total_energy > 0.0 {
        let weighted_sum: f32 = avg_magnitude
            .iter()
            .enumerate()
            .map(|(i, &m)| i as f32 * freq_per_bin * m * m)
            .sum();
        weighted_sum / total_energy
    } else {
        0.0
    };

    let bin_500 = (500.0 / freq_per_bin).ceil() as usize;
    let bin_2000 = (2000.0 / freq_per_bin).ceil() as usize;
    let bin_8000 = (8000.0 / freq_per_bin).ceil() as usize;

    let band_low: f32 = avg_magnitude[..bin_500.min(num_bins)]
        .iter()
        .map(|&m| m * m)
        .sum();
    let band_mid: f32 = avg_magnitude[bin_500.min(num_bins)..bin_2000.min(num_bins)]
        .iter()
        .map(|&m| m * m)
        .sum();
    let band_high: f32 = avg_magnitude[bin_2000.min(num_bins)..bin_8000.min(num_bins)]
        .iter()
        .map(|&m| m * m)
        .sum();

    (centroid, band_low, band_mid, band_high)
}

pub fn compute_snapshot(samples: &[f32], config: &SpectralConfig) -> AudioSnapshot {
    let (spectral_centroid, band_energy_low, band_energy_mid, band_energy_high) =
        compute_spectral_metrics(samples, config);
    AudioSnapshot {
        sample_count: samples.len(),
        rms_energy: compute_rms(samples),
        peak_amplitude: compute_peak(samples),
        zero_crossing_rate: compute_zero_crossing_rate(samples),
        spectral_centroid,
        band_energy_low,
        band_energy_mid,
        band_energy_high,
    }
}

pub fn should_update_snapshots() -> bool {
    std::env::var("UPDATE_SNAPSHOTS").is_ok()
}

pub fn save_snapshot(snapshot: &AudioSnapshot, path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let json = serde_json::to_string_pretty(snapshot).unwrap();
    std::fs::write(path, json).unwrap();
}

pub fn load_snapshot(path: &Path) -> AudioSnapshot {
    let json = std::fs::read_to_string(path).unwrap_or_else(|_| {
        panic!(
            "Snapshot file not found: {}\nRun with UPDATE_SNAPSHOTS=1 to generate baselines.",
            path.display()
        )
    });
    serde_json::from_str(&json).unwrap()
}

pub fn assert_snapshot_eq(
    actual: &AudioSnapshot,
    expected: &AudioSnapshot,
    label: &str,
    tolerances: &Tolerances,
) {
    assert_eq!(
        actual.sample_count, expected.sample_count,
        "[{label}] sample_count mismatch"
    );
    approx::assert_abs_diff_eq!(
        actual.rms_energy,
        expected.rms_energy,
        epsilon = tolerances.rms_energy_epsilon,
    );
    approx::assert_abs_diff_eq!(
        actual.peak_amplitude,
        expected.peak_amplitude,
        epsilon = tolerances.peak_amplitude_epsilon,
    );
    approx::assert_abs_diff_eq!(
        actual.zero_crossing_rate,
        expected.zero_crossing_rate,
        epsilon = tolerances.zero_crossing_rate_epsilon,
    );
    approx::assert_abs_diff_eq!(
        actual.spectral_centroid,
        expected.spectral_centroid,
        epsilon = tolerances.spectral_centroid_epsilon,
    );
    approx::assert_relative_eq!(
        actual.band_energy_low,
        expected.band_energy_low,
        max_relative = tolerances.band_energy_max_relative,
    );
    approx::assert_relative_eq!(
        actual.band_energy_mid,
        expected.band_energy_mid,
        max_relative = tolerances.band_energy_max_relative,
    );
    approx::assert_relative_eq!(
        actual.band_energy_high,
        expected.band_energy_high,
        max_relative = tolerances.band_energy_max_relative,
    );
}

pub fn assert_or_update(
    samples: &[f32],
    wav_path: &Path,
    snapshot_path: &Path,
    label: &str,
    config: &SpectralConfig,
    tolerances: &Tolerances,
) -> AudioSnapshot {
    let snapshot = compute_snapshot(samples, config);

    if let Some(parent) = wav_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: config.sample_rate as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(wav_path, spec).unwrap();
    for &sample in samples {
        writer.write_sample(sample).unwrap();
    }
    writer.finalize().unwrap();

    if should_update_snapshots() {
        save_snapshot(&snapshot, snapshot_path);
        println!("Updated snapshot: {}", snapshot_path.display());
    } else {
        let expected = load_snapshot(snapshot_path);
        assert_snapshot_eq(&snapshot, &expected, label, tolerances);
    }

    snapshot
}
