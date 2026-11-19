//! Which words around a dictation are worth telling speech-to-text and cleanup about: the ones it
//! can't guess. A product written with an inner capital (PostHog), letters with digits (GPT5), a
//! capitalized word that isn't an ordinary word (Woohyeok, Supabase), or a handle. Ordinary words
//! stay out even when capitalized: forcing "Linear" or "Notion" turns ordinary speech into brand
//! names.

use std::collections::{HashMap, HashSet};

use crate::text::{is_combining_mark, is_latin};

/// Every engine's own cap is at most this; smaller ones take the top of the ranked list.
pub const MAX_TERMS: usize = 100;
pub const MAX_TERM_LENGTH: usize = 100;

/// Lowercase words a dictionary lists as ordinary, plus everyday words it predates. Built once
/// from a word list such as macOS's `/usr/share/dict/words`; lines with capitals are names and
/// are left out.
pub struct OrdinaryWords {
    words: HashSet<String>,
}

impl OrdinaryWords {
    pub fn from_lines(lines: &str) -> Self {
        let words = lines
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.chars().any(char::is_uppercase))
            .map(str::to_owned)
            .collect();
        Self { words }
    }

    /// Whether a lowercase word is ordinary, in any of its inflections.
    pub fn contains(&self, word: &str) -> bool {
        stems(word)
            .iter()
            .any(|stem| MODERN_WORDS.contains(&stem.as_str()) || self.words.contains(stem))
    }
}

/// A word and what it may be inflected from: "insights" → "insight", "replies" → "reply".
fn stems(word: &str) -> Vec<String> {
    let mut out = vec![word.to_owned()];
    let count = word.chars().count();
    let mut add = |suffix: &str, replacement: &str| {
        if count > suffix.len() + 2 && word.ends_with(suffix) {
            out.push(format!(
                "{}{replacement}",
                &word[..word.len() - suffix.len()]
            ));
        }
    };
    add("s", "");
    add("es", "");
    add("ies", "y");
    add("ed", "");
    add("ed", "e");
    add("ing", "");
    add("ing", "e");
    add("er", "");
    add("ers", "");
    add("ly", "");
    out
}

// Everyday words newer than the system word list, mostly what apps put on their buttons.
const MODERN_WORDS: &[&str] = &[
    "app",
    "apps",
    "admin",
    "analytics",
    "api",
    "archive",
    "autosave",
    "avatar",
    "backend",
    "backlog",
    "blog",
    "blogger",
    "bookmark",
    "browser",
    "bugfix",
    "calendar",
    "changelog",
    "chatbot",
    "checkbox",
    "checkout",
    "codebase",
    "config",
    "cookie",
    "cyber",
    "dashboard",
    "database",
    "dataset",
    "debug",
    "deploy",
    "desktop",
    "devops",
    "dm",
    "download",
    "dropdown",
    "ebook",
    "email",
    "emoji",
    "favicon",
    "feedback",
    "filename",
    "firmware",
    "frontend",
    "gif",
    "hashtag",
    "homepage",
    "hotkey",
    "html",
    "inbox",
    "internet",
    "keyboard",
    "laptop",
    "livestream",
    "login",
    "logout",
    "malware",
    "meetup",
    "metadata",
    "microphone",
    "multimedia",
    "newsletter",
    "notification",
    "offline",
    "okay",
    "online",
    "onboarding",
    "password",
    "pdf",
    "playlist",
    "plugin",
    "podcast",
    "popup",
    "preview",
    "privacy",
    "profile",
    "repo",
    "runtime",
    "screenshot",
    "sdk",
    "selfie",
    "sidebar",
    "signup",
    "sitemap",
    "skillset",
    "smartphone",
    "snapshot",
    "software",
    "spam",
    "spreadsheet",
    "startup",
    "subscriber",
    "subtitle",
    "tablet",
    "teammate",
    "template",
    "textbox",
    "thumbnail",
    "timeline",
    "timestamp",
    "toolbar",
    "tooltip",
    "touchscreen",
    "typo",
    "unfollow",
    "unsubscribe",
    "upload",
    "url",
    "username",
    "vlog",
    "voicemail",
    "webcam",
    "webinar",
    "webpage",
    "website",
    "whiteboard",
    "widget",
    "wifi",
    "wiki",
    "workflow",
    "workspace",
    "youtuber",
];

struct Tally {
    term: String,
    count: usize,
    first: usize,
    inside: usize,
}

/// The distinctive terms of `text`, most frequent first, then in order of first appearance.
/// A capitalized word counts once it appears somewhere other than a sentence's start, or twice
/// anywhere. `heading` treats every word as mid-sentence, for a window title or an app's name.
/// Without `ordinary`, capitalized words can't be told from ordinary ones and are left out.
pub fn distinctive_terms(
    text: &str,
    ordinary: Option<&OrdinaryWords>,
    heading: bool,
    limit: usize,
) -> Vec<String> {
    let mut tallies: HashMap<String, Tally> = HashMap::new();
    for (index, (gap, token)) in tokens(text).into_iter().enumerate() {
        let starts_sentence =
            !heading && (index == 0 || gap.chars().any(|c| ".!?。！？\n".contains(c)));
        let Some(term) = candidate(token, ordinary) else {
            continue;
        };
        let key = term.to_lowercase();
        let capitalized = is_capitalized(&term);
        let tally = tallies.entry(key).or_insert(Tally {
            term,
            count: 0,
            first: index + 1,
            inside: 0,
        });
        tally.count += 1;
        if !starts_sentence || !capitalized {
            tally.inside += 1;
        }
    }
    let mut kept: Vec<Tally> = tallies
        .into_values()
        .filter(|tally| tally.inside > 0 || tally.count > 1)
        .collect();
    kept.sort_by(|a, b| b.count.cmp(&a.count).then(a.first.cmp(&b.first)));
    kept.into_iter()
        .take(limit)
        .map(|tally| tally.term)
        .collect()
}

/// Tokens of letters, marks and digits joined by apostrophes and underscores, optionally led by
/// `@`, each with the text since the previous token. Hyphens, dots and slashes split words.
fn tokens(text: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut gap_start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let handle = c == '@' && chars.peek().is_some_and(|(_, next)| next.is_alphanumeric());
        if !handle && !c.is_alphanumeric() {
            continue;
        }
        let mut end = start + c.len_utf8();
        if handle {
            let (i, next) = chars.next().expect("peeked");
            end = i + next.len_utf8();
        }
        while let Some(&(i, next)) = chars.peek() {
            if next.is_alphanumeric() || is_combining_mark(next) || matches!(next, '\'' | '’' | '_')
            {
                end = i + next.len_utf8();
                chars.next();
            } else {
                break;
            }
        }
        out.push((&text[gap_start..start], &text[start..end]));
        gap_start = end;
    }
    out
}

fn is_capitalized(word: &str) -> bool {
    let mut chars = word.chars();
    let (Some(first), Some(second)) = (chars.next(), chars.next()) else {
        return false;
    };
    first.is_uppercase()
        && second.is_lowercase()
        && chars.all(|c| c.is_lowercase() || matches!(c, '\'' | '’') || is_combining_mark(c))
}

fn has_inner_capital(word: &str) -> bool {
    word.chars()
        .zip(word.chars().skip(1))
        .any(|(a, b)| a.is_lowercase() && b.is_uppercase())
}

fn is_numberish(word: &str) -> bool {
    let lower = word.to_lowercase();
    if lower.len() >= 6 && lower.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    if let Some(rest) = lower.strip_prefix('v')
        && !rest.is_empty()
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return true;
    }
    let digits = lower.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return false;
    }
    let suffix = &lower[digits..];
    const UNITS: &[&str] = &[
        "st", "nd", "rd", "th", "am", "pm", "k", "m", "b", "x", "h", "d", "s", "w", "ms", "kb",
        "mb", "gb", "tb", "px", "fps", "hz", "mhz", "ghz", "yo", "min", "sec", "hr", "hrs",
    ];
    UNITS.contains(&suffix)
}

fn candidate(token: &str, ordinary: Option<&OrdinaryWords>) -> Option<String> {
    let handle = token.starts_with('@');
    let word = token.trim_start_matches('@');
    let word = word
        .strip_suffix("'s")
        .or_else(|| word.strip_suffix("’s"))
        .unwrap_or(word);
    let length = word.chars().count();
    if !(2..=MAX_TERM_LENGTH).contains(&length)
        || !word.chars().all(|c| {
            is_latin(c) || c.is_numeric() || is_combining_mark(c) || matches!(c, '\'' | '’' | '_')
        })
        || !word.chars().any(char::is_alphabetic)
    {
        return None;
    }
    let is_ordinary = |w: &str| ordinary.is_some_and(|words| words.contains(&w.to_lowercase()));
    if handle {
        return (length >= 3 && !is_ordinary(word)).then(|| word.to_owned());
    }
    if word.contains('_') {
        return None;
    }
    if word.chars().any(char::is_numeric) {
        return (!is_numberish(word)).then(|| word.to_owned());
    }
    if has_inner_capital(word) && word != word.to_uppercase() {
        return Some(word.to_owned());
    }
    if ordinary.is_some() && is_capitalized(word) && length >= 3 && !is_ordinary(word) {
        return Some(word.to_owned());
    }
    None
}

/// The ranked list every engine reads, highest first, from sources in priority order.
/// Duplicates keep their first place; anything too long for an engine is dropped.
pub fn rank_sources<'a, I, S>(sources: I, limit: usize) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: IntoIterator<Item = &'a str>,
{
    let mut seen = HashSet::new();
    let mut ranked = Vec::new();
    for source in sources {
        for raw in source {
            if ranked.len() == limit {
                return ranked;
            }
            let term = raw.trim();
            if term.is_empty() || term.chars().count() > MAX_TERM_LENGTH {
                continue;
            }
            if seen.insert(term.to_lowercase()) {
                ranked.push(term.to_owned());
            }
        }
    }
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordinary() -> OrdinaryWords {
        OrdinaryWords::from_lines(
            "broken\nonboarding\nis\nlinear\nnotion\nship\nthe\nask\nmark\nhill\nAaron\n",
        )
    }

    #[test]
    fn keeps_names_speech_to_text_cannot_guess_and_leaves_ordinary_capitals_out() {
        let words = ordinary();
        let text = "Onboarding is broken. Ask Woohyeok about PostHog and GPT5 on Linear, \
                    then @mina ships v2 at 3pm with Supabase-backed auth.";
        assert_eq!(
            distinctive_terms(text, Some(&words), false, MAX_TERMS),
            ["Woohyeok", "PostHog", "GPT5", "mina", "Supabase"]
        );
        // Without a word list, only spellings no ordinary word has are kept.
        assert_eq!(
            distinctive_terms(text, None, false, MAX_TERMS),
            ["PostHog", "GPT5", "mina"]
        );
        // A name on the word list only as a capitalized line is still a name.
        assert_eq!(
            distinctive_terms("Talk to Aaron", Some(&words), false, MAX_TERMS),
            ["Aaron"]
        );
    }
}
