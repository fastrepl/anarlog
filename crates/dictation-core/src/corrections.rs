//! Reviewable dictionary suggestions from how the person corrected a dictation they just made.
//! Only the dictation's own text and what the person kept of it are compared; nothing watches
//! typing elsewhere.

use serde::{Deserialize, Serialize};

use crate::terms::MAX_TERM_LENGTH;
use crate::text::{char_len, distance, is_unspaced};

/// The most suggestions one dictation offers.
pub const MAX_SUGGESTIONS: usize = 5;
// Past this many words either side, the diff is skipped rather than paid for.
const MAX_WORDS: usize = 400;
// A correction replaces a short run of words, like "chart" with "char" or "fast ripple" with
// "Fastrepl"; a longer rewrite is the person rephrasing.
const MAX_RUN: usize = 3;
// Text read around the caret moves with it, so a long field's ends are matched by the text right
// next to the dictation.
const END_ANCHOR_LENGTH: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SuggestRequest {
    /// What the dictation typed.
    pub inserted: String,
    /// What the person kept of it after editing.
    pub kept: String,
    /// The Dictionary as it is now; terms already in it are not suggested.
    pub dictionary: Vec<String>,
    /// Names seen in this dictation's context, used only to spell a suggestion as it was seen.
    pub seen: Vec<String>,
}

/// Terms worth offering for the Dictionary: each short run of words the person replaced with a
/// similar spelling, spelled as seen on screen when it was. Never a term already in the
/// Dictionary or one the dictation already wrote elsewhere.
pub fn suggest_terms(request: &SuggestRequest) -> Vec<String> {
    let listed: Vec<String> = request
        .dictionary
        .iter()
        .map(|term| term.trim().to_lowercase())
        .collect();
    let mut out: Vec<String> = Vec::new();
    for term in dictionary_corrections(&request.inserted, &request.kept) {
        let term = seen_spelling(&term, &request.seen);
        let key = term.to_lowercase();
        if char_len(&term) > MAX_TERM_LENGTH
            || listed.contains(&key)
            || out.iter().any(|t| t.to_lowercase() == key)
        {
            continue;
        }
        out.push(term);
        if out.len() == MAX_SUGGESTIONS {
            break;
        }
    }
    out
}

fn words(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .collect()
}

/// Whether `to` reads as a fix of a misheard `from` rather than a different word: spelled alike,
/// spaces and case aside, as "chart" and "char" or "jon" and "John" are.
fn sounds_alike(from: &str, to: &str) -> bool {
    let a: String = from.to_lowercase().split_whitespace().collect();
    let b: String = to.to_lowercase().split_whitespace().collect();
    let longest = char_len(&a).max(char_len(&b));
    distance(&a, &b) <= (longest / 2).max(1)
}

fn dictionary_corrections(typed: &str, kept: &str) -> Vec<String> {
    let before = words(typed);
    let after = words(kept);
    if before.is_empty() || after.is_empty() || before.len() > MAX_WORDS || after.len() > MAX_WORDS
    {
        return Vec::new();
    }
    // Longest common subsequence of words, walked from the start so runs come out in order.
    let (n, m) = (before.len(), after.len());
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if before[i] == after[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut terms: Vec<String> = Vec::new();
    let mut removed: Vec<&str> = Vec::new();
    let mut added: Vec<&str> = Vec::new();
    let mut flush = |removed: &mut Vec<&str>, added: &mut Vec<&str>| {
        let from = removed.join(" ");
        let to = added.join(" ");
        if !removed.is_empty()
            && !added.is_empty()
            && removed.len() <= MAX_RUN
            && added.len() <= MAX_RUN
            && !added.iter().all(|word| before.contains(word))
            && to.chars().any(char::is_alphabetic)
            && sounds_alike(&from, &to)
            && !respaced_unspaced_script(&from, &to)
            && !terms.contains(&to)
        {
            terms.push(to);
        }
        removed.clear();
        added.clear();
    };
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && before[i] == after[j] {
            flush(&mut removed, &mut added);
            i += 1;
            j += 1;
        } else if j >= m || (i < n && lcs[i + 1][j] >= lcs[i][j + 1]) {
            removed.push(before[i]);
            i += 1;
        } else {
            added.push(after[j]);
            j += 1;
        }
    }
    flush(&mut removed, &mut added);
    terms
}

/// A spacing fix in Korean, Chinese or Japanese ("우혁 씨한테" to "우혁씨한테") is about where
/// particles attach, not a misheard word.
fn respaced_unspaced_script(from: &str, to: &str) -> bool {
    let joined = |text: &str| text.split_whitespace().collect::<String>();
    to.chars().any(is_unspaced) && joined(from) == joined(to)
}

/// How a suggestion is spelled: the way it was seen, when something seen matches it, case and
/// spacing aside or by sounding alike. Only a match of about the same length counts, so
/// "Wisperflow" becomes "Wispr Flow" when that was seen, but never just "Wispr".
fn seen_spelling(term: &str, seen: &[String]) -> String {
    let flat = |text: &str| -> String {
        text.to_lowercase()
            .chars()
            .filter(|c| !(c.is_whitespace() || "-_.".contains(*c)))
            .collect()
    };
    let wanted = flat(term);
    if wanted.is_empty() {
        return term.to_owned();
    }
    let wanted_len = char_len(&wanted);
    let mut best: Option<(&str, usize)> = None;
    for spelling in seen {
        let found = flat(spelling);
        if found.is_empty() {
            continue;
        }
        if found == wanted {
            return spelling.clone();
        }
        if char_len(&found).abs_diff(wanted_len) > (wanted_len * 3 / 10).max(2) {
            continue;
        }
        if !sounds_alike(term, spelling) {
            continue;
        }
        let apart = distance(&wanted, &found);
        if best.is_none_or(|(_, d)| apart < d) {
            best = Some((spelling, apart));
        }
    }
    best.map_or_else(|| term.to_owned(), |(spelling, _)| spelling.to_owned())
}

/// A field as read around the caret.
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FieldText {
    pub before: String,
    pub selected: String,
    pub after: String,
}

/// What the person left between the text that surrounded the caret when the dictation started,
/// or `None` when the field no longer has those ends (sent, cleared, or rewritten around it) or
/// nothing is left between them.
pub fn kept_text(start: &FieldText, now: &FieldText) -> Option<String> {
    let full: Vec<char> = now
        .before
        .chars()
        .chain(now.selected.chars())
        .chain(now.after.chars())
        .collect();
    let caret = char_len(&now.before);
    let start_before: Vec<char> = start.before.chars().collect();
    let start_after: Vec<char> = start.after.chars().collect();
    let prefix = &start_before[start_before.len().saturating_sub(END_ANCHOR_LENGTH)..];
    let suffix = &start_after[..start_after.len().min(END_ANCHOR_LENGTH)];
    let from = if start_before.len() > END_ANCHOR_LENGTH {
        rfind(&full, prefix, caret.saturating_sub(prefix.len()))
    } else {
        find(&full, prefix, 0)
    }?;
    let to = if start_after.len() > END_ANCHOR_LENGTH {
        find(&full, suffix, caret + char_len(&now.selected))
    } else {
        rfind(&full, suffix, full.len())
    }?;
    if to < from + prefix.len() {
        return None;
    }
    let kept: String = full[from + prefix.len()..to].iter().collect();
    let kept = kept.trim();
    (!kept.is_empty()).then(|| kept.to_owned())
}

fn find(haystack: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(from.min(haystack.len()));
    }
    (from..=haystack.len().checked_sub(needle.len())?).find(|&i| haystack[i..].starts_with(needle))
}

/// The last match starting at or before `at`.
fn rfind(haystack: &[char], needle: &[char], at: usize) -> Option<usize> {
    let last = haystack.len().checked_sub(needle.len())?;
    (0..=at.min(last))
        .rev()
        .find(|&i| haystack[i..].starts_with(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suggest(inserted: &str, kept: &str) -> Vec<String> {
        suggest_terms(&SuggestRequest {
            inserted: inserted.into(),
            kept: kept.into(),
            dictionary: vec![],
            seen: vec![],
        })
    }

    #[test]
    fn suggests_the_spelling_a_misheard_word_was_corrected_to() {
        assert_eq!(
            suggest(
                "Open chart and check the build.",
                "Open char and check the build."
            ),
            ["char"]
        );
        assert_eq!(suggest("Ask jon about it", "Ask John about it"), ["John"]);
        assert_eq!(
            suggest("ship fast ripple today", "ship Fastrepl today"),
            ["Fastrepl"]
        );
        assert_eq!(
            suggest("우혁 씨한테 물어봐", "우혁씨한테 물어봐"),
            Vec::<String>::new()
        );
        assert_eq!(suggest("ask 정우혘 today", "ask 정우혁 today"), ["정우혁"]);
    }

    #[test]
    fn ignores_rephrasing_additions_removals_numbers_and_words_dictation_already_wrote() {
        let none = Vec::<String>::new();
        assert_eq!(
            suggest("That was great work", "That was awesome work"),
            none
        );
        assert_eq!(suggest("Send it", "Send it to Char tomorrow"), none);
        assert_eq!(suggest("Send it to them now", "Send it now"), none);
        assert_eq!(
            suggest("Ask Char, chart is wrong", "Ask Char, Char is wrong"),
            none
        );
        assert_eq!(
            suggest("one two three four five", "uno dos tres cuatro cinco"),
            none
        );
        assert_eq!(suggest("Room 41", "Room 42"), none);
    }

    #[test]
    fn skips_dictionary_terms_respells_as_seen_and_bounds_the_list() {
        let request = SuggestRequest {
            inserted: "try wisper flow and post hog with jon".into(),
            kept: "try Wisperflow and posthog with John".into(),
            dictionary: vec!["john".into()],
            seen: vec!["Wispr".into(), "Wispr Flow".into(), "PostHog".into()],
        };
        assert_eq!(suggest_terms(&request), ["Wispr Flow", "PostHog"]);

        let inserted = (0..8)
            .map(|n| format!("wordx{n}a"))
            .collect::<Vec<_>>()
            .join(" ");
        let kept = (0..8)
            .map(|n| format!("wordx{n}b"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            suggest(&inserted, &kept).len(),
            0,
            "eight changed words is a rewrite"
        );
        let inserted = (0..8)
            .map(|n| format!("wordx{n}a keep{n}"))
            .collect::<Vec<_>>()
            .join(" ");
        let kept = (0..8)
            .map(|n| format!("wordx{n}b keep{n}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(suggest(&inserted, &kept).len(), MAX_SUGGESTIONS);
    }

    #[test]
    fn finds_what_was_kept_between_the_fields_original_ends() {
        let start = FieldText {
            before: "Hi Mina, ".into(),
            selected: String::new(),
            after: "\nThanks".into(),
        };
        let now = FieldText {
            before: "Hi Mina, ask John about it.".into(),
            selected: String::new(),
            after: "\nThanks".into(),
        };
        assert_eq!(
            kept_text(&start, &now).as_deref(),
            Some("ask John about it.")
        );
        let sent = FieldText::default();
        assert_eq!(kept_text(&start, &sent), None);
    }
}
