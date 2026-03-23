use crate::types::{
    ChannelProfile, FinalizedWord, PartialWord, RuntimeSpeakerHint, Segment, SegmentBuilderOptions,
    SegmentKey, SpeakerHintData, WordRef, WordState,
};

use super::build_segments;

fn fw(text: &str, start: i64, end: i64, ch: i32) -> FinalizedWord {
    FinalizedWord {
        id: format!("w-{text}"),
        text: text.to_string(),
        start_ms: start,
        end_ms: end,
        channel: ch,
        state: WordState::Final,
    }
}

fn pw(text: &str, start: i64, end: i64, ch: i32) -> PartialWord {
    PartialWord {
        text: text.to_string(),
        start_ms: start,
        end_ms: end,
        channel: ch,
    }
}

fn hint_idx(word_id: &str, speaker_index: i32) -> RuntimeSpeakerHint {
    RuntimeSpeakerHint {
        target: WordRef::FinalWordId(word_id.to_string()),
        data: SpeakerHintData::ProviderSpeakerIndex {
            speaker_index,
            provider: None,
            channel: None,
        },
    }
}

fn hint_runtime_idx(word_index: usize, speaker_index: i32) -> RuntimeSpeakerHint {
    RuntimeSpeakerHint {
        target: WordRef::RuntimeIndex(word_index),
        data: SpeakerHintData::ProviderSpeakerIndex {
            speaker_index,
            provider: None,
            channel: None,
        },
    }
}

fn hint_human_id(target: WordRef, human_id: &str) -> RuntimeSpeakerHint {
    RuntimeSpeakerHint {
        target,
        data: SpeakerHintData::UserSpeakerAssignment {
            human_id: human_id.to_string(),
        },
    }
}

fn key(ch: i32) -> SegmentKey {
    SegmentKey {
        channel: ChannelProfile::from(ch),
        speaker_index: None,
        speaker_human_id: None,
    }
}

fn key_speaker(ch: i32, si: i32) -> SegmentKey {
    SegmentKey {
        channel: ChannelProfile::from(ch),
        speaker_index: Some(si),
        speaker_human_id: None,
    }
}

fn texts(seg: &Segment) -> Vec<&str> {
    seg.words.iter().map(|word| word.text.as_str()).collect()
}

fn is_finals(seg: &Segment) -> Vec<bool> {
    seg.words.iter().map(|word| word.is_final).collect()
}

#[test]
fn empty_input() {
    let result = build_segments(&[], &[], &[], None);
    assert!(result.is_empty());
}

#[test]
fn single_word() {
    let finals = vec![fw("0", 0, 100, 0)];
    let result = build_segments(&finals, &[], &[], None);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].key, key(0));
    assert_eq!(texts(&result[0]), vec!["0"]);
    assert_eq!(is_finals(&result[0]), vec![true]);
}

#[test]
fn simple_multi_channel_without_merging() {
    let finals = vec![fw("0", 0, 100, 0)];
    let partials = vec![
        pw("1", 150, 200, 0),
        pw("2", 150, 200, 1),
        pw("3", 210, 260, 1),
    ];
    let result = build_segments(&finals, &partials, &[], None);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].key, key(0));
    assert_eq!(texts(&result[0]), vec!["0", "1"]);
    assert_eq!(is_finals(&result[0]), vec![true, false]);
    assert_eq!(result[1].key, key(1));
    assert_eq!(texts(&result[1]), vec!["2", "3"]);
}

#[test]
fn interleaves_same_channel_turns() {
    let finals = vec![fw("0", 300, 400, 1)];
    let partials = vec![pw("1", 0, 100, 0), pw("2", 600, 700, 0)];
    let result = build_segments(&finals, &partials, &[], None);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].key, key(0));
    assert_eq!(texts(&result[0]), vec!["1"]);
    assert_eq!(result[1].key, key(1));
    assert_eq!(texts(&result[1]), vec!["0"]);
    assert_eq!(result[2].key, key(0));
    assert_eq!(texts(&result[2]), vec!["2"]);
}

#[test]
fn sorted_by_start_ms() {
    let finals = vec![fw("2", 400, 450, 0)];
    let partials = vec![pw("0", 100, 150, 0), pw("1", 250, 300, 0)];
    let result = build_segments(&finals, &partials, &[], None);
    assert_eq!(result.len(), 1);
    assert_eq!(texts(&result[0]), vec!["0", "1", "2"]);
}

#[test]
fn does_not_merge_past_max_gap() {
    let finals = vec![
        fw("0", 0, 100, 0),
        fw("2", 2101, 2201, 0),
        fw("1", 150, 200, 1),
    ];
    let opts = SegmentBuilderOptions {
        max_gap_ms: Some(2000),
        ..Default::default()
    };
    let result = build_segments(&finals, &[], &[], Some(&opts));
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].key, key(0));
    assert_eq!(texts(&result[0]), vec!["0"]);
    assert_eq!(result[1].key, key(1));
    assert_eq!(texts(&result[1]), vec!["1"]);
    assert_eq!(result[2].key, key(0));
    assert_eq!(texts(&result[2]), vec!["2"]);
}

#[test]
fn merges_at_exact_threshold() {
    let finals = vec![fw("0", 0, 100, 0), fw("1", 2100, 2200, 0)];
    let opts = SegmentBuilderOptions {
        max_gap_ms: Some(2000),
        ..Default::default()
    };
    let result = build_segments(&finals, &[], &[], Some(&opts));
    assert_eq!(result.len(), 1);
    assert_eq!(texts(&result[0]), vec!["0", "1"]);
}

#[test]
fn three_distinct_channels() {
    let finals = vec![
        fw("0", 0, 100, 0),
        fw("1", 150, 250, 1),
        fw("2", 300, 400, 2),
    ];
    let result = build_segments(&finals, &[], &[], None);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].key, key(0));
    assert_eq!(result[1].key, key(1));
    assert_eq!(result[2].key, key(2));
}

#[test]
fn splits_by_speaker_within_channel() {
    let finals = vec![
        fw("0", 0, 100, 0),
        fw("1", 150, 250, 0),
        fw("2", 300, 400, 0),
    ];
    let hints = vec![hint_idx("w-0", 0), hint_idx("w-1", 1), hint_idx("w-2", 0)];
    let result = build_segments(&finals, &[], &hints, None);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].key, key_speaker(0, 0));
    assert_eq!(result[1].key, key_speaker(0, 1));
    assert_eq!(result[2].key, key_speaker(0, 0));
}

#[test]
fn interleaves_short_turns() {
    let finals = vec![
        fw("0", 0, 100, 0),
        fw("1", 150, 200, 1),
        fw("2", 250, 300, 0),
        fw("3", 350, 400, 1),
        fw("4", 450, 500, 0),
    ];
    let result = build_segments(&finals, &[], &[], None);
    assert_eq!(result.len(), 5);
    assert_eq!(result[0].key, key(0));
    assert_eq!(result[1].key, key(1));
    assert_eq!(result[2].key, key(0));
    assert_eq!(result[3].key, key(1));
    assert_eq!(result[4].key, key(0));
}

#[test]
fn propagates_human_id_across_shared_speaker_index() {
    let finals = vec![fw("0", 0, 100, 0), fw("1", 200, 300, 0)];
    let hints = vec![
        hint_idx("w-0", 1),
        hint_idx("w-1", 1),
        hint_human_id(WordRef::FinalWordId("w-1".to_string()), "alice"),
    ];
    let result = build_segments(&finals, &[], &hints, None);
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].key,
        SegmentKey {
            channel: ChannelProfile::DirectMic,
            speaker_index: Some(1),
            speaker_human_id: Some("alice".to_string()),
        }
    );
    assert_eq!(texts(&result[0]), vec!["0", "1"]);
}

#[test]
fn partial_word_inherits_previous_segment_key() {
    let finals = vec![fw("0", 0, 90, 0)];
    let partials = vec![pw("1", 140, 220, 0)];
    let result = build_segments(&finals, &partials, &[], None);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].key, key(0));
    assert_eq!(texts(&result[0]), vec!["0", "1"]);
    assert_eq!(is_finals(&result[0]), vec![true, false]);
}

#[test]
fn partial_with_intermittent_speaker_hint_stays_in_previous_segment() {
    let finals = vec![fw("0", 0, 100, 0)];
    let partials = vec![pw("1", 150, 250, 0)];
    let hints = vec![hint_idx("w-0", 0)];
    let result = build_segments(&finals, &partials, &hints, None);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].key, key_speaker(0, 0));
    assert_eq!(texts(&result[0]), vec!["0", "1"]);
}

#[test]
fn places_partial_words_after_interleaving() {
    let finals = vec![fw("0", 0, 100, 0), fw("1", 150, 220, 1)];
    let partials = vec![pw("2", 230, 300, 0)];
    let result = build_segments(&finals, &partials, &[], None);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].key, key(0));
    assert_eq!(texts(&result[0]), vec!["0"]);
    assert_eq!(result[1].key, key(1));
    assert_eq!(texts(&result[1]), vec!["1"]);
    assert_eq!(result[2].key, key(0));
    assert_eq!(texts(&result[2]), vec!["2"]);
    assert_eq!(is_finals(&result[2]), vec![false]);
}

#[test]
fn custom_max_gap_ms() {
    let finals = vec![
        fw("0", 0, 100, 0),
        fw("1", 500, 600, 0),
        fw("2", 1700, 1800, 0),
    ];
    let opts = SegmentBuilderOptions {
        max_gap_ms: Some(1000),
        ..Default::default()
    };
    let result = build_segments(&finals, &[], &[], Some(&opts));
    assert_eq!(result.len(), 2);
    assert_eq!(texts(&result[0]), vec!["0", "1"]);
    assert_eq!(texts(&result[1]), vec!["2"]);
}

#[test]
fn partial_words_inherit_speaker_index_across_channels() {
    let finals = vec![fw("0", 0, 100, 0), fw("1", 150, 250, 1)];
    let partials = vec![pw("2", 300, 400, 0), pw("3", 450, 550, 1)];
    let hints = vec![hint_idx("w-0", 0), hint_idx("w-1", 1)];
    let result = build_segments(&finals, &partials, &hints, None);
    assert_eq!(result.len(), 4);
    assert_eq!(result[0].key, key_speaker(0, 0));
    assert_eq!(result[1].key, key_speaker(1, 1));
    assert_eq!(result[2].key, key_speaker(0, 0));
    assert_eq!(result[3].key, key_speaker(1, 1));
}

#[test]
fn overlapping_channels_produce_interleaved_segments() {
    let finals = vec![
        fw("0", 0, 100, 0),
        fw("1", 50, 150, 1),
        fw("2", 200, 300, 0),
        fw("3", 250, 350, 1),
    ];
    let result = build_segments(&finals, &[], &[], None);
    assert_eq!(result.len(), 4);
    assert_eq!(result[0].key, key(0));
    assert_eq!(result[1].key, key(1));
    assert_eq!(result[2].key, key(0));
    assert_eq!(result[3].key, key(1));
}

#[test]
fn auto_assign_based_on_provider_speaker_index() {
    let finals = vec![
        fw("0", 0, 100, 0),
        fw("1", 100, 200, 1),
        fw("2", 200, 300, 0),
    ];
    let hints = vec![hint_idx("w-0", 0), hint_idx("w-1", 1), hint_idx("w-2", 0)];
    let result = build_segments(&finals, &[], &hints, None);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].key, key_speaker(0, 0));
    assert_eq!(result[1].key, key_speaker(1, 1));
    assert_eq!(result[2].key, key_speaker(0, 0));
}

#[test]
fn handles_partial_only_stream_with_hints() {
    let partials = vec![pw("0", 0, 80, 0), pw("1", 120, 200, 0)];
    let hints = vec![
        hint_runtime_idx(0, 3),
        hint_human_id(WordRef::RuntimeIndex(0), "alice"),
    ];
    let result = build_segments(&[], &partials, &hints, None);
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].key,
        SegmentKey {
            channel: ChannelProfile::DirectMic,
            speaker_index: Some(3),
            speaker_human_id: Some("alice".to_string()),
        }
    );
    assert_eq!(texts(&result[0]), vec!["0", "1"]);
    assert_eq!(is_finals(&result[0]), vec![false, false]);
}

#[test]
fn propagates_direct_mic_channel_identity_forward() {
    let finals = vec![
        fw("0", 0, 100, 0),
        fw("1", 200, 300, 0),
        fw("2", 1200, 1300, 1),
        fw("3", 1500, 1600, 1),
        fw("4", 2601, 2701, 0),
    ];
    let hints = vec![hint_human_id(
        WordRef::FinalWordId("w-0".to_string()),
        "carol",
    )];
    let result = build_segments(&finals, &[], &hints, None);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].key.speaker_human_id.as_deref(), Some("carol"));
    assert_eq!(result[1].key, key(1));
    assert_eq!(result[2].key.speaker_human_id.as_deref(), Some("carol"));
}

#[test]
fn propagates_remote_party_identity_when_channel_marked_complete() {
    let finals = vec![fw("0", 0, 100, 1), fw("1", 200, 300, 1)];
    let hints = vec![hint_human_id(
        WordRef::FinalWordId("w-0".to_string()),
        "remote",
    )];
    let opts = SegmentBuilderOptions {
        complete_channels: Some(vec![ChannelProfile::DirectMic, ChannelProfile::RemoteParty]),
        ..Default::default()
    };
    let result = build_segments(&finals, &[], &hints, Some(&opts));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].key.speaker_human_id.as_deref(), Some("remote"));
}

#[test]
fn partial_word_ignores_its_own_runtime_hint_and_keeps_previous_segment_key() {
    let finals = vec![fw("0", 0, 100, 0)];
    let partials = vec![pw("1", 150, 250, 0)];
    let hints = vec![hint_idx("w-0", 0), hint_runtime_idx(1, 1)];
    let result = build_segments(&finals, &partials, &hints, None);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].key, key_speaker(0, 0));
    assert_eq!(texts(&result[0]), vec!["0", "1"]);
}
