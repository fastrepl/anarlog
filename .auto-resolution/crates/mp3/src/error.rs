use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unsupported channel count: {0} (expected 1 or 2)")]
    UnsupportedChannelCount(u16),

    #[error("unsupported float bit depth: {0}")]
    UnsupportedFloatBitDepth(u16),

    #[error("unsupported integer bit depth: {0}")]
    UnsupportedIntBitDepth(u16),

    #[error("failed to create LAME encoder")]
    LameInit,

    #[error("LAME configuration error: {0}")]
    LameConfig(String),

    #[error("LAME build error: {0}")]
    LameBuild(String),

    #[error("LAME encode error: {0}")]
    LameEncode(String),

    #[error("LAME flush error: {0}")]
    LameFlush(String),

    #[error(transparent)]
    Wav(#[from] hound::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    AudioUtils(#[from] hypr_audio_utils::Error),
}
