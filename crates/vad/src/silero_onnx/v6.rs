use hypr_onnx::ndarray::{Array3, ArrayView1};
use hypr_onnx::ort::value::Tensor;

pub const CHUNK_SIZE_16KHZ: usize = 512;
const CONTEXT_SIZE_16KHZ: usize = 64;
const STATE_SIZE: usize = 128;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("onnx error: {0}")]
    Onnx(#[from] hypr_onnx::Error),
    #[error("ort error: {0}")]
    Ort(#[from] hypr_onnx::ort::Error),
}

pub struct SileroVad {
    session: hypr_onnx::ort::session::Session,
    state: Array3<f32>,
    context: Vec<f32>,
}

const MODEL_BYTES: &[u8] = include_bytes!("../../data/models/silero_v6.2.onnx");

impl Default for SileroVad {
    fn default() -> Self {
        Self::new_from_bytes(MODEL_BYTES).unwrap()
    }
}

impl SileroVad {
    pub fn new(model_path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        let session = hypr_onnx::load_model_from_path(model_path)?;
        Ok(Self {
            session,
            state: Array3::zeros((2, 1, STATE_SIZE)),
            context: vec![0.0; CONTEXT_SIZE_16KHZ],
        })
    }

    pub fn new_from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let session = hypr_onnx::load_model_from_bytes(bytes)?;
        Ok(Self {
            session,
            state: Array3::zeros((2, 1, STATE_SIZE)),
            context: vec![0.0; CONTEXT_SIZE_16KHZ],
        })
    }

    pub fn reset_states(&mut self) {
        self.state = Array3::zeros((2, 1, STATE_SIZE));
        self.context = vec![0.0; CONTEXT_SIZE_16KHZ];
    }

    pub fn process_chunk(&mut self, x: &ArrayView1<f32>, sr: u32) -> Result<f32, Error> {
        if sr != 16000 {
            return Err(Error::InvalidInput("sampling rate must be 16kHz".into()));
        }
        if x.len() != CHUNK_SIZE_16KHZ {
            return Err(Error::InvalidInput(format!(
                "input chunk must be {} samples, got {}",
                CHUNK_SIZE_16KHZ,
                x.len()
            )));
        }

        let mut input_data: Vec<f32> = Vec::with_capacity(CONTEXT_SIZE_16KHZ + CHUNK_SIZE_16KHZ);
        input_data.extend_from_slice(&self.context);
        input_data.extend_from_slice(x.as_slice().unwrap());
        self.context = input_data[CHUNK_SIZE_16KHZ..].to_vec();

        let state_shape = self.state.shape().to_vec();
        let (state_data, _) = self.state.clone().into_raw_vec_and_offset();

        let inputs = vec![
            (
                "input",
                Tensor::from_array((vec![1, input_data.len()], input_data))?.into_dyn(),
            ),
            (
                "state",
                Tensor::from_array((state_shape, state_data))?.into_dyn(),
            ),
            (
                "sr",
                Tensor::from_array((vec![1], vec![sr as i64]))?.into_dyn(),
            ),
        ];

        let outputs = self.session.run(inputs)?;

        let (_, out_data) = outputs[0].try_extract_tensor::<f32>()?;
        let prob = out_data.first().copied().unwrap_or(0.0);

        let (_, state_data) = outputs[1].try_extract_tensor::<f32>()?;
        self.state =
            Array3::from_shape_vec((2, 1, STATE_SIZE), state_data.iter().copied().collect())
                .map_err(|e| Error::InvalidInput(e.to_string()))?;

        Ok(prob)
    }
}

pub fn pcm_i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| s as f32 / 32768.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use hypr_onnx::ndarray::ArrayView1;
    use serde::{Deserialize, Serialize};
    use std::path::Path;

    #[derive(Debug, Serialize, Deserialize)]
    struct VadSnapshot {
        chunk_count: usize,
        probabilities: Vec<f32>,
        speech_ratio: f32,
        mean_probability: f32,
    }

    fn save_snapshot(snapshot: &VadSnapshot, path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let json = serde_json::to_string_pretty(snapshot).unwrap();
        std::fs::write(path, json).unwrap();
    }

    fn load_snapshot(path: &Path) -> VadSnapshot {
        let json = std::fs::read_to_string(path).unwrap_or_else(|_| {
            panic!(
                "Snapshot file not found: {}\nRun with UPDATE_SNAPSHOTS=1 to generate baselines.",
                path.display()
            )
        });
        serde_json::from_str(&json).unwrap()
    }

    fn assert_snapshot_eq(actual: &VadSnapshot, expected: &VadSnapshot) {
        assert_eq!(actual.chunk_count, expected.chunk_count);
        assert_abs_diff_eq!(actual.speech_ratio, expected.speech_ratio, epsilon = 0.01);
        assert_abs_diff_eq!(
            actual.mean_probability,
            expected.mean_probability,
            epsilon = 0.01
        );
        assert_eq!(actual.probabilities.len(), expected.probabilities.len());
        for (i, (a, e)) in actual
            .probabilities
            .iter()
            .zip(expected.probabilities.iter())
            .enumerate()
        {
            assert_abs_diff_eq!(a, e, epsilon = 0.02);
            if (a - e).abs() > 0.02 {
                panic!("probability mismatch at chunk {i}: actual={a}, expected={e}");
            }
        }
    }

    fn pcm_bytes_to_i16(bytes: &[u8]) -> Vec<i16> {
        bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect()
    }

    fn run_snapshot_test(audio_bytes: &[u8], snapshot_name: &str) {
        let mut model = SileroVad::default();

        let samples_f32: Vec<f32> = pcm_i16_to_f32(&pcm_bytes_to_i16(audio_bytes));

        let mut probabilities = Vec::new();
        for chunk in samples_f32.chunks(CHUNK_SIZE_16KHZ) {
            if chunk.len() == CHUNK_SIZE_16KHZ {
                let view = ArrayView1::from(chunk);
                probabilities.push(model.process_chunk(&view, 16000).unwrap());
            }
        }

        let chunk_count = probabilities.len();
        let speech_count = probabilities.iter().filter(|&&p| p > 0.5).count();
        let speech_ratio = speech_count as f32 / chunk_count as f32;
        let mean_probability = probabilities.iter().sum::<f32>() / chunk_count as f32;

        let actual = VadSnapshot {
            chunk_count,
            probabilities,
            speech_ratio,
            mean_probability,
        };

        let snapshot_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("data/snapshots/{snapshot_name}.json"));

        if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
            save_snapshot(&actual, &snapshot_path);
            println!("Updated snapshot: {}", snapshot_path.display());
        } else {
            let expected = load_snapshot(&snapshot_path);
            assert_snapshot_eq(&actual, &expected);
        }
    }

    #[test]
    fn test_silero_v6_english_1() {
        run_snapshot_test(hypr_data::english_1::AUDIO, "silero_v6_english_1");
    }

    #[test]
    fn test_silero_v6_english_2() {
        run_snapshot_test(hypr_data::english_2::AUDIO, "silero_v6_english_2");
    }

    #[test]
    fn test_silero_v6_english_3() {
        run_snapshot_test(hypr_data::english_3::AUDIO, "silero_v6_english_3");
    }

    #[test]
    fn test_silero_v6_korean_1() {
        run_snapshot_test(hypr_data::korean_1::AUDIO, "silero_v6_korean_1");
    }

    #[test]
    fn test_silero_v6_korean_2() {
        run_snapshot_test(hypr_data::korean_2::AUDIO, "silero_v6_korean_2");
    }
}
