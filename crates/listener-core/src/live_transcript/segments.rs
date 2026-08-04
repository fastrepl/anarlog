use std::collections::{BTreeMap, BTreeSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

use anlg_transcript::{
    FinalizedWord, IdentityAssignment, PartialWord, SegmentBuilderOptions, build_segments,
    normalize_rendered_segment_words, stable_segment_id,
};

use super::{LiveTranscriptDelta, LiveTranscriptSegment, LiveTranscriptSegmentDelta};

const MAX_RENDERED_WORDS: usize = 4_096;
const MAX_RENDERED_TEXT_BYTES: usize = 64 * 1024;
const MAX_RENDERED_WINDOW_MS: i64 = 30 * 60 * 1_000;

#[derive(Default)]
pub(super) struct RenderedSegmentState {
    words: Vec<FinalizedWord>,
    segment_revisions: BTreeMap<String, u64>,
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
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let new_word_ids = delta
            .new_words
            .iter()
            .map(|word| word.id.as_str())
            .collect::<BTreeSet<_>>();

        self.words.retain(|word| {
            !replaced_ids.contains(word.id.as_str()) && !new_word_ids.contains(word.id.as_str())
        });

        for word in &delta.new_words {
            insert_bounded_word(&mut self.words, bounded_finalized_word(word));
        }

        let latest_end_ms = self
            .words
            .iter()
            .map(|word| word.end_ms)
            .chain(delta.partials.iter().map(|word| word.end_ms))
            .max()
            .unwrap_or_default();
        prune_rendered_words(&mut self.words, latest_end_ms);
        let partials = bounded_partials(
            &delta.partials,
            latest_end_ms,
            MAX_RENDERED_WORDS.saturating_sub(self.words.len()),
            MAX_RENDERED_TEXT_BYTES.saturating_sub(rendered_text_bytes(&self.words)),
        );

        let next_segments = build_live_segments(
            &self.words,
            &partials,
            &self.channel_assignments,
            self.segment_options.as_ref(),
        );
        let mut next_revisions = BTreeMap::new();
        let mut upserts = Vec::new();

        for segment in next_segments {
            let revision = segment_revision(&segment);
            let id = segment.id.clone();
            if self.segment_revisions.get(&id) != Some(&revision) {
                upserts.push(segment);
            }
            next_revisions.insert(id, revision);
        }

        let removed_ids = self
            .segment_revisions
            .keys()
            .filter(|id| !next_revisions.contains_key(*id))
            .cloned()
            .collect::<Vec<_>>();
        self.segment_revisions = next_revisions;

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

fn bounded_finalized_word(word: &FinalizedWord) -> FinalizedWord {
    let mut word = word.clone();
    word.text = bounded_text(&word.text, MAX_RENDERED_TEXT_BYTES);
    word
}

fn insert_bounded_word(words: &mut Vec<FinalizedWord>, word: FinalizedWord) {
    let index = words
        .binary_search_by(|existing| {
            existing
                .start_ms
                .cmp(&word.start_ms)
                .then_with(|| existing.id.cmp(&word.id))
        })
        .unwrap_or_else(|index| index);

    if words.len() < MAX_RENDERED_WORDS {
        words.insert(index, word);
    } else if index > 0 {
        words.remove(0);
        words.insert(index - 1, word);
    }
}

fn bounded_partials(
    partials: &[PartialWord],
    latest_end_ms: i64,
    max_words: usize,
    max_text_bytes: usize,
) -> Vec<PartialWord> {
    if max_words == 0 || max_text_bytes == 0 {
        return Vec::new();
    }

    let cutoff_ms = latest_end_ms.saturating_sub(MAX_RENDERED_WINDOW_MS);
    let mut latest = Vec::with_capacity(max_words.min(partials.len()));
    for partial in partials {
        if partial.end_ms < cutoff_ms {
            continue;
        }

        let index = latest
            .binary_search_by(|existing: &&PartialWord| {
                existing
                    .start_ms
                    .cmp(&partial.start_ms)
                    .then_with(|| existing.end_ms.cmp(&partial.end_ms))
                    .then_with(|| existing.channel.cmp(&partial.channel))
            })
            .unwrap_or_else(|index| index);
        if latest.len() < max_words {
            latest.insert(index, partial);
        } else if index > 0 {
            latest.remove(0);
            latest.insert(index - 1, partial);
        }
    }

    let mut retained = Vec::with_capacity(latest.len());
    let mut retained_bytes = 0;

    for partial in latest.into_iter().rev() {
        let remaining_bytes = max_text_bytes.saturating_sub(retained_bytes);
        if remaining_bytes == 0 {
            break;
        }

        let mut partial = partial.clone();
        partial.text = bounded_text(&partial.text, remaining_bytes);
        retained_bytes += partial.text.len();
        retained.push(partial);
    }

    retained.reverse();
    retained
}

fn prune_rendered_words(words: &mut Vec<FinalizedWord>, latest_end_ms: i64) {
    let cutoff_ms = latest_end_ms.saturating_sub(MAX_RENDERED_WINDOW_MS);
    words.retain(|word| word.end_ms >= cutoff_ms);

    if words.len() > MAX_RENDERED_WORDS {
        words.drain(0..words.len() - MAX_RENDERED_WORDS);
    }

    let mut text_bytes = rendered_text_bytes(words);
    let mut remove_count = 0;
    while text_bytes > MAX_RENDERED_TEXT_BYTES && words.len() - remove_count > 1 {
        text_bytes = text_bytes.saturating_sub(words[remove_count].text.len());
        remove_count += 1;
    }
    if remove_count > 0 {
        words.drain(0..remove_count);
    }

    if let Some(word) = words.first_mut()
        && word.text.len() > MAX_RENDERED_TEXT_BYTES
    {
        word.text = bounded_text(&word.text, MAX_RENDERED_TEXT_BYTES);
    }
}

fn rendered_text_bytes(words: &[FinalizedWord]) -> usize {
    words.iter().map(|word| word.text.len()).sum()
}

fn bounded_text(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_owned();
    }

    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

fn segment_revision(segment: &LiveTranscriptSegment) -> u64 {
    let mut hasher = DefaultHasher::new();
    segment.key.hash(&mut hasher);
    segment.start_ms.hash(&mut hasher);
    segment.end_ms.hash(&mut hasher);
    segment.text.hash(&mut hasher);
    segment.words.len().hash(&mut hasher);

    for word in &segment.words {
        word.text.hash(&mut hasher);
        word.start_ms.hash(&mut hasher);
        word.end_ms.hash(&mut hasher);
        word.channel.hash(&mut hasher);
        word.is_final.hash(&mut hasher);
        word.id.hash(&mut hasher);
    }

    hasher.finish()
}

fn build_live_segments(
    final_words: &[FinalizedWord],
    partials: &[PartialWord],
    channel_assignments: &[IdentityAssignment],
    segment_options: Option<&SegmentBuilderOptions>,
) -> Vec<LiveTranscriptSegment> {
    build_segments(final_words, partials, channel_assignments, segment_options)
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

#[cfg(test)]
mod tests {
    use anlg_transcript::{FinalizedWord, PartialWord, SegmentBuilderOptions, WordState};

    use super::{
        LiveTranscriptDelta, MAX_RENDERED_TEXT_BYTES, MAX_RENDERED_WINDOW_MS, MAX_RENDERED_WORDS,
        RenderedSegmentState, bounded_partials, rendered_text_bytes,
    };

    fn finalized_word(id: impl Into<String>, text: String, start_ms: i64) -> FinalizedWord {
        FinalizedWord {
            id: id.into(),
            text,
            start_ms,
            end_ms: start_ms + 100,
            channel: 0,
            state: WordState::Final,
            speaker_index: None,
        }
    }

    fn state() -> RenderedSegmentState {
        RenderedSegmentState::new(
            Vec::new(),
            SegmentBuilderOptions {
                max_gap_ms: Some(500),
                complete_channels: None,
                min_segment_words: None,
                min_segment_ms: None,
            },
        )
    }

    #[test]
    fn bounds_one_giant_same_speaker_segment() {
        let mut state = state();
        let update = state
            .apply_delta(&LiveTranscriptDelta {
                new_words: vec![finalized_word(
                    "giant",
                    "a".repeat(MAX_RENDERED_TEXT_BYTES * 2),
                    0,
                )],
                replaced_ids: Vec::new(),
                partials: Vec::new(),
            })
            .expect("giant word should render");

        assert!(rendered_text_bytes(&state.words) <= MAX_RENDERED_TEXT_BYTES);
        assert!(
            update
                .upserts
                .iter()
                .flat_map(|segment| &segment.words)
                .map(|word| word.text.len())
                .sum::<usize>()
                <= MAX_RENDERED_TEXT_BYTES
        );
    }

    #[test]
    fn bounds_same_speaker_word_history() {
        let mut state = state();
        state.apply_delta(&LiveTranscriptDelta {
            new_words: (0..MAX_RENDERED_WORDS + 20)
                .map(|index| {
                    finalized_word(
                        format!("word-{index:05}"),
                        " word".to_string(),
                        index as i64,
                    )
                })
                .collect(),
            replaced_ids: Vec::new(),
            partials: Vec::new(),
        });

        assert_eq!(state.words.len(), MAX_RENDERED_WORDS);
        assert_eq!(
            state.words.first().map(|word| word.id.as_str()),
            Some("word-00020")
        );
    }

    #[test]
    fn keeps_bounded_capacity_during_steady_state_eviction() {
        let mut state = state();
        state.apply_delta(&LiveTranscriptDelta {
            new_words: (0..MAX_RENDERED_WORDS)
                .map(|index| {
                    finalized_word(
                        format!("word-{index:05}"),
                        " word".to_string(),
                        index as i64,
                    )
                })
                .collect(),
            replaced_ids: Vec::new(),
            partials: Vec::new(),
        });
        let steady_capacity = state.words.capacity();

        for index in MAX_RENDERED_WORDS..MAX_RENDERED_WORDS + 100 {
            state.apply_delta(&LiveTranscriptDelta {
                new_words: vec![finalized_word(
                    format!("word-{index:05}"),
                    " word".to_string(),
                    index as i64,
                )],
                replaced_ids: Vec::new(),
                partials: Vec::new(),
            });
            assert_eq!(state.words.capacity(), steady_capacity);
        }

        assert!(steady_capacity <= MAX_RENDERED_WORDS);
        assert_eq!(state.words.len(), MAX_RENDERED_WORDS);
    }

    #[test]
    fn bounds_unsorted_partials_to_the_newest_words() {
        let partial = |text: &str, start_ms: i64| PartialWord {
            text: text.to_string(),
            start_ms,
            end_ms: start_ms + 10,
            channel: 0,
            speaker_index: None,
        };
        let partials = vec![
            partial(" newest", 300),
            partial(" oldest", 100),
            partial(" middle", 200),
        ];

        let retained = bounded_partials(&partials, 310, 2, MAX_RENDERED_TEXT_BYTES);

        assert_eq!(
            retained
                .iter()
                .map(|partial| partial.text.as_str())
                .collect::<Vec<_>>(),
            vec![" middle", " newest"]
        );
    }

    #[test]
    fn emits_removals_when_the_render_window_evicts_old_segments() {
        let mut state = state();
        let first = state
            .apply_delta(&LiveTranscriptDelta {
                new_words: vec![finalized_word("old", " old".to_string(), 0)],
                replaced_ids: Vec::new(),
                partials: Vec::new(),
            })
            .expect("old segment should render");
        let old_ids = first
            .upserts
            .into_iter()
            .map(|segment| segment.id)
            .collect::<Vec<_>>();

        let next = state
            .apply_delta(&LiveTranscriptDelta {
                new_words: vec![finalized_word(
                    "new",
                    " new".to_string(),
                    MAX_RENDERED_WINDOW_MS + 1_000,
                )],
                replaced_ids: Vec::new(),
                partials: Vec::new(),
            })
            .expect("new segment should replace the evicted one");

        assert!(old_ids.iter().all(|id| next.removed_ids.contains(id)));
        assert!(state.words.iter().all(|word| word.id != "old"));
    }
}
