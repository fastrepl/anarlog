use std::path::Path;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, stream::SplitSink};
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse, Word};

pub(super) type WsSender = SplitSink<WebSocket, Message>;

pub(super) async fn send_ws(sender: &mut WsSender, value: &StreamResponse) -> bool {
    let payload = match serde_json::to_string(value) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!("failed to serialize ws response: {error}");
            return false;
        }
    };

    sender.send(Message::Text(payload.into())).await.is_ok()
}

pub(super) async fn send_ws_best_effort(sender: &mut WsSender, value: &StreamResponse) {
    let payload = match serde_json::to_string(value) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!("failed to serialize ws response: {error}");
            return;
        }
    };

    let _ = sender.send(Message::Text(payload.into())).await;
}

pub(super) fn build_session_metadata(model_path: &Path) -> Metadata {
    crate::service::build_metadata(model_path)
}

pub(super) fn build_transcript_response(
    text: &str,
    start: f64,
    duration: f64,
    confidence: f64,
    language: Option<&str>,
    is_final: bool,
    speech_final: bool,
    from_finalize: bool,
    metadata: &Metadata,
    channel_index: &[i32],
    extra_keys: Option<std::collections::HashMap<String, serde_json::Value>>,
) -> StreamResponse {
    let languages = language.map(|l| vec![l.to_string()]).unwrap_or_default();

    let word_strs: Vec<&str> = text.split_whitespace().filter(|w| !w.is_empty()).collect();
    let n = word_strs.len();
    let words: Vec<Word> = word_strs
        .into_iter()
        .enumerate()
        .map(|(i, w)| {
            let word_start = start + (i as f64 / n as f64) * duration;
            let word_end = if i + 1 == n {
                // Ensure the last word ends >50ms before the segment boundary so
                // the stitch heuristic in crates/transcript doesn't merge it with
                // the first word of the next segment (gap <= STITCH_MAX_GAP_MS=50ms).
                (start + duration - 0.1_f64).max(word_start + 0.05_f64)
            } else {
                start + ((i + 1) as f64 / n as f64) * duration
            };
            Word {
                word: w.to_string(),
                start: word_start,
                end: word_end,
                confidence,
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
        start,
        duration,
        is_final,
        speech_final,
        from_finalize,
        channel: Channel {
            alternatives: vec![Alternatives {
                transcript: text.to_string(),
                languages,
                words,
                confidence,
            }],
        },
        metadata: meta,
        channel_index: channel_index.to_vec(),
    }
}

pub(super) fn format_timestamp_now() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = d.as_secs();
    let millis = d.subsec_millis();

    let mut days = total_secs / 86400;
    let day_secs = (total_secs % 86400) as u32;
    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;

    let mut year = 1970i32;
    loop {
        let ydays = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366u64
        } else {
            365
        };
        if days < ydays {
            break;
        }
        days -= ydays;
        year += 1;
    }

    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let mdays = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for &md in &mdays {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    let day = days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hours, minutes, seconds, millis
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use owhisper_interface::stream::StreamResponse;

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
            "hello world",
            0.0,
            1.5,
            0.95,
            Some("en"),
            true,
            true,
            false,
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
            "test",
            1.0,
            0.5,
            0.9,
            None,
            true,
            true,
            true,
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
            "speaker text",
            0.0,
            1.0,
            0.8,
            None,
            true,
            true,
            false,
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
            "one two three",
            10.0,
            6.0,
            0.9,
            None,
            true,
            true,
            false,
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
            "hello",
            5.0,
            3.0,
            0.9,
            None,
            true,
            true,
            false,
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
