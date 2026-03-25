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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_index: Option<i32>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_index: Option<i32>,
}
