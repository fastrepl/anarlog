use std::collections::{BTreeMap, HashMap};

use owhisper_interface::{
    batch::Response as BatchResponse,
    stream::{StreamResponse, Word},
};

use super::accumulator::ChannelState;
use super::types::{
    FinalizedWord, PartialWord, RuntimeSpeakerHint, TranscriptDelta, WordRef, WordState,
};
use super::words::{assemble, assemble_batch, finalize_words};

/// Stateful processor that converts raw `StreamResponse`s into
/// `TranscriptDelta`s and manages correction jobs from any source.
///
/// # Correction sources
///
/// All correction flows follow the same lifecycle:
///
/// 1. Words are finalized (with state `Pending` or `Final`)
/// 2. A correction source processes them asynchronously
/// 3. Correction resolves: pending words are replaced with corrected finals
/// 4. On timeout: pending words become final with original text
///
/// The processor supports two integration patterns:
///
/// - **Inline** (cactus cloud handoff): the streaming protocol itself carries
///   handoff/correction metadata. Handled automatically inside `process()`.
///
/// - **External** (LLM postprocessor, future sources): the caller finalizes
///   words via `process()`, then calls `submit_correction` / `apply_correction`
///   to manage the pending→final lifecycle.
pub struct TranscriptProcessor {
    channels: BTreeMap<i32, ChannelState>,
    pending_corrections: HashMap<u64, Vec<String>>,
    next_job_id: u64,
}

struct ParsedStreamResponse<'a> {
    is_final: bool,
    channel: i32,
    words: &'a [Word],
    transcript: &'a str,
    correction: CorrectionMetadata,
}

#[derive(Default)]
struct CorrectionMetadata {
    is_cloud_corrected: bool,
    is_cloud_handoff: bool,
    cloud_job_id: u64,
}

struct PartialSnapshot {
    partials: Vec<PartialWord>,
    partial_hints: Vec<RuntimeSpeakerHint>,
}

impl TranscriptProcessor {
    pub fn new() -> Self {
        Self {
            channels: BTreeMap::new(),
            pending_corrections: HashMap::new(),
            next_job_id: 1,
        }
    }

    /// Process one streaming response. Returns `None` for non-transcript
    /// responses or responses with no words.
    ///
    /// Cactus cloud handoff metadata (`cloud_handoff`, `cloud_corrected`) is
    /// handled inline: handoff words are emitted as `Pending`, corrections
    /// resolve the pending job and emit `replaced_ids`.
    pub fn process(&mut self, response: &StreamResponse) -> Option<TranscriptDelta> {
        let parsed = ParsedStreamResponse::from_response(response)?;
        let raw_words = assemble(parsed.words, parsed.transcript, parsed.channel);
        if raw_words.is_empty() {
            return None;
        }

        let channel_state = self
            .channels
            .entry(parsed.channel)
            .or_insert_with(ChannelState::new);

        if parsed.is_final {
            let word_state = if parsed.correction.is_handoff_job() {
                WordState::Pending
            } else {
                WordState::Final
            };

            let (new_words, hints) = channel_state.apply_final(raw_words, word_state);

            let replaced_ids = if parsed.correction.is_corrected_job() {
                self.resolve_job(parsed.correction.cloud_job_id)
            } else {
                vec![]
            };

            if parsed.correction.is_handoff_job() {
                let ids: Vec<String> = new_words.iter().map(|w| w.id.clone()).collect();
                self.register_job(parsed.correction.cloud_job_id, ids);
            }

            let snapshot = self.partial_snapshot();

            if new_words.is_empty() && replaced_ids.is_empty() {
                return None;
            }

            Some(snapshot.into_delta(new_words, hints, replaced_ids))
        } else {
            channel_state.apply_partial(raw_words);
            Some(self.partial_snapshot().into_delta(vec![], vec![], vec![]))
        }
    }

    // ── Generic correction API ──────────────────────────────────────────────

    /// Submit already-emitted `Final` words for asynchronous correction.
    ///
    /// Returns `(job_id, delta)`. The delta re-emits the words with
    /// `Pending` state and sets `replaced_ids` to their current IDs, so the
    /// frontend transitions them from Final→Pending.
    ///
    /// The caller should spawn the correction task and later call
    /// `apply_correction` with the same `job_id`.
    pub fn submit_correction(&mut self, words: Vec<FinalizedWord>) -> (u64, TranscriptDelta) {
        let job_id = self.next_job_id();
        let replaced_ids: Vec<String> = words.iter().map(|w| w.id.clone()).collect();

        self.register_job(job_id, replaced_ids.clone());

        let pending_words: Vec<FinalizedWord> = words
            .into_iter()
            .map(|w| FinalizedWord {
                state: WordState::Pending,
                ..w
            })
            .collect();

        let delta = self
            .partial_snapshot()
            .into_delta(pending_words, vec![], replaced_ids);

        (job_id, delta)
    }

    /// Resolve a pending correction job with corrected words.
    ///
    /// `corrected_words` should have `state: Final`. Their IDs can differ
    /// from the originals (the correction may change word boundaries).
    /// `replaced_ids` in the returned delta contains the original pending IDs.
    pub fn apply_correction(
        &mut self,
        job_id: u64,
        corrected_words: Vec<FinalizedWord>,
    ) -> TranscriptDelta {
        let replaced_ids = self.resolve_job(job_id);

        self.partial_snapshot()
            .into_delta(corrected_words, vec![], replaced_ids)
    }

    /// Drain all remaining state at session end.
    pub fn flush(&mut self) -> TranscriptDelta {
        let mut new_words = vec![];
        let mut hints = vec![];

        for state in self.channels.values_mut() {
            let (words, word_hints) = state.drain();
            new_words.extend(words);
            hints.extend(word_hints);
        }

        self.channels.clear();
        self.pending_corrections.clear();

        TranscriptDelta {
            new_words,
            hints,
            replaced_ids: vec![],
            partials: vec![],
            partial_hints: vec![],
        }
    }

    /// Convert a complete batch response into a `TranscriptDelta`.
    ///
    /// Stateless — batch responses are already final and don't need the
    /// streaming state (watermark, held word, etc.) used by `process()`.
    pub fn process_batch_response(response: &BatchResponse) -> TranscriptDelta {
        let mut new_words = Vec::new();
        let mut hints = Vec::new();

        for (channel_idx, channel) in response.results.channels.iter().enumerate() {
            let Some(alt) = channel.alternatives.first() else {
                continue;
            };
            if alt.words.is_empty() {
                continue;
            }

            let ch = channel_idx as i32;
            let raw = assemble_batch(&alt.words, &alt.transcript, ch);
            let (channel_words, channel_hints) = finalize_words(raw, WordState::Final);
            new_words.extend(channel_words);
            hints.extend(channel_hints);
        }

        TranscriptDelta {
            new_words,
            hints,
            replaced_ids: vec![],
            partials: vec![],
            partial_hints: vec![],
        }
    }

    // ── Internal ────────────────────────────────────────────────────────────

    fn register_job(&mut self, job_id: u64, word_ids: Vec<String>) {
        self.pending_corrections.insert(job_id, word_ids);
    }

    fn resolve_job(&mut self, job_id: u64) -> Vec<String> {
        self.pending_corrections.remove(&job_id).unwrap_or_default()
    }

    fn next_job_id(&mut self) -> u64 {
        let id = self.next_job_id;
        self.next_job_id += 1;
        id
    }

    fn partial_snapshot(&self) -> PartialSnapshot {
        PartialSnapshot::from_channels(self.channels.values())
    }
}

impl Default for TranscriptProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> ParsedStreamResponse<'a> {
    fn from_response(response: &'a StreamResponse) -> Option<Self> {
        let StreamResponse::TranscriptResponse {
            is_final,
            channel,
            channel_index,
            metadata,
            ..
        } = response
        else {
            return None;
        };

        let alt = channel.alternatives.first()?;
        if alt.words.is_empty() && alt.transcript.is_empty() {
            return None;
        }

        Some(Self {
            is_final: *is_final,
            channel: channel_index.first().copied().unwrap_or(0),
            words: &alt.words,
            transcript: &alt.transcript,
            correction: CorrectionMetadata::from_extra(metadata.extra.as_ref()),
        })
    }
}

impl CorrectionMetadata {
    fn from_extra(extra: Option<&HashMap<String, serde_json::Value>>) -> Self {
        let get_bool = |key: &str| -> bool {
            extra
                .and_then(|value| value.get(key))
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        };
        let get_u64 = |key: &str| -> u64 {
            extra
                .and_then(|value| value.get(key))
                .and_then(|value| value.as_u64())
                .unwrap_or(0)
        };

        Self {
            is_cloud_corrected: get_bool("cloud_corrected"),
            is_cloud_handoff: get_bool("cloud_handoff"),
            cloud_job_id: get_u64("cloud_job_id"),
        }
    }

    fn is_corrected_job(&self) -> bool {
        self.is_cloud_corrected && self.cloud_job_id != 0
    }

    fn is_handoff_job(&self) -> bool {
        self.is_cloud_handoff && self.cloud_job_id != 0
    }
}

impl PartialSnapshot {
    fn from_channels<'a>(states: impl Iterator<Item = &'a ChannelState>) -> Self {
        let mut partials = Vec::new();
        let mut partial_hints = Vec::new();
        let mut offset = 0usize;

        for state in states {
            let channel_partials: Vec<_> = state.current_partials().collect();

            for mut hint in state.current_partial_hints() {
                if let WordRef::RuntimeIndex(index) = &mut hint.target {
                    *index += offset;
                }
                partial_hints.push(hint);
            }

            offset += channel_partials.len();
            partials.extend(channel_partials);
        }

        Self {
            partials,
            partial_hints,
        }
    }

    fn into_delta(
        self,
        new_words: Vec<FinalizedWord>,
        hints: Vec<RuntimeSpeakerHint>,
        replaced_ids: Vec<String>,
    ) -> TranscriptDelta {
        TranscriptDelta {
            new_words,
            hints,
            replaced_ids,
            partials: self.partials,
            partial_hints: self.partial_hints,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RawWord;

    #[test]
    fn partial_snapshot_preserves_partial_hint_indices_across_channels() {
        let mut processor = TranscriptProcessor::new();

        let ch0 = processor
            .channels
            .entry(0)
            .or_insert_with(ChannelState::new);
        ch0.apply_partial(vec![
            RawWord {
                text: " hello".to_string(),
                start_ms: 0,
                end_ms: 100,
                channel: 0,
                speaker: Some(4),
            },
            RawWord {
                text: " world".to_string(),
                start_ms: 100,
                end_ms: 200,
                channel: 0,
                speaker: None,
            },
        ]);

        let ch1 = processor
            .channels
            .entry(1)
            .or_insert_with(ChannelState::new);
        ch1.apply_partial(vec![RawWord {
            text: " remote".to_string(),
            start_ms: 0,
            end_ms: 100,
            channel: 1,
            speaker: Some(7),
        }]);

        let snapshot = processor.partial_snapshot();

        assert_eq!(snapshot.partials.len(), 3);
        assert_eq!(snapshot.partial_hints.len(), 2);
        assert!(matches!(
            snapshot.partial_hints[0].target,
            WordRef::RuntimeIndex(0)
        ));
        assert!(matches!(
            snapshot.partial_hints[1].target,
            WordRef::RuntimeIndex(2)
        ));
    }
}
