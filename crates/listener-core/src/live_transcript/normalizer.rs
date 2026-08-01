use std::collections::BTreeMap;

use owhisper_interface::stream::{Alternatives, StreamResponse, Word};

const SONIQO_CUMULATIVE_PREFIX_MIN_TOKENS: usize = 4;
const SONIQO_HISTORY_TOKEN_LIMIT: usize = 160;
const SONIQO_REPEAT_MIN_TOKENS: usize = 4;
const SONIQO_INTERNAL_REPEAT_MIN_TOKENS: usize = 6;
const SONIQO_INTERNAL_REPEAT_MAX_EXTRA_TOKENS: usize = 3;

#[derive(Default)]
pub(super) enum TranscriptNormalizer {
    Soniqo(SoniqoTranscriptNormalizer),
    AppleSpeech,
    #[default]
    Passthrough,
}

impl TranscriptNormalizer {
    pub(super) fn for_provider(provider_name: &str) -> Self {
        match provider_name {
            "soniqo" => Self::Soniqo(SoniqoTranscriptNormalizer::default()),
            "apple-speech" => Self::AppleSpeech,
            _ => Self::Passthrough,
        }
    }

    pub(super) fn normalize(&mut self, response: &mut StreamResponse) {
        match self {
            Self::Soniqo(normalizer) => normalizer.normalize(response),
            Self::AppleSpeech | Self::Passthrough => {}
        }
    }

    pub(super) fn finalize_partials(&self) -> bool {
        matches!(self, Self::Passthrough)
    }

    pub(super) fn flush_partials(&self) -> bool {
        !matches!(self, Self::AppleSpeech)
    }
}

#[derive(Default)]
pub(super) struct SoniqoTranscriptNormalizer {
    pub(super) channels: BTreeMap<i32, SoniqoChannelState>,
}

#[derive(Default)]
pub(super) struct SoniqoChannelState {
    pub(super) active_start_ms: Option<i64>,
    pub(super) active_tokens: Vec<String>,
    committed_tokens: Vec<String>,
}

impl SoniqoTranscriptNormalizer {
    pub(super) fn normalize(&mut self, response: &mut StreamResponse) {
        let StreamResponse::TranscriptResponse {
            start,
            duration,
            channel,
            channel_index,
            is_final,
            ..
        } = response
        else {
            return;
        };

        let Some(alternative) = channel.alternatives.first_mut() else {
            return;
        };
        if alternative.words.is_empty() {
            return;
        }

        let channel_idx = channel_index.first().copied().unwrap_or_default();
        let state = self.channels.entry(channel_idx).or_default();
        let mut current_tokens = normalize_tokens_for_overlap(&alternative.words);

        collapse_soniqo_internal_repeats(alternative, &mut current_tokens);
        if alternative.words.is_empty() {
            return;
        }
        sync_soniqo_timing(start, duration, &alternative.words);
        let mut current_start_ms =
            word_start_ms(alternative.words.first().expect("checked non-empty"));
        let mut current_end_ms = word_end_ms(alternative.words.last().expect("checked non-empty"));

        let committed_overlap = find_soniqo_history_prefix(
            &current_tokens,
            &state.committed_tokens,
            SONIQO_REPEAT_MIN_TOKENS,
        );
        if committed_overlap > 0 {
            drain_soniqo_prefix(alternative, &mut current_tokens, committed_overlap);

            if alternative.words.is_empty() {
                if *is_final {
                    state.active_start_ms = None;
                    state.active_tokens.clear();
                }
                return;
            }

            sync_soniqo_timing(start, duration, &alternative.words);
            current_start_ms = word_start_ms(alternative.words.first().expect("checked non-empty"));
            current_end_ms = word_end_ms(alternative.words.last().expect("checked non-empty"));
        }

        if is_soniqo_cumulative_update(&state.active_tokens, &current_tokens) {
            let active_start_ms = state.active_start_ms.unwrap_or(current_start_ms);
            retime_words(&mut alternative.words, active_start_ms, current_end_ms);
            *start = active_start_ms as f64 / 1000.0;
            *duration = ((current_end_ms - active_start_ms).max(50)) as f64 / 1000.0;
            state.active_start_ms = Some(active_start_ms);
        } else {
            let overlap = find_soniqo_overlap_prefix(&current_tokens, &state.active_tokens);
            if overlap > 0 {
                let overlapped_tokens = current_tokens.clone();
                let overlapped_start_ms = current_start_ms;
                drain_soniqo_prefix(alternative, &mut current_tokens, overlap);

                if alternative.words.is_empty() {
                    if *is_final {
                        state.active_start_ms = None;
                        state.active_tokens.clear();
                    } else {
                        state.active_start_ms = Some(overlapped_start_ms);
                        state.active_tokens = overlapped_tokens;
                    }
                    return;
                }

                sync_soniqo_timing(start, duration, &alternative.words);
                current_start_ms =
                    word_start_ms(alternative.words.first().expect("checked non-empty"));
                state.active_start_ms = Some(current_start_ms);
            } else if let Some(active_start_ms) = state.active_start_ms {
                retime_words(&mut alternative.words, active_start_ms, current_end_ms);
                *start = active_start_ms as f64 / 1000.0;
                *duration = ((current_end_ms - active_start_ms).max(50)) as f64 / 1000.0;
            } else {
                state.active_start_ms = Some(current_start_ms);
            }
        }

        if *is_final {
            extend_soniqo_committed_tokens(&mut state.committed_tokens, current_tokens);
            state.active_start_ms = None;
            state.active_tokens.clear();
        } else {
            state.active_tokens = current_tokens;
        }
    }
}

fn find_soniqo_overlap_prefix(current_tokens: &[String], previous_tokens: &[String]) -> usize {
    if current_tokens.is_empty() || previous_tokens.is_empty() {
        return 0;
    }

    let max_overlap = previous_tokens.len().min(current_tokens.len());

    for overlap in (1..=max_overlap).rev() {
        let previous_suffix = &previous_tokens[previous_tokens.len() - overlap..];
        let current_prefix = &current_tokens[..overlap];

        if previous_suffix == current_prefix {
            return overlap;
        }
    }

    0
}

fn find_soniqo_history_prefix(
    current_tokens: &[String],
    history_tokens: &[String],
    min_tokens: usize,
) -> usize {
    find_soniqo_history_prefix_match(current_tokens, history_tokens, min_tokens)
        .map(|(_, overlap)| overlap)
        .unwrap_or(0)
}

fn find_soniqo_history_prefix_match(
    current_tokens: &[String],
    history_tokens: &[String],
    min_tokens: usize,
) -> Option<(usize, usize)> {
    if current_tokens.len() < min_tokens || history_tokens.len() < min_tokens {
        return None;
    }

    let max_overlap = history_tokens.len().min(current_tokens.len());

    for overlap in (min_tokens..=max_overlap).rev() {
        let current_prefix = &current_tokens[..overlap];
        if let Some(start) = history_tokens
            .windows(overlap)
            .position(|tokens| tokens == current_prefix)
        {
            return Some((start, overlap));
        }
    }

    None
}

fn is_soniqo_cumulative_update(previous_tokens: &[String], current_tokens: &[String]) -> bool {
    if previous_tokens.is_empty() || current_tokens.is_empty() {
        return false;
    }

    if current_tokens.starts_with(previous_tokens) || previous_tokens.starts_with(current_tokens) {
        return true;
    }

    let common_prefix_len = common_prefix_len(previous_tokens, current_tokens);
    let shorter_len = previous_tokens.len().min(current_tokens.len());
    if common_prefix_len < SONIQO_CUMULATIVE_PREFIX_MIN_TOKENS
        || common_prefix_len + 1 < shorter_len
    {
        return false;
    }

    match (
        previous_tokens.get(common_prefix_len),
        current_tokens.get(common_prefix_len),
    ) {
        (Some(previous), Some(current)) => {
            previous.starts_with(current) || current.starts_with(previous)
        }
        _ => true,
    }
}

fn common_prefix_len(left: &[String], right: &[String]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}

fn collapse_soniqo_internal_repeats(
    alternative: &mut Alternatives,
    current_tokens: &mut Vec<String>,
) {
    let mut next_words = Vec::with_capacity(alternative.words.len());
    let mut next_tokens = Vec::with_capacity(current_tokens.len());
    let mut next_token_word_indexes = Vec::with_capacity(current_tokens.len());
    let word_tokens = alternative
        .words
        .iter()
        .map(normalize_word_token)
        .collect::<Vec<_>>();
    let mut index = 0;

    while index < alternative.words.len() {
        if word_tokens[index].is_empty() {
            next_words.push(alternative.words[index].clone());
            index += 1;
            continue;
        }

        let repeat = find_soniqo_history_prefix_match(
            &word_tokens[index..],
            &next_tokens,
            SONIQO_INTERNAL_REPEAT_MIN_TOKENS,
        );

        if let Some((history_start, overlap)) = repeat {
            let history_gap = next_tokens.len() - history_start;
            if history_gap <= overlap + SONIQO_INTERNAL_REPEAT_MAX_EXTRA_TOKENS {
                let remove_from = next_token_word_indexes[history_start];
                next_words.truncate(remove_from);
                let rebuilt = rebuild_soniqo_token_index(&next_words);
                next_tokens = rebuilt.0;
                next_token_word_indexes = rebuilt.1;
                continue;
            } else {
                index += overlap;
                continue;
            }
        }

        next_tokens.push(word_tokens[index].clone());
        next_token_word_indexes.push(next_words.len());
        next_words.push(alternative.words[index].clone());
        index += 1;
    }

    if next_words.len() == alternative.words.len() {
        return;
    }

    alternative.words = next_words;
    alternative.transcript = transcript_from_words(&alternative.words);
    *current_tokens = normalize_tokens_for_overlap(&alternative.words);
}

fn rebuild_soniqo_token_index(words: &[Word]) -> (Vec<String>, Vec<usize>) {
    let mut tokens = Vec::new();
    let mut word_indexes = Vec::new();

    for (index, word) in words.iter().enumerate() {
        let token = normalize_word_token(word);
        if token.is_empty() {
            continue;
        }

        tokens.push(token);
        word_indexes.push(index);
    }

    (tokens, word_indexes)
}

pub(super) fn drain_soniqo_prefix(
    alternative: &mut Alternatives,
    current_tokens: &mut Vec<String>,
    count: usize,
) {
    if count == 0 {
        return;
    }

    let mut drained_tokens = 0;
    let mut drained_words = 0;

    for word in &alternative.words {
        drained_words += 1;

        if !normalize_word_token(word).is_empty() {
            drained_tokens += 1;
            if drained_tokens == count {
                break;
            }
        }
    }

    while drained_words < alternative.words.len()
        && normalize_word_token(&alternative.words[drained_words]).is_empty()
    {
        drained_words += 1;
    }

    alternative.words.drain(..drained_words);
    alternative.transcript = transcript_from_words(&alternative.words);
    *current_tokens = normalize_tokens_for_overlap(&alternative.words);
}

fn extend_soniqo_committed_tokens(committed_tokens: &mut Vec<String>, tokens: Vec<String>) {
    committed_tokens.extend(tokens);

    if committed_tokens.len() > SONIQO_HISTORY_TOKEN_LIMIT {
        committed_tokens.drain(..committed_tokens.len() - SONIQO_HISTORY_TOKEN_LIMIT);
    }
}

fn sync_soniqo_timing(start: &mut f64, duration: &mut f64, words: &[Word]) {
    let (Some(first), Some(last)) = (words.first(), words.last()) else {
        return;
    };

    *start = first.start;
    *duration = (last.end - first.start).max(0.05);
}

pub(super) fn normalize_tokens_for_overlap(words: &[Word]) -> Vec<String> {
    words
        .iter()
        .map(normalize_word_token)
        .filter(|token| !token.is_empty())
        .collect()
}

fn retime_words(words: &mut [Word], start_ms: i64, end_ms: i64) {
    let count = words.len();
    if count == 0 {
        return;
    }

    let duration_ms = (end_ms - start_ms).max(50);

    for (index, word) in words.iter_mut().enumerate() {
        let word_start_ms = start_ms + (index as i64 * duration_ms / count as i64);
        let word_end_ms = if index + 1 == count {
            (start_ms + duration_ms - 50).max(word_start_ms + 50)
        } else {
            start_ms + ((index + 1) as i64 * duration_ms / count as i64)
        };

        word.start = word_start_ms as f64 / 1000.0;
        word.end = word_end_ms as f64 / 1000.0;
    }
}

fn transcript_from_words(words: &[Word]) -> String {
    words
        .iter()
        .map(|word| {
            word.punctuated_word
                .as_deref()
                .unwrap_or(word.word.as_str())
                .trim()
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_word_token(word: &Word) -> String {
    let raw = word
        .punctuated_word
        .as_deref()
        .unwrap_or(word.word.as_str());
    raw.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '\'')
        .to_ascii_lowercase()
}

fn word_start_ms(word: &Word) -> i64 {
    (word.start * 1000.0).round() as i64
}

fn word_end_ms(word: &Word) -> i64 {
    (word.end * 1000.0).round() as i64
}
