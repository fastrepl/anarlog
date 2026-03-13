mod frame;
mod joiner;
mod stream;

pub use frame::CaptureFrame;
pub use stream::CaptureStream;

use crate::Error;
use stream::{CaptureSide, setup_mic_stream, setup_speaker_stream};

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub sample_rate: u32,
    pub chunk_size: usize,
    pub mic_device: Option<String>,
    pub enable_aec: bool,
}

pub(crate) fn open_capture(config: CaptureConfig) -> Result<CaptureStream, Error> {
    let mic_stream = setup_mic_stream(config.sample_rate, config.chunk_size, config.mic_device)?;

    std::thread::sleep(std::time::Duration::from_millis(50));

    let speaker_stream = setup_speaker_stream(config.sample_rate, config.chunk_size)?;

    Ok(stream::open_dual(
        mic_stream,
        speaker_stream,
        config.enable_aec,
    ))
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
