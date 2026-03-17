use std::collections::HashMap;

use crate::types::{
    RuntimeSpeakerHint, SegmentBuilderOptions, SegmentKey, SpeakerHintData, WordRef,
};

use super::model::{
    NormalizedWord, ProtoSegment, ResolvedWordFrame, SpeakerIdentity, SpeakerState,
};

pub(super) fn create_speaker_state(
    speaker_hints: &[RuntimeSpeakerHint],
    normalized_words: &[NormalizedWord],
    options: Option<&SegmentBuilderOptions>,
) -> SpeakerState {
    let complete_channels = options
        .and_then(|opts| opts.complete_channels.clone())
        .or_else(|| SegmentBuilderOptions::default().complete_channels)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let id_to_index: HashMap<&str, usize> = normalized_words
        .iter()
        .filter_map(|word| word.id.as_deref().map(|id| (id, word.order)))
        .collect();

    let mut assignment_by_word_index: HashMap<usize, SpeakerIdentity> = HashMap::new();
    let mut human_id_by_speaker_index: HashMap<i32, String> = HashMap::new();

    for hint in speaker_hints {
        if let Some(word_index) = resolve_hint_target(hint, normalized_words.len(), &id_to_index) {
            let entry = assignment_by_word_index.entry(word_index).or_default();

            match &hint.data {
                SpeakerHintData::ProviderSpeakerIndex { speaker_index, .. } => {
                    entry.speaker_index = Some(*speaker_index);
                }
                SpeakerHintData::UserSpeakerAssignment { human_id } => {
                    entry.human_id = Some(human_id.clone());
                }
            }

            if let (Some(speaker_index), Some(human_id)) =
                (entry.speaker_index, entry.human_id.as_ref())
            {
                human_id_by_speaker_index.insert(speaker_index, human_id.clone());
            }
        }
    }

    SpeakerState {
        assignment_by_word_index,
        human_id_by_speaker_index,
        human_id_by_channel: HashMap::new(),
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

fn resolve_hint_target(
    hint: &RuntimeSpeakerHint,
    words_len: usize,
    id_to_index: &HashMap<&str, usize>,
) -> Option<usize> {
    match &hint.target {
        WordRef::FinalWordId(word_id) => id_to_index.get(word_id.as_str()).copied(),
        WordRef::RuntimeIndex(index) if *index < words_len => Some(*index),
        WordRef::RuntimeIndex(_) => None,
    }
}

fn apply_identity_rules(
    word: &NormalizedWord,
    assignment: Option<&SpeakerIdentity>,
    state: &SpeakerState,
) -> SpeakerIdentity {
    let mut identity = assignment.cloned().unwrap_or_default();

    if identity.speaker_index.is_some() && identity.human_id.is_none() {
        if let Some(speaker_index) = identity.speaker_index {
            if let Some(human_id) = state.human_id_by_speaker_index.get(&speaker_index) {
                identity.human_id = Some(human_id.clone());
            }
        }
    }

    if identity.human_id.is_none() && state.complete_channels.contains(&word.channel) {
        if let Some(human_id) = state.human_id_by_channel.get(&word.channel) {
            identity.human_id = Some(human_id.clone());
        }
    }

    if !word.is_final && !(identity.speaker_index.is_some() && identity.human_id.is_some()) {
        if let Some(last) = state.last_speaker_by_channel.get(&word.channel) {
            if identity.speaker_index.is_none() {
                identity.speaker_index = last.speaker_index;
            }
            if identity.human_id.is_none() {
                identity.human_id = last.human_id.clone();
            }
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
            .human_id_by_speaker_index
            .insert(speaker_index, human_id.clone());
    }

    if state.complete_channels.contains(&word.channel)
        && identity.human_id.is_some()
        && identity.speaker_index.is_none()
    {
        if let Some(human_id) = identity.human_id.clone() {
            state.human_id_by_channel.insert(word.channel, human_id);
        }
    }

    if (!word.is_final || identity.speaker_index.is_some() || has_explicit_assignment)
        && !identity.is_empty()
    {
        state
            .last_speaker_by_channel
            .insert(word.channel, identity.clone());
    }
}
