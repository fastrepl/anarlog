pub const BYTES: &[u8] = include_bytes!("./embedding.onnx");

pub const INPUT_NAME: &str = "feats";
pub const OUTPUT_NAME: &str = "embs";

pub const SAMPLE_RATE_HZ: u32 = 16000;
pub const NUM_MEL_BINS: usize = 80;
pub const EMBEDDING_DIM: usize = 256;
