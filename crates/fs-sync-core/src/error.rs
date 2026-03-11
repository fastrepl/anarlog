use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unknown error")]
    Unknown,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Path error: {0}")]
    Path(String),
    #[error(transparent)]
    Frontmatter(#[from] hypr_frontmatter::Error),
    #[error("Markdown error: {0}")]
    Markdown(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AudioProcessingError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Decoder(#[from] rodio::decoder::DecoderError),
    #[error(transparent)]
    AudioUtils(#[from] hypr_audio_utils::Error),
    #[error("audio_import_unsupported_channel_count")]
    UnsupportedChannelCount { count: u16 },
    #[error("audio_import_invalid_channel_count")]
    InvalidChannelCount,
    #[error("audio_import_empty_input")]
    EmptyInput,
    #[error("audio_import_invalid_target_rate")]
    InvalidTargetSampleRate,
    #[error("audio_import_afconvert_failed: {0}")]
    #[allow(dead_code)]
    AfconvertFailed(String),
}

#[derive(Debug, thiserror::Error)]
pub enum AudioImportError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Processing(#[from] AudioProcessingError),
}
