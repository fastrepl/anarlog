use std::sync::Arc;

use futures_util::StreamExt;
use owhisper_interface::MixedMessage;

use hypr_audio::{CaptureConfig, CaptureFrame};
use hypr_audio_utils::{chunk_size_for_stt, f32_to_i16_bytes};

pub use crate::cli::AudioSource;
pub use hypr_audio::AudioProvider;
pub use hypr_audio_actual::ActualAudio;

use crate::error::{CliError, CliResult};

pub const DEFAULT_SAMPLE_RATE: u32 = 16_000;
pub const DEFAULT_TIMEOUT_SECS: u64 = 600;

#[derive(Clone, Copy)]
pub enum ChannelKind {
    Mic,
    Speaker,
}

#[derive(Clone, Copy)]
pub enum DisplayMode {
    Single(ChannelKind),
    Dual,
}

impl AudioSource {
    pub fn is_dual(&self) -> bool {
        matches!(self, Self::RawDual | Self::AecDual)
    }

    fn uses_aec(&self) -> bool {
        matches!(self, Self::AecDual)
    }

    pub fn is_mock(&self) -> bool {
        matches!(self, Self::Mock)
    }
}

pub fn create_single_audio_stream(
    audio: &Arc<dyn AudioProvider>,
    source: &AudioSource,
    sample_rate: u32,
) -> CliResult<
    std::pin::Pin<
        Box<
            dyn futures_util::Stream<
                    Item = MixedMessage<bytes::Bytes, owhisper_interface::ControlMessage>,
                > + Send,
        >,
    >,
> {
    let chunk_size = chunk_size_for_stt(sample_rate);
    let use_mic = match source {
        AudioSource::Input => true,
        AudioSource::Output => false,
        AudioSource::Mock => true,
        AudioSource::RawDual | AudioSource::AecDual => {
            return Err(CliError::operation_failed(
                "create single audio stream",
                "dual audio modes use create_dual_audio_stream",
            ));
        }
    };

    if use_mic {
        let capture = audio
            .open_mic_capture(None, sample_rate, chunk_size)
            .map_err(|e| CliError::operation_failed("open mic capture", e.to_string()))?;
        Ok(Box::pin(capture.filter_map(|result| async move {
            match result {
                Ok(frame) => Some(MixedMessage::Audio(f32_to_i16_bytes(
                    frame.raw_mic.iter().copied(),
                ))),
                Err(error) => {
                    eprintln!("capture failed: {error}");
                    None
                }
            }
        })))
    } else {
        let capture = audio
            .open_speaker_capture(sample_rate, chunk_size)
            .map_err(|e| CliError::operation_failed("open speaker capture", e.to_string()))?;
        Ok(Box::pin(capture.filter_map(|result| async move {
            match result {
                Ok(frame) => Some(MixedMessage::Audio(f32_to_i16_bytes(
                    frame.raw_speaker.iter().copied(),
                ))),
                Err(error) => {
                    eprintln!("capture failed: {error}");
                    None
                }
            }
        })))
    }
}

pub fn create_dual_audio_stream(
    audio: &Arc<dyn AudioProvider>,
    source: &AudioSource,
    sample_rate: u32,
) -> CliResult<
    std::pin::Pin<
        Box<
            dyn futures_util::Stream<
                    Item = MixedMessage<
                        (bytes::Bytes, bytes::Bytes),
                        owhisper_interface::ControlMessage,
                    >,
                > + Send,
        >,
    >,
> {
    let chunk_size = chunk_size_for_stt(sample_rate);
    let capture_stream = audio
        .open_capture(CaptureConfig {
            sample_rate,
            chunk_size,
            mic_device: None,
            enable_aec: source.uses_aec(),
        })
        .map_err(|e| CliError::operation_failed("open realtime capture", e.to_string()))?;
    let source = source.clone();

    Ok(Box::pin(capture_stream.filter_map(move |result| {
        let source = source.clone();
        async move {
            match result {
                Ok(frame) => Some(MixedMessage::Audio(capture_frame_to_bytes(&source, frame))),
                Err(error) => {
                    eprintln!("capture failed: {error}");
                    None
                }
            }
        }
    })))
}

pub fn capture_frame_to_bytes(
    source: &AudioSource,
    frame: CaptureFrame,
) -> (bytes::Bytes, bytes::Bytes) {
    let (mic, speaker) = match source {
        AudioSource::RawDual => frame.raw_dual(),
        AudioSource::AecDual => frame.aec_dual(),
        AudioSource::Input | AudioSource::Output => unreachable!(),
        AudioSource::Mock => unreachable!(),
    };

    (
        f32_to_i16_bytes(mic.iter().copied()),
        f32_to_i16_bytes(speaker.iter().copied()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_source_reports_dual_modes() {
        assert!(!AudioSource::Input.is_dual());
        assert!(!AudioSource::Output.is_dual());
        assert!(AudioSource::RawDual.is_dual());
        assert!(AudioSource::AecDual.is_dual());
    }

    #[test]
    fn capture_frame_to_bytes_preserves_channel_order() {
        let frame = CaptureFrame {
            raw_mic: std::sync::Arc::from([0.25_f32, -0.25]),
            raw_speaker: std::sync::Arc::from([0.75_f32, -0.75]),
            aec_mic: Some(std::sync::Arc::from([0.1_f32, -0.1])),
        };

        let (raw_mic, raw_speaker) = capture_frame_to_bytes(&AudioSource::RawDual, frame.clone());
        assert_eq!(&raw_mic[..], &[0x00, 0x20, 0x00, 0xe0]);
        assert_eq!(&raw_speaker[..], &[0x00, 0x60, 0x00, 0xa0]);

        let (aec_mic, aec_speaker) = capture_frame_to_bytes(&AudioSource::AecDual, frame);
        assert_eq!(&aec_mic[..], &[0xcc, 0x0c, 0x34, 0xf3]);
        assert_eq!(&aec_speaker[..], &[0x00, 0x60, 0x00, 0xa0]);
    }
}
