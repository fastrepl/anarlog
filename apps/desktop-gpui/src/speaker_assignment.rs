//! Speaker assignment and transcript text edits over the stored
//! `words_json` / `speaker_hints_json`: the port of `stt/utils.ts`'s
//! `upsertSpeakerAssignment`, `findSpeakerAssignmentAnchorWordId`,
//! `mergeTranscriptSegmentAssignments` and `stt/queries.ts`'s
//! `updateTranscriptSegmentText`, writing the same JSON the Tauri frontend
//! does.

use std::collections::{HashMap, HashSet};

use anlg_transcript::{ChannelProfile, SegmentKey};
use serde_json::{Map, Value, json};

/// `mode` of `upsertSpeakerAssignment`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// `"all"`: every segment of the speaker (channel + provider index).
    All,
    /// `"segment"`: only the listed words.
    Segment { word_ids: Vec<String> },
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

/// `parseHintValue`: the stored `value` is usually a JSON string.
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

fn is_assignment(hint: &Value) -> bool {
    matches!(
        text_field(hint, "type"),
        "automatic_speaker_assignment" | "user_speaker_assignment"
    )
}

fn channel_number(channel: ChannelProfile) -> i64 {
    match channel {
        ChannelProfile::DirectMic => 0,
        ChannelProfile::RemoteParty => 1,
        ChannelProfile::MixedCapture => 2,
    }
}

/// `getUniqueWordIds`
fn unique_word_ids<'a>(word_ids: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut seen = HashSet::new();
    word_ids
        .into_iter()
        .filter(|id| !id.is_empty())
        .filter(|id| seen.insert(id.to_string()))
        .map(str::to_string)
        .collect()
}

/// `findSpeakerIndexForWord`
fn speaker_index_for_word(hints: &[Value], word_id: &str) -> Option<i64> {
    let provider = hints.iter().find(|hint| {
        text_field(hint, "type") == "provider_speaker_index"
            && text_field(hint, "word_id") == word_id
    })?;
    hint_value(provider)?.get("speaker_index")?.as_i64()
}

/// `SpeakerAssignmentScope`
#[derive(Debug, Clone, PartialEq)]
enum Scope {
    All {
        channel: Option<i64>,
        speaker_index: Option<i64>,
    },
    Words(HashSet<String>),
}

/// `getSpeakerAssignmentScopeForHint`
fn scope_for_hint(
    hints: &[Value],
    words_by_id: &HashMap<&str, &Value>,
    hint: &Value,
) -> Option<Scope> {
    let value = hint_value(hint);
    if let Some(value) = &value
        && value.get("scope").and_then(Value::as_str) == Some("speaker")
        && let Some(channel) = value.get("channel").and_then(Value::as_i64)
        && (0..=2).contains(&channel)
    {
        match value.get("speaker_index") {
            Some(Value::Null) | None => {
                return Some(Scope::All {
                    channel: Some(channel),
                    speaker_index: None,
                });
            }
            Some(Value::Number(index)) => {
                return Some(Scope::All {
                    channel: Some(channel),
                    speaker_index: index.as_i64(),
                });
            }
            _ => {}
        }
    }
    if let Some(value) = &value
        && value.get("scope").and_then(Value::as_str) == Some("segment")
        && let Some(word_ids) = value.get("word_ids").and_then(Value::as_array)
    {
        return Some(Scope::Words(
            word_ids
                .iter()
                .filter_map(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .collect(),
        ));
    }
    let word_id = hint.get("word_id").and_then(Value::as_str)?;
    let word = words_by_id.get(word_id)?;
    Some(Scope::All {
        channel: word.get("channel").and_then(Value::as_i64),
        speaker_index: speaker_index_for_word(hints, word_id),
    })
}

/// `speakerAssignmentScopesConflict`
fn scopes_conflict(
    left: &Scope,
    right: &Scope,
    hints: &[Value],
    words_by_id: &HashMap<&str, &Value>,
) -> bool {
    match (left, right) {
        (Scope::Words(left), Scope::Words(right)) => !left.is_disjoint(right),
        (Scope::All { .. }, Scope::Words(_)) => false,
        (
            Scope::Words(left),
            Scope::All {
                channel,
                speaker_index,
            },
        ) => left.iter().any(|word_id| {
            let Some(word) = words_by_id.get(word_id.as_str()) else {
                return false;
            };
            if word.get("channel").and_then(Value::as_i64) != *channel {
                return false;
            }
            speaker_index.is_none() || speaker_index_for_word(hints, word_id) == *speaker_index
        }),
        (
            Scope::All {
                channel: left_channel,
                speaker_index: left_index,
            },
            Scope::All {
                channel: right_channel,
                speaker_index: right_index,
            },
        ) => {
            left_channel == right_channel
                && (left_index.is_none() || right_index.is_none() || left_index == right_index)
        }
    }
}

/// `upsertSpeakerAssignment`: drop the assignments the new one conflicts
/// with and append the `user_speaker_assignment` hint. Returns the next
/// `speaker_hints_json`.
pub fn upsert_speaker_assignment(
    words_json: &str,
    hints_json: &str,
    segment_key: &SegmentKey,
    human_id: &str,
    anchor_word_id: &str,
    mode: &Mode,
) -> String {
    let hints = parse_array(hints_json);
    let words = parse_array(words_json);
    let words_by_id: HashMap<&str, &Value> = words
        .iter()
        .map(|word| (text_field(word, "id"), word))
        .collect();
    let channel = channel_number(segment_key.channel);
    let (next_scope, new_hint) = match mode {
        Mode::Segment { word_ids } => {
            let assignment_word_ids = unique_word_ids(
                word_ids
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(anchor_word_id)),
            );
            let hint = json!({
                "id": format!("{anchor_word_id}:user_speaker_assignment:segment"),
                "word_id": anchor_word_id,
                "type": "user_speaker_assignment",
                "value": json!({
                    "human_id": human_id,
                    "scope": "segment",
                    "word_ids": assignment_word_ids,
                })
                .to_string(),
            });
            (
                Scope::Words(assignment_word_ids.into_iter().collect()),
                hint,
            )
        }
        Mode::All => {
            let speaker_index = segment_key.speaker_index.map(i64::from);
            let hint = json!({
                "id": format!("{anchor_word_id}:user_speaker_assignment"),
                "word_id": anchor_word_id,
                "type": "user_speaker_assignment",
                "value": json!({
                    "human_id": human_id,
                    "scope": "speaker",
                    "channel": channel,
                    "speaker_index": speaker_index,
                })
                .to_string(),
            });
            (
                Scope::All {
                    channel: Some(channel),
                    speaker_index,
                },
                hint,
            )
        }
    };

    let new_id = text_field(&new_hint, "id").to_string();
    let mut next: Vec<Value> = hints
        .iter()
        .filter(|hint| {
            if !is_assignment(hint) {
                return true;
            }
            if text_field(hint, "id") == new_id {
                return false;
            }
            match scope_for_hint(&hints, &words_by_id, hint) {
                Some(scope) => !scopes_conflict(&scope, &next_scope, &hints, &words_by_id),
                None => true,
            }
        })
        .cloned()
        .collect();
    next.push(new_hint);
    Value::Array(next).to_string()
}

/// `findSpeakerAssignmentAnchorWordId`: the first word on the segment's
/// channel with its provider speaker index.
pub fn find_anchor_word_id(
    words_json: &str,
    hints_json: &str,
    segment_key: &SegmentKey,
) -> Option<String> {
    let words = parse_array(words_json);
    let hints = parse_array(hints_json);
    let channel = channel_number(segment_key.channel);
    let speaker_index = segment_key.speaker_index.map(i64::from);
    words
        .iter()
        .find(|word| {
            word.get("channel").and_then(Value::as_i64) == Some(channel)
                && (speaker_index.is_none()
                    || speaker_index_for_word(&hints, text_field(word, "id")) == speaker_index)
        })
        .map(|word| text_field(word, "id").to_string())
}

/// `mergeTranscriptSegmentAssignments`: pull `word_ids` into the target
/// segment, either as a segment assignment of its human or by unifying the
/// provider speaker index. Returns the next `speaker_hints_json`, or `None`
/// when nothing changes.
pub fn merge_segment_assignments(
    words_json: &str,
    hints_json: &str,
    segment_key: &SegmentKey,
    word_ids: &[String],
) -> Option<String> {
    let assignment_word_ids = unique_word_ids(word_ids.iter().map(String::as_str));
    if assignment_word_ids.is_empty() {
        return None;
    }
    if let Some(human_id) = &segment_key.speaker_human_id {
        return Some(upsert_speaker_assignment(
            words_json,
            hints_json,
            segment_key,
            human_id,
            &assignment_word_ids[0],
            &Mode::Segment {
                word_ids: assignment_word_ids.clone(),
            },
        ));
    }
    let speaker_index = segment_key.speaker_index? as i64;
    Some(unify_provider_speaker_index(
        words_json,
        hints_json,
        &assignment_word_ids,
        speaker_index,
    ))
}

/// `unifyProviderSpeakerIndex`
fn unify_provider_speaker_index(
    words_json: &str,
    hints_json: &str,
    word_ids: &[String],
    speaker_index: i64,
) -> String {
    let word_id_set: HashSet<&str> = word_ids.iter().map(String::as_str).collect();
    let words = parse_array(words_json);
    let words_by_id: HashMap<&str, &Value> = words
        .iter()
        .map(|word| (text_field(word, "id"), word))
        .collect();
    let hints = parse_array(hints_json);
    let mut next: Vec<Value> = Vec::new();
    let mut updated_provider_word_ids: HashSet<String> = HashSet::new();

    for hint in &hints {
        let word_id = text_field(hint, "word_id");
        if text_field(hint, "type") == "provider_speaker_index" && word_id_set.contains(word_id) {
            let channel = hint_value(hint)
                .and_then(|value| value.get("channel").and_then(Value::as_i64))
                .or_else(|| {
                    words_by_id
                        .get(word_id)
                        .and_then(|word| word.get("channel").and_then(Value::as_i64))
                })
                .unwrap_or(0);
            let mut updated = hint.clone();
            if let Some(object) = updated.as_object_mut() {
                object.insert(
                    "value".into(),
                    Value::String(
                        json!({ "channel": channel, "speaker_index": speaker_index }).to_string(),
                    ),
                );
            }
            next.push(updated);
            updated_provider_word_ids.insert(word_id.to_string());
            continue;
        }

        if is_assignment(hint) {
            match scope_for_hint(&hints, &words_by_id, hint) {
                Some(Scope::All { .. }) => {
                    next.push(hint.clone());
                    continue;
                }
                Some(Scope::Words(scope_ids))
                    if scope_ids.iter().any(|id| word_id_set.contains(id.as_str())) =>
                {
                    continue;
                }
                _ => {}
            }
            if word_id_set.contains(word_id) {
                continue;
            }
        }

        next.push(hint.clone());
    }

    for word_id in word_ids {
        if updated_provider_word_ids.contains(word_id) {
            continue;
        }
        let Some(word) = words_by_id.get(word_id.as_str()) else {
            continue;
        };
        next.push(json!({
            "id": format!("{word_id}:provider_speaker_index"),
            "word_id": word_id,
            "type": "provider_speaker_index",
            "value": json!({
                "channel": word.get("channel").cloned().unwrap_or(Value::Null),
                "speaker_index": speaker_index,
            })
            .to_string(),
        }));
    }

    Value::Array(next).to_string()
}

/// `updateTranscriptSegmentText`: hand the edited text's whitespace-split
/// tokens to the segment's words in order, the last word taking the rest.
/// Returns the next `words_json`, or `None` when no selected word exists.
pub fn update_segment_text(words_json: &str, word_ids: &[String], text: &str) -> Option<String> {
    let selected: HashSet<&str> = word_ids.iter().map(String::as_str).collect();
    let mut words = parse_array(words_json);
    let selected_ids: Vec<String> = words
        .iter()
        .map(|word| text_field(word, "id"))
        .filter(|id| selected.contains(id))
        .map(str::to_string)
        .collect();
    if selected_ids.is_empty() {
        return None;
    }
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut text_by_word_id: HashMap<&str, String> = HashMap::new();
    let last = selected_ids.len() - 1;
    for (index, id) in selected_ids.iter().enumerate() {
        let next = if index == last {
            tokens.get(index..).unwrap_or(&[]).join(" ")
        } else {
            tokens.get(index).copied().unwrap_or("").to_string()
        };
        text_by_word_id.insert(id, next);
    }
    for word in &mut words {
        let id = text_field(word, "id").to_string();
        let Some(next_text) = text_by_word_id.get(id.as_str()) else {
            continue;
        };
        if text_field(word, "text") == next_text {
            continue;
        }
        if let Some(object) = word.as_object_mut() {
            object.insert("text".into(), Value::String(next_text.clone()));
        }
    }
    Some(Value::Array(words).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(id: &str, text: &str, start: i64, channel: i64) -> Value {
        json!({ "id": id, "text": text, "start_ms": start, "end_ms": start + 100, "channel": channel })
    }

    fn provider(word_id: &str, channel: i64, speaker_index: i64) -> Value {
        json!({
            "id": format!("{word_id}:provider_speaker_index"),
            "word_id": word_id,
            "type": "provider_speaker_index",
            "value": json!({ "channel": channel, "speaker_index": speaker_index }).to_string(),
        })
    }

    fn assignment(kind: &str, word_id: &str, value: Value) -> Value {
        json!({
            "id": format!("{word_id}:{kind}"),
            "word_id": word_id,
            "type": kind,
            "value": value.to_string(),
        })
    }

    fn segment_assignment(word_id: &str, value: Value) -> Value {
        json!({
            "id": format!("{word_id}:user_speaker_assignment:segment"),
            "word_id": word_id,
            "type": "user_speaker_assignment",
            "value": value.to_string(),
        })
    }

    fn remote_speaker_key(speaker_index: Option<i32>) -> SegmentKey {
        SegmentKey {
            channel: ChannelProfile::RemoteParty,
            speaker_index,
            speaker_human_id: None,
        }
    }

    fn speaker_value(human_id: &str, channel: i64, speaker_index: i64) -> Value {
        json!({ "human_id": human_id, "scope": "speaker", "channel": channel, "speaker_index": speaker_index })
    }

    fn parsed(json: &str) -> Value {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn finds_the_matching_speaker_in_a_resumed_transcript() {
        let words = Value::Array(vec![
            word("speaker-1-word", "First", 0, 1),
            word("speaker-2-word", "Second", 100, 1),
        ])
        .to_string();
        let hints = Value::Array(vec![
            provider("speaker-1-word", 1, 1),
            provider("speaker-2-word", 1, 2),
        ])
        .to_string();
        assert_eq!(
            find_anchor_word_id(&words, &hints, &remote_speaker_key(Some(2))),
            Some("speaker-2-word".to_string())
        );
    }

    #[test]
    fn removes_a_conflicting_automatic_assignment_when_a_user_assigns_the_speaker() {
        let words = Value::Array(vec![word("word-1", " hello", 0, 1)]).to_string();
        let hints = Value::Array(vec![
            provider("word-1", 1, 2),
            assignment(
                "automatic_speaker_assignment",
                "word-1",
                json!({ "human_id": "alice" }),
            ),
        ])
        .to_string();
        let next = upsert_speaker_assignment(
            &words,
            &hints,
            &remote_speaker_key(Some(2)),
            "bob",
            "word-1",
            &Mode::All,
        );
        assert_eq!(
            parsed(&next),
            Value::Array(vec![
                provider("word-1", 1, 2),
                assignment(
                    "user_speaker_assignment",
                    "word-1",
                    speaker_value("bob", 1, 2)
                ),
            ])
        );
    }

    #[test]
    fn removes_a_stale_channel_wide_assignment_when_reassigning_a_speaker() {
        let words = Value::Array(vec![
            word("old-word", " hello", 0, 1),
            word("new-word", " there", 100, 1),
        ])
        .to_string();
        let hints = Value::Array(vec![
            assignment(
                "user_speaker_assignment",
                "old-word",
                json!({ "human_id": "alice" }),
            ),
            provider("new-word", 1, 2),
        ])
        .to_string();
        let next = upsert_speaker_assignment(
            &words,
            &hints,
            &remote_speaker_key(Some(2)),
            "bob",
            "new-word",
            &Mode::All,
        );
        assert_eq!(
            parsed(&next),
            Value::Array(vec![
                provider("new-word", 1, 2),
                assignment(
                    "user_speaker_assignment",
                    "new-word",
                    speaker_value("bob", 1, 2)
                ),
            ])
        );
    }

    #[test]
    fn keeps_other_speaker_assignments_on_the_same_channel() {
        let words = Value::Array(vec![
            word("speaker-1-word", " first", 0, 1),
            word("speaker-2-word-old", " second", 100, 1),
            word("speaker-2-word-new", " later", 200, 1),
        ])
        .to_string();
        let hints = Value::Array(vec![
            provider("speaker-1-word", 1, 1),
            assignment(
                "user_speaker_assignment",
                "speaker-1-word",
                json!({ "human_id": "alice" }),
            ),
            provider("speaker-2-word-old", 1, 2),
            assignment(
                "user_speaker_assignment",
                "speaker-2-word-old",
                json!({ "human_id": "bob" }),
            ),
            provider("speaker-2-word-new", 1, 2),
        ])
        .to_string();
        let next = upsert_speaker_assignment(
            &words,
            &hints,
            &remote_speaker_key(Some(2)),
            "carol",
            "speaker-2-word-new",
            &Mode::All,
        );
        assert_eq!(
            parsed(&next),
            Value::Array(vec![
                provider("speaker-1-word", 1, 1),
                assignment(
                    "user_speaker_assignment",
                    "speaker-1-word",
                    json!({ "human_id": "alice" }),
                ),
                provider("speaker-2-word-old", 1, 2),
                provider("speaker-2-word-new", 1, 2),
                assignment(
                    "user_speaker_assignment",
                    "speaker-2-word-new",
                    speaker_value("carol", 1, 2),
                ),
            ])
        );
    }

    #[test]
    fn stores_segment_only_assignments_with_the_selected_word_ids() {
        let words = Value::Array(vec![
            word("word-1", " first", 0, 0),
            word("word-2", " second", 100, 0),
        ])
        .to_string();
        let hints =
            Value::Array(vec![provider("word-1", 0, 2), provider("word-2", 0, 2)]).to_string();
        let next = upsert_speaker_assignment(
            &words,
            &hints,
            &SegmentKey {
                channel: ChannelProfile::DirectMic,
                speaker_index: Some(2),
                speaker_human_id: None,
            },
            "john",
            "word-1",
            &Mode::Segment {
                word_ids: vec!["word-1".into(), "word-2".into()],
            },
        );
        assert_eq!(
            parsed(&next),
            Value::Array(vec![
                provider("word-1", 0, 2),
                provider("word-2", 0, 2),
                segment_assignment(
                    "word-1",
                    json!({ "human_id": "john", "scope": "segment", "word_ids": ["word-1", "word-2"] }),
                ),
            ])
        );
        // The frontend's `JSON.stringify` key order, byte for byte.
        assert!(next.contains(
            r#"{"id":"word-1:user_speaker_assignment:segment","word_id":"word-1","type":"user_speaker_assignment","value":"{\"human_id\":\"john\",\"scope\":\"segment\",\"word_ids\":[\"word-1\",\"word-2\"]}"}"#
        ));
    }

    #[test]
    fn removes_segment_overrides_when_assigning_the_full_matching_speaker() {
        let words = Value::Array(vec![word("word-1", " first", 0, 1)]).to_string();
        let hints = Value::Array(vec![
            provider("word-1", 1, 2),
            segment_assignment(
                "word-1",
                json!({ "human_id": "alice", "scope": "segment", "word_ids": ["word-1"] }),
            ),
        ])
        .to_string();
        let next = upsert_speaker_assignment(
            &words,
            &hints,
            &remote_speaker_key(Some(2)),
            "bob",
            "word-1",
            &Mode::All,
        );
        assert_eq!(
            parsed(&next),
            Value::Array(vec![
                provider("word-1", 1, 2),
                assignment(
                    "user_speaker_assignment",
                    "word-1",
                    speaker_value("bob", 1, 2)
                ),
            ])
        );
    }

    #[test]
    fn keeps_segment_overrides_without_speaker_identity_when_assigning_a_specific_full_speaker() {
        let words = Value::Array(vec![
            word("word-1", " first", 0, 1),
            word("word-2", " second", 100, 1),
        ])
        .to_string();
        let hints = Value::Array(vec![
            provider("word-1", 1, 2),
            segment_assignment(
                "word-2",
                json!({ "human_id": "alice", "scope": "segment", "word_ids": ["word-2"] }),
            ),
        ])
        .to_string();
        let next = upsert_speaker_assignment(
            &words,
            &hints,
            &remote_speaker_key(Some(2)),
            "bob",
            "word-1",
            &Mode::All,
        );
        assert_eq!(
            parsed(&next),
            Value::Array(vec![
                provider("word-1", 1, 2),
                segment_assignment(
                    "word-2",
                    json!({ "human_id": "alice", "scope": "segment", "word_ids": ["word-2"] }),
                ),
                assignment(
                    "user_speaker_assignment",
                    "word-1",
                    speaker_value("bob", 1, 2)
                ),
            ])
        );
    }

    #[test]
    fn keeps_full_speaker_assignment_when_adding_a_segment_override() {
        let words = Value::Array(vec![word("word-1", " first", 0, 1)]).to_string();
        let hints = Value::Array(vec![
            provider("word-1", 1, 2),
            assignment(
                "user_speaker_assignment",
                "word-1",
                json!({ "human_id": "alice" }),
            ),
        ])
        .to_string();
        let next = upsert_speaker_assignment(
            &words,
            &hints,
            &remote_speaker_key(Some(2)),
            "bob",
            "word-1",
            &Mode::Segment {
                word_ids: vec!["word-1".into()],
            },
        );
        assert_eq!(
            parsed(&next),
            Value::Array(vec![
                provider("word-1", 1, 2),
                assignment(
                    "user_speaker_assignment",
                    "word-1",
                    json!({ "human_id": "alice" }),
                ),
                segment_assignment(
                    "word-1",
                    json!({ "human_id": "bob", "scope": "segment", "word_ids": ["word-1"] }),
                ),
            ])
        );
    }

    #[test]
    fn merge_assigns_contiguous_words_to_the_target_human() {
        let words = Value::Array(vec![
            word("word-1", " hello", 0, 1),
            word("word-2", " there", 150, 1),
        ])
        .to_string();
        let hints =
            Value::Array(vec![provider("word-1", 1, 0), provider("word-2", 1, 1)]).to_string();
        let next = merge_segment_assignments(
            &words,
            &hints,
            &SegmentKey {
                channel: ChannelProfile::RemoteParty,
                speaker_index: Some(0),
                speaker_human_id: Some("alice".into()),
            },
            &["word-1".to_string(), "word-2".to_string()],
        )
        .unwrap();
        assert_eq!(
            parsed(&next),
            Value::Array(vec![
                provider("word-1", 1, 0),
                provider("word-2", 1, 1),
                segment_assignment(
                    "word-1",
                    json!({ "human_id": "alice", "scope": "segment", "word_ids": ["word-1", "word-2"] }),
                ),
            ])
        );
    }

    #[test]
    fn merge_unifies_unlabeled_speaker_indexes_and_drops_overlapping_segment_assignments() {
        let words = Value::Array(vec![
            word("word-1", " hello", 0, 1),
            word("word-2", " there", 150, 1),
            word("word-3", " later", 400, 1),
        ])
        .to_string();
        let hints = Value::Array(vec![
            provider("word-1", 1, 0),
            provider("word-2", 1, 1),
            provider("word-3", 1, 1),
            segment_assignment(
                "word-2",
                json!({ "human_id": "bob", "scope": "segment", "word_ids": ["word-2"] }),
            ),
            assignment(
                "user_speaker_assignment",
                "word-3",
                speaker_value("carol", 1, 1),
            ),
        ])
        .to_string();
        let next = merge_segment_assignments(
            &words,
            &hints,
            &remote_speaker_key(Some(0)),
            &["word-1".to_string(), "word-2".to_string()],
        )
        .unwrap();
        assert_eq!(
            parsed(&next),
            Value::Array(vec![
                provider("word-1", 1, 0),
                provider("word-2", 1, 0),
                provider("word-3", 1, 1),
                assignment(
                    "user_speaker_assignment",
                    "word-3",
                    speaker_value("carol", 1, 1),
                ),
            ])
        );
    }

    #[test]
    fn update_segment_text_hands_tokens_to_words_in_order() {
        let words = Value::Array(vec![
            word("a", "Good", 0, 0),
            word("b", " morning", 100, 0),
            word("c", " everyone", 200, 0),
            word("d", " unrelated", 300, 0),
        ])
        .to_string();
        let next = update_segment_text(
            &words,
            &["a".into(), "b".into(), "c".into()],
            "  Good   evening  to you all ",
        )
        .unwrap();
        let parsed = parse_array(&next);
        let texts: Vec<&str> = parsed.iter().map(|word| text_field(word, "text")).collect();
        assert_eq!(texts, ["Good", "evening", "to you all", " unrelated"]);
        // Untouched fields keep their order (`{ ...word, text }`).
        assert!(
            next.starts_with(r#"[{"id":"a","text":"Good","start_ms":0,"end_ms":100,"channel":0}"#)
        );
        // Fewer tokens than words empties the trailing ones.
        let next = update_segment_text(&words, &["a".into(), "b".into()], "Hi").unwrap();
        let parsed = parse_array(&next);
        assert_eq!(text_field(&parsed[0], "text"), "Hi");
        assert_eq!(text_field(&parsed[1], "text"), "");
        assert!(update_segment_text(&words, &["zzz".into()], "x").is_none());
    }
}
