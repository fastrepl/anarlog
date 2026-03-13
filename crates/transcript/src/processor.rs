use std::collections::{BTreeMap, HashMap};

use owhisper_interface::{batch::Response as BatchResponse, stream::StreamResponse};

use super::accumulator::ChannelState;
use super::types::{FinalizedWord, PartialWord, TranscriptDelta, WordState};
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
        let (is_final, channel, channel_index, metadata) = match response {
            StreamResponse::TranscriptResponse {
                is_final,
                channel,
                channel_index,
                metadata,
                ..
            } => (*is_final, channel, channel_index, metadata),
            _ => return None,
        };

        let alt = channel.alternatives.first()?;
        if alt.words.is_empty() && alt.transcript.is_empty() {
            return None;
        }

        let ch = channel_index.first().copied().unwrap_or(0);
        let raw_words = assemble(&alt.words, &alt.transcript, ch);
        if raw_words.is_empty() {
            return None;
        }

        let extra = metadata.extra.as_ref();
        let get_bool = |key: &str| -> bool {
            extra
                .and_then(|e| e.get(key))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        };
        let get_u64 = |key: &str| -> u64 {
            extra
                .and_then(|e| e.get(key))
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
        };

        let is_cloud_corrected = get_bool("cloud_corrected");
        let is_cloud_handoff = get_bool("cloud_handoff");
        let cloud_job_id = get_u64("cloud_job_id");

        let channel_state = self.channels.entry(ch).or_insert_with(ChannelState::new);

        if is_final {
            let word_state = if is_cloud_handoff && cloud_job_id != 0 {
                WordState::Pending
            } else {
                WordState::Final
            };

            let (new_words, hints) = channel_state.apply_final(raw_words, word_state);

            let replaced_ids = if is_cloud_corrected && cloud_job_id != 0 {
                self.resolve_job(cloud_job_id)
            } else {
                vec![]
            };

            if is_cloud_handoff && cloud_job_id != 0 {
                let ids: Vec<String> = new_words.iter().map(|w| w.id.clone()).collect();
                self.register_job(cloud_job_id, ids);
            }

            let partials = self.all_partials();

            if new_words.is_empty() && replaced_ids.is_empty() {
                return None;
            }

            Some(TranscriptDelta {
                new_words,
                hints,
                replaced_ids,
                partials,
            })
        } else {
            channel_state.apply_partial(raw_words);

            Some(TranscriptDelta {
                new_words: vec![],
                hints: vec![],
                replaced_ids: vec![],
                partials: self.all_partials(),
            })
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

        let delta = TranscriptDelta {
            new_words: pending_words,
            hints: vec![],
            replaced_ids,
            partials: self.all_partials(),
        };

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

        TranscriptDelta {
            new_words: corrected_words,
            hints: vec![],
            replaced_ids,
            partials: self.all_partials(),
        }
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

    fn all_partials(&self) -> Vec<PartialWord> {
        self.channels
            .values()
            .flat_map(|s| s.current_partials())
            .collect()
    }
}

impl Default for TranscriptProcessor {
    fn default() -> Self {
        Self::new()
    }
}
