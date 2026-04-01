pub const BYTES_1: &[u8] = include_bytes!("../../data/models/model_1.onnx");
pub const BYTES_2: &[u8] = include_bytes!("../../data/models/model_2.onnx");

pub const BLOCK_SIZE: usize = 512;
pub const BLOCK_SHIFT: usize = 128;
pub const FFT_OUT_SIZE: usize = BLOCK_SIZE / 2 + 1; // 257
pub const STATE_SIZE: usize = 128;
