use hypr_onnx::{
    ndarray::{self, Array2, Array3, Axis},
    ort::{self, session::Session, value::TensorRef},
};

use crate::{Error, Result};

pub mod model;

pub use model::{EMBEDDING_DIM, SAMPLE_RATE_HZ};

#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    pub scale_waveform_by_1_15: bool,
    pub mask_threshold: f32,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            // Matches pyannote.audio's ONNXWeSpeakerPretrainedSpeakerEmbedding implementation.
            scale_waveform_by_1_15: true,
            mask_threshold: 0.5,
        }
    }
}

pub struct EmbeddingExtractor {
    session: Session,
    config: EmbeddingConfig,
}

impl EmbeddingExtractor {
    pub fn new() -> Result<Self> {
        Self::from_model_bytes(model::BYTES)
    }

    pub fn from_model_bytes(model_bytes: &[u8]) -> Result<Self> {
        let session = hypr_onnx::load_model_from_bytes(model_bytes)?;
        Ok(Self {
            session,
            config: EmbeddingConfig::default(),
        })
    }

    pub fn with_config(mut self, config: EmbeddingConfig) -> Self {
        self.config = config;
        self
    }

    pub fn config(&self) -> &EmbeddingConfig {
        &self.config
    }

    pub fn compute(&mut self, samples_f32: &[f32]) -> Result<Vec<f32>> {
        self.compute_optional(samples_f32)?.ok_or(Error::TooShort)
    }

    pub fn compute_optional(&mut self, samples_f32: &[f32]) -> Result<Option<Vec<f32>>> {
        if samples_f32.is_empty() {
            return Err(Error::EmptyInput);
        }

        let Some(features) =
            compute_fbank_optional(samples_f32, self.config.scale_waveform_by_1_15)?
        else {
            return Ok(None);
        };

        self.run_features(features)
    }

    pub fn compute_with_mask_optional(
        &mut self,
        samples_f32: &[f32],
        mask: &[f32],
    ) -> Result<Option<Vec<f32>>> {
        if samples_f32.is_empty() {
            return Err(Error::EmptyInput);
        }

        if samples_f32.len() != mask.len() {
            return Err(Error::MaskLengthMismatch {
                mask_len: mask.len(),
                samples_len: samples_f32.len(),
            });
        }

        let Some(features) =
            compute_fbank_optional(samples_f32, self.config.scale_waveform_by_1_15)?
        else {
            return Ok(None);
        };
        let num_frames = features.nrows();
        if num_frames == 0 {
            return Ok(None);
        }

        let frame_mask = resample_mask_nearest(mask, num_frames, self.config.mask_threshold);
        let masked = select_rows(&features, &frame_mask)?;
        match masked {
            None => Ok(None),
            Some(masked) => self.run_features(masked),
        }
    }

    fn run_features(&mut self, features: Array2<f32>) -> Result<Option<Vec<f32>>> {
        let feats: Array3<f32> = features.insert_axis(Axis(0));

        let inputs = ort::inputs![model::INPUT_NAME => TensorRef::from_array_view(feats.view())?];
        let outputs = self.session.run(inputs)?;

        let out = outputs
            .get(model::OUTPUT_NAME)
            .ok_or_else(|| Error::MissingOutput(model::OUTPUT_NAME.to_string()))?
            .try_extract_array::<f32>()?;

        let embs = out.iter().copied().collect::<Vec<_>>();
        if embs.iter().all(|v| v.is_finite()) {
            Ok(Some(embs))
        } else {
            Ok(None)
        }
    }
}

fn compute_fbank_optional(
    samples_f32: &[f32],
    scale_waveform_by_1_15: bool,
) -> Result<Option<Array2<f32>>> {
    let mut scaled = Vec::with_capacity(samples_f32.len());
    if scale_waveform_by_1_15 {
        for &s in samples_f32 {
            scaled.push(s * 32768.0);
        }
    } else {
        scaled.extend_from_slice(samples_f32);
    }

    let features_knf = match knf_rs::compute_fbank(&scaled) {
        Ok(f) => f,
        Err(e) => {
            let msg = e.to_string();
            // kaldi-native-fbank returns zero frames when audio is too short
            if msg.contains("frames array is empty") {
                return Ok(None);
            }
            return Err(Error::KnfError(msg));
        }
    };

    let shape = features_knf.shape().to_vec();

    let features: Array2<f32> = ndarray::Array2::from_shape_vec(
        (shape[0], shape[1]),
        features_knf.iter().copied().collect(),
    )
    .map_err(|e| Error::KnfError(e.to_string()))?;

    Ok(Some(features))
}

fn resample_mask_nearest(mask: &[f32], target_len: usize, threshold: f32) -> Vec<bool> {
    if target_len == 0 {
        return vec![];
    }

    let src_len = mask.len();
    if src_len == 0 {
        return vec![false; target_len];
    }

    let mut out = Vec::with_capacity(target_len);
    for t in 0..target_len {
        let src_idx = (t * src_len) / target_len;
        out.push(mask[src_idx] > threshold);
    }
    out
}

fn select_rows(features: &Array2<f32>, keep: &[bool]) -> Result<Option<Array2<f32>>> {
    if keep.len() != features.nrows() {
        return Err(Error::Internal("mask length does not match feature frames"));
    }

    let bins = features.ncols();
    let mut out = Vec::new();
    let mut rows = 0usize;

    for (r, &k) in keep.iter().enumerate() {
        if !k {
            continue;
        }

        out.extend(features.row(r).iter().copied());
        rows += 1;
    }

    if rows == 0 {
        return Ok(None);
    }

    Ok(Some(Array2::from_shape_vec((rows, bins), out)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use serde::{Deserialize, Serialize};
    use std::path::Path;

    fn xorshift32(seed: &mut u32) -> u32 {
        let mut x = *seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        *seed = x;
        x
    }

    fn noise(len: usize) -> Vec<f32> {
        let mut seed = 0x1234_5678;
        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            let x = xorshift32(&mut seed);
            let f = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;
            v.push(f);
        }
        v
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct EmbeddingSnapshot {
        embedding_dim: usize,
        l2_norm: f64,
        mean: f64,
        std_dev: f64,
        head: Vec<f32>,
        tail: Vec<f32>,
    }

    fn pcm_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
            .collect()
    }

    fn save_snapshot(snapshot: &EmbeddingSnapshot, path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let json = serde_json::to_string_pretty(snapshot).unwrap();
        std::fs::write(path, json).unwrap();
    }

    fn load_snapshot(path: &Path) -> EmbeddingSnapshot {
        let json = std::fs::read_to_string(path).unwrap_or_else(|_| {
            panic!(
                "Snapshot file not found: {}
Run with UPDATE_SNAPSHOTS=1 to generate baselines.",
                path.display()
            )
        });
        serde_json::from_str(&json).unwrap()
    }

    fn assert_snapshot_eq(actual: &EmbeddingSnapshot, expected: &EmbeddingSnapshot) {
        assert_eq!(actual.embedding_dim, expected.embedding_dim);
        assert_eq!(actual.head.len(), expected.head.len());
        assert_eq!(actual.tail.len(), expected.tail.len());

        assert_abs_diff_eq!(actual.l2_norm, expected.l2_norm, epsilon = 1e-4);
        assert_abs_diff_eq!(actual.mean, expected.mean, epsilon = 1e-5);
        assert_abs_diff_eq!(actual.std_dev, expected.std_dev, epsilon = 1e-5);

        for (a, e) in actual.head.iter().zip(expected.head.iter()) {
            assert_abs_diff_eq!(*a as f64, *e as f64, epsilon = 5e-4);
        }

        for (a, e) in actual.tail.iter().zip(expected.tail.iter()) {
            assert_abs_diff_eq!(*a as f64, *e as f64, epsilon = 5e-4);
        }
    }

    fn run_snapshot_test(audio_bytes: &[u8], snapshot_name: &str) {
        let samples = pcm_bytes_to_f32(audio_bytes);
        let mut extractor = EmbeddingExtractor::new().unwrap();
        let embedding = extractor.compute(&samples).unwrap();

        let l2_norm = embedding
            .iter()
            .map(|&v| (v as f64) * (v as f64))
            .sum::<f64>()
            .sqrt();
        let mean = embedding.iter().map(|&v| v as f64).sum::<f64>() / embedding.len() as f64;
        let variance = embedding
            .iter()
            .map(|&v| {
                let d = v as f64 - mean;
                d * d
            })
            .sum::<f64>()
            / embedding.len() as f64;
        let std_dev = variance.sqrt();

        let k = embedding.len().min(12);
        let snapshot = EmbeddingSnapshot {
            embedding_dim: embedding.len(),
            l2_norm,
            mean,
            std_dev,
            head: embedding[..k].to_vec(),
            tail: embedding[embedding.len() - k..].to_vec(),
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
    fn embedding_has_expected_dim_and_is_finite() {
        let samples = noise(SAMPLE_RATE_HZ as usize);
        let mut extractor = EmbeddingExtractor::new().unwrap();
        let embs = extractor.compute_optional(&samples).unwrap().unwrap();
        assert_eq!(embs.len(), EMBEDDING_DIM);
        assert!(embs.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn masked_embedding_none_when_mask_is_all_zero() {
        let samples = noise(SAMPLE_RATE_HZ as usize);
        let mask = vec![0.0f32; samples.len()];
        let mut extractor = EmbeddingExtractor::new().unwrap();
        let embs = extractor
            .compute_with_mask_optional(&samples, &mask)
            .unwrap();
        assert!(embs.is_none());
    }

    #[test]
    fn snapshot_english_1() {
        run_snapshot_test(hypr_data::english_1::AUDIO, "embedding_english_1");
    }

    #[test]
    fn snapshot_english_2() {
        run_snapshot_test(hypr_data::english_2::AUDIO, "embedding_english_2");
    }

    #[test]
    fn snapshot_korean_1() {
        run_snapshot_test(hypr_data::korean_1::AUDIO, "embedding_korean_1");
    }
}
