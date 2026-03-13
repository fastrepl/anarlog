#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("no input device found")]
    NoInputDevice,
    #[error("mic_open_failed")]
    MicOpenFailed,
    #[error("mic_stream_setup_failed")]
    MicStreamSetupFailed,
    #[error("speaker_stream_setup_failed")]
    SpeakerStreamSetupFailed,
    #[error("mic_resample_failed")]
    MicResampleFailed,
    #[error("speaker_resample_failed")]
    SpeakerResampleFailed,
    #[error("mic_stream_ended")]
    MicStreamEnded,
    #[error("speaker_stream_ended")]
    SpeakerStreamEnded,
}
