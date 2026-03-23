use crate::types::RawWord;

pub(crate) fn dedup(words: Vec<RawWord>, watermark: i64) -> Vec<RawWord> {
    words
        .into_iter()
        .skip_while(|word| word.end_ms <= watermark)
        .collect()
}

pub(crate) fn stitch(
    held: Option<RawWord>,
    mut words: Vec<RawWord>,
) -> (Vec<RawWord>, Option<RawWord>) {
    if words.is_empty() {
        return (held.into_iter().collect(), None);
    }

    if let Some(held_word) = held {
        if should_stitch(&held_word, &words[0]) {
            words[0] = merge_words(held_word, words[0].clone());
        } else {
            words.insert(0, held_word);
        }
    }

    let new_held = words.pop();
    (words, new_held)
}

const STITCH_MAX_GAP_MS: i64 = 300;

fn should_stitch(tail: &RawWord, head: &RawWord) -> bool {
    !head.text.starts_with(' ') && (head.start_ms - tail.end_ms) <= STITCH_MAX_GAP_MS
}

fn merge_words(mut left: RawWord, right: RawWord) -> RawWord {
    left.text.push_str(&right.text);
    left.end_ms = right.end_ms;
    if left.speaker.is_none() {
        left.speaker = right.speaker;
    }
    left
}
