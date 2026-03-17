use std::collections::VecDeque;
use std::sync::Arc;

use realfft::{ComplexToReal, RealFftPlanner, RealToComplex, num_complex::Complex32};

const PEAK_NEIGHBORHOOD: isize = 3;

pub struct GccPhatLagEstimator {
    window_samples: usize,
    max_lag_samples: usize,
    fft_len: usize,
    forward: Arc<dyn RealToComplex<f32>>,
    inverse: Arc<dyn ComplexToReal<f32>>,
    reference_time: Vec<f32>,
    observed_time: Vec<f32>,
    reference_freq: Vec<Complex32>,
    observed_freq: Vec<Complex32>,
    cross_freq: Vec<Complex32>,
    correlation: Vec<f32>,
    forward_scratch: Vec<Complex32>,
    inverse_scratch: Vec<Complex32>,
}

impl GccPhatLagEstimator {
    pub fn new(window_samples: usize, max_lag_samples: usize) -> Self {
        let fft_len = (window_samples * 2).next_power_of_two();
        let mut planner = RealFftPlanner::<f32>::new();
        let forward = planner.plan_fft_forward(fft_len);
        let inverse = planner.plan_fft_inverse(fft_len);

        Self {
            window_samples,
            max_lag_samples,
            fft_len,
            reference_time: vec![0.0; fft_len],
            observed_time: vec![0.0; fft_len],
            reference_freq: forward.make_output_vec(),
            observed_freq: forward.make_output_vec(),
            cross_freq: inverse.make_input_vec(),
            correlation: inverse.make_output_vec(),
            forward_scratch: forward.make_scratch_vec(),
            inverse_scratch: inverse.make_scratch_vec(),
            forward,
            inverse,
        }
    }

    pub fn estimate(&mut self, reference: &[f32], observed: &[f32]) -> Option<LagEstimate> {
        if reference.len() != self.window_samples || observed.len() != self.window_samples {
            return None;
        }

        copy_centered(reference, &mut self.reference_time[..self.window_samples]);
        self.reference_time[self.window_samples..].fill(0.0);
        copy_centered(observed, &mut self.observed_time[..self.window_samples]);
        self.observed_time[self.window_samples..].fill(0.0);

        self.forward
            .process_with_scratch(
                &mut self.reference_time,
                &mut self.reference_freq,
                &mut self.forward_scratch,
            )
            .ok()?;
        self.forward
            .process_with_scratch(
                &mut self.observed_time,
                &mut self.observed_freq,
                &mut self.forward_scratch,
            )
            .ok()?;

        for ((cross, reference_bin), observed_bin) in self
            .cross_freq
            .iter_mut()
            .zip(self.reference_freq.iter())
            .zip(self.observed_freq.iter())
        {
            let value = *observed_bin * reference_bin.conj();
            let norm = value.norm();
            *cross = if norm > f32::EPSILON {
                value / norm
            } else {
                Complex32::new(0.0, 0.0)
            };
        }

        self.inverse
            .process_with_scratch(
                &mut self.cross_freq,
                &mut self.correlation,
                &mut self.inverse_scratch,
            )
            .ok()?;

        let mut best_lag = 0isize;
        let mut peak = 0.0f32;
        let mut sum_abs = 0.0f32;
        let mut count = 0usize;

        for lag in -(self.max_lag_samples as isize)..=(self.max_lag_samples as isize) {
            let value = self.correlation[idx_for_lag(lag, self.fft_len)].abs();
            if value > peak {
                peak = value;
                best_lag = lag;
            }
            sum_abs += value;
            count += 1;
        }

        if count == 0 || peak <= f32::EPSILON {
            return None;
        }

        let noise_floor = (sum_abs - peak).max(0.0) / (count.saturating_sub(1).max(1) as f32);

        let mut second_peak = 0.0f32;
        for lag in -(self.max_lag_samples as isize)..=(self.max_lag_samples as isize) {
            if (lag - best_lag).abs() <= PEAK_NEIGHBORHOOD {
                continue;
            }
            second_peak = second_peak.max(self.correlation[idx_for_lag(lag, self.fft_len)].abs());
        }

        Some(LagEstimate {
            lag_samples: best_lag,
            peak_ratio: peak / noise_floor.max(1e-6),
            distinctiveness: peak / second_peak.max(1e-6),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LagEstimate {
    pub lag_samples: isize,
    pub peak_ratio: f32,
    pub distinctiveness: f32,
}

#[derive(Default)]
pub struct LagTrendTracker {
    last_capture_time_sec: Option<f64>,
    last_lag_samples: Option<f32>,
    smoothed_drift_samples_per_sec: Option<f32>,
}

impl LagTrendTracker {
    pub fn update(
        &mut self,
        capture_time_sec: f64,
        lag_samples: f32,
        sample_rate: u32,
    ) -> DriftTrendSnapshot {
        let mut snapshot = DriftTrendSnapshot::default();

        if let (Some(last_time), Some(last_lag)) =
            (self.last_capture_time_sec, self.last_lag_samples)
        {
            let dt = (capture_time_sec - last_time) as f32;
            if dt > 0.0 {
                let instant_drift = (lag_samples - last_lag) / dt;
                let smoothed = match self.smoothed_drift_samples_per_sec {
                    Some(previous) => previous * 0.8 + instant_drift * 0.2,
                    None => instant_drift,
                };
                self.smoothed_drift_samples_per_sec = Some(smoothed);

                snapshot.drift_samples_per_sec = Some(smoothed);
                snapshot.drift_ms_per_min = Some(smoothed * 60_000.0 / sample_rate as f32);
                snapshot.drift_ppm = Some(smoothed * 1_000_000.0 / sample_rate as f32);
            }
        }

        self.last_capture_time_sec = Some(capture_time_sec);
        self.last_lag_samples = Some(lag_samples);
        snapshot
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DriftTrendSnapshot {
    pub drift_samples_per_sec: Option<f32>,
    pub drift_ms_per_min: Option<f32>,
    pub drift_ppm: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub struct SyncProbeConfig {
    pub sample_rate: u32,
    pub window_samples: usize,
    pub max_lag_samples: usize,
    pub interval_samples: usize,
    pub min_rms: f32,
    pub level_interval_samples: usize,
}

impl SyncProbeConfig {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            window_samples: 4096,
            max_lag_samples: 960,
            interval_samples: 16_000,
            min_rms: 0.003,
            level_interval_samples: sample_rate as usize,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncProbeInputSide {
    Reference,
    Observed,
}

pub struct SyncProbe {
    sample_rate: u32,
    window_samples: usize,
    interval_samples: usize,
    min_rms: f32,
    reference_history: VecDeque<f32>,
    observed_history: VecDeque<f32>,
    interval_progress: usize,
    processed_samples: u64,
    estimator: GccPhatLagEstimator,
    trend: LagTrendTracker,
    reference_input_levels: LevelAccumulator,
    observed_input_levels: LevelAccumulator,
}

impl SyncProbe {
    pub fn new(config: SyncProbeConfig) -> Self {
        let window_samples = config.window_samples.max(256);
        let max_lag_samples = config.max_lag_samples.min(window_samples.saturating_sub(1));
        let level_interval_samples = config.level_interval_samples.max(1);

        Self {
            sample_rate: config.sample_rate,
            window_samples,
            interval_samples: config.interval_samples.max(1),
            min_rms: config.min_rms.max(0.0),
            reference_history: VecDeque::with_capacity(window_samples),
            observed_history: VecDeque::with_capacity(window_samples),
            interval_progress: 0,
            processed_samples: 0,
            estimator: GccPhatLagEstimator::new(window_samples, max_lag_samples),
            trend: LagTrendTracker::default(),
            reference_input_levels: LevelAccumulator::new(level_interval_samples),
            observed_input_levels: LevelAccumulator::new(level_interval_samples),
        }
    }

    pub fn config(&self) -> SyncProbeConfig {
        SyncProbeConfig {
            sample_rate: self.sample_rate,
            window_samples: self.window_samples,
            max_lag_samples: self.estimator.max_lag_samples,
            interval_samples: self.interval_samples,
            min_rms: self.min_rms,
            level_interval_samples: self.reference_input_levels.interval_samples,
        }
    }

    pub fn observe_input_chunk(
        &mut self,
        side: SyncProbeInputSide,
        data: &[f32],
    ) -> Option<LevelSnapshot> {
        let accumulator = match side {
            SyncProbeInputSide::Reference => &mut self.reference_input_levels,
            SyncProbeInputSide::Observed => &mut self.observed_input_levels,
        };

        accumulator.observe(data)
    }

    pub fn observe(&mut self, reference: &[f32], observed: &[f32]) -> Option<SyncProbeEvent> {
        let len = reference.len().min(observed.len());
        if len == 0 {
            return None;
        }

        Self::append_history(
            self.window_samples,
            &mut self.reference_history,
            &reference[..len],
        );
        Self::append_history(
            self.window_samples,
            &mut self.observed_history,
            &observed[..len],
        );
        self.processed_samples += len as u64;
        self.interval_progress += len;

        if self.reference_history.len() < self.window_samples
            || self.observed_history.len() < self.window_samples
            || self.interval_progress < self.interval_samples
        {
            return None;
        }
        self.interval_progress = 0;

        let reference_window: Vec<f32> = self.reference_history.iter().copied().collect();
        let observed_window: Vec<f32> = self.observed_history.iter().copied().collect();

        let reference_rms = rms(&reference_window);
        let observed_rms = rms(&observed_window);
        let capture_time_sec = self.capture_time_sec();

        if reference_rms < self.min_rms || observed_rms < self.min_rms {
            return Some(SyncProbeEvent::SkippedLowEnergy(SyncProbeLowEnergy {
                capture_time_sec,
                reference_rms,
                observed_rms,
            }));
        }

        let estimate = self
            .estimator
            .estimate(&reference_window, &observed_window)?;
        let trend = self.trend.update(
            capture_time_sec,
            estimate.lag_samples as f32,
            self.sample_rate,
        );

        Some(SyncProbeEvent::Measured(SyncProbeMeasurement {
            capture_time_sec,
            estimate,
            reference_rms,
            observed_rms,
            trend,
        }))
    }

    fn append_history(window_samples: usize, history: &mut VecDeque<f32>, data: &[f32]) {
        for &sample in data {
            if history.len() == window_samples {
                history.pop_front();
            }
            history.push_back(sample);
        }
    }

    fn capture_time_sec(&self) -> f64 {
        self.processed_samples as f64 / self.sample_rate as f64
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SyncProbeEvent {
    SkippedLowEnergy(SyncProbeLowEnergy),
    Measured(SyncProbeMeasurement),
}

#[derive(Debug, Clone, Copy)]
pub struct SyncProbeLowEnergy {
    pub capture_time_sec: f64,
    pub reference_rms: f32,
    pub observed_rms: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct SyncProbeMeasurement {
    pub capture_time_sec: f64,
    pub estimate: LagEstimate,
    pub reference_rms: f32,
    pub observed_rms: f32,
    pub trend: DriftTrendSnapshot,
}

pub struct LevelAccumulator {
    interval_samples: usize,
    sum_squares: f64,
    peak: f32,
    nonzero_samples: usize,
    samples: usize,
}

impl LevelAccumulator {
    pub fn new(interval_samples: usize) -> Self {
        Self {
            interval_samples: interval_samples.max(1),
            sum_squares: 0.0,
            peak: 0.0,
            nonzero_samples: 0,
            samples: 0,
        }
    }

    pub fn observe(&mut self, data: &[f32]) -> Option<LevelSnapshot> {
        for &sample in data {
            self.sum_squares += f64::from(sample) * f64::from(sample);
            self.peak = self.peak.max(sample.abs());
            if sample != 0.0 {
                self.nonzero_samples += 1;
            }
            self.samples += 1;
        }

        if self.samples < self.interval_samples {
            return None;
        }

        let snapshot = LevelSnapshot {
            rms: (self.sum_squares / self.samples as f64).sqrt() as f32,
            peak: self.peak,
            nonzero_ratio: self.nonzero_samples as f32 / self.samples as f32,
            samples: self.samples,
        };

        self.sum_squares = 0.0;
        self.peak = 0.0;
        self.nonzero_samples = 0;
        self.samples = 0;

        Some(snapshot)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LevelSnapshot {
    pub rms: f32,
    pub peak: f32,
    pub nonzero_ratio: f32,
    pub samples: usize,
}

pub fn rms_to_dbfs(rms: f32) -> f32 {
    20.0 * rms.max(1e-9).log10()
}

pub fn amplitude_to_dbfs(value: f32) -> f32 {
    20.0 * value.max(1e-9).log10()
}

fn copy_centered(input: &[f32], output: &mut [f32]) {
    let mean = input.iter().copied().sum::<f32>() / input.len().max(1) as f32;
    for (out, &sample) in output.iter_mut().zip(input.iter()) {
        *out = sample - mean;
    }
}

fn idx_for_lag(lag: isize, fft_len: usize) -> usize {
    if lag >= 0 {
        lag as usize
    } else {
        (fft_len as isize + lag) as usize
    }
}

fn rms(data: &[f32]) -> f32 {
    let energy = data.iter().map(|sample| sample * sample).sum::<f32>() / data.len().max(1) as f32;
    energy.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn excitation(len: usize) -> Vec<f32> {
        let mut state = 0x1234_5678u32;
        (0..len)
            .map(|idx| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                let noise = (state as f32 / u32::MAX as f32) * 2.0 - 1.0;
                let pulse = if idx % 257 == 0 { 0.75 } else { 0.0 };
                0.6 * noise + pulse
            })
            .collect()
    }

    fn delay_signal(input: &[f32], delay_samples: usize) -> Vec<f32> {
        let mut out = vec![0.0; input.len()];
        for idx in delay_samples..input.len() {
            out[idx] = input[idx - delay_samples];
        }
        out
    }

    #[test]
    fn gcc_phat_estimates_positive_delay() {
        let window = 4096;
        let delay = 192;
        let reference = excitation(window);
        let observed = delay_signal(&reference, delay);

        let mut estimator = GccPhatLagEstimator::new(window, 512);
        let estimate = estimator.estimate(&reference, &observed).unwrap();

        assert_eq!(estimate.lag_samples, delay as isize);
        assert!(estimate.peak_ratio > 1.0);
    }

    #[test]
    fn lag_trend_reports_positive_drift_ppm() {
        let mut trend = LagTrendTracker::default();
        let first = trend.update(1.0, 100.0, 16_000);
        assert!(first.drift_ppm.is_none());

        let second = trend.update(11.0, 108.0, 16_000);
        let drift_ppm = second.drift_ppm.unwrap();
        assert!(drift_ppm > 0.0);
        assert!((drift_ppm - 50.0).abs() < 1.0);
    }

    #[test]
    fn level_accumulator_reports_snapshot() {
        let mut levels = LevelAccumulator::new(4);
        assert!(levels.observe(&[0.0, 0.5]).is_none());
        let snapshot = levels.observe(&[0.25, -0.25]).unwrap();
        assert_eq!(snapshot.samples, 4);
        assert_eq!(snapshot.nonzero_ratio, 0.75);
        assert!(snapshot.peak >= 0.5);
    }

    #[test]
    fn sync_probe_reports_measurement() {
        let window = 4096;
        let delay = 192;
        let reference = excitation(window);
        let observed = delay_signal(&reference, delay);
        let mut probe = SyncProbe::new(SyncProbeConfig {
            sample_rate: 16_000,
            window_samples: window,
            max_lag_samples: 512,
            interval_samples: window,
            min_rms: 0.0,
            level_interval_samples: 16_000,
        });

        let event = probe.observe(&reference, &observed).unwrap();

        match event {
            SyncProbeEvent::Measured(measurement) => {
                assert_eq!(measurement.estimate.lag_samples, delay as isize);
                assert!(measurement.reference_rms > 0.0);
                assert!(measurement.observed_rms > 0.0);
            }
            SyncProbeEvent::SkippedLowEnergy(_) => panic!("expected measurement"),
        }
    }

    #[test]
    fn sync_probe_reports_low_energy_skip() {
        let mut probe = SyncProbe::new(SyncProbeConfig {
            sample_rate: 16_000,
            window_samples: 256,
            max_lag_samples: 64,
            interval_samples: 256,
            min_rms: 0.1,
            level_interval_samples: 16_000,
        });

        let event = probe.observe(&[0.0; 256], &[0.0; 256]).unwrap();

        match event {
            SyncProbeEvent::SkippedLowEnergy(skip) => {
                assert_eq!(skip.reference_rms, 0.0);
                assert_eq!(skip.observed_rms, 0.0);
            }
            SyncProbeEvent::Measured(_) => panic!("expected low energy skip"),
        }
    }
}
