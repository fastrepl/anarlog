use hypr_onnx::{
    ndarray::{self, ArrayBase, Axis, IxDyn, ViewRepr},
    ort::{self, session::Session, value::TensorRef},
};

use crate::{Error, Result};

pub mod model;

pub use model::{FRAME_SIZE, FRAME_START};

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub sample_start: usize,
    pub sample_end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SegmenterConfig {
    pub step_seconds: f64,
    pub onset: f32,
    pub offset: f32,
    pub min_duration_on_seconds: f64,
    pub min_duration_off_seconds: f64,
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        Self {
            step_seconds: model::WINDOW_STEP_SECONDS,
            onset: model::ONSET_THRESHOLD,
            offset: model::OFFSET_THRESHOLD,
            min_duration_on_seconds: model::MIN_DURATION_ON_SECONDS,
            min_duration_off_seconds: model::MIN_DURATION_OFF_SECONDS,
        }
    }
}

#[derive(Debug)]
pub struct Segmenter {
    session: Session,
    sample_rate: u32,
    window_size: usize,
    window_step_size: usize,
    frame_size: usize,
    frame_start: usize,
    step_seconds: f64,
    onset: f32,
    offset: f32,
    min_duration_on_samples: usize,
    min_duration_off_samples: usize,
}

impl Segmenter {
    pub fn new(sample_rate: u32) -> Result<Self> {
        Self::from_model_bytes(model::BYTES, sample_rate)
    }

    pub fn from_model_bytes(model_bytes: &[u8], sample_rate: u32) -> Result<Self> {
        let session = hypr_onnx::load_model_from_bytes(model_bytes)?;
        Self::with_session(session, sample_rate)
    }

    pub fn with_session(session: Session, sample_rate: u32) -> Result<Self> {
        let window_size = sample_rate as usize * model::WINDOW_SECONDS;
        let mut segmenter = Self {
            session,
            sample_rate,
            window_size,
            window_step_size: 0,
            frame_size: model::FRAME_SIZE,
            frame_start: model::FRAME_START,
            step_seconds: model::WINDOW_STEP_SECONDS,
            onset: model::ONSET_THRESHOLD,
            offset: model::OFFSET_THRESHOLD,
            min_duration_on_samples: 0,
            min_duration_off_samples: 0,
        };
        segmenter.apply_config(&SegmenterConfig::default())?;
        Ok(segmenter)
    }

    pub fn with_config(mut self, config: SegmenterConfig) -> Result<Self> {
        self.apply_config(&config)?;
        Ok(self)
    }

    pub fn with_step_seconds(mut self, step_seconds: f64) -> Result<Self> {
        let mut config = self.config();
        config.step_seconds = step_seconds;
        self.apply_config(&config)?;
        Ok(self)
    }

    pub fn with_thresholds(mut self, onset: f32, offset: f32) -> Result<Self> {
        let mut config = self.config();
        config.onset = onset;
        config.offset = offset;
        self.apply_config(&config)?;
        Ok(self)
    }

    pub fn with_min_durations(
        mut self,
        min_duration_on_seconds: f64,
        min_duration_off_seconds: f64,
    ) -> Result<Self> {
        let mut config = self.config();
        config.min_duration_on_seconds = min_duration_on_seconds;
        config.min_duration_off_seconds = min_duration_off_seconds;
        self.apply_config(&config)?;
        Ok(self)
    }

    pub fn config(&self) -> SegmenterConfig {
        SegmenterConfig {
            step_seconds: self.step_seconds,
            onset: self.onset,
            offset: self.offset,
            min_duration_on_seconds: self.min_duration_on_samples as f64 / self.sample_rate as f64,
            min_duration_off_seconds: self.min_duration_off_samples as f64
                / self.sample_rate as f64,
        }
    }

    fn apply_config(&mut self, config: &SegmenterConfig) -> Result<()> {
        validate_config(config, self.window_size, self.sample_rate)?;

        self.step_seconds = config.step_seconds;
        self.window_step_size = seconds_to_samples(config.step_seconds, self.sample_rate);
        self.onset = config.onset;
        self.offset = config.offset;
        self.min_duration_on_samples =
            seconds_to_samples(config.min_duration_on_seconds, self.sample_rate);
        self.min_duration_off_samples =
            seconds_to_samples(config.min_duration_off_seconds, self.sample_rate);
        Ok(())
    }

    pub fn process(&mut self, samples: &[i16], sample_rate: u32) -> Result<Vec<Segment>> {
        if sample_rate != self.sample_rate {
            return Err(Error::SampleRateMismatch {
                expected: self.sample_rate,
                actual: sample_rate,
            });
        }

        let chunk_starts = self.chunk_starts(samples.len());
        if chunk_starts.is_empty() {
            return Ok(Vec::new());
        }

        let mut chunk_scores = Vec::with_capacity(chunk_starts.len());
        for chunk_start in chunk_starts {
            let window = self.window_from(samples, chunk_start);
            let outputs = self.infer_window(&window)?;
            let speech_scores = chunk_speech_scores(outputs.view())?;
            chunk_scores.push(ChunkScores {
                chunk_start,
                speech_scores,
            });
        }

        let aggregated_scores = aggregate_scores(
            &chunk_scores,
            self.frame_size,
            self.frame_start,
            self.window_size,
            self.window_step_size,
        );

        let raw_segments = scores_to_segments(
            &aggregated_scores,
            self.frame_size,
            self.frame_start,
            self.onset,
            self.offset,
        );

        let post_processed_segments = post_process_segments(
            raw_segments,
            self.min_duration_on_samples,
            self.min_duration_off_samples,
        );

        Ok(finalize_segments(
            post_processed_segments,
            sample_rate,
            samples.len(),
        ))
    }

    fn chunk_starts(&self, num_samples: usize) -> Vec<usize> {
        if self.window_step_size == 0 {
            return Vec::new();
        }

        let mut starts = Vec::new();
        let mut num_full_chunks = 0usize;

        if num_samples >= self.window_size {
            num_full_chunks = 1 + ((num_samples - self.window_size) / self.window_step_size);
            for idx in 0..num_full_chunks {
                starts.push(idx * self.window_step_size);
            }
        }

        let has_last_chunk = num_samples < self.window_size
            || !(num_samples - self.window_size).is_multiple_of(self.window_step_size);

        if has_last_chunk {
            starts.push(num_full_chunks * self.window_step_size);
        }

        starts
    }

    fn window_from(&self, samples: &[i16], chunk_start: usize) -> Vec<i16> {
        let end = chunk_start + self.window_size;
        if end <= samples.len() {
            return samples[chunk_start..end].to_vec();
        }

        let available = samples.len().saturating_sub(chunk_start);
        let mut padded = Vec::with_capacity(self.window_size);
        padded.extend_from_slice(&samples[chunk_start..chunk_start + available]);
        padded.resize(self.window_size, 0);
        padded
    }

    fn infer_window(&mut self, window: &[i16]) -> Result<ndarray::ArrayD<f32>> {
        let array = ndarray::Array1::from_iter(window.iter().map(|&x| x as f32))
            .insert_axis(Axis(0))
            .insert_axis(Axis(1))
            .into_dyn();

        let inputs = ort::inputs![TensorRef::from_array_view(array.view())?];
        let run_output = self.session.run(inputs)?;
        let output_tensor = run_output.values().next().ok_or(Error::EmptyOutputRow)?;
        Ok(output_tensor.try_extract_array::<f32>()?.to_owned())
    }
}

fn validate_config(config: &SegmenterConfig, window_size: usize, sample_rate: u32) -> Result<()> {
    if !(config.step_seconds.is_finite() && config.step_seconds > 0.0) {
        return Err(Error::InvalidConfiguration {
            field: "step_seconds",
            reason: "must be a finite value greater than 0".to_string(),
        });
    }

    let window_seconds = window_size as f64 / sample_rate as f64;
    if config.step_seconds > window_seconds {
        return Err(Error::InvalidConfiguration {
            field: "step_seconds",
            reason: format!("must be <= window duration ({window_seconds:.3}s)"),
        });
    }

    for (field, value) in [("onset", config.onset), ("offset", config.offset)] {
        if !(value.is_finite() && (0.0..=1.0).contains(&value)) {
            return Err(Error::InvalidConfiguration {
                field,
                reason: "must be between 0.0 and 1.0".to_string(),
            });
        }
    }

    for (field, value) in [
        ("min_duration_on_seconds", config.min_duration_on_seconds),
        ("min_duration_off_seconds", config.min_duration_off_seconds),
    ] {
        if !(value.is_finite() && value >= 0.0) {
            return Err(Error::InvalidConfiguration {
                field,
                reason: "must be a finite value >= 0".to_string(),
            });
        }
    }

    Ok(())
}

fn seconds_to_samples(seconds: f64, sample_rate: u32) -> usize {
    (seconds * sample_rate as f64).round() as usize
}

#[derive(Debug)]
struct ChunkScores {
    chunk_start: usize,
    speech_scores: Vec<f32>,
}

fn chunk_speech_scores(outputs: ArrayBase<ViewRepr<&f32>, IxDyn>) -> Result<Vec<f32>> {
    let mut speech_scores = Vec::new();
    for row in outputs.outer_iter() {
        for sub_row in row.axis_iter(Axis(0)) {
            let max_index = find_max_index(sub_row)?;
            speech_scores.push(if max_index == 0 { 0.0 } else { 1.0 });
        }
    }

    Ok(speech_scores)
}

fn aggregate_scores(
    chunk_scores: &[ChunkScores],
    frame_size: usize,
    frame_start: usize,
    window_size: usize,
    window_step_size: usize,
) -> Vec<f32> {
    if chunk_scores.is_empty() {
        return Vec::new();
    }

    let num_frames_per_chunk = chunk_scores[0].speech_scores.len();
    let hamming = hamming_window(num_frames_per_chunk);

    let max_chunk_start = chunk_scores
        .iter()
        .map(|chunk| chunk.chunk_start)
        .max()
        .unwrap_or(0);
    let estimated_end = frame_start + window_size + max_chunk_start + window_step_size;
    let max_frames = (estimated_end / frame_size) + num_frames_per_chunk + 2;

    let mut aggregated = vec![0.0f64; max_frames];
    let mut weights = vec![0.0f64; max_frames];

    for chunk in chunk_scores {
        for (frame_idx, score) in chunk.speech_scores.iter().enumerate() {
            let center_sample = chunk.chunk_start + frame_start + (frame_idx * frame_size);
            let global_frame = (((center_sample as f64) - (frame_start as f64))
                / (frame_size as f64))
                .round() as usize;
            let weight = hamming[frame_idx];
            aggregated[global_frame] += (*score as f64) * weight;
            weights[global_frame] += weight;
        }
    }

    aggregated
        .into_iter()
        .zip(weights)
        .map(|(sum, weight)| {
            if weight > 0.0 {
                (sum / weight) as f32
            } else {
                0.0
            }
        })
        .collect()
}

fn hamming_window(size: usize) -> Vec<f64> {
    if size <= 1 {
        return vec![1.0; size];
    }

    (0..size)
        .map(|idx| {
            0.54 - (0.46 * ((2.0 * std::f64::consts::PI * idx as f64) / (size - 1) as f64).cos())
        })
        .collect()
}

fn scores_to_segments(
    scores: &[f32],
    frame_size: usize,
    frame_start: usize,
    onset: f32,
    offset: f32,
) -> Vec<(usize, usize)> {
    if scores.is_empty() {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let frame_center = |index: usize| frame_start + (index * frame_size);

    let mut start = frame_center(0);
    let mut is_active = scores[0] > onset;

    for (frame_idx, score) in scores.iter().enumerate().skip(1) {
        let timestamp = frame_center(frame_idx);
        if is_active {
            if *score < offset {
                segments.push((start, timestamp));
                start = timestamp;
                is_active = false;
            }
        } else if *score > onset {
            start = timestamp;
            is_active = true;
        }
    }

    if is_active {
        let last_timestamp = frame_center(scores.len() - 1);
        segments.push((start, last_timestamp));
    }

    segments
}

fn post_process_segments(
    mut segments: Vec<(usize, usize)>,
    min_duration_on_samples: usize,
    min_duration_off_samples: usize,
) -> Vec<(usize, usize)> {
    if segments.is_empty() {
        return segments;
    }

    if min_duration_off_samples > 0 {
        let mut merged = Vec::with_capacity(segments.len());
        let mut current = segments[0];

        for next in segments.iter().copied().skip(1) {
            let gap = next.0.saturating_sub(current.1);
            if gap < min_duration_off_samples {
                current.1 = next.1;
            } else {
                merged.push(current);
                current = next;
            }
        }
        merged.push(current);
        segments = merged;
    }

    if min_duration_on_samples > 0 {
        segments.retain(|(start, end)| end.saturating_sub(*start) >= min_duration_on_samples);
    }

    segments
}

fn finalize_segments(
    raw_segments: Vec<(usize, usize)>,
    sample_rate: u32,
    max_sample: usize,
) -> Vec<Segment> {
    raw_segments
        .into_iter()
        .filter_map(|(start, end)| create_segment(start, end, sample_rate, max_sample))
        .collect()
}

fn create_segment(
    start_offset: usize,
    end_offset: usize,
    sample_rate: u32,
    max_sample: usize,
) -> Option<Segment> {
    let segment_start = start_offset.min(max_sample);
    let segment_end = end_offset.min(max_sample);
    if segment_end <= segment_start {
        return None;
    }

    Some(Segment {
        start: segment_start as f64 / sample_rate as f64,
        end: segment_end as f64 / sample_rate as f64,
        sample_start: segment_start,
        sample_end: segment_end,
    })
}

fn find_max_index(row: ArrayBase<ViewRepr<&f32>, IxDyn>) -> Result<usize> {
    row.iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(idx, _)| idx)
        .ok_or(Error::EmptyOutputRow)
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;
    use serde::{Deserialize, Serialize};
    use std::path::Path;

    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    struct SegmentEntry {
        start: f64,
        end: f64,
        duration: f64,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct SegmentationSnapshot {
        segment_count: usize,
        speech_ratio: f64,
        total_duration: f64,
        segments: Vec<SegmentEntry>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct PyannoteReferenceSegment {
        index: usize,
        start_sec: f64,
        end_sec: f64,
        duration_sec: f64,
        sample_start: usize,
        sample_end: usize,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct PyannoteReferenceSnapshot {
        model: String,
        sample_rate: u32,
        segment_count: usize,
        segments: Vec<PyannoteReferenceSegment>,
    }

    fn pcm_bytes_to_i16(bytes: &[u8]) -> Vec<i16> {
        bytes
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect()
    }

    fn save_snapshot(snapshot: &SegmentationSnapshot, path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let json = serde_json::to_string_pretty(snapshot).unwrap();
        std::fs::write(path, json).unwrap();
    }

    fn load_snapshot(path: &Path) -> SegmentationSnapshot {
        let json = std::fs::read_to_string(path).unwrap_or_else(|_| {
            panic!(
                "Snapshot file not found: {}\nRun with UPDATE_SNAPSHOTS=1 to generate baselines.",
                path.display()
            )
        });
        serde_json::from_str(&json).unwrap()
    }

    fn load_pyannote_reference(path: &Path) -> PyannoteReferenceSnapshot {
        let json = std::fs::read_to_string(path).unwrap_or_else(|_| {
            panic!(
                "Pyannote reference snapshot not found: {}\nGenerate it from the official pipeline before running this test.",
                path.display()
            )
        });
        serde_json::from_str(&json).unwrap()
    }

    fn merge_intervals(mut intervals: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
        intervals.sort_unstable();

        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (start, end) in intervals {
            if let Some((_, last_end)) = merged.last_mut()
                && start <= *last_end
            {
                *last_end = (*last_end).max(end);
                continue;
            }

            merged.push((start, end));
        }

        merged
    }

    fn intervals_total(intervals: &[(usize, usize)]) -> usize {
        intervals.iter().map(|(start, end)| end - start).sum()
    }

    fn intervals_intersection(a: &[(usize, usize)], b: &[(usize, usize)]) -> usize {
        let mut i = 0;
        let mut j = 0;
        let mut total = 0;

        while i < a.len() && j < b.len() {
            let (a_start, a_end) = a[i];
            let (b_start, b_end) = b[j];
            let overlap_start = a_start.max(b_start);
            let overlap_end = a_end.min(b_end);

            if overlap_end > overlap_start {
                total += overlap_end - overlap_start;
            }

            if a_end < b_end {
                i += 1;
            } else {
                j += 1;
            }
        }

        total
    }

    fn assert_snapshot_eq(actual: &SegmentationSnapshot, expected: &SegmentationSnapshot) {
        assert_eq!(actual.segment_count, expected.segment_count);
        assert_abs_diff_eq!(actual.speech_ratio, expected.speech_ratio, epsilon = 0.02);
        assert_abs_diff_eq!(
            actual.total_duration,
            expected.total_duration,
            epsilon = 0.05
        );
        assert_eq!(actual.segments.len(), expected.segments.len());

        for (i, (a, e)) in actual
            .segments
            .iter()
            .zip(expected.segments.iter())
            .enumerate()
        {
            assert_abs_diff_eq!(a.start, e.start, epsilon = 0.05);
            assert_abs_diff_eq!(a.end, e.end, epsilon = 0.05);
            assert_abs_diff_eq!(a.duration, e.duration, epsilon = 0.05);
            if (a.start - e.start).abs() > 0.05 || (a.end - e.end).abs() > 0.05 {
                panic!(
                    "segment mismatch at index {i}: actual=({:.3}, {:.3}) expected=({:.3}, {:.3})",
                    a.start, a.end, e.start, e.end
                );
            }
        }
    }

    fn run_snapshot_test(audio_bytes: &[u8], snapshot_name: &str) {
        let samples = pcm_bytes_to_i16(audio_bytes);
        let total_samples = samples.len();
        let sample_rate = 16000u32;

        let mut segmenter = Segmenter::new(sample_rate).unwrap();
        let segments = segmenter.process(&samples, sample_rate).unwrap();
        assert!(!segments.is_empty());

        let speech_samples: usize = segments
            .iter()
            .map(|segment| segment.sample_end.saturating_sub(segment.sample_start))
            .sum();
        let speech_ratio = speech_samples as f64 / total_samples as f64;
        let total_duration = total_samples as f64 / sample_rate as f64;

        let snapshot = SegmentationSnapshot {
            segment_count: segments.len(),
            speech_ratio,
            total_duration,
            segments: segments
                .iter()
                .map(|segment| SegmentEntry {
                    start: segment.start,
                    end: segment.end,
                    duration: segment.end - segment.start,
                })
                .collect(),
        };

        let snapshot_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("snapshots")
            .join(format!("{snapshot_name}.json"));

        if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
            save_snapshot(&snapshot, &snapshot_path);
            println!("Updated snapshot: {}", snapshot_path.display());
        } else {
            let expected = load_snapshot(&snapshot_path);
            assert_snapshot_eq(&snapshot, &expected);
        }
    }

    #[test]
    fn snapshot_english_1() {
        run_snapshot_test(hypr_data::english_1::AUDIO, "segmentation_english_1");
    }

    #[test]
    fn snapshot_english_2() {
        run_snapshot_test(hypr_data::english_2::AUDIO, "segmentation_english_2");
    }

    #[test]
    fn snapshot_korean_1() {
        run_snapshot_test(hypr_data::korean_1::AUDIO, "segmentation_korean_1");
    }

    #[test]
    fn aligns_with_pyannote_community1_on_english_1() {
        let samples = pcm_bytes_to_i16(hypr_data::english_1::AUDIO);
        let sample_rate = 16000u32;

        let mut segmenter = Segmenter::new(sample_rate).unwrap();
        let segments = segmenter.process(&samples, sample_rate).unwrap();

        let reference_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("snapshots")
            .join("segmentation_english_1_pyannote_community1.json");
        let reference = load_pyannote_reference(&reference_path);

        assert_eq!(reference.sample_rate, sample_rate);
        assert_eq!(segments.len(), reference.segment_count);

        let paired_len = segments.len().min(reference.segments.len());
        let mut max_abs_start_delta = 0.0f64;
        let mut max_abs_end_delta = 0.0f64;

        for (segment, expected) in segments
            .iter()
            .zip(reference.segments.iter())
            .take(paired_len)
        {
            max_abs_start_delta =
                max_abs_start_delta.max((segment.start - expected.start_sec).abs());
            max_abs_end_delta = max_abs_end_delta.max((segment.end - expected.end_sec).abs());
        }

        let ours = merge_intervals(
            segments
                .iter()
                .map(|segment| (segment.sample_start, segment.sample_end))
                .collect(),
        );
        let pyannote = merge_intervals(
            reference
                .segments
                .iter()
                .map(|segment| (segment.sample_start, segment.sample_end))
                .collect(),
        );

        let intersection = intervals_intersection(&ours, &pyannote);
        let union = intervals_total(&ours) + intervals_total(&pyannote) - intersection;
        let iou = if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        };

        assert!(iou >= 0.995, "timeline IoU below expectation: {iou:.6}");
        assert!(
            max_abs_start_delta <= 0.03,
            "max start delta too large: {max_abs_start_delta:.6}s"
        );
        assert!(
            max_abs_end_delta <= 0.10,
            "max end delta too large: {max_abs_end_delta:.6}s"
        );
    }

    #[test]
    fn aggregate_scores_rebases_chunk_timeline() {
        let chunks = vec![
            ChunkScores {
                chunk_start: 0,
                speech_scores: vec![1.0, 0.0],
            },
            ChunkScores {
                chunk_start: 13,
                speech_scores: vec![1.0, 0.0],
            },
        ];

        let aggregated = aggregate_scores(&chunks, 10, 5, 100, 10);
        assert!(aggregated[0] > 0.9);
        assert!(aggregated[1] > 0.4 && aggregated[1] < 0.6);
        assert!(aggregated[2] < 0.1);
    }

    #[test]
    fn scores_to_segments_keeps_half_threshold_state() {
        let scores = vec![0.6, 0.5, 0.5, 0.4];
        let segments = scores_to_segments(&scores, 10, 5, 0.5, 0.5);
        assert_eq!(segments, vec![(5, 35)]);
    }

    #[test]
    fn post_process_merges_short_gaps_and_removes_short_segments() {
        let segments = vec![(0, 100), (120, 220), (400, 430)];
        let processed = post_process_segments(segments, 60, 30);
        assert_eq!(processed, vec![(0, 220)]);
    }

    #[test]
    fn config_builder_updates_runtime_parameters() {
        let segmenter = Segmenter::new(16000)
            .unwrap()
            .with_step_seconds(2.0)
            .unwrap()
            .with_thresholds(0.7, 0.3)
            .unwrap()
            .with_min_durations(0.25, 0.1)
            .unwrap();

        let config = segmenter.config();
        assert_abs_diff_eq!(config.step_seconds, 2.0, epsilon = 1e-9);
        assert_abs_diff_eq!(config.onset, 0.7, epsilon = 1e-6);
        assert_abs_diff_eq!(config.offset, 0.3, epsilon = 1e-6);
        assert_abs_diff_eq!(config.min_duration_on_seconds, 0.25, epsilon = 1e-9);
        assert_abs_diff_eq!(config.min_duration_off_seconds, 0.1, epsilon = 1e-9);
    }

    #[test]
    fn config_validation_rejects_invalid_values() {
        let err = Segmenter::new(16000)
            .unwrap()
            .with_step_seconds(0.0)
            .unwrap_err();
        assert!(matches!(
            err,
            Error::InvalidConfiguration {
                field: "step_seconds",
                ..
            }
        ));

        let err = Segmenter::new(16000)
            .unwrap()
            .with_thresholds(1.2, 0.5)
            .unwrap_err();
        assert!(matches!(
            err,
            Error::InvalidConfiguration { field: "onset", .. }
        ));

        let err = Segmenter::new(16000)
            .unwrap()
            .with_min_durations(-0.1, 0.0)
            .unwrap_err();
        assert!(matches!(
            err,
            Error::InvalidConfiguration {
                field: "min_duration_on_seconds",
                ..
            }
        ));
    }
}
