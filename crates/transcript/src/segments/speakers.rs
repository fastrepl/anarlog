use std::collections::HashMap;

use crate::types::{ChannelProfile, IdentityAssignment, IdentityScope};
use crate::types::{SegmentBuilderOptions, SegmentKey};

use super::model::{
    NormalizedWord, ProtoSegment, ResolvedWordFrame, SpeakerIdentity, SpeakerState,
};

pub(super) fn create_speaker_state(
    assignments: &[IdentityAssignment],
    normalized_words: &[NormalizedWord],
    options: Option<&SegmentBuilderOptions>,
) -> SpeakerState {
    let complete_channels = options
        .and_then(|opts| opts.complete_channels.clone())
        .or_else(|| SegmentBuilderOptions::default().complete_channels)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let mut assignment_by_word_index: HashMap<usize, SpeakerIdentity> = HashMap::new();
    let mut human_id_by_scoped_speaker: HashMap<(ChannelProfile, i32), String> = HashMap::new();

    // Populate human_id_by_scoped_speaker from assignments
    for assignment in assignments {
        if let IdentityScope::ChannelSpeaker {
            channel,
            speaker_index,
        } = assignment.scope
        {
            human_id_by_scoped_speaker
                .insert((channel, speaker_index), assignment.human_id.clone());
        }
    }

    // Build per-word identities from word.speaker_index + assignment cross-reference
    for word in normalized_words {
        if let Some(speaker_index) = word.speaker_index {
            let entry = assignment_by_word_index.entry(word.order).or_default();
            entry.speaker_index = Some(speaker_index);

            if let Some(human_id) = human_id_by_scoped_speaker.get(&(word.channel, speaker_index)) {
                entry.human_id = Some(human_id.clone());
            }
        }
    }

    // Channel-scoped assignments (no speaker_index) apply to entire channel
    let mut human_id_by_channel: HashMap<ChannelProfile, String> = HashMap::new();
    for assignment in assignments {
        if let IdentityScope::Channel { channel } = assignment.scope {
            human_id_by_channel.insert(channel, assignment.human_id.clone());
        }
    }

    SpeakerState {
        assignment_by_word_index,
        human_id_by_scoped_speaker,
        human_id_by_channel,
        last_speaker_by_channel: HashMap::new(),
        complete_channels,
    }
}

pub(super) fn resolve_identities(
    words: &[NormalizedWord],
    speaker_state: &mut SpeakerState,
) -> Vec<ResolvedWordFrame> {
    words
        .iter()
        .map(|word| {
            let assignment = speaker_state
                .assignment_by_word_index
                .get(&word.order)
                .cloned();
            let identity = apply_identity_rules(word, assignment.as_ref(), speaker_state);
            remember_identity(word, assignment.as_ref(), &identity, speaker_state);

            ResolvedWordFrame {
                word: word.clone(),
                identity: (!identity.is_empty()).then_some(identity),
            }
        })
        .collect()
}

pub(super) fn assign_complete_channel_human_id(segment: &mut ProtoSegment, state: &SpeakerState) {
    if segment.key.speaker_human_id.is_some() {
        return;
    }

    let channel = segment.key.channel;
    if !state.complete_channels.contains(&channel) {
        return;
    }

    if let Some(human_id) = state.human_id_by_channel.get(&channel) {
        segment.key = SegmentKey {
            channel,
            speaker_index: segment.key.speaker_index,
            speaker_human_id: Some(human_id.clone()),
        };
    }
}

fn apply_identity_rules(
    word: &NormalizedWord,
    assignment: Option<&SpeakerIdentity>,
    state: &SpeakerState,
) -> SpeakerIdentity {
    let mut identity = assignment.cloned().unwrap_or_default();

    if let (Some(speaker_index), None) = (identity.speaker_index, &identity.human_id)
        && let Some(human_id) = state
            .human_id_by_scoped_speaker
            .get(&(word.channel, speaker_index))
    {
        identity.human_id = Some(human_id.clone());
    }

    if identity.human_id.is_none()
        && state.complete_channels.contains(&word.channel)
        && let Some(human_id) = state.human_id_by_channel.get(&word.channel)
    {
        identity.human_id = Some(human_id.clone());
    }

    if !(word.is_final || identity.speaker_index.is_some() && identity.human_id.is_some())
        && let Some(last) = state.last_speaker_by_channel.get(&word.channel)
    {
        if identity.speaker_index.is_none() {
            identity.speaker_index = last.speaker_index;
        }
        if identity.human_id.is_none() {
            identity.human_id = last.human_id.clone();
        }
    }

    identity
}

fn remember_identity(
    word: &NormalizedWord,
    assignment: Option<&SpeakerIdentity>,
    identity: &SpeakerIdentity,
    state: &mut SpeakerState,
) {
    let has_explicit_assignment = assignment
        .map(|value| value.speaker_index.is_some() || value.human_id.is_some())
        .unwrap_or(false);

    if let (Some(speaker_index), Some(human_id)) = (identity.speaker_index, &identity.human_id) {
        state
            .human_id_by_scoped_speaker
            .insert((word.channel, speaker_index), human_id.clone());
    }

    if state.complete_channels.contains(&word.channel)
        && identity.speaker_index.is_none()
        && let Some(human_id) = identity.human_id.clone()
    {
        state.human_id_by_channel.insert(word.channel, human_id);
    }

    if (!word.is_final || identity.speaker_index.is_some() || has_explicit_assignment)
        && !identity.is_empty()
    {
        state
            .last_speaker_by_channel
            .insert(word.channel, identity.clone());
    }
}
