use std::collections::BTreeMap;

use anlg_transcript::{
    FinalizedWord, IdentityAssignment, PartialWord, SegmentBuilderOptions, build_segments,
    normalize_rendered_segment_words, stable_segment_id,
};

use super::{LiveTranscriptDelta, LiveTranscriptSegment, LiveTranscriptSegmentDelta};

#[derive(Default)]
pub(super) struct RenderedSegmentState {
    words: BTreeMap<String, FinalizedWord>,
    partials: Vec<PartialWord>,
    segments: BTreeMap<String, LiveTranscriptSegment>,
    channel_assignments: Vec<IdentityAssignment>,
    segment_options: Option<SegmentBuilderOptions>,
}

impl RenderedSegmentState {
    pub(super) fn new(
        channel_assignments: Vec<IdentityAssignment>,
        segment_options: SegmentBuilderOptions,
    ) -> Self {
        Self {
            channel_assignments,
            segment_options: Some(segment_options),
            ..Default::default()
        }
    }

    pub(super) fn update_participants(
        &mut self,
        channel_assignments: Vec<IdentityAssignment>,
        segment_options: SegmentBuilderOptions,
    ) {
        self.channel_assignments = channel_assignments;
        self.segment_options = Some(segment_options);
    }

    pub(super) fn apply_delta(
        &mut self,
        delta: &LiveTranscriptDelta,
    ) -> Option<LiveTranscriptSegmentDelta> {
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
