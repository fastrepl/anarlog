//! Live transcript persistence (`stt/utils.ts`'s `TranscriptAccumulator`
//! and `transcript-persistence-worker.ts`'s coalescing): the engine's
//! `LiveTranscriptDelta`s become the stored `words_json` /
//! `speaker_hints_json` exactly as the Tauri frontend writes them.

use std::collections::{HashMap, HashSet};

use anlg_listener_core::LiveTranscriptDelta;
use anlg_transcript::FinalizedWord;
use serde_json::{Map, Value, json};

/// `WordWithId` as `applyLiveDelta` builds it from a finalized word, in the
/// object's key order.
fn storage_word(word: &FinalizedWord) -> Value {
    json!({
        "id": word.id,
        "text": word.text,
        "start_ms": word.start_ms,
        "end_ms": word.end_ms,
        "channel": word.channel,
    })
}

/// `toStorageSpeakerHints`
fn storage_hints(word: &FinalizedWord) -> Option<Value> {
    let speaker_index = word.speaker_index?;
    Some(json!({
        "id": format!("{}:provider_speaker_index", word.id),
        "word_id": word.id,
        "type": "provider_speaker_index",
        "value": json!({ "channel": word.channel, "speaker_index": speaker_index }).to_string(),
    }))
}

fn parse_array(json: &str) -> Vec<Value> {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|value| match value {
            Value::Array(items) => Some(items),
            _ => None,
        })
        .unwrap_or_default()
}

fn text_field<'a>(object: &'a Value, key: &str) -> &'a str {
    object.get(key).and_then(Value::as_str).unwrap_or("")
}

fn hint_value(hint: &Value) -> Option<Map<String, Value>> {
    let raw = hint.get("value")?;
    match raw {
        Value::String(text) => serde_json::from_str::<Value>(text).ok(),
        other => Some(other.clone()),
    }
    .and_then(|value| match value {
        Value::Object(map) => Some(map),
        _ => None,
    })
}

/// `isSegmentSpeakerAssignmentHint`
fn is_segment_speaker_assignment(hint: &Value) -> bool {
    text_field(hint, "type") == "user_speaker_assignment"
        && hint_value(hint).is_some_and(|value| {
            value.get("scope").and_then(Value::as_str) == Some("segment")
                && value.get("word_ids").is_some_and(Value::is_array)
        })
}

/// `isSpeakerScopedAssignmentHint`
fn is_speaker_scoped_assignment(hint: &Value) -> bool {
    matches!(
        text_field(hint, "type"),
        "automatic_speaker_assignment" | "user_speaker_assignment"
    ) && hint_value(hint).is_some_and(|value| {
        value.get("scope").and_then(Value::as_str) == Some("speaker")
            && value.get("channel").is_some_and(Value::is_number)
            && value.get("speaker_index").is_some_and(Value::is_number)
    })
}

/// `TranscriptAccumulator.applyLiveDelta` over the stored JSON: drop the
/// replaced and re-issued words, append the new ones sorted by `start_ms`,
/// keep segment / speaker-scoped assignments, drop per-word hints whose
/// word left, and add the provider speaker hints of the new words sorted by
/// `word_id`. Returns the next `(words_json, speaker_hints_json)`.
pub fn apply_live_delta(
    words_json: &str,
    hints_json: &str,
    delta: &LiveTranscriptDelta,
) -> (String, String) {
    let replaced: HashSet<&str> = delta.replaced_ids.iter().map(String::as_str).collect();
    let new_ids: HashSet<&str> = delta
        .new_words
        .iter()
        .map(|word| word.id.as_str())
        .collect();

    let mut words: Vec<Value> = parse_array(words_json)
        .into_iter()
        .filter(|word| {
            let id = text_field(word, "id");
            !replaced.contains(id) && !new_ids.contains(id)
        })
        .collect();
    words.extend(delta.new_words.iter().map(storage_word));
    words.sort_by_key(|word| word.get("start_ms").and_then(Value::as_i64).unwrap_or(0));

    let mut hints: Vec<Value> = parse_array(hints_json)
        .into_iter()
        .filter(|hint| {
            if is_segment_speaker_assignment(hint) || is_speaker_scoped_assignment(hint) {
                return true;
            }
            let word_id = text_field(hint, "word_id");
            !replaced.contains(word_id) && !new_ids.contains(word_id)
        })
        .collect();
    hints.extend(delta.new_words.iter().filter_map(storage_hints));
    hints.sort_by(|a, b| text_field(a, "word_id").cmp(text_field(b, "word_id")));

    (
        Value::Array(words).to_string(),
        Value::Array(hints).to_string(),
    )
}

/// `coalesceLiveTranscriptDeltas`: fold journaled deltas into one, dropping
/// pending words that a later delta replaced and only carrying replaced ids
/// that may exist in the persisted base.
pub fn coalesce_deltas(deltas: &[LiveTranscriptDelta]) -> LiveTranscriptDelta {
    struct Pending {
        words: Vec<FinalizedWord>,
        may_exist_in_base: bool,
    }
    let mut order: Vec<String> = Vec::new();
    let mut words_by_id: HashMap<String, Pending> = HashMap::new();
    let mut replaced_root_ids: Vec<String> = Vec::new();
    let mut replaced_set: HashSet<String> = HashSet::new();
    let mut add_replaced = |id: &str, replaced_root_ids: &mut Vec<String>| {
        if replaced_set.insert(id.to_string()) {
            replaced_root_ids.push(id.to_string());
        }
    };
    for delta in deltas {
        for replaced in &delta.replaced_ids {
            if let Some(pending) = words_by_id.remove(replaced) {
                order.retain(|id| id != replaced);
                if pending.may_exist_in_base {
                    add_replaced(replaced, &mut replaced_root_ids);
                }
            } else {
                add_replaced(replaced, &mut replaced_root_ids);
            }
        }
        let mut next: Vec<(String, Vec<FinalizedWord>)> = Vec::new();
        for word in &delta.new_words {
            match next.iter_mut().find(|(id, _)| *id == word.id) {
                Some((_, words)) => words.push(word.clone()),
                None => next.push((word.id.clone(), vec![word.clone()])),
            }
        }
        for (id, words) in next {
            let existing = words_by_id.remove(&id);
            if existing.is_some() {
                order.retain(|pending| *pending != id);
            }
            order.push(id.clone());
            words_by_id.insert(
                id,
                Pending {
                    words,
                    may_exist_in_base: existing
                        .map(|pending| pending.may_exist_in_base)
                        .unwrap_or(delta.replaced_ids.is_empty()),
                },
            );
        }
    }
    LiveTranscriptDelta {
        new_words: order
            .iter()
            .flat_map(|id| {
                words_by_id
                    .get(id)
                    .map(|pending| pending.words.clone())
                    .unwrap_or_default()
            })
            .collect(),
        replaced_ids: replaced_root_ids,
        partials: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_transcript::WordState;

    fn word(id: &str, text: &str, start: i64, speaker: Option<i32>) -> FinalizedWord {
        FinalizedWord {
            id: id.to_string(),
            text: text.to_string(),
            start_ms: start,
            end_ms: start + 400,
            channel: 0,
            state: WordState::Final,
            speaker_index: speaker,
        }
    }

    fn delta(new_words: Vec<FinalizedWord>, replaced: &[&str]) -> LiveTranscriptDelta {
        LiveTranscriptDelta {
            new_words,
            replaced_ids: replaced.iter().map(|id| id.to_string()).collect(),
            partials: Vec::new(),
        }
    }

    #[test]
    fn applies_a_first_delta_to_empty_storage_in_the_frontend_shape() {
        let (words, hints) = apply_live_delta(
            "[]",
            "[]",
            &delta(
                vec![
                    word("b", "world", 500, Some(1)),
                    word("a", "hello", 0, None),
                ],
                &[],
            ),
        );
        assert_eq!(
            words,
            r#"[{"id":"a","text":"hello","start_ms":0,"end_ms":400,"channel":0},{"id":"b","text":"world","start_ms":500,"end_ms":900,"channel":0}]"#
        );
        assert_eq!(
            hints,
            r#"[{"id":"b:provider_speaker_index","word_id":"b","type":"provider_speaker_index","value":"{\"channel\":0,\"speaker_index\":1}"}]"#
        );
    }

    #[test]
    fn replaces_words_and_their_hints_but_keeps_assignments() {
        let words = r#"[{"id":"a","text":"hello","start_ms":0,"end_ms":400,"channel":0,"extra":true},{"id":"b","text":"wrld","start_ms":500,"end_ms":900,"channel":0}]"#;
        let hints = r#"[{"id":"b:provider_speaker_index","word_id":"b","type":"provider_speaker_index","value":"{\"channel\":0,\"speaker_index\":1}"},{"id":"seg","word_id":"b","type":"user_speaker_assignment","value":"{\"scope\":\"segment\",\"word_ids\":[\"b\"],\"human_id\":\"h1\"}"}]"#;
        let (next_words, next_hints) = apply_live_delta(
            words,
            hints,
            &delta(vec![word("c", "world", 500, Some(2))], &["b"]),
        );
        let parsed: Vec<Value> = serde_json::from_str(&next_words).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["id"], "a");
        assert_eq!(
            parsed[0]["extra"], true,
            "untouched words keep their fields"
        );
        assert_eq!(parsed[1]["id"], "c");
        let parsed: Vec<Value> = serde_json::from_str(&next_hints).unwrap();
        let ids: Vec<&str> = parsed
            .iter()
            .map(|hint| hint["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["seg", "c:provider_speaker_index"]);
    }

    #[test]
    fn coalescing_drops_pending_words_a_later_delta_replaced() {
        let first = delta(
            vec![word("a", "hel", 0, None), word("b", "lo", 400, None)],
            &[],
        );
        let second = delta(vec![word("c", "hello", 0, None)], &["a", "b"]);
        let third = delta(vec![word("d", "there", 900, None)], &["x"]);
        let merged = coalesce_deltas(&[first, second, third]);
        let ids: Vec<&str> = merged.new_words.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, ["c", "d"]);
        // `a` and `b` arrived in a delta without replacements, so they may
        // already sit in the persisted base like `x`.
        assert_eq!(
            merged.replaced_ids,
            vec!["a".to_string(), "b".to_string(), "x".to_string()]
        );
    }
}
