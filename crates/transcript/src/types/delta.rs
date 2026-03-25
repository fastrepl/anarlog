use super::word::{FinalizedWord, PartialWord};

/// Delta emitted after processing.
///
/// 1. Remove words listed in `replaced_ids`
/// 2. Persist `new_words` (honoring `state`)
/// 3. Store `partials` in ephemeral state for rendering
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct TranscriptDelta {
    pub new_words: Vec<FinalizedWord>,
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
