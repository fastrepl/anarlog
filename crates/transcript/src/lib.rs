mod batch_refine;
mod channel_state;
mod label;
mod live_materialize;
mod postprocessor;
mod processor;
mod render;
mod segments;
mod speaker_context;
mod synthetic_render;
pub use speaker_context::{
    ProvisionalSpeakerLabel, SpeakerContext, SpeakerContextInterval, SpeakerResolutionReason,
    segment_options_for_assignments,
};
pub use synthetic_render::synthetic_timing_from_metadata;
mod types;
mod words;

pub use batch_refine::{
    BatchRefinementOutcome, BatchRefinementRequest, BatchRefinementSource,
    BatchTranscriptPromotion, SpeakerClusterReconciliationRequest, StoredSpeakerHint,
    StoredTranscriptWord, parse_stored_speaker_hints, parse_stored_transcript_words,
    reconcile_refined_speaker_clusters, refine_batch_transcript, render_input_from_stored,
};
pub use label::{SpeakerLabelContext, SpeakerLabeler, render_speaker_label};
pub use live_materialize::{
    StoredLiveTranscriptDelta, apply_live_transcript_delta, coalesce_live_transcript_deltas,
    materialize_live_transcript, serialize_batch_transcript_hints,
    serialize_batch_transcript_words,
};
pub use postprocessor::{
    TranscriptPostprocessor, TranscriptPostprocessorError, TranscriptPostprocessorRequest,
    TranscriptPostprocessorResult,
};
pub use processor::TranscriptProcessor;
pub use render::{
    RenderTranscriptHuman, RenderTranscriptInput, RenderTranscriptRequest,
    RenderTranscriptWordInput, RenderedTranscriptSegment, SyntheticTiming,
    normalize_rendered_segment_words, render_transcript_segments, stable_segment_id,
};
pub use segments::build_segments;
pub use types::{
    ChannelProfile, FinalizedWord, IdentityAssignment, IdentityScope, PartialWord, RawWord,
    Segment, SegmentBuilderOptions, SegmentKey, SegmentWord, TranscriptDelta, WordState,
    channel_assignments_for_participants, segment_options_for_participants,
};
