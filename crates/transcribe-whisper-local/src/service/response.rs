#[cfg(test)]
use std::path::Path;

pub(super) use hypr_transcribe_core::{format_timestamp_now, send_ws, send_ws_best_effort};
use owhisper_interface::{batch, stream};

#[derive(Debug, Clone, Copy)]
pub(super) enum TranscriptKind {
    Confirmed,
    Finalized,
}

#[cfg(test)]
pub(super) fn build_session_metadata(model_path: &Path) -> stream::Metadata {
    crate::service::build_metadata(model_path)
}

pub(super) fn build_transcript_response(
    segment: &crate::service::Segment,
    kind: TranscriptKind,
    metadata: &stream::Metadata,
    channel_index: &[i32],
) -> stream::StreamResponse {
    let from_finalize = matches!(kind, TranscriptKind::Finalized);
    let languages = segment
        .language
        .as_ref()
        .map(|value| vec![value.clone()])
        .unwrap_or_default();

    stream::StreamResponse::TranscriptResponse {
        start: segment.start,
        duration: segment.duration,
        is_final: true,
        speech_final: true,
        from_finalize,
        channel: stream::Channel {
            alternatives: vec![stream::Alternatives {
                transcript: segment.text.clone(),
                languages,
                words: build_stream_words(segment),
                confidence: segment.confidence,
            }],
        },
        metadata: metadata.clone(),
        channel_index: channel_index.to_vec(),
    }
}

pub(super) fn build_stream_words(segment: &crate::service::Segment) -> Vec<stream::Word> {
    let word_strs: Vec<&str> = segment
        .text
        .split_whitespace()
        .filter(|word| !word.is_empty())
        .collect();
    let count = word_strs.len();

    if count == 0 || segment.duration <= 0.0 {
        return vec![];
    }

    word_strs
        .into_iter()
        .enumerate()
        .map(|(index, word)| {
            let word_start = segment.start + (index as f64 / count as f64) * segment.duration;
            let word_end = if index + 1 == count {
                (segment.start + segment.duration - 0.1_f64).max(word_start + 0.05_f64)
            } else {
                segment.start + ((index + 1) as f64 / count as f64) * segment.duration
            };

            stream::Word {
                word: word.to_string(),
                start: word_start,
                end: word_end,
                confidence: segment.confidence,
                speaker: None,
                punctuated_word: None,
                language: None,
            }
        })
        .collect()
}

pub(super) fn build_batch_words(
    segment: &crate::service::Segment,
    channel: i32,
) -> Vec<batch::Word> {
    build_stream_words(segment)
        .into_iter()
        .map(|word| batch::Word {
            channel,
            ..batch::Word::from(word)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_metadata_has_required_fields() {
        let meta = build_session_metadata(Path::new("/tmp/ggml-small-q8_0.bin"));
        assert!(!meta.request_id.is_empty());
        assert!(!meta.model_uuid.is_empty());
        assert_eq!(meta.model_info.name, "ggml-small-q8_0");
        assert_eq!(meta.model_info.arch, "whisper-local");
        assert!(meta.extra.is_some());
    }

    #[test]
    fn transcript_response_serializes_as_results() {
        let meta = build_session_metadata(Path::new("/tmp/model.bin"));
        let response = build_transcript_response(
            &crate::service::Segment {
                text: "hello world".to_string(),
                start: 0.0,
                duration: 1.5,
                confidence: 0.9,
                language: Some("en".to_string()),
            },
            TranscriptKind::Confirmed,
            &meta,
            &[0, 1],
        );

        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&response).unwrap()).unwrap();
        assert_eq!(value["type"], "Results");
        assert_eq!(
            value["channel"]["alternatives"][0]["transcript"],
            "hello world"
        );
        assert_eq!(value["channel"]["alternatives"][0]["languages"][0], "en");
    }
}
