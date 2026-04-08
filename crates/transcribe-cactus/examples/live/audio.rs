use std::sync::Arc;

use futures_util::StreamExt;
use owhisper_interface::MixedMessage;

use hypr_audio::{AudioProvider, CaptureConfig};
use hypr_audio_utils::{chunk_size_for_stt, f32_to_i16_bytes};
use transcribe_cactus::CactusConfig;

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

pub fn print_info(audio: &dyn AudioProvider, source: &AudioSource, cactus_config: &CactusConfig) {
    let capture_chunk_size = chunk_size_for_stt(SAMPLE_RATE);
    let capture_chunk_ms = capture_chunk_size as f64 * 1000.0 / SAMPLE_RATE as f64;
    let cactus_chunk_size =
        (SAMPLE_RATE as u64 * cactus_config.chunk_size_ms as u64 / 1000) as usize;
    let stream_chunk_sec = cactus_config.chunk_size_ms as f32 / 1000.0;
    let min_chunk_samples = (cactus_config.min_chunk_sec * SAMPLE_RATE as f32) as usize;

    eprintln!("source: {}", describe_source(audio, source));
    if source.is_dual() {
        eprintln!(
            "capture: {} Hz, {} samples ({capture_chunk_ms:.0} ms), AEC: {}",
            SAMPLE_RATE,
            capture_chunk_size,
            if source.uses_aec() {
                "enabled"
            } else {
                "disabled"
            }
        );
    } else {
        eprintln!(
            "capture: {} Hz, {} samples ({capture_chunk_ms:.0} ms)",
            SAMPLE_RATE, capture_chunk_size
        );
    }
    eprintln!(
        "cactus: stream chunk {:.1} s ({} ms, {} samples), min chunk {:.1} s ({} samples)",
        stream_chunk_sec,
        cactus_config.chunk_size_ms,
        cactus_chunk_size,
        cactus_config.min_chunk_sec,
        min_chunk_samples
    );
    eprintln!("(set CACTUS_DEBUG=1 for raw engine output)");
    eprintln!();
}

fn describe_source(audio: &dyn AudioProvider, source: &AudioSource) -> String {
    match source {
        AudioSource::Input => format!("input (mic: {})", audio.default_device_name()),
        AudioSource::Output => "output (speaker capture)".to_string(),
        AudioSource::RawDual => {
            format!(
                "raw-dual (mic: {}, speaker: system capture)",
                audio.default_device_name()
            )
        }
        AudioSource::AecDual => {
            format!(
                "aec-dual (mic: {}, speaker: system capture)",
                audio.default_device_name()
            )
        }
        AudioSource::Mock => "mock (mic: mock-mic, speaker: mock-speaker)".to_string(),
    }
}
