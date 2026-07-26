use uuid::Uuid;

use crate::types::{FinalizedWord, PartialWord, RawWord, WordState};

pub(crate) fn to_partial(word: &RawWord) -> PartialWord {
    PartialWord {
        text: word.text.clone(),
        start_ms: word.start_ms,
        end_ms: word.end_ms,
        channel: word.channel,
        speaker_index: word.speaker,
    }
}

pub(crate) fn finalize_words(words: Vec<RawWord>, state: WordState) -> Vec<FinalizedWord> {
    words
        .into_iter()
        .map(|word| FinalizedWord {
            id: Uuid::new_v4().to_string(),
            text: word.text,
            start_ms: word.start_ms,
            end_ms: word.end_ms,
            channel: word.channel,
            state,
            speaker_index: word.speaker,
        })
        .collect()
}
