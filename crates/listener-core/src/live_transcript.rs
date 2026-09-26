use anlg_transcript::{
    FinalizedWord, IdentityAssignment, PartialWord, SegmentKey, SegmentWord, TranscriptDelta,
    TranscriptProcessor, segment_options_for_assignments,
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
    provider_name: String,
    processor: TranscriptProcessor,
    normalizer: TranscriptNormalizer,
    rendered_segments: RenderedSegmentState,
    mic_isolated: bool,
    /// Session-relative ms ranges where the mic verdict was isolated. The
    /// verdict can flip mid-capture in both directions: words captured before
    /// an era begins must not inherit the local speaker, and words captured
    /// during a closed era keep it even after headphones disconnect. An open
    /// era runs to `i64::MAX`.
    isolated_ranges: Vec<(i64, i64)>,
}

impl LiveTranscriptEngine {
    pub fn new(
        provider_name: &str,
        participant_human_ids: &[String],
        self_human_id: Option<&str>,
    ) -> Self {
        Self::with_speaker_assignments(
            provider_name,
            participant_human_ids,
            self_human_id,
            Vec::new(),
            false,
            0,
        )
    }

    /// `speaker_assignments` are the persisted identity hints of the transcript
    /// being captured (user picks and automatic matches), so segments the
    /// engine emits carry the same names the settled render will.
    /// `session_elapsed_ms` is the session clock position of this call; it
    /// bounds the isolated range so words captured before a mid-capture verdict
    /// flip keep their earlier identity.
    pub fn with_speaker_assignments(
        provider_name: &str,
        _participant_human_ids: &[String],
        self_human_id: Option<&str>,
        speaker_assignments: Vec<IdentityAssignment>,
        mic_isolated: bool,
        session_elapsed_ms: i64,
    ) -> Self {
        let isolated_ranges = if mic_isolated {
            vec![(session_elapsed_ms, i64::MAX)]
        } else {
            Vec::new()
        };
        let mut segment_options = segment_options_for_assignments(&speaker_assignments);
        segment_options.isolated_mic_ranges =
            (!isolated_ranges.is_empty()).then(|| isolated_ranges.clone());
        segment_options.isolated_mic_human =
            isolated_mic_human(self_human_id, !isolated_ranges.is_empty());

        let normalizer = TranscriptNormalizer::for_provider(provider_name);

        Self {
            provider_name: provider_name.to_owned(),
            processor: TranscriptProcessor::new()
                .with_partial_finalization(normalizer.finalize_partials())
                .with_flush_partial_finalization(normalizer.flush_partials())
                .with_final_word_stitching(!matches!(normalizer, TranscriptNormalizer::Nari)),
            normalizer,
            rendered_segments: RenderedSegmentState::new(
                Vec::new(),
                speaker_assignments,
                segment_options,
            ),
            mic_isolated,
            isolated_ranges,
        }
    }

    pub fn process(&mut self, response: &StreamResponse) -> Option<LiveTranscriptUpdate> {
        let mut normalized = response.clone();
        self.normalizer.normalize(&mut normalized);
        let delta = match &normalized {
            StreamResponse::TranscriptResponse {
                is_final: true,
                start,
                duration,
                channel,
                channel_index,
                ..
            } if matches!(self.normalizer, TranscriptNormalizer::Nari)
                && *duration > 0.0
                && channel
                    .alternatives
                    .first()
                    .is_some_and(|alt| alt.transcript.is_empty() && alt.words.is_empty()) =>
            {
                self.processor.clear_partials(
                    channel_index.first().copied().unwrap_or(0),
                    (*start * 1000.0) as i64,
                    ((*start + *duration) * 1000.0) as i64,
                )
            }
            _ => self.processor.process(&normalized)?,
        };
        let transcript_delta: LiveTranscriptDelta = delta.into();
        let segment_delta = self.rendered_segments.apply_delta(&transcript_delta);
        Some(LiveTranscriptUpdate {
            transcript_delta,
            segment_delta,
        })
    }

    /// Returns the segments whose labels changed so they can be pushed to
    /// listeners right away; a user naming a speaker mid-meeting should not
    /// wait for the next stream response to see it.
    pub fn update_identities(
        &mut self,
        _participant_human_ids: &[String],
        self_human_id: Option<&str>,
        speaker_assignments: Vec<IdentityAssignment>,
        mic_isolated: bool,
        session_elapsed_ms: i64,
    ) -> Option<LiveTranscriptSegmentDelta> {
        if mic_isolated && !self.mic_isolated {
            self.isolated_ranges.push((session_elapsed_ms, i64::MAX));
        } else if !mic_isolated
            && self.mic_isolated
            && let Some((_, end)) = self.isolated_ranges.last_mut()
        {
            *end = session_elapsed_ms;
        }
        self.mic_isolated = mic_isolated;
        let mut segment_options = segment_options_for_assignments(&speaker_assignments);
        segment_options.isolated_mic_ranges =
            (!self.isolated_ranges.is_empty()).then(|| self.isolated_ranges.clone());
        segment_options.isolated_mic_human =
            isolated_mic_human(self_human_id, !self.isolated_ranges.is_empty());
        self.rendered_segments
            .update_identities(Vec::new(), speaker_assignments, segment_options)
    }

    pub fn provider_name(&self) -> &str {
        &self.provider_name
    }

    /// Finalize pending words for a stream that is about to be replaced. Unlike `flush`, the
    /// delivery position survives, so the replacement stream's replayed audio is not emitted twice.
    pub fn checkpoint(&mut self) -> Option<LiveTranscriptUpdate> {
        let delta = self.processor.checkpoint();
        self.update_from(delta.into())
    }

    pub fn flush(&mut self) -> Option<LiveTranscriptUpdate> {
        let delta = self.processor.flush();
        self.update_from(delta.into())
    }

    fn update_from(
        &mut self,
        transcript_delta: LiveTranscriptDelta,
    ) -> Option<LiveTranscriptUpdate> {
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

// The desktop stops synthesizing channel defaults once the session records a
// speaker context, but the engine never sees that context, so index-less mic
// words would stay anonymous until the settled render. An isolated mic only
// carries the local voice — the same premise `resolve_speaker` uses for
// isolated intervals — so the engine names it as the range-gated fallback.
// Words keep their own scope precedence, and the ranges only cover audio from
// isolated eras themselves.
fn isolated_mic_human(self_human_id: Option<&str>, has_isolated_ranges: bool) -> Option<String> {
    self_human_id
        .filter(|id| has_isolated_ranges && !id.is_empty())
        .map(String::from)
}

#[cfg(test)]
mod tests;
