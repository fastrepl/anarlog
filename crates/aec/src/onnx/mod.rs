use realfft::{ComplexToReal, RealFftPlanner, RealToComplex, num_complex::Complex};
use std::sync::Arc;

use hypr_onnx::{
    ndarray::{Array3, Array4},
    ort::{session::Session, value::TensorRef},
};

use super::CircularBuffer;

pub(crate) mod model;

struct ProcessingContext {
    scratch: Vec<Complex<f32>>,
    ifft_scratch: Vec<Complex<f32>>,
    in_buffer_fft: Vec<f32>,
    in_block_fft: Vec<Complex<f32>>,
    lpb_buffer_fft: Vec<f32>,
    lpb_block_fft: Vec<Complex<f32>>,
    estimated_block_vec: Vec<f32>,
    in_mag: Array3<f32>,
    lpb_mag: Array3<f32>,
    estimated_block: Array3<f32>,
    in_lpb: Array3<f32>,
    out_mask: Vec<f32>,
    out_block: Vec<f32>,
}

impl ProcessingContext {
    fn new(
        block_len: usize,
        fft: &Arc<dyn RealToComplex<f32>>,
        ifft: &Arc<dyn ComplexToReal<f32>>,
    ) -> Self {
        Self {
            scratch: vec![Complex::new(0.0f32, 0.0f32); fft.get_scratch_len()],
            ifft_scratch: vec![Complex::new(0.0f32, 0.0f32); ifft.get_scratch_len()],
            in_buffer_fft: vec![0.0f32; block_len],
            in_block_fft: vec![Complex::new(0.0f32, 0.0f32); block_len / 2 + 1],
            lpb_buffer_fft: vec![0.0f32; block_len],
            lpb_block_fft: vec![Complex::new(0.0f32, 0.0f32); block_len / 2 + 1],
            estimated_block_vec: vec![0.0f32; block_len],
            in_mag: Array3::<f32>::zeros((1, 1, block_len / 2 + 1)),
            lpb_mag: Array3::<f32>::zeros((1, 1, block_len / 2 + 1)),
            estimated_block: Array3::<f32>::zeros((1, 1, block_len)),
            in_lpb: Array3::<f32>::zeros((1, 1, block_len)),
            out_mask: vec![0.0f32; block_len / 2 + 1],
            out_block: vec![0.0f32; block_len],
        }
    }
}

pub struct AEC {
    session_1: Session,
    session_2: Session,
    block_len: usize,
    block_shift: usize,
    fft: Arc<dyn RealToComplex<f32>>,
    ifft: Arc<dyn ComplexToReal<f32>>,
    // streaming state
    states_1: Array4<f32>,
    states_2: Array4<f32>,
    in_buffer: CircularBuffer,
    in_buffer_lpb: CircularBuffer,
    out_buffer: CircularBuffer,
    is_first_chunk: bool,
}

impl AEC {
    pub fn new() -> Result<Self, crate::Error> {
        let (block_len, block_shift) = (model::BLOCK_SIZE, model::BLOCK_SHIFT);

        let mut fft_planner = RealFftPlanner::<f32>::new();
        let fft = fft_planner.plan_fft_forward(block_len);
        let ifft = fft_planner.plan_fft_inverse(block_len);

        let session_1 = hypr_onnx::load_model_from_bytes(model::BYTES_1)?;
        let session_2 = hypr_onnx::load_model_from_bytes(model::BYTES_2)?;

        let state_size = model::STATE_SIZE;

        Ok(AEC {
            session_1,
            session_2,
            block_len,
            block_shift,
            fft,
            ifft,
            states_1: Array4::<f32>::zeros((1, 2, state_size, 2)),
            states_2: Array4::<f32>::zeros((1, 2, state_size, 2)),
            in_buffer: CircularBuffer::new(block_len, block_shift),
            in_buffer_lpb: CircularBuffer::new(block_len, block_shift),
            out_buffer: CircularBuffer::new(block_len, block_shift),
            is_first_chunk: true,
        })
    }

    pub fn reset(&mut self) {
        let state_size = model::STATE_SIZE;
        self.states_1 = Array4::<f32>::zeros((1, 2, state_size, 2));
        self.states_2 = Array4::<f32>::zeros((1, 2, state_size, 2));
        self.in_buffer.clear();
        self.in_buffer_lpb.clear();
        self.out_buffer.clear();
        self.is_first_chunk = true;
    }

    fn calculate_fft_magnitude(
        &self,
        input: &[f32],
        fft_buffer: &mut [f32],
        fft_result: &mut [Complex<f32>],
        scratch: &mut [Complex<f32>],
        magnitude: &mut Array3<f32>,
    ) -> Result<(), crate::Error> {
        fft_buffer.copy_from_slice(input);
        self.fft
            .process_with_scratch(fft_buffer, fft_result, scratch)?;

        for (i, &c) in fft_result.iter().enumerate() {
            magnitude[[0, 0, i]] = c.norm();
        }

        Ok(())
    }

    fn run_model_1(&mut self, ctx: &mut ProcessingContext) -> Result<(), crate::Error> {
        let mut outputs = self.session_1.run(hypr_onnx::ort::inputs![
            TensorRef::from_array_view(ctx.in_mag.view())?,
            TensorRef::from_array_view(self.states_1.view())?,
            TensorRef::from_array_view(ctx.lpb_mag.view())?
        ])?;

        let out_mask = outputs
            .remove("Identity")
            .ok_or_else(|| crate::Error::MissingOutput("Identity".to_string()))?;
        let out_mask_view = out_mask.try_extract_array::<f32>()?;
        ctx.out_mask
            .copy_from_slice(out_mask_view.view().as_slice().ok_or_else(|| {
                crate::Error::ShapeError(hypr_onnx::ndarray::ShapeError::from_kind(
                    hypr_onnx::ndarray::ErrorKind::IncompatibleLayout,
                ))
            })?);

        let new_states = outputs
            .remove("Identity_1")
            .ok_or_else(|| crate::Error::MissingOutput("Identity_1".to_string()))?;
        let new_states_view = new_states.try_extract_array::<f32>()?;
        self.states_1
            .as_slice_mut()
            .ok_or_else(|| {
                crate::Error::ShapeError(hypr_onnx::ndarray::ShapeError::from_kind(
                    hypr_onnx::ndarray::ErrorKind::IncompatibleLayout,
                ))
            })?
            .copy_from_slice(new_states_view.view().as_slice().ok_or_else(|| {
                crate::Error::ShapeError(hypr_onnx::ndarray::ShapeError::from_kind(
                    hypr_onnx::ndarray::ErrorKind::IncompatibleLayout,
                ))
            })?);

        Ok(())
    }

    fn run_model_2(&mut self, ctx: &mut ProcessingContext) -> Result<(), crate::Error> {
        let mut outputs = self.session_2.run(hypr_onnx::ort::inputs![
            TensorRef::from_array_view(ctx.estimated_block.view())?,
            TensorRef::from_array_view(self.states_2.view())?,
            TensorRef::from_array_view(ctx.in_lpb.view())?
        ])?;

        let out_block = outputs
            .remove("Identity")
            .ok_or_else(|| crate::Error::MissingOutput("Identity".into()))?;
        let out_block_view = out_block.try_extract_array::<f32>()?;
        ctx.out_block
            .copy_from_slice(out_block_view.view().as_slice().ok_or_else(|| {
                crate::Error::ShapeError(hypr_onnx::ndarray::ShapeError::from_kind(
                    hypr_onnx::ndarray::ErrorKind::IncompatibleLayout,
                ))
            })?);

        let new_states = outputs
            .remove("Identity_1")
            .ok_or_else(|| crate::Error::MissingOutput("Identity_1".into()))?;
        let new_states_view = new_states.try_extract_array::<f32>()?;
        self.states_2
            .as_slice_mut()
            .ok_or_else(|| {
                crate::Error::ShapeError(hypr_onnx::ndarray::ShapeError::from_kind(
                    hypr_onnx::ndarray::ErrorKind::IncompatibleLayout,
                ))
            })?
            .copy_from_slice(new_states_view.view().as_slice().ok_or_else(|| {
                crate::Error::ShapeError(hypr_onnx::ndarray::ShapeError::from_kind(
                    hypr_onnx::ndarray::ErrorKind::IncompatibleLayout,
                ))
            })?);

        Ok(())
    }

    pub fn process_streaming(
        &mut self,
        mic_input: &[f32],
        lpb_input: &[f32],
    ) -> Result<Vec<f32>, crate::Error> {
        let len_audio = mic_input.len().min(lpb_input.len());
        let mic_input = &mic_input[..len_audio];
        let lpb_input = &lpb_input[..len_audio];

        // For streaming, we don't add padding to each chunk
        // Only process if we have enough samples
        if len_audio == 0 {
            return Ok(vec![]);
        }

        self._process_internal(mic_input, lpb_input, false)
    }

    pub fn process(
        &mut self,
        mic_input: &[f32],
        lpb_input: &[f32],
    ) -> Result<Vec<f32>, crate::Error> {
        self.reset();

        let len_audio = mic_input.len().min(lpb_input.len());
        let mic_input = &mic_input[..len_audio];
        let lpb_input = &lpb_input[..len_audio];

        // Add padding for non-streaming mode
        let padding = vec![0.0f32; self.block_len - self.block_shift];
        let mut audio = Vec::with_capacity(padding.len() * 2 + len_audio);
        audio.extend(&padding);
        audio.extend(mic_input);
        audio.extend(&padding);

        let mut lpb = Vec::with_capacity(padding.len() * 2 + len_audio);
        lpb.extend(&padding);
        lpb.extend(lpb_input);
        lpb.extend(&padding);

        let result = self._process_internal(&audio, &lpb, true)?;

        // Cut audio to original length
        let start_idx = self.block_len - self.block_shift;
        Ok(result[start_idx..start_idx + len_audio].to_vec())
    }

    fn _process_internal(
        &mut self,
        audio: &[f32],
        lpb: &[f32],
        with_padding: bool,
    ) -> Result<Vec<f32>, crate::Error> {
        let mut out_file = vec![0.0f32; audio.len()];

        // Calculate number of frames
        let effective_len = if with_padding {
            audio.len() - (self.block_len - self.block_shift)
        } else {
            // For streaming, we might not have a full final block
            audio.len()
        };
        let num_blocks = effective_len / self.block_shift;

        // Create processing context with all buffers
        let mut ctx = ProcessingContext::new(self.block_len, &self.fft, &self.ifft);

        for idx in 0..num_blocks {
            // Shift values and write to buffer of the input audio
            let start = idx * self.block_shift;
            let end = (start + self.block_shift).min(audio.len());
            let chunk_len = end - start;

            if chunk_len > 0 {
                self.in_buffer.push_chunk(&audio[start..end]);
                self.in_buffer_lpb.push_chunk(&lpb[start..end]);
            }

            // Calculate FFT of input block
            self.calculate_fft_magnitude(
                self.in_buffer.data(),
                &mut ctx.in_buffer_fft,
                &mut ctx.in_block_fft,
                &mut ctx.scratch,
                &mut ctx.in_mag,
            )?;

            // Calculate FFT of lpb block
            self.calculate_fft_magnitude(
                self.in_buffer_lpb.data(),
                &mut ctx.lpb_buffer_fft,
                &mut ctx.lpb_block_fft,
                &mut ctx.scratch,
                &mut ctx.lpb_mag,
            )?;

            self.run_model_1(&mut ctx)?;

            // Apply mask and calculate IFFT
            for (c, &m) in ctx.in_block_fft.iter_mut().zip(ctx.out_mask.iter()) {
                *c *= m;
            }

            // IFFT
            self.ifft.process_with_scratch(
                &mut ctx.in_block_fft,
                &mut ctx.estimated_block_vec,
                &mut ctx.ifft_scratch,
            )?;

            // Normalize IFFT result and copy to Array3 for second model (fused)
            let norm_factor = 1.0 / self.block_len as f32;
            let est_slice = ctx.estimated_block.as_slice_mut().unwrap();
            for (d, &s) in est_slice.iter_mut().zip(ctx.estimated_block_vec.iter()) {
                *d = s * norm_factor;
            }
            ctx.in_lpb
                .as_slice_mut()
                .unwrap()
                .copy_from_slice(self.in_buffer_lpb.data());

            self.run_model_2(&mut ctx)?;

            self.out_buffer.shift_and_accumulate(&ctx.out_block);

            // Write to output file
            let out_start = idx * self.block_shift;
            let out_end = (out_start + self.block_shift).min(out_file.len());
            let out_chunk_len = out_end - out_start;
            if out_chunk_len > 0 {
                out_file[out_start..out_end]
                    .copy_from_slice(&self.out_buffer.data()[..out_chunk_len]);
            }
        }

        self.normalize_output(&mut out_file);
        Ok(out_file)
    }

    fn normalize_output(&self, output: &mut [f32]) {
        let max_val = output.iter().fold(0.0f32, |max, &x| max.max(x.abs()));
        if max_val > 1.0 {
            let scale = 0.99 / max_val;
            output.iter_mut().for_each(|x| *x *= scale);
        }
    }
}

// cargo test -p aec --no-default-features --features onnx,128
// cargo test -p aec --no-default-features --features onnx,256
// cargo test -p aec --no-default-features --features onnx,512
// cargo bench -p aec --no-default-features --features onnx,128
//
// Set UPDATE_SNAPSHOTS=1 to regenerate baseline snapshots.
#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use dasp::sample::Sample;
    use hypr_audio_snapshot::{SpectralConfig, Tolerances};
    use model::{BLOCK_SHIFT, BLOCK_SIZE};
    use std::path::PathBuf;

    mod data {
        pub const DOUBLETALK_LPB: &[u8] =
            include_bytes!("../../data/inputs/doubletalk_lpb_sample.wav");
        pub const DOUBLETALK_MIC: &[u8] =
            include_bytes!("../../data/inputs/doubletalk_mic_sample.wav");

        pub const HYPRNOTE_LPB: &[u8] = include_bytes!("../../data/inputs/hyprnote_lpb.wav");
        pub const HYPRNOTE_MIC: &[u8] = include_bytes!("../../data/inputs/hyprnote_mic.wav");

        pub const THEO_LPB: &[u8] = include_bytes!("../../data/inputs/theo_lpb.wav");
        pub const THEO_MIC: &[u8] = include_bytes!("../../data/inputs/theo_mic.wav");
    }

    fn get_feature() -> &'static str {
        if cfg!(feature = "128") {
            "128"
        } else if cfg!(feature = "256") {
            "256"
        } else if cfg!(feature = "512") {
            "512"
        } else {
            unreachable!()
        }
    }

    fn output_path(prefix: &str, feature: &str, mode: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("outputs")
            .join(format!("{prefix}_{feature}_{mode}.wav"))
    }

    fn snapshot_path(prefix: &str, feature: &str, mode: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("snapshots")
            .join(format!("{prefix}_{feature}_{mode}.json"))
    }

    macro_rules! aec_test {
        ($test_name:ident, $lpb_data:expr, $mic_data:expr, $output_prefix:literal) => {
            #[test]
            fn $test_name() {
                let feature = get_feature();
                let config = SpectralConfig {
                    fft_size: BLOCK_SIZE,
                    hop_size: BLOCK_SHIFT,
                    sample_rate: 16000.0,
                };
                let tolerances = Tolerances::default();

                let lpb_sample =
                    rodio::Decoder::new(std::io::BufReader::new(std::io::Cursor::new($lpb_data)))
                        .unwrap()
                        .collect::<Vec<_>>();

                let mic_sample =
                    rodio::Decoder::new(std::io::BufReader::new(std::io::Cursor::new($mic_data)))
                        .unwrap()
                        .collect::<Vec<_>>();

                let lpb_samples: Vec<f32> = lpb_sample.into_iter().map(|s| s.to_sample()).collect();
                let mic_samples: Vec<f32> = mic_sample.into_iter().map(|s| s.to_sample()).collect();

                let batch_result = {
                    let mut aec = AEC::new().unwrap();
                    let result = aec.process(&mic_samples, &lpb_samples).unwrap();
                    assert!(result.iter().all(|&x| x.is_finite()));
                    result
                };

                let streaming_result = {
                    let mut aec = AEC::new().unwrap();
                    let mut streaming_result = Vec::new();

                    let len_audio = mic_samples.len().min(lpb_samples.len());
                    let mic_samples = &mic_samples[..len_audio];
                    let lpb_samples = &lpb_samples[..len_audio];

                    let chunk_size = model::BLOCK_SIZE * 2;
                    let mut processed = 0;

                    while processed < len_audio {
                        let end = (processed + chunk_size).min(len_audio);
                        let mic_chunk = &mic_samples[processed..end];
                        let lpb_chunk = &lpb_samples[processed..end];

                        let chunk_result = aec.process_streaming(mic_chunk, lpb_chunk).unwrap();
                        streaming_result.extend(chunk_result);

                        processed = end;
                    }

                    assert!(streaming_result.iter().all(|&x| x.is_finite()));
                    streaming_result
                };

                let batch_snap = hypr_audio_snapshot::assert_or_update(
                    &batch_result,
                    &output_path($output_prefix, feature, "batch"),
                    &snapshot_path($output_prefix, feature, "batch"),
                    &format!("{} {} batch", $output_prefix, feature),
                    &config,
                    &tolerances,
                );

                let streaming_snap = hypr_audio_snapshot::assert_or_update(
                    &streaming_result,
                    &output_path($output_prefix, feature, "streaming"),
                    &snapshot_path($output_prefix, feature, "streaming"),
                    &format!("{} {} streaming", $output_prefix, feature),
                    &config,
                    &tolerances,
                );

                assert_abs_diff_eq!(
                    batch_snap.rms_energy,
                    streaming_snap.rms_energy,
                    epsilon = 0.05,
                );
                assert_abs_diff_eq!(
                    batch_snap.spectral_centroid,
                    streaming_snap.spectral_centroid,
                    epsilon = 300.0,
                );
            }
        };
    }

    aec_test!(
        test_aec_doubletalk,
        data::DOUBLETALK_LPB,
        data::DOUBLETALK_MIC,
        "doubletalk"
    );

    aec_test!(
        test_aec_hyprnote,
        data::HYPRNOTE_LPB,
        data::HYPRNOTE_MIC,
        "hyprnote"
    );

    aec_test!(test_aec_theo, data::THEO_LPB, data::THEO_MIC, "theo");
}
