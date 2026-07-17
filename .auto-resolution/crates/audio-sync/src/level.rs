pub struct LevelAccumulator {
    pub(crate) interval_samples: usize,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_accumulator_reports_snapshot() {
        let mut levels = LevelAccumulator::new(4);
        assert!(levels.observe(&[0.0, 0.5]).is_none());
        let snapshot = levels.observe(&[0.25, -0.25]).unwrap();
        assert_eq!(snapshot.samples, 4);
        assert_eq!(snapshot.nonzero_ratio, 0.75);
        assert!(snapshot.peak >= 0.5);
    }
}
