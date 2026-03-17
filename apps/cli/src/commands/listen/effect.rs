use super::audio_drop::AudioDropRequest;

pub(crate) enum Effect {
    StartBatch(AudioDropRequest),
    Exit { force: bool },
}
