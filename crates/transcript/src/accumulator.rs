use super::types::{FinalizedWord, PartialWord, RawWord, SpeakerHint, WordState};
use super::words::{dedup, finalize_words, stitch, to_partial};

pub(super) struct ChannelState {
    watermark: i64,
    held: Option<RawWord>,
    partials: Vec<RawWord>,
}

impl ChannelState {
    pub(super) fn new() -> Self {
        Self {
            watermark: 0,
            held: None,
            partials: Vec::new(),
        }
    }

    /// Process a confirmed final batch.
    ///
    /// Partials that end before this batch starts are promoted to final.
    /// Partials overlapping the final range are dropped.
    pub(super) fn apply_final(
        &mut self,
        words: Vec<RawWord>,
        state: WordState,
    ) -> (Vec<FinalizedWord>, Vec<SpeakerHint>) {
        let new_words = dedup(words, self.watermark);
        if new_words.is_empty() {
            return (vec![], vec![]);
        }

        let final_start = new_words.first().map_or(0, |w| w.start_ms);
        let final_end = new_words.last().map_or(0, |w| w.end_ms);

        let (pre_final, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.partials)
            .into_iter()
            .partition(|w| w.end_ms <= final_start);

        self.partials = rest
            .into_iter()
            .filter(|w| w.start_ms > final_end)
            .collect();
        self.watermark = final_end;

        let mut to_finalize: Vec<RawWord> = pre_final;
        let (emitted, held) = stitch(self.held.take(), new_words);
        self.held = held;
        to_finalize.extend(emitted);

        finalize_words(to_finalize, state)
    }

    /// Update the partial buffer with a simple time-range replacement.
    pub(super) fn apply_partial(&mut self, words: Vec<RawWord>) {
        let words = dedup(words, self.watermark);
        if words.is_empty() {
            return;
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
    }

    /// Drain remaining state at session end.
    ///
    /// The held word is always promoted. Remaining partials are promoted as
    /// final (they survived the session without being superseded).
    pub(super) fn drain(&mut self) -> (Vec<FinalizedWord>, Vec<SpeakerHint>) {
        let mut raw: Vec<RawWord> = self.held.take().into_iter().collect();
        raw.extend(std::mem::take(&mut self.partials));
        finalize_words(raw, WordState::Final)
    }

    pub(super) fn current_partials(&self) -> impl Iterator<Item = PartialWord> + '_ {
        self.partials.iter().chain(self.held.iter()).map(to_partial)
    }
}
