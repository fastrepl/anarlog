mod channel_state;
mod label;
mod postprocessor;
mod processor;
mod render;
mod segments;
mod types;
mod words;

pub use label::{SpeakerLabelContext, SpeakerLabeler, render_speaker_label};
pub use postprocessor::{
    TranscriptPostprocessor, TranscriptPostprocessorError, TranscriptPostprocessorRequest,
    TranscriptPostprocessorResult,
};
pub use processor::TranscriptProcessor;
pub use render::{
    RenderTranscriptHuman, RenderTranscriptInput, RenderTranscriptRequest,
    RenderTranscriptWordInput, RenderedTranscriptSegment, normalize_rendered_segment_words,
    render_transcript_segments, stable_segment_id,
};
pub use segments::build_segments;
pub use types::{
    ChannelProfile, FinalizedWord, IdentityAssignment, IdentityScope, PartialWord, RawWord,
    Segment, SegmentBuilderOptions, SegmentKey, SegmentWord, TranscriptDelta, WordState,
    channel_assignments_for_participants, segment_options_for_participants,
};
