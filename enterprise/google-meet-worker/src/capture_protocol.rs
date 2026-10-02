// Source-informed adaptation of Vexa v0.12.18 gmeet-capture.ts and pcm-capture.ts.
// Licensed under Apache-2.0; see ../THIRD_PARTY_NOTICES.md and ../third-party/VEXA-LICENSE.

use std::collections::HashMap;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;

const PROTOCOL_VERSION: u8 = 1;
const SAMPLE_RATE: u32 = 16_000;
const MAX_PAYLOAD_BYTES: usize = 16 * 1024;
const MAX_SAMPLES_PER_FRAME: usize = 4096;
const MAX_TRACK_INDEX: u32 = 4095;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFrame {
    pub sequence: u64,
    pub track_index: u32,
    pub sample_rate: u32,
    pub start_ms: u64,
    pub samples: Vec<i16>,
    pub speaker: Option<SpeakerHint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakerHint {
    pub display_name: Option<String>,
    pub participant_id: Option<String>,
}

#[derive(Debug, Default)]
pub struct CaptureProtocol {
    last_sequence: Option<u64>,
    track_ends_ms: HashMap<u32, u64>,
}

impl CaptureProtocol {
    pub fn decode(&mut self, payload: &str) -> Result<AudioFrame, CaptureProtocolError> {
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(CaptureProtocolError::PayloadTooLarge(payload.len()));
        }
        let wire: WireAudioFrame = serde_json::from_str(payload)?;
        if wire.version != PROTOCOL_VERSION {
            return Err(CaptureProtocolError::UnsupportedVersion(wire.version));
        }
        if wire.kind != "audio" {
            return Err(CaptureProtocolError::UnsupportedKind(wire.kind));
        }
        if wire.sample_rate != SAMPLE_RATE {
            return Err(CaptureProtocolError::UnsupportedSampleRate(
                wire.sample_rate,
            ));
        }
        if wire.track_index > MAX_TRACK_INDEX {
            return Err(CaptureProtocolError::TrackIndexOutOfRange(wire.track_index));
        }
        let expected_sequence = match self.last_sequence {
            Some(sequence) => sequence
                .checked_add(1)
                .ok_or(CaptureProtocolError::SequenceExhausted)?,
            None => 1,
        };
        if wire.sequence != expected_sequence {
            return Err(CaptureProtocolError::UnexpectedSequence {
                expected: expected_sequence,
                actual: wire.sequence,
            });
        }

        let bytes = STANDARD
            .decode(wire.pcm_s16le)
            .map_err(CaptureProtocolError::InvalidPcmEncoding)?;
        if bytes.is_empty() || bytes.len() % 2 != 0 {
            return Err(CaptureProtocolError::InvalidPcmLength(bytes.len()));
        }
        let sample_count = bytes.len() / 2;
        if sample_count > MAX_SAMPLES_PER_FRAME {
            return Err(CaptureProtocolError::TooManySamples(sample_count));
        }
        let duration_ms = (sample_count as u64 * 1000).div_ceil(SAMPLE_RATE as u64);
        if let Some(previous_end_ms) = self.track_ends_ms.get(&wire.track_index)
            && wire.start_ms < *previous_end_ms
        {
            return Err(CaptureProtocolError::OverlappingTrackFrame {
                track_index: wire.track_index,
                previous_end_ms: *previous_end_ms,
                start_ms: wire.start_ms,
            });
        }

        let samples = bytes
            .chunks_exact(2)
            .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
            .collect();
        let speaker = SpeakerHint::from_wire(wire.speaker_name, wire.participant_id);
        let end_ms = wire
            .start_ms
            .checked_add(duration_ms)
            .ok_or(CaptureProtocolError::TimestampOverflow)?;
        self.last_sequence = Some(wire.sequence);
        self.track_ends_ms.insert(wire.track_index, end_ms);
        Ok(AudioFrame {
            sequence: wire.sequence,
            track_index: wire.track_index,
            sample_rate: wire.sample_rate,
            start_ms: wire.start_ms,
            samples,
            speaker,
        })
    }
}

impl SpeakerHint {
    fn from_wire(display_name: Option<String>, participant_id: Option<String>) -> Option<Self> {
        let display_name = display_name.and_then(|value| sanitize_hint(value, 100));
        let participant_id = participant_id.and_then(|value| sanitize_hint(value, 256));
        if display_name.is_none() && participant_id.is_none() {
            None
        } else {
            Some(Self {
                display_name,
                participant_id,
            })
        }
    }
}

fn sanitize_hint(value: String, max_length: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > max_length
        || trimmed.chars().any(char::is_control)
    {
        return None;
    }
    Some(trimmed.to_owned())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAudioFrame {
    #[serde(rename = "v")]
    version: u8,
    kind: String,
    sequence: u64,
    track_index: u32,
    sample_rate: u32,
    start_ms: u64,
    pcm_s16le: String,
    #[serde(default)]
    speaker_name: Option<String>,
    #[serde(default)]
    participant_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureProtocolError {
    #[error("capture payload is too large: {0} bytes")]
    PayloadTooLarge(usize),
    #[error("invalid capture payload")]
    Json(#[from] serde_json::Error),
    #[error("unsupported capture protocol version: {0}")]
    UnsupportedVersion(u8),
    #[error("unsupported capture payload kind: {0}")]
    UnsupportedKind(String),
    #[error("unsupported capture sample rate: {0}")]
    UnsupportedSampleRate(u32),
    #[error("capture track index is out of range: {0}")]
    TrackIndexOutOfRange(u32),
    #[error("unexpected capture sequence: expected {expected}, got {actual}")]
    UnexpectedSequence { expected: u64, actual: u64 },
    #[error("capture sequence is exhausted")]
    SequenceExhausted,
    #[error("capture PCM is not valid base64")]
    InvalidPcmEncoding(#[source] base64::DecodeError),
    #[error("capture PCM has invalid byte length: {0}")]
    InvalidPcmLength(usize),
    #[error("capture PCM contains too many samples: {0}")]
    TooManySamples(usize),
    #[error("capture timestamp overflowed")]
    TimestampOverflow,
    #[error(
        "capture frame overlaps track {track_index}: previous end {previous_end_ms}ms, start {start_ms}ms"
    )]
    OverlappingTrackFrame {
        track_index: u32,
        previous_end_ms: u64,
        start_ms: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn pcm(samples: &[i16]) -> String {
        let bytes: Vec<_> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        STANDARD.encode(bytes)
    }

    fn frame(
        sequence: u64,
        track_index: u32,
        start_ms: u64,
        pcm_s16le: &str,
        extra: Value,
    ) -> String {
        let mut frame = json!({
            "v": 1,
            "kind": "audio",
            "sequence": sequence,
            "track_index": track_index,
            "sample_rate": 16000,
            "start_ms": start_ms,
            "pcm_s16le": pcm_s16le,
        });
        if let (Some(frame), Some(extra)) = (frame.as_object_mut(), extra.as_object()) {
            frame.extend(extra.clone());
        }
        frame.to_string()
    }

    fn payload(sequence: u64, track_index: u32, start_ms: u64, samples: &[i16]) -> String {
        frame(sequence, track_index, start_ms, &pcm(samples), json!({}))
    }

    #[test]
    fn decodes_little_endian_pcm_and_tracks_sequence() {
        let mut protocol = CaptureProtocol::default();

        let frame = protocol
            .decode(&payload(1, 3, 0, &[-32768, 0, 32767]))
            .unwrap();

        assert_eq!(frame.sequence, 1);
        assert_eq!(frame.track_index, 3);
        assert_eq!(frame.samples, vec![-32768, 0, 32767]);
    }

    #[test]
    fn rejects_malformed_frames_without_advancing_sequence() {
        type Case = (&'static str, String, fn(&CaptureProtocolError) -> bool);
        let cases: [Case; 4] = [
            ("sequence gap", payload(2, 0, 0, &[1]), |error| {
                matches!(
                    error,
                    CaptureProtocolError::UnexpectedSequence {
                        expected: 1,
                        actual: 2
                    }
                )
            }),
            (
                "invalid base64",
                frame(1, 0, 0, "not base64", json!({})),
                |error| matches!(error, CaptureProtocolError::InvalidPcmEncoding(_)),
            ),
            (
                "odd pcm length",
                frame(1, 0, 0, &STANDARD.encode([1]), json!({})),
                |error| matches!(error, CaptureProtocolError::InvalidPcmLength(1)),
            ),
            (
                "timestamp overflow",
                payload(1, 0, u64::MAX, &[1]),
                |error| matches!(error, CaptureProtocolError::TimestampOverflow),
            ),
        ];

        for (name, invalid, expected) in cases {
            let mut protocol = CaptureProtocol::default();
            let error = protocol.decode(&invalid).unwrap_err();
            assert!(expected(&error), "{name}: {error:?}");
            assert!(protocol.decode(&payload(1, 0, 0, &[1])).is_ok(), "{name}");
        }
    }

    #[test]
    fn rejects_overlapping_frames_per_track() {
        let mut protocol = CaptureProtocol::default();
        protocol.decode(&payload(1, 4, 100, &[1; 160])).unwrap();

        assert!(matches!(
            protocol.decode(&payload(2, 4, 109, &[2])),
            Err(CaptureProtocolError::OverlappingTrackFrame {
                track_index: 4,
                previous_end_ms: 110,
                start_ms: 109
            })
        ));
    }

    #[test]
    fn carries_bounded_speaker_attribution_and_drops_invalid_fields() {
        for (speaker_name, expected_display_name) in [
            ("  Ada Lovelace  ", Some("Ada Lovelace")),
            ("Ada\nLovelace", None),
        ] {
            let mut protocol = CaptureProtocol::default();
            let attributed = frame(
                1,
                0,
                0,
                &pcm(&[1]),
                json!({
                    "speaker_name": speaker_name,
                    "participant_id": "spaces/abc/devices/123",
                }),
            );

            assert_eq!(
                protocol.decode(&attributed).unwrap().speaker,
                Some(SpeakerHint {
                    display_name: expected_display_name.map(Into::into),
                    participant_id: Some("spaces/abc/devices/123".into()),
                })
            );
            assert!(protocol.decode(&payload(2, 1, 0, &[2])).is_ok());
        }
    }
}
