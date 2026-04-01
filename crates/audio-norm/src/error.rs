#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Decoder(#[from] rodio::decoder::DecoderError),
    #[error(transparent)]
    AudioUtils(#[from] hypr_audio_utils::Error),
    #[error(transparent)]
    Resampler(#[from] hypr_resampler::Error),
    #[error("audio_import_unsupported_channel_count")]
    UnsupportedChannelCount { count: u16 },
    #[error("audio_import_invalid_channel_count")]
    InvalidChannelCount,
    #[error("audio_import_empty_input")]
    EmptyInput,
    #[error("audio_import_mp3_encode: {0}")]
    Mp3Encode(String),
    #[error("audio_import_afconvert_failed: {0}")]
    AfconvertFailed(String),
}
