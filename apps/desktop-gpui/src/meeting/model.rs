use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use anlg_listener_core::LiveTranscriptDelta;
use anlg_transcript::{
    IdentityAssignment, IdentityScope, RenderTranscriptInput, RenderTranscriptWordInput,
};
use desktop_runtime::{Result, ServiceError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const MAX_WORDS: usize = 10_000;
pub const MAX_TEXT: usize = 1_000_000;
pub const MAX_SNAPSHOT_BYTES: usize = 32 * 1024 * 1024;

pub fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Word {
    pub id: String,
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    #[serde(default)]
    pub channel: i32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Word {
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() || self.end_ms < self.start_ms {
            return Err(failure("Invalid transcript word; original data preserved."));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Transcript {
    pub id: Arc<str>,
    pub started_at: i64,
    pub revision: i64,
    pub sequence: i64,
    pub words: Vec<Word>,
    pub hints: Vec<Value>,
}

impl Transcript {
    pub fn apply(&mut self, delta: &LiveTranscriptDelta) -> Result<()> {
        validate_delta(delta)?;
        let replaced: HashSet<&str> = delta
            .replaced_ids
            .iter()
            .map(String::as_str)
            .chain(delta.new_words.iter().map(|word| word.id.as_str()))
            .collect();
        let previous: HashMap<&str, &Word> = self
            .words
            .iter()
            .map(|word| (word.id.as_str(), word))
            .collect();
        for hint in &mut self.hints {
            if hint["type"] != "user_speaker_assignment"
                && hint["type"] != "automatic_speaker_assignment"
            {
                continue;
            }
            let mut value = hint_value(hint)?;
            if value["scope"] != "segment" {
                continue;
            }
            let Some(ids) = value["word_ids"].as_array() else {
                return Err(failure("Malformed segment speaker assignment."));
            };
            let mut next: HashSet<String> = ids
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            for id in ids.iter().filter_map(Value::as_str) {
                if !replaced.contains(id) {
                    continue;
                }
                next.remove(id);
                if let Some(old) = previous.get(id) {
                    for new in &delta.new_words {
                        if new.channel == old.channel
                            && new.start_ms < old.end_ms
                            && new.end_ms > old.start_ms
                        {
                            next.insert(new.id.clone());
                        }
                    }
                }
            }
            let mut ids: Vec<_> = next.into_iter().collect();
            ids.sort();
            value["word_ids"] = json!(ids);
            hint["value"] = Value::String(value.to_string());
        }
        let extras: HashMap<_, _> = delta
            .new_words
            .iter()
            .filter_map(|word| {
                previous
                    .get(word.id.as_str())
                    .map(|previous| (word.id.clone(), previous.extra.clone()))
            })
            .collect();
        self.words
            .retain(|word| !replaced.contains(word.id.as_str()));
        self.hints.retain(|hint| {
            hint["type"] != "provider_speaker_index"
                || !hint["word_id"]
                    .as_str()
                    .is_some_and(|id| replaced.contains(id))
        });
        for word in &delta.new_words {
            self.words.push(Word {
                id: word.id.clone(),
                text: word.text.clone(),
                start_ms: word.start_ms,
                end_ms: word.end_ms,
                channel: word.channel,
                extra: extras.get(&word.id).cloned().unwrap_or_default(),
            });
            if let Some(speaker_index) = word.speaker_index {
                self.hints.push(json!({
                    "id": format!("{}:provider_speaker_index", word.id),
                    "word_id": word.id,
                    "type": "provider_speaker_index",
                    "value": json!({"channel": word.channel, "speaker_index": speaker_index}).to_string(),
                }));
            }
        }
        self.words.sort_by_key(|word| word.start_ms);
        Ok(())
    }

    pub fn render_input(&self) -> Result<RenderTranscriptInput> {
        let speakers = self
            .hints
            .iter()
            .filter(|hint| hint["type"] == "provider_speaker_index")
            .map(|hint| {
                let value = hint_value(hint)?;
                Ok((
                    hint["word_id"].as_str().unwrap_or_default().to_owned(),
                    value["speaker_index"].as_i64().map(|index| index as i32),
                ))
            })
            .collect::<Result<HashMap<_, _>>>()?;
        let mut assignments = Vec::new();
        for kind in ["automatic_speaker_assignment", "user_speaker_assignment"] {
            for hint in self.hints.iter().filter(|hint| hint["type"] == kind) {
                let value = hint_value(hint)?;
                let human_id = value["human_id"].as_str().ok_or_else(|| {
                    failure("Malformed speaker assignment; original data preserved.")
                })?;
                let anchor = self
                    .words
                    .iter()
                    .find(|word| Some(word.id.as_str()) == hint["word_id"].as_str());
                let scope = if value["scope"] == "segment" {
                    IdentityScope::Words {
                        word_ids: serde_json::from_value(value["word_ids"].clone())
                            .map_err(failure)?,
                    }
                } else {
                    let channel = value["channel"]
                        .as_i64()
                        .map(|channel| channel as i32)
                        .or_else(|| anchor.map(|word| word.channel));
                    let speaker = value["speaker_index"]
                        .as_i64()
                        .map(|speaker| speaker as i32)
                        .or_else(|| {
                            anchor.and_then(|word| speakers.get(&word.id).copied().flatten())
                        });
                    match (channel, speaker) {
                        (Some(channel), Some(speaker_index)) => IdentityScope::ChannelSpeaker {
                            channel: channel.into(),
                            speaker_index,
                        },
                        (Some(channel), None) => IdentityScope::Channel {
                            channel: channel.into(),
                        },
                        _ => continue,
                    }
                };
                assignments.push(IdentityAssignment {
                    human_id: human_id.to_owned(),
                    scope,
                });
            }
        }
        Ok(RenderTranscriptInput {
            started_at: Some(self.started_at),
            words: self
                .words
                .iter()
                .map(|word| RenderTranscriptWordInput {
                    id: word.id.clone(),
                    text: word.text.clone(),
                    start_ms: word.start_ms,
                    end_ms: word.end_ms,
                    channel: word.channel,
                    speaker_index: speakers.get(&word.id).copied().flatten(),
                    // Live capture words carry real timing; synthetic timing only comes from imports.
                    synthetic_timing: None,
                })
                .collect(),
            assignments,
        })
    }
}

pub fn hint_value(hint: &Value) -> Result<Value> {
    match &hint["value"] {
        Value::String(value) => serde_json::from_str(value).map_err(failure),
        value if value.is_object() => Ok(value.clone()),
        _ => Err(failure("Malformed speaker hint; original data preserved.")),
    }
}

pub fn validate_delta(delta: &LiveTranscriptDelta) -> Result<()> {
    if delta.new_words.len() > MAX_WORDS
        || delta.replaced_ids.len() > MAX_WORDS
        || delta
            .new_words
            .iter()
            .map(|word| word.text.len() + word.id.len())
            .sum::<usize>()
            + delta.replaced_ids.iter().map(String::len).sum::<usize>()
            > MAX_TEXT
    {
        return Err(failure(
            "Transcript backlog exceeded its safe memory bounds.",
        ));
    }
    if delta
        .new_words
        .iter()
        .any(|word| word.id.is_empty() || word.end_ms < word.start_ms)
    {
        return Err(failure("Invalid live transcript word."));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Interval {
    pub start: i64,
    pub end: i64,
}

pub fn recovered_additions(
    recovered: Vec<Word>,
    before: &[Word],
    current: &[Word],
    gaps: &[Interval],
) -> Vec<Word> {
    let mut protected: HashMap<i32, Vec<&Word>> = HashMap::new();
    let mut ids = HashSet::new();
    for word in before.iter().chain(current) {
        protected.entry(word.channel).or_default().push(word);
        ids.insert(word.id.as_str());
    }
    for words in protected.values_mut() {
        words.sort_by_key(|word| word.start_ms);
    }
    let mut accepted = HashSet::new();
    recovered
        .into_iter()
        .filter(|word| {
            word.validate().is_ok()
                && accepted.insert(word.id.clone())
                && !ids.contains(word.id.as_str())
                && gaps.iter().any(|gap| {
                    word.start_ms < gap.end
                        && (word.end_ms > gap.start
                            || (word.start_ms == word.end_ms && word.start_ms >= gap.start))
                })
                && !protected.get(&word.channel).is_some_and(|words| {
                    let end = words
                        .partition_point(|saved| saved.start_ms < word.end_ms.saturating_add(150));
                    words[..end]
                        .iter()
                        .any(|saved| saved.end_ms > word.start_ms.saturating_sub(150))
                })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Retention {
    Never,
    Days(u32),
    Forever,
}

impl Retention {
    pub fn from_setting(value: &Value) -> Result<Self> {
        match value {
            Value::Bool(false) => Ok(Self::Never),
            Value::Bool(true) => Ok(Self::Forever),
            Value::String(value) => match value.as_str() {
                "none" => Ok(Self::Never),
                "oneDay" => Ok(Self::Days(1)),
                "threeDays" => Ok(Self::Days(3)),
                "oneWeek" => Ok(Self::Days(7)),
                "oneMonth" => Ok(Self::Days(30)),
                "forever" => Ok(Self::Forever),
                _ => Err(failure("Unknown audio retention policy.")),
            },
            _ => Err(failure("Invalid audio retention policy.")),
        }
    }
}
