#[derive(Clone, Debug)]
pub struct CactusConfig {
    pub cloud: hypr_cactus::CloudConfig,
    pub chunk_size_ms: u32,
    pub min_chunk_sec: f32,
}

impl Default for CactusConfig {
    fn default() -> Self {
        Self {
            cloud: hypr_cactus::CloudConfig::default(),
            chunk_size_ms: 200,
            min_chunk_sec: 2.0,
        }
    }
}
