use std::collections::{HashMap, HashSet};

use crate::types::{ChannelProfile, SegmentKey};

#[derive(Debug, Clone)]
pub(super) struct NormalizedWord {
    pub(super) text: String,
    pub(super) start_ms: i64,
    pub(super) end_ms: i64,
    pub(super) channel: ChannelProfile,
    pub(super) is_final: bool,
    pub(super) id: Option<String>,
    pub(super) order: usize,
    pub(super) speaker_index: Option<i32>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct SpeakerIdentity {
    pub(super) speaker_index: Option<i32>,
    pub(super) human_id: Option<String>,
}

impl SpeakerIdentity {
    pub(super) fn is_empty(&self) -> bool {
        self.speaker_index.is_none() && self.human_id.is_none()
    }
}

#[derive(Debug)]
pub(super) struct ResolvedWordFrame {
    pub(super) word: NormalizedWord,
    pub(super) identity: Option<SpeakerIdentity>,
}

#[derive(Debug)]
pub(super) struct ProtoSegment {
    pub(super) key: SegmentKey,
    pub(super) words: Vec<ResolvedWordFrame>,
}

pub(super) struct SpeakerState {
    pub(super) assignment_by_word_index: HashMap<usize, SpeakerIdentity>,
    pub(super) human_id_by_scoped_speaker: HashMap<(ChannelProfile, i32), String>,
    pub(super) human_id_by_channel: HashMap<ChannelProfile, String>,
    pub(super) last_speaker_by_channel: HashMap<ChannelProfile, SpeakerIdentity>,
    pub(super) complete_channels: HashSet<ChannelProfile>,
}
