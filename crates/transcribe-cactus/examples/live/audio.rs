use std::sync::Arc;

use futures_util::StreamExt;
use owhisper_interface::MixedMessage;

use hypr_audio::{AudioProvider, CaptureConfig};
use hypr_audio_utils::{chunk_size_for_stt, f32_to_i16_bytes};

use crate::{AudioSource, SAMPLE_RATE};

type SingleAudioStream = std::pin::Pin<
    Box<
        dyn futures_util::Stream<
                Item = MixedMessage<bytes::Bytes, owhisper_interface::ControlMessage>,
            > + Send,
    >,
>;

type DualAudioStream = std::pin::Pin<
    Box<
        dyn futures_util::Stream<
                Item = MixedMessage<
                    (bytes::Bytes, bytes::Bytes),
                    owhisper_interface::ControlMessage,
                >,
            > + Send,
    >,
>;

pub fn create_single_stream(
    audio: &Arc<dyn AudioProvider>,
    source: &AudioSource,
) -> SingleAudioStream {
    let chunk_size = chunk_size_for_stt(SAMPLE_RATE);
    match source {
        AudioSource::Input => {
            let capture = audio
                .open_mic_capture(None, SAMPLE_RATE, chunk_size)
                .expect("failed to open mic capture");
            Box::pin(capture.map(|result| {
                let frame = result.unwrap_or_else(|e| panic!("capture failed: {e}"));
                MixedMessage::Audio(f32_to_i16_bytes(frame.raw_mic.iter().copied()))
            }))
        }
        AudioSource::Output => {
            let capture = audio
                .open_speaker_capture(SAMPLE_RATE, chunk_size)
                .expect("failed to open speaker capture");
            Box::pin(capture.map(|result| {
                let frame = result.unwrap_or_else(|e| panic!("capture failed: {e}"));
                MixedMessage::Audio(f32_to_i16_bytes(frame.raw_speaker.iter().copied()))
            }))
        }
        AudioSource::RawDual | AudioSource::AecDual | AudioSource::Mock => {
            panic!("dual audio modes use create_dual_stream")
        }
    }
}

pub fn create_dual_stream(audio: &Arc<dyn AudioProvider>, source: &AudioSource) -> DualAudioStream {
    let chunk_size = chunk_size_for_stt(SAMPLE_RATE);
    let capture = audio
        .open_capture(CaptureConfig {
            sample_rate: SAMPLE_RATE,
            chunk_size,
            mic_device: None,
            enable_aec: source.uses_aec(),
        })
        .expect("failed to open capture");
    let source = source.clone();

    Box::pin(capture.map(move |result| {
        let frame = result.unwrap_or_else(|e| panic!("capture failed: {e}"));
        let (mic, speaker) = match source {
            AudioSource::RawDual | AudioSource::Mock => frame.raw_dual(),
            AudioSource::AecDual => frame.aec_dual(),
            _ => unreachable!(),
        };
        MixedMessage::Audio((
            f32_to_i16_bytes(mic.iter().copied()),
            f32_to_i16_bytes(speaker.iter().copied()),
        ))
    }))
}

pub fn print_info(audio: &dyn AudioProvider, source: &AudioSource) {
    let chunk_size = chunk_size_for_stt(SAMPLE_RATE);

    if source.is_dual() {
        eprintln!(
            "source: {} (input: {}, output: RealtimeSpeaker)",
            source,
            audio.default_device_name()
        );
        eprintln!(
            "sample rate: {} Hz, chunk size: {} samples, AEC: {}",
            SAMPLE_RATE,
            chunk_size,
            if source.uses_aec() {
                "enabled"
            } else {
                "disabled"
            }
        );
    } else {
        let device = match source {
            AudioSource::Output => "RealtimeSpeaker".to_string(),
            _ => audio.default_device_name(),
        };
        eprintln!("source: {} ({})", source, device);
        eprintln!(
            "sample rate: {} Hz, chunk size: {} samples",
            SAMPLE_RATE, chunk_size
        );
    }
    eprintln!("(set CACTUS_DEBUG=1 for raw engine output)");
    eprintln!();
}
