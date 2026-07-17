pub(super) struct TranscriptRaw {
    pub id: String,
    pub user_id: String,
    pub created_at: String,
    pub session_id: String,
    pub started_at: f64,
    pub ended_at: Option<f64>,
    pub inline_words: Option<String>,
    pub inline_hints: Option<String>,
}

#[derive(Clone)]
pub(super) struct WordWithTranscript {
    pub transcript_id: String,
    pub word: crate::types::Word,
}

pub(super) struct SpeakerHintRaw {
    pub word_id: String,
    pub hint_type: String,
    pub value: String,
}
