//! Whether a model's reply is the dictation, cleaned up or edited, or something else that must not
//! be typed. Adapted from Char's `dictation-polish` acceptance rules, with added checks for
//! looping replies and leaked prompt markup.

use crate::text::{char_len, is_latin, is_unspaced_writing, is_word_char, plain_words};

use super::RefineReason;

// Prompt markup a usable reply never contains.
const PROMPT_TAGS: &[&str] = &[
    "<selection>",
    "</selection>",
    "<instruction>",
    "</instruction>",
    "<instructions>",
    "</instructions>",
    "<before>",
    "</before>",
    "<screen>",
    "</screen>",
];

/// `reply` with each `<name>`, `<name/>`, `</name>` (spacing aside) removed, and whether any was.
fn strip_tag(reply: &str, name: &str) -> (String, bool) {
    let mut out = String::with_capacity(reply.len());
    let mut found = false;
    let mut rest = reply;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        let body = tail.strip_prefix('/').unwrap_or(tail);
        let tag_end = body.strip_prefix(name).and_then(|after| {
            let after = after.trim_start();
            let after = after.strip_prefix('/').unwrap_or(after);
            after.strip_prefix('>')
        });
        match tag_end {
            Some(after) => {
                found = true;
                rest = after;
            }
            None => {
                out.push('<');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    (out, found)
}

/// Reasoning a model wrote into its reply, removed; `None` when a block is left unclosed.
fn without_reasoning(reply: &str) -> Option<String> {
    let mut out = reply.to_owned();
    for (open, close) in [("<think>", "</think>"), ("<thinking>", "</thinking>")] {
        while let Some(start) = out.find(open) {
            let end = out[start..].find(close)? + start + close.len();
            out.replace_range(start..end, "");
        }
        if out.contains(close) {
            return None;
        }
    }
    Some(out)
}

fn has_prompt_markup(text: &str) -> bool {
    let lower = text.to_lowercase();
    PROMPT_TAGS.iter().any(|tag| lower.contains(tag))
}

/// The edited text, keeping the whitespace around the selection so it fits back in place.
pub(super) fn accept_edit(selection: &str, reply: &str) -> Result<String, RefineReason> {
    let reply = without_reasoning(reply).ok_or(RefineReason::Malformed)?;
    let start = reply.find("<result>").ok_or(RefineReason::Malformed)?;
    let end = reply.rfind("</result>").ok_or(RefineReason::Malformed)?;
    if end < start + "<result>".len() {
        return Err(RefineReason::Malformed);
    }
    let text = reply[start + "<result>".len()..end].trim();
    if text.is_empty() {
        return Err(RefineReason::EmptyReply);
    }
    if has_prompt_markup(text) || text.contains("<result>") {
        return Err(RefineReason::Malformed);
    }
    let lead = &selection[..selection.len() - selection.trim_start().len()];
    let trail = &selection[selection.trim_end().len()..];
    Ok(format!("{lead}{text}{trail}"))
}

/// The cleaned text, or why the raw transcript is typed instead: the model answered what it
/// heard, dropped most of it, repeated the text before the caret, looped, or replied with
/// markup. A self-correction resolved to what the speaker settled on (`<corrected/>`), personal
/// instructions, and spoken numbers or spellings written out can rightly change the length more.
pub(super) fn accept_cleanup(
    raw: &str,
    reply: &str,
    instructed: bool,
    before: Option<&str>,
) -> Result<String, RefineReason> {
    let reply = without_reasoning(reply).ok_or(RefineReason::Malformed)?;
    let (reply, corrected) = strip_tag(&reply, "corrected");
    let has_before = before.is_some_and(|b| !b.trim().is_empty());
    let joined = has_before && starts_with_join(&reply);
    let (cleaned, _) = strip_tag(&reply, "join");
    let cleaned = cleaned.trim();
    let text = if joined && !cleaned.is_empty() {
        let rest = cleaned.strip_prefix(',').map_or(cleaned, str::trim_start);
        format!(", {rest}")
    } else {
        cleaned.to_owned()
    };
    let original = raw.trim();
    if text.is_empty() {
        return Err(RefineReason::EmptyReply);
    }
    if (has_prompt_markup(&text) && !has_prompt_markup(original)) || text.contains("<result>") {
        return Err(RefineReason::Malformed);
    }
    if repeats_before(&text, original, before) {
        return Err(RefineReason::RepeatsBefore);
    }
    let length = char_len(&text) as f64;
    let original_length = char_len(original) as f64;
    let longest = if instructed {
        original_length * 2.0 + 100.0
    } else {
        original_length * 1.2 + 10.0
    };
    if length > longest {
        return Err(RefineReason::Answered);
    }
    let shortest = if written_characters(&text) > written_characters(original) {
        0.12
    } else if instructed || corrected {
        0.2
    } else {
        0.4
    };
    if length < written_length(original, &text) as f64 * shortest {
        return Err(RefineReason::Dropped);
    }
    if loops(&text, original) {
        return Err(RefineReason::Repetition);
    }
    Ok(text)
}

fn starts_with_join(reply: &str) -> bool {
    let rest = reply.trim_start();
    let Some(rest) = rest.strip_prefix("<join") else {
        return false;
    };
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    rest.starts_with('>')
}

/// Whether `text` repeats the sentence before the caret, already in the field, without the
/// person having said it again. A short sentence ("Thanks.") can rightly come up again.
fn repeats_before(text: &str, raw: &str, before: Option<&str>) -> bool {
    let Some(before) = before else { return false };
    let before = before.trim();
    let chars: Vec<char> = before.chars().collect();
    let mut start = 0;
    let mut last_visible: Option<char> = None;
    for (i, &c) in chars.iter().enumerate() {
        if c == '\n' || (c.is_whitespace() && last_visible.is_some_and(|l| ".!?".contains(l))) {
            start = i + 1;
        }
        if !c.is_whitespace() {
            last_visible = Some(c);
        }
    }
    let sentence: String = chars[start..].iter().collect();
    if char_len(&sentence) < 15 {
        return false;
    }
    let words = plain_words(&sentence);
    let words: Vec<&str> = words.split_whitespace().collect();
    let tail = format!(" {} ", words[words.len().saturating_sub(6)..].join(" "));
    plain_words(text).contains(&tail) && !plain_words(raw).contains(&tail)
}

fn is_currency(c: char) -> bool {
    matches!(
        c,
        '$' | '¢'
            | '£'
            | '¤'
            | '¥'
            | '֏'
            | '؋'
            | '৲'
            | '৳'
            | '฿'
            | '៛'
            | '﷼'
            | '﹩'
            | '＄'
            | '￠'
            | '￡'
            | '￥'
            | '￦'
    ) || ('\u{20A0}'..='\u{20CF}').contains(&c)
}

fn written_characters(text: &str) -> usize {
    text.chars()
        .filter(|&c| c.is_numeric() || is_currency(c) || c == '%' || c == '@')
        .count()
}

fn is_word_at(chars: &[char], i: usize) -> bool {
    chars.get(i).copied().is_some_and(is_word_char)
}

/// `raw` with each bounded, case-insensitive `word` (from `words`) replaced by `to`, swallowing
/// the whitespace around it when `swallow` is set.
fn replace_spoken(raw: &[char], words: &[&str], to: &str, swallow: bool) -> Vec<char> {
    let mut out: Vec<char> = Vec::with_capacity(raw.len());
    let mut i = 0;
    'scan: while i < raw.len() {
        if !is_word_at(raw, i.wrapping_sub(1)) {
            for word in words {
                let word: Vec<char> = word.chars().collect();
                let end = i + word.len();
                if end <= raw.len()
                    && raw[i..end]
                        .iter()
                        .zip(&word)
                        .all(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
                    && !is_word_at(raw, end)
                {
                    let mut next = end;
                    if swallow {
                        while out.last().is_some_and(|c| c.is_whitespace()) {
                            out.pop();
                        }
                        while raw.get(next).is_some_and(|c| c.is_whitespace()) {
                            next += 1;
                        }
                    }
                    out.extend(to.chars());
                    i = next;
                    continue 'scan;
                }
            }
        }
        out.push(raw[i]);
        i += 1;
    }
    out
}

/// How long the transcript is once its spelling and spoken signs are written: "double u" is only
/// "w", "J O N" only "JON", "dot" only ".", and a word followed by the spelling the cleaned text
/// kept is only that spelling. Every other word still counts, so dropping words still falls short.
fn written_length(raw: &str, polished: &str) -> usize {
    let chars: Vec<char> = raw.chars().collect();
    let chars = replace_spoken(
        &chars,
        &[
            "double u",
            "double-u",
            "doubleu",
            "double you",
            "double-you",
            "doubleyou",
        ],
        "w",
        false,
    );
    let chars = spelled_letters(&chars, &polished.to_lowercase());
    replace_spoken(
        &chars,
        &[
            "dot",
            "slash",
            "colon",
            "underscore",
            "hyphen",
            "dash",
            "hashtag",
        ],
        ".",
        true,
    )
    .len()
}

/// Latin letters said one by one ("J O N", "J-O-N") written as the word they spell, with the
/// word said before them dropped when the cleaned text kept that spelling.
fn spelled_letters(chars: &[char], kept: &str) -> Vec<char> {
    let is_letter = |i: usize| {
        chars
            .get(i)
            .copied()
            .is_some_and(|c| is_latin(c) && c.is_alphabetic())
            && !is_word_at(chars, i.wrapping_sub(1))
            && !is_word_at(chars, i + 1)
    };
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let mut letters = Vec::new();
        let mut j = i;
        let mut end = i;
        while is_letter(j) {
            letters.push(chars[j]);
            end = j + 1;
            let mut k = j + 1;
            while chars
                .get(k)
                .is_some_and(|c| c.is_whitespace() || ".,-".contains(*c))
            {
                k += 1;
            }
            if k == j + 1 || !is_letter(k) {
                break;
            }
            j = k;
        }
        if letters.len() >= 2 {
            if chars.get(end) == Some(&'.') {
                end += 1;
            }
            let spelled: String = letters.iter().collect();
            if kept.contains(&spelled.to_lowercase()) {
                drop_spelled_word(&mut out);
            }
            out.extend(letters);
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Removes the word (two letters or more) and separators just before a spelling, as in
/// "Jon, J O N": the spelling stands for it.
fn drop_spelled_word(out: &mut Vec<char>) {
    let mut cut = out.len();
    while cut > 0 && (out[cut - 1].is_whitespace() || ",:;-".contains(out[cut - 1])) {
        cut -= 1;
    }
    if cut == out.len() {
        return;
    }
    let mut start = cut;
    while start > 0 && (out[start - 1].is_alphabetic() || "'’".contains(out[start - 1])) {
        start -= 1;
    }
    if cut - start >= 2 {
        out.truncate(start);
    }
}

/// Tokens for repetition: Chinese and Japanese characters one by one, other words whole.
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if is_unspaced_writing(c) && c.is_alphanumeric() {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            out.push(c.to_string());
        } else if is_word_char(c) || "'’".contains(c) {
            word.push(c);
        } else if !word.is_empty() {
            out.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// The most times any run of `period` tokens repeats back to back.
fn longest_loop(tokens: &[String], period: usize) -> usize {
    let mut best = 1;
    let mut start = 0;
    while start + period <= tokens.len() {
        let mut repeats = 1;
        while start + (repeats + 1) * period <= tokens.len()
            && tokens[start..start + period]
                == tokens[start + repeats * period..start + (repeats + 1) * period]
        {
            repeats += 1;
        }
        best = best.max(repeats);
        start += 1;
    }
    best
}

/// Whether the reply loops: a phrase repeated back to back at least three times, spanning six
/// tokens or more, more often than the speaker said it.
fn loops(text: &str, raw: &str) -> bool {
    let reply = tokens(text);
    let said = tokens(raw);
    (1..=8).any(|period| {
        let repeats = longest_loop(&reply, period);
        repeats >= 3 && repeats * period >= 6 && repeats > longest_loop(&said, period)
    })
}
