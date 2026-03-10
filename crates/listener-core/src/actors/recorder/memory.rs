use ractor::ActorProcessingErr;

use super::RecorderEncoder;

pub(super) struct MemorySink {
    pub(super) encoder: RecorderEncoder,
    pub(super) data: Vec<u8>,
}

pub(super) fn create_memory_sink() -> Result<MemorySink, ActorProcessingErr> {
    let encoder = RecorderEncoder::Stereo(hypr_mp3::StereoStreamEncoder::new(
        super::super::SAMPLE_RATE,
    )?);

    Ok(MemorySink {
        encoder,
        data: Vec::new(),
    })
}
