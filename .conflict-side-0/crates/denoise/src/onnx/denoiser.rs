use super::{buffer::CircularBuffer, context::ProcessingContext, error::Error, model};
use hypr_onnx::{
    ndarray::{Array3, Array4},
    ort::{session::Session, value::TensorRef},
};
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::sync::Arc;

pub struct Denoiser {
    session_1: Session,
    session_2: Session,
    block_len: usize,
    block_shift: usize,
    fft: Arc<dyn RealToComplex<f32>>,
    ifft: Arc<dyn ComplexToReal<f32>>,
    states_1: Array4<f32>,
    states_2: Array4<f32>,
    in_buffer: CircularBuffer,
    out_buffer: CircularBuffer,
}

impl Denoiser {
    pub fn new() -> Result<Self, Error> {
        let block_len = model::BLOCK_SIZE;
        let block_shift = model::BLOCK_SHIFT;

        let mut fft_planner = RealFftPlanner::<f32>::new();
        let fft = fft_planner.plan_fft_forward(block_len);
        let ifft = fft_planner.plan_fft_inverse(block_len);

        let session_1 = hypr_onnx::load_model_from_bytes(model::BYTES_1)?;
        let session_2 = hypr_onnx::load_model_from_bytes(model::BYTES_2)?;

        let state_size = model::STATE_SIZE;

        Ok(Denoiser {
            session_1,
            session_2,
            block_len,
            block_shift,
            fft,
            ifft,
            states_1: Array4::<f32>::zeros((1, 2, state_size, 2)),
            states_2: Array4::<f32>::zeros((1, 2, state_size, 2)),
            in_buffer: CircularBuffer::new(block_len, block_shift),
            out_buffer: CircularBuffer::new(block_len, block_shift),
        })
    }

    pub fn reset(&mut self) {
        let state_size = model::STATE_SIZE;
        self.states_1 = Array4::<f32>::zeros((1, 2, state_size, 2));
        self.states_2 = Array4::<f32>::zeros((1, 2, state_size, 2));
        self.in_buffer.clear();
        self.out_buffer.clear();
    }

    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>, Error> {
        self.reset();

        let len_audio = input.len();

        let padding = vec![0.0f32; self.block_len - self.block_shift];
        let mut audio = Vec::with_capacity(padding.len() * 2 + len_audio);
        audio.extend(&padding);
        audio.extend(input);
        audio.extend(&padding);

        let result = self._process_internal(&audio, true)?;

        let start_idx = self.block_len - self.block_shift;
        Ok(result[start_idx..start_idx + len_audio].to_vec())
    }

    pub fn process_streaming(&mut self, input: &[f32]) -> Result<Vec<f32>, Error> {
        if input.is_empty() {
            return Ok(vec![]);
        }

        self._process_internal(input, false)
    }

    fn _process_internal(&mut self, audio: &[f32], with_padding: bool) -> Result<Vec<f32>, Error> {
        let mut out_file = vec![0.0f32; audio.len()];

        let effective_len = if with_padding {
            audio.len() - (self.block_len - self.block_shift)
        } else {
            audio.len()
        };
        let num_blocks = effective_len / self.block_shift;

        let mut ctx = ProcessingContext::new(self.block_len, &self.fft, &self.ifft);

        for idx in 0..num_blocks {
            let start = idx * self.block_shift;
            let end = (start + self.block_shift).min(audio.len());

            self.in_buffer.push_chunk(&audio[start..end]);

            // FFT
            ctx.fft_buffer.copy_from_slice(self.in_buffer.data());
            self.fft.process_with_scratch(
                &mut ctx.fft_buffer,
                &mut ctx.fft_result,
                &mut ctx.scratch,
            )?;

            // Extract magnitude
            for (i, &c) in ctx.fft_result.iter().enumerate() {
                ctx.in_mag[[0, 0, i]] = c.norm();
            }

            // Model 1: magnitude + states → mask + new states
            let out_mask = self.run_model_1(&ctx.in_mag)?;

            // Apply mask to complex spectrum
            for (i, c) in ctx.fft_result.iter_mut().enumerate() {
                *c *= out_mask[[0, 0, i]];
            }

            // IFFT
            self.ifft.process_with_scratch(
                &mut ctx.fft_result,
                &mut ctx.estimated_block_vec,
                &mut ctx.ifft_scratch,
            )?;

            // Normalize
            let norm_factor = 1.0 / self.block_len as f32;
            ctx.estimated_block_vec
                .iter_mut()
                .for_each(|x| *x *= norm_factor);

            // Copy to Array3 for model 2
            for (i, &val) in ctx.estimated_block_vec.iter().enumerate() {
                ctx.estimated_block[[0, 0, i]] = val;
            }

            // Model 2: time-domain samples + states → refined samples + new states
            let out_block = self.run_model_2(&ctx.estimated_block)?;

            // Overlap-add
            let out_slice = out_block.as_slice().ok_or_else(|| {
                Error::ShapeError(hypr_onnx::ndarray::ShapeError::from_kind(
                    hypr_onnx::ndarray::ErrorKind::IncompatibleLayout,
                ))
            })?;
            self.out_buffer.shift_and_accumulate(out_slice);

            // Write to output
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

    fn run_model_1(&mut self, in_mag: &Array3<f32>) -> Result<Array3<f32>, Error> {
        let mut outputs = self.session_1.run(hypr_onnx::ort::inputs![
            "input_2" => TensorRef::from_array_view(in_mag.view())?,
            "input_3" => TensorRef::from_array_view(self.states_1.view())?
        ])?;

        let out_mask = outputs
            .remove("activation_2")
            .ok_or_else(|| Error::MissingOutput("activation_2".to_string()))?
            .try_extract_array::<f32>()?
            .view()
            .to_owned()
            .into_shape_with_order((1, 1, model::FFT_OUT_SIZE))?;

        self.states_1 = outputs
            .remove("tf_op_layer_stack_2")
            .ok_or_else(|| Error::MissingOutput("tf_op_layer_stack_2".to_string()))?
            .try_extract_array::<f32>()?
            .view()
            .to_owned()
            .into_shape_with_order((1, 2, model::STATE_SIZE, 2))?;

        Ok(out_mask)
    }

    fn run_model_2(&mut self, estimated_block: &Array3<f32>) -> Result<Array3<f32>, Error> {
        let mut outputs = self.session_2.run(hypr_onnx::ort::inputs![
            "input_4" => TensorRef::from_array_view(estimated_block.view())?,
            "input_5" => TensorRef::from_array_view(self.states_2.view())?
        ])?;

        let out_block = outputs
            .remove("conv1d_3")
            .ok_or_else(|| Error::MissingOutput("conv1d_3".into()))?
            .try_extract_array::<f32>()?
            .view()
            .to_owned()
            .into_shape_with_order((1, 1, model::BLOCK_SIZE))?;

        self.states_2 = outputs
            .remove("tf_op_layer_stack_5")
            .ok_or_else(|| Error::MissingOutput("tf_op_layer_stack_5".into()))?
            .try_extract_array::<f32>()?
            .view()
            .to_owned()
            .into_shape_with_order((1, 2, model::STATE_SIZE, 2))?;

        Ok(out_block)
    }

    fn normalize_output(&self, output: &mut [f32]) {
        let max_val = output.iter().fold(0.0f32, |max, &x| max.max(x.abs()));
        if max_val > 1.0 {
            let scale = 0.99 / max_val;
            output.iter_mut().for_each(|x| *x *= scale);
        }
    }
}
