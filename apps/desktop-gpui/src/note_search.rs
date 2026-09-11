//! Find-in-note matching (`note-input/search/matching.ts`): the query and
//! text preparation, occurrence scan with the `\w` word boundary, the
//! transcript word index, and the per-word highlight segments.

use unicode_normalization::UnicodeNormalization as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
}

/// JavaScript's `\w` without the unicode flag.
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `isWordBoundary`: outside the text, or not a word character.
fn is_word_boundary(text: &str, index: Option<usize>) -> bool {
    match index {
        None => true,
        Some(index) => text[index..]
            .chars()
            .next()
            .is_none_or(|c| !is_word_char(c)),
    }
}

/// `prepareQuery`: trimmed, NFC, folded unless case sensitive.
pub fn prepare_query(query: &str, case_sensitive: bool) -> String {
    let trimmed: String = query.trim().nfc().collect();
    if case_sensitive {
        trimmed
    } else {
        trimmed.to_lowercase()
    }
}

/// `prepareText`
pub fn prepare_text(text: &str, case_sensitive: bool) -> String {
    let normalized: String = text.nfc().collect();
    if case_sensitive {
        normalized
    } else {
        normalized.to_lowercase()
    }
}

/// `findOccurrences`: every (overlapping) start of `query` in `text`, kept
/// only at word boundaries when `whole_word`.
pub fn find_occurrences(text: &str, query: &str, whole_word: bool) -> Vec<usize> {
    let mut indices = Vec::new();
    if query.is_empty() || query.len() > text.len() {
        return indices;
    }
    let mut from = 0;
    while from + query.len() <= text.len() {
        let Some(found) = text[from..].find(query) else {
            break;
        };
        let index = from + found;
        if whole_word {
            let before = text[..index].char_indices().next_back().map(|(i, _)| i);
            let after = (index + query.len() < text.len()).then_some(index + query.len());
            if is_word_boundary(text, before) && is_word_boundary(text, after) {
                indices.push(index);
            }
        } else {
            indices.push(index);
        }
        // `from = idx + 1` in UTF-16 units: the next character boundary.
        from = index
            + text[index..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
    }
    indices
}

/// `createTranscriptEntryIndex` + `getTranscriptSearchIndexMatches`: the
/// words joined by single spaces, each occurrence attributed to the word it
/// starts in (or the next word when it starts on the joining space). Returns
/// the matching word indexes, one per occurrence.
pub fn transcript_matches(words: &[&str], prepared: &str, options: SearchOptions) -> Vec<usize> {
    let mut full = String::new();
    let mut spans: Vec<(usize, usize)> = Vec::with_capacity(words.len());
    for (index, word) in words.iter().enumerate() {
        if index > 0 {
            full.push(' ');
        }
        let start = full.len();
        full.extend(word.nfc());
        spans.push((start, full.len()));
    }
    let search_text = if options.case_sensitive {
        full.clone()
    } else {
        full.to_lowercase()
    };
    // Lowercasing can change byte lengths; map occurrences back through the
    // NFC text when it does not.
    let same_length = search_text.len() == full.len();
    let indices = find_occurrences(&search_text, prepared, options.whole_word);
    let mut result = Vec::new();
    let mut span_index = 0;
    for index in indices {
        let index = if same_length {
            index
        } else {
            // Fall back to a character-count mapping.
            let chars = search_text[..index].chars().count();
            full.char_indices()
                .nth(chars)
                .map(|(i, _)| i)
                .unwrap_or(full.len())
        };
        while span_index + 1 < spans.len()
            && index >= spans[span_index].1
            && index >= spans[span_index + 1].0
        {
            span_index += 1;
        }
        let Some(&(start, end)) = spans.get(span_index) else {
            continue;
        };
        if index >= start && index < end {
            result.push(span_index);
            continue;
        }
        if span_index + 1 < spans.len() && index >= end && index < spans[span_index + 1].0 {
            result.push(span_index + 1);
        }
    }
    result
}

/// `createHighlightSegments` as byte ranges of `text` (NFC): every
/// whitespace-separated token of the query, matched and merged.
pub fn highlight_ranges(
    raw_text: &str,
    query: &str,
    options: SearchOptions,
) -> Vec<std::ops::Range<usize>> {
    let text: String = raw_text.nfc().collect();
    let search_text = if options.case_sensitive {
        text.clone()
    } else {
        text.to_lowercase()
    };
    if search_text.len() != text.len() {
        // Case folding changed byte lengths; match on the folded text and
        // map through character counts.
        return highlight_ranges_by_chars(&text, &search_text, query, options);
    }
    let normalized_query: String = query.nfc().collect();
    let tokens: Vec<String> = normalized_query
        .split_whitespace()
        .map(|token| {
            if options.case_sensitive {
                token.to_string()
            } else {
                token.to_lowercase()
            }
        })
        .collect();
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    for token in &tokens {
        for start in find_occurrences(&search_text, token, options.whole_word) {
            ranges.push(start..start + token.len());
        }
    }
    merge_ranges(ranges)
}

fn highlight_ranges_by_chars(
    text: &str,
    search_text: &str,
    query: &str,
    options: SearchOptions,
) -> Vec<std::ops::Range<usize>> {
    let boundaries: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect();
    let folded_boundaries: Vec<usize> = search_text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(search_text.len()))
        .collect();
    let to_text = |folded: usize| {
        let chars = folded_boundaries
            .iter()
            .position(|b| *b >= folded)
            .unwrap_or(boundaries.len() - 1);
        boundaries[chars.min(boundaries.len() - 1)]
    };
    let normalized_query: String = query.nfc().collect();
    let mut ranges = Vec::new();
    for token in normalized_query.split_whitespace() {
        let token = if options.case_sensitive {
            token.to_string()
        } else {
            token.to_lowercase()
        };
        for start in find_occurrences(search_text, &token, options.whole_word) {
            ranges.push(to_text(start)..to_text(start + token.len()));
        }
    }
    merge_ranges(ranges)
}

fn merge_ranges(mut ranges: Vec<std::ops::Range<usize>>) -> Vec<std::ops::Range<usize>> {
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<std::ops::Range<usize>> = Vec::new();
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_queries_and_text() {
        assert_eq!(prepare_query("  Hello World ", false), "hello world");
        assert_eq!(prepare_query("Hello", true), "Hello");
        assert_eq!(prepare_text("Ünïcode", false), "ünïcode");
    }

    #[test]
    fn finds_overlapping_and_whole_word_occurrences() {
        assert_eq!(find_occurrences("aaa", "aa", false), vec![0, 1]);
        assert_eq!(
            find_occurrences("cat concat cat.", "cat", false),
            vec![0, 7, 11]
        );
        assert_eq!(
            find_occurrences("cat concat cat.", "cat", true),
            vec![0, 11]
        );
        assert_eq!(find_occurrences("under_score cat", "cat", true), vec![12]);
        assert_eq!(find_occurrences("a_cat", "cat", true), Vec::<usize>::new());
        assert!(find_occurrences("short", "longer query", false).is_empty());
    }

    #[test]
    fn attributes_transcript_occurrences_to_words() {
        let words = ["Good", "morning", "everyone,", "good", "night"];
        let options = SearchOptions::default();
        assert_eq!(transcript_matches(&words, "good", options), vec![0, 3]);
        // A match starting on the joining space lands on the next word.
        assert_eq!(transcript_matches(&words, " morning", options), vec![1]);
        // Case sensitive keeps only the exact one.
        assert_eq!(
            transcript_matches(
                &words,
                "Good",
                SearchOptions {
                    case_sensitive: true,
                    whole_word: false
                }
            ),
            vec![0]
        );
        // A phrase across two words is attributed to the first.
        assert_eq!(
            transcript_matches(&words, "morning everyone", options),
            vec![1]
        );
    }

    #[test]
    fn highlight_ranges_merge_tokens() {
        let options = SearchOptions::default();
        assert_eq!(
            highlight_ranges("Good morning", "good MORN", options),
            vec![0..4, 5..9]
        );
        assert_eq!(highlight_ranges("banana", "ana an", options), vec![1..6]);
        assert!(highlight_ranges("banana", "   ", options).is_empty());
        assert!(highlight_ranges("banana", "xyz", options).is_empty());
        let whole = SearchOptions {
            case_sensitive: false,
            whole_word: true,
        };
        assert_eq!(highlight_ranges("cat concat", "cat", whole), vec![0..3]);
    }
}
