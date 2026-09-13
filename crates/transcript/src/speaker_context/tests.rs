use super::*;
use crate::{
    RenderTranscriptInput, RenderTranscriptRequest, RenderTranscriptWordInput,
    render_transcript_segments,
};

fn context() -> SpeakerContext {
    SpeakerContext {
        intervals: vec![SpeakerContextInterval {
            start_ms: 1000,
            end_ms: 10000,
            active_call: true,
            calendar_call: false,
            mic_isolated: Some(true),
            shared_microphone: false,
            title: "지헌 🤝 덕행".into(),
            self_names: vec!["지헌".into()],
            participants: vec![],
        }],
    }
}

fn request(context: SpeakerContext, speakers: &[(i32, i32)]) -> RenderTranscriptRequest {
    RenderTranscriptRequest {
        speaker_context: Some(context),
        preview: None,
        participant_human_ids: vec![],
        self_human_id: Some("self".into()),
        humans: vec![RenderTranscriptHuman {
            human_id: "self".into(),
            name: "John".into(),
        }],
        transcripts: vec![RenderTranscriptInput {
            started_at: Some(1000),
            assignments: vec![],
            words: speakers
                .iter()
                .enumerate()
                .map(|(i, (channel, speaker))| RenderTranscriptWordInput {
                    id: i.to_string(),
                    text: format!("word{i} "),
                    start_ms: i as i64 * 1000,
                    end_ms: i as i64 * 1000 + 500,
                    channel: *channel,
                    speaker_index: Some(*speaker),
                })
                .collect(),
        }],
    }
}

#[test]
fn no_attendees_call_names_self_and_title_candidate_without_creating_identities() {
    let segments = render_transcript_segments(request(context(), &[(0, 0), (1, 1)]));
    assert_eq!(
        segments
            .iter()
            .map(|s| s.speaker_label.as_str())
            .collect::<Vec<_>>(),
        ["John", "덕행"]
    );
    assert!(segments.iter().all(|s| s.key.speaker_human_id.is_none()));
    assert_eq!(
        segments[1].provisional_speaker.as_ref().unwrap().human_id,
        None
    );
}

#[test]
fn unknown_title_leaves_remote_numbered_from_one() {
    let mut context = context();
    context.intervals[0].self_names.clear();
    let segments = render_transcript_segments(request(context, &[(0, 0), (1, 1)]));
    assert_eq!(segments[0].speaker_label, "John");
    assert_eq!(segments[1].speaker_label, "Speaker 1");
}

#[test]
fn distinct_remote_voices_are_not_capped_by_one_invitee() {
    let mut context = context();
    context.intervals[0]
        .participants
        .push(RenderTranscriptHuman {
            human_id: "remote".into(),
            name: "Alex".into(),
        });
    let mut req = request(context, &[(1, 0), (1, 1), (1, 2)]);
    req.participant_human_ids = vec!["remote".into()];
    let segments = render_transcript_segments(req);
    assert_eq!(segments.len(), 3);
    assert_eq!(
        segments
            .iter()
            .map(|s| s.speaker_label.as_str())
            .collect::<Vec<_>>(),
        ["Speaker 1", "Speaker 2", "Speaker 3"]
    );
}

#[test]
fn sole_remote_participant_wins_over_title() {
    let mut context = context();
    context.intervals[0]
        .participants
        .push(RenderTranscriptHuman {
            human_id: "remote".into(),
            name: "Alex".into(),
        });
    let segments = render_transcript_segments(request(context, &[(1, 8)]));
    assert_eq!(segments[0].speaker_label, "Alex");
    assert_eq!(
        segments[0].provisional_speaker.as_ref().unwrap().reason,
        SpeakerResolutionReason::SoleRemoteParticipant
    );
}

#[test]
fn shared_or_multiple_local_voices_remain_anonymous() {
    let mut shared = context();
    shared.intervals[0].shared_microphone = true;
    assert_eq!(
        render_transcript_segments(request(shared, &[(0, 0)]))[0].speaker_label,
        "Speaker 1"
    );
    let segments = render_transcript_segments(request(context(), &[(0, 0), (0, 1)]));
    assert_eq!(segments.len(), 2);
    assert!(segments.iter().all(|s| s.provisional_speaker.is_none()));
}

#[test]
fn mixed_audio_and_missing_observations_never_use_current_devices() {
    assert!(
        render_transcript_segments(request(context(), &[(2, 0), (2, 1)]))
            .iter()
            .all(|s| s.provisional_speaker.is_none())
    );
    assert_eq!(
        render_transcript_segments(request(SpeakerContext::default(), &[(0, 0)]))[0].speaker_label,
        "Speaker 1"
    );
}

#[test]
fn room_capture_without_active_call_stays_unknown() {
    let mut context = context();
    context.intervals[0].active_call = false;
    context.intervals[0].mic_isolated = Some(false);
    assert_eq!(
        render_transcript_segments(request(context, &[(0, 0)]))[0].speaker_label,
        "Speaker 1"
    );
}

#[test]
fn changing_devices_splits_the_interval_without_renaming_earlier_words() {
    let mut context = context();
    context.intervals[0].end_ms = 2000;
    let mut next = context.intervals[0].clone();
    next.start_ms = 2000;
    next.end_ms = 10000;
    next.shared_microphone = true;
    context.intervals.push(next);
    let segments = render_transcript_segments(request(context, &[(0, 0), (0, 0)]));
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].speaker_label, "John");
    assert_eq!(segments[1].speaker_label, "Speaker 1");
}

#[test]
fn expired_or_conflicting_observations_fail_closed() {
    let mut context = context();
    context.intervals.push(context.intervals[0].clone());
    assert!(
        render_transcript_segments(request(context, &[(0, 0)]))[0]
            .provisional_speaker
            .is_none()
    );
    let mut context = super::tests::context();
    context.intervals[0].end_ms = 1100;
    assert!(
        render_transcript_segments(request(context, &[(0, 0)]))[0]
            .provisional_speaker
            .is_none()
    );
}

#[test]
fn explicit_correction_survives_serialization_and_diarization_changes() {
    let mut req = request(context(), &[(0, 7)]);
    req.humans.push(RenderTranscriptHuman {
        human_id: "corrected".into(),
        name: "Alex".into(),
    });
    req.transcripts[0]
        .assignments
        .push(crate::IdentityAssignment {
            human_id: "corrected".into(),
            scope: crate::IdentityScope::Words {
                word_ids: vec!["0".into()],
            },
        });
    let json = serde_json::to_string(&req).unwrap();
    let mut restored: RenderTranscriptRequest = serde_json::from_str(&json).unwrap();
    restored.transcripts[0].words[0].speaker_index = Some(42);
    let segments = render_transcript_segments(restored);
    assert_eq!(segments[0].speaker_label, "Alex");
    assert!(segments[0].provisional_speaker.is_none());
}

#[test]
fn title_candidates_require_exact_known_self_and_two_names() {
    for title in [
        "John x Alex",
        "[John 🤝 Alex]",
        "John / Alex",
        "(Alex × John)",
        "John <> Alex",
        "[John] [Alex]",
        "(John)(Alex)",
    ] {
        assert_eq!(
            remote_name_from_title(title, &["John".into()]),
            Some("Alex".into())
        );
    }
    assert_eq!(
        remote_name_from_title("덕행 지헌", &["지헌".into()]),
        Some("덕행".into())
    );
    for title in [
        "Product planning",
        "John x Alex x Sam",
        "John x John",
        "[Interview]",
        "덕행 지헌",
    ] {
        assert_eq!(remote_name_from_title(title, &["John".into()]), None);
    }
}

#[test]
fn channel_defaults_do_not_override_specific_corrections() {
    let mut req = request(context(), &[(0, 0), (0, 1), (0, 2)]);
    for id in ["channel", "speaker", "word"] {
        req.humans.push(RenderTranscriptHuman {
            human_id: id.into(),
            name: id.into(),
        });
    }
    req.transcripts[0].assignments = vec![
        crate::IdentityAssignment {
            human_id: "channel".into(),
            scope: crate::IdentityScope::Channel {
                channel: ChannelProfile::DirectMic,
            },
        },
        crate::IdentityAssignment {
            human_id: "speaker".into(),
            scope: crate::IdentityScope::ChannelSpeaker {
                channel: ChannelProfile::DirectMic,
                speaker_index: 1,
            },
        },
        crate::IdentityAssignment {
            human_id: "word".into(),
            scope: crate::IdentityScope::Words {
                word_ids: vec!["2".into()],
            },
        },
    ];
    let segments = render_transcript_segments(req);
    assert_eq!(
        segments
            .iter()
            .map(|s| s.speaker_label.as_str())
            .collect::<Vec<_>>(),
        ["channel", "speaker", "word"]
    );
    assert!(segments.iter().all(|s| s.provisional_speaker.is_none()));
}

#[test]
fn live_preview_and_saved_render_resolve_the_same_names() {
    let req = request(context(), &[(0, 0), (1, 1)]);
    let saved = render_transcript_segments(req.clone());
    let mut preview = req;
    preview.preview = Some(saved.clone());
    for transcript in &mut preview.transcripts {
        transcript.words.clear();
        transcript.assignments.clear();
    }
    let live = render_transcript_segments(preview);
    assert_eq!(
        serde_json::to_value(saved).unwrap(),
        serde_json::to_value(live).unwrap()
    );
}
