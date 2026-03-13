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
pub struct SpeakerHint {
    pub word_id: String,
    pub speaker_index: i32,
}

/// Delta emitted to the frontend after processing.
///
/// The frontend should:
/// 1. Remove words listed in `replaced_ids` from TinyBase
/// 2. Persist `new_words` to TinyBase (honoring `state`)
/// 3. Store `partials` in ephemeral Zustand state for rendering
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
    pub hints: Vec<SpeakerHint>,
    /// IDs of words superseded by `new_words`. Empty for normal finalization.
    pub replaced_ids: Vec<String>,
    /// Current in-progress words across all channels. Global snapshot.
    pub partials: Vec<PartialWord>,
}

impl TranscriptDelta {
    pub fn is_empty(&self) -> bool {
        self.new_words.is_empty() && self.replaced_ids.is_empty() && self.partials.is_empty()
    }
}
