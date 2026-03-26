use std::collections::VecDeque;

use crate::drift::{DriftTrendSnapshot, LagTrendTracker};
use crate::estimator::{GccPhatLagEstimator, LagEstimate};
use crate::level::{LevelAccumulator, LevelSnapshot};

#[derive(Debug, Clone, Copy)]
pub struct SyncProbeThresholds {
    pub min_peak_ratio: f32,
    pub min_distinctiveness: f32,
}

impl SyncProbeThresholds {
    pub const fn new(min_peak_ratio: f32, min_distinctiveness: f32) -> Self {
        Self {
            min_peak_ratio,
            min_distinctiveness,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SyncProbeTuning {
    pub acquire: SyncProbeThresholds,
    pub hold: SyncProbeThresholds,
    pub acquire_lock_count: usize,
    pub acquire_window: usize,
    pub acquire_cluster_tolerance_samples: usize,
    pub hold_interval_count: usize,
    pub lost_after_rejections: usize,
    pub lock_outlier_tolerance_samples: usize,
    pub stable_lag_window: usize,
}

impl Default for SyncProbeTuning {
    fn default() -> Self {
        Self {
            acquire: SyncProbeThresholds::new(10.0, 1.15),
            hold: SyncProbeThresholds::new(8.0, 1.05),
            acquire_lock_count: 3,
            acquire_window: 4,
            acquire_cluster_tolerance_samples: 24,
            hold_interval_count: 2,
            lost_after_rejections: 3,
            lock_outlier_tolerance_samples: 48,
            stable_lag_window: 5,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SyncProbeConfig {
    pub sample_rate: u32,
    pub window_samples: usize,
    pub max_lag_samples: usize,
    pub interval_samples: usize,
    pub min_rms: f32,
    pub tuning: SyncProbeTuning,
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
            tuning: SyncProbeTuning::default(),
            level_interval_samples: sample_rate as usize,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncProbeInputSide {
    Reference,
    Observed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncProbeState {
    Searching,
    Acquiring,
    Locked,
    Holdover,
    Lost,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SyncProbeRejectionCounts {
    pub low_energy: u64,
    pub weak_correlation: u64,
    pub lag_outlier: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct SyncProbeSnapshot {
    pub state: SyncProbeState,
    pub stable_lag_samples: Option<isize>,
    pub candidate_lag_samples: Option<isize>,
    pub accepted_window_count: usize,
    pub confidence: Option<f32>,
    pub rejections: SyncProbeRejectionCounts,
}

pub struct SyncProbe {
    sample_rate: u32,
    window_samples: usize,
    interval_samples: usize,
    min_rms: f32,
    tuning: SyncProbeTuning,
    reference_history: Vec<f32>,
    observed_history: Vec<f32>,
    reference_window: Vec<f32>,
    observed_window: Vec<f32>,
    history_write_cursor: usize,
    history_len: usize,
    acquisition_entries: VecDeque<Option<isize>>,
    stable_lag_history: VecDeque<isize>,
    state: SyncProbeState,
    stable_lag_samples: Option<isize>,
    consecutive_rejections: usize,
    rejections: SyncProbeRejectionCounts,
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
            tuning: config.tuning,
            reference_history: vec![0.0; window_samples],
            observed_history: vec![0.0; window_samples],
            reference_window: vec![0.0; window_samples],
            observed_window: vec![0.0; window_samples],
            history_write_cursor: 0,
            history_len: 0,
            acquisition_entries: VecDeque::with_capacity(config.tuning.acquire_window.max(1)),
            stable_lag_history: VecDeque::with_capacity(config.tuning.stable_lag_window.max(1)),
            state: SyncProbeState::Searching,
            stable_lag_samples: None,
            consecutive_rejections: 0,
            rejections: SyncProbeRejectionCounts::default(),
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
            tuning: self.tuning,
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

        self.append_history(&reference[..len], &observed[..len]);
        self.processed_samples += len as u64;
        self.interval_progress += len;

        if self.history_len < self.window_samples || self.interval_progress < self.interval_samples
        {
            return None;
        }
        self.interval_progress = 0;

        Self::fill_window(
            self.window_samples,
            self.history_write_cursor,
            &self.reference_history,
            &mut self.reference_window,
        );
        Self::fill_window(
            self.window_samples,
            self.history_write_cursor,
            &self.observed_history,
            &mut self.observed_window,
        );

        let reference_rms = rms(&self.reference_window);
        let observed_rms = rms(&self.observed_window);
        let capture_time_sec = self.capture_time_sec();

        if reference_rms < self.min_rms || observed_rms < self.min_rms {
            self.rejections.low_energy += 1;
            let snapshot = self.handle_missing_interval(None, None);
            return Some(SyncProbeEvent::SkippedLowEnergy(SyncProbeLowEnergy {
                capture_time_sec,
                reference_rms,
                observed_rms,
                snapshot,
            }));
        }

        let estimate = self
            .estimator
            .estimate(&self.reference_window, &self.observed_window)?;

        let thresholds = self.thresholds_for_current_state();
        let confidence = Some(self.confidence_for(estimate, thresholds));

        if !self.meets_thresholds(estimate, thresholds) {
            self.rejections.weak_correlation += 1;
            let snapshot = self.handle_missing_interval(Some(estimate.lag_samples), confidence);
            return Some(SyncProbeEvent::SkippedLowConfidence(
                SyncProbeLowConfidence {
                    capture_time_sec,
                    estimate,
                    reference_rms,
                    observed_rms,
                    reason: SyncProbeLowConfidenceReason::WeakCorrelation,
                    snapshot,
                },
            ));
        }

        if self.is_locking_state()
            && self
                .stable_lag_samples
                .is_some_and(|stable| self.is_lag_outlier(estimate.lag_samples, stable))
        {
            self.rejections.lag_outlier += 1;
            let nearest_accepted_lag_samples = self.stable_lag_samples;
            let snapshot = self.handle_missing_interval(Some(estimate.lag_samples), confidence);
            return Some(SyncProbeEvent::SkippedLowConfidence(
                SyncProbeLowConfidence {
                    capture_time_sec,
                    estimate,
                    reference_rms,
                    observed_rms,
                    reason: SyncProbeLowConfidenceReason::LagOutlier {
                        nearest_accepted_lag_samples: nearest_accepted_lag_samples
                            .unwrap_or(estimate.lag_samples),
                    },
                    snapshot,
                },
            ));
        }

        let snapshot = self.handle_accepted_estimate(estimate.lag_samples, confidence);
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
            snapshot,
        }))
    }

    fn append_history(&mut self, reference: &[f32], observed: &[f32]) {
        for (&reference_sample, &observed_sample) in reference.iter().zip(observed.iter()) {
            self.reference_history[self.history_write_cursor] = reference_sample;
            self.observed_history[self.history_write_cursor] = observed_sample;
            self.history_write_cursor = (self.history_write_cursor + 1) % self.window_samples;
            self.history_len = (self.history_len + 1).min(self.window_samples);
        }
    }

    fn fill_window(
        window_samples: usize,
        history_write_cursor: usize,
        history: &[f32],
        window: &mut [f32],
    ) {
        let split = history_write_cursor;
        let tail = window_samples - split;
        window[..tail].copy_from_slice(&history[split..]);
        window[tail..].copy_from_slice(&history[..split]);
    }

    fn capture_time_sec(&self) -> f64 {
        self.processed_samples as f64 / self.sample_rate as f64
    }

    fn thresholds_for_current_state(&self) -> SyncProbeThresholds {
        if self.is_locking_state() {
            self.tuning.hold
        } else {
            self.tuning.acquire
        }
    }

    fn is_locking_state(&self) -> bool {
        matches!(
            self.state,
            SyncProbeState::Locked | SyncProbeState::Holdover
        ) && self.stable_lag_samples.is_some()
    }

    fn meets_thresholds(&self, estimate: LagEstimate, thresholds: SyncProbeThresholds) -> bool {
        estimate.peak_ratio >= thresholds.min_peak_ratio.max(0.0)
            && estimate.distinctiveness >= thresholds.min_distinctiveness.max(0.0)
    }

    fn confidence_for(&self, estimate: LagEstimate, thresholds: SyncProbeThresholds) -> f32 {
        let peak_ratio = if thresholds.min_peak_ratio <= f32::EPSILON {
            1.0
        } else {
            estimate.peak_ratio / thresholds.min_peak_ratio
        };
        let distinctiveness = if thresholds.min_distinctiveness <= f32::EPSILON {
            1.0
        } else {
            estimate.distinctiveness / thresholds.min_distinctiveness
        };

        peak_ratio.min(distinctiveness)
    }

    fn is_lag_outlier(&self, lag_samples: isize, stable_lag_samples: isize) -> bool {
        (lag_samples - stable_lag_samples).unsigned_abs()
            > self.tuning.lock_outlier_tolerance_samples
    }

    fn handle_missing_interval(
        &mut self,
        candidate_lag_samples: Option<isize>,
        confidence: Option<f32>,
    ) -> SyncProbeSnapshot {
        if self.is_locking_state() {
            return self.advance_holdover_or_loss(candidate_lag_samples, confidence);
        }

        if matches!(self.state, SyncProbeState::Acquiring) {
            self.push_acquisition_entry(None);
            if self.acquisition_match_count() == 0 {
                self.state = SyncProbeState::Searching;
                self.acquisition_entries.clear();
            }
        } else {
            self.state = SyncProbeState::Searching;
        }

        self.snapshot(self.state, candidate_lag_samples, confidence)
    }

    fn advance_holdover_or_loss(
        &mut self,
        candidate_lag_samples: Option<isize>,
        confidence: Option<f32>,
    ) -> SyncProbeSnapshot {
        self.consecutive_rejections += 1;
        let lost_after = self
            .tuning
            .lost_after_rejections
            .max(self.tuning.hold_interval_count.saturating_add(1))
            .max(1);

        if self.consecutive_rejections >= lost_after {
            let snapshot = self.snapshot(SyncProbeState::Lost, candidate_lag_samples, confidence);
            self.reset_after_loss();
            return snapshot;
        }

        self.state = SyncProbeState::Holdover;
        self.snapshot(SyncProbeState::Holdover, candidate_lag_samples, confidence)
    }

    fn handle_accepted_estimate(
        &mut self,
        lag_samples: isize,
        confidence: Option<f32>,
    ) -> SyncProbeSnapshot {
        self.consecutive_rejections = 0;

        if self.is_locking_state() {
            self.state = SyncProbeState::Locked;
            self.push_stable_lag(lag_samples);
            return self.snapshot(SyncProbeState::Locked, Some(lag_samples), confidence);
        }

        if let Some(center) = self.acquisition_center() {
            if (lag_samples - center).unsigned_abs() > self.tuning.acquire_cluster_tolerance_samples
            {
                self.acquisition_entries.clear();
            }
        }

        self.push_acquisition_entry(Some(lag_samples));
        self.state = SyncProbeState::Acquiring;

        if self.acquisition_match_count() >= self.tuning.acquire_lock_count.max(1) {
            self.promote_acquisition_to_lock();
            return self.snapshot(SyncProbeState::Locked, Some(lag_samples), confidence);
        }

        self.snapshot(SyncProbeState::Acquiring, Some(lag_samples), confidence)
    }

    fn push_acquisition_entry(&mut self, lag_samples: Option<isize>) {
        let acquire_window = self.tuning.acquire_window.max(1);
        if self.acquisition_entries.len() == acquire_window {
            self.acquisition_entries.pop_front();
        }
        self.acquisition_entries.push_back(lag_samples);
    }

    fn acquisition_match_count(&self) -> usize {
        self.acquisition_entries.iter().flatten().count()
    }

    fn acquisition_center(&self) -> Option<isize> {
        median_isize(self.acquisition_entries.iter().flatten().copied())
    }

    fn promote_acquisition_to_lock(&mut self) {
        let lags: Vec<isize> = self.acquisition_entries.iter().flatten().copied().collect();
        self.stable_lag_history.clear();
        for lag in lags {
            self.push_stable_lag(lag);
        }
        self.acquisition_entries.clear();
        self.state = SyncProbeState::Locked;
    }

    fn push_stable_lag(&mut self, lag_samples: isize) {
        let stable_lag_window = self.tuning.stable_lag_window.max(1);
        if self.stable_lag_history.len() == stable_lag_window {
            self.stable_lag_history.pop_front();
        }
        self.stable_lag_history.push_back(lag_samples);
        self.stable_lag_samples = median_isize(self.stable_lag_history.iter().copied());
    }

    fn reset_after_loss(&mut self) {
        self.state = SyncProbeState::Searching;
        self.stable_lag_samples = None;
        self.stable_lag_history.clear();
        self.acquisition_entries.clear();
        self.consecutive_rejections = 0;
    }

    fn snapshot(
        &self,
        state: SyncProbeState,
        candidate_lag_samples: Option<isize>,
        confidence: Option<f32>,
    ) -> SyncProbeSnapshot {
        let accepted_window_count = match state {
            SyncProbeState::Searching => 0,
            SyncProbeState::Acquiring => self.acquisition_match_count(),
            SyncProbeState::Locked | SyncProbeState::Holdover | SyncProbeState::Lost => {
                self.stable_lag_history.len()
            }
        };

        SyncProbeSnapshot {
            state,
            stable_lag_samples: self.stable_lag_samples,
            candidate_lag_samples,
            accepted_window_count,
            confidence,
            rejections: self.rejections,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SyncProbeEvent {
    SkippedLowEnergy(SyncProbeLowEnergy),
    SkippedLowConfidence(SyncProbeLowConfidence),
    Measured(SyncProbeMeasurement),
}

impl SyncProbeEvent {
    pub fn snapshot(&self) -> SyncProbeSnapshot {
        match self {
            Self::SkippedLowEnergy(event) => event.snapshot,
            Self::SkippedLowConfidence(event) => event.snapshot,
            Self::Measured(event) => event.snapshot,
        }
    }

    pub fn capture_time_sec(&self) -> f64 {
        match self {
            Self::SkippedLowEnergy(event) => event.capture_time_sec,
            Self::SkippedLowConfidence(event) => event.capture_time_sec,
            Self::Measured(event) => event.capture_time_sec,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SyncProbeLowEnergy {
    pub capture_time_sec: f64,
    pub reference_rms: f32,
    pub observed_rms: f32,
    pub snapshot: SyncProbeSnapshot,
}

#[derive(Debug, Clone, Copy)]
pub struct SyncProbeLowConfidence {
    pub capture_time_sec: f64,
    pub estimate: LagEstimate,
    pub reference_rms: f32,
    pub observed_rms: f32,
    pub reason: SyncProbeLowConfidenceReason,
    pub snapshot: SyncProbeSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncProbeLowConfidenceReason {
    WeakCorrelation,
    LagOutlier { nearest_accepted_lag_samples: isize },
}

#[derive(Debug, Clone, Copy)]
pub struct SyncProbeMeasurement {
    pub capture_time_sec: f64,
    pub estimate: LagEstimate,
    pub reference_rms: f32,
    pub observed_rms: f32,
    pub trend: DriftTrendSnapshot,
    pub snapshot: SyncProbeSnapshot,
}

fn rms(data: &[f32]) -> f32 {
    let energy = data.iter().map(|sample| sample * sample).sum::<f32>() / data.len().max(1) as f32;
    energy.sqrt()
}

fn median_isize(iter: impl IntoIterator<Item = isize>) -> Option<isize> {
    let mut values: Vec<isize> = iter.into_iter().collect();
    if values.is_empty() {
        return None;
    }

    values.sort_unstable();
    let mid = values.len() / 2;
    Some(if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2
    } else {
        values[mid]
    })
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

    fn test_config(window_samples: usize) -> SyncProbeConfig {
        SyncProbeConfig {
            sample_rate: 16_000,
            window_samples,
            max_lag_samples: 512,
            interval_samples: window_samples,
            min_rms: 0.0,
            tuning: SyncProbeTuning {
                acquire: SyncProbeThresholds::new(0.0, 0.0),
                hold: SyncProbeThresholds::new(0.0, 0.0),
                acquire_lock_count: 3,
                acquire_window: 4,
                acquire_cluster_tolerance_samples: 24,
                hold_interval_count: 2,
                lost_after_rejections: 3,
                lock_outlier_tolerance_samples: 48,
                stable_lag_window: 5,
            },
            level_interval_samples: 16_000,
        }
    }

    #[test]
    fn sync_probe_requires_acquisition_before_locking() {
        let window = 4096;
        let delay = 192;
        let reference = excitation(window);
        let observed = delay_signal(&reference, delay);
        let mut probe = SyncProbe::new(test_config(window));

        let first = probe.observe(&reference, &observed).unwrap();
        assert_eq!(first.snapshot().state, SyncProbeState::Acquiring);

        let second = probe.observe(&reference, &observed).unwrap();
        assert_eq!(second.snapshot().state, SyncProbeState::Acquiring);

        let third = probe.observe(&reference, &observed).unwrap();
        let snapshot = third.snapshot();
        assert_eq!(snapshot.state, SyncProbeState::Locked);
        assert_eq!(snapshot.stable_lag_samples, Some(delay as isize));
        assert_eq!(snapshot.accepted_window_count, 3);
    }

    #[test]
    fn sync_probe_enters_holdover_and_then_lost() {
        let window = 4096;
        let delay = 192;
        let reference = excitation(window);
        let observed = delay_signal(&reference, delay);
        let silence = vec![0.0; window];
        let mut config = test_config(window);
        config.min_rms = 0.1;
        let mut probe = SyncProbe::new(config);

        for _ in 0..3 {
            let event = probe.observe(&reference, &observed).unwrap();
            assert!(matches!(event, SyncProbeEvent::Measured(_)));
        }

        let first = probe.observe(&silence, &silence).unwrap();
        assert_eq!(first.snapshot().state, SyncProbeState::Holdover);
        assert_eq!(first.snapshot().stable_lag_samples, Some(delay as isize));

        let second = probe.observe(&silence, &silence).unwrap();
        assert_eq!(second.snapshot().state, SyncProbeState::Holdover);

        let third = probe.observe(&silence, &silence).unwrap();
        assert_eq!(third.snapshot().state, SyncProbeState::Lost);

        let recovered = probe.observe(&reference, &observed).unwrap();
        assert_eq!(recovered.snapshot().state, SyncProbeState::Acquiring);
    }

    #[test]
    fn sync_probe_reports_weak_correlation_skip() {
        let window = 4096;
        let delay = 192;
        let reference = excitation(window);
        let observed = delay_signal(&reference, delay);
        let mut config = test_config(window);
        config.tuning.acquire = SyncProbeThresholds::new(0.0, 100.0);
        config.tuning.hold = config.tuning.acquire;
        let mut probe = SyncProbe::new(config);

        let event = probe.observe(&reference, &observed).unwrap();

        match event {
            SyncProbeEvent::SkippedLowConfidence(skip) => {
                assert!(matches!(
                    skip.reason,
                    SyncProbeLowConfidenceReason::WeakCorrelation
                ));
                assert_eq!(skip.snapshot.state, SyncProbeState::Searching);
            }
            SyncProbeEvent::Measured(_) => panic!("expected low-confidence skip"),
            SyncProbeEvent::SkippedLowEnergy(_) => panic!("expected low-confidence skip"),
        }
    }

    #[test]
    fn sync_probe_rejects_lag_outliers_while_locked() {
        let window = 4096;
        let delay = 192;
        let reference = excitation(window);
        let delayed = delay_signal(&reference, delay);
        let aligned = reference.clone();
        let mut probe = SyncProbe::new(test_config(window));

        for _ in 0..3 {
            let event = probe.observe(&reference, &delayed).unwrap();
            assert!(matches!(event, SyncProbeEvent::Measured(_)));
        }

        let event = probe.observe(&reference, &aligned).unwrap();

        match event {
            SyncProbeEvent::SkippedLowConfidence(skip) => {
                assert!(matches!(
                    skip.reason,
                    SyncProbeLowConfidenceReason::LagOutlier {
                        nearest_accepted_lag_samples: 192
                    }
                ));
                assert_eq!(skip.snapshot.state, SyncProbeState::Holdover);
            }
            SyncProbeEvent::Measured(_) => panic!("expected outlier rejection"),
            SyncProbeEvent::SkippedLowEnergy(_) => panic!("expected outlier rejection"),
        }
    }

    #[test]
    fn sync_probe_lag_outlier_can_transition_to_lost_without_panicking() {
        let window = 4096;
        let delay = 192;
        let reference = excitation(window);
        let delayed = delay_signal(&reference, delay);
        let aligned = reference.clone();
        let mut config = test_config(window);
        config.tuning.hold_interval_count = 0;
        config.tuning.lost_after_rejections = 1;
        let mut probe = SyncProbe::new(config);

        for _ in 0..3 {
            let event = probe.observe(&reference, &delayed).unwrap();
            assert!(matches!(event, SyncProbeEvent::Measured(_)));
        }

        let event = probe.observe(&reference, &aligned).unwrap();

        match event {
            SyncProbeEvent::SkippedLowConfidence(skip) => {
                assert!(matches!(
                    skip.reason,
                    SyncProbeLowConfidenceReason::LagOutlier {
                        nearest_accepted_lag_samples: 192
                    }
                ));
                assert_eq!(skip.snapshot.state, SyncProbeState::Lost);
            }
            SyncProbeEvent::Measured(_) => panic!("expected lost transition"),
            SyncProbeEvent::SkippedLowEnergy(_) => panic!("expected lost transition"),
        }
    }
}
