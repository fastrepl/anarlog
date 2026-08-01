use anlg_transcript::{
    FinalizedWord, PartialWord, SegmentKey, SegmentWord, TranscriptDelta, TranscriptProcessor,
    channel_assignments_for_participants, segment_options_for_participants,
};
use owhisper_interface::stream::StreamResponse;

mod normalizer;
mod segments;

use normalizer::TranscriptNormalizer;
#[cfg(test)]
use normalizer::{SoniqoTranscriptNormalizer, drain_soniqo_prefix, normalize_tokens_for_overlap};
use segments::RenderedSegmentState;

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
    max_speaker_index: Option<i32>,
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
        let max_speaker_index =
            max_speaker_index_for_participants(participant_human_ids, self_human_id);

        let normalizer = TranscriptNormalizer::for_provider(provider_name);

        Self {
            processor: TranscriptProcessor::new()
                .with_partial_finalization(normalizer.finalize_partials())
                .with_flush_partial_finalization(normalizer.flush_partials()),
            normalizer,
            rendered_segments: RenderedSegmentState::new(channel_assignments, segment_options),
            max_speaker_index,
        }
    }

    pub fn process(&mut self, response: &StreamResponse) -> Option<LiveTranscriptUpdate> {
        let mut normalized = response.clone();
        self.normalizer.normalize(&mut normalized);
        clamp_response_speaker_indices(&mut normalized, self.max_speaker_index);
        let transcript_delta: LiveTranscriptDelta = self.processor.process(&normalized)?.into();
        let segment_delta = self.rendered_segments.apply_delta(&transcript_delta);
        Some(LiveTranscriptUpdate {
            transcript_delta,
            segment_delta,
        })
    }

    pub fn update_participants(
        &mut self,
        participant_human_ids: &[String],
        self_human_id: Option<&str>,
    ) {
        self.rendered_segments.update_participants(
            channel_assignments_for_participants(participant_human_ids, self_human_id),
            segment_options_for_participants(participant_human_ids, self_human_id),
        );
        self.max_speaker_index =
            max_speaker_index_for_participants(participant_human_ids, self_human_id);
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

fn max_speaker_index_for_participants(
    participant_human_ids: &[String],
    self_human_id: Option<&str>,
) -> Option<i32> {
    crate::expected_speakers_per_channel(participant_human_ids, self_human_id)
        .and_then(|count| count.checked_sub(1))
        .and_then(|index| i32::try_from(index).ok())
}

fn clamp_response_speaker_indices(response: &mut StreamResponse, max_speaker_index: Option<i32>) {
    let Some(max_speaker_index) = max_speaker_index else {
        return;
    };

    let StreamResponse::TranscriptResponse { channel, .. } = response else {
        return;
    };

    for alternative in &mut channel.alternatives {
        for word in &mut alternative.words {
            if let Some(speaker) = word.speaker {
                word.speaker = Some(speaker.clamp(0, max_speaker_index));
            }
        }
    }
}

#[cfg(test)]
mod tests;
