use std::path::PathBuf;

use audio_sync::{
    SyncProbe, SyncProbeConfig, SyncProbeEvent, SyncProbeMeasurement, SyncProbeState,
};
use serde::Serialize;

const SAMPLE_RATE: u32 = 16_000;
const CHUNK_SIZE: usize = 1_920;

fn output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("outputs")
}

fn load_wav(bytes: &'static [u8]) -> Vec<f32> {
    let decoder =
        rodio::Decoder::new(std::io::BufReader::new(std::io::Cursor::new(bytes))).unwrap();
    decoder.collect()
}

fn delay_signal(input: &[f32], delay_samples: usize) -> Vec<f32> {
    let mut out = vec![0.0; input.len()];
    for idx in delay_samples..input.len() {
        out[idx] = input[idx - delay_samples];
    }
    out
}

#[derive(Serialize)]
struct MeasurementRecord {
    capture_time_sec: f64,
    state: &'static str,
    lag_samples: isize,
    lag_ms: f32,
    peak_ratio: f32,
    distinctiveness: f32,
    stable_lag_samples: Option<isize>,
    confidence: Option<f32>,
    reference_rms: f32,
    observed_rms: f32,
    drift_ppm: Option<f32>,
}

impl From<&SyncProbeMeasurement> for MeasurementRecord {
    fn from(m: &SyncProbeMeasurement) -> Self {
        Self {
            capture_time_sec: m.capture_time_sec,
            state: state_name(m.snapshot.state),
            lag_samples: m.estimate.lag_samples,
            lag_ms: m.estimate.lag_samples as f32 / SAMPLE_RATE as f32 * 1000.0,
            peak_ratio: m.estimate.peak_ratio,
            distinctiveness: m.estimate.distinctiveness,
            stable_lag_samples: m.snapshot.stable_lag_samples,
            confidence: m.snapshot.confidence,
            reference_rms: m.reference_rms,
            observed_rms: m.observed_rms,
            drift_ppm: m.trend.drift_ppm,
        }
    }
}

#[derive(Serialize)]
struct TestResult {
    name: String,
    artificial_delay_samples: usize,
    measurements: Vec<MeasurementRecord>,
    skipped_low_energy: usize,
    skipped_low_confidence: usize,
    locked_measurements: usize,
    mean_lag: Option<f32>,
    median_lag: Option<f32>,
}

fn run_probe(
    name: &str,
    reference: &[f32],
    observed: &[f32],
    artificial_delay: usize,
) -> TestResult {
    let len = reference.len().min(observed.len());
    let reference = &reference[..len];
    let observed = &observed[..len];

    let mut probe = SyncProbe::new(SyncProbeConfig::new(SAMPLE_RATE));

    let mut measurements = Vec::new();
    let mut skipped = 0usize;
    let mut skipped_low_confidence = 0usize;
    let mut offset = 0;

    while offset + CHUNK_SIZE <= len {
        let ref_chunk = &reference[offset..offset + CHUNK_SIZE];
        let obs_chunk = &observed[offset..offset + CHUNK_SIZE];

        if let Some(event) = probe.observe(ref_chunk, obs_chunk) {
            match event {
                SyncProbeEvent::Measured(m) => {
                    measurements.push(MeasurementRecord::from(&m));
                }
                SyncProbeEvent::SkippedLowEnergy(_) => {
                    skipped += 1;
                }
                SyncProbeEvent::SkippedLowConfidence(_) => {
                    skipped_low_confidence += 1;
                }
            }
        }

        offset += CHUNK_SIZE;
    }

    let preferred_lags = preferred_lags(&measurements);
    let mean_lag = if preferred_lags.is_empty() {
        None
    } else {
        let sum: f32 = preferred_lags.iter().map(|lag| *lag as f32).sum();
        Some(sum / preferred_lags.len() as f32)
    };
    let median_lag = median_lag_values(&preferred_lags);
    let locked_measurements = measurements
        .iter()
        .filter(|measurement| measurement.state == state_name(SyncProbeState::Locked))
        .count();

    TestResult {
        name: name.to_string(),
        artificial_delay_samples: artificial_delay,
        measurements,
        skipped_low_energy: skipped,
        skipped_low_confidence,
        locked_measurements,
        mean_lag,
        median_lag,
    }
}

fn print_result(result: &TestResult, baseline_median_lag: Option<f32>) {
    println!(
        "\n=== {} (artificial delay: {} samples, {:.1}ms) ===",
        result.name,
        result.artificial_delay_samples,
        result.artificial_delay_samples as f32 / SAMPLE_RATE as f32 * 1000.0
    );

    for (i, m) in result.measurements.iter().enumerate() {
        println!(
            "  measurement {:2}: state={} lag={:5}  ({:+6.1}ms)  distinctiveness={:6.1}  peak_ratio={:6.1}  confidence={:5}  drift_ppm={:>8}",
            i + 1,
            m.state,
            m.lag_samples,
            m.lag_ms,
            m.distinctiveness,
            m.peak_ratio,
            m.confidence
                .map_or("n/a".to_string(), |value| format!("{:.2}", value)),
            m.drift_ppm
                .map_or("n/a".to_string(), |v| format!("{:.1}", v)),
        );
    }

    if result.skipped_low_energy > 0 {
        println!(
            "  ({} intervals skipped due to low energy)",
            result.skipped_low_energy
        );
    }
    if result.skipped_low_confidence > 0 {
        println!(
            "  ({} intervals skipped due to low confidence)",
            result.skipped_low_confidence
        );
    }

    if let Some(mean) = result.mean_lag {
        println!(
            "  mean_lag={:.1} samples ({:+.2}ms)  locked_measurements={}",
            mean,
            mean / SAMPLE_RATE as f32 * 1000.0,
            result.locked_measurements,
        );
    }

    if let Some(median) = result.median_lag {
        println!(
            "  median_lag={:.1} samples ({:+.2}ms)",
            median,
            median / SAMPLE_RATE as f32 * 1000.0,
        );

        if let Some(baseline) = baseline_median_lag {
            let delta = median - baseline;
            let expected_delta = -(result.artificial_delay_samples as f32);
            println!(
                "  delta_from_baseline={:+.1}  expected_delta={:+.1} samples ({:+.2}ms)",
                delta,
                expected_delta,
                delta / SAMPLE_RATE as f32 * 1000.0,
            );
        }
    } else {
        println!("  no measurements produced");
    }
}

fn save_result(result: &TestResult) {
    let path = output_dir().join(format!(
        "{}_delay{}.json",
        result.name, result.artificial_delay_samples
    ));
    let json = serde_json::to_string_pretty(result).unwrap();
    std::fs::write(&path, json).unwrap();
}

fn median_lag_values(lags: &[isize]) -> Option<f32> {
    if lags.is_empty() {
        return None;
    }

    let mut lags = lags.to_vec();
    lags.sort_unstable();

    let mid = lags.len() / 2;
    Some(if lags.len() % 2 == 0 {
        (lags[mid - 1] as f32 + lags[mid] as f32) / 2.0
    } else {
        lags[mid] as f32
    })
}

fn preferred_lags(measurements: &[MeasurementRecord]) -> Vec<isize> {
    let locked: Vec<isize> = measurements
        .iter()
        .filter(|measurement| measurement.state == state_name(SyncProbeState::Locked))
        .map(|measurement| measurement.lag_samples)
        .collect();
    if !locked.is_empty() {
        return locked;
    }

    measurements
        .iter()
        .map(|measurement| measurement.lag_samples)
        .collect()
}

fn lag_spread(measurements: &[MeasurementRecord]) -> Option<isize> {
    let lags = preferred_lags(measurements);
    let min = lags.iter().min()?;
    let max = lags.iter().max()?;
    Some(*max - *min)
}

fn state_name(state: SyncProbeState) -> &'static str {
    match state {
        SyncProbeState::Searching => "searching",
        SyncProbeState::Acquiring => "acquiring",
        SyncProbeState::Locked => "locked",
        SyncProbeState::Holdover => "holdover",
        SyncProbeState::Lost => "lost",
    }
}

struct AudioPair {
    name: &'static str,
    mic: &'static [u8],
    lpb: &'static [u8],
}

const PAIRS: &[AudioPair] = &[
    AudioPair {
        name: "doubletalk",
        mic: include_bytes!("../../aec/data/inputs/doubletalk_mic_sample.wav"),
        lpb: include_bytes!("../../aec/data/inputs/doubletalk_lpb_sample.wav"),
    },
    AudioPair {
        name: "hyprnote",
        mic: include_bytes!("../../aec/data/inputs/hyprnote_mic.wav"),
        lpb: include_bytes!("../../aec/data/inputs/hyprnote_lpb.wav"),
    },
    AudioPair {
        name: "theo",
        mic: include_bytes!("../../aec/data/inputs/theo_mic.wav"),
        lpb: include_bytes!("../../aec/data/inputs/theo_lpb.wav"),
    },
];

const ARTIFICIAL_DELAYS: &[usize] = &[0, 50, 100, 200, 480];

#[test]
fn natural_lag_measurement() {
    println!("\n{}", "=".repeat(60));
    println!("NATURAL LAG MEASUREMENT (no artificial delay)");
    println!("Measures the inherent lag in the original recordings.");
    println!("{}", "=".repeat(60));

    for pair in PAIRS {
        let mic = load_wav(pair.mic);
        let lpb = load_wav(pair.lpb);

        let result = run_probe(pair.name, &lpb, &mic, 0);
        print_result(&result, None);
        save_result(&result);

        match pair.name {
            "doubletalk" => {
                assert!(result.measurements.is_empty());
                assert!(result.skipped_low_confidence > 0);
                assert_eq!(result.locked_measurements, 0);
            }
            "hyprnote" => {
                assert!(result.measurements.is_empty());
                assert!(result.skipped_low_energy > 0);
                assert_eq!(result.locked_measurements, 0);
            }
            "theo" => {
                let median = result.median_lag.expect("theo should produce measurements");
                assert!(
                    (median - 474.0).abs() <= 8.0,
                    "unexpected theo median lag: {median}"
                );
                assert!(result.locked_measurements >= 8);
                let spread = lag_spread(&result.measurements).expect("theo spread");
                assert!(spread <= 12, "theo lag spread too large: {spread}");
            }
            other => panic!("unexpected pair: {other}"),
        }
    }
}

#[test]
fn artificial_delay_detection() {
    println!("\n{}", "=".repeat(60));
    println!("ARTIFICIAL DELAY DETECTION");
    println!("Reference (lpb) is delayed by known amounts.");
    println!("Measured lag should move downward relative to the natural baseline.");
    println!("{}", "=".repeat(60));

    for pair in PAIRS {
        let mic = load_wav(pair.mic);
        let lpb = load_wav(pair.lpb);
        let baseline = run_probe(pair.name, &lpb, &mic, 0);
        let baseline_median_lag = baseline.median_lag;

        for &delay in ARTIFICIAL_DELAYS {
            if delay == 0 {
                continue;
            }
            let delayed_lpb = delay_signal(&lpb, delay);
            let result = run_probe(&format!("{}_shifted", pair.name), &delayed_lpb, &mic, delay);
            print_result(&result, baseline_median_lag);
            save_result(&result);

            match pair.name {
                "doubletalk" => {
                    assert!(result.measurements.is_empty());
                    assert!(result.skipped_low_confidence > 0);
                    assert_eq!(result.locked_measurements, 0);
                }
                "hyprnote" => {
                    assert!(result.measurements.is_empty());
                    assert!(result.skipped_low_energy > 0);
                    assert_eq!(result.locked_measurements, 0);
                }
                "theo" => {
                    let baseline_lag = baseline_median_lag.expect("theo baseline lag");
                    let median_lag = result.median_lag.expect("theo shifted lag");
                    let delta = median_lag - baseline_lag;
                    assert!(
                        (delta + delay as f32).abs() <= 8.0,
                        "unexpected theo lag delta for delay {delay}: {delta}"
                    );

                    let spread = lag_spread(&result.measurements).expect("theo spread");
                    assert!(result.locked_measurements >= 8);
                    assert!(spread <= 12, "theo lag spread too large: {spread}");
                }
                other => panic!("unexpected pair: {other}"),
            }
        }
    }
}
