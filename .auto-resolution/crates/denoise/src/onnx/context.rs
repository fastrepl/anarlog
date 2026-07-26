use hypr_onnx::ndarray::Array3;
use realfft::{ComplexToReal, RealToComplex, num_complex::Complex};
use std::sync::Arc;

pub(super) struct ProcessingContext {
    pub scratch: Vec<Complex<f32>>,
    pub ifft_scratch: Vec<Complex<f32>>,
    pub fft_buffer: Vec<f32>,
    pub fft_result: Vec<Complex<f32>>,
    pub estimated_block_vec: Vec<f32>,
    pub in_mag: Array3<f32>,
    pub estimated_block: Array3<f32>,
}

impl ProcessingContext {
    pub fn new(
        block_len: usize,
        fft: &Arc<dyn RealToComplex<f32>>,
        ifft: &Arc<dyn ComplexToReal<f32>>,
    ) -> Self {
        Self {
            scratch: vec![Complex::new(0.0f32, 0.0f32); fft.get_scratch_len()],
            ifft_scratch: vec![Complex::new(0.0f32, 0.0f32); ifft.get_scratch_len()],
            fft_buffer: vec![0.0f32; block_len],
            fft_result: vec![Complex::new(0.0f32, 0.0f32); block_len / 2 + 1],
            estimated_block_vec: vec![0.0f32; block_len],
            in_mag: Array3::<f32>::zeros((1, 1, block_len / 2 + 1)),
            estimated_block: Array3::<f32>::zeros((1, 1, block_len)),
        }
    }
}
