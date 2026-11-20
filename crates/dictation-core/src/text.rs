use unicode_script::{Script, UnicodeScript};

/// A letter or a number: what `[\p{L}\p{N}]` matches in the reference rules.
pub(crate) fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

/// Scripts written without spaces between words, so a phrase can carry a particle straight after it.
pub(crate) fn is_unspaced(c: char) -> bool {
    matches!(
        c.script(),
        Script::Hangul | Script::Han | Script::Hiragana | Script::Katakana
    )
}

/// Chinese and Japanese text, with their punctuation and full-width forms: no space goes between.
pub(crate) fn is_unspaced_writing(c: char) -> bool {
    matches!(
        c.script(),
        Script::Han | Script::Hiragana | Script::Katakana
    ) || ('\u{3000}'..='\u{303F}').contains(&c)
        || ('\u{FF01}'..='\u{FF60}').contains(&c)
}

/// Combining diacritics, which belong to the letter before them.
pub(crate) fn is_combining_mark(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036F}' | '\u{1AB0}'..='\u{1AFF}' | '\u{1DC0}'..='\u{1DFF}' | '\u{20D0}'..='\u{20FF}' | '\u{FE20}'..='\u{FE2F}')
}

pub(crate) fn is_latin(c: char) -> bool {
    c.script() == Script::Latin
}

pub(crate) fn same_char(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

pub(crate) fn char_len(text: &str) -> usize {
    text.chars().count()
}

/// The last `limit` characters of `text`.
pub(crate) fn tail_chars(text: &str, limit: usize) -> &str {
    let count = char_len(text);
    if count <= limit {
        return text;
    }
    let skip = count - limit;
    let start = text.char_indices().nth(skip).map_or(text.len(), |(i, _)| i);
    &text[start..]
}

/// The first `limit` characters of `text`.
pub(crate) fn head_chars(text: &str, limit: usize) -> &str {
    let end = text
        .char_indices()
        .nth(limit)
        .map_or(text.len(), |(i, _)| i);
    &text[..end]
}

/// Lowercase letters and digits only, so "Wispr Flow" and "wispr-flow" read alike.
pub(crate) fn flatten(text: &str) -> String {
    text.chars()
        .filter(|c| is_word_char(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// The words alone, spaced apart and padded, so a sentence with other punctuation still matches.
pub(crate) fn plain_words(text: &str) -> String {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !is_word_char(c))
        .filter(|word| !word.is_empty())
        .collect();
    format!(" {} ", words.join(" "))
}

/// Whether `text` says `term` as whole words, case aside, with any spacing (or none) between its
/// words.
pub(crate) fn says(text: &str, term: &str) -> bool {
    let words: Vec<Vec<char>> = term
        .split_whitespace()
        .map(|word| word.chars().collect())
        .collect();
    if words.is_empty() {
        return false;
    }
    let chars: Vec<char> = text.chars().collect();
    (0..chars.len()).any(|start| {
        if start > 0 && is_word_char(chars[start - 1]) {
            return false;
        }
        let mut i = start;
        for (n, word) in words.iter().enumerate() {
            if n > 0 {
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
            }
            for &expected in word {
                if i >= chars.len() || !same_char(expected, chars[i]) {
                    return false;
                }
                i += 1;
            }
        }
        i >= chars.len() || !is_word_char(chars[i])
    })
}

/// Edit distance between two strings, by characters.
pub(crate) fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut current = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let substitution = previous[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            current[j] = (previous[j] + 1).min(current[j - 1] + 1).min(substitution);
        }
        previous = current;
    }
    previous[b.len()]
}
