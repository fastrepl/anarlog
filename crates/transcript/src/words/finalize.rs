use uuid::Uuid;

use crate::types::{
    FinalizedWord, PartialWord, RawWord, RuntimeSpeakerHint, SpeakerHintData, WordRef, WordState,
};

pub(crate) fn to_partial(word: &RawWord) -> PartialWord {
    PartialWord {
        text: word.text.clone(),
        start_ms: word.start_ms,
        end_ms: word.end_ms,
        channel: word.channel,
    }
}

pub(crate) fn to_partial_hint(word: &RawWord, index: usize) -> Option<RuntimeSpeakerHint> {
    Some(RuntimeSpeakerHint {
        target: WordRef::RuntimeIndex(index),
        data: provider_speaker_hint(word.speaker?, word.channel),
    })
}

pub(crate) fn finalize_words(
    words: Vec<RawWord>,
    state: WordState,
) -> (Vec<FinalizedWord>, Vec<RuntimeSpeakerHint>) {
    let mut finalized_words = Vec::with_capacity(words.len());
    let mut hints = Vec::new();

    for word in words {
        let id = Uuid::new_v4().to_string();

        if let Some(speaker_index) = word.speaker {
            hints.push(RuntimeSpeakerHint {
                target: WordRef::FinalWordId(id.clone()),
                data: provider_speaker_hint(speaker_index, word.channel),
            });
        }

        finalized_words.push(FinalizedWord {
            id,
            text: word.text,
            start_ms: word.start_ms,
            end_ms: word.end_ms,
            channel: word.channel,
            state,
        });
    }

    (finalized_words, hints)
}

fn provider_speaker_hint(speaker_index: i32, channel: i32) -> SpeakerHintData {
    SpeakerHintData::ProviderSpeakerIndex {
        speaker_index,
        provider: None,
        channel: Some(channel),
    }
}
