use std::collections::BTreeMap;

use hypr_transcript::{
    FinalizedWord, IdentityAssignment, PartialWord, SegmentBuilderOptions, SegmentKey, SegmentWord,
    TranscriptDelta, TranscriptProcessor, build_segments, channel_assignments_for_participants,
    normalize_rendered_segment_words, segment_options_for_participants, stable_segment_id,
};
use owhisper_interface::stream::{StreamResponse, Word};

const CACTUS_OVERLAP_MAX_GAP_MS: i64 = 500;

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

        Self {
            processor: TranscriptProcessor::new(),
            normalizer: TranscriptNormalizer::for_provider(provider_name),
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
        if transcript_delta.is_empty() {
            return None;
        }

        let segment_delta = self.rendered_segments.apply_delta(&transcript_delta);
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
    #[default]
    Passthrough,
}

impl TranscriptNormalizer {
    fn for_provider(provider_name: &str) -> Self {
        if provider_name == "cactus" {
            Self::Cactus(CactusTranscriptNormalizer::default())
        } else {
            Self::Passthrough
        }
    }

    fn normalize(&mut self, response: &mut StreamResponse) {
        match self {
            Self::Cactus(normalizer) => normalizer.normalize(response),
            Self::Passthrough => {}
        }
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

fn normalize_tokens_for_overlap(words: &[Word]) -> Vec<String> {
    words
        .iter()
        .map(normalize_word_token)
        .filter(|token| !token.is_empty())
        .collect()
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
        StreamResponse::TranscriptResponse {
            start: 0.0,
            duration: 0.0,
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
