pub const BYTES: &[u8] = include_bytes!("../../data/models/segmentation.onnx");

pub const WINDOW_SECONDS: usize = 10;
pub const WINDOW_STEP_SECONDS: f64 = 1.0;
pub const FRAME_SIZE: usize = 270;
pub const FRAME_START: usize = 496;
pub const ONSET_THRESHOLD: f32 = 0.5;
pub const OFFSET_THRESHOLD: f32 = 0.5;
pub const MIN_DURATION_ON_SECONDS: f64 = 0.0;
pub const MIN_DURATION_OFF_SECONDS: f64 = 0.0;
