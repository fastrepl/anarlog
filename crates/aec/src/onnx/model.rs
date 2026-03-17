#[cfg(feature = "128")]
pub const BYTES_1: &[u8] = include_bytes!("../../data/models/model_128_1.onnx");
#[cfg(feature = "128")]
pub const BYTES_2: &[u8] = include_bytes!("../../data/models/model_128_2.onnx");
#[cfg(feature = "128")]
pub const STATE_SIZE: usize = 128;

#[cfg(feature = "256")]
pub const BYTES_1: &[u8] = include_bytes!("../../data/models/model_256_1.onnx");
#[cfg(feature = "256")]
pub const BYTES_2: &[u8] = include_bytes!("../../data/models/model_256_2.onnx");
#[cfg(feature = "256")]
pub const STATE_SIZE: usize = 256;

#[cfg(feature = "512")]
pub const BYTES_1: &[u8] = include_bytes!("../../data/models/model_512_1.onnx");
#[cfg(feature = "512")]
pub const BYTES_2: &[u8] = include_bytes!("../../data/models/model_512_2.onnx");
#[cfg(feature = "512")]
pub const STATE_SIZE: usize = 512;

// model already trained with these numbers.
pub const BLOCK_SIZE: usize = 512;
pub const BLOCK_SHIFT: usize = 128;
