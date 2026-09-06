//! Batch transcription responses → stored transcript rows, ported from
//! `store/zustand/listener/{batch,utils}.ts`, `stt/timing.ts`, and the
//! persist callback in `stt/useRunBatch.ts`.

use owhisper_interface::batch::{Response, Word};
use serde_json::{Map, Value, json};

/// `EMPTY_BATCH_TRANSCRIPT_ERROR`
pub const EMPTY_BATCH_TRANSCRIPT_ERROR: &str = "No speech was detected in the audio.";

const SYNTHETIC_TEXT_WORD_SECONDS: f64 = 0.4;
const MIN_SYNTHETIC_TEXT_WORD_SECONDS: f64 = 0.05;

const SYNTHETIC_BATCH_PROGRESS_INITIAL: f64 = 0.06;
const SYNTHETIC_BATCH_PROGRESS_MAX: f64 = 0.88;
pub const SYNTHETIC_BATCH_PROGRESS_INTERVAL_MS: u64 = 800;
const SYNTHETIC_BATCH_PROGRESS_TIME_CONSTANT_MS: f64 = 32_000.0;

/// `TranscriptTimingSource`
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimingSource {
    ProviderWord,
    ProviderSegmentInterpolated,
    SyntheticText,
}

impl TimingSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::ProviderWord => "provider_word",
            Self::ProviderSegmentInterpolated => "provider_segment_interpolated",
            Self::SyntheticText => "synthetic_text",
        }
    }

    /// `getValidTimingSource`
    fn parse(value: Option<&Value>) -> Option<Self> {
        match value.and_then(Value::as_str) {
            Some("provider_word") => Some(Self::ProviderWord),
            Some("provider_segment_interpolated") => Some(Self::ProviderSegmentInterpolated),
            Some("synthetic_text") => Some(Self::SyntheticText),
            _ => None,
        }
    }
}

/// `WordLike` with the `RuntimeSpeakerHint` folded in: the transform's
/// output before ids are assigned.
#[derive(Clone, PartialEq, Debug)]
pub struct BatchWord {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub channel: i64,
    pub metadata: Value,
    pub speaker_index: Option<i64>,
}

/// `createTranscriptTimingMetadata(source, metadata)`
fn timing_metadata(source: TimingSource, metadata: Option<&Value>) -> Value {
    let mut base = metadata
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut timing = base
        .get("timing")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    timing.insert("source".into(), Value::String(source.as_str().into()));
    base.insert("timing".into(), Value::Object(timing));
    Value::Object(base)
}

/// `fixSpacingForWords(words, transcript)`: each word keeps the transcript's
/// exact inter-word text as its prefix (the first word gets a single space).
pub fn fix_spacing_for_words(words: &[String], transcript: &str) -> Vec<String> {
    let mut result = Vec::with_capacity(words.len());
    let mut pos = 0usize;
    for (i, word) in words.iter().enumerate() {
        let trimmed = word.trim();
        if trimmed.is_empty() {
            result.push(word.clone());
            continue;
        }
        let Some(found_at) = transcript
            .get(pos..)
            .and_then(|rest| rest.find(trimmed))
            .map(|ix| pos + ix)
        else {
            result.push(word.clone());
            continue;
        };
        let prefix = if i == 0 {
            " "
        } else {
            &transcript[pos..found_at]
        };
        result.push(format!("{prefix}{trimmed}"));
        pos = found_at + trimmed.len();
    }
    result
}

/// `getWordTimingSourceForBatchResponse`
fn response_timing_source(
    metadata: &Value,
    has_provider_words: bool,
    fallback: TimingSource,
) -> TimingSource {
    if !has_provider_words {
        return fallback;
    }
    let explicit = metadata
        .as_object()
        .and_then(|record| match record.get("timing") {
            Some(Value::Object(timing)) => TimingSource::parse(timing.get("source")),
            _ => TimingSource::parse(record.get("timing_source")),
        });
    explicit.unwrap_or(TimingSource::ProviderWord)
}

/// `getBatchDurationSeconds`
fn response_duration_seconds(metadata: &Value) -> Option<f64> {
    metadata
        .get("duration")
        .and_then(Value::as_f64)
        .filter(|duration| duration.is_finite() && *duration > 0.0)
}

/// `wordEntriesFromTranscript` + `transformWordEntries` for one channel.
fn transform_channel(
    words: &[Word],
    transcript: &str,
    channel: i64,
    duration_seconds: Option<f64>,
    timing_source: TimingSource,
) -> Vec<BatchWord> {
    struct Entry {
        text: String,
        start: f64,
        end: f64,
        channel: i64,
        speaker: Option<i64>,
    }
    let entries: Vec<Entry> = if !words.is_empty() {
        words
            .iter()
            .map(|word| Entry {
                text: word
                    .punctuated_word
                    .clone()
                    .unwrap_or_else(|| word.word.clone()),
                start: word.start,
                end: word.end,
                channel: word.channel as i64,
                speaker: word.speaker.map(|speaker| speaker as i64),
            })
            .collect()
    } else {
        let tokens: Vec<&str> = transcript.split_whitespace().collect();
        if tokens.is_empty() {
            return Vec::new();
        }
        let count = tokens.len() as f64;
        let duration = if timing_source == TimingSource::SyntheticText {
            count * SYNTHETIC_TEXT_WORD_SECONDS
        } else {
            duration_seconds
                .filter(|duration| duration.is_finite())
                .unwrap_or(count * SYNTHETIC_TEXT_WORD_SECONDS)
                .max(count * MIN_SYNTHETIC_TEXT_WORD_SECONDS)
        };
        tokens
            .iter()
            .enumerate()
            .map(|(index, token)| Entry {
                text: token.to_string(),
                start: (index as f64 / count) * duration,
                end: ((index + 1) as f64 / count) * duration,
                channel,
                speaker: None,
            })
            .collect()
    };
    let texts: Vec<String> = entries.iter().map(|entry| entry.text.clone()).collect();
    let spaced = fix_spacing_for_words(&texts, transcript);
    entries
        .into_iter()
        .zip(spaced)
        .map(|(entry, text)| BatchWord {
            text,
            start_ms: (entry.start * 1000.0).round() as i64,
            end_ms: (entry.end * 1000.0).round() as i64,
            channel: entry.channel,
            metadata: timing_metadata(timing_source, None),
            speaker_index: entry.speaker,
        })
        .collect()
}

/// `transformBatch(response)`: every channel's first alternative, in channel
/// order; provider words when present, otherwise the transcript split into
/// synthetically timed tokens.
pub fn transform_batch(response: &Response) -> Vec<BatchWord> {
    let mut all = Vec::new();
    for (channel_index, channel) in response.results.channels.iter().enumerate() {
        let Some(alternative) = channel.alternatives.first() else {
            continue;
        };
        let timing_source = response_timing_source(
            &response.metadata,
            !alternative.words.is_empty(),
            TimingSource::SyntheticText,
        );
        all.extend(transform_channel(
            &alternative.words,
            &alternative.transcript,
            channel_index as i64,
            response_duration_seconds(&response.metadata),
            timing_source,
        ));
    }
    all
}

/// The persist callback in `useRunBatch`: `WordWithId` rows (`metadata` as a
/// JSON string) and `provider_speaker_index` hints bound to the new word ids.
pub fn stage_words(words: &[BatchWord], provider: &str) -> (Vec<Value>, Vec<Value>) {
    let mut rows = Vec::with_capacity(words.len());
    let mut hints = Vec::new();
    for word in words {
        let word_id = uuid::Uuid::new_v4().to_string();
        let mut row = Map::new();
        row.insert("id".into(), Value::String(word_id.clone()));
        row.insert("text".into(), Value::String(word.text.clone()));
        row.insert("start_ms".into(), json!(word.start_ms));
        row.insert("end_ms".into(), json!(word.end_ms));
        row.insert("channel".into(), json!(word.channel));
        if !word.metadata.is_null() {
            row.insert("metadata".into(), Value::String(word.metadata.to_string()));
        }
        rows.push(Value::Object(row));
        if let Some(speaker_index) = word.speaker_index {
            hints.push(json!({
                "id": uuid::Uuid::new_v4().to_string(),
                "word_id": word_id,
                "type": "provider_speaker_index",
                "value": json!({
                    "provider": provider,
                    "channel": word.channel,
                    "speaker_index": speaker_index,
                })
                .to_string(),
            }));
        }
    }
    (rows, hints)
}

/// `syntheticBatchProgress(elapsedMs)`
pub fn synthetic_batch_progress(elapsed_ms: f64) -> f64 {
    let elapsed = elapsed_ms.max(0.0);
    let eased = 1.0 - (-elapsed / SYNTHETIC_BATCH_PROGRESS_TIME_CONSTANT_MS).exp();
    (SYNTHETIC_BATCH_PROGRESS_INITIAL
        + eased * (SYNTHETIC_BATCH_PROGRESS_MAX - SYNTHETIC_BATCH_PROGRESS_INITIAL))
        .min(SYNTHETIC_BATCH_PROGRESS_MAX)
}

/// `shouldUseSyntheticBatchProgress(params)`: providers that stream their own
/// progress (soniqo, argmax, whispercpp, the progressive OpenAI models, and
/// `am` against a local Argmax server or those models) do not get the timer.
pub fn should_use_synthetic_batch_progress(
    provider: &str,
    model: Option<&str>,
    base_url: &str,
) -> bool {
    const OPENAI_PROGRESSIVE_BATCH_MODELS: [&str; 4] = [
        "gpt-transcribe",
        "gpt-4o-transcribe",
        "gpt-4o-mini-transcribe",
        "gpt-4o-mini-transcribe-2025-12-15",
    ];
    let progressive_model =
        model.is_some_and(|model| OPENAI_PROGRESSIVE_BATCH_MODELS.contains(&model));
    match provider {
        "soniqo" | "argmax" | "whispercpp" => false,
        "am" => {
            let url = url::Url::parse(base_url).ok();
            let local_argmax = url.as_ref().is_some_and(|url| {
                matches!(
                    url.host_str(),
                    Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
                ) && !url.path().contains("/stt")
            });
            let openai = url.as_ref().is_some_and(|url| {
                url.host_str()
                    .is_some_and(|host| host == "openai.com" || host.ends_with(".openai.com"))
            });
            !(local_argmax || (openai && progressive_model))
        }
        "openai" => !progressive_model,
        _ => true,
    }
}

/// `getSessionSpeakerCount(participantHumanIds, selfHumanId)`
pub fn session_speaker_count<'a>(
    participant_human_ids: impl IntoIterator<Item = &'a str>,
    self_human_id: Option<&str>,
) -> Option<u32> {
    let mut ids: std::collections::HashSet<&str> = participant_human_ids
        .into_iter()
        .filter(|id| !id.is_empty())
        .collect();
    if let Some(self_id) = self_human_id.filter(|id| !id.is_empty()) {
        ids.insert(self_id);
    }
    (ids.len() > 1).then_some(ids.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use owhisper_interface::batch::{Alternatives, Channel, Results};

    fn word(text: &str, start: f64, end: f64, speaker: Option<usize>) -> Word {
        Word {
            word: text.to_string(),
            start,
            end,
            confidence: 1.0,
            channel: 0,
            speaker,
            punctuated_word: None,
        }
    }

    fn build_response(metadata: Value, channels: Vec<(&str, Vec<Word>)>) -> Response {
        Response {
            metadata,
            results: Results {
                channels: channels
                    .into_iter()
                    .map(|(transcript, words)| Channel {
                        alternatives: vec![Alternatives {
                            transcript: transcript.to_string(),
                            confidence: 1.0,
                            words,
                        }],
                    })
                    .collect(),
            },
        }
    }

    #[test]
    fn spacing_follows_the_transcript_and_leads_with_one_space() {
        let words = ["Hello,", "world", "again"].map(String::from);
        assert_eq!(
            fix_spacing_for_words(&words, "Hello,  world again"),
            vec![" Hello,", "  world", " again"]
        );
        // A word missing from the transcript keeps its own text.
        let words = ["Hello", "missing"].map(String::from);
        assert_eq!(
            fix_spacing_for_words(&words, "Hello there"),
            vec![" Hello", "missing"]
        );
    }

    #[test]
    fn provider_words_keep_timings_speakers_and_channel_order() {
        let response = build_response(
            json!({}),
            vec![
                (
                    "hello there",
                    vec![
                        word("hello", 0.0, 0.5, Some(0)),
                        word("there", 0.5, 1.0, Some(1)),
                    ],
                ),
                ("yes", vec![word("yes", 1.0, 1.5, None)]),
            ],
        );
        let words = transform_batch(&response);
        assert_eq!(words.len(), 3);
        assert_eq!(words[0].text, " hello");
        assert_eq!(words[1].text, " there");
        assert_eq!((words[1].start_ms, words[1].end_ms), (500, 1000));
        assert_eq!(words[0].speaker_index, Some(0));
        assert_eq!(words[1].speaker_index, Some(1));
        assert_eq!(words[2].speaker_index, None);
        // The word's own channel wins; the batch word here reports 0.
        assert_eq!(words[2].channel, 0);
        assert_eq!(
            words[0].metadata,
            json!({ "timing": { "source": "provider_word" } })
        );
    }

    #[test]
    fn transcripts_without_words_are_split_into_synthetic_tokens() {
        let response = build_response(json!({ "duration": 10.0 }), vec![("one two three", vec![])]);
        let words = transform_batch(&response);
        assert_eq!(words.len(), 3);
        // `synthetic_text` ignores the response duration: 0.4s per token.
        assert_eq!((words[0].start_ms, words[0].end_ms), (0, 400));
        assert_eq!((words[2].start_ms, words[2].end_ms), (800, 1200));
        assert_eq!(words[1].channel, 0);
        assert_eq!(
            words[1].metadata,
            json!({ "timing": { "source": "synthetic_text" } })
        );
        assert!(transform_batch(&build_response(json!({}), vec![("   ", vec![])])).is_empty());
    }

    #[test]
    fn explicit_response_timing_source_is_honoured() {
        let response = build_response(
            json!({ "timing": { "source": "provider_segment_interpolated" } }),
            vec![("hi", vec![word("hi", 0.0, 0.2, None)])],
        );
        assert_eq!(
            transform_batch(&response)[0].metadata,
            json!({ "timing": { "source": "provider_segment_interpolated" } })
        );
        let response = build_response(
            json!({ "timing_source": "synthetic_text" }),
            vec![("hi", vec![word("hi", 0.0, 0.2, None)])],
        );
        assert_eq!(
            transform_batch(&response)[0].metadata,
            json!({ "timing": { "source": "synthetic_text" } })
        );
    }

    #[test]
    fn staged_rows_match_the_persist_callback() {
        let words = transform_batch(&build_response(
            json!({}),
            vec![(
                "hello there",
                vec![
                    word("hello", 0.0, 0.5, Some(2)),
                    word("there", 0.5, 1.0, None),
                ],
            )],
        ));
        let (rows, hints) = stage_words(&words, "deepgram");
        assert_eq!(rows.len(), 2);
        let row = rows[0].as_object().unwrap();
        assert_eq!(
            row.keys().collect::<Vec<_>>(),
            ["id", "text", "start_ms", "end_ms", "channel", "metadata"]
        );
        assert_eq!(
            row["metadata"],
            json!(r#"{"timing":{"source":"provider_word"}}"#)
        );
        assert_eq!(hints.len(), 1);
        let hint = hints[0].as_object().unwrap();
        assert_eq!(hint["word_id"], row["id"]);
        assert_eq!(hint["type"], "provider_speaker_index");
        assert_eq!(
            hint["value"],
            json!(r#"{"provider":"deepgram","channel":0,"speaker_index":2}"#)
        );
    }

    #[test]
    fn synthetic_progress_eases_from_six_to_eighty_eight_percent() {
        assert!((synthetic_batch_progress(0.0) - 0.06).abs() < 1e-9);
        assert!(
            synthetic_batch_progress(32_000.0) > 0.57 && synthetic_batch_progress(32_000.0) < 0.58
        );
        assert!((synthetic_batch_progress(1e9) - 0.88).abs() < 1e-9);
        assert!((synthetic_batch_progress(-5.0) - 0.06).abs() < 1e-9);
    }

    #[test]
    fn synthetic_progress_applies_to_non_streaming_providers() {
        assert!(should_use_synthetic_batch_progress(
            "deepgram",
            Some("nova-3"),
            "https://api.deepgram.com"
        ));
        assert!(!should_use_synthetic_batch_progress(
            "soniqo",
            None,
            "soniqo://local"
        ));
        assert!(!should_use_synthetic_batch_progress("whispercpp", None, ""));
        assert!(!should_use_synthetic_batch_progress(
            "openai",
            Some("gpt-4o-transcribe"),
            "https://api.openai.com/v1"
        ));
        assert!(should_use_synthetic_batch_progress(
            "openai",
            Some("whisper-1"),
            "https://api.openai.com/v1"
        ));
        assert!(!should_use_synthetic_batch_progress(
            "am",
            None,
            "http://localhost:50060"
        ));
        assert!(should_use_synthetic_batch_progress(
            "am",
            None,
            "http://localhost:50060/stt"
        ));
        assert!(!should_use_synthetic_batch_progress(
            "am",
            Some("gpt-4o-transcribe"),
            "https://api.openai.com/v1"
        ));
        assert!(should_use_synthetic_batch_progress(
            "am",
            Some("whisper-1"),
            "https://api.openai.com/v1"
        ));
    }

    #[test]
    fn speaker_count_needs_more_than_one_distinct_human() {
        assert_eq!(session_speaker_count(["a", "b"], None), Some(2));
        assert_eq!(session_speaker_count(["a", "", "a"], Some("me")), Some(2));
        assert_eq!(session_speaker_count(["a"], None), None);
        assert_eq!(session_speaker_count(["a"], Some("a")), None);
        assert_eq!(session_speaker_count([], Some("me")), None);
    }
}
