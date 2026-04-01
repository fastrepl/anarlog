use std::sync::Arc;

use realfft::{ComplexToReal, RealFftPlanner, RealToComplex, num_complex::Complex32};

const PEAK_NEIGHBORHOOD: isize = 3;

pub struct GccPhatLagEstimator {
    pub(crate) window_samples: usize,
    pub(crate) max_lag_samples: usize,
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
}
