use crate::types::{
    ChannelProfile, IdentityAssignment, IdentityScope, Segment, SegmentBuilderOptions, SegmentKey,
};
use crate::types::{FinalizedWord, PartialWord, WordState};

use super::build_segments;

fn fw(text: &str, start: i64, end: i64, ch: i32) -> FinalizedWord {
    FinalizedWord {
        id: format!("w-{text}"),
        text: text.to_string(),
        start_ms: start,
        end_ms: end,
        channel: ch,
        state: WordState::Final,
        speaker_index: None,
    }
}

fn fw_si(text: &str, start: i64, end: i64, ch: i32, si: i32) -> FinalizedWord {
    FinalizedWord {
        id: format!("w-{text}"),
        text: text.to_string(),
        start_ms: start,
        end_ms: end,
        channel: ch,
        state: WordState::Final,
        speaker_index: Some(si),
    }
}

fn pw(text: &str, start: i64, end: i64, ch: i32) -> PartialWord {
    PartialWord {
        text: text.to_string(),
        start_ms: start,
        end_ms: end,
        channel: ch,
        speaker_index: None,
    }
}

fn pw_si(text: &str, start: i64, end: i64, ch: i32, si: i32) -> PartialWord {
    PartialWord {
        text: text.to_string(),
        start_ms: start,
        end_ms: end,
        channel: ch,
        speaker_index: Some(si),
    }
}

fn channel_human(human_id: &str, ch: ChannelProfile) -> IdentityAssignment {
    IdentityAssignment {
        human_id: human_id.to_string(),
        scope: IdentityScope::Channel { channel: ch },
    }
}

fn speaker_human(human_id: &str, ch: ChannelProfile, si: i32) -> IdentityAssignment {
    IdentityAssignment {
        human_id: human_id.to_string(),
        scope: IdentityScope::ChannelSpeaker {
            channel: ch,
            speaker_index: si,
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
        min_segment_words: Some(0),
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
        fw_si("0", 0, 100, 0, 0),
        fw_si("1", 150, 250, 0, 1),
        fw_si("2", 300, 400, 0, 0),
    ];
    let result = build_segments(&finals, &[], &[], None);
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
    let finals = vec![fw_si("0", 0, 100, 0, 1), fw_si("1", 200, 300, 0, 1)];
    let assignments = vec![speaker_human("alice", ChannelProfile::DirectMic, 1)];
    let result = build_segments(&finals, &[], &assignments, None);
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
fn does_not_leak_human_id_across_channels_with_same_speaker_index() {
    let finals = vec![
        fw_si("0", 0, 100, 0, 0),
        fw_si("1", 200, 300, 1, 0),
        fw_si("2", 400, 500, 0, 0),
    ];
    let assignments = vec![
        speaker_human("john", ChannelProfile::DirectMic, 0),
        speaker_human("janet", ChannelProfile::RemoteParty, 0),
    ];
    let result = build_segments(&finals, &[], &assignments, None);

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].key.speaker_human_id.as_deref(), Some("john"));
    assert_eq!(result[1].key.speaker_human_id.as_deref(), Some("janet"));
    assert_eq!(result[2].key.speaker_human_id.as_deref(), Some("john"));
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
    let finals = vec![fw_si("0", 0, 100, 0, 0)];
    let partials = vec![pw("1", 150, 250, 0)];
    let result = build_segments(&finals, &partials, &[], None);
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
    let finals = vec![fw_si("0", 0, 100, 0, 0), fw_si("1", 150, 250, 1, 1)];
    let partials = vec![pw("2", 300, 400, 0), pw("3", 450, 550, 1)];
    let result = build_segments(&finals, &partials, &[], None);
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
        fw_si("0", 0, 100, 0, 0),
        fw_si("1", 100, 200, 1, 1),
        fw_si("2", 200, 300, 0, 0),
    ];
    let result = build_segments(&finals, &[], &[], None);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].key, key_speaker(0, 0));
    assert_eq!(result[1].key, key_speaker(1, 1));
    assert_eq!(result[2].key, key_speaker(0, 0));
}

#[test]
fn handles_partial_only_stream_with_speaker_and_assignment() {
    let partials = vec![pw_si("0", 0, 80, 0, 3), pw("1", 120, 200, 0)];
    let assignments = vec![speaker_human("alice", ChannelProfile::DirectMic, 3)];
    let result = build_segments(&[], &partials, &assignments, None);
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
    let assignments = vec![channel_human("carol", ChannelProfile::DirectMic)];
    let result = build_segments(&finals, &[], &assignments, None);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].key.speaker_human_id.as_deref(), Some("carol"));
    assert_eq!(result[1].key, key(1));
    assert_eq!(result[2].key.speaker_human_id.as_deref(), Some("carol"));
}

#[test]
fn propagates_remote_party_identity_when_channel_marked_complete() {
    let finals = vec![fw("0", 0, 100, 1), fw("1", 200, 300, 1)];
    let assignments = vec![channel_human("remote", ChannelProfile::RemoteParty)];
    let opts = SegmentBuilderOptions {
        complete_channels: Some(vec![ChannelProfile::DirectMic, ChannelProfile::RemoteParty]),
        ..Default::default()
    };
    let result = build_segments(&finals, &[], &assignments, Some(&opts));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].key.speaker_human_id.as_deref(), Some("remote"));
}

#[test]
fn partial_word_ignores_its_own_runtime_hint_and_keeps_previous_segment_key() {
    let finals = vec![fw_si("0", 0, 100, 0, 0)];
    let partials = vec![pw_si("1", 150, 250, 0, 1)];
    let result = build_segments(&finals, &partials, &[], None);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].key, key_speaker(0, 0));
    assert_eq!(texts(&result[0]), vec!["0", "1"]);
}

#[test]
fn consolidates_rapid_crosstalk_micro_segments() {
    let finals = vec![
        fw("alright", 78000, 84000, 1),
        fw("mean", 84000, 84500, 0),
        fw("but", 85000, 85200, 1),
        fw("look", 85200, 85400, 0),
        fw("yeah", 85400, 85500, 1),
        fw("everyone", 85500, 86000, 0),
        fw("knows", 86000, 86500, 0),
        fw("the", 86500, 87000, 0),
        fw("truth", 87000, 105000, 0),
    ];
    let opts = SegmentBuilderOptions::default();
    let result = build_segments(&finals, &[], &[], Some(&opts));
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].key, key(1));
    assert_eq!(texts(&result[0]), vec!["alright", "but", "yeah"]);
    assert_eq!(result[1].key, key(0));
    assert_eq!(
        texts(&result[1]),
        vec!["mean", "look", "everyone", "knows", "the", "truth"]
    );
}

#[test]
fn no_consolidation_when_segment_duration_exceeds_threshold() {
    let finals = vec![
        fw("hello", 0, 2500, 0),
        fw("ok", 3000, 3200, 1),
        fw("world", 3300, 5800, 0),
    ];
    let opts = SegmentBuilderOptions::default();
    let result = build_segments(&finals, &[], &[], Some(&opts));
    assert_eq!(result.len(), 3);
}

#[test]
fn no_consolidation_when_disabled() {
    let finals = vec![
        fw("a", 0, 100, 0),
        fw("b", 150, 200, 1),
        fw("c", 250, 300, 0),
    ];
    let opts = SegmentBuilderOptions {
        min_segment_words: Some(0),
        ..Default::default()
    };
    let result = build_segments(&finals, &[], &[], Some(&opts));
    assert_eq!(result.len(), 3);
}

#[test]
fn micro_segment_not_absorbed_across_long_segment() {
    let finals = vec![
        fw("hi", 0, 100, 0),
        fw("this", 200, 300, 1),
        fw("is", 300, 400, 1),
        fw("a", 400, 500, 1),
        fw("long", 500, 600, 1),
        fw("turn", 600, 3000, 1),
        fw("ok", 3100, 3200, 0),
    ];
    let opts = SegmentBuilderOptions::default();
    let result = build_segments(&finals, &[], &[], Some(&opts));
    assert_eq!(result.len(), 3);
    assert_eq!(texts(&result[0]), vec!["hi"]);
    assert_eq!(texts(&result[2]), vec!["ok"]);
}
