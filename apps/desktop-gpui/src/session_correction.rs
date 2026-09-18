//! `chat/tools/session-correction.ts`: the exact / bounded / loose text
//! replacements the `apply_session_correction` tool plans over a session's
//! summaries, transcripts, and title.

use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub text: String,
    pub count: usize,
}

/// `replaceExact`: every occurrence, case-sensitive.
pub fn replace_exact(value: &str, old_text: &str, new_text: &str) -> Replacement {
    if old_text.is_empty() {
        return Replacement {
            text: value.to_string(),
            count: 0,
        };
    }
    let count = value.matches(old_text).count();
    if count == 0 {
        return Replacement {
            text: value.to_string(),
            count: 0,
        };
    }
    Replacement {
        text: value.replace(old_text, new_text),
        count,
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

/// `replaceBoundedExact`: case-insensitive matches not touching letters or
/// digits on either side.
pub fn replace_bounded_exact(value: &str, old_text: &str, new_text: &str) -> Replacement {
    if old_text.is_empty() {
        return Replacement {
            text: value.to_string(),
            count: 0,
        };
    }
    let lower_value = value.to_lowercase();
    let lower_old = old_text.to_lowercase();
    // A case fold that changes the byte length cannot map back to `value`.
    if lower_value.len() != value.len() || lower_old.len() != old_text.len() {
        return replace_exact(value, old_text, new_text);
    }
    let mut out = String::new();
    let mut cursor = 0;
    let mut count = 0;
    let mut search = 0;
    while let Some(found) = lower_value[search..].find(&lower_old) {
        let start = search + found;
        let end = start + lower_old.len();
        let before_ok = value[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word_char(c));
        let after_ok = value[end..].chars().next().is_none_or(|c| !is_word_char(c));
        if before_ok && after_ok {
            out.push_str(&value[cursor..start]);
            out.push_str(new_text);
            cursor = end;
            count += 1;
            search = end.max(start + 1);
        } else {
            search = start + lower_old[..].chars().next().map_or(1, char::len_utf8);
        }
        if search > value.len() {
            break;
        }
    }
    if count == 0 {
        return Replacement {
            text: value.to_string(),
            count: 0,
        };
    }
    out.push_str(&value[cursor..]);
    Replacement { text: out, count }
}

/// `trimTokenPunctuation`
fn trim_token_punctuation(value: &str) -> String {
    let trimmed = value.trim_matches(|c: char| !is_word_char(c));
    if trimmed.is_empty() {
        value.to_string()
    } else {
        trimmed.to_string()
    }
}

/// `normalizeComparableToken`: NFKC-ish lowercase with only letters and
/// digits kept.
pub fn normalize_comparable_token(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .filter(|c| is_word_char(*c))
        .collect()
}

/// `tokenizeReplacement`
pub fn tokenize_replacement(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .map(|item| trim_token_punctuation(item.trim()))
        .filter(|item| !item.is_empty())
        .collect()
}

/// `tokenizeComparable`
pub fn tokenize_comparable(value: &str) -> Vec<String> {
    tokenize_replacement(value)
        .iter()
        .map(|item| normalize_comparable_token(item))
        .filter(|item| !item.is_empty())
        .collect()
}

fn word_text(word: &Value) -> &str {
    word.get("text")
        .and_then(|t| t.as_str())
        .unwrap_or_default()
}

fn word_ms(word: &Value, key: &str) -> Option<f64> {
    word.get(key).and_then(|v| v.as_f64())
}

/// `wordRangeMatchesAt`
fn word_range_matches_at(words: &[Value], target: &[String], start: usize) -> bool {
    if target.is_empty() || start + target.len() > words.len() {
        return false;
    }
    target
        .iter()
        .enumerate()
        .all(|(index, text)| normalize_comparable_token(word_text(&words[start + index])) == *text)
}

/// `buildReplacementWords`: the replacement tokens spread evenly over the
/// original span, the first keeping its id and the rest `id:correction:n`.
fn build_replacement_words(original: &[Value], new_text: &str) -> Vec<Value> {
    let tokens = tokenize_replacement(new_text);
    if tokens.is_empty() {
        return Vec::new();
    }
    let first = original.first().cloned().unwrap_or_else(|| json!({}));
    let last = original.last().cloned().unwrap_or_else(|| first.clone());
    let start_ms = word_ms(&first, "start_ms");
    let end_ms = word_ms(&last, "end_ms").or(start_ms);
    let duration = match (start_ms, end_ms) {
        (Some(start), Some(end)) => (end - start).max(0.0),
        _ => 0.0,
    };
    let step = duration / tokens.len() as f64;
    let base_id = first
        .get("id")
        .and_then(|id| id.as_str())
        .filter(|id| !id.is_empty())
        .map(str::to_string);
    tokens
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let mut word = first.clone();
            let object = word.as_object_mut().expect("word objects");
            if index > 0 {
                object.insert(
                    "id".into(),
                    format!(
                        "{}:correction:{index}",
                        base_id.clone().unwrap_or_else(|| "word".to_string())
                    )
                    .into(),
                );
            }
            object.insert("text".into(), text.clone().into());
            if let Some(start) = start_ms {
                object.insert(
                    "start_ms".into(),
                    json!((start + step * index as f64).round() as i64),
                );
                object.insert(
                    "end_ms".into(),
                    json!((start + step * (index + 1) as f64).round() as i64),
                );
            }
            word
        })
        .collect()
}

/// `replaceTranscriptWords`
pub fn replace_transcript_words(
    words: &[Value],
    old_text: &str,
    new_text: &str,
) -> (Vec<Value>, usize) {
    let target = tokenize_comparable(old_text);
    let replacement_tokens = tokenize_replacement(new_text);
    if target.is_empty() || replacement_tokens.is_empty() || target.len() > words.len() {
        return (words.to_vec(), 0);
    }
    let mut next: Vec<Value> = Vec::new();
    let mut count = 0;
    let mut index = 0;
    while index < words.len() {
        if target.len() == 1 && replacement_tokens.len() == 1 {
            let word = &words[index];
            let replaced = replace_bounded_exact(word_text(word), old_text, new_text);
            if replaced.count > 0 {
                let mut word = word.clone();
                if let Some(object) = word.as_object_mut() {
                    object.insert("text".into(), replaced.text.into());
                }
                next.push(word);
                count += replaced.count;
                index += 1;
                continue;
            }
        }
        if word_range_matches_at(words, &target, index) {
            next.extend(build_replacement_words(
                &words[index..index + target.len()],
                new_text,
            ));
            index += target.len();
            count += 1;
            continue;
        }
        next.push(words[index].clone());
        index += 1;
    }
    if count == 0 {
        (words.to_vec(), 0)
    } else {
        (next, count)
    }
}

/// `TEXT_TOKEN_PATTERN`: `[\p{L}\p{N}]+(?:['’_-][\p{L}\p{N}]+)*` with each
/// match's comparable form and byte range.
fn find_comparable_text_tokens(value: &str) -> Vec<(String, usize, usize)> {
    let chars: Vec<(usize, char)> = value.char_indices().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !is_word_char(chars[i].1) {
            i += 1;
            continue;
        }
        let start = chars[i].0;
        let mut j = i;
        while j < chars.len() && is_word_char(chars[j].1) {
            j += 1;
        }
        // Joiners continue the token only when a word character follows.
        while j + 1 < chars.len()
            && matches!(chars[j].1, '\'' | '’' | '_' | '-')
            && is_word_char(chars[j + 1].1)
        {
            j += 1;
            while j < chars.len() && is_word_char(chars[j].1) {
                j += 1;
            }
        }
        let end = if j < chars.len() {
            chars[j].0
        } else {
            value.len()
        };
        let text = normalize_comparable_token(&value[start..end]);
        if !text.is_empty() {
            tokens.push((text, start, end));
        }
        i = j;
    }
    tokens
}

/// `replaceLoosePhrase`: the comparable token sequence, whatever punctuation
/// and case sit between.
pub fn replace_loose_phrase(value: &str, old_text: &str, new_text: &str) -> Replacement {
    let target = tokenize_comparable(old_text);
    let tokens = find_comparable_text_tokens(value);
    if target.is_empty() || target.len() > tokens.len() {
        return Replacement {
            text: value.to_string(),
            count: 0,
        };
    }
    let mut parts = String::new();
    let mut cursor = 0;
    let mut count = 0;
    let mut index = 0;
    while index < tokens.len() {
        let matches = target
            .iter()
            .enumerate()
            .all(|(offset, text)| tokens.get(index + offset).is_some_and(|t| t.0 == *text));
        if !matches {
            index += 1;
            continue;
        }
        let first = &tokens[index];
        let last = &tokens[index + target.len() - 1];
        parts.push_str(&value[cursor..first.1]);
        parts.push_str(new_text);
        cursor = last.2;
        count += 1;
        index += target.len();
    }
    if count == 0 {
        return Replacement {
            text: value.to_string(),
            count: 0,
        };
    }
    parts.push_str(&value[cursor..]);
    Replacement { text: parts, count }
}

/// `replaceTranscriptText`: bounded for single tokens, exact otherwise, the
/// loose phrase as the fallback.
pub fn replace_transcript_text(value: &str, old_text: &str, new_text: &str) -> Replacement {
    if tokenize_comparable(old_text).len() == 1 && tokenize_replacement(new_text).len() == 1 {
        let bounded = replace_bounded_exact(value, old_text, new_text);
        return if bounded.count > 0 {
            bounded
        } else {
            replace_loose_phrase(value, old_text, new_text)
        };
    }
    let exact = replace_exact(value, old_text, new_text);
    if exact.count > 0 {
        exact
    } else {
        replace_loose_phrase(value, old_text, new_text)
    }
}

/// `dictionaryKey`
pub fn dictionary_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_bounded_replacements() {
        assert_eq!(
            replace_exact("a b a", "a", "c"),
            Replacement {
                text: "c b c".into(),
                count: 2
            }
        );
        assert_eq!(replace_exact("abc", "", "x").count, 0);
        let bounded = replace_bounded_exact("Acme and acme, not Acmes.", "acme", "Zed");
        assert_eq!(bounded.text, "Zed and Zed, not Acmes.");
        assert_eq!(bounded.count, 2);
        assert_eq!(replace_bounded_exact("cat", "at", "x").count, 0);
    }

    #[test]
    fn tokens_follow_the_frontend() {
        assert_eq!(tokenize_replacement(" Hello,  world! "), ["Hello", "world"]);
        assert_eq!(tokenize_comparable("It's Q3-Review."), ["its", "q3review"]);
        assert_eq!(
            find_comparable_text_tokens("don't stop - now")
                .into_iter()
                .map(|t| t.0)
                .collect::<Vec<_>>(),
            ["dont", "stop", "now"]
        );
    }

    #[test]
    fn transcript_words_replace_singles_and_phrases() {
        let words = vec![
            json!({ "id": "w1", "text": "Hello", "start_ms": 0, "end_ms": 100 }),
            json!({ "id": "w2", "text": "open", "start_ms": 100, "end_ms": 200 }),
            json!({ "id": "w3", "text": "world.", "start_ms": 200, "end_ms": 400 }),
        ];
        let (single, count) = replace_transcript_words(&words, "world", "planet");
        assert_eq!(count, 1);
        assert_eq!(word_text(&single[2]), "planet.");
        let (phrase, count) = replace_transcript_words(&words, "open world", "Open World Games");
        assert_eq!(count, 1);
        assert_eq!(phrase.len(), 4);
        assert_eq!(phrase[1]["id"], "w2");
        assert_eq!(phrase[2]["id"], "w2:correction:1");
        assert_eq!(phrase[3]["id"], "w2:correction:2");
        assert_eq!(phrase[1]["start_ms"], 100);
        assert_eq!(phrase[3]["end_ms"], 400);
        assert_eq!(replace_transcript_words(&words, "missing", "x").1, 0);
    }

    #[test]
    fn loose_phrases_and_transcript_text() {
        // `open-world` is one token under `TEXT_TOKEN_PATTERN`, so the
        // two-token target does not match it; punctuation between tokens does.
        assert_eq!(
            replace_loose_phrase("The open-world game.", "open world", "OpenWorld").count,
            0
        );
        let loose = replace_loose_phrase("The open, World game.", "open world", "OpenWorld");
        assert_eq!(loose.text, "The OpenWorld game.");
        assert_eq!(loose.count, 1);
        assert_eq!(
            replace_transcript_text("Ship acme now", "acme", "Zed").text,
            "Ship Zed now"
        );
        assert_eq!(
            replace_transcript_text("open, world", "open world", "OW").text,
            "OW"
        );
        assert_eq!(dictionary_key("  Acme   Corp "), "acme corp");
    }
}
