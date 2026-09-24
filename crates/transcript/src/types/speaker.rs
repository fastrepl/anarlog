use std::collections::{BTreeMap, HashSet};

use super::segment::{ChannelProfile, SegmentBuilderOptions};
use super::word::FinalizedWord;

/// A point sample from a meeting platform naming who is talking at `at_ms`.
/// Each sample holds until the next one; an empty list means silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSpeakerSample {
    pub at_ms: i64,
    pub human_ids: Vec<String>,
}

/// Attributes each word to the single participant the platform reported as
/// talking for the largest share of the word's duration. Words that overlap
/// no sample, or only silence, are left unassigned.
pub fn word_assignments_from_active_speakers(
    words: &[FinalizedWord],
    samples: &[ActiveSpeakerSample],
) -> Vec<IdentityAssignment> {
    if samples.is_empty() {
        return vec![];
    }
    let mut ordered: Vec<&ActiveSpeakerSample> = samples.iter().collect();
    ordered.sort_by_key(|sample| sample.at_ms);

    let mut by_human: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for word in words {
        let end = word.end_ms.max(word.start_ms + 1);
        let mut coverage: BTreeMap<&str, i64> = BTreeMap::new();
        for (index, sample) in ordered.iter().enumerate() {
            let sample_end = ordered
                .get(index + 1)
                .map(|next| next.at_ms)
                .unwrap_or(i64::MAX);
            let overlap = sample_end.min(end) - sample.at_ms.max(word.start_ms);
            if overlap <= 0 {
                continue;
            }
            for human_id in &sample.human_ids {
                if !human_id.is_empty() {
                    *coverage.entry(human_id.as_str()).or_default() += overlap;
                }
            }
        }
        if let Some((human_id, _)) = coverage.into_iter().max_by_key(|(_, ms)| *ms) {
            by_human.entry(human_id).or_default().push(word.id.clone());
        }
    }

    by_human
        .into_iter()
        .map(|(human_id, word_ids)| IdentityAssignment {
            human_id: human_id.to_string(),
            scope: IdentityScope::Words { word_ids },
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IdentityScope {
    Channel {
        channel: ChannelProfile,
    },
    ChannelSpeaker {
        channel: ChannelProfile,
        speaker_index: i32,
    },
    Words {
        word_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct IdentityAssignment {
    pub human_id: String,
    pub scope: IdentityScope,
}

pub fn channel_assignments_for_participants(
    participant_human_ids: &[String],
    self_human_id: Option<&str>,
) -> Vec<IdentityAssignment> {
    let self_id = match self_human_id {
        Some(id) if !id.is_empty() => id,
        _ => return vec![],
    };

    let mut assignments = vec![IdentityAssignment {
        human_id: self_id.to_string(),
        scope: IdentityScope::Channel {
            channel: ChannelProfile::DirectMic,
        },
    }];

    if let Some(remote_id) = unique_other_participant(participant_human_ids, self_id) {
        assignments.push(IdentityAssignment {
            human_id: remote_id.to_string(),
            scope: IdentityScope::Channel {
                channel: ChannelProfile::RemoteParty,
            },
        });
    }

    assignments
}

pub fn segment_options_for_participants(
    participant_human_ids: &[String],
    self_human_id: Option<&str>,
) -> SegmentBuilderOptions {
    let mut unique_participants: HashSet<&str> = participant_human_ids
        .iter()
        .map(|s| s.as_str())
        .filter(|human_id| !human_id.is_empty())
        .collect();

    if let Some(self_id) = self_human_id
        && !self_id.is_empty()
    {
        unique_participants.insert(self_id);
    }

    let mut complete_channels = vec![ChannelProfile::DirectMic];
    if unique_participants.len() == 2 {
        complete_channels.push(ChannelProfile::RemoteParty);
    }

    SegmentBuilderOptions {
        complete_channels: Some(complete_channels),
        ..Default::default()
    }
}

fn unique_other_participant<'a>(
    participant_human_ids: &'a [String],
    self_human_id: &str,
) -> Option<&'a str> {
    let others: Vec<&str> = participant_human_ids
        .iter()
        .map(|s| s.as_str())
        .filter(|&id| !id.is_empty() && id != self_human_id)
        .collect();

    if others.len() == 1 {
        Some(others[0])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::word::WordState;

    fn word(id: &str, start_ms: i64, end_ms: i64) -> FinalizedWord {
        FinalizedWord {
            id: id.into(),
            text: id.into(),
            start_ms,
            end_ms,
            channel: 1,
            state: WordState::Final,
            speaker_index: None,
        }
    }

    #[test]
    fn attributes_words_to_dominant_active_speaker() {
        let samples = [
            ActiveSpeakerSample {
                at_ms: 0,
                human_ids: vec!["alice".into()],
            },
            ActiveSpeakerSample {
                at_ms: 1000,
                human_ids: vec!["bob".into()],
            },
            ActiveSpeakerSample {
                at_ms: 2000,
                human_ids: vec![],
            },
        ];
        let words = [
            word("w1", 100, 600),
            word("w2", 900, 1400),
            word("w3", 1500, 1900),
            word("w4", 2500, 2800),
        ];

        let assignments = word_assignments_from_active_speakers(&words, &samples);

        assert_eq!(
            assignments,
            vec![
                IdentityAssignment {
                    human_id: "alice".into(),
                    scope: IdentityScope::Words {
                        word_ids: vec!["w1".into()],
                    },
                },
                IdentityAssignment {
                    human_id: "bob".into(),
                    scope: IdentityScope::Words {
                        word_ids: vec!["w2".into(), "w3".into()],
                    },
                },
            ]
        );
        assert!(word_assignments_from_active_speakers(&words, &[]).is_empty());
    }

    #[test]
    fn assigns_self_to_direct_mic_without_unique_remote() {
        let assignments = channel_assignments_for_participants(&[], Some("self"));

        assert_eq!(
            assignments,
            vec![IdentityAssignment {
                human_id: "self".to_string(),
                scope: IdentityScope::Channel {
                    channel: ChannelProfile::DirectMic,
                },
            }]
        );
    }

    #[test]
    fn assigns_unique_remote_to_remote_party() {
        let assignments = channel_assignments_for_participants(
            &["self".to_string(), "remote".to_string()],
            Some("self"),
        );

        assert_eq!(
            assignments,
            vec![
                IdentityAssignment {
                    human_id: "self".to_string(),
                    scope: IdentityScope::Channel {
                        channel: ChannelProfile::DirectMic,
                    },
                },
                IdentityAssignment {
                    human_id: "remote".to_string(),
                    scope: IdentityScope::Channel {
                        channel: ChannelProfile::RemoteParty,
                    },
                },
            ]
        );
    }

    #[test]
    fn skips_remote_assignment_when_remote_is_ambiguous() {
        let assignments = channel_assignments_for_participants(
            &["remote-a".to_string(), "remote-b".to_string()],
            Some("self"),
        );

        assert_eq!(
            assignments,
            vec![IdentityAssignment {
                human_id: "self".to_string(),
                scope: IdentityScope::Channel {
                    channel: ChannelProfile::DirectMic,
                },
            }]
        );
    }
}
