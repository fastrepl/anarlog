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

#[cfg(test)]
mod tests {
    use super::*;

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
}
