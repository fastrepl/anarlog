use owhisper_interface::{batch, stream};

use crate::{DiarizationSegment, FileTranscript, FileTranscriptChunk, SoniqoModel};

pub(crate) const SYNTHETIC_BATCH_WORD_SECONDS: f64 = 0.4;
const MIN_SYNTHETIC_DURATION_SECONDS: f64 = 0.05;

pub fn stream_response_from_text(
    model: SoniqoModel,
    text: String,
    start: f64,
    duration: f64,
    is_final: bool,
    channel_index: &[i32],
) -> stream::StreamResponse {
    let text = normalize_transcript_text(&text);
    let duration = duration.max(MIN_SYNTHETIC_DURATION_SECONDS);
    let words = stream_words_from_text(&text, start, duration);

    stream::StreamResponse::TranscriptResponse {
        start,
        duration,
        is_final,
        speech_final: is_final,
        from_finalize: false,
        channel: stream::Channel {
            alternatives: vec![stream::Alternatives {
                transcript: text,
                words,
                confidence: 1.0,
                languages: vec![],
            }],
        },
        metadata: metadata(model),
        channel_index: channel_index.to_vec(),
    }
}

pub fn batch_response_from_text(
    model: SoniqoModel,
    text: String,
    duration_seconds: f64,
) -> batch::Response {
    batch_response_from_channels(model, vec![FileTranscript::new(text, duration_seconds)])
}

pub fn batch_response_from_channels(
    model: SoniqoModel,
    channels: Vec<FileTranscript>,
) -> batch::Response {
    let channels = if channels.is_empty() {
        vec![FileTranscript::new(String::new(), 0.05)]
    } else {
        channels
    };
    let duration_seconds = channels
        .iter()
        .map(|channel| channel.duration_seconds)
        .fold(0.05, f64::max);
    let has_diarization = channels
        .iter()
        .any(|channel| !channel.speaker_segments.is_empty());
    let metadata = metadata_json(
        model,
        duration_seconds,
        channels.len() as u32,
        has_diarization,
    );

    batch::Response {
        metadata,
        results: batch::Results {
            channels: channels
                .into_iter()
                .enumerate()
                .map(|(channel_index, channel)| {
                    let transcript = normalize_transcript_text(&channel.text);
                    let words = if channel.chunks.is_empty() {
                        let synthetic_duration = synthetic_text_duration(&transcript);
                        batch_words_from_text(
                            &transcript,
                            0.0,
                            synthetic_duration,
                            channel_index as i32,
                        )
                    } else {
                        batch_words_from_chunks(
                            &channel.chunks,
                            &channel.speaker_segments,
                            channel_index as i32,
                        )
                    };
                    batch::Channel {
                        alternatives: vec![batch::Alternatives {
                            words,
                            transcript,
                            confidence: 1.0,
                        }],
                    }
                })
                .collect(),
        },
    }
}

fn metadata(model: SoniqoModel) -> stream::Metadata {
    stream::Metadata {
        model_info: stream::ModelInfo {
            name: model.as_str().to_string(),
            version: "0.0.9".to_string(),
            arch: "soniqo".to_string(),
        },
        extra: Some(stream::Extra::default().into()),
        ..Default::default()
    }
}

fn metadata_json(
    model: SoniqoModel,
    duration_seconds: f64,
    channels: u32,
    has_diarization: bool,
) -> serde_json::Value {
    let mut value = serde_json::to_value(metadata(model)).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = value.as_object_mut() {
        object.insert("duration".to_string(), serde_json::json!(duration_seconds));
        object.insert("channels".to_string(), serde_json::json!(channels));
        object.insert(
            "timing_source".to_string(),
            serde_json::json!(if has_diarization {
                "diarized_speech"
            } else {
                "synthetic_text"
            }),
        );
    }
    value
}

fn stream_words_from_text(text: &str, start: f64, duration: f64) -> Vec<stream::Word> {
    let word_strs = split_words(text);
    let count = word_strs.len();

    if count == 0 {
        return Vec::new();
    }

    word_strs
        .into_iter()
        .enumerate()
        .map(|(index, word)| {
            let word_start = start + (index as f64 / count as f64) * duration;
            let word_end = if index + 1 == count {
                (start + duration - 0.05).max(word_start + 0.05)
            } else {
                start + ((index + 1) as f64 / count as f64) * duration
            };

            stream::Word {
                word: word.to_string(),
                start: word_start,
                end: word_end,
                confidence: 1.0,
                speaker: None,
                punctuated_word: Some(word.to_string()),
                language: None,
            }
        })
        .collect()
}

fn batch_words_from_chunks(
    chunks: &[FileTranscriptChunk],
    speaker_segments: &[DiarizationSegment],
    channel: i32,
) -> Vec<batch::Word> {
    chunks
        .iter()
        .flat_map(|chunk| {
            let text = normalize_transcript_text(&chunk.text);
            if !speaker_segments.is_empty() {
                return batch_words_from_diarized_speech(
                    &text,
                    chunk.start_seconds,
                    chunk.duration_seconds,
                    channel,
                    speaker_segments,
                );
            }

            let duration = synthetic_text_duration(&text)
                .min(chunk.duration_seconds.max(MIN_SYNTHETIC_DURATION_SECONDS));
            let start = chunk.start_seconds.max(0.0);
            batch_words_from_text(&text, start, duration, channel)
        })
        .collect()
}

fn batch_words_from_diarized_speech(
    text: &str,
    start: f64,
    duration: f64,
    channel: i32,
    speaker_segments: &[DiarizationSegment],
) -> Vec<batch::Word> {
    let words = split_words(text);
    if words.is_empty() {
        return Vec::new();
    }

    let chunk_start = start.max(0.0);
    let chunk_end = chunk_start + duration.max(MIN_SYNTHETIC_DURATION_SECONDS);
    let mut spans = speaker_segments
        .iter()
        .filter_map(|segment| {
            let start = segment.start_seconds.max(chunk_start);
            let end = segment.end_seconds.min(chunk_end);
            (end > start).then_some((start, end, segment.speaker_index))
        })
        .collect::<Vec<_>>();
    spans.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    let mut non_overlapping = Vec::with_capacity(spans.len());
    let mut previous_end = chunk_start;
    for (start, end, speaker) in spans {
        let start = start.max(previous_end);
        if end > start {
            non_overlapping.push((start, end, speaker));
            previous_end = end;
        }
    }
    let spans = non_overlapping;

    let active_duration = spans.iter().map(|(start, end, _)| end - start).sum::<f64>();
    if active_duration <= 0.0 {
        let duration =
            synthetic_text_duration(text).min(duration.max(MIN_SYNTHETIC_DURATION_SECONDS));
        return batch_words_from_text(text, chunk_start, duration, channel);
    }

    let position = |offset: f64| {
        let mut remaining = offset.clamp(0.0, active_duration);
        for (index, (start, end, speaker)) in spans.iter().enumerate() {
            let span_duration = end - start;
            if remaining <= span_duration || index + 1 == spans.len() {
                return ((start + remaining.min(span_duration)), index, *speaker);
            }
            remaining -= span_duration;
        }
        (chunk_end, spans.len() - 1, spans[spans.len() - 1].2)
    };
    let count = words.len();

    words
        .into_iter()
        .enumerate()
        .map(|(index, word)| {
            let start_offset = index as f64 / count as f64 * active_duration;
            let end_offset = (index + 1) as f64 / count as f64 * active_duration;
            let midpoint_offset = (start_offset + end_offset) / 2.0;
            let (mapped_start, _, _) = position(start_offset);
            let (mapped_end, _, _) = position(end_offset);
            let (midpoint, span_index, speaker) = position(midpoint_offset);
            let (span_start, span_end, _) = spans[span_index];
            let word_start = mapped_start.max(span_start).min(span_end);
            let word_end = mapped_end
                .min(span_end)
                .max((word_start + MIN_SYNTHETIC_DURATION_SECONDS).min(span_end));
            let (word_start, word_end) = if word_end > word_start {
                (word_start, word_end)
            } else {
                let half = MIN_SYNTHETIC_DURATION_SECONDS / 2.0;
                (
                    (midpoint - half).max(span_start),
                    (midpoint + half).min(span_end),
                )
            };

            batch::Word {
                word: word.to_string(),
                start: word_start,
                end: word_end.max(word_start + f64::EPSILON),
                confidence: 1.0,
                channel,
                speaker: Some(speaker),
                punctuated_word: Some(word.to_string()),
            }
        })
        .collect()
}

fn batch_words_from_text(text: &str, start: f64, duration: f64, channel: i32) -> Vec<batch::Word> {
    let word_strs = split_words(text);
    let count = word_strs.len();

    if count == 0 {
        return Vec::new();
    }

    word_strs
        .into_iter()
        .enumerate()
        .map(|(index, word)| batch::Word {
            word: word.to_string(),
            start: start + (index as f64 / count as f64) * duration,
            end: start + ((index + 1) as f64 / count as f64) * duration,
            confidence: 1.0,
            channel,
            speaker: None,
            punctuated_word: Some(word.to_string()),
        })
        .collect()
}

fn split_words(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .filter(|word| !word.is_empty())
        .collect()
}

fn normalize_transcript_text(text: &str) -> String {
    split_words(text).join(" ")
}

fn synthetic_text_duration(text: &str) -> f64 {
    let word_count = split_words(text).len();
    if word_count == 0 {
        MIN_SYNTHETIC_DURATION_SECONDS
    } else {
        (word_count as f64 * SYNTHETIC_BATCH_WORD_SECONDS).max(MIN_SYNTHETIC_DURATION_SECONDS)
    }
}
