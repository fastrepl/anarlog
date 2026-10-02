use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use serde::Serialize;
use serde_json::Value;

use crate::{FinalizedWord, StoredSpeakerHint, StoredTranscriptWord, batch_refine::json_number};

const MAX_SEGMENT_GAP_MS: f64 = 3000.0;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct StoredLiveTranscriptDelta {
    #[serde(default)]
    pub new_words: Vec<FinalizedWord>,
    #[serde(default)]
    pub replaced_ids: Vec<String>,
}

#[derive(Default)]
struct PendingWrite {
    words_by_id: Vec<PendingWordGroup>,
    replaced_root_ids: Vec<String>,
}

struct PendingWordGroup {
    id: String,
    words: Vec<FinalizedWord>,
    may_exist_in_persisted_base: bool,
}

pub fn coalesce_live_transcript_deltas(
    deltas: &[StoredLiveTranscriptDelta],
) -> StoredLiveTranscriptDelta {
    let mut pending = PendingWrite::default();
    for delta in deltas {
        merge_delta(&mut pending, delta);
    }

    StoredLiveTranscriptDelta {
        new_words: pending
            .words_by_id
            .into_iter()
            .flat_map(|group| group.words)
            .collect(),
        replaced_ids: pending.replaced_root_ids,
    }
}

fn merge_delta(pending: &mut PendingWrite, delta: &StoredLiveTranscriptDelta) {
    for replaced_id in &delta.replaced_ids {
        if let Some(replaced) = remove_pending_words(pending, replaced_id) {
            if replaced.may_exist_in_persisted_base {
                insert_set_value(&mut pending.replaced_root_ids, replaced_id.clone());
            }
        } else {
            insert_set_value(&mut pending.replaced_root_ids, replaced_id.clone());
        }
    }

    let mut next_words_by_id: Vec<(String, Vec<FinalizedWord>)> = Vec::new();
    for word in &delta.new_words {
        if let Some((_, words)) = next_words_by_id.iter_mut().find(|(id, _)| id == &word.id) {
            words.push(word.clone());
        } else {
            next_words_by_id.push((word.id.clone(), vec![word.clone()]));
        }
    }

    for (word_id, words) in next_words_by_id {
        let existing = remove_pending_words(pending, &word_id);
        let may_exist_in_persisted_base = existing
            .map(|group| group.may_exist_in_persisted_base)
            .unwrap_or(delta.replaced_ids.is_empty());
        pending.words_by_id.push(PendingWordGroup {
            id: word_id,
            words,
            may_exist_in_persisted_base,
        });
    }
}

fn remove_pending_words(pending: &mut PendingWrite, word_id: &str) -> Option<PendingWordGroup> {
    pending
        .words_by_id
        .iter()
        .position(|group| group.id == word_id)
        .map(|index| pending.words_by_id.remove(index))
}

fn insert_set_value(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

pub fn apply_live_transcript_delta(
    mut words: Vec<StoredTranscriptWord>,
    hints: Vec<StoredSpeakerHint>,
    delta: &StoredLiveTranscriptDelta,
) -> (Vec<StoredTranscriptWord>, Vec<StoredSpeakerHint>) {
    let previous_words = words.clone();
    let replaced_ids: HashSet<&str> = delta.replaced_ids.iter().map(String::as_str).collect();
    let new_words: Vec<StoredTranscriptWord> = delta
        .new_words
        .iter()
        .map(|word| StoredTranscriptWord {
            id: word.id.clone(),
            text: Some(word.text.clone()),
            start_ms: Some(word.start_ms as f64),
            end_ms: Some(word.end_ms as f64),
            channel: Some(word.channel as f64),
            speaker: None,
            metadata: None,
        })
        .collect();
    let new_word_ids: HashSet<String> = new_words.iter().map(|word| word.id.clone()).collect();

    words.retain(|word| {
        !replaced_ids.contains(word.id.as_str()) && !new_word_ids.contains(word.id.as_str())
    });
    words.extend(new_words);
    words.sort_by(|left, right| {
        compare_numbers(
            left.start_ms.unwrap_or_default(),
            right.start_ms.unwrap_or_default(),
        )
    });

    let mut next_hints = Vec::new();
    for hint in &hints {
        for reconciled_hint in reconcile_segment_speaker_assignment_hint(
            hint,
            &replaced_ids,
            &previous_words,
            &words,
            &hints,
            &delta.new_words,
        ) {
            if is_segment_speaker_assignment_hint(&reconciled_hint)
                || is_speaker_scoped_assignment_hint(&reconciled_hint)
            {
                next_hints.push(reconciled_hint);
                continue;
            }

            let word_id = reconciled_hint.word_id.as_deref().unwrap_or_default();
            if !replaced_ids.contains(word_id) && !new_word_ids.contains(word_id) {
                next_hints.push(reconciled_hint);
            }
        }
    }

    for word in &delta.new_words {
        if let Some(hint) = to_storage_speaker_hint(word) {
            next_hints.push(hint);
        }
    }
    next_hints.sort_by(|left, right| {
        left.word_id
            .as_deref()
            .unwrap_or_default()
            .cmp(right.word_id.as_deref().unwrap_or_default())
    });

    (words, next_hints)
}

pub fn materialize_live_transcript(
    words: Vec<StoredTranscriptWord>,
    hints: Vec<StoredSpeakerHint>,
    deltas: &[StoredLiveTranscriptDelta],
) -> (Vec<StoredTranscriptWord>, Vec<StoredSpeakerHint>) {
    if deltas.is_empty() {
        return (words, hints);
    }

    let delta = coalesce_live_transcript_deltas(deltas);
    apply_live_transcript_delta(words, hints, &delta)
}

#[derive(Clone, Copy)]
struct SpeakerSegmentKey {
    channel: f64,
    speaker_index: Option<f64>,
}

#[derive(Clone, Copy)]
struct WordRange {
    start_ms: f64,
    end_ms: f64,
}

fn reconcile_segment_speaker_assignment_hint(
    hint: &StoredSpeakerHint,
    replaced_ids: &HashSet<&str>,
    previous_words: &[StoredTranscriptWord],
    next_words: &[StoredTranscriptWord],
    hints: &[StoredSpeakerHint],
    new_final_words: &[FinalizedWord],
) -> Vec<StoredSpeakerHint> {
    let Some((mut assignment_value, segment_word_ids)) = get_segment_speaker_assignment(hint)
    else {
        return vec![hint.clone()];
    };

    let extend_to_adjacent = assignment_value
        .get("extend_to_adjacent")
        .and_then(Value::as_bool)
        != Some(false);
    let next_word_ids = get_reconciled_segment_word_ids(
        &segment_word_ids,
        extend_to_adjacent,
        replaced_ids,
        previous_words,
        next_words,
        hints,
        new_final_words,
    );
    let hint_word_id = hint.word_id.as_deref().unwrap_or_default();
    let next_anchor_word_id = if replaced_ids.contains(hint_word_id) {
        next_word_ids.first().cloned()
    } else {
        Some(hint_word_id.to_string())
    };

    let Some(next_anchor_word_id) = next_anchor_word_id.filter(|_| !next_word_ids.is_empty())
    else {
        return Vec::new();
    };

    let Some(object) = assignment_value.as_object_mut() else {
        return Vec::new();
    };
    object.insert(
        "word_ids".to_string(),
        Value::Array(next_word_ids.into_iter().map(Value::String).collect()),
    );
    let value =
        serde_json::to_string(&assignment_value).expect("serializing a JSON value should not fail");

    vec![StoredSpeakerHint {
        id: format!("{next_anchor_word_id}:user_speaker_assignment:segment"),
        word_id: Some(next_anchor_word_id),
        hint_type: hint.hint_type.clone(),
        value: Value::String(value),
    }]
}

fn get_reconciled_segment_word_ids(
    segment_word_ids: &[String],
    extend_to_adjacent: bool,
    replaced_ids: &HashSet<&str>,
    previous_words: &[StoredTranscriptWord],
    next_words: &[StoredTranscriptWord],
    hints: &[StoredSpeakerHint],
    new_final_words: &[FinalizedWord],
) -> Vec<String> {
    let previous_words_by_id: HashMap<&str, &StoredTranscriptWord> = previous_words
        .iter()
        .map(|word| (word.id.as_str(), word))
        .collect();
    let next_words_by_id: HashMap<&str, &StoredTranscriptWord> = next_words
        .iter()
        .map(|word| (word.id.as_str(), word))
        .collect();
    let new_speaker_index_by_word_id: HashMap<&str, f64> = new_final_words
        .iter()
        .filter_map(|word| Some((word.id.as_str(), word.speaker_index? as f64)))
        .collect();
    let scoped_previous_words: Vec<&StoredTranscriptWord> = segment_word_ids
        .iter()
        .filter_map(|word_id| previous_words_by_id.get(word_id.as_str()).copied())
        .collect();
    let scoped_next_words: Vec<&StoredTranscriptWord> = segment_word_ids
        .iter()
        .filter_map(|word_id| next_words_by_id.get(word_id.as_str()).copied())
        .collect();

    let segment_key = if let Some(anchor_word) = scoped_previous_words.first() {
        get_speaker_segment_key(anchor_word, hints, &new_speaker_index_by_word_id)
    } else if let Some(anchor_word) = scoped_next_words.first() {
        get_speaker_segment_key(anchor_word, hints, &new_speaker_index_by_word_id)
    } else {
        return Vec::new();
    };

    let mut seed_word_ids: HashSet<String> = segment_word_ids
        .iter()
        .filter(|word_id| {
            !replaced_ids.contains(word_id.as_str())
                && next_words_by_id.contains_key(word_id.as_str())
        })
        .cloned()
        .collect();

    if segment_word_ids
        .iter()
        .any(|word_id| replaced_ids.contains(word_id.as_str()))
    {
        let previous_range = get_word_range(&scoped_previous_words);
        for word in new_final_words {
            let in_replaced_overlap = scoped_previous_words.iter().any(|previous| {
                replaced_ids.contains(previous.id.as_str())
                    && previous.end_ms.unwrap_or_default().min(word.end_ms as f64)
                        > previous
                            .start_ms
                            .unwrap_or_default()
                            .max(word.start_ms as f64)
            });
            if is_same_speaker_segment(word, segment_key, hints, &new_speaker_index_by_word_id)
                && if !extend_to_adjacent || segment_key.speaker_index.is_none() {
                    in_replaced_overlap
                } else {
                    previous_range.is_some_and(|range| is_within_segment_range(word, range))
                }
            {
                seed_word_ids.insert(word.id.clone());
            }
        }
    }

    if seed_word_ids.is_empty() {
        return Vec::new();
    }
    if !extend_to_adjacent || segment_key.speaker_index.is_none() {
        return next_words
            .iter()
            .filter(|word| seed_word_ids.contains(&word.id))
            .map(|word| word.id.clone())
            .collect();
    }

    let seed_indexes: Vec<usize> = next_words
        .iter()
        .enumerate()
        .filter_map(|(index, word)| seed_word_ids.contains(&word.id).then_some(index))
        .collect();
    let Some(mut start_index) = seed_indexes.iter().min().copied() else {
        return Vec::new();
    };
    let Some(mut end_index) = seed_indexes.iter().max().copied() else {
        return Vec::new();
    };

    while start_index > 0
        && can_merge_segment_words(
            &next_words[start_index - 1],
            &next_words[start_index],
            segment_key,
            hints,
            &new_speaker_index_by_word_id,
        )
    {
        start_index -= 1;
    }

    while end_index < next_words.len() - 1
        && can_merge_segment_words(
            &next_words[end_index],
            &next_words[end_index + 1],
            segment_key,
            hints,
            &new_speaker_index_by_word_id,
        )
    {
        end_index += 1;
    }

    let mut word_ids = Vec::new();
    for word in next_words[start_index..=end_index].iter().filter(|word| {
        is_same_speaker_segment_stored(word, segment_key, hints, &new_speaker_index_by_word_id)
    }) {
        insert_set_value(&mut word_ids, word.id.clone());
    }
    word_ids
}

fn get_speaker_segment_key(
    word: &StoredTranscriptWord,
    hints: &[StoredSpeakerHint],
    new_speaker_index_by_word_id: &HashMap<&str, f64>,
) -> SpeakerSegmentKey {
    SpeakerSegmentKey {
        channel: word.channel.unwrap_or_default(),
        speaker_index: new_speaker_index_by_word_id
            .get(word.id.as_str())
            .copied()
            .or_else(|| find_speaker_index_for_word(hints, &word.id)),
    }
}

fn is_same_speaker_segment(
    word: &FinalizedWord,
    key: SpeakerSegmentKey,
    hints: &[StoredSpeakerHint],
    new_speaker_index_by_word_id: &HashMap<&str, f64>,
) -> bool {
    let word_key = SpeakerSegmentKey {
        channel: word.channel as f64,
        speaker_index: new_speaker_index_by_word_id
            .get(word.id.as_str())
            .copied()
            .or_else(|| find_speaker_index_for_word(hints, &word.id)),
    };
    word_key.channel == key.channel && word_key.speaker_index == key.speaker_index
}

fn is_same_speaker_segment_stored(
    word: &StoredTranscriptWord,
    key: SpeakerSegmentKey,
    hints: &[StoredSpeakerHint],
    new_speaker_index_by_word_id: &HashMap<&str, f64>,
) -> bool {
    let word_key = get_speaker_segment_key(word, hints, new_speaker_index_by_word_id);
    word_key.channel == key.channel && word_key.speaker_index == key.speaker_index
}

fn can_merge_segment_words(
    left: &StoredTranscriptWord,
    right: &StoredTranscriptWord,
    key: SpeakerSegmentKey,
    hints: &[StoredSpeakerHint],
    new_speaker_index_by_word_id: &HashMap<&str, f64>,
) -> bool {
    is_same_speaker_segment_stored(left, key, hints, new_speaker_index_by_word_id)
        && is_same_speaker_segment_stored(right, key, hints, new_speaker_index_by_word_id)
        && right.start_ms.unwrap_or_default() - left.end_ms.unwrap_or_default()
            <= MAX_SEGMENT_GAP_MS
}

fn get_word_range(words: &[&StoredTranscriptWord]) -> Option<WordRange> {
    let first = words.first()?;
    Some(words.iter().skip(1).fold(
        WordRange {
            start_ms: first.start_ms.unwrap_or_default(),
            end_ms: first.end_ms.unwrap_or_default(),
        },
        |range, word| WordRange {
            start_ms: range.start_ms.min(word.start_ms.unwrap_or_default()),
            end_ms: range.end_ms.max(word.end_ms.unwrap_or_default()),
        },
    ))
}

fn is_within_segment_range(word: &FinalizedWord, range: WordRange) -> bool {
    (word.start_ms as f64) <= range.end_ms + MAX_SEGMENT_GAP_MS
        && (word.end_ms as f64) >= range.start_ms - MAX_SEGMENT_GAP_MS
}

fn is_segment_speaker_assignment_hint(hint: &StoredSpeakerHint) -> bool {
    get_segment_speaker_assignment(hint).is_some()
}

fn is_speaker_scoped_assignment_hint(hint: &StoredSpeakerHint) -> bool {
    if hint.hint_type != "automatic_speaker_assignment"
        && hint.hint_type != "user_speaker_assignment"
    {
        return false;
    }

    let Some(value) = parse_hint_value(&hint.value) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    let channel = object.get("channel").and_then(Value::as_f64);
    let speaker_index = object.get("speaker_index");
    object.get("scope").and_then(Value::as_str) == Some("speaker")
        && channel.is_some_and(|channel| channel == 0.0 || channel == 1.0 || channel == 2.0)
        && speaker_index.is_some_and(|value| value.is_null() || value.is_number())
}

fn get_segment_speaker_assignment(hint: &StoredSpeakerHint) -> Option<(Value, Vec<String>)> {
    if hint.hint_type != "user_speaker_assignment" {
        return None;
    }
    let value = parse_hint_value(&hint.value)?;
    let object = value.as_object()?;
    if object.get("scope").and_then(Value::as_str) != Some("segment") {
        return None;
    }
    let word_ids = object.get("word_ids")?.as_array()?;

    let unique_word_ids = get_unique_word_ids(word_ids);
    Some((value, unique_word_ids))
}

fn get_unique_word_ids(word_ids: &[Value]) -> Vec<String> {
    let mut unique = Vec::new();
    for word_id in word_ids {
        if let Some(word_id) = word_id.as_str().filter(|word_id| !word_id.is_empty()) {
            insert_set_value(&mut unique, word_id.to_string());
        }
    }
    unique
}

fn find_speaker_index_for_word(hints: &[StoredSpeakerHint], word_id: &str) -> Option<f64> {
    hints.iter().rev().find_map(|hint| {
        if hint.hint_type != "provider_speaker_index" || hint.word_id.as_deref() != Some(word_id) {
            return None;
        }
        let value = parse_hint_value(&hint.value)?;
        value.as_object()?.get("speaker_index")?.as_f64()
    })
}

fn parse_hint_value(value: &Value) -> Option<Value> {
    match value {
        Value::String(value) => serde_json::from_str(value).ok(),
        value => Some(value.clone()),
    }
}

fn to_storage_speaker_hint(word: &FinalizedWord) -> Option<StoredSpeakerHint> {
    let speaker_index = word.speaker_index?;
    #[derive(Serialize)]
    struct ProviderSpeakerIndex {
        channel: i32,
        speaker_index: i32,
    }

    let value = serde_json::to_string(&ProviderSpeakerIndex {
        channel: word.channel,
        speaker_index,
    })
    .expect("serializing a provider speaker index should not fail");
    Some(StoredSpeakerHint {
        id: format!("{}:provider_speaker_index", word.id),
        word_id: Some(word.id.clone()),
        hint_type: "provider_speaker_index".to_string(),
        value: Value::String(value),
    })
}

fn compare_numbers(left: f64, right: f64) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

#[derive(Serialize)]
struct StoredWordJson<'a> {
    id: &'a str,
    text: &'a str,
    start_ms: NormalizedNumber,
    end_ms: NormalizedNumber,
    channel: NormalizedNumber,
    #[serde(skip_serializing_if = "Option::is_none")]
    speaker: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<&'a Value>,
}

struct NormalizedNumber(f64);

impl Serialize for NormalizedNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        json_number(self.0).serialize(serializer)
    }
}

#[derive(Serialize)]
struct StoredHintJson<'a> {
    id: &'a str,
    word_id: &'a str,
    #[serde(rename = "type")]
    hint_type: &'a str,
    value: &'a Value,
}

pub fn serialize_batch_transcript_words(
    words: &[StoredTranscriptWord],
) -> Result<String, serde_json::Error> {
    let words: Vec<StoredWordJson<'_>> = words
        .iter()
        .map(|word| {
            let start_ms = word.start_ms.unwrap_or_default();
            StoredWordJson {
                id: &word.id,
                text: word.text.as_deref().unwrap_or_default(),
                start_ms: NormalizedNumber(start_ms),
                end_ms: NormalizedNumber(word.end_ms.unwrap_or(start_ms)),
                channel: NormalizedNumber(word.channel.unwrap_or_default()),
                speaker: word.speaker.as_deref(),
                metadata: word
                    .metadata
                    .as_ref()
                    .filter(|metadata| !metadata.is_null()),
            }
        })
        .collect();
    serde_json::to_string(&words)
}

pub fn serialize_batch_transcript_hints(
    hints: &[StoredSpeakerHint],
) -> Result<String, serde_json::Error> {
    let hints: Vec<StoredHintJson<'_>> = hints
        .iter()
        .map(|hint| StoredHintJson {
            id: &hint.id,
            word_id: hint.word_id.as_deref().unwrap_or_default(),
            hint_type: &hint.hint_type,
            value: &hint.value,
        })
        .collect();
    serde_json::to_string(&hints)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WordState;

    fn word(
        id: &str,
        text: &str,
        start_ms: i64,
        end_ms: i64,
        channel: i32,
    ) -> StoredTranscriptWord {
        StoredTranscriptWord {
            id: id.to_string(),
            text: Some(text.to_string()),
            start_ms: Some(start_ms as f64),
            end_ms: Some(end_ms as f64),
            channel: Some(channel as f64),
            speaker: None,
            metadata: None,
        }
    }

    fn finalized_word(
        id: &str,
        text: &str,
        start_ms: i64,
        end_ms: i64,
        channel: i32,
        speaker_index: Option<i32>,
    ) -> FinalizedWord {
        FinalizedWord {
            id: id.to_string(),
            text: text.to_string(),
            start_ms,
            end_ms,
            channel,
            state: WordState::Final,
            speaker_index,
        }
    }

    fn delta(new_words: Vec<FinalizedWord>, replaced_ids: &[&str]) -> StoredLiveTranscriptDelta {
        StoredLiveTranscriptDelta {
            new_words,
            replaced_ids: replaced_ids.iter().map(|id| (*id).to_string()).collect(),
        }
    }

    fn hint(id: &str, word_id: Option<&str>, hint_type: &str, value: Value) -> StoredSpeakerHint {
        StoredSpeakerHint {
            id: id.to_string(),
            word_id: word_id.map(str::to_string),
            hint_type: hint_type.to_string(),
            value,
        }
    }

    fn assignment_hint(
        anchor: &str,
        word_ids: &[&str],
        extend_to_adjacent: Option<bool>,
    ) -> StoredSpeakerHint {
        let mut value = serde_json::Map::new();
        value.insert("human_id".to_string(), Value::String("human-1".to_string()));
        value.insert("scope".to_string(), Value::String("segment".to_string()));
        value.insert(
            "word_ids".to_string(),
            Value::Array(
                word_ids
                    .iter()
                    .map(|word_id| Value::String((*word_id).to_string()))
                    .collect(),
            ),
        );
        if let Some(extend_to_adjacent) = extend_to_adjacent {
            value.insert(
                "extend_to_adjacent".to_string(),
                Value::Bool(extend_to_adjacent),
            );
        }
        hint(
            &format!("{anchor}:user_speaker_assignment:segment"),
            Some(anchor),
            "user_speaker_assignment",
            Value::String(Value::Object(value).to_string()),
        )
    }

    fn provider_hint(word_id: &str, channel: i32, speaker_index: i32) -> StoredSpeakerHint {
        hint(
            &format!("{word_id}:provider_speaker_index"),
            Some(word_id),
            "provider_speaker_index",
            Value::String(
                serde_json::json!({
                    "channel": channel,
                    "speaker_index": speaker_index
                })
                .to_string(),
            ),
        )
    }

    fn segment_word_ids(hint: &StoredSpeakerHint) -> Vec<String> {
        let Value::String(value) = &hint.value else {
            panic!("assignment value should be stored as JSON text");
        };
        serde_json::from_str::<Value>(value)
            .unwrap()
            .get("word_ids")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn applies_replacements_and_discards_hints_for_replaced_words() {
        let old_word = word("word-1", "hello", 100, 200, 0);
        let old_hint = provider_hint("word-1", 0, 1);
        let (words, hints) = apply_live_transcript_delta(
            vec![old_word],
            vec![old_hint],
            &delta(
                vec![finalized_word("word-2", "hello", 100, 220, 0, None)],
                &["word-1"],
            ),
        );

        assert_eq!(words, vec![word("word-2", "hello", 100, 220, 0)]);
        assert!(hints.is_empty());
    }

    #[test]
    fn keeps_segment_assignment_word_ids_current_when_a_scoped_word_is_replaced() {
        let assignment = assignment_hint("word-1", &["word-1", "word-2"], None);
        let (_, hints) = apply_live_transcript_delta(
            vec![
                word("word-1", "hello", 0, 100, 0),
                word("word-2", "there", 100, 200, 0),
            ],
            vec![assignment],
            &delta(
                vec![finalized_word("word-2b", "there", 100, 220, 0, None)],
                &["word-2"],
            ),
        );

        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].id, "word-1:user_speaker_assignment:segment");
        assert_eq!(hints[0].word_id.as_deref(), Some("word-1"));
        assert_eq!(segment_word_ids(&hints[0]), ["word-1", "word-2b"]);
    }

    #[test]
    fn moves_a_segment_assignment_anchor_when_the_anchor_word_is_replaced() {
        let assignment = assignment_hint("word-1", &["word-1", "word-2"], None);
        let (_, hints) = apply_live_transcript_delta(
            vec![
                word("word-1", "hello", 0, 100, 0),
                word("word-2", "there", 100, 200, 0),
            ],
            vec![assignment],
            &delta(
                vec![
                    finalized_word("word-1b", "hello", 0, 110, 0, None),
                    finalized_word("word-2b", "there", 110, 220, 0, None),
                ],
                &["word-1", "word-2"],
            ),
        );

        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].id, "word-1b:user_speaker_assignment:segment");
        assert_eq!(hints[0].word_id.as_deref(), Some("word-1b"));
        assert_eq!(segment_word_ids(&hints[0]), ["word-1b", "word-2b"]);
    }

    #[test]
    fn does_not_expand_an_unindexed_assignment_as_live_words_arrive() {
        for replace in [false, true] {
            let assignment = assignment_hint("selected", &["selected"], None);
            let mut new_words = Vec::new();
            let mut replaced_ids = Vec::new();
            if replace {
                new_words.push(finalized_word("replacement", "hello", 0, 100, 2, None));
                replaced_ids.push("selected");
            }
            new_words.push(finalized_word("other", "another voice", 100, 200, 2, None));

            let (_, hints) = apply_live_transcript_delta(
                vec![word("selected", "hello", 0, 100, 2)],
                vec![assignment],
                &delta(new_words, &replaced_ids),
            );

            assert_eq!(
                segment_word_ids(&hints[0]),
                [if replace { "replacement" } else { "selected" }]
            );
        }
    }

    #[test]
    fn keeps_reconstructed_indexed_word_selections_fixed_during_live_persistence() {
        let words = vec![
            word("first", "first", 0, 100, 1),
            word("other", "other", 100, 200, 1),
            word("last", "last", 200, 300, 1),
        ];
        let hints = vec![
            provider_hint("first", 1, 0),
            provider_hint("other", 1, 0),
            provider_hint("last", 1, 0),
            assignment_hint("first", &["first", "last"], Some(false)),
        ];
        let (_, hints) = apply_live_transcript_delta(
            words,
            hints,
            &delta(
                vec![finalized_word("appended", "another", 300, 400, 1, Some(0))],
                &[],
            ),
        );
        let assignment = hints
            .iter()
            .find(|hint| hint.hint_type == "user_speaker_assignment")
            .unwrap();

        assert_eq!(segment_word_ids(assignment), ["first", "last"]);
    }

    #[test]
    fn adds_appended_live_words_to_a_continuing_segment_assignment() {
        let words = vec![
            word("word-1", "hello", 0, 100, 0),
            word("word-2", "there", 100, 200, 0),
        ];
        let hints = vec![
            provider_hint("word-1", 0, 2),
            provider_hint("word-2", 0, 2),
            assignment_hint("word-1", &["word-1", "word-2"], None),
        ];
        let (_, hints) = apply_live_transcript_delta(
            words,
            hints,
            &delta(
                vec![finalized_word("word-3", "again", 200, 300, 0, Some(2))],
                &[],
            ),
        );
        let assignment = hints
            .iter()
            .find(|hint| hint.hint_type == "user_speaker_assignment")
            .unwrap();

        assert_eq!(segment_word_ids(assignment), ["word-1", "word-2", "word-3"]);
    }

    #[test]
    fn does_not_add_unrelated_words_from_a_delta_to_a_segment_assignment() {
        let words = vec![
            word("word-1", "hello", 0, 100, 0),
            word("word-2", "there", 100, 200, 0),
        ];
        let hints = vec![
            provider_hint("word-1", 0, 2),
            provider_hint("word-2", 0, 2),
            assignment_hint("word-1", &["word-1", "word-2"], None),
        ];
        let (_, hints) = apply_live_transcript_delta(
            words,
            hints,
            &delta(
                vec![
                    finalized_word("word-3", "again", 200, 300, 0, Some(2)),
                    finalized_word("word-4", "other", 300, 400, 0, Some(3)),
                    finalized_word("word-5", "later", 400, 500, 0, Some(2)),
                ],
                &[],
            ),
        );
        let assignment = hints
            .iter()
            .find(|hint| hint.hint_type == "user_speaker_assignment")
            .unwrap();

        assert_eq!(segment_word_ids(assignment), ["word-1", "word-2", "word-3"]);
    }

    #[test]
    fn coalesces_a_word_that_is_replaced_then_readded() {
        let deltas = vec![
            delta(
                vec![finalized_word("word-1", "first", 0, 100, 0, None)],
                &[],
            ),
            delta(
                vec![finalized_word("word-2", "interim", 0, 110, 0, None)],
                &["word-1"],
            ),
            delta(
                vec![finalized_word("word-1", "final", 0, 120, 0, None)],
                &["word-2"],
            ),
        ];
        let coalesced = coalesce_live_transcript_deltas(&deltas);

        assert_eq!(coalesced.replaced_ids, ["word-1"]);
        assert_eq!(coalesced.new_words.len(), 1);
        assert_eq!(coalesced.new_words[0].id, "word-1");
        assert_eq!(coalesced.new_words[0].text, "final");
    }

    #[test]
    fn materialization_without_deltas_is_a_noop() {
        let words = vec![
            word("word-2", "second", 100, 200, 0),
            word("word-1", "first", 0, 100, 0),
        ];
        let hints = vec![hint("z", Some("z"), "custom", Value::Null)];
        let (actual_words, actual_hints) =
            materialize_live_transcript(words.clone(), hints.clone(), &[]);

        assert_eq!(actual_words, words);
        assert_eq!(actual_hints, hints);
    }

    #[test]
    fn serializes_batch_words_and_hints_with_integer_numbers() {
        let words = vec![StoredTranscriptWord {
            id: "word-1".to_string(),
            text: None,
            start_ms: Some(1200.0),
            end_ms: None,
            channel: Some(0.0),
            speaker: None,
            metadata: Some(Value::Null),
        }];
        let hints = vec![hint(
            "hint-1",
            None,
            "custom",
            Value::String("value".to_string()),
        )];

        assert_eq!(
            serde_json::from_str::<Value>(&serialize_batch_transcript_words(&words).unwrap())
                .unwrap(),
            serde_json::json!([{
                "id": "word-1",
                "text": "",
                "start_ms": 1200,
                "end_ms": 1200,
                "channel": 0,
            }])
        );
        assert_eq!(
            serde_json::from_str::<Value>(&serialize_batch_transcript_hints(&hints).unwrap())
                .unwrap(),
            serde_json::json!([{
                "id": "hint-1",
                "word_id": "",
                "type": "custom",
                "value": "value",
            }])
        );
    }
}
