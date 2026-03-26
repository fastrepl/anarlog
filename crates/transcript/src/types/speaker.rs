use std::collections::HashSet;

use super::segment::{ChannelProfile, SegmentBuilderOptions};

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

    let remote_id = unique_other_participant(participant_human_ids, self_id);
    let remote_id = match remote_id {
        Some(id) => id,
        None => return vec![],
    };

    vec![
        IdentityAssignment {
            human_id: self_id.to_string(),
            scope: IdentityScope::Channel {
                channel: ChannelProfile::DirectMic,
            },
        },
        IdentityAssignment {
            human_id: remote_id.to_string(),
            scope: IdentityScope::Channel {
                channel: ChannelProfile::RemoteParty,
            },
        },
    ]
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
