mod joiner;
mod recovery;
mod stream;

use anlg_audio::{CaptureConfig, CaptureStream, Error};
use stream::{CaptureSide, setup_mic_stream, setup_speaker_stream};

pub(crate) fn open_capture(config: CaptureConfig) -> Result<CaptureStream, Error> {
    Ok(stream::open_recovering(config))
}

pub(crate) fn open_speaker_capture(
    sample_rate: u32,
    chunk_size: usize,
) -> Result<CaptureStream, Error> {
    let speaker_stream = setup_speaker_stream(sample_rate, chunk_size)?;
    Ok(stream::open_single(speaker_stream, CaptureSide::Speaker))
}

pub(crate) fn open_mic_capture(
    device: Option<String>,
    sample_rate: u32,
    chunk_size: usize,
) -> Result<CaptureStream, Error> {
    let mic_stream = setup_mic_stream(sample_rate, chunk_size, device)?;
    Ok(stream::open_single(mic_stream, CaptureSide::Mic))
}
