#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Cactus(#[from] hypr_cactus::Error),

    #[error(transparent)]
    Audio(#[from] hypr_audio_utils::Error),

    #[error(transparent)]
    Vad(#[from] hypr_vad_chunking::Error),
}
