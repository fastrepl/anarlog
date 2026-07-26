#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ResampleError(#[from] rubato::ResampleError),
    #[error(transparent)]
    ResamplerConstructionError(#[from] rubato::ResamplerConstructionError),
    #[error(transparent)]
    DecoderError(#[from] rodio::decoder::DecoderError),
    #[error(transparent)]
    Resampler(#[from] hypr_resampler::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Hound(#[from] hound::Error),
    #[error(transparent)]
    Vorbis(#[from] vorbis_rs::VorbisError),
    #[error("vorbis channel data length mismatch for channel {channel}")]
    ChannelDataLengthMismatch { channel: usize },
    #[error("unsupported channel count {count}")]
    UnsupportedChannelCount { count: u16 },
    #[error("invalid sample rate {0}")]
    InvalidSampleRate(u32),
    #[error("vorbis channel data is empty")]
    EmptyChannelSet,
    #[error("too many channels: {count}")]
    TooManyChannels { count: usize },
}
