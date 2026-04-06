use hypr_transcript::FinalizedWord;

pub enum StorageSpeakerHintData {
    ProviderSpeakerIndex {
        speaker_index: i32,
        provider: Option<String>,
        channel: Option<i32>,
    },
    UserSpeakerAssignment {
        human_id: String,
    },
}

pub struct StorageSpeakerHint {
    pub word_id: String,
    pub data: StorageSpeakerHintData,
}

pub struct TranscriptDeltaPersist {
    pub new_words: Vec<FinalizedWord>,
    pub speaker_hints: Vec<StorageSpeakerHint>,
    pub replaced_ids: Vec<String>,
}
