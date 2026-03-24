mod channel_state;
mod label;
mod postprocessor;
mod processor;
mod render;
mod segment_types;
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
    RenderTranscriptSpeakerHint, RenderTranscriptWordInput, RenderedTranscriptSegment,
    normalize_rendered_segment_words, render_transcript_segments, stable_segment_id,
};
pub use segment_types::{ChannelProfile, Segment, SegmentBuilderOptions, SegmentKey, SegmentWord};
pub use segments::build_segments;
pub use types::{
    FinalizedWord, PartialWord, RawWord, RuntimeSpeakerHint, SpeakerHintData, TranscriptDelta,
    WordRef, WordState,
};
