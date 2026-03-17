use std::collections::HashMap;

use crate::types::{ChannelProfile, Segment, SegmentBuilderOptions, SegmentKey, SegmentWord};

use super::model::{ProtoSegment, ResolvedWordFrame, SpeakerIdentity, SpeakerState};
use super::speakers::assign_complete_channel_human_id;

pub(super) fn collect_segments(
    frames: Vec<ResolvedWordFrame>,
    options: Option<&SegmentBuilderOptions>,
) -> Vec<ProtoSegment> {
    let max_gap_ms = options.and_then(|opts| opts.max_gap_ms).unwrap_or(2000);
    let mut segments: Vec<ProtoSegment> = Vec::new();
    let mut last_segment_by_channel: HashMap<ChannelProfile, usize> = HashMap::new();

    for frame in frames {
        let key = determine_key(&frame, &segments, &last_segment_by_channel);

        let should_merge = segments.last().is_some_and(|last| {
            last.key == key
                && (frame.word.start_ms - last.words.last().map_or(0, |word| word.word.end_ms))
                    <= max_gap_ms
        });

        if should_merge {
            segments.last_mut().unwrap().words.push(frame);
            continue;
        }

        let channel = key.channel;
        segments.push(ProtoSegment {
            key,
            words: vec![frame],
        });
        last_segment_by_channel.insert(channel, segments.len() - 1);
    }

    segments
}

pub(super) fn propagate_identity(segments: &mut Vec<ProtoSegment>, speaker_state: &SpeakerState) {
    let mut write_index = 0;
    let mut last_kept_key: Option<SegmentKey> = None;
    let mut last_kept_idx: Option<usize> = None;

    for read_index in 0..segments.len() {
        assign_complete_channel_human_id(&mut segments[read_index], speaker_state);

        let should_merge = last_kept_key.as_ref().is_some_and(|last_key| {
            *last_key == segments[read_index].key && segments[read_index].key.has_speaker_identity()
        });

        if should_merge {
            let words = std::mem::take(&mut segments[read_index].words);
            if let Some(kept_index) = last_kept_idx {
                segments[kept_index].words.extend(words);
            }
            continue;
        }

        last_kept_key = Some(segments[read_index].key.clone());
        if write_index != read_index {
            segments.swap(write_index, read_index);
        }
        last_kept_idx = Some(write_index);
        write_index += 1;
    }

    segments.truncate(write_index);
}

pub(super) fn finalize_segments(proto_segments: Vec<ProtoSegment>) -> Vec<Segment> {
    proto_segments
        .into_iter()
        .map(|segment| Segment {
            key: segment.key,
            words: segment
                .words
                .into_iter()
                .map(|frame| SegmentWord {
                    text: frame.word.text,
                    start_ms: frame.word.start_ms,
                    end_ms: frame.word.end_ms,
                    channel: frame.word.channel,
                    is_final: frame.word.is_final,
                    id: frame.word.id,
                })
                .collect(),
        })
        .collect()
}

fn determine_key(
    frame: &ResolvedWordFrame,
    segments: &[ProtoSegment],
    last_segment_by_channel: &HashMap<ChannelProfile, usize>,
) -> SegmentKey {
    if !frame.word.is_final {
        if let Some(&index) = last_segment_by_channel.get(&frame.word.channel) {
            return segments[index].key.clone();
        }
    }

    create_segment_key(frame.word.channel, frame.identity.as_ref())
}

fn create_segment_key(channel: ChannelProfile, identity: Option<&SpeakerIdentity>) -> SegmentKey {
    SegmentKey {
        channel,
        speaker_index: identity.and_then(|value| value.speaker_index),
        speaker_human_id: identity.and_then(|value| value.human_id.clone()),
    }
}
