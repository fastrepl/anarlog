use hypr_transcript::{FinalizedWord, SpeakerHintData};

pub struct TranscriptDeltaPersist {
    pub new_words: Vec<FinalizedWord>,
    pub hints: Vec<PersistableSpeakerHint>,
    pub replaced_ids: Vec<String>,
}

pub struct PersistableSpeakerHint {
    pub word_id: String,
    pub data: SpeakerHintData,
}
