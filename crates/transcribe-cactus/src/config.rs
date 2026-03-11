#[derive(Clone, Debug)]
pub struct CactusConfig {
    pub cloud: hypr_cactus::CloudConfig,
    pub min_chunk_sec: f32,
}

impl Default for CactusConfig {
    fn default() -> Self {
        Self {
            cloud: hypr_cactus::CloudConfig::default(),
            min_chunk_sec: 2.0,
        }
    }
}
