use super::types::{FinalizedWord, PartialWord, RawWord, WordState};
use super::words::{dedup, finalize_words, stitch, to_partial};

pub(super) const MAX_PARTIAL_WORDS: usize = 512;
pub(super) const MAX_PARTIAL_TEXT_BYTES: usize = 128 * 1024;
const MAX_PARTIAL_WINDOW_MS: i64 = 2 * 60 * 1000;

pub(super) struct ChannelState {
    watermark: i64,
    partial_watermark: i64,
    held: Option<RawWord>,
    partials: Vec<RawWord>,
}

impl ChannelState {
    pub(super) fn new() -> Self {
        Self {
            watermark: 0,
            partial_watermark: 0,
            held: None,
            partials: Vec::new(),
        }
    }

    /// Process a confirmed final batch.
    ///
    /// When partial finalization is enabled, partials that end before this
    /// batch starts are promoted to final.
    /// Partials overlapping the final range are dropped.
    pub(super) fn apply_final(
        &mut self,
        words: Vec<RawWord>,
        state: WordState,
        finalize_partials: bool,
    ) -> Vec<FinalizedWord> {
        let new_words = dedup(words, self.watermark);
        if new_words.is_empty() {
            return vec![];
        }

        let final_start = new_words.first().map_or(0, |w| w.start_ms);
        let final_end = new_words.last().map_or(0, |w| w.end_ms);

        let partials = std::mem::take(&mut self.partials);
        let (pre_final, rest): (Vec<_>, Vec<_>) = if finalize_partials {
            partials.into_iter().partition(|w| w.end_ms <= final_start)
        } else {
            (Vec::new(), partials)
        };

        self.partials = rest
            .into_iter()
            .filter(|w| w.start_ms > final_end)
            .collect();
        self.watermark = final_end;

        let mut to_finalize: Vec<RawWord> = pre_final;
        let (mut emitted, held) = stitch(self.held.take(), new_words);
        self.held = held;
        self.release_oversized_held(&mut emitted);
        to_finalize.extend(emitted);

        finalize_words(to_finalize, state)
    }

    /// Update the partial buffer with a simple time-range replacement.
    ///
    /// Old preview words are promoted when partials are commit-worthy. For
    /// snapshot-only providers, they are discarded without advancing the
    /// finalized watermark so a later confirmed final can still replace them.
    pub(super) fn apply_partial(
        &mut self,
        words: Vec<RawWord>,
        finalize_partials: bool,
    ) -> Vec<FinalizedWord> {
        let words = dedup(words, self.watermark.max(self.partial_watermark));
        if words.is_empty() {
            return vec![];
        }

        let first_start = words.first().map_or(0, |w| w.start_ms);
        let last_end = words.last().map_or(0, |w| w.end_ms);

        let before = self
            .partials
            .iter()
            .filter(|w| w.end_ms <= first_start)
            .cloned();
        let after = self
            .partials
            .iter()
            .filter(|w| w.start_ms >= last_end)
            .cloned();

        self.partials = before.chain(words).chain(after).collect();
        self.partials
            .sort_by_key(|word| (word.start_ms, word.end_ms));

        let overflow_count = self.overflow_prefix_len();
        if overflow_count == 0 {
            return vec![];
        }

        if !finalize_partials {
            let overflow: Vec<RawWord> = self.partials.drain(..overflow_count).collect();
            let overflow_end = overflow
                .iter()
                .map(|word| word.end_ms)
                .max()
                .unwrap_or_default();
            self.partial_watermark = self.partial_watermark.max(overflow_end);
            return vec![];
        }

        let mut emitted = Vec::new();
        let mut overflow_count = overflow_count;
        while overflow_count > 0 {
            let overflow: Vec<RawWord> = self.partials.drain(..overflow_count).collect();
            let overflow_end = overflow
                .iter()
                .map(|word| word.end_ms)
                .max()
                .unwrap_or_default();
            self.watermark = self.watermark.max(overflow_end);

            let (newly_emitted, held) = stitch(self.held.take(), overflow);
            self.held = held;
            emitted.extend(newly_emitted);
            self.release_oversized_held(&mut emitted);
            overflow_count = self.overflow_prefix_len();
        }

        finalize_words(emitted, WordState::Final)
    }

    /// Drain remaining state at session end.
    ///
    /// The held word is always promoted. Remaining partials are promoted as
    /// final (they survived the session without being superseded).
    pub(super) fn drain(&mut self) -> Vec<FinalizedWord> {
        let mut raw: Vec<RawWord> = self.held.take().into_iter().collect();
        raw.extend(std::mem::take(&mut self.partials));
        finalize_words(raw, WordState::Final)
    }

    pub(super) fn drain_final_words(&mut self) -> Vec<FinalizedWord> {
        let raw: Vec<RawWord> = self.held.take().into_iter().collect();
        self.partials.clear();
        finalize_words(raw, WordState::Final)
    }

    pub(super) fn current_partials(&self) -> impl Iterator<Item = PartialWord> + '_ {
        self.partials.iter().chain(self.held.iter()).map(to_partial)
    }

    fn overflow_prefix_len(&self) -> usize {
        let Some(latest_end) = self.partials.iter().map(|word| word.end_ms).max() else {
            return 0;
        };
        let age_cutoff = latest_end.saturating_sub(MAX_PARTIAL_WINDOW_MS);
        let age_overflow = self
            .partials
            .iter()
            .take_while(|word| word.end_ms <= age_cutoff)
            .count();
        let retained_word_limit =
            MAX_PARTIAL_WORDS.saturating_sub(usize::from(self.held.is_some()));
        let count_overflow = self.partials.len().saturating_sub(retained_word_limit);

        let mut retained_bytes = self.held.as_ref().map_or(0, |word| word.text.len());
        let mut text_keep_from = self.partials.len();
        for (index, word) in self.partials.iter().enumerate().rev() {
            let next_bytes = retained_bytes.saturating_add(word.text.len());
            if next_bytes > MAX_PARTIAL_TEXT_BYTES {
                break;
            }
            retained_bytes = next_bytes;
            text_keep_from = index;
        }
        let text_overflow = text_keep_from;

        age_overflow.max(count_overflow).max(text_overflow)
    }

    fn release_oversized_held(&mut self, emitted: &mut Vec<RawWord>) {
        if self
            .held
            .as_ref()
            .is_some_and(|word| word.text.len() > MAX_PARTIAL_TEXT_BYTES)
        {
            emitted.extend(self.held.take());
        }
    }
}
