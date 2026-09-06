use serde::{Deserialize, Serialize};

/// Distribution summary for one metric over a run. `cv` is the coefficient of
/// variation (population standard deviation / mean), `None` when the mean is 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Distribution {
    pub count: usize,
    pub mean: f64,
    pub median: f64,
    pub p95: f64,
    pub min: f64,
    pub max: f64,
    pub cv: Option<f64>,
}

impl Distribution {
    pub fn of(values: &[f64]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        let mean = mean(values);
        Some(Self {
            count: values.len(),
            mean,
            median: median(values),
            p95: percentile(values, 95.0),
            min: values.iter().copied().fold(f64::INFINITY, f64::min),
            max: values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            cv: (mean != 0.0).then(|| std_dev(values) / mean),
        })
    }
}

pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Nearest-rank percentile; `p` in 0..=100.
pub fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

pub fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    (values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / values.len() as f64).sqrt()
}

/// Least-squares slope of `y` over `x`. Used for memory growth per minute.
pub fn slope(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }
    let mx = mean(x);
    let my = mean(y);
    let denominator: f64 = x.iter().map(|xi| (xi - mx).powi(2)).sum();
    if denominator == 0.0 {
        return None;
    }
    let numerator: f64 = x.iter().zip(y).map(|(xi, yi)| (xi - mx) * (yi - my)).sum();
    Some(numerator / denominator)
}

/// Trapezoidal integral of `values` sampled at `t_seconds`.
pub fn integrate(t_seconds: &[f64], values: &[f64]) -> f64 {
    t_seconds
        .windows(2)
        .zip(values.windows(2))
        .map(|(t, v)| (t[1] - t[0]) * (v[0] + v[1]) / 2.0)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_handles_even_and_odd_counts() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), 2.5);
        assert_eq!(median(&[]), 0.0);
    }

    #[test]
    fn percentile_uses_nearest_rank() {
        let values: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&values, 95.0), 95.0);
        assert_eq!(percentile(&values, 100.0), 100.0);
        assert_eq!(percentile(&values, 0.0), 1.0);
        assert_eq!(percentile(&[7.0], 95.0), 7.0);
    }

    #[test]
    fn distribution_reports_cv_only_for_nonzero_mean() {
        let d = Distribution::of(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]).unwrap();
        assert_eq!(d.mean, 5.0);
        assert_eq!(d.median, 4.5);
        assert_eq!(d.max, 9.0);
        assert!((d.cv.unwrap() - 0.4).abs() < 1e-12);
        assert_eq!(Distribution::of(&[0.0, 0.0]).unwrap().cv, None);
        assert!(Distribution::of(&[]).is_none());
    }

    #[test]
    fn slope_recovers_linear_growth() {
        let x = [0.0, 1.0, 2.0, 3.0];
        let y = [10.0, 12.0, 14.0, 16.0];
        assert!((slope(&x, &y).unwrap() - 2.0).abs() < 1e-12);
        assert_eq!(slope(&[1.0], &[1.0]), None);
        assert_eq!(slope(&[1.0, 1.0], &[1.0, 2.0]), None);
    }

    #[test]
    fn integrate_is_trapezoidal() {
        assert_eq!(integrate(&[0.0, 1.0, 2.0], &[100.0, 100.0, 100.0]), 200.0);
        assert_eq!(integrate(&[0.0, 2.0], &[0.0, 100.0]), 100.0);
    }
}
