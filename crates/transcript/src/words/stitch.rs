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

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, start: i64, end: i64) -> RawWord {
        RawWord {
            text: text.to_string(),
            start_ms: start,
            end_ms: end,
            channel: 0,
            speaker: None,
        }
    }

    #[test]
    fn does_not_stitch_regular_words() {
        let tail = word(" hello", 0, 100);
        let head = word(" world", 100, 200);
        assert!(!should_stitch(&tail, &head));
    }

    #[test]
    fn stitches_punctuation() {
        let tail = word(" hello", 0, 100);
        let head = word(",", 100, 110);
        assert!(should_stitch(&tail, &head));
    }

    #[test]
    fn stitches_split_word_continuation() {
        let tail = word(" mill", 0, 100);
        let head = word("ions", 100, 200);
        assert!(should_stitch(&tail, &head));
    }

    #[test]
    fn does_not_stitch_beyond_gap() {
        let tail = word(" hello", 0, 100);
        let head = word(",", 500, 510);
        assert!(!should_stitch(&tail, &head));
    }
}
