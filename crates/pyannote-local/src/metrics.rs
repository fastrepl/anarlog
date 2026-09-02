//! Diarization error rate with an optimal one-to-one speaker mapping and a
//! collar around reference boundaries, matching the pyannote.metrics
//! conventions (`collar` is the total width, split evenly on both sides).

use std::collections::BTreeMap;

use crate::SpeakerSegment;

const RESOLUTION: f64 = 0.01;

#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceTurn {
    pub start: f64,
    pub end: f64,
    pub speaker: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DerReport {
    /// Reference speech scored, in seconds.
    pub total: f64,
    pub missed: f64,
    pub false_alarm: f64,
    pub confusion: f64,
    pub reference_speakers: usize,
    pub hypothesis_speakers: usize,
}

impl DerReport {
    pub fn der(&self) -> f64 {
        if self.total <= 0.0 {
            return 0.0;
        }
        (self.missed + self.false_alarm + self.confusion) / self.total
    }

    pub fn accumulate(&mut self, other: &DerReport) {
        self.total += other.total;
        self.missed += other.missed;
        self.false_alarm += other.false_alarm;
        self.confusion += other.confusion;
        self.reference_speakers += other.reference_speakers;
        self.hypothesis_speakers += other.hypothesis_speakers;
    }
}

pub fn diarization_error_rate(
    reference: &[ReferenceTurn],
    hypothesis: &[SpeakerSegment],
    collar: f64,
) -> DerReport {
    let mut reference_ids: BTreeMap<&str, usize> = BTreeMap::new();
    for turn in reference {
        let next = reference_ids.len();
        reference_ids.entry(turn.speaker.as_str()).or_insert(next);
    }
    let hypothesis_speakers = hypothesis
        .iter()
        .map(|segment| segment.speaker + 1)
        .max()
        .unwrap_or(0);

    let end = reference
        .iter()
        .map(|turn| turn.end)
        .chain(hypothesis.iter().map(|segment| segment.end))
        .fold(0.0, f64::max);
    let bins = (end / RESOLUTION).ceil() as usize;
    let mut reference_active = vec![0u64; bins];
    let mut hypothesis_active = vec![0u64; bins];
    let mut excluded = vec![false; bins];

    let to_bin = |time: f64| -> usize { ((time / RESOLUTION).round().max(0.0)) as usize };
    for turn in reference {
        let id = reference_ids[turn.speaker.as_str()];
        let (from, to) = (to_bin(turn.start).min(bins), to_bin(turn.end).min(bins));
        for slot in &mut reference_active[from..to.max(from)] {
            *slot |= 1 << id.min(63);
        }
        for boundary in [turn.start, turn.end] {
            let from = to_bin(boundary - collar / 2.0).min(bins);
            let to = to_bin(boundary + collar / 2.0).min(bins);
            excluded[from..to.max(from)].fill(true);
        }
    }
    for segment in hypothesis {
        let (from, to) = (
            to_bin(segment.start).min(bins),
            to_bin(segment.end).min(bins),
        );
        for slot in &mut hypothesis_active[from..to.max(from)] {
            *slot |= 1 << segment.speaker.min(63);
        }
    }

    let reference_count = reference_ids.len();
    let mut overlap = vec![vec![0.0f64; hypothesis_speakers]; reference_count];
    for bin in 0..bins {
        if excluded[bin] {
            continue;
        }
        for (r, row) in overlap.iter_mut().enumerate() {
            if reference_active[bin] & (1 << r) == 0 {
                continue;
            }
            for (h, cell) in row.iter_mut().enumerate() {
                if hypothesis_active[bin] & (1 << h) != 0 {
                    *cell += RESOLUTION;
                }
            }
        }
    }
    let mapping = max_weight_matching(&overlap);
    let mut hypothesis_to_reference = vec![None; hypothesis_speakers];
    for (r, h) in mapping {
        hypothesis_to_reference[h] = Some(r);
    }

    let mut report = DerReport {
        reference_speakers: reference_count,
        hypothesis_speakers,
        ..Default::default()
    };
    for bin in 0..bins {
        if excluded[bin] {
            continue;
        }
        let reference_set = reference_active[bin];
        let reference_size = reference_set.count_ones() as f64;
        let mut hypothesis_size = 0.0;
        let mut correct = 0.0;
        for (h, mapped) in hypothesis_to_reference.iter().enumerate() {
            if hypothesis_active[bin] & (1 << h) == 0 {
                continue;
            }
            hypothesis_size += 1.0;
            if let Some(r) = *mapped
                && reference_set & (1 << r) != 0
            {
                correct += 1.0;
            }
        }
        report.total += reference_size * RESOLUTION;
        report.missed += (reference_size - hypothesis_size).max(0.0) * RESOLUTION;
        report.false_alarm += (hypothesis_size - reference_size).max(0.0) * RESOLUTION;
        report.confusion += (reference_size.min(hypothesis_size) - correct).max(0.0) * RESOLUTION;
    }
    report
}

/// Hungarian algorithm on a rectangular weight matrix; returns `(row, col)`
/// pairs maximising total weight.
fn max_weight_matching(weights: &[Vec<f64>]) -> Vec<(usize, usize)> {
    let rows = weights.len();
    let cols = weights.first().map_or(0, |row| row.len());
    if rows == 0 || cols == 0 {
        return Vec::new();
    }
    let n = rows.max(cols);
    let max = weights.iter().flatten().copied().fold(0.0, f64::max);
    let cost = |r: usize, c: usize| -> f64 {
        if r < rows && c < cols {
            max - weights[r][c]
        } else {
            max
        }
    };

    let inf = f64::INFINITY;
    let mut u = vec![0.0; n + 1];
    let mut v = vec![0.0; n + 1];
    let mut p = vec![0usize; n + 1];
    let mut way = vec![0usize; n + 1];
    for i in 1..=n {
        p[0] = i;
        let mut j0 = 0;
        let mut minv = vec![inf; n + 1];
        let mut used = vec![false; n + 1];
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let mut delta = inf;
            let mut j1 = 0;
            for j in 1..=n {
                if used[j] {
                    continue;
                }
                let current = cost(i0 - 1, j - 1) - u[i0] - v[j];
                if current < minv[j] {
                    minv[j] = current;
                    way[j] = j0;
                }
                if minv[j] < delta {
                    delta = minv[j];
                    j1 = j;
                }
            }
            for j in 0..=n {
                if used[j] {
                    u[p[j]] += delta;
                    v[j] -= delta;
                } else {
                    minv[j] -= delta;
                }
            }
            j0 = j1;
            if p[j0] == 0 {
                break;
            }
        }
        loop {
            let j1 = way[j0];
            p[j0] = p[j1];
            j0 = j1;
            if j0 == 0 {
                break;
            }
        }
    }

    (1..=n)
        .filter_map(|j| {
            let i = p[j];
            (i >= 1 && i - 1 < rows && j - 1 < cols && weights[i - 1][j - 1] > 0.0)
                .then_some((i - 1, j - 1))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(start: f64, end: f64, speaker: &str) -> ReferenceTurn {
        ReferenceTurn {
            start,
            end,
            speaker: speaker.to_string(),
        }
    }

    fn segment(start: f64, end: f64, speaker: usize) -> SpeakerSegment {
        SpeakerSegment {
            start,
            end,
            speaker,
        }
    }

    #[test]
    fn perfect_hypothesis_scores_zero_under_any_labelling() {
        let reference = vec![turn(0.0, 5.0, "A"), turn(5.0, 10.0, "B")];
        let hypothesis = vec![segment(0.0, 5.0, 1), segment(5.0, 10.0, 0)];
        let report = diarization_error_rate(&reference, &hypothesis, 0.0);
        assert!(report.der().abs() < 1e-9, "{report:?}");
        assert!((report.total - 10.0).abs() < 1e-6);
    }

    #[test]
    fn missed_false_alarm_and_confusion_are_separated() {
        let reference = vec![turn(0.0, 4.0, "A"), turn(4.0, 8.0, "B")];
        let hypothesis = vec![
            segment(0.0, 2.0, 0),
            segment(2.0, 4.0, 1),
            segment(4.0, 8.0, 1),
            segment(8.0, 9.0, 1),
        ];
        let report = diarization_error_rate(&reference, &hypothesis, 0.0);
        assert!((report.missed - 0.0).abs() < 1e-6);
        assert!((report.false_alarm - 1.0).abs() < 1e-6, "{report:?}");
        assert!((report.confusion - 2.0).abs() < 1e-6, "{report:?}");
        assert!((report.der() - 3.0 / 8.0).abs() < 1e-6);
    }

    #[test]
    fn collar_forgives_boundary_jitter() {
        let reference = vec![turn(0.0, 5.0, "A"), turn(5.0, 10.0, "B")];
        let hypothesis = vec![segment(0.0, 5.1, 0), segment(5.1, 10.0, 1)];
        let strict = diarization_error_rate(&reference, &hypothesis, 0.0);
        let lenient = diarization_error_rate(&reference, &hypothesis, 0.25);
        assert!(strict.der() > 0.0);
        assert!(lenient.der().abs() < 1e-9, "{lenient:?}");
    }

    #[test]
    fn overlapping_reference_counts_both_speakers() {
        let reference = vec![turn(0.0, 4.0, "A"), turn(2.0, 4.0, "B")];
        let hypothesis = vec![segment(0.0, 4.0, 0)];
        let report = diarization_error_rate(&reference, &hypothesis, 0.0);
        assert!((report.total - 6.0).abs() < 1e-6);
        assert!((report.missed - 2.0).abs() < 1e-6, "{report:?}");
    }

    #[test]
    fn empty_hypothesis_is_all_missed() {
        let reference = vec![turn(0.0, 3.0, "A")];
        let report = diarization_error_rate(&reference, &[], 0.0);
        assert!((report.der() - 1.0).abs() < 1e-9);
        assert_eq!(report.hypothesis_speakers, 0);
    }
}
