mod batch;
mod streaming;
pub use streaming::*;

use std::path::Path;

use owhisper_interface::stream::{Extra, Metadata, ModelInfo};

pub(crate) fn build_metadata(model_path: &Path) -> Metadata {
    let model_name = model_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("cactus")
        .to_string();

    Metadata {
        model_info: ModelInfo {
            name: model_name,
            version: "1.0".to_string(),
            arch: "cactus".to_string(),
        },
        extra: Some(Extra::default().into()),
        ..Default::default()
    }
}
