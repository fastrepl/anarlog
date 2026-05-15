#[derive(Debug, Clone, PartialEq)]
pub(super) struct TokenWord {
    pub text: String,
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub confidence: f64,
    pub speaker: Option<i32>,
    pub language: Option<String>,
}

#[derive(Default)]
struct PendingWord {
    text: String,
    start_ms: Option<u64>,
    end_ms: Option<u64>,
    speaker: Option<i32>,
    language: Option<String>,
    confidence_sum: f64,
    confidence_count: u32,
}

impl PendingWord {
    fn push_token(&mut self, token: &soniox::Token, text: &str) {
        if self.start_ms.is_none() {
            self.start_ms = token.start_ms;
        }
        self.end_ms = token.end_ms.or(self.end_ms);
        self.text.push_str(text);

        if self.speaker.is_none() {
            self.speaker = token.speaker.as_ref().and_then(|speaker| speaker.as_i32());
        }
        if self.language.is_none() {
            self.language = token.language.clone();
        }

        self.confidence_sum += token.confidence.unwrap_or(1.0);
        self.confidence_count += 1;
    }

    fn build(self) -> Option<TokenWord> {
        if self.text.is_empty() {
            return None;
        }

        let confidence = if self.confidence_count == 0 {
            1.0
        } else {
            self.confidence_sum / f64::from(self.confidence_count)
        };

        Some(TokenWord {
            text: self.text,
            start_ms: self.start_ms,
            end_ms: self.end_ms,
            confidence,
            speaker: self.speaker,
            language: self.language,
        })
    }
}

pub(super) fn build_token_words(tokens: &[&soniox::Token]) -> Vec<TokenWord> {
    token_groups_from_refs(tokens)
        .into_iter()
        .filter_map(|group| {
            let mut pending = PendingWord::default();
            for token in group {
                let trimmed = token.text.trim();
                if !trimmed.is_empty() {
                    pending.push_token(token, trimmed);
                }
            }
            pending.build()
        })
        .collect()
}

pub(super) fn partition_tokens_by_word_finality(
    tokens: &[soniox::Token],
) -> (Vec<&soniox::Token>, Vec<&soniox::Token>) {
    let mut final_tokens = Vec::new();
    let mut non_final_tokens = Vec::new();
    for group in token_groups_from_values(tokens) {
        if group.iter().all(|token| token.is_final.unwrap_or(true)) {
            final_tokens.extend(group);
        } else {
            non_final_tokens.extend(group);
        }
    }

    (final_tokens, non_final_tokens)
}

fn token_groups_from_refs<'a>(tokens: &[&'a soniox::Token]) -> Vec<Vec<&'a soniox::Token>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    let mut current_has_content = false;

    let flush = |groups: &mut Vec<Vec<&'a soniox::Token>>, current: &mut Vec<&'a soniox::Token>| {
        if !current.is_empty() {
            groups.push(std::mem::take(current));
        }
    };

    for token in tokens {
        let has_content = !token.text.trim().is_empty();
        let starts_with_ws = token.text.chars().next().is_some_and(char::is_whitespace);

        if starts_with_ws && current_has_content {
            flush(&mut groups, &mut current);
            current_has_content = false;
        }

        current.push(*token);
        current_has_content |= has_content;
    }

    flush(&mut groups, &mut current);
    groups
}

fn token_groups_from_values<'a>(tokens: &'a [soniox::Token]) -> Vec<Vec<&'a soniox::Token>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    let mut current_has_content = false;

    let flush = |groups: &mut Vec<Vec<&'a soniox::Token>>, current: &mut Vec<&'a soniox::Token>| {
        if !current.is_empty() {
            groups.push(std::mem::take(current));
        }
    };

    for token in tokens {
        let has_content = !token.text.trim().is_empty();
        let starts_with_ws = token.text.chars().next().is_some_and(char::is_whitespace);

        if starts_with_ws && current_has_content {
            flush(&mut groups, &mut current);
            current_has_content = false;
        }

        current.push(token);
        current_has_content |= has_content;
    }

    flush(&mut groups, &mut current);
    groups
}
