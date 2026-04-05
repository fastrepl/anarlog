use owhisper_interface::{batch, stream::Word};

use crate::types::RawWord;

pub(crate) fn assemble(raw: &[Word], transcript: &str, channel: i32) -> Vec<RawWord> {
    assemble_words(
        raw.iter().map(|word| AssemblyToken {
            word: word.word.as_str(),
            punctuated_word: word.punctuated_word.as_deref(),
            start_ms: (word.start * 1000.0).round() as i64,
            end_ms: (word.end * 1000.0).round() as i64,
            channel,
            speaker: word.speaker,
        }),
        transcript,
        channel,
    )
}

pub(crate) fn assemble_batch(raw: &[batch::Word], transcript: &str) -> Vec<RawWord> {
    assemble_words(
        raw.iter().map(|word| AssemblyToken {
            word: word.word.as_str(),
            punctuated_word: word.punctuated_word.as_deref(),
            start_ms: (word.start * 1000.0).round() as i64,
            end_ms: (word.end * 1000.0).round() as i64,
            channel: word.channel,
            speaker: word.speaker.map(|speaker| speaker as i32),
        }),
        transcript,
        0,
    )
}

#[derive(Clone, Copy)]
struct AssemblyToken<'a> {
    word: &'a str,
    punctuated_word: Option<&'a str>,
    start_ms: i64,
    end_ms: i64,
    channel: i32,
    speaker: Option<i32>,
}

fn assemble_words<'a>(
    tokens: impl Iterator<Item = AssemblyToken<'a>>,
    transcript: &str,
    _channel: i32,
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
        push_assembled_word(&mut result, token, text);
    }

    result
}

fn push_assembled_word(result: &mut Vec<RawWord>, token: AssemblyToken<'_>, text: String) {
    let should_merge = !text.starts_with(' ')
        && result.last().is_some()
        && !should_insert_boundary_space(
            result.last().unwrap().text.as_str(),
            token.word,
            text.trim_start(),
        );

    if should_merge {
        merge_into_previous(result.last_mut().unwrap(), token, &text);
        return;
    }

    let text = if !text.starts_with(' ')
        && result.last().is_some()
        && should_insert_boundary_space(
            result.last().unwrap().text.as_str(),
            token.word,
            text.trim_start(),
        ) {
        format!(" {}", text.trim_start())
    } else {
        text
    };

    result.push(RawWord {
        text,
        start_ms: token.start_ms,
        end_ms: token.end_ms,
        channel: token.channel,
        speaker: token.speaker,
    });
}

fn merge_into_previous(previous: &mut RawWord, token: AssemblyToken<'_>, text: &str) {
    previous.text.push_str(text);
    previous.end_ms = token.end_ms;
    if previous.speaker.is_none() {
        previous.speaker = token.speaker;
    }
}

fn should_insert_boundary_space(previous_text: &str, word: &str, trimmed_text: &str) -> bool {
    let trimmed_previous = previous_text.trim_end();
    let Some(previous_char) = trimmed_previous.chars().next_back() else {
        return false;
    };

    if !is_boundary_punctuation(previous_char) {
        return false;
    }

    let next_char = trimmed_text
        .chars()
        .next()
        .or_else(|| word.chars().next())
        .filter(|ch| is_word_boundary_start(*ch));

    let Some(next_char) = next_char else {
        return false;
    };

    !is_period_continuation(trimmed_previous, previous_char, next_char)
}

fn is_boundary_punctuation(ch: char) -> bool {
    ",.;:!?)}]'\"".contains(ch)
}

fn is_word_boundary_start(ch: char) -> bool {
    ch.is_alphanumeric()
}

fn is_period_continuation(previous_text: &str, previous_char: char, next_char: char) -> bool {
    if previous_char != '.' {
        return false;
    }

    if next_char.is_numeric() {
        return true;
    }

    if !next_char.is_lowercase() {
        return false;
    }

    previous_text
        .strip_suffix('.')
        .and_then(|text| text.chars().next_back())
        .is_some_and(|ch| ch.is_alphanumeric())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn token<'a>(
        word: &'a str,
        punctuated: Option<&'a str>,
        start: i64,
        end: i64,
    ) -> AssemblyToken<'a> {
        AssemblyToken {
            word,
            punctuated_word: punctuated,
            start_ms: start,
            end_ms: end,
            channel: 0,
            speaker: None,
        }
    }

    #[test]
    fn merges_punctuation_into_previous_word() {
        let mut result = Vec::new();
        push_assembled_word(&mut result, token("look", None, 0, 100), "look".to_string());
        push_assembled_word(&mut result, token(".", None, 100, 110), ".".to_string());

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "look.");
    }

    #[test]
    fn does_not_merge_regular_words_without_space() {
        let mut result = Vec::new();
        push_assembled_word(
            &mut result,
            token("look", None, 0, 100),
            "look.".to_string(),
        );
        push_assembled_word(
            &mut result,
            token("Everyone", None, 110, 200),
            "Everyone".to_string(),
        );

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].text, "look.");
        assert_eq!(result[1].text, " Everyone");
    }

    #[test]
    fn preserves_spacing_from_good_transcript() {
        let transcript = "look. Everyone knows";
        let words = assemble_words(
            vec![
                token("look", Some("look."), 0, 100),
                token("everyone", Some("Everyone"), 110, 200),
                token("knows", Some("knows"), 210, 300),
            ]
            .into_iter(),
            transcript,
            0,
        );

        assert_eq!(words.len(), 3);
        assert_eq!(words[0].text, "look.");
        assert_eq!(words[1].text, " Everyone");
        assert_eq!(words[2].text, " knows");
    }

    #[test]
    fn fixes_spacing_from_bad_transcript() {
        let transcript = "look.Everyone knows";
        let words = assemble_words(
            vec![
                token("look", Some("look."), 0, 100),
                token("everyone", Some("Everyone"), 110, 200),
                token("knows", Some("knows"), 210, 300),
            ]
            .into_iter(),
            transcript,
            0,
        );

        assert_eq!(words.len(), 3);
        assert_eq!(words[0].text, "look.");
        assert_eq!(words[1].text, " Everyone");
        assert_eq!(words[2].text, " knows");
    }

    #[test]
    fn merges_contraction_into_previous_word() {
        let mut result = Vec::new();
        push_assembled_word(&mut result, token("it", None, 0, 100), " it".to_string());
        push_assembled_word(&mut result, token("'s", None, 100, 150), "'s".to_string());

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, " it's");
    }

    #[test]
    fn keeps_split_words_without_spacing() {
        let transcript = "millions";
        let words = assemble_words(
            vec![token("mill", None, 0, 100), token("ions", None, 100, 200)].into_iter(),
            transcript,
            0,
        );

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "millions");
    }

    #[test]
    fn keeps_unicode_split_words_without_spacing() {
        let transcript = "한국";
        let words = assemble_words(
            vec![token("한", None, 0, 100), token("국", None, 100, 200)].into_iter(),
            transcript,
            0,
        );

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "한국");
    }

    #[test]
    fn keeps_decimal_tokens_merged_after_period() {
        let transcript = "3.14";
        let words = assemble_words(
            vec![token("3", Some("3."), 0, 100), token("14", None, 100, 200)].into_iter(),
            transcript,
            0,
        );

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "3.14");
    }

    #[test]
    fn keeps_domain_tokens_merged_after_period() {
        let transcript = "example.com";
        let words = assemble_words(
            vec![
                token("example", Some("example."), 0, 100),
                token("com", None, 100, 200),
            ]
            .into_iter(),
            transcript,
            0,
        );

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "example.com");
    }
}
