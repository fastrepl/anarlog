use std::collections::BTreeMap;

use hypr_transcript::{
    FinalizedWord, IdentityAssignment, PartialWord, SegmentBuilderOptions, SegmentKey, SegmentWord,
    TranscriptDelta, TranscriptProcessor, build_segments, channel_assignments_for_participants,
    normalize_rendered_segment_words, segment_options_for_participants, stable_segment_id,
};
use owhisper_interface::stream::{Alternatives, StreamResponse, Word};

const CACTUS_OVERLAP_MAX_GAP_MS: i64 = 500;
const SONIQO_CUMULATIVE_PREFIX_MIN_TOKENS: usize = 4;
const SONIQO_HISTORY_TOKEN_LIMIT: usize = 160;
const SONIQO_REPEAT_MIN_TOKENS: usize = 4;
const SONIQO_INTERNAL_REPEAT_MIN_TOKENS: usize = 6;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LiveTranscriptDelta {
    pub new_words: Vec<FinalizedWord>,
    pub replaced_ids: Vec<String>,
    pub partials: Vec<PartialWord>,
}

impl LiveTranscriptDelta {
    pub fn is_empty(&self) -> bool {
        self.new_words.is_empty() && self.replaced_ids.is_empty() && self.partials.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LiveTranscriptSegment {
    pub id: String,
    pub key: SegmentKey,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub words: Vec<SegmentWord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LiveTranscriptSegmentDelta {
    pub upserts: Vec<LiveTranscriptSegment>,
    pub removed_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LiveTranscriptUpdate {
    pub transcript_delta: LiveTranscriptDelta,
    pub segment_delta: Option<LiveTranscriptSegmentDelta>,
}

impl From<TranscriptDelta> for LiveTranscriptDelta {
    fn from(delta: TranscriptDelta) -> Self {
        Self {
            new_words: delta.new_words,
            replaced_ids: delta.replaced_ids,
            partials: delta.partials,
        }
    }
}

#[derive(Default)]
pub struct LiveTranscriptEngine {
    processor: TranscriptProcessor,
    normalizer: TranscriptNormalizer,
    rendered_segments: RenderedSegmentState,
}

impl LiveTranscriptEngine {
    pub fn new(
        provider_name: &str,
        participant_human_ids: &[String],
        self_human_id: Option<&str>,
    ) -> Self {
        let channel_assignments =
            channel_assignments_for_participants(participant_human_ids, self_human_id);
        let segment_options =
            segment_options_for_participants(participant_human_ids, self_human_id);

        let normalizer = TranscriptNormalizer::for_provider(provider_name);

        Self {
            processor: TranscriptProcessor::new()
                .with_partial_finalization(normalizer.finalize_partials()),
            normalizer,
            rendered_segments: RenderedSegmentState {
                channel_assignments,
                segment_options: Some(segment_options),
                ..Default::default()
            },
        }
    }

    pub fn process(&mut self, response: &StreamResponse) -> Option<LiveTranscriptUpdate> {
        let mut normalized = response.clone();
        self.normalizer.normalize(&mut normalized);
        let transcript_delta: LiveTranscriptDelta = self.processor.process(&normalized)?.into();
        let segment_delta = self.rendered_segments.apply_delta(&transcript_delta);
        Some(LiveTranscriptUpdate {
            transcript_delta,
            segment_delta,
        })
    }

    pub fn flush(&mut self) -> Option<LiveTranscriptUpdate> {
        let transcript_delta: LiveTranscriptDelta = self.processor.flush().into();
        let segment_delta = self.rendered_segments.apply_delta(&transcript_delta);
        if transcript_delta.is_empty() && segment_delta.is_none() {
            return None;
        }

        Some(LiveTranscriptUpdate {
            transcript_delta,
            segment_delta,
        })
    }
}

#[derive(Default)]
struct RenderedSegmentState {
    words: BTreeMap<String, FinalizedWord>,
    partials: Vec<PartialWord>,
    segments: BTreeMap<String, LiveTranscriptSegment>,
    channel_assignments: Vec<IdentityAssignment>,
    segment_options: Option<SegmentBuilderOptions>,
}

impl RenderedSegmentState {
    fn apply_delta(&mut self, delta: &LiveTranscriptDelta) -> Option<LiveTranscriptSegmentDelta> {
        let replaced_ids = delta
            .replaced_ids
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        let new_word_ids = delta
            .new_words
            .iter()
            .map(|word| word.id.clone())
            .collect::<std::collections::BTreeSet<_>>();

        self.words.retain(|id, _| !replaced_ids.contains(id));
        self.words.retain(|id, _| !new_word_ids.contains(id));

        for word in &delta.new_words {
            self.words.insert(word.id.clone(), word.clone());
        }

        self.partials = delta.partials.clone();

        let next_segments = build_live_segments(
            self.words.values().cloned().collect(),
            self.partials.clone(),
            &self.channel_assignments,
            self.segment_options.as_ref(),
        );
        let next_map = next_segments
            .into_iter()
            .map(|segment| (segment.id.clone(), segment))
            .collect::<BTreeMap<_, _>>();

        let removed_ids = self
            .segments
            .keys()
            .filter(|id| !next_map.contains_key(*id))
            .cloned()
            .collect::<Vec<_>>();
        let upserts = next_map
            .iter()
            .filter_map(|(id, segment)| match self.segments.get(id) {
                Some(existing) if existing == segment => None,
                _ => Some(segment.clone()),
            })
            .collect::<Vec<_>>();

        self.segments = next_map;

        if upserts.is_empty() && removed_ids.is_empty() {
            None
        } else {
            Some(LiveTranscriptSegmentDelta {
                upserts,
                removed_ids,
            })
        }
    }
}

#[derive(Default)]
enum TranscriptNormalizer {
    Cactus(CactusTranscriptNormalizer),
    Soniqo(SoniqoTranscriptNormalizer),
    #[default]
    Passthrough,
}

impl TranscriptNormalizer {
    fn for_provider(provider_name: &str) -> Self {
        match provider_name {
            "cactus" => Self::Cactus(CactusTranscriptNormalizer::default()),
            "soniqo" => Self::Soniqo(SoniqoTranscriptNormalizer::default()),
            _ => Self::Passthrough,
        }
    }

    fn normalize(&mut self, response: &mut StreamResponse) {
        match self {
            Self::Cactus(normalizer) => normalizer.normalize(response),
            Self::Soniqo(normalizer) => normalizer.normalize(response),
            Self::Passthrough => {}
        }
    }

    fn finalize_partials(&self) -> bool {
        !matches!(self, Self::Soniqo(_))
    }
}

#[derive(Default)]
struct CactusTranscriptNormalizer {
    channels: BTreeMap<i32, CactusChannelState>,
}

#[derive(Default)]
struct CactusChannelState {
    last_final_tokens: Vec<String>,
    last_final_end_ms: i64,
}

#[derive(Default)]
struct SoniqoTranscriptNormalizer {
    channels: BTreeMap<i32, SoniqoChannelState>,
}

#[derive(Default)]
struct SoniqoChannelState {
    active_start_ms: Option<i64>,
    active_tokens: Vec<String>,
    committed_tokens: Vec<String>,
}

impl CactusTranscriptNormalizer {
    fn normalize(&mut self, response: &mut StreamResponse) {
        let StreamResponse::TranscriptResponse {
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
        let overlap = find_cactus_overlap_prefix(
            &alternative.words,
            &state.last_final_tokens,
            state.last_final_end_ms,
        );

        if overlap > 0 {
            alternative.words.drain(..overlap);
        }

        if *is_final && !alternative.words.is_empty() {
            state.last_final_tokens = normalize_tokens_for_overlap(&alternative.words);
            state.last_final_end_ms =
                word_end_ms(alternative.words.last().expect("checked non-empty"));
        }
    }
}

impl SoniqoTranscriptNormalizer {
    fn normalize(&mut self, response: &mut StreamResponse) {
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
        let mut current_start_ms = (*start * 1000.0).round() as i64;
        let mut current_end_ms = ((*start + *duration) * 1000.0).round() as i64;

        collapse_soniqo_internal_repeats(alternative, &mut current_tokens);
        if alternative.words.is_empty() {
            return;
        }

        let committed_overlap =
            find_soniqo_committed_prefix(&current_tokens, &state.committed_tokens);
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
        } else {
            let overlap = find_soniqo_overlap_prefix(&current_tokens, &state.active_tokens);
            if overlap > 0 {
                drain_soniqo_prefix(alternative, &mut current_tokens, overlap);

                if alternative.words.is_empty() {
                    return;
                }

                sync_soniqo_timing(start, duration, &alternative.words);
                current_start_ms =
                    word_start_ms(alternative.words.first().expect("checked non-empty"));
            }
            state.active_start_ms = Some(current_start_ms);
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

fn find_cactus_overlap_prefix(
    words: &[Word],
    last_final_tokens: &[String],
    last_final_end_ms: i64,
) -> usize {
    if words.is_empty()
        || last_final_tokens.is_empty()
        || word_start_ms(&words[0]) > last_final_end_ms + CACTUS_OVERLAP_MAX_GAP_MS
    {
        return 0;
    }

    let current_tokens = normalize_tokens_for_overlap(words);
    let max_overlap = last_final_tokens.len().min(current_tokens.len());

    for overlap in (1..=max_overlap).rev() {
        let suffix = &last_final_tokens[last_final_tokens.len() - overlap..];
        let prefix = &current_tokens[..overlap];

        if suffix == prefix {
            return overlap;
        }
    }

    0
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

fn find_soniqo_committed_prefix(current_tokens: &[String], committed_tokens: &[String]) -> usize {
    if current_tokens.len() < SONIQO_REPEAT_MIN_TOKENS
        || committed_tokens.len() < SONIQO_REPEAT_MIN_TOKENS
    {
        return 0;
    }

    let max_overlap = committed_tokens.len().min(current_tokens.len());

    for overlap in (SONIQO_REPEAT_MIN_TOKENS..=max_overlap).rev() {
        let current_prefix = &current_tokens[..overlap];
        if committed_tokens
            .windows(overlap)
            .any(|tokens| tokens == current_prefix)
        {
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
    if current_tokens.len() < min_tokens || history_tokens.len() < min_tokens {
        return 0;
    }

    let max_overlap = history_tokens.len().min(current_tokens.len());

    for overlap in (min_tokens..=max_overlap).rev() {
        let current_prefix = &current_tokens[..overlap];
        if history_tokens
            .windows(overlap)
            .any(|tokens| tokens == current_prefix)
        {
            return overlap;
        }
    }

    0
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

        let overlap = find_soniqo_history_prefix(
            &word_tokens[index..],
            &next_tokens,
            SONIQO_INTERNAL_REPEAT_MIN_TOKENS,
        );

        if overlap > 0 {
            index += overlap;
            continue;
        }

        next_tokens.push(word_tokens[index].clone());
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

fn drain_soniqo_prefix(
    alternative: &mut Alternatives,
    current_tokens: &mut Vec<String>,
    count: usize,
) {
    alternative.words.drain(..count);
    current_tokens.drain(..count);
    alternative.transcript = transcript_from_words(&alternative.words);
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

fn normalize_tokens_for_overlap(words: &[Word]) -> Vec<String> {
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

fn build_live_segments(
    final_words: Vec<FinalizedWord>,
    partials: Vec<PartialWord>,
    channel_assignments: &[IdentityAssignment],
    segment_options: Option<&SegmentBuilderOptions>,
) -> Vec<LiveTranscriptSegment> {
    build_segments(
        &final_words,
        &partials,
        channel_assignments,
        segment_options,
    )
    .into_iter()
    .filter_map(|segment| {
        let words = normalize_rendered_segment_words(segment.words);
        let first = words.first()?;
        let last = words.last()?;
        let text = words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<String>()
            .trim()
            .to_string();
        if text.is_empty() {
            return None;
        }

        Some(LiveTranscriptSegment {
            id: stable_segment_id(&segment.key, &words),
            key: segment.key,
            start_ms: first.start_ms,
            end_ms: last.end_ms,
            text,
            words,
        })
    })
    .collect()
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

#[cfg(test)]
mod tests {
    use owhisper_interface::stream::{Alternatives, Channel, Metadata, ModelInfo};

    use super::*;

    fn transcript_response(
        transcript: &str,
        words: Vec<Word>,
        is_final: bool,
        channel_idx: i32,
    ) -> StreamResponse {
        transcript_response_at(transcript, words, is_final, channel_idx, 0.0, 0.0)
    }

    fn transcript_response_at(
        transcript: &str,
        words: Vec<Word>,
        is_final: bool,
        channel_idx: i32,
        start: f64,
        duration: f64,
    ) -> StreamResponse {
        StreamResponse::TranscriptResponse {
            start,
            duration,
            is_final,
            speech_final: is_final,
            from_finalize: false,
            channel: Channel {
                alternatives: vec![Alternatives {
                    transcript: transcript.to_string(),
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
                    arch: "cactus".to_string(),
                },
                model_uuid: "uuid".to_string(),
                extra: None,
            },
            channel_index: vec![channel_idx, 2],
        }
    }

    fn word(text: &str, start: f64, end: f64) -> Word {
        Word {
            word: text.to_string(),
            start,
            end,
            confidence: 1.0,
            speaker: None,
            punctuated_word: Some(text.to_string()),
            language: None,
        }
    }

    fn words_from_text(text: &str, start: f64, duration: f64) -> Vec<Word> {
        let parts = text.split_whitespace().collect::<Vec<_>>();
        let count = parts.len();

        parts
            .into_iter()
            .enumerate()
            .map(|(index, part)| {
                let word_start = start + (index as f64 / count as f64) * duration;
                let word_end = start + ((index + 1) as f64 / count as f64) * duration;
                word(part, word_start, word_end)
            })
            .collect()
    }

    #[test]
    fn cactus_normalizer_trims_partial_overlap_from_last_confirmed_chunk() {
        let mut normalizer = CactusTranscriptNormalizer::default();

        let mut final_response = transcript_response("Mark", vec![word("Mark", 0.0, 1.0)], true, 0);
        normalizer.normalize(&mut final_response);

        let mut partial_response = transcript_response(
            "Mark Zuckerberg speaks",
            vec![
                word("Mark", 0.8, 1.2),
                word("Zuckerberg", 1.2, 2.0),
                word("speaks", 2.0, 2.8),
            ],
            false,
            0,
        );
        normalizer.normalize(&mut partial_response);

        let StreamResponse::TranscriptResponse { channel, .. } = partial_response else {
            panic!("expected transcript response");
        };
        let words = &channel.alternatives[0].words;
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].word, "Zuckerberg");
        assert_eq!(words[1].word, "speaks");
    }

    #[test]
    fn cactus_normalizer_does_not_trim_later_repeated_word() {
        let mut normalizer = CactusTranscriptNormalizer::default();

        let mut final_response = transcript_response("Mark", vec![word("Mark", 0.0, 1.0)], true, 0);
        normalizer.normalize(&mut final_response);

        let mut partial_response = transcript_response(
            "Mark later",
            vec![word("Mark", 2.0, 2.4), word("later", 2.4, 3.0)],
            false,
            0,
        );
        normalizer.normalize(&mut partial_response);

        let StreamResponse::TranscriptResponse { channel, .. } = partial_response else {
            panic!("expected transcript response");
        };
        let words = &channel.alternatives[0].words;
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].word, "Mark");
    }

    #[test]
    fn soniqo_normalizer_retimes_cumulative_partials() {
        let mut normalizer = SoniqoTranscriptNormalizer::default();

        let mut first =
            transcript_response_at("see", vec![word("see", 0.0, 0.25)], false, 0, 0.0, 0.25);
        normalizer.normalize(&mut first);

        let mut second = transcript_response_at(
            "see the need",
            vec![
                word("see", 0.25, 0.33),
                word("the", 0.33, 0.41),
                word("need", 0.41, 0.50),
            ],
            false,
            0,
            0.25,
            0.25,
        );
        normalizer.normalize(&mut second);

        let StreamResponse::TranscriptResponse {
            start,
            duration,
            channel,
            ..
        } = second
        else {
            panic!("expected transcript response");
        };
        let words = &channel.alternatives[0].words;

        assert_eq!(start, 0.0);
        assert_eq!(duration, 0.5);
        assert_eq!(words.len(), 3);
        assert_eq!(words[0].word, "see");
        assert_eq!(words[0].start, 0.0);
        assert_eq!(words[2].end, 0.45);
    }

    #[test]
    fn soniqo_normalizer_trims_sliding_overlap() {
        let mut normalizer = SoniqoTranscriptNormalizer::default();

        let mut first = transcript_response_at(
            "see the need",
            vec![
                word("see", 0.0, 0.20),
                word("the", 0.20, 0.40),
                word("need", 0.40, 0.60),
            ],
            false,
            0,
            0.0,
            0.60,
        );
        normalizer.normalize(&mut first);

        let mut second = transcript_response_at(
            "the need now",
            vec![
                word("the", 0.60, 0.70),
                word("need", 0.70, 0.80),
                word("now", 0.80, 0.90),
            ],
            false,
            0,
            0.60,
            0.30,
        );
        normalizer.normalize(&mut second);

        let StreamResponse::TranscriptResponse { channel, .. } = second else {
            panic!("expected transcript response");
        };
        let alternative = &channel.alternatives[0];

        assert_eq!(alternative.transcript, "now");
        assert_eq!(
            alternative
                .words
                .iter()
                .map(|word| word.word.as_str())
                .collect::<Vec<_>>(),
            vec!["now"],
        );
    }

    #[test]
    fn soniqo_normalizer_drops_repeated_committed_history() {
        let mut normalizer = SoniqoTranscriptNormalizer::default();
        let repeated = "and it tested if you feel";

        let mut first = transcript_response_at(
            repeated,
            words_from_text(repeated, 0.0, 1.0),
            true,
            0,
            0.0,
            1.0,
        );
        normalizer.normalize(&mut first);

        let mut second = transcript_response_at(
            repeated,
            words_from_text(repeated, 10.0, 1.0),
            true,
            0,
            10.0,
            1.0,
        );
        normalizer.normalize(&mut second);

        let StreamResponse::TranscriptResponse { channel, .. } = second else {
            panic!("expected transcript response");
        };
        let alternative = &channel.alternatives[0];

        assert!(alternative.words.is_empty());
        assert_eq!(alternative.transcript, "");
    }

    #[test]
    fn soniqo_normalizer_trims_repeated_committed_prefix_from_later_update() {
        let mut normalizer = SoniqoTranscriptNormalizer::default();
        let repeated = "and it tested if you feel";
        let filler = "centralized online url for";
        let repeated_with_tail = "and it tested if you feel like new material";

        let mut first = transcript_response_at(
            repeated,
            words_from_text(repeated, 0.0, 1.0),
            true,
            0,
            0.0,
            1.0,
        );
        normalizer.normalize(&mut first);

        let mut second =
            transcript_response_at(filler, words_from_text(filler, 2.0, 1.0), true, 0, 2.0, 1.0);
        normalizer.normalize(&mut second);

        let mut third = transcript_response_at(
            repeated_with_tail,
            words_from_text(repeated_with_tail, 10.0, 1.0),
            false,
            0,
            10.0,
            1.0,
        );
        normalizer.normalize(&mut third);

        let StreamResponse::TranscriptResponse { channel, .. } = third else {
            panic!("expected transcript response");
        };
        let alternative = &channel.alternatives[0];

        assert_eq!(alternative.transcript, "like new material");
        assert_eq!(
            alternative
                .words
                .iter()
                .map(|word| word.word.as_str())
                .collect::<Vec<_>>(),
            vec!["like", "new", "material"],
        );
    }

    #[test]
    fn soniqo_normalizer_collapses_internal_partial_loop() {
        let mut normalizer = SoniqoTranscriptNormalizer::default();
        let looped = concat!(
            "yeah but but there's super valuable information in there right ",
            "it's just like it's a little bit like extracting it out of this like junior develop ",
            "yeah but but there's super valuable information in there right ",
            "it's just like it's a little bit like extracting it out of this like junior developer's ",
            "kind of like private freak out it's it's a very difficult problem set because ",
            "it's so you know yeah but but there's super valuable information in there right ",
            "it's just like it's a little bit like extracting it out of this like junior developer's ",
            "kind of like private freak out it's it's a very"
        );

        let mut response = transcript_response_at(
            looped,
            words_from_text(looped, 0.0, 10.0),
            false,
            0,
            0.0,
            10.0,
        );
        normalizer.normalize(&mut response);

        let StreamResponse::TranscriptResponse { channel, .. } = response else {
            panic!("expected transcript response");
        };
        let transcript = &channel.alternatives[0].transcript;

        assert_eq!(transcript.matches("yeah but but").count(), 1);
        assert!(transcript.contains("private freak out"));
    }

    #[test]
    fn soniqo_engine_replaces_cumulative_live_partials() {
        let mut engine = LiveTranscriptEngine::new("soniqo", &[], None);

        let first =
            transcript_response_at("see", vec![word("see", 0.0, 0.25)], false, 0, 0.0, 0.25);
        engine.process(&first).expect("first update");

        let second = transcript_response_at(
            "see the need",
            vec![
                word("see", 0.25, 0.33),
                word("the", 0.33, 0.41),
                word("need", 0.41, 0.50),
            ],
            false,
            0,
            0.25,
            0.25,
        );
        let update = engine.process(&second).expect("second update");
        let segment_delta = update.segment_delta.expect("segment delta");

        assert_eq!(segment_delta.upserts.len(), 1);
        assert_eq!(segment_delta.upserts[0].text, "see the need");
    }

    #[test]
    fn soniqo_engine_does_not_persist_repeated_final_history() {
        let mut engine = LiveTranscriptEngine::new("soniqo", &[], None);
        let repeated = "and it tested if you feel";

        let first = transcript_response_at(
            repeated,
            words_from_text(repeated, 0.0, 1.0),
            true,
            0,
            0.0,
            1.0,
        );
        let first_update = engine.process(&first).expect("first update");

        let second = transcript_response_at(
            repeated,
            words_from_text(repeated, 10.0, 1.0),
            true,
            0,
            10.0,
            1.0,
        );
        assert!(engine.process(&second).is_none());

        let flush_update = engine.flush().expect("flush update");
        let final_text = first_update
            .transcript_delta
            .new_words
            .iter()
            .chain(flush_update.transcript_delta.new_words.iter())
            .map(|word| word.text.as_str())
            .collect::<String>();

        assert_eq!(final_text.trim(), repeated);
    }

    #[test]
    fn soniqo_engine_does_not_persist_internal_partial_loop() {
        let mut engine = LiveTranscriptEngine::new("soniqo", &[], None);
        let looped = concat!(
            "yeah but but there's super valuable information in there right ",
            "it's just like it's a little bit like extracting it out of this like junior develop ",
            "yeah but but there's super valuable information in there right ",
            "it's just like it's a little bit like extracting it out of this like junior developer's ",
            "kind of like private freak out it's it's a very difficult problem set because ",
            "it's so you know yeah but but there's super valuable information in there right ",
            "it's just like it's a little bit like extracting it out of this like junior developer's ",
            "kind of like private freak out it's it's a very"
        );
        let response = transcript_response_at(
            looped,
            words_from_text(looped, 0.0, 10.0),
            false,
            0,
            0.0,
            10.0,
        );

        engine.process(&response).expect("partial update");
        let flush_update = engine.flush().expect("flush update");
        let segment_delta = flush_update.segment_delta.expect("segment delta");

        assert!(flush_update.transcript_delta.new_words.is_empty());
        assert!(flush_update.transcript_delta.partials.is_empty());
        assert!(segment_delta.upserts.is_empty());
        assert!(!segment_delta.removed_ids.is_empty());
    }

    #[test]
    fn soniqo_engine_persists_model_final_words_on_flush() {
        let mut engine = LiveTranscriptEngine::new("soniqo", &[], None);
        let response = transcript_response_at(
            "hello world",
            words_from_text("hello world", 0.0, 1.0),
            true,
            0,
            0.0,
            1.0,
        );

        let first_update = engine.process(&response).expect("first update");
        let flush_update = engine.flush().expect("flush update");
        let final_text = first_update
            .transcript_delta
            .new_words
            .iter()
            .chain(flush_update.transcript_delta.new_words.iter())
            .map(|word| word.text.as_str())
            .collect::<String>();

        assert_eq!(final_text.trim(), "hello world");
        assert!(flush_update.transcript_delta.partials.is_empty());
    }

    #[test]
    fn live_transcript_delta_keeps_speaker_index_on_words() {
        let delta = TranscriptDelta {
            new_words: vec![FinalizedWord {
                id: "word-1".to_string(),
                text: "hello".to_string(),
                start_ms: 0,
                end_ms: 100,
                channel: 0,
                state: hypr_transcript::WordState::Final,
                speaker_index: Some(1),
            }],
            replaced_ids: vec!["replaced".to_string()],
            partials: vec![PartialWord {
                text: "world".to_string(),
                start_ms: 100,
                end_ms: 200,
                channel: 1,
                speaker_index: Some(2),
            }],
        };

        let converted: LiveTranscriptDelta = delta.into();
        assert_eq!(converted.new_words[0].speaker_index, Some(1));
        assert_eq!(converted.partials[0].speaker_index, Some(2));
        assert_eq!(converted.replaced_ids, vec!["replaced"]);
    }
}
