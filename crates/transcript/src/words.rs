use owhisper_interface::{batch, stream::Word};
use uuid::Uuid;

use super::types::{FinalizedWord, PartialWord, RawWord, SpeakerHint, WordState};

// ── Assembly ──────────────────────────────────────────────────────────────────

/// Assemble raw ASR tokens into merged `RawWord`s.
///
/// The transcript string is the sole oracle for word boundaries within a
/// single response. Adjacent tokens without a space prefix are merged — no
/// timing heuristics.
pub(super) fn assemble(raw: &[Word], transcript: &str, channel: i32) -> Vec<RawWord> {
    let spaced = spacing_from_transcript(raw, transcript);
    let mut result: Vec<RawWord> = Vec::new();

    for (w, text) in raw.iter().zip(&spaced) {
        let start_ms = (w.start * 1000.0).round() as i64;
        let end_ms = (w.end * 1000.0).round() as i64;

        let should_merge = !text.starts_with(' ') && result.last().is_some();

        if should_merge {
            let last = result.last_mut().unwrap();
            last.text.push_str(text);
            last.end_ms = end_ms;
            if last.speaker.is_none() {
                last.speaker = w.speaker;
            }
        } else {
            result.push(RawWord {
                text: text.clone(),
                start_ms,
                end_ms,
                channel,
                speaker: w.speaker,
            });
        }
    }

    result
}

/// Assemble batch `Word`s using the same oracle-based spacing as `assemble`.
///
/// Unlike the streaming path, batch words don't need stitching — the response
/// is already final. Channel index comes from the caller (position in the
/// `results.channels` array).
pub(super) fn assemble_batch(raw: &[batch::Word], transcript: &str, channel: i32) -> Vec<RawWord> {
    let spaced = spacing_from_slice(
        raw.iter()
            .map(|w| (w.word.as_str(), w.punctuated_word.as_deref())),
        transcript,
    );
    let mut result: Vec<RawWord> = Vec::new();

    for (w, text) in raw.iter().zip(&spaced) {
        let start_ms = (w.start * 1000.0).round() as i64;
        let end_ms = (w.end * 1000.0).round() as i64;

        let should_merge = !text.starts_with(' ') && result.last().is_some();

        if should_merge {
            let last = result.last_mut().unwrap();
            last.text.push_str(text);
            last.end_ms = end_ms;
        } else {
            result.push(RawWord {
                text: text.clone(),
                start_ms,
                end_ms,
                channel,
                speaker: w.speaker.map(|s| s as i32),
            });
        }
    }

    result
}

/// Align each token to the transcript string and recover its spacing.
///
/// The transcript is the oracle: if a token is found, the whitespace before it
/// is prepended verbatim. If not found, a space is forced ("unknown = word
/// boundary").
fn spacing_from_transcript(raw: &[Word], transcript: &str) -> Vec<String> {
    spacing_from_slice(
        raw.iter()
            .map(|w| (w.word.as_str(), w.punctuated_word.as_deref())),
        transcript,
    )
}

/// Core spacing oracle: aligns each token to the transcript string and
/// recovers the whitespace that precedes it. Works for any word source that
/// provides `(word, punctuated_word)` pairs.
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
                let abs = pos + found;
                result.push(format!("{}{trimmed}", &transcript[pos..abs]));
                pos = abs + trimmed.len();
            }
            None => {
                let mut fallback = text.to_string();
                if !fallback.starts_with(' ') {
                    fallback.insert(0, ' ');
                }
                result.push(fallback);
            }
        }
    }

    result
}

// ── Pipeline stages ───────────────────────────────────────────────────────────

/// Drop words already covered by the watermark (deduplication).
pub(super) fn dedup(words: Vec<RawWord>, watermark: i64) -> Vec<RawWord> {
    words
        .into_iter()
        .skip_while(|w| w.end_ms <= watermark)
        .collect()
}

/// Cross-response word boundary handling.
///
/// Holds back the last word of each finalized batch so it can be merged with
/// the first word of the next batch if the provider split a word across
/// responses (common with Korean particles, contractions, etc.).
///
/// # Invariant
///
/// The returned held word **must** eventually be released by calling
/// `ChannelState::drain()` at session end. Dropping a `ChannelState` that
/// still has a held word will silently lose that word.
pub(super) fn stitch(
    held: Option<RawWord>,
    mut words: Vec<RawWord>,
) -> (Vec<RawWord>, Option<RawWord>) {
    if words.is_empty() {
        return (held.into_iter().collect(), None);
    }

    if let Some(h) = held {
        if should_stitch(&h, &words[0]) {
            words[0] = merge_words(h, words[0].clone());
        } else {
            words.insert(0, h);
        }
    }

    let new_held = words.pop();
    (words, new_held)
}

/// Maximum gap between consecutive responses for cross-response stitching.
///
/// 300 ms is conservative enough for most languages (including Korean particles
/// and English contractions) without accidentally joining words from separate
/// utterances. Providers vary in how quickly they finalize segments, so keep
/// this tight enough to avoid false positives.
const STITCH_MAX_GAP_MS: i64 = 300;

fn should_stitch(tail: &RawWord, head: &RawWord) -> bool {
    !head.text.starts_with(' ') && (head.start_ms - tail.end_ms) <= STITCH_MAX_GAP_MS
}

fn merge_words(mut left: RawWord, right: RawWord) -> RawWord {
    left.text.push_str(&right.text);
    left.end_ms = right.end_ms;
    if left.speaker.is_none() {
        left.speaker = right.speaker;
    }
    left
}

// ── Word-level transforms ─────────────────────────────────────────────────────

fn ensure_space_prefix(w: &mut RawWord) {
    if !w.text.starts_with(' ') {
        w.text.insert(0, ' ');
    }
}

pub(super) fn to_partial(w: &RawWord) -> PartialWord {
    let mut text = w.text.clone();
    if !text.starts_with(' ') {
        text.insert(0, ' ');
    }
    PartialWord {
        text,
        start_ms: w.start_ms,
        end_ms: w.end_ms,
        channel: w.channel,
    }
}

/// Convert finalized `RawWord`s into `FinalizedWord`s and `SpeakerHint`s.
pub(super) fn finalize_words(
    mut words: Vec<RawWord>,
    state: WordState,
) -> (Vec<FinalizedWord>, Vec<SpeakerHint>) {
    words.iter_mut().for_each(ensure_space_prefix);

    let mut final_words = Vec::with_capacity(words.len());
    let mut hints = Vec::new();

    for w in words {
        let id = Uuid::new_v4().to_string();

        if let Some(speaker_index) = w.speaker {
            hints.push(SpeakerHint {
                word_id: id.clone(),
                speaker_index,
            });
        }

        final_words.push(FinalizedWord {
            id,
            text: w.text,
            start_ms: w.start_ms,
            end_ms: w.end_ms,
            channel: w.channel,
            state,
        });
    }

    (final_words, hints)
}
