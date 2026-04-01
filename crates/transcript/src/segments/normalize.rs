use crate::types::ChannelProfile;
use crate::types::{FinalizedWord, PartialWord};

use super::model::NormalizedWord;

pub(super) fn normalize_words(
    final_words: &[FinalizedWord],
    partial_words: &[PartialWord],
) -> Vec<NormalizedWord> {
    let mut combined: Vec<NormalizedWord> =
        Vec::with_capacity(final_words.len() + partial_words.len());

    combined.extend(final_words.iter().map(|word| NormalizedWord {
        text: word.text.clone(),
        start_ms: word.start_ms,
        end_ms: word.end_ms,
        channel: ChannelProfile::from(word.channel),
        is_final: true,
        id: Some(word.id.clone()),
        order: 0,
        speaker_index: word.speaker_index,
    }));

    combined.extend(partial_words.iter().map(|word| NormalizedWord {
        text: word.text.clone(),
        start_ms: word.start_ms,
        end_ms: word.end_ms,
        channel: ChannelProfile::from(word.channel),
        is_final: false,
        id: None,
        order: 0,
        speaker_index: word.speaker_index,
    }));

    combined.sort_by_key(|word| word.start_ms);

    for (index, word) in combined.iter_mut().enumerate() {
        word.order = index;
    }

    combined
}
