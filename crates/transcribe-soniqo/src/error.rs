use crate::model::SoniqoModel;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported Soniqo model: {0}")]
    UnsupportedModel(String),
    #[error("Soniqo is only available on macOS Apple Silicon")]
    UnsupportedPlatform,
    #[error("{} requires macOS 15 or newer.", .0.display_name())]
    RequiresMacOs15(SoniqoModel),
    #[error("Soniqo bridge failed: {0}")]
    Bridge(String),
    #[error("failed to parse Soniqo bridge response: {0}")]
    ResponseParse(#[from] serde_json::Error),
    #[error("failed to delete Soniqo model: {0}")]
    Delete(std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
