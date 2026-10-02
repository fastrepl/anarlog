use std::{cmp::Ordering, collections::HashMap, hash::Hash, sync::LazyLock};

use regex::Regex;
use serde::de::DeserializeOwned;
use serde_json::{Number, Value};

use crate::{ChannelProfile, IdentityScope, RenderTranscriptInput, RenderTranscriptWordInput};

static NON_LETTER_OR_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^\p{L}\p{N}]").expect("valid Unicode character regex"));
static PUNCTUATION_OR_SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\p{P}\p{Z}]").expect("valid Unicode category regex"));

const MIN_TRANSCRIPT_CHARACTER_LOSS: usize = 200;
const MIN_TRANSCRIPT_RETAINED_RATIO: f64 = 0.5;
const MIN_REFINED_SPEAKER_OVERLAP_RATIO: f64 = 0.6;
const MIN_REFINED_ASSIGNMENT_COVERAGE_RATIO: f64 = 0.8;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct StoredTranscriptWord {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct StoredSpeakerHint {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub word_id: Option<String>,
    #[serde(rename = "type")]
    pub hint_type: String,
    #[serde(default)]
    pub value: Value,
}

pub fn parse_stored_transcript_words(json: &str) -> Vec<StoredTranscriptWord> {
    parse_stored_json_array(json)
}

pub fn parse_stored_speaker_hints(json: &str) -> Vec<StoredSpeakerHint> {
    let Ok(values) = serde_json::from_str::<Vec<Value>>(json) else {
        return Vec::new();
    };
    values
        .into_iter()
        .filter_map(|mut value| {
            if let Value::Object(object) = &mut value
                && !object.get("id").is_some_and(Value::is_string)
                && let Some(id) = object
                    .get("word_id")
                    .and_then(Value::as_str)
                    .zip(object.get("type").and_then(Value::as_str))
                    .map(|(word_id, hint_type)| format!("{word_id}:{hint_type}"))
            {
                object.insert("id".to_string(), Value::String(id));
            }
            serde_json::from_value(value).ok()
        })
        .collect()
}

fn parse_stored_json_array<T: DeserializeOwned>(json: &str) -> Vec<T> {
    serde_json::from_str::<Vec<Value>>(json)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect()
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum BatchTranscriptPromotion {
    PreserveExisting,
    WholeSession,
    CurrentCapture {
        audio_offset_ms: f64,
        #[serde(default)]
        replace_transcript_id: Option<String>,
        started_at: f64,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct BatchRefinementSource {
    pub id: String,
    pub started_at: f64,
    pub words: Vec<StoredTranscriptWord>,
    pub speaker_hints: Vec<StoredSpeakerHint>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct BatchRefinementRequest {
    pub words: Vec<StoredTranscriptWord>,
    pub hints: Vec<StoredSpeakerHint>,
    pub promotion: BatchTranscriptPromotion,
    pub previous_transcripts: Vec<BatchRefinementSource>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SpeakerClusterReconciliationRequest {
    pub source: BatchRefinementSource,
    pub words: Vec<StoredTranscriptWord>,
    pub hints: Vec<StoredSpeakerHint>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BatchRefinementOutcome {
    Ready {
        words: Vec<StoredTranscriptWord>,
        speaker_hints: Vec<StoredSpeakerHint>,
        replace_session: bool,
        replace_transcript_id: Option<String>,
        started_at: Option<f64>,
    },
    EmptyCurrentCapture,
    Truncated,
}

fn rebase_synthetic_chunk_start(metadata: &mut Value, offset_ms: f64) {
    if let Value::String(json) = metadata {
        if let Ok(mut value) = serde_json::from_str::<Value>(json) {
            rebase_synthetic_chunk_start(&mut value, offset_ms);
            *json = value.to_string();
        }
        return;
    }
    let Some(timing) = metadata.get_mut("timing").and_then(Value::as_object_mut) else {
        return;
    };
    if timing.get("source").and_then(Value::as_str) != Some("synthetic_text") {
        return;
    }
    if let Some(start) = timing.get("chunk_start_ms").and_then(Value::as_f64)
        && start.is_finite()
    {
        timing.insert(
            "chunk_start_ms".into(),
            Value::from((start - offset_ms).max(0.0)),
        );
    }
}

pub fn refine_batch_transcript(request: BatchRefinementRequest) -> BatchRefinementOutcome {
    let BatchRefinementRequest {
        words,
        hints,
        promotion,
        previous_transcripts,
    } = request;
    let (mut words, mut hints, replace_session, replace_transcript_id, promoted_started_at) =
        match promotion {
            BatchTranscriptPromotion::PreserveExisting => (words, hints, false, None, None),
            BatchTranscriptPromotion::WholeSession => (words, hints, true, None, None),
            BatchTranscriptPromotion::CurrentCapture {
                audio_offset_ms,
                replace_transcript_id,
                started_at,
            } => {
                let offset_ms = if audio_offset_ms.is_finite() {
                    audio_offset_ms.max(0.0)
                } else {
                    0.0
                };
                let words = words
                    .into_iter()
                    .filter_map(|mut word| {
                        let start_ms = word.start_ms.unwrap_or(0.0);
                        let end_ms = word.end_ms.unwrap_or(start_ms);
                        if end_ms <= offset_ms {
                            return None;
                        }
                        word.start_ms = Some((start_ms - offset_ms).max(0.0));
                        word.end_ms = Some((end_ms - offset_ms).max(0.0));
                        if let Some(metadata) = word.metadata.as_mut() {
                            rebase_synthetic_chunk_start(metadata, offset_ms);
                        }
                        Some(word)
                    })
                    .collect::<Vec<_>>();
                let current_word_ids = words
                    .iter()
                    .map(|word| word.id.as_str())
                    .collect::<std::collections::HashSet<_>>();
                let hints = hints
                    .into_iter()
                    .filter(|hint| {
                        hint.word_id
                            .as_ref()
                            .is_some_and(|word_id| current_word_ids.contains(word_id.as_str()))
                    })
                    .collect();
                (words, hints, false, replace_transcript_id, Some(started_at))
            }
        };

    if matches!((replace_session, promoted_started_at), (false, Some(_))) && words.is_empty() {
        return BatchRefinementOutcome::EmptyCurrentCapture;
    }

    let refined_source = if let Some(replace_transcript_id) = replace_transcript_id.as_ref() {
        previous_transcripts
            .iter()
            .find(|source| &source.id == replace_transcript_id)
    } else {
        None
    };

    if let [source] = previous_transcripts.as_slice() {
        words = restore_refined_source_channels(&source.words, words);
        hints = restore_refined_source_hints(&words, hints);
    }

    let previous_words = previous_transcripts
        .iter()
        .flat_map(|source| source.words.iter())
        .cloned()
        .collect::<Vec<_>>();
    if is_transcript_truncated(&previous_words, &words) {
        return BatchRefinementOutcome::Truncated;
    }

    let speaker_hints = if !words.is_empty() {
        refined_source.map_or(hints.clone(), |source| {
            reconcile_refined_speaker_clusters(source, &words, hints)
        })
    } else {
        hints
    };
    let started_at = promoted_started_at
        .or_else(|| (previous_transcripts.len() == 1).then(|| previous_transcripts[0].started_at));

    BatchRefinementOutcome::Ready {
        words,
        speaker_hints,
        replace_session,
        replace_transcript_id,
        started_at,
    }
}

#[derive(Clone)]
struct SpeakerKey {
    channel: f64,
    speaker_index: f64,
    key: String,
}

#[derive(Clone)]
struct SpeakerInterval {
    speaker_key: String,
    start_ms: f64,
    end_ms: f64,
}

#[derive(Clone)]
struct TargetWord {
    speaker_key: String,
    channel: f64,
    start_ms: f64,
    end_ms: f64,
}

#[derive(Clone)]
struct RenderWord {
    id: String,
    text: String,
    start_ms: f64,
    end_ms: f64,
    channel: f64,
    speaker_index: Option<f64>,
}

#[derive(Clone)]
enum AssignmentScope {
    Channel(ChannelProfile),
    ChannelSpeaker {
        channel: ChannelProfile,
        speaker_index: f64,
    },
    Words(Vec<String>),
}

#[derive(Clone)]
struct IdentityAssignment {
    human_id: String,
    scope: AssignmentScope,
}

#[derive(serde::Serialize)]
struct UserSegmentAssignmentValue<'a> {
    human_id: &'a str,
    scope: &'static str,
    word_ids: &'a [String],
    extend_to_adjacent: bool,
}

#[derive(Clone)]
struct RenderTranscript {
    words: Vec<RenderWord>,
    assignments: Vec<IdentityAssignment>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct NumericKey(u64);

impl NumericKey {
    fn new(value: f64) -> Self {
        Self(if value == 0.0 { 0 } else { value.to_bits() })
    }
}

struct OrderedMap<K, V> {
    entries: Vec<(K, V)>,
    indices: HashMap<K, usize>,
}

impl<K, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            indices: HashMap::new(),
        }
    }
}

impl<K: Eq + Hash + Clone, V> OrderedMap<K, V> {
    fn get(&self, key: &K) -> Option<&V> {
        self.indices.get(key).map(|index| &self.entries[*index].1)
    }

    fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let index = *self.indices.get(key)?;
        Some(&mut self.entries[index].1)
    }

    fn set(&mut self, key: K, value: V) {
        if let Some(index) = self.indices.get(&key).copied() {
            self.entries[index].1 = value;
        } else {
            self.indices.insert(key.clone(), self.entries.len());
            self.entries.push((key, value));
        }
    }

    fn contains_key(&self, key: &K) -> bool {
        self.indices.contains_key(key)
    }

    fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }
}

fn restore_refined_source_channels(
    previous: &[StoredTranscriptWord],
    replacement: Vec<StoredTranscriptWord>,
) -> Vec<StoredTranscriptWord> {
    let mut original = previous
        .iter()
        .filter(|word| word.channel == Some(0.0) || word.channel == Some(1.0))
        .cloned()
        .collect::<Vec<_>>();
    original.sort_by(|left, right| {
        compare_numbers(left.start_ms.unwrap_or(0.0), right.start_ms.unwrap_or(0.0))
    });

    let mut cursor = 0;
    let mut last_start = f64::NEG_INFINITY;
    replacement
        .into_iter()
        .map(|mut word| {
            if word.channel != Some(2.0) {
                return word;
            }

            let start_ms = word.start_ms.unwrap_or(0.0);
            let end_ms = word.end_ms.unwrap_or(start_ms);
            let text = normalize_source_channel_text(word.text.as_deref().unwrap_or(""));
            if text.is_empty() || end_ms <= start_ms {
                return word;
            }
            if start_ms < last_start {
                cursor = 0;
            }
            last_start = start_ms;

            while cursor < original.len() && original[cursor].end_ms.unwrap_or(0.0) <= start_ms {
                cursor += 1;
            }

            let mut channels = Vec::new();
            let mut matched = false;
            for source in &original[cursor..] {
                let source_start = source.start_ms.unwrap_or(0.0);
                if source_start >= end_ms {
                    break;
                }
                let source_end = source.end_ms.unwrap_or(source_start);
                let overlap = end_ms.min(source_end) - start_ms.max(source_start);
                if overlap <= 0.0 {
                    continue;
                }
                let channel = source.channel.expect("source channels were filtered");
                insert_set_value(&mut channels, channel);
                if normalize_source_channel_text(source.text.as_deref().unwrap_or("")) == text
                    && overlap >= 0.8 * (end_ms - start_ms).max(source_end - source_start)
                {
                    matched = true;
                }
            }
            if matched && channels.len() == 1 {
                word.channel = channels.first().copied();
            }
            word
        })
        .collect()
}

fn restore_refined_source_hints(
    words: &[StoredTranscriptWord],
    hints: Vec<StoredSpeakerHint>,
) -> Vec<StoredSpeakerHint> {
    let mut channels = OrderedMap::default();
    for word in words {
        channels.set(word.id.clone(), word.channel);
    }

    hints
        .into_iter()
        .map(|mut hint| {
            if hint.hint_type != "provider_speaker_index" {
                return hint;
            }
            let word_id = hint.word_id.as_deref().unwrap_or("");
            let Some(channel) = channels
                .get(&word_id.to_string())
                .copied()
                .flatten()
                .filter(|channel| *channel == 0.0 || *channel == 1.0)
            else {
                return hint;
            };
            let Some(mut value) = parse_string_hint_value(&hint.value) else {
                return hint;
            };
            if value.get("channel").and_then(Value::as_f64) != Some(2.0) {
                return hint;
            }
            if let Value::Object(object) = &mut value {
                object.insert("channel".to_string(), json_number(channel));
                hint.value = Value::String(value.to_string());
            }
            hint
        })
        .collect()
}

fn is_transcript_truncated(
    previous: &[StoredTranscriptWord],
    replacement: &[StoredTranscriptWord],
) -> bool {
    let character_count = |words: &[StoredTranscriptWord]| {
        words
            .iter()
            .map(|word| {
                let text = word.text.as_deref().unwrap_or("");
                NON_LETTER_OR_NUMBER
                    .replace_all(text, "")
                    .encode_utf16()
                    .count()
            })
            .sum::<usize>()
    };
    let previous_length = character_count(previous);
    let replacement_length = character_count(replacement);
    previous_length.saturating_sub(replacement_length) >= MIN_TRANSCRIPT_CHARACTER_LOSS
        && (replacement_length as f64) < (previous_length as f64) * MIN_TRANSCRIPT_RETAINED_RATIO
}

fn normalize_source_channel_text(text: &str) -> String {
    let lowercased = text.trim().to_lowercase();
    PUNCTUATION_OR_SEPARATOR
        .replace_all(&lowercased, "")
        .into_owned()
}

fn speaker_keys_by_word_id(hints: &[StoredSpeakerHint]) -> OrderedMap<String, SpeakerKey> {
    let mut keys = OrderedMap::default();
    for hint in hints {
        if hint.hint_type != "provider_speaker_index"
            || hint.word_id.as_deref().is_none_or(str::is_empty)
        {
            continue;
        }
        let Some(value) = parse_string_hint_value(&hint.value) else {
            continue;
        };
        let Some(channel) = integral_number(value.get("channel")) else {
            continue;
        };
        let Some(speaker_index) = integral_number(value.get("speaker_index")) else {
            continue;
        };
        keys.set(
            hint.word_id.clone().expect("word IDs were checked"),
            SpeakerKey {
                channel,
                speaker_index,
                key: format!(
                    "{}:{}",
                    javascript_number_string(channel),
                    javascript_number_string(speaker_index)
                ),
            },
        );
    }
    keys
}

pub fn reconcile_refined_speaker_clusters(
    source: &BatchRefinementSource,
    words: &[StoredTranscriptWord],
    hints: Vec<StoredSpeakerHint>,
) -> Vec<StoredSpeakerHint> {
    let source_speaker_keys = speaker_keys_by_word_id(&source.speaker_hints);
    let target_speaker_keys = speaker_keys_by_word_id(&hints);
    if source_speaker_keys.entries.is_empty() || target_speaker_keys.entries.is_empty() {
        return reconcile_refined_speaker_assignments(source, words, hints);
    }

    let mut source_intervals_by_channel: OrderedMap<NumericKey, Vec<SpeakerInterval>> =
        OrderedMap::default();
    for word in &source.words {
        let Some(speaker_key) = source_speaker_keys.get(&word.id) else {
            continue;
        };
        let start_ms = word.start_ms.unwrap_or(0.0);
        let end_ms = (start_ms + 1.0).max(word.end_ms.unwrap_or(start_ms));
        let channel_key = NumericKey::new(speaker_key.channel);
        if let Some(intervals) = source_intervals_by_channel.get_mut(&channel_key) {
            intervals.push(SpeakerInterval {
                speaker_key: speaker_key.key.clone(),
                start_ms,
                end_ms,
            });
        } else {
            source_intervals_by_channel.set(
                channel_key,
                vec![SpeakerInterval {
                    speaker_key: speaker_key.key.clone(),
                    start_ms,
                    end_ms,
                }],
            );
        }
    }
    for (_, intervals) in source_intervals_by_channel.entries.iter_mut() {
        intervals.sort_by(|left, right| compare_numbers(left.start_ms, right.start_ms));
    }

    let mut target_words = words
        .iter()
        .filter_map(|word| {
            let speaker_key = target_speaker_keys.get(&word.id)?;
            let start_ms = word.start_ms.unwrap_or(0.0);
            Some(TargetWord {
                speaker_key: speaker_key.key.clone(),
                channel: speaker_key.channel,
                start_ms,
                end_ms: (start_ms + 1.0).max(word.end_ms.unwrap_or(start_ms)),
            })
        })
        .collect::<Vec<_>>();
    target_words.sort_by(|left, right| {
        compare_numbers(left.channel, right.channel)
            .then_with(|| compare_numbers(left.start_ms, right.start_ms))
    });

    let mut cursors: OrderedMap<NumericKey, usize> = OrderedMap::default();
    let mut weights: OrderedMap<String, OrderedMap<String, f64>> = OrderedMap::default();
    for word in &target_words {
        let channel_key = NumericKey::new(word.channel);
        let Some(source_intervals) = source_intervals_by_channel.get(&channel_key) else {
            continue;
        };
        let mut cursor = cursors.get(&channel_key).copied().unwrap_or(0);
        while cursor < source_intervals.len() && source_intervals[cursor].end_ms <= word.start_ms {
            cursor += 1;
        }
        cursors.set(channel_key, cursor);

        for source in source_intervals
            .iter()
            .skip(cursor)
            .take_while(|source| source.start_ms < word.end_ms)
        {
            let overlap_ms = word.end_ms.min(source.end_ms) - word.start_ms.max(source.start_ms);
            if overlap_ms <= 0.0 {
                continue;
            }
            if !weights.contains_key(&word.speaker_key) {
                weights.set(word.speaker_key.clone(), OrderedMap::default());
            }
            let source_weights = weights
                .get_mut(&word.speaker_key)
                .expect("target weights were initialized");
            let total = source_weights
                .get(&source.speaker_key)
                .copied()
                .unwrap_or(0.0)
                + overlap_ms;
            source_weights.set(source.speaker_key.clone(), total);
        }
    }

    let mut source_speaker_by_target: OrderedMap<String, f64> = OrderedMap::default();
    for (target_speaker_key, source_weights) in weights.iter() {
        let mut candidates = source_weights
            .iter()
            .map(|(speaker_key, overlap_ms)| (speaker_key.clone(), *overlap_ms))
            .collect::<Vec<_>>();
        candidates.sort_by(|(left_key, left_weight), (right_key, right_weight)| {
            compare_numbers(*right_weight, *left_weight).then_with(|| left_key.cmp(right_key))
        });
        let total_overlap_ms = candidates.iter().map(|(_, overlap)| overlap).sum::<f64>();
        let Some((source_speaker_key, overlap_ms)) = candidates.first() else {
            continue;
        };
        let Some(source_speaker) = parse_speaker_key(source_speaker_key) else {
            continue;
        };
        if total_overlap_ms > 0.0
            && overlap_ms / total_overlap_ms >= MIN_REFINED_SPEAKER_OVERLAP_RATIO
        {
            source_speaker_by_target.set(target_speaker_key.clone(), source_speaker.speaker_index);
        }
    }

    let mut used_speaker_indices_by_channel: OrderedMap<NumericKey, Vec<f64>> =
        OrderedMap::default();
    for (target_speaker_key, speaker_index) in source_speaker_by_target.iter() {
        let Some(target_speaker) = parse_speaker_key(target_speaker_key) else {
            continue;
        };
        let channel_key = NumericKey::new(target_speaker.channel);
        if let Some(used_speaker_indices) = used_speaker_indices_by_channel.get_mut(&channel_key) {
            insert_set_value(used_speaker_indices, *speaker_index);
        } else {
            used_speaker_indices_by_channel.set(channel_key, vec![*speaker_index]);
        }
    }

    let mut unique_target_keys = Vec::new();
    for (_, speaker_key) in target_speaker_keys.iter() {
        insert_set_value(&mut unique_target_keys, speaker_key.key.clone());
    }
    let mut colliding_target_speakers = Vec::new();
    for target_speaker_key in unique_target_keys {
        if source_speaker_by_target.contains_key(&target_speaker_key) {
            continue;
        }
        let Some(target_speaker) = parse_speaker_key(&target_speaker_key) else {
            continue;
        };
        let channel_key = NumericKey::new(target_speaker.channel);
        let used_speaker_indices = used_speaker_indices_by_channel.get_mut(&channel_key);
        let Some(used_speaker_indices) = used_speaker_indices else {
            used_speaker_indices_by_channel.set(channel_key, vec![target_speaker.speaker_index]);
            continue;
        };
        if used_speaker_indices.contains(&target_speaker.speaker_index) {
            colliding_target_speakers.push((target_speaker_key, target_speaker.channel));
        } else {
            used_speaker_indices.push(target_speaker.speaker_index);
        }
    }

    for (speaker_key, channel) in colliding_target_speakers {
        let Some(used_speaker_indices) =
            used_speaker_indices_by_channel.get_mut(&NumericKey::new(channel))
        else {
            continue;
        };
        let mut speaker_index = used_speaker_indices
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
            + 1.0;
        while used_speaker_indices.contains(&speaker_index) {
            speaker_index += 1.0;
        }
        used_speaker_indices.push(speaker_index);
        source_speaker_by_target.set(speaker_key, speaker_index);
    }

    let reconciled_provider_hints = hints
        .into_iter()
        .map(|mut hint| {
            if hint.hint_type != "provider_speaker_index"
                || hint.word_id.as_deref().is_none_or(str::is_empty)
            {
                return hint;
            }
            let Some(target_speaker_key) =
                target_speaker_keys.get(hint.word_id.as_ref().expect("word IDs were checked"))
            else {
                return hint;
            };
            let Some(speaker_index) = source_speaker_by_target.get(&target_speaker_key.key) else {
                return hint;
            };
            let Some(mut value) = parse_string_hint_value(&hint.value) else {
                return hint;
            };
            if let Value::Object(object) = &mut value {
                object.insert("speaker_index".to_string(), json_number(*speaker_index));
                hint.value = Value::String(value.to_string());
            }
            hint
        })
        .collect();
    reconcile_refined_speaker_assignments(source, words, reconciled_provider_hints)
}

fn reconcile_refined_speaker_assignments(
    source: &BatchRefinementSource,
    words: &[StoredTranscriptWord],
    hints: Vec<StoredSpeakerHint>,
) -> Vec<StoredSpeakerHint> {
    if !source
        .speaker_hints
        .iter()
        .any(|hint| hint.hint_type == "user_speaker_assignment")
    {
        return hints;
    }
    let previous_hints = source
        .speaker_hints
        .iter()
        .filter(|hint| hint.hint_type != "automatic_speaker_assignment")
        .cloned()
        .collect::<Vec<_>>();
    let Some(previous) = build_render_transcript(&source.words, &previous_hints) else {
        return hints;
    };
    let Some(next) = build_render_transcript(words, &hints) else {
        return hints;
    };

    let previous_humans = resolve_scoped_word_human_ids(&previous);
    let next_humans = resolve_scoped_word_human_ids(&next);
    let mut channels = Vec::new();
    for word in &next.words {
        insert_set_value(&mut channels, word.channel);
    }
    let mut word_ids_by_human: OrderedMap<String, Vec<String>> = OrderedMap::default();

    for channel in channels.iter().copied() {
        let mut candidates = previous
            .words
            .iter()
            .filter(|word| {
                has_timing(word)
                    && (word.channel == channel
                        || (channel == 2.0
                            && (word.channel == 0.0 || word.channel == 1.0)
                            && !channels.contains(&word.channel)))
            })
            .cloned()
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| compare_numbers(left.start_ms, right.start_ms));
        let mut targets = next
            .words
            .iter()
            .filter(|word| word.channel == channel && has_timing(word))
            .cloned()
            .collect::<Vec<_>>();
        targets.sort_by(|left, right| compare_numbers(left.start_ms, right.start_ms));
        let mut cursor = 0;
        let mut active = Vec::<RenderWord>::new();

        for word in targets {
            if next_humans.contains_key(&word.id) {
                continue;
            }
            while cursor < candidates.len() && candidates[cursor].start_ms < word.end_ms {
                active.push(candidates[cursor].clone());
                cursor += 1;
            }
            active.retain(|candidate| candidate.end_ms > word.start_ms);
            let mut human_id: Option<String> = None;
            let mut covered_ms = 0.0;
            let mut covered_until = word.start_ms;
            let mut ambiguous = false;

            for candidate in &active {
                let start = word.start_ms.max(candidate.start_ms);
                let end = word.end_ms.min(candidate.end_ms);
                if end <= start {
                    continue;
                }
                let Some(candidate_human) = previous_humans.get(&candidate.id) else {
                    ambiguous = true;
                    break;
                };
                if human_id
                    .as_ref()
                    .is_some_and(|existing| existing != candidate_human)
                {
                    ambiguous = true;
                    break;
                }
                human_id = Some(candidate_human.clone());
                covered_ms += (end - start.max(covered_until)).max(0.0);
                covered_until = covered_until.max(end);
            }

            let Some(human_id) = human_id else {
                continue;
            };
            if ambiguous
                || covered_ms / (word.end_ms - word.start_ms)
                    < MIN_REFINED_ASSIGNMENT_COVERAGE_RATIO
            {
                continue;
            }
            if let Some(word_ids) = word_ids_by_human.get_mut(&human_id) {
                word_ids.push(word.id);
            } else {
                word_ids_by_human.set(human_id, vec![word.id]);
            }
        }
    }

    let mut result = hints;
    for (human_id, word_ids) in word_ids_by_human.entries {
        let Some(first_word_id) = word_ids.first() else {
            continue;
        };
        let value = UserSegmentAssignmentValue {
            human_id: &human_id,
            scope: "segment",
            word_ids: &word_ids,
            extend_to_adjacent: false,
        };
        result.push(StoredSpeakerHint {
            id: format!("{first_word_id}:user_speaker_assignment:segment"),
            word_id: Some(first_word_id.clone()),
            hint_type: "user_speaker_assignment".to_string(),
            value: Value::String(
                serde_json::to_string(&value).expect("segment assignment serializes"),
            ),
        });
    }
    result
}

fn build_render_transcript(
    source_words: &[StoredTranscriptWord],
    hints: &[StoredSpeakerHint],
) -> Option<RenderTranscript> {
    let mut words = Vec::new();
    let mut word_index_by_id: OrderedMap<String, usize> = OrderedMap::default();
    for word in source_words {
        let (Some(text), Some(start_ms), Some(end_ms)) = (&word.text, word.start_ms, word.end_ms)
        else {
            continue;
        };
        word_index_by_id.set(word.id.clone(), words.len());
        words.push(RenderWord {
            id: word.id.clone(),
            text: text.clone(),
            start_ms,
            end_ms,
            channel: word.channel.unwrap_or(0.0),
            speaker_index: None,
        });
    }
    if words.is_empty() {
        return None;
    }

    for hint in hints
        .iter()
        .filter(|hint| hint.hint_type == "provider_speaker_index")
    {
        normalize_speaker_hint(hint, &mut words, &word_index_by_id);
    }
    let mut assignments = Vec::new();
    for hint_type in ["automatic_speaker_assignment", "user_speaker_assignment"] {
        for hint in hints.iter().filter(|hint| hint.hint_type == hint_type) {
            if let Some(assignment) = normalize_speaker_hint(hint, &mut words, &word_index_by_id) {
                assignments.push(assignment);
            }
        }
    }
    Some(RenderTranscript { words, assignments })
}

pub fn render_input_from_stored(
    started_at: Option<i64>,
    words: &[StoredTranscriptWord],
    hints: &[StoredSpeakerHint],
) -> Option<RenderTranscriptInput> {
    let transcript = build_render_transcript(words, hints)?;

    Some(RenderTranscriptInput {
        started_at,
        words: transcript
            .words
            .into_iter()
            .map(|word| RenderTranscriptWordInput {
                id: word.id,
                text: word.text,
                start_ms: word.start_ms.round() as i64,
                end_ms: word.end_ms.round() as i64,
                channel: word.channel.round() as i32,
                speaker_index: word.speaker_index.map(|index| index.round() as i32),
            })
            .collect(),
        assignments: transcript
            .assignments
            .into_iter()
            .map(|assignment| crate::IdentityAssignment {
                human_id: assignment.human_id,
                scope: match assignment.scope {
                    AssignmentScope::Channel(channel) => IdentityScope::Channel { channel },
                    AssignmentScope::ChannelSpeaker {
                        channel,
                        speaker_index,
                    } => IdentityScope::ChannelSpeaker {
                        channel,
                        speaker_index: speaker_index.round() as i32,
                    },
                    AssignmentScope::Words(word_ids) => IdentityScope::Words { word_ids },
                },
            })
            .collect(),
    })
}

fn normalize_speaker_hint(
    hint: &StoredSpeakerHint,
    words: &mut [RenderWord],
    word_index_by_id: &OrderedMap<String, usize>,
) -> Option<IdentityAssignment> {
    let word_id = hint.word_id.as_ref()?;
    let value = parse_hint_value_for_render(&hint.value)?;
    let is_speaker_assignment = matches!(
        hint.hint_type.as_str(),
        "automatic_speaker_assignment" | "user_speaker_assignment"
    );
    let human_id = value
        .get("human_id")
        .and_then(Value::as_str)
        .map(str::to_owned);

    if is_speaker_assignment && let Some(human_id) = human_id.as_ref() {
        if let Some(scope) = explicit_speaker_scope(&value) {
            return Some(IdentityAssignment {
                human_id: human_id.clone(),
                scope,
            });
        }
        if value.get("scope").and_then(Value::as_str) == Some("segment")
            && let Some(word_ids) = value.get("word_ids").and_then(Value::as_array)
        {
            let word_ids = word_ids
                .iter()
                .filter_map(Value::as_str)
                .filter(|word_id| !word_id.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if !word_ids.is_empty() {
                return Some(IdentityAssignment {
                    human_id: human_id.clone(),
                    scope: AssignmentScope::Words(word_ids),
                });
            }
        }
    }

    let word_index = *word_index_by_id.get(word_id)?;
    let word = words.get_mut(word_index)?;
    if hint.hint_type == "provider_speaker_index" {
        if let Some(speaker_index) = value.get("speaker_index").and_then(Value::as_f64) {
            word.speaker_index = Some(speaker_index);
            if let Some(channel) = value.get("channel").and_then(Value::as_f64) {
                word.channel = channel;
            }
        }
        return None;
    }
    if !is_speaker_assignment {
        return None;
    }
    let human_id = human_id?;
    let channel = channel_profile(word.channel);
    let scope = word
        .speaker_index
        .map_or(AssignmentScope::Channel(channel), |speaker_index| {
            AssignmentScope::ChannelSpeaker {
                channel,
                speaker_index,
            }
        });
    Some(IdentityAssignment { human_id, scope })
}

fn explicit_speaker_scope(value: &Value) -> Option<AssignmentScope> {
    if value.get("scope").and_then(Value::as_str) != Some("speaker") {
        return None;
    }
    let channel = value.get("channel").and_then(Value::as_f64)?;
    let channel_profile = match channel {
        0.0 => ChannelProfile::DirectMic,
        1.0 => ChannelProfile::RemoteParty,
        2.0 => ChannelProfile::MixedCapture,
        _ => return None,
    };
    match value.get("speaker_index") {
        Some(Value::Number(speaker_index)) => Some(AssignmentScope::ChannelSpeaker {
            channel: channel_profile,
            speaker_index: speaker_index.as_f64()?,
        }),
        Some(Value::Null) => Some(AssignmentScope::Channel(channel_profile)),
        _ => None,
    }
}

fn resolve_scoped_word_human_ids(transcript: &RenderTranscript) -> OrderedMap<String, String> {
    let mut humans_by_word: OrderedMap<String, String> = OrderedMap::default();
    let mut humans_by_speaker: OrderedMap<String, String> = OrderedMap::default();
    for assignment in &transcript.assignments {
        if assignment.human_id.trim().is_empty() {
            continue;
        }
        match &assignment.scope {
            AssignmentScope::Words(word_ids) => {
                for word_id in word_ids {
                    humans_by_word.set(word_id.clone(), assignment.human_id.clone());
                }
            }
            AssignmentScope::ChannelSpeaker {
                channel,
                speaker_index,
            } => {
                humans_by_speaker.set(
                    format!(
                        "{}:{}",
                        channel_profile_number(*channel),
                        javascript_number_string(*speaker_index)
                    ),
                    assignment.human_id.clone(),
                );
            }
            AssignmentScope::Channel(_) => {}
        }
    }

    let mut result = OrderedMap::default();
    for word in &transcript.words {
        let human_id = humans_by_word.get(&word.id).cloned().or_else(|| {
            word.speaker_index.and_then(|speaker_index| {
                humans_by_speaker
                    .get(&format!(
                        "{}:{}",
                        channel_profile_number(channel_profile(word.channel)),
                        javascript_number_string(speaker_index)
                    ))
                    .cloned()
            })
        });
        if let Some(human_id) = human_id {
            result.set(word.id.clone(), human_id);
        }
    }
    result
}

fn channel_profile(channel: f64) -> ChannelProfile {
    match channel {
        0.0 => ChannelProfile::DirectMic,
        1.0 => ChannelProfile::RemoteParty,
        _ => ChannelProfile::MixedCapture,
    }
}

fn channel_profile_number(channel: ChannelProfile) -> u8 {
    match channel {
        ChannelProfile::DirectMic => 0,
        ChannelProfile::RemoteParty => 1,
        ChannelProfile::MixedCapture => 2,
    }
}

fn has_timing(word: &RenderWord) -> bool {
    word.start_ms.is_finite() && word.end_ms.is_finite() && word.end_ms > word.start_ms
}

fn parse_hint_value_for_render(value: &Value) -> Option<Value> {
    let parsed = match value {
        Value::String(value) => serde_json::from_str(value).ok()?,
        Value::Object(_) | Value::Array(_) => value.clone(),
        _ => return None,
    };
    matches!(&parsed, Value::Object(_) | Value::Array(_)).then_some(parsed)
}

fn parse_string_hint_value(value: &Value) -> Option<Value> {
    let parsed = serde_json::from_str(value.as_str()?).ok()?;
    matches!(&parsed, Value::Object(_) | Value::Array(_)).then_some(parsed)
}

fn integral_number(value: Option<&Value>) -> Option<f64> {
    let number = value?.as_f64()?;
    (number.is_finite() && number.fract() == 0.0).then_some(number)
}

fn parse_speaker_key(key: &str) -> Option<SpeakerKey> {
    let (channel, speaker_index) = key.split_once(':')?;
    let channel = channel.parse::<f64>().ok()?;
    let speaker_index = speaker_index.parse::<f64>().ok()?;
    (channel.is_finite() && speaker_index.is_finite()).then(|| SpeakerKey {
        channel,
        speaker_index,
        key: key.to_string(),
    })
}

fn javascript_number_string(number: f64) -> String {
    if number == 0.0 {
        return "0".to_string();
    }
    if number.fract() == 0.0 && number.abs() < 1.0e21 {
        return format!("{number:.0}");
    }
    let number = number.to_string();
    if let Some((mantissa, exponent)) = number.split_once('e') {
        let exponent = exponent.parse::<i32>().unwrap_or_default();
        return if exponent >= 0 {
            format!("{mantissa}e+{exponent}")
        } else {
            format!("{mantissa}e{exponent}")
        };
    }
    number
}

pub(crate) fn json_number(number: f64) -> Value {
    if number.fract() == 0.0 && number >= i64::MIN as f64 && number <= i64::MAX as f64 {
        return Value::Number(Number::from(number as i64));
    }
    Value::Number(Number::from_f64(number).expect("JSON number is finite"))
}

fn compare_numbers(left: f64, right: f64) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

fn insert_set_value<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(id: &str, start_ms: f64, end_ms: f64, channel: f64) -> StoredTranscriptWord {
        StoredTranscriptWord {
            id: id.to_string(),
            text: Some(id.to_string()),
            start_ms: Some(start_ms),
            end_ms: Some(end_ms),
            channel: Some(channel),
            speaker: None,
            metadata: None,
        }
    }

    fn hint(id: &str, word_id: Option<&str>, hint_type: &str, value: Value) -> StoredSpeakerHint {
        StoredSpeakerHint {
            id: id.to_string(),
            word_id: word_id.map(str::to_owned),
            hint_type: hint_type.to_string(),
            value: Value::String(value.to_string()),
        }
    }

    fn object_hint(
        id: &str,
        word_id: Option<&str>,
        hint_type: &str,
        value: Value,
    ) -> StoredSpeakerHint {
        StoredSpeakerHint {
            id: id.to_string(),
            word_id: word_id.map(str::to_owned),
            hint_type: hint_type.to_string(),
            value,
        }
    }

    fn provider(word: &StoredTranscriptWord, speaker_index: i64) -> StoredSpeakerHint {
        hint(
            &format!("{}:provider_speaker_index", word.id),
            Some(&word.id),
            "provider_speaker_index",
            serde_json::json!({
                "channel": word.channel.unwrap_or(0.0),
                "speaker_index": speaker_index,
            }),
        )
    }

    fn assignment(mut value: Value) -> StoredSpeakerHint {
        let Value::Object(object) = &mut value else {
            panic!("assignment value must be an object");
        };
        object
            .entry("human_id")
            .or_insert_with(|| Value::String("alice".to_string()));
        hint(
            "old-a:user_speaker_assignment",
            Some("old-a"),
            "user_speaker_assignment",
            value,
        )
    }

    fn source(
        words: Vec<StoredTranscriptWord>,
        speaker_hints: Vec<StoredSpeakerHint>,
    ) -> BatchRefinementSource {
        BatchRefinementSource {
            id: "live-transcript".to_string(),
            started_at: 0.0,
            words,
            speaker_hints,
        }
    }

    fn speaker_value(hint: &StoredSpeakerHint) -> Value {
        parse_string_hint_value(&hint.value).expect("speaker hint contains JSON")
    }

    fn user_assignments(
        source_words: Vec<StoredTranscriptWord>,
        source_hints: Vec<StoredSpeakerHint>,
        words: Vec<StoredTranscriptWord>,
        hints: Vec<StoredSpeakerHint>,
    ) -> Vec<Value> {
        let result =
            reconcile_refined_speaker_clusters(&source(source_words, source_hints), &words, hints);
        result
            .into_iter()
            .filter(|hint| hint.hint_type == "user_speaker_assignment")
            .map(|hint| {
                let mut value = speaker_value(&hint);
                let Value::Object(object) = &mut value else {
                    panic!("generated assignment must be an object");
                };
                assert_eq!(
                    object.remove("extend_to_adjacent"),
                    Some(Value::Bool(false))
                );
                object.insert(
                    "word_id".to_string(),
                    hint.word_id.map_or(Value::Null, Value::String),
                );
                value
            })
            .collect()
    }

    fn full_speaker() -> StoredSpeakerHint {
        assignment(serde_json::json!({
            "scope": "speaker",
            "channel": 1,
            "speaker_index": 0,
        }))
    }

    #[test]
    fn render_input_from_stored_applies_provider_and_user_speaker_hints() {
        let mut stored_word = word("word-1", 10.4, 20.6, 0.0);
        stored_word.text = Some("hello".to_string());
        let mut untimed_word = word("untimed", 30.0, 40.0, 0.0);
        untimed_word.start_ms = None;
        let rendered = render_input_from_stored(
            Some(123),
            &[stored_word, untimed_word],
            &[
                hint(
                    "word-1:provider_speaker_index",
                    Some("word-1"),
                    "provider_speaker_index",
                    serde_json::json!({"channel": 1, "speaker_index": 2.6}),
                ),
                hint(
                    "word-1:user_speaker_assignment",
                    Some("word-1"),
                    "user_speaker_assignment",
                    serde_json::json!({"human_id": "alice"}),
                ),
            ],
        )
        .unwrap();

        assert_eq!(rendered.started_at, Some(123));
        assert_eq!(rendered.words.len(), 1);
        assert_eq!(rendered.words[0].id, "word-1");
        assert_eq!(rendered.words[0].text, "hello");
        assert_eq!(rendered.words[0].start_ms, 10);
        assert_eq!(rendered.words[0].end_ms, 21);
        assert_eq!(rendered.words[0].channel, 1);
        assert_eq!(rendered.words[0].speaker_index, Some(3));
        assert_eq!(
            rendered.assignments,
            vec![crate::IdentityAssignment {
                human_id: "alice".to_string(),
                scope: IdentityScope::ChannelSpeaker {
                    channel: ChannelProfile::RemoteParty,
                    speaker_index: 3,
                },
            }]
        );
    }

    fn current_request(
        words: Vec<StoredTranscriptWord>,
        hints: Vec<StoredSpeakerHint>,
        previous_transcripts: Vec<BatchRefinementSource>,
        audio_offset_ms: f64,
        replace_transcript_id: Option<&str>,
    ) -> BatchRefinementRequest {
        BatchRefinementRequest {
            words,
            hints,
            promotion: BatchTranscriptPromotion::CurrentCapture {
                audio_offset_ms,
                replace_transcript_id: replace_transcript_id.map(str::to_owned),
                started_at: 123_000.0,
            },
            previous_transcripts,
        }
    }

    fn ready(
        outcome: BatchRefinementOutcome,
    ) -> (
        Vec<StoredTranscriptWord>,
        Vec<StoredSpeakerHint>,
        bool,
        Option<String>,
        Option<f64>,
    ) {
        let BatchRefinementOutcome::Ready {
            words,
            speaker_hints,
            replace_session,
            replace_transcript_id,
            started_at,
        } = outcome
        else {
            panic!("expected ready outcome");
        };
        (
            words,
            speaker_hints,
            replace_session,
            replace_transcript_id,
            started_at,
        )
    }

    #[test]
    fn collapses_split_batch_clusters_onto_dominant_live_clusters() {
        let source_words = vec![
            word("live-lex-1", 0.0, 100.0, 1.0),
            word("live-george-1", 100.0, 200.0, 1.0),
            word("live-lex-2", 200.0, 300.0, 1.0),
            word("live-george-2", 300.0, 400.0, 1.0),
        ];
        let source_hints = source_words
            .iter()
            .enumerate()
            .map(|(index, word)| provider(word, if index % 2 == 0 { 0 } else { 1 }))
            .collect();
        let words = vec![
            word("batch-lex-primary", 0.0, 100.0, 1.0),
            word("batch-george-primary", 100.0, 200.0, 1.0),
            word("batch-lex-split", 200.0, 300.0, 1.0),
            word("batch-george-split", 300.0, 400.0, 1.0),
        ];
        let hints = words
            .iter()
            .enumerate()
            .map(|(index, word)| provider(word, index as i64))
            .collect::<Vec<_>>();

        let result =
            reconcile_refined_speaker_clusters(&source(source_words, source_hints), &words, hints);

        assert_eq!(
            result
                .iter()
                .map(|hint| speaker_value(hint)["speaker_index"]
                    .as_i64()
                    .expect("speaker index is an integer"))
                .collect::<Vec<_>>(),
            [0, 1, 0, 1]
        );
    }

    #[test]
    fn reanchors_manual_names_to_validated_replacement_words_on_channels_one_and_two() {
        for channel in [1.0, 2.0] {
            let source_word = word("old-a", 0.0, 100.0, 1.0);
            let next_word = word("new-a", 0.0, 100.0, channel);
            assert_eq!(
                user_assignments(
                    vec![source_word.clone()],
                    vec![provider(&source_word, 0), full_speaker()],
                    vec![next_word.clone()],
                    vec![provider(&next_word, 7)],
                ),
                [serde_json::json!({
                    "word_id": "new-a",
                    "human_id": "alice",
                    "scope": "segment",
                    "word_ids": ["new-a"],
                })]
            );
        }
    }

    #[test]
    fn does_not_carry_manual_names_without_validated_overlap_or_coverage() {
        let source_word = word("old-a", 0.0, 100.0, 1.0);
        let next_word = word("new-a", 200.0, 300.0, 1.0);
        assert!(
            user_assignments(
                vec![source_word.clone()],
                vec![provider(&source_word, 0), full_speaker()],
                vec![next_word.clone()],
                vec![provider(&next_word, 0)],
            )
            .is_empty()
        );

        let source_words = vec![
            word("old-a", 0.0, 100.0, 1.0),
            word("old-b", 100.0, 200.0, 1.0),
        ];
        let next_word = word("new-ab", 0.0, 200.0, 1.0);
        assert!(
            user_assignments(
                source_words.clone(),
                vec![
                    provider(&source_words[0], 0),
                    provider(&source_words[1], 1),
                    full_speaker(),
                ],
                vec![next_word.clone()],
                vec![provider(&next_word, 0)],
            )
            .is_empty()
        );

        let source_word = word("old-a", 0.0, 100.0, 1.0);
        let next_word = word("new-a", 0.0, 1_000.0, 1.0);
        assert!(
            user_assignments(
                vec![source_word.clone()],
                vec![provider(&source_word, 0), full_speaker()],
                vec![next_word.clone()],
                vec![provider(&next_word, 0)],
            )
            .is_empty()
        );
    }

    #[test]
    fn downmixing_does_not_merge_simultaneous_speakers_but_keeps_stereo_channels_separate() {
        let source_words = vec![word("old-a", 0.0, 100.0, 1.0), word("mic", 0.0, 100.0, 0.0)];
        let source_hints = vec![
            provider(&source_words[0], 0),
            provider(&source_words[1], 0),
            full_speaker(),
        ];
        let mixed = word("mixed", 0.0, 100.0, 2.0);
        assert!(
            user_assignments(
                source_words.clone(),
                source_hints.clone(),
                vec![mixed.clone()],
                vec![provider(&mixed, 0)],
            )
            .is_empty()
        );

        let remote = word("remote", 0.0, 100.0, 1.0);
        assert_eq!(
            user_assignments(
                source_words,
                source_hints,
                vec![remote.clone()],
                vec![provider(&remote, 0)],
            ),
            [serde_json::json!({
                "word_id": "remote",
                "human_id": "alice",
                "scope": "segment",
                "word_ids": ["remote"],
            })]
        );
    }

    #[test]
    fn remaps_segment_overrides_to_replacement_word_ids_without_extending_them() {
        let source_words = vec![
            word("old-a", 0.0, 100.0, 1.0),
            word("old-b", 100.0, 200.0, 1.0),
        ];
        let next_words = vec![
            word("new-a", 0.0, 50.0, 1.0),
            word("new-b", 50.0, 100.0, 1.0),
            word("new-c", 100.0, 200.0, 1.0),
        ];
        assert_eq!(
            user_assignments(
                source_words,
                vec![assignment(serde_json::json!({
                    "scope": "segment",
                    "word_ids": ["old-a"],
                }))],
                next_words,
                vec![],
            ),
            [serde_json::json!({
                "word_id": "new-a",
                "human_id": "alice",
                "scope": "segment",
                "word_ids": ["new-a", "new-b"],
            })]
        );
    }

    #[test]
    fn keeps_segment_overrides_ahead_of_full_speaker_names() {
        let source_words = vec![
            word("old-a", 0.0, 100.0, 1.0),
            word("old-b", 100.0, 200.0, 1.0),
        ];
        let next_words = vec![
            word("new-a", 0.0, 100.0, 1.0),
            word("new-b", 100.0, 200.0, 1.0),
        ];
        let mut hints = source_words
            .iter()
            .map(|word| provider(word, 0))
            .collect::<Vec<_>>();
        hints.push(assignment(serde_json::json!({
            "human_id": "bob",
            "scope": "segment",
            "word_ids": ["old-b"],
        })));
        hints.push(full_speaker());
        assert_eq!(
            user_assignments(
                source_words,
                hints,
                next_words.clone(),
                next_words.iter().map(|word| provider(word, 0)).collect(),
            ),
            [
                serde_json::json!({
                    "word_id": "new-a",
                    "human_id": "alice",
                    "scope": "segment",
                    "word_ids": ["new-a"],
                }),
                serde_json::json!({
                    "word_id": "new-b",
                    "human_id": "bob",
                    "scope": "segment",
                    "word_ids": ["new-b"],
                }),
            ]
        );
    }

    #[test]
    fn ignores_legacy_mixed_channel_identity_hints() {
        let source_word = word("old-a", 0.0, 100.0, 2.0);
        let next_word = word("new-a", 0.0, 100.0, 2.0);
        assert!(
            user_assignments(
                vec![source_word],
                vec![assignment(serde_json::json!({}))],
                vec![next_word],
                vec![],
            )
            .is_empty()
        );
    }

    #[test]
    fn does_not_carry_legacy_inferred_identities_into_refined_captures() {
        let mut source_word = word("live-word", 100.0, 500.0, 1.0);
        source_word.text = Some("answer".to_string());
        let mut replacement_word = word("batch-word", 60_100.0, 60_500.0, 1.0);
        replacement_word.text = Some("answer".to_string());
        let source = BatchRefinementSource {
            id: "live-current".to_string(),
            started_at: 99_000.0,
            words: vec![source_word.clone()],
            speaker_hints: vec![
                provider(&source_word, 0),
                hint(
                    "live-auto-assignment",
                    Some("live-word"),
                    "automatic_speaker_assignment",
                    serde_json::json!({
                        "human_id": "human-1",
                        "confidence": 0.93,
                        "source": "enhance",
                    }),
                ),
            ],
        };

        let (_, speaker_hints, _, _, _) = ready(refine_batch_transcript(current_request(
            vec![replacement_word.clone()],
            vec![provider(&replacement_word, 3)],
            vec![source],
            60_000.0,
            Some("live-current"),
        )));

        assert_eq!(speaker_hints.len(), 1);
        assert_eq!(speaker_hints[0].hint_type, "provider_speaker_index");
        assert_eq!(speaker_value(&speaker_hints[0])["speaker_index"], 0);
    }

    #[test]
    fn keeps_ambiguous_batch_clusters_unchanged() {
        let source_words = vec![
            word("live-a", 0.0, 100.0, 1.0),
            word("live-b", 100.0, 200.0, 1.0),
        ];
        let source_hints = vec![provider(&source_words[0], 0), provider(&source_words[1], 1)];
        let words = vec![word("batch-ambiguous", 0.0, 200.0, 1.0)];
        let hints = vec![provider(&words[0], 4)];
        let result =
            reconcile_refined_speaker_clusters(&source(source_words, source_hints), &words, hints);
        assert_eq!(speaker_value(&result[0])["speaker_index"], 4);
    }

    #[test]
    fn moves_an_unmapped_batch_cluster_that_collides_with_a_live_cluster() {
        let source_word = word("live-speaker", 0.0, 100.0, 1.0);
        let words = vec![
            word("batch-mapped", 0.0, 100.0, 1.0),
            word("batch-unmapped", 200.0, 300.0, 1.0),
        ];
        let result = reconcile_refined_speaker_clusters(
            &source(vec![source_word.clone()], vec![provider(&source_word, 1)]),
            &words,
            vec![provider(&words[0], 0), provider(&words[1], 1)],
        );
        assert_eq!(
            result
                .iter()
                .map(|hint| speaker_value(hint)["speaker_index"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            [1, 2]
        );
    }

    #[test]
    fn assigns_colliding_batch_clusters_in_target_hint_insertion_order() {
        let source_words = vec![
            word("live-one", 0.0, 100.0, 1.0),
            word("live-two", 100.0, 200.0, 1.0),
        ];
        let words = vec![
            word("mapped-one", 0.0, 100.0, 1.0),
            word("mapped-two", 100.0, 200.0, 1.0),
            word("collision-one", 300.0, 400.0, 1.0),
            word("collision-two", 400.0, 500.0, 1.0),
        ];
        let hints = vec![
            provider(&words[0], 0),
            provider(&words[1], 3),
            provider(&words[3], 2),
            provider(&words[2], 1),
        ];
        let result = reconcile_refined_speaker_clusters(
            &source(
                source_words.clone(),
                vec![provider(&source_words[0], 1), provider(&source_words[1], 2)],
            ),
            &words,
            hints,
        );
        assert_eq!(
            result
                .iter()
                .map(|hint| speaker_value(hint)["speaker_index"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
    }

    #[test]
    fn restores_original_channels_after_downmixing() {
        let previous = vec![
            word("a", 0.0, 500.0, 0.0),
            StoredTranscriptWord {
                text: Some("안녕".to_string()),
                ..word("b", 500.0, 1_000.0, 1.0)
            },
        ];
        let replacement = previous
            .iter()
            .map(|word| StoredTranscriptWord {
                id: format!("new-{}", word.id),
                channel: Some(2.0),
                ..word.clone()
            })
            .collect::<Vec<_>>();

        let result = restore_refined_source_channels(&previous, replacement);

        assert_eq!(
            result.iter().map(|word| word.channel).collect::<Vec<_>>(),
            [Some(0.0), Some(1.0)]
        );
    }

    #[test]
    fn keeps_overlapping_or_rewritten_speech_mixed() {
        let local = word("a", 0.0, 500.0, 0.0);
        let remote = word("b", 0.0, 500.0, 1.0);
        let mixed = word("new", 0.0, 500.0, 2.0);
        assert_eq!(
            restore_refined_source_channels(&[local.clone(), remote], vec![mixed.clone()])[0]
                .channel,
            Some(2.0)
        );

        let rewritten = StoredTranscriptWord {
            text: Some("goodbye".to_string()),
            ..mixed.clone()
        };
        assert_eq!(
            restore_refined_source_channels(std::slice::from_ref(&local), vec![rewritten])[0]
                .channel,
            Some(2.0)
        );

        let insufficient_overlap = StoredTranscriptWord {
            start_ms: Some(490.0),
            end_ms: Some(990.0),
            ..mixed
        };
        assert_eq!(
            restore_refined_source_channels(&[local], vec![insufficient_overlap])[0].channel,
            Some(2.0)
        );
    }

    #[test]
    fn restores_source_channels_in_provider_hints() {
        let hints = vec![hint(
            "hint",
            Some("a"),
            "provider_speaker_index",
            serde_json::json!({"channel": 2, "speaker_index": 7}),
        )];
        let restored = restore_refined_source_hints(&[word("a", 0.0, 500.0, 0.0)], hints.clone());
        assert_eq!(speaker_value(&restored[0])["channel"], 0);
        assert_eq!(speaker_value(&restored[0])["speaker_index"], 7);
        assert_eq!(
            restore_refined_source_hints(&[word("a", 0.0, 500.0, 2.0)], hints.clone()),
            hints
        );
    }

    #[test]
    fn batch_cluster_keys_ignore_object_values_and_non_integral_numbers() {
        let source_word = word("source", 0.0, 100.0, 1.0);
        let target_word = word("target", 0.0, 100.0, 1.0);
        let source_hints = vec![object_hint(
            "source-provider",
            Some("source"),
            "provider_speaker_index",
            serde_json::json!({"channel": 1, "speaker_index": 1}),
        )];
        let target_hint = object_hint(
            "target-provider",
            Some("target"),
            "provider_speaker_index",
            serde_json::json!({"channel": 1, "speaker_index": 0}),
        );
        let result = reconcile_refined_speaker_clusters(
            &source(vec![source_word], source_hints),
            &[target_word],
            vec![target_hint.clone()],
        );
        assert_eq!(result, [target_hint]);

        let source_hint = hint(
            "source-provider",
            Some("source"),
            "provider_speaker_index",
            serde_json::json!({"channel": 1.5, "speaker_index": 1}),
        );
        let target_hint = hint(
            "target-provider",
            Some("target"),
            "provider_speaker_index",
            serde_json::json!({"channel": 1.5, "speaker_index": 0}),
        );
        let result = reconcile_refined_speaker_clusters(
            &source(vec![word("source", 0.0, 100.0, 1.0)], vec![source_hint]),
            &[word("target", 0.0, 100.0, 1.0)],
            vec![target_hint.clone()],
        );
        assert_eq!(result, [target_hint]);

        let object_hint = object_hint(
            "object-value",
            Some("target"),
            "provider_speaker_index",
            serde_json::json!({"channel": 2, "speaker_index": 7}),
        );
        assert_eq!(
            restore_refined_source_hints(
                &[word("target", 0.0, 100.0, 0.0)],
                vec![object_hint.clone()]
            ),
            [object_hint]
        );
    }

    #[test]
    fn render_identity_restoration_accepts_object_valued_hints() {
        let source_word = word("old", 0.0, 100.0, 1.0);
        let next_word = word("new", 0.0, 100.0, 1.0);
        let source_hints = vec![
            object_hint(
                "provider",
                Some("old"),
                "provider_speaker_index",
                serde_json::json!({"channel": 1, "speaker_index": 0}),
            ),
            object_hint(
                "assignment",
                Some("old"),
                "user_speaker_assignment",
                serde_json::json!({
                    "human_id": "alice",
                    "scope": "speaker",
                    "channel": 1,
                    "speaker_index": 0,
                }),
            ),
        ];
        let result = reconcile_refined_speaker_clusters(
            &source(vec![source_word.clone()], source_hints),
            std::slice::from_ref(&next_word),
            vec![object_hint(
                "next-provider",
                Some("new"),
                "provider_speaker_index",
                serde_json::json!({"channel": 1, "speaker_index": 7}),
            )],
        );
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].word_id.as_deref(), Some("new"));
        assert_eq!(
            speaker_value(&result[1])["word_ids"],
            serde_json::json!(["new"])
        );
    }

    #[test]
    fn preserves_user_hint_insertion_order_when_reanchoring_assignments() {
        let source_words = vec![
            word("old-local", 0.0, 100.0, 0.0),
            word("old-remote", 0.0, 100.0, 1.0),
        ];
        let source_hints = vec![
            provider(&source_words[0], 0),
            provider(&source_words[1], 0),
            assignment(serde_json::json!({
                "human_id": "bob",
                "scope": "speaker",
                "channel": 0,
                "speaker_index": 0,
            })),
            hint(
                "remote-assignment",
                Some("old-remote"),
                "user_speaker_assignment",
                serde_json::json!({
                    "human_id": "alice",
                    "scope": "speaker",
                    "channel": 1,
                    "speaker_index": 0,
                }),
            ),
        ];
        let words = vec![
            word("new-local", 0.0, 100.0, 0.0),
            word("new-remote", 0.0, 100.0, 1.0),
        ];
        let hints = words
            .iter()
            .map(|word| provider(word, 0))
            .collect::<Vec<_>>();
        let result =
            reconcile_refined_speaker_clusters(&source(source_words, source_hints), &words, hints);
        assert_eq!(
            result
                .iter()
                .filter(|hint| hint.hint_type == "user_speaker_assignment")
                .map(|hint| hint.word_id.as_deref().unwrap())
                .collect::<Vec<_>>(),
            ["new-local", "new-remote"]
        );
    }

    #[test]
    fn preserves_generated_segment_assignment_fields() {
        let old_word = word("old", 0.0, 100.0, 1.0);
        let new_word = word("new", 0.0, 100.0, 1.0);
        let result = reconcile_refined_speaker_clusters(
            &source(
                vec![old_word],
                vec![assignment(serde_json::json!({
                    "scope": "segment",
                    "word_ids": ["old"],
                }))],
            ),
            &[new_word],
            vec![],
        );
        let generated = result
            .iter()
            .find(|hint| hint.hint_type == "user_speaker_assignment")
            .expect("segment assignment was restored");
        assert_eq!(generated.id, "new:user_speaker_assignment:segment");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(generated.value.as_str().unwrap()).unwrap(),
            serde_json::json!({
                "human_id": "alice",
                "scope": "segment",
                "word_ids": ["new"],
                "extend_to_adjacent": false,
            })
        );
    }

    #[test]
    fn promotes_current_capture_words_and_filters_hints() {
        let words = vec![
            word("old", 10_000.0, 10_500.0, 0.0),
            word("new", 60_100.0, 60_500.0, 0.0),
        ];
        let hints = vec![provider(&words[0], 0), provider(&words[1], 1)];
        let previous = BatchRefinementSource {
            id: "live-current".to_string(),
            started_at: 99_000.0,
            words: vec![word("live-word", 0.0, 500.0, 1.0)],
            speaker_hints: vec![],
        };
        let (words, hints, replace_session, replace_transcript_id, started_at) =
            ready(refine_batch_transcript(current_request(
                words,
                hints,
                vec![previous],
                60_000.0,
                Some("live-current"),
            )));
        assert_eq!(
            words,
            [StoredTranscriptWord {
                start_ms: Some(100.0),
                end_ms: Some(500.0),
                ..word("new", 60_100.0, 60_500.0, 0.0)
            }]
        );
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].word_id.as_deref(), Some("new"));
        assert!(!replace_session);
        assert_eq!(replace_transcript_id.as_deref(), Some("live-current"));
        assert_eq!(started_at, Some(123_000.0));
    }

    #[test]
    fn current_capture_rebases_synthetic_chunks_with_word_times_before_rendering() {
        for encoded in [false, true] {
            let mut synthetic = word("synthetic-mic", 61_000.0, 61_400.0, 0.0);
            let metadata = serde_json::json!({
                "timing": {"source": "synthetic_text", "chunk_start_ms": 60_000},
                "retained": "context",
            });
            synthetic.metadata = Some(if encoded {
                Value::String(metadata.to_string())
            } else {
                metadata
            });
            let (words, hints, _, _, _) = ready(refine_batch_transcript(current_request(
                vec![synthetic, word("timed-remote", 62_000.0, 62_400.0, 1.0)],
                vec![],
                vec![],
                60_000.0,
                None,
            )));
            let metadata = match words[0].metadata.as_ref().unwrap() {
                Value::String(json) => serde_json::from_str::<Value>(json).unwrap(),
                metadata => metadata.clone(),
            };
            assert_eq!(metadata["timing"]["chunk_start_ms"].as_f64(), Some(0.0));
            assert_eq!(metadata["retained"], "context");
            let segments = crate::render_stored_transcript_segments(
                vec![(Some(123_000), words, hints)],
                crate::RenderTranscriptRequest {
                    transcripts: vec![],
                    participant_human_ids: vec![],
                    self_human_id: None,
                    humans: vec![],
                    speaker_context: None,
                    preview: None,
                },
            )
            .unwrap();
            assert_eq!(segments[0].words[0].id.as_deref(), Some("synthetic-mic"));
            assert_eq!(segments[1].words[0].id.as_deref(), Some("timed-remote"));
            assert_eq!(segments[0].start_ms, 1_000);
            assert_eq!(segments[1].start_ms, 2_000);
        }
        let mut metadata =
            serde_json::json!({"timing": {"source": "synthetic_text", "chunk_start_ms": 59_500}});
        rebase_synthetic_chunk_start(&mut metadata, 60_000.0);
        assert_eq!(metadata["timing"]["chunk_start_ms"].as_f64(), Some(0.0));
    }

    #[test]
    fn returns_empty_current_capture_when_promotion_filters_every_word() {
        let outcome = refine_batch_transcript(current_request(
            vec![word("old", 10_000.0, 10_500.0, 0.0)],
            vec![],
            vec![],
            60_000.0,
            Some("live-current"),
        ));
        assert_eq!(outcome, BatchRefinementOutcome::EmptyCurrentCapture);
    }

    #[test]
    fn defaults_missing_end_time_to_start_and_clamps_negative_offsets() {
        let mut word_without_end = word("missing-end", 60.0, 0.0, 0.0);
        word_without_end.end_ms = None;
        let (words, _, _, _, _) = ready(refine_batch_transcript(current_request(
            vec![word_without_end],
            vec![],
            vec![],
            50.0,
            None,
        )));
        assert_eq!(words[0].start_ms, Some(10.0));
        assert_eq!(words[0].end_ms, Some(10.0));

        let (words, _, _, _, _) = ready(refine_batch_transcript(current_request(
            vec![word("negative-offset", 10.0, 20.0, 0.0)],
            vec![],
            vec![],
            -20.0,
            None,
        )));
        assert_eq!(words[0].start_ms, Some(10.0));
        assert_eq!(words[0].end_ms, Some(20.0));
    }

    #[test]
    fn rejects_replacements_that_cross_both_truncation_thresholds() {
        let previous = BatchRefinementSource {
            id: "saved".to_string(),
            started_at: 10.0,
            words: vec![word("old", 0.0, 100.0, 0.0); 500]
                .into_iter()
                .map(|mut word| {
                    word.text = Some("a".to_string());
                    word
                })
                .collect(),
            speaker_hints: vec![],
        };
        let outcome = refine_batch_transcript(BatchRefinementRequest {
            words: vec![StoredTranscriptWord {
                text: Some("a".repeat(200)),
                ..word("replacement", 0.0, 100.0, 0.0)
            }],
            hints: vec![],
            promotion: BatchTranscriptPromotion::WholeSession,
            previous_transcripts: vec![previous],
        });
        assert_eq!(outcome, BatchRefinementOutcome::Truncated);
    }

    #[test]
    fn counts_astral_unicode_letters_as_javascript_utf16_units() {
        let previous = BatchRefinementSource {
            id: "saved".to_string(),
            started_at: 10.0,
            words: vec![StoredTranscriptWord {
                text: Some("𝒜".repeat(200)),
                ..word("old", 0.0, 100.0, 0.0)
            }],
            speaker_hints: vec![],
        };
        let outcome = refine_batch_transcript(BatchRefinementRequest {
            words: vec![StoredTranscriptWord {
                text: Some("𝒜".repeat(99)),
                ..word("replacement", 0.0, 100.0, 0.0)
            }],
            hints: vec![],
            promotion: BatchTranscriptPromotion::WholeSession,
            previous_transcripts: vec![previous],
        });
        assert_eq!(outcome, BatchRefinementOutcome::Truncated);
    }

    #[test]
    fn tolerates_tokenization_languages_small_captures_and_ordinary_corrections() {
        let examples = [
            ("meeting ".repeat(100), "meeting ".repeat(100)),
            ("会议记录".repeat(100), "会议记录".repeat(100)),
            ("meeting ".repeat(100), "meeting ".repeat(70)),
            ("meeting ".repeat(10), "hello".to_string()),
        ];
        for (previous_text, replacement_text) in examples {
            let previous = BatchRefinementSource {
                id: "saved".to_string(),
                started_at: 10.0,
                words: vec![StoredTranscriptWord {
                    text: Some(previous_text),
                    ..word("old", 0.0, 100.0, 0.0)
                }],
                speaker_hints: vec![],
            };
            let outcome = refine_batch_transcript(BatchRefinementRequest {
                words: vec![StoredTranscriptWord {
                    text: Some(replacement_text),
                    ..word("new", 0.0, 100.0, 0.0)
                }],
                hints: vec![],
                promotion: BatchTranscriptPromotion::WholeSession,
                previous_transcripts: vec![previous],
            });
            assert!(matches!(outcome, BatchRefinementOutcome::Ready { .. }));
        }
    }

    #[test]
    fn truncation_check_uses_only_the_current_capture_after_offset_filtering() {
        let previous_text = "meeting ".repeat(100);
        let replacement_text = "meeting ".repeat(70);
        let previous = BatchRefinementSource {
            id: "live-current".to_string(),
            started_at: 123_000.0,
            words: vec![StoredTranscriptWord {
                text: Some(previous_text),
                ..word("live-word", 0.0, 100.0, 0.0)
            }],
            speaker_hints: vec![],
        };
        let outcome = refine_batch_transcript(current_request(
            vec![
                StoredTranscriptWord {
                    text: Some("An earlier capture ".repeat(1_000)),
                    ..word("earlier", 0.0, 59_000.0, 0.0)
                },
                StoredTranscriptWord {
                    text: Some(replacement_text),
                    ..word("replacement", 60_000.0, 61_000.0, 0.0)
                },
            ],
            vec![],
            vec![previous],
            60_000.0,
            Some("live-current"),
        ));
        assert!(matches!(outcome, BatchRefinementOutcome::Ready { .. }));
    }

    #[test]
    fn uses_the_sole_previous_transcript_start_time_without_promotion_time() {
        let outcome = refine_batch_transcript(BatchRefinementRequest {
            words: vec![word("new", 0.0, 100.0, 0.0)],
            hints: vec![],
            promotion: BatchTranscriptPromotion::WholeSession,
            previous_transcripts: vec![BatchRefinementSource {
                id: "previous".to_string(),
                started_at: 50.0,
                words: vec![],
                speaker_hints: vec![],
            }],
        });
        let (_, _, replace_session, replace_transcript_id, started_at) = ready(outcome);
        assert!(replace_session);
        assert_eq!(replace_transcript_id, None);
        assert_eq!(started_at, Some(50.0));
    }
}
