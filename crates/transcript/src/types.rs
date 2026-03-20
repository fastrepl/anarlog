/// Pre-finalization word data from the ASR pipeline, before ID assignment.
#[derive(Debug, Clone)]
pub struct RawWord {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub channel: i32,
    pub speaker: Option<i32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PartialWord {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub channel: i32,
}

/// Whether a finalized word is stable or awaiting correction.
///
/// A word is `Pending` when it has been confirmed by the STT model but a
/// correction source (cloud STT fallback, LLM postprocessor, etc.) is still
/// processing it. The word has an ID and is persisted, but its text may be
/// replaced when the correction resolves via `TranscriptDelta::replaced_ids`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum WordState {
    Final,
    Pending,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FinalizedWord {
    pub id: String,
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub channel: i32,
    pub state: WordState,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerHintData {
    ProviderSpeakerIndex {
        speaker_index: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        provider: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        channel: Option<i32>,
    },
    UserSpeakerAssignment {
        human_id: String,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum WordRef {
    FinalWordId(String),
    RuntimeIndex(usize),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RuntimeSpeakerHint {
    pub target: WordRef,
    pub data: SpeakerHintData,
}

/// Delta emitted to the frontend after processing.
///
/// The frontend should:
/// 1. Remove words listed in `replaced_ids` from TinyBase
/// 2. Persist `new_words` to TinyBase (honoring `state`)
/// 3. Store `partials` and `partial_hints` in ephemeral state for rendering
///
/// This shape handles all correction flows uniformly:
/// - Normal finalization: `new_words` with `Final`, empty `replaced_ids`
/// - Pending correction submitted: `new_words` with `Pending`, `replaced_ids`
///   pointing at the same words' previous `Final` versions
/// - Correction resolved: `new_words` with `Final` (corrected text),
///   `replaced_ids` pointing at the `Pending` versions
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct TranscriptDelta {
    pub new_words: Vec<FinalizedWord>,
    pub hints: Vec<RuntimeSpeakerHint>,
    /// IDs of words superseded by `new_words`. Empty for normal finalization.
    pub replaced_ids: Vec<String>,
    /// Current in-progress words across all channels. Global snapshot.
    pub partials: Vec<PartialWord>,
    /// Speaker hints for `partials`, indexed relative to the `partials` snapshot.
    pub partial_hints: Vec<RuntimeSpeakerHint>,
}

impl TranscriptDelta {
    pub fn is_empty(&self) -> bool {
        self.new_words.is_empty()
            && self.replaced_ids.is_empty()
            && self.partials.is_empty()
            && self.partial_hints.is_empty()
    }
}

// --- Segment types ---

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[repr(i32)]
pub enum ChannelProfile {
    DirectMic = 0,
    RemoteParty = 1,
    MixedCapture = 2,
}

impl From<i32> for ChannelProfile {
    fn from(value: i32) -> Self {
        match value {
            0 => ChannelProfile::DirectMic,
            1 => ChannelProfile::RemoteParty,
            2 => ChannelProfile::MixedCapture,
            _ => ChannelProfile::MixedCapture,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SegmentKey {
    pub channel: ChannelProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_human_id: Option<String>,
}

impl SegmentKey {
    pub fn has_speaker_identity(&self) -> bool {
        self.speaker_index.is_some() || self.speaker_human_id.is_some()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SegmentWord {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub channel: ChannelProfile,
    pub is_final: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Segment {
    pub key: SegmentKey,
    pub words: Vec<SegmentWord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SegmentBuilderOptions {
    pub max_gap_ms: Option<i64>,
    pub complete_channels: Option<Vec<ChannelProfile>>,
}

impl Default for SegmentBuilderOptions {
    fn default() -> Self {
        Self {
            max_gap_ms: None,
            complete_channels: Some(vec![ChannelProfile::DirectMic]),
        }
    }
}
