#[cfg(test)]
use std::path::Path;

pub(super) use hypr_transcribe_core::{
    WsSender, format_timestamp_now, send_ws, send_ws_best_effort,
};
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse, Word};

#[derive(Debug, Clone, Copy)]
pub(super) enum TranscriptKind {
    Confirmed,
    Pending,
    Finalized,
}

#[cfg(test)]
pub(super) fn build_session_metadata(model_path: &Path) -> Metadata {
    crate::service::build_metadata(model_path)
}

pub(super) fn build_transcript_response(
    segment: &crate::service::Segment<'_>,
    language: Option<&str>,
    kind: TranscriptKind,
    metadata: &Metadata,
    channel_index: &[i32],
    extra_keys: Option<std::collections::HashMap<String, serde_json::Value>>,
) -> StreamResponse {
    let (is_final, speech_final, from_finalize) = match kind {
        TranscriptKind::Confirmed => (true, true, false),
        TranscriptKind::Pending => (false, false, false),
        TranscriptKind::Finalized => (true, true, true),
    };
    let languages = language.map(|l| vec![l.to_string()]).unwrap_or_default();

    let word_strs: Vec<&str> = segment
        .text
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .collect();
    let n = word_strs.len();
    let words: Vec<Word> = word_strs
        .into_iter()
        .enumerate()
        .map(|(i, w)| {
            let word_start = segment.start + (i as f64 / n as f64) * segment.duration;
            let word_end = if i + 1 == n {
                // Ensure the last word ends >50ms before the segment boundary so
                // the stitch heuristic in crates/transcript doesn't merge it with
                // the first word of the next segment (gap <= STITCH_MAX_GAP_MS=50ms).
                (segment.start + segment.duration - 0.1_f64).max(word_start + 0.05_f64)
            } else {
                segment.start + ((i + 1) as f64 / n as f64) * segment.duration
            };
            Word {
                word: w.to_string(),
                start: word_start,
                end: word_end,
                confidence: segment.confidence,
                speaker: None,
                punctuated_word: None,
                language: None,
            }
        })
        .collect();

    let mut meta = metadata.clone();
    if let Some(keys) = extra_keys {
        match &mut meta.extra {
            Some(existing) => existing.extend(keys),
            slot => *slot = Some(keys),
        }
    }

    StreamResponse::TranscriptResponse {
        start: segment.start,
        duration: segment.duration,
        is_final,
        speech_final,
        from_finalize,
        channel: Channel {
            alternatives: vec![Alternatives {
                transcript: segment.text.to_string(),
                languages,
                words,
                confidence: segment.confidence,
            }],
        },
        metadata: meta,
        channel_index: channel_index.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use owhisper_interface::stream::StreamResponse;

    use crate::service::Segment;

    use super::*;

    #[test]
    fn session_metadata_has_required_fields() {
        let meta = build_session_metadata(Path::new("/some/path/whisper-large-v3"));
        assert!(!meta.request_id.is_empty());
        assert!(!meta.model_uuid.is_empty());
        assert_eq!(meta.model_info.name, "whisper-large-v3");
        assert_eq!(meta.model_info.arch, "cactus");
        assert!(meta.extra.is_some());
    }

    #[test]
    fn format_timestamp_produces_iso8601() {
        let ts = format_timestamp_now();
        assert!(ts.ends_with('Z'));
        assert!(ts.contains('T'));
        assert_eq!(ts.len(), 24);
    }

    #[test]
    fn transcript_response_serializes_as_results() {
        let meta = build_session_metadata(Path::new("/models/whisper-small"));
        let resp = build_transcript_response(
            &Segment {
                text: "hello world",
                start: 0.0,
                duration: 1.5,
                confidence: 0.95,
            },
            Some("en"),
            TranscriptKind::Confirmed,
            &meta,
            &[0, 1],
            None,
        );

        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(v["type"], "Results");
        assert_eq!(v["is_final"], true);
        assert_eq!(v["speech_final"], true);
        assert_eq!(v["from_finalize"], false);
        assert_eq!(v["start"], 0.0);
        assert_eq!(v["duration"], 1.5);
        assert_eq!(v["channel"]["alternatives"][0]["transcript"], "hello world");
        assert_eq!(
            v["channel"]["alternatives"][0]["words"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(v["channel"]["alternatives"][0]["languages"][0], "en");
        assert!(!v["metadata"]["request_id"].as_str().unwrap().is_empty());
        assert_eq!(v["metadata"]["model_info"]["name"], "whisper-small");
        assert_eq!(v["metadata"]["model_info"]["arch"], "cactus");
        assert!(
            v["metadata"]["extra"]["started_unix_millis"]
                .as_u64()
                .is_some()
        );
        assert_eq!(v["channel_index"], serde_json::json!([0, 1]));
    }

    #[test]
    fn transcript_response_from_finalize_flag() {
        let meta = build_session_metadata(Path::new("/models/test"));
        let resp = build_transcript_response(
            &Segment {
                text: "test",
                start: 1.0,
                duration: 0.5,
                confidence: 0.9,
            },
            None,
            TranscriptKind::Finalized,
            &meta,
            &[0, 2],
            None,
        );
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(v["from_finalize"], true);
        assert_eq!(v["channel_index"], serde_json::json!([0, 2]));
    }

    #[test]
    fn transcript_response_channel_index() {
        let meta = build_session_metadata(Path::new("/models/test"));
        let resp = build_transcript_response(
            &Segment {
                text: "speaker text",
                start: 0.0,
                duration: 1.0,
                confidence: 0.8,
            },
            None,
            TranscriptKind::Confirmed,
            &meta,
            &[1, 2],
            None,
        );
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(v["channel_index"], serde_json::json!([1, 2]));
    }

    #[test]
    fn terminal_response_serializes_as_metadata() {
        let resp = StreamResponse::TerminalResponse {
            request_id: "test-id".to_string(),
            created: "2026-01-01T00:00:00.000Z".to_string(),
            duration: 10.5,
            channels: 1,
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(v["type"], "Metadata");
        assert_eq!(v["request_id"], "test-id");
        assert_eq!(v["duration"], 10.5);
        assert_eq!(v["channels"], 1);
    }

    #[test]
    fn error_response_serializes() {
        let resp = StreamResponse::ErrorResponse {
            error_code: None,
            error_message: "model failed".to_string(),
            provider: "cactus".to_string(),
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(v["type"], "Error");
        assert_eq!(v["error_message"], "model failed");
        assert_eq!(v["provider"], "cactus");
    }

    #[test]
    fn speech_started_response_serializes() {
        let resp = StreamResponse::SpeechStartedResponse {
            channel: vec![0],
            timestamp: 1.23,
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(v["type"], "SpeechStarted");
        assert_eq!(v["timestamp"], 1.23);
    }

    #[test]
    fn utterance_end_response_serializes() {
        let resp = StreamResponse::UtteranceEndResponse {
            channel: vec![0],
            last_word_end: 5.67,
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(v["type"], "UtteranceEnd");
        assert_eq!(v["last_word_end"], 5.67);
    }

    #[test]
    fn word_timestamps_are_distributed_across_segment() {
        let meta = build_session_metadata(Path::new("/models/test"));
        let resp = build_transcript_response(
            &Segment {
                text: "one two three",
                start: 10.0,
                duration: 6.0,
                confidence: 0.9,
            },
            None,
            TranscriptKind::Confirmed,
            &meta,
            &[0, 1],
            None,
        );

        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        let words = v["channel"]["alternatives"][0]["words"].as_array().unwrap();
        assert_eq!(words.len(), 3);

        let starts: Vec<f64> = words.iter().map(|w| w["start"].as_f64().unwrap()).collect();
        let ends: Vec<f64> = words.iter().map(|w| w["end"].as_f64().unwrap()).collect();

        assert!(
            starts[0] < starts[1],
            "words must have ascending start times"
        );
        assert!(
            starts[1] < starts[2],
            "words must have ascending start times"
        );

        assert!(ends[0] < ends[2], "end times must increase");

        let segment_end = 10.0 + 6.0;
        let last_end = ends[2];
        assert!(
            segment_end - last_end > 0.05,
            "last word must end >50ms before segment boundary (gap={:.3}s)",
            segment_end - last_end
        );
    }

    #[test]
    fn single_word_has_gap_before_segment_end() {
        let meta = build_session_metadata(Path::new("/models/test"));
        let resp = build_transcript_response(
            &Segment {
                text: "hello",
                start: 5.0,
                duration: 3.0,
                confidence: 0.9,
            },
            None,
            TranscriptKind::Confirmed,
            &meta,
            &[0, 1],
            None,
        );

        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        let words = v["channel"]["alternatives"][0]["words"].as_array().unwrap();
        assert_eq!(words.len(), 1);

        let word_end = words[0]["end"].as_f64().unwrap();
        let segment_end = 5.0 + 3.0;
        assert!(
            segment_end - word_end > 0.05,
            "single word must end >50ms before segment boundary (gap={:.3}s)",
            segment_end - word_end
        );
    }
}
