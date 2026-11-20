use serde::{Deserialize, Serialize};

use crate::text::{char_len, is_unspaced_writing, is_word_char, tail_chars};

/// How a dictation lands at the caret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaretFit {
    /// What to type at the caret.
    pub text: String,
    /// The dictation finishes the sentence before the caret: `text` starts with ", " and goes
    /// over the one period directly before the caret, so "now." becomes "now, if…".
    pub join: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Caret {
    Start,
    Sentence,
    Mid,
}

/// `text`, a dictation, as it should be typed after `before`, the text before the caret, or
/// `before` `None` when the field cannot be read.
///
/// - Spacing: one space after a word or a sentence's end; none at the start of a field or line,
///   after whitespace or an opening bracket or quote, after Chinese or Japanese, or before leading
///   punctuation.
/// - Casing: after a sentence's end the first word is capitalized, unless the text there shows
///   the person writes in lowercase; mid-sentence a common first word is lowercased. A name, "I",
///   or an acronym keeps its capital.
/// - Joining (`can_join`): a dictation starting with a comma right after a period finishes that
///   sentence and goes over the period. Without `can_join`, or where a comma cannot follow, the
///   comma is dropped and the dictation is its own sentence.
///
/// Unreadable (`None`) leaves the text as given, with no space: a missing space is a one-key fix,
/// while a guessed one would land at the start of a fresh line or message.
pub fn fit_to_caret(text: &str, before: Option<&str>, can_join: bool) -> CaretFit {
    let fit = |text: String, join| CaretFit { text, join };
    let context = before.and_then(caret_context);
    let comma = text
        .strip_prefix(',')
        .map(|rest| rest.trim_start_matches(char::is_whitespace));
    if let Some(rest) = comma {
        if before.is_some() && context == Some(Caret::Mid) {
            return fit(text.to_owned(), false);
        }
        if rest.is_empty() {
            return fit(String::new(), false);
        }
        if can_join && before.is_some_and(joinable) {
            return fit(format!(", {}", lowercase_first(rest)), true);
        }
        // What followed the comma is a sentence of its own after all.
        let rest = if context == Some(Caret::Sentence) {
            rest.to_owned()
        } else {
            capitalize_first(rest)
        };
        return fit_to_caret(&rest, before, false);
    }
    let Some(before) = before else {
        return fit(text.to_owned(), false);
    };
    let cased = match context {
        Some(Caret::Sentence) if !writes_lowercase(before) => capitalize_first(text),
        Some(Caret::Mid) => lowercase_first(text),
        _ => text.to_owned(),
    };
    if needs_space(before, &cased) {
        fit(format!(" {cased}"), false)
    } else {
        fit(cased, false)
    }
}

// Closes what came before it, so it takes no space ahead of it.
const LEADING_PUNCTUATION: &str = ".,!?;:)]}%'’”»…";
// Opens what comes after it, or binds two words together.
const OPENS: &str = "([{<“‘«¿¡-/@#_—$";
const SENTENCE_END: &str = ".!?。！？";
// Closing quotes and brackets that can follow a sentence's end ("It works.)").
const CLOSERS: &str = "\"'”’)]}»";
// Abbreviations whose period does not end the sentence ("Dr.", "e.g."), besides initials.
const ABBREVIATIONS: &[&str] = &[
    "e.g", "i.e", "mr", "mrs", "ms", "dr", "st", "vs", "cf", "approx", "jr", "sr", "prof",
];
const OBJECT_REPLACEMENT: char = '\u{FFFC}';

/// Where the caret is in its sentence, or `None` when that cannot be told.
fn caret_context(before: &str) -> Option<Caret> {
    let line = before.rsplit('\n').next().unwrap_or(before);
    if line.chars().all(|c| c == ' ' || c == '\t') {
        return Some(Caret::Start);
    }
    let trimmed = before.trim_end_matches([' ', '\t', '\u{a0}']);
    let last = trimmed.chars().last()?;
    if OPENS.contains(last) {
        return None;
    }
    let end = trimmed.trim_end_matches(|c| CLOSERS.contains(c));
    if end.ends_with("..") || end.ends_with('…') || ends_with_abbreviation(end) {
        return None;
    }
    let last = end.chars().last()?;
    if SENTENCE_END.contains(last) {
        return Some(Caret::Sentence);
    }
    if is_word_char(last) || last == ',' || last == ';' || last == OBJECT_REPLACEMENT {
        return Some(Caret::Mid);
    }
    None
}

fn ends_with_abbreviation(end: &str) -> bool {
    let Some(body) = end.strip_suffix('.') else {
        return false;
    };
    let token = body
        .rsplit(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or(body);
    let mut chars = token.chars();
    let single_letter =
        matches!((chars.next(), chars.next()), (Some(c), None) if c.is_alphabetic());
    single_letter || ABBREVIATIONS.contains(&token.to_lowercase().as_str())
}

/// A period right at the caret that ends a sentence, which a comma can take the place of.
fn joinable(before: &str) -> bool {
    let mut chars = before.chars().rev();
    matches!((chars.next(), chars.next()), (Some('.'), Some(c)) if is_word_char(c))
        && caret_context(before) == Some(Caret::Sentence)
}

fn needs_space(before: &str, text: &str) -> bool {
    let (Some(last), Some(first)) = (before.chars().last(), text.chars().next()) else {
        return false;
    };
    if last.is_whitespace() || first.is_whitespace() || LEADING_PUNCTUATION.contains(first) {
        return false;
    }
    if OPENS.contains(last) || is_unspaced_writing(last) {
        return false;
    }
    if last == '"' || last == '\'' {
        // A straight quote opens when nothing but space or an opening comes before it.
        let previous = before.chars().rev().nth(1);
        return previous.is_some_and(|c| !(c.is_whitespace() || "([{".contains(c)));
    }
    true
}

/// Whether the sentences before the caret start in lowercase: then so does the next one.
fn writes_lowercase(before: &str) -> bool {
    // Cut from a longer text, its first letter may be mid-sentence.
    let cut = char_len(before) > 400;
    let chars: Vec<char> = tail_chars(before, 400).chars().collect();
    let mut starts = Vec::new();
    if !cut {
        starts.push(0);
    }
    for (i, &c) in chars.iter().enumerate() {
        let mut j = i + 1;
        if ".!?".contains(c) {
            while j < chars.len() && "\"'”’)".contains(chars[j]) {
                j += 1;
            }
            let gap = j;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j > gap {
                starts.push(j);
            }
        } else if c == '\n' {
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            starts.push(j);
        }
    }
    let letters: Vec<char> = starts
        .into_iter()
        .filter_map(|i| chars.get(i).copied())
        .filter(|c| c.is_alphabetic())
        .collect();
    !letters.is_empty()
        && letters
            .iter()
            .all(|c| c.to_uppercase().ne(std::iter::once(*c)))
}

/// The first word, after any opening quotes or brackets, as (opening, word, rest after word).
fn first_word(text: &str) -> Option<(&str, &str, &str)> {
    let open_len = text
        .char_indices()
        .find(|(_, c)| !"(\"'“‘[¿¡".contains(*c))
        .map_or(text.len(), |(i, _)| i);
    let rest = &text[open_len..];
    let word_len = rest
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map_or(rest.len(), |(i, _)| i);
    if word_len == 0 {
        return None;
    }
    Some((&text[..open_len], &rest[..word_len], &rest[word_len..]))
}

fn capitalize_first(text: &str) -> String {
    let Some((open, word, rest)) = first_word(text) else {
        return text.to_owned();
    };
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return text.to_owned();
    };
    let tail = chars.as_str();
    // Mixed case ("iPhone"), and addresses, handles and file names keep how they are written.
    let dotted_letter = word
        .chars()
        .zip(word.chars().skip(1))
        .any(|(a, b)| a == '.' && b.is_alphabetic());
    if !first.is_lowercase()
        || tail.chars().any(char::is_uppercase)
        || word
            .chars()
            .any(|c| "@/_".contains(c) || c.is_ascii_digit())
        || dotted_letter
    {
        return text.to_owned();
    }
    format!("{open}{}{tail}{rest}", first.to_uppercase())
}

fn lowercase_first(text: &str) -> String {
    let Some((open, token, rest)) = first_word(text) else {
        return text.to_owned();
    };
    let word = token.trim_end_matches(|c: char| !(c.is_alphabetic() || c == '\'' || c == '’'));
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return text.to_owned();
    };
    let tail = chars.as_str();
    let plain = first.is_uppercase()
        && tail
            .chars()
            .all(|c| c.is_lowercase() || c == '\'' || c == '’');
    if !plain || !COMMON_WORDS.contains(&word.to_lowercase().replace('’', "'").as_str()) {
        return text.to_owned();
    }
    let after_first = &token[first.len_utf8()..];
    format!("{open}{}{after_first}{rest}", first.to_lowercase())
}

/// Words that start a sentence capitalized only because they start it. Names, "I", acronyms, and
/// words that are also names or months ("Will", "May", "Mark", "Grace") are never here.
const COMMON_WORDS: &[&str] = &[
    "a",
    "about",
    "above",
    "after",
    "again",
    "against",
    "all",
    "almost",
    "already",
    "also",
    "although",
    "always",
    "am",
    "among",
    "an",
    "and",
    "another",
    "any",
    "anyone",
    "anything",
    "anyway",
    "are",
    "as",
    "at",
    "be",
    "because",
    "been",
    "before",
    "being",
    "below",
    "besides",
    "between",
    "both",
    "but",
    "by",
    "can",
    "can't",
    "cannot",
    "could",
    "couldn't",
    "did",
    "didn't",
    "do",
    "does",
    "doesn't",
    "doing",
    "don't",
    "done",
    "down",
    "during",
    "each",
    "either",
    "else",
    "even",
    "ever",
    "every",
    "everyone",
    "everything",
    "except",
    "few",
    "finally",
    "first",
    "for",
    "from",
    "further",
    "had",
    "hadn't",
    "has",
    "hasn't",
    "have",
    "haven't",
    "having",
    "he",
    "he'd",
    "he'll",
    "he's",
    "her",
    "here",
    "here's",
    "hers",
    "herself",
    "him",
    "himself",
    "his",
    "how",
    "how's",
    "however",
    "if",
    "in",
    "instead",
    "into",
    "is",
    "isn't",
    "it",
    "it'd",
    "it'll",
    "it's",
    "its",
    "itself",
    "just",
    "later",
    "least",
    "less",
    "let",
    "let's",
    "like",
    "maybe",
    "me",
    "meanwhile",
    "might",
    "more",
    "most",
    "much",
    "must",
    "my",
    "myself",
    "neither",
    "never",
    "next",
    "no",
    "none",
    "nor",
    "not",
    "nothing",
    "now",
    "of",
    "off",
    "often",
    "on",
    "once",
    "one",
    "only",
    "or",
    "other",
    "otherwise",
    "our",
    "ours",
    "ourselves",
    "out",
    "over",
    "perhaps",
    "please",
    "plus",
    "probably",
    "quite",
    "rather",
    "really",
    "same",
    "she",
    "she'd",
    "she'll",
    "she's",
    "should",
    "shouldn't",
    "since",
    "so",
    "some",
    "someone",
    "something",
    "sometimes",
    "soon",
    "still",
    "such",
    "than",
    "thanks",
    "that",
    "that's",
    "the",
    "their",
    "theirs",
    "them",
    "themselves",
    "then",
    "there",
    "there's",
    "therefore",
    "these",
    "they",
    "they'd",
    "they'll",
    "they're",
    "they've",
    "this",
    "those",
    "though",
    "through",
    "thus",
    "to",
    "too",
    "under",
    "unless",
    "until",
    "up",
    "us",
    "usually",
    "very",
    "was",
    "wasn't",
    "we",
    "we'd",
    "we'll",
    "we're",
    "we've",
    "were",
    "weren't",
    "what",
    "what's",
    "whatever",
    "when",
    "where",
    "where's",
    "whether",
    "which",
    "while",
    "who",
    "who's",
    "whom",
    "whose",
    "why",
    "why's",
    "with",
    "within",
    "without",
    "won't",
    "would",
    "wouldn't",
    "yes",
    "yet",
    "you",
    "you'd",
    "you'll",
    "you're",
    "you've",
    "your",
    "yours",
    "yourself",
    "yourselves",
    "ok",
    "okay",
    "sure",
    "yeah",
    "hey",
    "hi",
    "hello",
    "thank",
    "actually",
    "anyhow",
    "basically",
    "especially",
    "honestly",
    "hopefully",
    "ideally",
    "oh",
    "well",
    "right",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str, before: Option<&str>) -> String {
        fit_to_caret(text, before, false).text
    }

    #[test]
    fn spaces_a_dictation_from_the_sentence_it_follows_and_starts_the_next_sentence() {
        assert_eq!(
            typed("If it's ready to go.", Some("Start using it right now.")),
            " If it's ready to go."
        );
        assert_eq!(
            typed("if it's ready to go.", Some("Start using it right now.")),
            " If it's ready to go."
        );
        assert_eq!(typed("Done.", Some("Is it ready?")), " Done.");
    }

    #[test]
    fn types_no_space_at_a_line_start_after_space_or_opening_before_punctuation_or_after_cjk() {
        assert_eq!(typed("Hello.", Some("")), "Hello.");
        assert_eq!(typed("Hello.", Some("Notes\n")), "Hello.");
        assert_eq!(typed("hello", Some("Notes:\n  ")), "hello");
        assert_eq!(typed("Then.", Some("It works. ")), "Then.");
        assert_eq!(typed("see below", Some("Details (")), "see below");
        assert_eq!(typed("launch", Some("#")), "launch");
        assert_eq!(typed(", then", Some("First")), ", then");
        assert_eq!(typed("確認します", Some("了解です。")), "確認します");
        assert_eq!(typed("明天见", Some("好的")), "明天见");
        assert_eq!(
            typed("다음 주에 봐요.", Some("좋아요.")),
            " 다음 주에 봐요."
        );
    }

    #[test]
    fn continues_a_sentence_in_lowercase_but_keeps_names_i_and_acronyms() {
        assert_eq!(
            typed("If it's ready to go.", Some("Start using it right now, ")),
            "if it's ready to go."
        );
        assert_eq!(
            typed("If it's ready to go.", Some("Start using it right now")),
            " if it's ready to go."
        );
        assert_eq!(
            typed("Mina said yes.", Some("I asked and")),
            " Mina said yes."
        );
        assert_eq!(typed("I think so.", Some("Well,")), " I think so.");
        assert_eq!(
            typed("API keys rotate.", Some("Every week the")),
            " API keys rotate."
        );
        assert_eq!(typed("Über alles.", Some("Wir sind")), " Über alles.");
    }

    #[test]
    fn leaves_casing_alone_where_the_sentence_is_unclear_or_the_person_writes_lowercase() {
        assert_eq!(typed("then we ship", Some("Plan:")), " then we ship");
        assert_eq!(typed("Smith joined.", Some("Ask Dr.")), " Smith joined.");
        assert_eq!(typed("maybe not", Some("I was thinking...")), " maybe not");
        assert_eq!(
            typed("sounds good", Some("ok. see you there.")),
            " sounds good"
        );
        assert_eq!(
            typed("iPhone is fine.", Some("Use the phone.")),
            " iPhone is fine."
        );
        assert_eq!(
            typed("mina@acme.io works", Some("Use this.")),
            " mina@acme.io works"
        );
    }

    #[test]
    fn types_an_unreadable_fields_dictation_as_given() {
        assert_eq!(
            fit_to_caret("If it's ready to go.", None, true),
            CaretFit {
                text: "If it's ready to go.".into(),
                join: false
            }
        );
    }

    #[test]
    fn joins_over_the_period_only_when_the_host_can() {
        assert_eq!(
            fit_to_caret(
                ", if it's ready to go.",
                Some("Start using it right now."),
                true
            ),
            CaretFit {
                text: ", if it's ready to go.".into(),
                join: true
            }
        );
        assert_eq!(
            fit_to_caret(
                ", If it's ready to go.",
                Some("Start using it right now."),
                true
            )
            .text,
            ", if it's ready to go."
        );
        assert_eq!(
            fit_to_caret(
                ", if it's ready to go.",
                Some("Start using it right now."),
                false
            ),
            CaretFit {
                text: " If it's ready to go.".into(),
                join: false
            }
        );
        assert_eq!(
            typed(", if it's ready to go.", None),
            "If it's ready to go."
        );
        assert_eq!(
            typed(", if it's ready to go.", Some("")),
            "If it's ready to go."
        );
    }

    #[test]
    fn never_joins_over_a_question_an_exclamation_an_ellipsis_or_an_abbreviation() {
        assert_eq!(
            fit_to_caret(", if you can.", Some("Can you send it?"), true),
            CaretFit {
                text: " If you can.".into(),
                join: false
            }
        );
        assert!(!fit_to_caret(", if you can.", Some("Wait..."), true).join);
        assert!(!fit_to_caret(", if you can.", Some("Ask Dr."), true).join);
        assert!(!fit_to_caret(", if you can.", Some("It works. "), true).join);
    }
}
