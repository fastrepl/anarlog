use owhisper_interface::{batch, stream::Word};

use crate::types::RawWord;

pub(crate) fn assemble(raw: &[Word], transcript: &str, channel: i32) -> Vec<RawWord> {
    assemble_words(
        raw.iter().map(|word| AssemblyToken {
            word: word.word.as_str(),
            punctuated_word: word.punctuated_word.as_deref(),
            start_ms: (word.start * 1000.0).round() as i64,
            end_ms: (word.end * 1000.0).round() as i64,
            speaker: word.speaker,
        }),
        transcript,
        channel,
    )
}

pub(crate) fn assemble_batch(raw: &[batch::Word], transcript: &str, channel: i32) -> Vec<RawWord> {
    assemble_words(
        raw.iter().map(|word| AssemblyToken {
            word: word.word.as_str(),
            punctuated_word: word.punctuated_word.as_deref(),
            start_ms: (word.start * 1000.0).round() as i64,
            end_ms: (word.end * 1000.0).round() as i64,
            speaker: word.speaker.map(|speaker| speaker as i32),
        }),
        transcript,
        channel,
    )
}

#[derive(Clone, Copy)]
struct AssemblyToken<'a> {
    word: &'a str,
    punctuated_word: Option<&'a str>,
    start_ms: i64,
    end_ms: i64,
    speaker: Option<i32>,
}

fn assemble_words<'a>(
    tokens: impl Iterator<Item = AssemblyToken<'a>>,
    transcript: &str,
    channel: i32,
) -> Vec<RawWord> {
    let tokens: Vec<AssemblyToken<'a>> = tokens.collect();
    let spaced = spacing_from_slice(
        tokens
            .iter()
            .map(|token| (token.word, token.punctuated_word)),
        transcript,
    );
    let mut result = Vec::new();

    for (token, text) in tokens.into_iter().zip(spaced) {
        push_assembled_word(&mut result, token, text, channel);
    }

    result
}

fn push_assembled_word(
    result: &mut Vec<RawWord>,
    token: AssemblyToken<'_>,
    text: String,
    channel: i32,
) {
    let should_merge = !text.starts_with(' ') && result.last().is_some();

    if should_merge {
        let last = result.last_mut().unwrap();
        last.text.push_str(&text);
        last.end_ms = token.end_ms;
        if last.speaker.is_none() {
            last.speaker = token.speaker;
        }
        return;
    }

    result.push(RawWord {
        text,
        start_ms: token.start_ms,
        end_ms: token.end_ms,
        channel,
        speaker: token.speaker,
    });
}

fn spacing_from_slice<'a>(
    tokens: impl Iterator<Item = (&'a str, Option<&'a str>)>,
    transcript: &str,
) -> Vec<String> {
    let mut result = Vec::new();
    let mut pos = 0;

    for (word, punctuated) in tokens {
        let text = punctuated.unwrap_or(word);
        let trimmed = text.trim();

        if trimmed.is_empty() {
            result.push(text.to_string());
            continue;
        }

        match transcript[pos..].find(trimmed) {
            Some(found) => {
                let absolute = pos + found;
                result.push(format!("{}{trimmed}", &transcript[pos..absolute]));
                pos = absolute + trimmed.len();
            }
            None => result.push(with_leading_space(text)),
        }
    }

    result
}

fn with_leading_space(text: &str) -> String {
    let mut fallback = text.to_string();
    if !fallback.starts_with(' ') {
        fallback.insert(0, ' ');
    }
    fallback
}
