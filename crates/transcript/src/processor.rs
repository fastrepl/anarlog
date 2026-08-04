use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use owhisper_interface::{
    batch::Response as BatchResponse,
    stream::{StreamResponse, Word},
};

use super::channel_state::ChannelState;
#[cfg(test)]
use super::channel_state::{MAX_PARTIAL_TEXT_BYTES, MAX_PARTIAL_WORDS};
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
/// 4. On bounded-state pressure or session end: pending words become final
///    with their original text
///
/// The processor supports two integration patterns:
///
/// - **Inline** cloud handoff: the streaming protocol itself carries
///   handoff/correction metadata. Handled automatically inside `process()`.
///
/// - **External** (LLM postprocessor, future sources): the caller finalizes
///   words via `process()`, then calls `submit_correction` / `apply_correction`
///   to manage the pending→final lifecycle.
pub struct TranscriptProcessor {
    channels: BTreeMap<i32, ChannelState>,
    pending_corrections: HashMap<u64, PendingCorrection>,
    pending_correction_order: VecDeque<u64>,
    pending_correction_words: usize,
    pending_correction_bytes: usize,
    next_job_id: u64,
    finalize_partials: bool,
    flush_partials: bool,
}

const MAX_PENDING_CORRECTION_JOBS: usize = 64;
const MAX_PENDING_CORRECTION_WORDS: usize = 4_096;
const MAX_PENDING_CORRECTION_BYTES: usize = 256 * 1024;
const MAX_CHANNEL_STATES: usize = 16;

struct PendingCorrection {
    original_words: Vec<FinalizedWord>,
    retained_bytes: usize,
}

#[derive(Default)]
struct CorrectionPromotion {
    words: Vec<FinalizedWord>,
    replaced_ids: Vec<String>,
}

impl CorrectionPromotion {
    fn extend_job(&mut self, job: PendingCorrection) {
        self.replaced_ids
            .extend(job.original_words.iter().map(|word| word.id.clone()));
        self.words.extend(job.original_words);
    }

    fn apply_to(self, new_words: &mut Vec<FinalizedWord>, replaced_ids: &mut Vec<String>) {
        if self.words.is_empty() {
            return;
        }

        let promoted_ids = self
            .words
            .iter()
            .map(|word| word.id.clone())
            .collect::<HashSet<_>>();
        new_words.retain(|word| !promoted_ids.contains(&word.id));
        new_words.extend(self.words);
        replaced_ids.extend(self.replaced_ids);

        let mut seen = HashSet::new();
        replaced_ids.retain(|id| seen.insert(id.clone()));
    }
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
}

impl TranscriptProcessor {
    pub fn new() -> Self {
        Self {
            channels: BTreeMap::new(),
            pending_corrections: HashMap::new(),
            pending_correction_order: VecDeque::new(),
            pending_correction_words: 0,
            pending_correction_bytes: 0,
            next_job_id: 1,
            finalize_partials: true,
            flush_partials: true,
        }
    }

    /// Disable this for providers whose partials are UI snapshots rather than
    /// commit-worthy transcript words.
    pub fn with_partial_finalization(mut self, finalize_partials: bool) -> Self {
        self.finalize_partials = finalize_partials;
        self.flush_partials = finalize_partials;
        self
    }

    /// Control whether remaining partials are committed when the session ends.
    pub fn with_flush_partial_finalization(mut self, flush_partials: bool) -> Self {
        self.flush_partials = flush_partials;
        self
    }

    pub fn process(&mut self, response: &StreamResponse) -> Option<TranscriptDelta> {
        let parsed = ParsedStreamResponse::from_response(response)?;
        let raw_words = assemble(parsed.words, parsed.transcript, parsed.channel);
        if raw_words.is_empty() {
            return None;
        }

        if parsed.correction.is_corrected_job()
            && !self
                .pending_corrections
                .contains_key(&parsed.correction.cloud_job_id)
        {
            return None;
        }

        if !self.channels.contains_key(&parsed.channel) && self.channels.len() >= MAX_CHANNEL_STATES
        {
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

            let mut new_words =
                channel_state.apply_final(raw_words, word_state, self.finalize_partials);

            let mut replaced_ids = vec![];

            if parsed.correction.is_corrected_job() {
                if new_words.is_empty() {
                    let mut promotion = CorrectionPromotion::default();
                    if let Some(job) = self.remove_job(parsed.correction.cloud_job_id) {
                        promotion.extend_job(job);
                    }
                    promotion.apply_to(&mut new_words, &mut replaced_ids);
                } else {
                    replaced_ids = self
                        .resolve_job(parsed.correction.cloud_job_id)
                        .unwrap_or_default();
                }
            }

            if parsed.correction.is_handoff_job() {
                let promotion = self.register_job(parsed.correction.cloud_job_id, &new_words);
                promotion.apply_to(&mut new_words, &mut replaced_ids);
            }

            let snapshot = self.partial_snapshot();

            if new_words.is_empty() && replaced_ids.is_empty() {
                return None;
            }

            Some(snapshot.into_delta(new_words, replaced_ids))
        } else {
            let new_words = channel_state
                .apply_partial(raw_words, self.finalize_partials || self.flush_partials);
            Some(self.partial_snapshot().into_delta(new_words, vec![]))
        }
    }

    // ── Generic correction API ──────────────────────────────────────────────

    pub fn submit_correction(&mut self, words: Vec<FinalizedWord>) -> (u64, TranscriptDelta) {
        let job_id = self.next_job_id();
        let mut replaced_ids: Vec<String> = words.iter().map(|w| w.id.clone()).collect();

        let mut pending_words: Vec<FinalizedWord> = words
            .into_iter()
            .map(|w| FinalizedWord {
                state: WordState::Pending,
                ..w
            })
            .collect();

        let promotion = self.register_job(job_id, &pending_words);
        promotion.apply_to(&mut pending_words, &mut replaced_ids);

        let delta = self
            .partial_snapshot()
            .into_delta(pending_words, replaced_ids);

        (job_id, delta)
    }

    pub fn apply_correction(
        &mut self,
        job_id: u64,
        corrected_words: Vec<FinalizedWord>,
    ) -> TranscriptDelta {
        let Some(replaced_ids) = self.resolve_job(job_id) else {
            return self.partial_snapshot().into_delta(vec![], vec![]);
        };

        self.partial_snapshot()
            .into_delta(corrected_words, replaced_ids)
    }

    /// Drain all remaining state at session end.
    pub fn flush(&mut self) -> TranscriptDelta {
        let mut new_words = vec![];

        for state in self.channels.values_mut() {
            if self.flush_partials {
                new_words.extend(state.drain());
            } else {
                new_words.extend(state.drain_final_words());
            }
        }

        self.channels.clear();
        let promotion = self.promote_all_corrections();
        let mut replaced_ids = vec![];
        promotion.apply_to(&mut new_words, &mut replaced_ids);

        TranscriptDelta {
            new_words,
            replaced_ids,
            partials: vec![],
        }
    }

    /// Convert a complete batch response into a `TranscriptDelta`.
    pub fn process_batch_response(response: &BatchResponse) -> TranscriptDelta {
        let mut new_words = Vec::new();

        for channel in &response.results.channels {
            let Some(alt) = channel.alternatives.first() else {
                continue;
            };
            if alt.words.is_empty() {
                continue;
            }

            let raw = assemble_batch(&alt.words, &alt.transcript);
            new_words.extend(finalize_words(raw, WordState::Final));
        }

        TranscriptDelta {
            new_words,
            replaced_ids: vec![],
            partials: vec![],
        }
    }

    // ── Internal ────────────────────────────────────────────────────────────

    fn register_job(&mut self, job_id: u64, words: &[FinalizedWord]) -> CorrectionPromotion {
        let mut promotion = CorrectionPromotion::default();

        if let Some(displaced) = self.remove_job(job_id) {
            promotion.extend_job(displaced);
        }

        let original_words = words
            .iter()
            .cloned()
            .map(|word| FinalizedWord {
                state: WordState::Final,
                ..word
            })
            .collect::<Vec<_>>();
        let retained_bytes = original_words
            .iter()
            .map(|word| word.id.len().saturating_add(word.text.len()))
            .fold(0usize, usize::saturating_add);

        self.pending_correction_words = self
            .pending_correction_words
            .saturating_add(original_words.len());
        self.pending_correction_bytes =
            self.pending_correction_bytes.saturating_add(retained_bytes);
        self.pending_correction_order.push_back(job_id);
        self.pending_corrections.insert(
            job_id,
            PendingCorrection {
                original_words,
                retained_bytes,
            },
        );

        while self.pending_corrections.len() > MAX_PENDING_CORRECTION_JOBS
            || self.pending_correction_words > MAX_PENDING_CORRECTION_WORDS
            || self.pending_correction_bytes > MAX_PENDING_CORRECTION_BYTES
        {
            let Some(oldest_job_id) = self.pending_correction_order.front().copied() else {
                break;
            };
            if let Some(expired) = self.remove_job(oldest_job_id) {
                promotion.extend_job(expired);
            }
        }

        promotion
    }

    fn resolve_job(&mut self, job_id: u64) -> Option<Vec<String>> {
        self.remove_job(job_id)
            .map(|job| job.original_words.into_iter().map(|word| word.id).collect())
    }

    fn remove_job(&mut self, job_id: u64) -> Option<PendingCorrection> {
        let job = self.pending_corrections.remove(&job_id)?;
        self.pending_correction_order.retain(|id| *id != job_id);
        self.pending_correction_words = self
            .pending_correction_words
            .saturating_sub(job.original_words.len());
        self.pending_correction_bytes = self
            .pending_correction_bytes
            .saturating_sub(job.retained_bytes);
        Some(job)
    }

    fn promote_all_corrections(&mut self) -> CorrectionPromotion {
        let mut promotion = CorrectionPromotion::default();
        while let Some(job_id) = self.pending_correction_order.front().copied() {
            if let Some(job) = self.remove_job(job_id) {
                promotion.extend_job(job);
            }
        }
        promotion
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

        for state in states {
            partials.extend(state.current_partials());
        }

        Self { partials }
    }

    fn into_delta(
        self,
        new_words: Vec<FinalizedWord>,
        replaced_ids: Vec<String>,
    ) -> TranscriptDelta {
        TranscriptDelta {
            new_words,
            replaced_ids,
            partials: self.partials,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RawWord;
    use owhisper_interface::stream::{Alternatives, Channel, Metadata, ModelInfo, Word};

    fn stream_response(
        index: usize,
        is_final: bool,
        metadata_extra: Option<HashMap<String, serde_json::Value>>,
    ) -> StreamResponse {
        let token = format!("word-{index}");
        let transcript = format!(" {token}");
        let start = index as f64 / 10.0;

        StreamResponse::TranscriptResponse {
            start,
            duration: 0.05,
            is_final,
            speech_final: is_final,
            from_finalize: false,
            channel: Channel {
                alternatives: vec![Alternatives {
                    transcript,
                    words: vec![Word {
                        word: token.clone(),
                        start,
                        end: start + 0.05,
                        confidence: 1.0,
                        speaker: None,
                        punctuated_word: Some(token),
                        language: None,
                    }],
                    confidence: 1.0,
                    languages: vec![],
                }],
            },
            metadata: Metadata {
                request_id: "request".to_string(),
                model_info: ModelInfo {
                    name: "model".to_string(),
                    version: "1".to_string(),
                    arch: "test".to_string(),
                },
                model_uuid: "uuid".to_string(),
                extra: metadata_extra,
            },
            channel_index: vec![0, 1],
        }
    }

    fn multi_word_stream_response(
        words: &[&str],
        is_final: bool,
        metadata_extra: Option<HashMap<String, serde_json::Value>>,
    ) -> StreamResponse {
        let transcript = format!(" {}", words.join(" "));
        let words = words
            .iter()
            .enumerate()
            .map(|(index, word)| {
                let start = index as f64 / 10.0;
                Word {
                    word: (*word).to_string(),
                    start,
                    end: start + 0.05,
                    confidence: 1.0,
                    speaker: None,
                    punctuated_word: Some((*word).to_string()),
                    language: None,
                }
            })
            .collect::<Vec<_>>();

        StreamResponse::TranscriptResponse {
            start: 0.0,
            duration: words.len() as f64 / 10.0,
            is_final,
            speech_final: is_final,
            from_finalize: false,
            channel: Channel {
                alternatives: vec![Alternatives {
                    transcript,
                    words,
                    confidence: 1.0,
                    languages: vec![],
                }],
            },
            metadata: Metadata {
                request_id: "request".to_string(),
                model_info: ModelInfo {
                    name: "model".to_string(),
                    version: "1".to_string(),
                    arch: "test".to_string(),
                },
                model_uuid: "uuid".to_string(),
                extra: metadata_extra,
            },
            channel_index: vec![0, 1],
        }
    }

    fn finalized_word(id: impl Into<String>, text: impl Into<String>) -> FinalizedWord {
        FinalizedWord {
            id: id.into(),
            text: text.into(),
            start_ms: 0,
            end_ms: 100,
            channel: 0,
            state: WordState::Final,
            speaker_index: None,
        }
    }

    #[test]
    fn partial_snapshot_carries_speaker_index_on_words() {
        let mut processor = TranscriptProcessor::new();

        let ch0 = processor
            .channels
            .entry(0)
            .or_insert_with(ChannelState::new);
        ch0.apply_partial(
            vec![
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
            ],
            true,
        );

        let ch1 = processor
            .channels
            .entry(1)
            .or_insert_with(ChannelState::new);
        ch1.apply_partial(
            vec![RawWord {
                text: " remote".to_string(),
                start_ms: 0,
                end_ms: 100,
                channel: 1,
                speaker: Some(7),
            }],
            true,
        );

        let snapshot = processor.partial_snapshot();

        assert_eq!(snapshot.partials.len(), 3);
        assert_eq!(snapshot.partials[0].speaker_index, Some(4));
        assert_eq!(snapshot.partials[1].speaker_index, None);
        assert_eq!(snapshot.partials[2].speaker_index, Some(7));
    }

    #[test]
    fn flush_can_discard_partials_without_losing_held_finals() {
        let mut processor = TranscriptProcessor::new().with_partial_finalization(false);
        let channel = processor
            .channels
            .entry(0)
            .or_insert_with(ChannelState::new);

        channel.apply_partial(
            vec![RawWord {
                text: " repeated".to_string(),
                start_ms: 0,
                end_ms: 100,
                channel: 0,
                speaker: None,
            }],
            false,
        );
        channel.apply_final(
            vec![RawWord {
                text: " final".to_string(),
                start_ms: 1_000,
                end_ms: 1_100,
                channel: 0,
                speaker: None,
            }],
            WordState::Final,
            false,
        );

        let delta = processor.flush();
        let text = delta
            .new_words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<String>();

        assert_eq!(text, " final");
        assert!(delta.partials.is_empty());
    }

    #[test]
    fn flush_can_commit_partials_when_live_finalization_is_disabled() {
        let mut processor = TranscriptProcessor::new()
            .with_partial_finalization(false)
            .with_flush_partial_finalization(true);
        let channel = processor
            .channels
            .entry(0)
            .or_insert_with(ChannelState::new);

        channel.apply_partial(
            vec![RawWord {
                text: " tail".to_string(),
                start_ms: 0,
                end_ms: 100,
                channel: 0,
                speaker: None,
            }],
            false,
        );

        let delta = processor.flush();
        let text = delta
            .new_words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<String>();

        assert_eq!(text, " tail");
        assert!(delta.partials.is_empty());
    }

    #[test]
    fn no_final_stream_promotes_oldest_partials_without_losing_words() {
        let mut processor = TranscriptProcessor::new();
        let total_words = MAX_PARTIAL_WORDS + 200;
        let mut finalized = Vec::new();

        for index in 0..total_words {
            let response = stream_response(index, false, None);
            let delta = processor.process(&response).expect("partial delta");
            finalized.extend(delta.new_words);

            assert!(delta.partials.len() <= MAX_PARTIAL_WORDS);
            assert!(
                delta
                    .partials
                    .iter()
                    .map(|word| word.text.len())
                    .sum::<usize>()
                    <= MAX_PARTIAL_TEXT_BYTES
            );
        }

        assert!(!finalized.is_empty());
        finalized.extend(processor.flush().new_words);
        finalized.sort_by_key(|word| word.start_ms);

        assert_eq!(finalized.len(), total_words);
        for (index, word) in finalized.iter().enumerate() {
            assert_eq!(word.text, format!(" word-{index}"));
            assert_eq!(word.start_ms, index as i64 * 100);
            assert_eq!(word.state, WordState::Final);
        }
    }

    #[test]
    fn flush_aware_partial_overflow_preserves_all_words() {
        let mut processor = TranscriptProcessor::new()
            .with_partial_finalization(false)
            .with_flush_partial_finalization(true);
        let total_words = MAX_PARTIAL_WORDS + 200;
        let mut finalized = Vec::new();

        for index in 0..total_words {
            let response = stream_response(index, false, None);
            let delta = processor.process(&response).expect("partial delta");
            finalized.extend(delta.new_words);
            assert!(delta.partials.len() <= MAX_PARTIAL_WORDS);
        }

        finalized.extend(processor.flush().new_words);
        finalized.sort_by_key(|word| word.start_ms);

        assert_eq!(finalized.len(), total_words);
        for (index, word) in finalized.iter().enumerate() {
            assert_eq!(word.text, format!(" word-{index}"));
            assert_eq!(word.start_ms, index as i64 * 100);
            assert_eq!(word.state, WordState::Final);
        }
    }

    #[test]
    fn preview_only_partial_overflow_is_discarded_not_persisted() {
        let mut processor = TranscriptProcessor::new()
            .with_partial_finalization(false)
            .with_flush_partial_finalization(false);
        let total_words = MAX_PARTIAL_WORDS + 200;
        let mut last_delta = None;

        for index in 0..total_words {
            let response = stream_response(index, false, None);
            let delta = processor.process(&response).expect("partial delta");
            assert!(delta.new_words.is_empty());
            assert!(delta.partials.len() <= MAX_PARTIAL_WORDS);
            assert!(
                delta
                    .partials
                    .iter()
                    .map(|word| word.text.len())
                    .sum::<usize>()
                    <= MAX_PARTIAL_TEXT_BYTES
            );
            last_delta = Some(delta);
        }

        let partials = last_delta.unwrap().partials;
        assert_eq!(partials.len(), MAX_PARTIAL_WORDS);
        assert_eq!(
            partials.first().unwrap().text,
            format!(" word-{}", total_words - MAX_PARTIAL_WORDS)
        );
        assert!(processor.flush().new_words.is_empty());
    }

    #[test]
    fn unresolved_corrections_expire_oldest_and_flush_as_final() {
        let mut processor = TranscriptProcessor::new();
        let job_count = MAX_PENDING_CORRECTION_JOBS + 10;
        let mut finalized_ids = HashSet::new();
        let mut latest_job_id = 0;

        for index in 0..job_count {
            let id = format!("word-{index}");
            let (job_id, delta) =
                processor.submit_correction(vec![finalized_word(&id, format!(" text-{index}"))]);
            latest_job_id = job_id;
            finalized_ids.extend(
                delta
                    .new_words
                    .into_iter()
                    .filter(|word| word.state == WordState::Final)
                    .map(|word| word.id),
            );

            assert!(processor.pending_corrections.len() <= MAX_PENDING_CORRECTION_JOBS);
            assert!(processor.pending_correction_words <= MAX_PENDING_CORRECTION_WORDS);
            assert!(processor.pending_correction_bytes <= MAX_PENDING_CORRECTION_BYTES);
        }

        assert_eq!(finalized_ids.len(), 10);

        let late =
            processor.apply_correction(1, vec![finalized_word("word-0", " corrected too late")]);
        assert!(late.is_empty());

        let latest_id = format!("word-{}", job_count - 1);
        let corrected = processor.apply_correction(
            latest_job_id,
            vec![finalized_word(&latest_id, " corrected")],
        );
        assert_eq!(corrected.replaced_ids, vec![latest_id.clone()]);
        assert_eq!(corrected.new_words.len(), 1);
        assert_eq!(corrected.new_words[0].text, " corrected");
        finalized_ids.insert(latest_id);

        let flush = processor.flush();
        assert!(
            flush
                .new_words
                .iter()
                .all(|word| word.state == WordState::Final)
        );
        finalized_ids.extend(flush.new_words.into_iter().map(|word| word.id));

        assert_eq!(finalized_ids.len(), job_count);
        assert!(processor.pending_corrections.is_empty());
        assert!(processor.pending_correction_order.is_empty());
        assert_eq!(processor.pending_correction_words, 0);
        assert_eq!(processor.pending_correction_bytes, 0);
    }

    #[test]
    fn oversized_and_duplicate_correction_jobs_fall_back_to_originals() {
        let mut processor = TranscriptProcessor::new();
        let oversized_id = "x".repeat(MAX_PENDING_CORRECTION_BYTES + 1);
        let (_, oversized) =
            processor.submit_correction(vec![finalized_word(&oversized_id, " text")]);

        assert!(processor.pending_corrections.is_empty());
        assert_eq!(oversized.new_words.len(), 1);
        assert_eq!(oversized.new_words[0].id, oversized_id);
        assert_eq!(oversized.new_words[0].state, WordState::Final);
        assert_eq!(oversized.replaced_ids.len(), 1);

        let original = finalized_word("old", " original");
        let replacement = finalized_word("new", " replacement");
        assert!(processor.register_job(42, &[original]).words.is_empty());
        let displaced = processor.register_job(42, &[replacement]);

        assert_eq!(displaced.replaced_ids, vec!["old"]);
        assert_eq!(displaced.words.len(), 1);
        assert_eq!(displaced.words[0].id, "old");
        assert_eq!(displaced.words[0].state, WordState::Final);
        assert_eq!(processor.pending_corrections.len(), 1);
    }

    #[test]
    fn unknown_cloud_correction_payload_is_ignored() {
        let mut processor = TranscriptProcessor::new();
        let metadata = HashMap::from([
            ("cloud_corrected".to_string(), serde_json::json!(true)),
            ("cloud_job_id".to_string(), serde_json::json!(999)),
        ]);

        assert!(
            processor
                .process(&stream_response(0, true, Some(metadata)))
                .is_none()
        );
        assert!(processor.channels.is_empty());
    }

    #[test]
    fn ignores_new_channels_after_state_capacity_is_reached() {
        let mut processor = TranscriptProcessor::new();

        for channel in 0..MAX_CHANNEL_STATES {
            let mut response = stream_response(channel, false, None);
            let StreamResponse::TranscriptResponse { channel_index, .. } = &mut response else {
                unreachable!();
            };
            *channel_index = vec![channel as i32];
            assert!(processor.process(&response).is_some());
        }

        let mut overflow = stream_response(MAX_CHANNEL_STATES, false, None);
        let StreamResponse::TranscriptResponse { channel_index, .. } = &mut overflow else {
            unreachable!();
        };
        *channel_index = vec![MAX_CHANNEL_STATES as i32];

        assert!(processor.process(&overflow).is_none());
        assert_eq!(processor.channels.len(), MAX_CHANNEL_STATES);

        let existing = stream_response(MAX_CHANNEL_STATES + 1, false, None);
        assert!(processor.process(&existing).is_some());
        assert_eq!(processor.channels.len(), MAX_CHANNEL_STATES);
    }

    #[test]
    fn stale_cloud_correction_promotes_originals_instead_of_deleting_them() {
        let mut processor = TranscriptProcessor::new();
        let handoff_metadata = HashMap::from([
            ("cloud_handoff".to_string(), serde_json::json!(true)),
            ("cloud_job_id".to_string(), serde_json::json!(42)),
        ]);
        let handoff = processor
            .process(&multi_word_stream_response(
                &["local-one", "local-two", "local-tail"],
                true,
                Some(handoff_metadata),
            ))
            .expect("handoff delta");
        let pending_ids = handoff
            .new_words
            .iter()
            .map(|word| word.id.clone())
            .collect::<Vec<_>>();

        assert_eq!(handoff.new_words.len(), 2);
        assert!(
            handoff
                .new_words
                .iter()
                .all(|word| word.state == WordState::Pending)
        );

        let correction_metadata = HashMap::from([
            ("cloud_corrected".to_string(), serde_json::json!(true)),
            ("cloud_job_id".to_string(), serde_json::json!(42)),
        ]);
        let correction = processor
            .process(&multi_word_stream_response(
                &["cloud-one", "cloud-two", "cloud-tail"],
                true,
                Some(correction_metadata),
            ))
            .expect("correction fallback delta");

        assert_eq!(correction.replaced_ids, pending_ids);
        assert_eq!(correction.new_words.len(), 2);
        assert_eq!(correction.new_words[0].text, " local-one");
        assert_eq!(correction.new_words[1].text, " local-two");
        assert!(
            correction
                .new_words
                .iter()
                .all(|word| word.state == WordState::Final)
        );
        assert!(processor.pending_corrections.is_empty());
    }
}
