use serde::{Deserialize, Serialize};

use crate::text::{char_len, is_unspaced, is_word_char, same_char};

pub const MAX_REPLACEMENTS: usize = 100;
pub const MAX_REPLACEMENT_FROM_LENGTH: usize = 100;
pub const MAX_REPLACEMENT_TO_LENGTH: usize = 2_000;

/// A phrase said in dictation, and what is typed in its place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Replacement {
    pub from: String,
    pub to: String,
}

struct Phrase<'a> {
    words: Vec<Vec<char>>,
    // Korean, Japanese and Chinese attach particles straight to a word, so a phrase starting or
    // ending in those scripts still matches with more letters next to it.
    bounded_start: bool,
    bounded_end: bool,
    to: &'a str,
}

/// `text` with each phrase said swapped for what the person saved, in any case and spacing.
/// Longer phrases go first, and text put in is never replaced again. When the phrase is all that
/// was said, the period cleanup ended it with goes too, so "Work mail." types just the address.
pub fn apply_replacements(text: &str, replacements: &[Replacement]) -> String {
    let mut phrases: Vec<Phrase> = replacements
        .iter()
        .take(MAX_REPLACEMENTS)
        .filter(|r| {
            let from = r.from.trim();
            !from.is_empty()
                && !r.to.is_empty()
                && char_len(from) <= MAX_REPLACEMENT_FROM_LENGTH
                && char_len(&r.to) <= MAX_REPLACEMENT_TO_LENGTH
        })
        .map(|r| {
            let from = r.from.trim();
            Phrase {
                words: from
                    .split_whitespace()
                    .map(|w| w.chars().collect())
                    .collect(),
                bounded_start: !from.chars().next().is_some_and(is_unspaced),
                bounded_end: !from.chars().last().is_some_and(is_unspaced),
                to: &r.to,
            }
        })
        .collect();
    if phrases.is_empty() {
        return text.to_owned();
    }
    // Stable, so equally long phrases keep the person's order.
    phrases.sort_by_key(|phrase| std::cmp::Reverse(phrase_len(phrase)));

    let trimmed = text.trim();
    let unended = trimmed
        .strip_suffix('.')
        .or_else(|| trimmed.strip_suffix('。'))
        .unwrap_or(trimmed);
    let unended_chars: Vec<char> = unended.chars().collect();
    let alone = phrases
        .iter()
        .any(|phrase| match_at(&unended_chars, 0, phrase) == Some(unended_chars.len()));
    let chars: Vec<char> = if alone {
        unended_chars
    } else {
        text.chars().collect()
    };

    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let found = phrases
            .iter()
            .find_map(|phrase| match_at(&chars, i, phrase).map(|end| (end, phrase.to)));
        match found {
            Some((end, to)) if end > i => {
                out.push_str(to);
                i = end;
            }
            _ => {
                out.push(chars[i]);
                i += 1;
            }
        }
    }
    out
}

fn phrase_len(phrase: &Phrase) -> usize {
    phrase.words.iter().map(Vec::len).sum::<usize>() + phrase.words.len().saturating_sub(1)
}

/// Where `phrase` said at `start` ends, with any run of whitespace between its words.
fn match_at(text: &[char], start: usize, phrase: &Phrase) -> Option<usize> {
    if phrase.bounded_start && start > 0 && is_word_char(text[start - 1]) {
        return None;
    }
    let mut i = start;
    for (n, word) in phrase.words.iter().enumerate() {
        if n > 0 {
            let gap = i;
            while i < text.len() && text[i].is_whitespace() {
                i += 1;
            }
            if i == gap {
                return None;
            }
        }
        for &expected in word {
            if i >= text.len() || !same_char(expected, text[i]) {
                return None;
            }
            i += 1;
        }
    }
    if phrase.bounded_end && i < text.len() && is_word_char(text[i]) {
        return None;
    }
    Some(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(pairs: &[(&str, &str)]) -> Vec<Replacement> {
        pairs
            .iter()
            .map(|(from, to)| Replacement {
                from: (*from).into(),
                to: (*to).into(),
            })
            .collect()
    }

    #[test]
    fn swaps_phrases_in_any_case_and_spacing_and_drops_the_period_when_alone() {
        let list = saved(&[
            ("work mail", "john@fastrepl.com"),
            ("my website", "https://johnjeong.com"),
        ]);
        assert_eq!(
            apply_replacements("Send it to Work  Mail, please.", &list),
            "Send it to john@fastrepl.com, please."
        );
        assert_eq!(
            apply_replacements("My website.", &list),
            "https://johnjeong.com"
        );
        assert_eq!(
            apply_replacements("내 웹사이트。", &saved(&[("내 웹사이트", "x")])),
            "x"
        );
    }

    #[test]
    fn prefers_longer_phrases_and_never_replaces_what_it_put_in() {
        let list = saved(&[
            ("link", "my link"),
            ("calendar link", "https://cal.com/link"),
        ]);
        assert_eq!(
            apply_replacements("my calendar link and link", &list),
            "my https://cal.com/link and my link"
        );
        // A cascade would turn "a" into "b" and then into "c".
        assert_eq!(
            apply_replacements("say a", &saved(&[("a", "b"), ("b", "c")])),
            "say b"
        );
    }

    #[test]
    fn respects_word_edges_except_in_scripts_that_attach_particles() {
        assert_eq!(
            apply_replacements("recall the local cal", &saved(&[("cal", "calendar")])),
            "recall the local calendar"
        );
        assert_eq!(
            apply_replacements(
                "내 웹사이트를 봐",
                &saved(&[("내 웹사이트", "https://johnjeong.com")])
            ),
            "https://johnjeong.com를 봐"
        );
        assert_eq!(
            apply_replacements("東京駅に集合", &saved(&[("東京駅", "Tokyo Station")])),
            "Tokyo Stationに集合"
        );
        assert_eq!(
            apply_replacements("ÉCOLE et école", &saved(&[("école", "school")])),
            "school et school"
        );
    }

    #[test]
    fn types_replacement_text_literally_and_ignores_unusable_entries() {
        assert_eq!(
            apply_replacements("try C++ (new) now", &saved(&[("c++ (new)", "$& $1")])),
            "try $& $1 now"
        );
        assert_eq!(apply_replacements("Work mail.", &[]), "Work mail.");
        assert_eq!(
            apply_replacements("Work mail.", &saved(&[(" ", "x"), ("work", "")])),
            "Work mail."
        );
    }
}
