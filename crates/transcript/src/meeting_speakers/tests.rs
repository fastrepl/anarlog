use std::collections::BTreeMap;

use anlg_audio::meeting::{CaptureIdentity, ObservedParticipant, SpeakingCue};

use super::*;
use crate::{
    ChannelProfile, FinalizedWord, IdentityAssignment, IdentityScope, RenderTranscriptHuman,
    RenderTranscriptInput, RenderTranscriptRequest, RenderTranscriptWordInput,
    RenderedTranscriptSegment, SpeakerContext, SpeakerContextInterval, SpeakerResolutionReason,
    StoredLiveTranscriptDelta, StoredSpeakerHint, WordState, materialize_live_transcript,
    render_input_from_stored, render_transcript_segments,
};

const CAPTURE: &str = "capture-1";
/// The epoch ms of the capture's zero, which is its transcript's `started_at_ms`.
const T0: i64 = 1_800_000_000_000;
const LAG: i64 = MEET_CUE_DELAY_MS;

fn instant(ms: i64) -> CaptureInstant {
    CaptureInstant::from_millis(ms as f64).unwrap()
}

fn row(id: &str, name: &str, is_self: Option<bool>) -> ObservedParticipant {
    ObservedParticipant {
        participant_id: id.into(),
        display_name: Some(name.into()),
        is_self,
    }
}

fn me() -> ObservedParticipant {
    row("p-me", "John Jeong", Some(true))
}
fn alice() -> ObservedParticipant {
    row("p-alice", "Alice Kim", Some(false))
}
fn bob() -> ObservedParticipant {
    row("p-bob", "Bob Lee", Some(false))
}
fn dana() -> ObservedParticipant {
    row("p-dana", "Dana Park", Some(false))
}

fn three_way(_: i64) -> (Vec<ObservedParticipant>, Option<u32>) {
    (vec![me(), alice(), bob()], Some(3))
}

fn observation(
    source: ObservationSource,
    platform: CallPlatform,
    ms: i64,
    (participants, count): (Vec<ObservedParticipant>, Option<u32>),
    roster_complete: bool,
    speaking: Option<Vec<SpeakingCue>>,
) -> SpeakerObservation {
    SpeakerObservation {
        capture: CaptureIdentity {
            account_id: "account".into(),
            session_id: "session".into(),
            capture_id: CAPTURE.into(),
        },
        source,
        platform,
        at: instant(ms),
        participants,
        roster_complete,
        participant_count: count,
        speaking,
    }
}

/// A reader heartbeating its roster every 5 s until `heard_until`, plus a speaking cue `LAG` after
/// each turn starts (`None`: the app shows nobody). The capture ends at `ends_at`.
fn call(
    source: ObservationSource,
    platform: CallPlatform,
    roster_at: impl Fn(i64) -> (Vec<ObservedParticipant>, Option<u32>),
    turns: &[(i64, Option<ObservedParticipant>)],
    heard_until: i64,
    ends_at: Option<i64>,
) -> MeetingSpeakerEvidence {
    let mut evidence = MeetingSpeakerEvidence::new(CAPTURE, T0);
    let mut events = (0..=heard_until)
        .step_by(5_000)
        .map(|ms| (ms, None))
        .collect::<Vec<_>>();
    events.extend(turns.iter().map(|(ms, who)| (ms + LAG, Some(who.clone()))));
    events.sort_by_key(|(ms, _)| *ms);
    for (ms, turn) in events.into_iter().filter(|(ms, _)| *ms <= heard_until) {
        let speaking = turn.map(|who| {
            who.into_iter()
                .map(|who| SpeakingCue {
                    participant_id: Some(who.participant_id),
                    display_name: who.display_name,
                })
                .collect()
        });
        evidence.observe(&observation(
            source,
            platform,
            ms,
            roster_at(ms),
            false,
            speaking,
        ));
    }
    if let Some(end) = ends_at {
        evidence.close(instant(end));
    }
    evidence
}

fn meet(
    roster_at: impl Fn(i64) -> (Vec<ObservedParticipant>, Option<u32>),
    turns: &[(i64, Option<ObservedParticipant>)],
    until: i64,
) -> MeetingSpeakerEvidence {
    call(
        ObservationSource::BrowserExtension,
        CallPlatform::GoogleMeet,
        roster_at,
        turns,
        until,
        Some(until),
    )
}

/// Words of 300 ms with 100 ms between them, in one voice, from `from` to `to`.
fn speech(voice: i32, from: i64, to: i64) -> Vec<VoiceWord> {
    (from..)
        .step_by(400)
        .take_while(|ms| ms + 300 <= to)
        .map(|ms| VoiceWord {
            voice: VoiceKey {
                generation: 0,
                speaker_index: Some(voice),
            },
            start_ms: ms,
            end_ms: ms + 300,
        })
        .collect()
}

fn named(evidence: &MeetingSpeakerEvidence, words: &[VoiceWord]) -> BTreeMap<i32, String> {
    evidence
        .name_voices(words)
        .into_iter()
        .map(|(voice, named)| (voice.speaker_index.unwrap(), named.name))
        .collect()
}

fn names(entries: &[(i32, &str)]) -> BTreeMap<i32, String> {
    entries
        .iter()
        .map(|(voice, name)| (*voice, name.to_string()))
        .collect()
}

#[test]
fn a_one_on_one_roster_names_every_voice_of_the_other_person_only_once_it_held() {
    let solo = |_| (vec![me(), dana()], Some(2));
    // The diarizer heard Dana as two voices; both are her.
    let words = [speech(1, 2_000, 20_000), speech(2, 30_000, 31_000)].concat();
    assert_eq!(
        named(&meet(solo, &[], 60_000), &words),
        names(&[(1, "Dana Park"), (2, "Dana Park")])
    );
    // Too short to be more than someone joining, or the app's count says more people.
    assert!(named(&meet(solo, &[], 10_000), &speech(1, 0, 10_000)).is_empty());
    let crowded = |_| (vec![me(), dana()], Some(6));
    assert!(named(&meet(crowded, &[], 60_000), &speech(1, 0, 60_000)).is_empty());
}

#[test]
fn meet_cues_name_quick_turns_once_its_lag_is_taken_off() {
    let mut words = Vec::new();
    let mut turns = Vec::new();
    for turn in 0..40 {
        let (voice, who) = if turn % 2 == 0 {
            (1, alice())
        } else {
            (2, bob())
        };
        words.extend(speech(voice, turn * 1_000, turn * 1_000 + 1_000));
        turns.push((turn * 1_000, Some(who)));
    }
    assert_eq!(
        named(&meet(three_way, &turns, 41_000), &words),
        names(&[(1, "Alice Kim"), (2, "Bob Lee")])
    );
}

#[test]
fn backchannels_and_overlapping_speech_do_not_flip_a_voice() {
    let mut words = [speech(1, 0, 30_000), speech(2, 30_000, 45_000)].concat();
    let mut turns = vec![(0, Some(alice()))];
    // Bob says "mm-hm" over Alice three times, and Meet flips to him for a second each time.
    for at in [5_000, 12_000, 20_000] {
        words.extend(speech(2, at, at + 400));
        turns.extend([(at, Some(bob())), (at + 1_000, Some(alice()))]);
    }
    turns.push((30_000, Some(bob())));
    // Both talk for five seconds while Meet shows Bob; Alice says "right" twice in Bob's turn.
    words.extend([speech(1, 45_000, 50_000), speech(2, 45_000, 50_000)].concat());
    words.extend([speech(1, 33_000, 33_300), speech(1, 38_000, 38_300)].concat());
    assert_eq!(
        named(&meet(three_way, &turns, 50_000), &words),
        names(&[(1, "Alice Kim"), (2, "Bob Lee")])
    );
}

#[test]
fn a_voice_without_a_clear_leader_or_enough_cued_speech_stays_unnamed() {
    let turns = [(0, Some(alice())), (10_000, Some(bob())), (20_000, None)];
    let words = [
        // Half over Alice, half over Bob: the diarizer merged them.
        speech(1, 0, 20_000),
        // Clearly Alice's for a few seconds, but mostly heard while Meet showed nobody speaking.
        speech(2, 7_000, 10_000),
        speech(2, 25_000, 60_000),
        speech(2, 19_000, 19_800),
    ]
    .concat();
    assert!(named(&meet(three_way, &turns, 60_000), &words).is_empty());
}

#[test]
fn both_halves_of_a_person_the_diarizer_split_get_that_one_name() {
    let turns = [
        (0, Some(alice())),
        (20_000, Some(bob())),
        (30_000, Some(alice())),
    ];
    let words = [
        speech(1, 0, 20_000),
        speech(2, 20_000, 30_000),
        speech(3, 30_000, 50_000),
    ]
    .concat();
    assert_eq!(
        named(&meet(three_way, &turns, 50_000), &words),
        names(&[(1, "Alice Kim"), (2, "Bob Lee"), (3, "Alice Kim")])
    );
}

#[test]
fn the_local_users_own_tile_never_names_a_call_voice() {
    let evidence = meet(three_way, &[(0, Some(me()))], 30_000);
    assert!(named(&evidence, &speech(1, 0, 30_000)).is_empty());
}

#[test]
fn a_one_on_one_and_later_cues_must_agree_on_a_merged_voice() {
    let roster = |ms: i64| {
        if ms < 40_000 {
            (vec![me(), dana()], Some(2))
        } else {
            (vec![me(), dana(), bob()], Some(3))
        }
    };
    let words = [
        speech(1, 5_000, 35_000),
        speech(2, 45_000, 60_000),
        speech(1, 60_000, 80_000),
    ]
    .concat();
    let evidence = meet(
        roster,
        &[(45_000, Some(bob())), (60_000, Some(dana()))],
        80_000,
    );
    assert_eq!(
        named(&evidence, &words),
        names(&[(1, "Dana Park"), (2, "Bob Lee")])
    );

    // The diarizer heard Dana in the 1:1 and Bob after he joined as one voice. Meet also lit
    // Dana in the 1:1; that speech already voted through the roster and must not vote twice.
    let merged = [speech(1, 5_000, 25_000), speech(1, 45_000, 65_000)].concat();
    let disagreeing = meet(
        roster,
        &[(5_000, Some(dana())), (45_000, Some(bob())), (65_000, None)],
        80_000,
    );
    assert!(named(&disagreeing, &merged).is_empty());
    let agreeing = meet(roster, &[(45_000, Some(dana())), (65_000, None)], 80_000);
    assert_eq!(named(&agreeing, &merged), names(&[(1, "Dana Park")]));
}

#[test]
fn nothing_holds_after_the_reader_was_last_heard_from() {
    // The Meet tab stopped answering right after showing Alice; the capture went on.
    let evidence = call(
        ObservationSource::BrowserExtension,
        CallPlatform::GoogleMeet,
        three_way,
        &[(0, Some(alice()))],
        1_000,
        Some(30_000),
    );
    assert!(named(&evidence, &speech(1, 5_000, 30_000)).is_empty());

    // It went quiet for twenty seconds and came back without a new cue: Alice's tile from before
    // says nothing about the silence.
    let mut evidence = MeetingSpeakerEvidence::new(CAPTURE, T0);
    let report = |ms, speaking| {
        observation(
            ObservationSource::BrowserExtension,
            CallPlatform::GoogleMeet,
            ms,
            three_way(ms),
            false,
            speaking,
        )
    };
    let lit = SpeakingCue {
        participant_id: Some("p-alice".into()),
        display_name: None,
    };
    evidence.observe(&report(LAG, Some(vec![lit])));
    for ms in [25_000, 30_000] {
        evidence.observe(&report(ms, None));
    }
    evidence.close(instant(30_000));
    assert!(named(&evidence, &speech(1, 5_000, 20_000)).is_empty());
}

#[test]
fn only_meets_extension_cues_count_as_speaking_signals() {
    let turns = [(0, Some(alice())), (20_000, Some(bob()))];
    let words = [speech(1, 0, 20_000), speech(2, 20_000, 40_000)].concat();
    assert_eq!(named(&meet(three_way, &turns, 40_000), &words).len(), 2);
    for source in [
        ObservationSource::Accessibility,
        ObservationSource::BrowserExtension,
    ] {
        for platform in [
            CallPlatform::Zoom,
            CallPlatform::MicrosoftTeams,
            CallPlatform::Webex,
        ] {
            let evidence = call(source, platform, three_way, &turns, 40_000, Some(40_000));
            assert!(
                named(&evidence, &words).is_empty(),
                "{source:?} {platform:?}"
            );
        }
    }
    let read_by_accessibility = call(
        ObservationSource::Accessibility,
        CallPlatform::GoogleMeet,
        three_way,
        &turns,
        40_000,
        Some(40_000),
    );
    assert!(named(&read_by_accessibility, &words).is_empty());
}

#[test]
fn a_name_needs_more_speech_while_the_capture_still_records() {
    let words = speech(1, 0, 4_000);
    let open = call(
        ObservationSource::BrowserExtension,
        CallPlatform::GoogleMeet,
        three_way,
        &[(0, Some(alice()))],
        5_000,
        None,
    );
    assert!(named(&open, &words).is_empty());
    let mut closed = open.clone();
    closed.close(instant(5_000));
    assert_eq!(named(&closed, &words), names(&[(1, "Alice Kim")]));
}

fn webex_me() -> ObservedParticipant {
    row("w/me", "John Jeong", Some(true))
}
fn sam() -> ObservedParticipant {
    row("w/sam", "Sam Park", Some(false))
}

/// A 30-second native call with Sam talking throughout, read once a second; `read(second)` may
/// replace a read with another helper event.
fn webex_call(
    roster: (Vec<ObservedParticipant>, Option<u32>),
    complete: bool,
    platform: CallPlatform,
    read: impl Fn(i64) -> Option<MeetingCaptureEvent>,
) -> BTreeMap<i32, String> {
    let mut evidence = MeetingSpeakerEvidence::new(CAPTURE, T0);
    for second in 0..=30 {
        let ms = second * 1_000;
        let event = read(second).unwrap_or_else(|| {
            MeetingCaptureEvent::Observation(observation(
                ObservationSource::Accessibility,
                platform,
                ms,
                roster.clone(),
                complete,
                None,
            ))
        });
        evidence.apply(&event);
    }
    evidence.apply(&MeetingCaptureEvent::Ended {
        at: instant(30_000),
        reason: anlg_audio::meeting::CaptureEndReason::UserStopped,
    });
    named(&evidence, &speech(1, 0, 30_000))
}

#[test]
fn a_complete_webex_roster_alone_names_a_one_on_one() {
    let solo = (vec![webex_me(), sam()], Some(2));
    let none = |_| None;
    assert_eq!(
        webex_call(solo.clone(), true, CallPlatform::Webex, none),
        names(&[(1, "Sam Park")])
    );
    let twin = row("w/twin", "Sam Park", Some(false));
    for (case, roster, complete, platform) in [
        (
            "no count",
            (vec![webex_me(), sam()], None),
            true,
            CallPlatform::Webex,
        ),
        (
            "count says more",
            (vec![webex_me(), sam()], Some(3)),
            true,
            CallPlatform::Webex,
        ),
        (
            "self unknown",
            (vec![row("w/me", "John Jeong", None), sam()], Some(2)),
            true,
            CallPlatform::Webex,
        ),
        ("panel closed", (vec![], None), false, CallPlatform::Webex),
        (
            "helper says incomplete",
            solo.clone(),
            false,
            CallPlatform::Webex,
        ),
        (
            "two people of one name",
            (vec![webex_me(), sam(), twin], Some(3)),
            true,
            CallPlatform::Webex,
        ),
        ("unqualified app", solo.clone(), true, CallPlatform::Zoom),
        (
            "Meet, read by the extension",
            solo.clone(),
            true,
            CallPlatform::GoogleMeet,
        ),
    ] {
        assert!(
            webex_call(roster, complete, platform, none).is_empty(),
            "{case}"
        );
    }
}

#[test]
fn no_roster_holds_across_a_pause_or_a_lost_read() {
    let solo = (vec![webex_me(), sam()], Some(2));
    // Ten seconds each side: neither stretch is long enough to call a 1:1.
    let pause = |second: i64| match second {
        10 => Some(MeetingCaptureEvent::Paused {
            at: instant(10_000),
        }),
        20 => Some(MeetingCaptureEvent::Resumed {
            at: instant(20_000),
        }),
        _ => None,
    };
    let lost = |second: i64| {
        (10..20)
            .contains(&second)
            .then(|| MeetingCaptureEvent::ObservationUnavailable {
                at: instant(second * 1_000),
                reason: "incomplete_snapshot".into(),
            })
    };
    assert!(webex_call(solo.clone(), true, CallPlatform::Webex, pause).is_empty());
    assert!(webex_call(solo, true, CallPlatform::Webex, lost).is_empty());
}

#[test]
fn meets_extension_track_wins_over_the_helpers_roster() {
    let mut evidence = meet(three_way, &[(0, Some(alice()))], 30_000);
    evidence.ended_at_ms = None;
    for second in 0..=30 {
        evidence.observe(&observation(
            ObservationSource::Accessibility,
            CallPlatform::Webex,
            second * 1_000,
            (vec![webex_me(), sam()], Some(2)),
            true,
            None,
        ));
    }
    evidence.close(instant(30_000));
    let words = speech(1, 0, 30_000);
    assert_eq!(named(&evidence, &words), names(&[(1, "Alice Kim")]));
    evidence.browser = None;
    assert_eq!(named(&evidence, &words), names(&[(1, "Sam Park")]));
}

#[test]
fn a_provider_reconnect_starts_its_voice_indices_over() {
    let turns = [(0, Some(alice())), (20_000, Some(bob()))];
    let mut evidence = meet(three_way, &turns, 40_000);
    let heard = |evidence: &MeetingSpeakerEvidence| {
        let words = [speech(0, 0, 20_000), speech(0, 20_000, 40_000)]
            .concat()
            .into_iter()
            .map(|word| VoiceWord {
                voice: evidence.voice_at(word.start_ms, Some(0)),
                ..word
            })
            .collect::<Vec<_>>();
        let mut names = evidence
            .name_voices(&words)
            .into_iter()
            .map(|(voice, named)| (voice.generation, named.name))
            .collect::<Vec<_>>();
        names.sort();
        names
    };
    // Without the restart, two people's "speaker 0" look like one voice nobody leads.
    assert!(heard(&evidence).is_empty());
    evidence.ended_at_ms = None;
    evidence.provider_restarted(instant(20_000));
    evidence.close(instant(40_000));
    assert_eq!(
        heard(&evidence),
        [(0, "Alice Kim".to_string()), (1, "Bob Lee".to_string())]
    );
}

#[test]
fn reports_for_another_capture_after_the_end_or_while_paused_are_dropped() {
    let report = |capture: &str, ms: i64| SpeakerObservation {
        capture: CaptureIdentity {
            capture_id: capture.into(),
            ..observation(
                ObservationSource::BrowserExtension,
                CallPlatform::GoogleMeet,
                ms,
                three_way(ms),
                false,
                None,
            )
            .capture
        },
        ..observation(
            ObservationSource::BrowserExtension,
            CallPlatform::GoogleMeet,
            ms,
            three_way(ms),
            false,
            Some(vec![]),
        )
    };
    let mut evidence = MeetingSpeakerEvidence::new(CAPTURE, T0);
    evidence.observe(&report("previous-capture", 0));
    evidence.apply(&MeetingCaptureEvent::Paused { at: instant(1_000) });
    evidence.observe(&report(CAPTURE, 2_000));
    assert_eq!(evidence.browser, None);
    evidence.apply(&MeetingCaptureEvent::Resumed { at: instant(3_000) });
    evidence.close(instant(4_000));
    evidence.observe(&report(CAPTURE, 5_000));
    assert_eq!(evidence.browser, None);
}

#[test]
fn stored_evidence_is_upserted_per_capture_and_never_reopened() {
    let open = meet(three_way, &[(0, Some(alice()))], 10_000);
    let mut ended = open.clone();
    let mut reopened = open.clone();
    reopened.ended_at_ms = None;
    ended.ended_at_ms = Some(9_000);
    let other = MeetingSpeakerEvidence::new("capture-0", T0 - 60_000);

    let stored = upsert_meeting_evidence(&Value::Null, &ended).unwrap();
    let stored = upsert_meeting_evidence(&stored, &other).unwrap();
    assert_eq!(upsert_meeting_evidence(&stored, &reopened), None);
    let stored: Vec<MeetingSpeakerEvidence> = serde_json::from_value(stored).unwrap();
    assert_eq!(stored, vec![other, ended]);
}

#[test]
fn malformed_stored_evidence_is_dropped_without_losing_the_rest_of_the_context() {
    let valid = serde_json::to_value(meet(three_way, &[(0, Some(alice()))], 10_000)).unwrap();
    let mut unsorted = valid.clone();
    unsorted["browser"]["cues"] = serde_json::json!([{"at_ms": 900}, {"at_ms": 100}]);
    let mut unknown_participant = valid.clone();
    unknown_participant["browser"]["cues"] = serde_json::json!([{"at_ms": 0, "participant": 99}]);
    let stored = serde_json::json!({
        "intervals": [],
        "meetings": [valid, unsorted, unknown_participant, {"capture_id": 7}, null],
    });
    for context in [
        SpeakerContext::from_stored_value(&stored),
        serde_json::from_value::<SpeakerContext>(stored.clone()).unwrap(),
    ] {
        assert_eq!(context.meetings.len(), 1);
        assert_eq!(context.meetings[0].capture_id, CAPTURE);
    }
}

// --- Rendering against Anarlog's stored words, hints and assignments ---

fn remote(id: &str, start_ms: i64, speaker_index: i32) -> RenderTranscriptWordInput {
    RenderTranscriptWordInput {
        id: id.into(),
        text: format!(" {id}"),
        start_ms,
        end_ms: start_ms + 300,
        channel: 1,
        speaker_index: Some(speaker_index),
        synthetic_timing: None,
    }
}

/// Remote words every 400 ms, ids `<prefix><n>`.
fn remote_run(prefix: &str, speaker: i32, from: i64, to: i64) -> Vec<RenderTranscriptWordInput> {
    (from..)
        .step_by(400)
        .take_while(|ms| ms + 300 <= to)
        .enumerate()
        .map(|(n, ms)| remote(&format!("{prefix}{n}"), ms, speaker))
        .collect()
}

fn render(
    context: SpeakerContext,
    transcripts: Vec<RenderTranscriptInput>,
    humans: &[(&str, &str)],
) -> Vec<RenderedTranscriptSegment> {
    render_transcript_segments(RenderTranscriptRequest {
        speaker_context: Some(context),
        preview: None,
        transcripts,
        participant_human_ids: vec![],
        self_human_id: Some("self".into()),
        humans: humans
            .iter()
            .map(|(human_id, name)| RenderTranscriptHuman {
                human_id: human_id.to_string(),
                name: name.to_string(),
            })
            .collect(),
    })
}

fn with_meetings(meetings: Vec<MeetingSpeakerEvidence>) -> SpeakerContext {
    SpeakerContext {
        intervals: vec![],
        meetings,
    }
}

/// Each word id's label, failing if a word is rendered twice.
fn label_by_word(segments: &[RenderedTranscriptSegment]) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    for segment in segments {
        for word in &segment.words {
            let id = word.id.clone().unwrap();
            assert!(
                labels
                    .insert(id.clone(), segment.speaker_label.clone())
                    .is_none(),
                "{id} rendered twice"
            );
        }
    }
    labels
}

fn finalized(id: &str, text: &str, start_ms: i64, channel: i32, speaker: i32) -> FinalizedWord {
    FinalizedWord {
        id: id.into(),
        text: text.into(),
        start_ms,
        end_ms: start_ms + 300,
        channel,
        state: WordState::Final,
        speaker_index: Some(speaker),
    }
}

#[test]
fn replayed_live_rewrites_and_deletions_keep_one_word_per_id_under_its_voices_name() {
    let alice_turn = [(0, Some(alice())), (20_000, Some(bob()))];
    let evidence = meet(three_way, &alice_turn, 40_000);
    let mut first = (0..50)
        .map(|n| finalized(&format!("a{n}"), " word", n * 400, 1, 0))
        .collect::<Vec<_>>();
    first.extend((0..50).map(|n| finalized(&format!("b{n}"), " word", 20_000 + n * 400, 1, 1)));
    first.push(finalized("m0", " mine", 10_000, 0, 0));
    let deltas = [
        StoredLiveTranscriptDelta {
            new_words: first,
            replaced_ids: vec![],
        },
        // The provider merges a later token into a word it already sent...
        StoredLiveTranscriptDelta {
            new_words: vec![finalized("a3", " wordier", 1_200, 1, 0)],
            replaced_ids: vec!["a3".into()],
        },
        // ...and drops one.
        StoredLiveTranscriptDelta {
            new_words: vec![],
            replaced_ids: vec!["a4".into()],
        },
    ];
    let (words, hints) = materialize_live_transcript(vec![], vec![], &deltas);
    let input = render_input_from_stored(Some(T0), &words, &hints).unwrap();
    let context = with_meetings(vec![evidence]);
    let saved = render(context.clone(), vec![input.clone()], &[]);

    let labels = label_by_word(&saved);
    assert_eq!(
        labels.len(),
        100,
        "one entry per surviving word, no new ids"
    );
    assert!(!labels.contains_key("a4"));
    assert!(labels.keys().all(|id| !id.contains(':')));
    assert!(
        (0..50)
            .filter(|n| *n != 4)
            .all(|n| labels[&format!("a{n}")] == "Alice Kim")
    );
    assert!((0..50).all(|n| labels[&format!("b{n}")] == "Bob Lee"));
    assert_eq!(
        labels["m0"], "Speaker 1",
        "the microphone is never named from the call"
    );
    let rewritten = saved
        .iter()
        .flat_map(|segment| &segment.words)
        .find(|word| word.id.as_deref() == Some("a3"))
        .unwrap();
    assert_eq!(rewritten.text.trim(), "wordier");
    let named = saved
        .iter()
        .find(|s| s.speaker_label == "Alice Kim")
        .unwrap();
    assert_eq!(
        named
            .provisional_speaker
            .as_ref()
            .map(|p| (&p.human_id, &p.reason)),
        Some((&None, &SpeakerResolutionReason::MeetingActiveSpeaker))
    );

    // The live preview names the same voices the same way.
    let preview = render_transcript_segments(RenderTranscriptRequest {
        speaker_context: Some(context),
        preview: Some(saved.clone()),
        transcripts: vec![RenderTranscriptInput {
            words: vec![],
            assignments: vec![],
            ..input
        }],
        participant_human_ids: vec![],
        self_human_id: Some("self".into()),
        humans: vec![],
    });
    assert_eq!(
        serde_json::to_value(&saved).unwrap(),
        serde_json::to_value(&preview).unwrap()
    );
}

#[test]
fn manual_and_voiceprint_assignments_win_over_meeting_names() {
    let evidence = meet(
        three_way,
        &[(0, Some(alice())), (20_000, Some(bob()))],
        40_000,
    );
    let words = [
        remote_run("a", 0, 0, 20_000),
        remote_run("b", 1, 20_000, 40_000),
    ]
    .concat();
    let hint = |hint_type: &str, word_id: &str, value: serde_json::Value| StoredSpeakerHint {
        id: format!("{word_id}:{hint_type}"),
        word_id: Some(word_id.into()),
        hint_type: hint_type.into(),
        value,
    };
    let stored_words = words
        .iter()
        .map(|word| crate::StoredTranscriptWord {
            id: word.id.clone(),
            text: Some(word.text.clone()),
            start_ms: Some(word.start_ms as f64),
            end_ms: Some(word.end_ms as f64),
            channel: Some(1.0),
            speaker: None,
            metadata: None,
        })
        .collect::<Vec<_>>();
    let mut hints = words
        .iter()
        .map(|word| {
            hint(
                "provider_speaker_index",
                &word.id,
                serde_json::json!({"speaker_index": word.speaker_index, "channel": 1}),
            )
        })
        .collect::<Vec<_>>();
    // A manual correction of one of Alice's words, and a consented voiceprint match for Bob.
    hints.push(hint(
        "user_speaker_assignment",
        "a5",
        serde_json::json!({"human_id": "jordan", "scope": "segment", "word_ids": ["a5"]}),
    ));
    hints.push(hint(
        "automatic_speaker_assignment",
        "b0",
        serde_json::json!({"human_id": "bob-person", "source": "voiceprint", "scope": "speaker", "channel": 1, "speaker_index": 1}),
    ));
    let input = render_input_from_stored(Some(T0), &stored_words, &hints).unwrap();
    let segments = render(
        with_meetings(vec![evidence]),
        vec![input],
        &[("jordan", "Jordan"), ("bob-person", "Robert Lee")],
    );
    let labels = label_by_word(&segments);
    assert_eq!(labels["a5"], "Jordan");
    assert_eq!(labels["a6"], "Alice Kim");
    assert!((0..50).all(|n| labels[&format!("b{n}")] == "Robert Lee"));
    assert!(
        segments
            .iter()
            .filter(|s| s.key.speaker_human_id.is_some())
            .all(|s| s.provisional_speaker.is_none())
    );
}

#[test]
fn the_final_pass_corrects_provisional_names_and_names_batch_words_without_a_live_transcript() {
    // Meet showed Alice over the first eight seconds of this voice, then Bob for the rest of it.
    let turns = [(0, Some(alice())), (8_000, Some(bob()))];
    let live = call(
        ObservationSource::BrowserExtension,
        CallPlatform::GoogleMeet,
        three_way,
        &turns,
        9_000,
        None,
    );
    let early = RenderTranscriptInput {
        started_at: Some(T0),
        words: remote_run("w", 0, 0, 8_000),
        assignments: vec![],
    };
    let labels = label_by_word(&render(with_meetings(vec![live]), vec![early], &[]));
    assert!(labels.values().all(|label| label == "Alice Kim"));

    // Live transcription never ran; batch repair later transcribed the capture with its own ids
    // and diarization. The closed evidence names those words at render.
    let ended = meet(three_way, &turns, 40_000);
    let batch = RenderTranscriptInput {
        started_at: Some(T0),
        words: remote_run("batch-", 3, 0, 40_000),
        assignments: vec![],
    };
    let labels = label_by_word(&render(with_meetings(vec![ended]), vec![batch], &[]));
    assert_eq!(labels.len(), 100);
    assert!(labels.values().all(|label| label == "Bob Lee"));
}

#[test]
fn a_later_recording_of_the_session_never_inherits_a_captures_names() {
    // The first capture crashed before its evidence was closed.
    let mut first = meet(three_way, &[(0, Some(alice()))], 20_000);
    first.ended_at_ms = None;
    let second_start = T0 + 120_000;
    let transcripts = vec![
        RenderTranscriptInput {
            started_at: Some(T0),
            words: remote_run("a", 0, 0, 20_000),
            assignments: vec![],
        },
        RenderTranscriptInput {
            started_at: Some(second_start),
            words: remote_run("z", 0, 0, 20_000),
            assignments: vec![],
        },
    ];
    let labels = label_by_word(&render(with_meetings(vec![first]), transcripts, &[]));
    assert_eq!(labels["a0"], "Alice Kim");
    assert!((0..50).all(|n| labels[&format!("z{n}")] != "Alice Kim"));
}

#[test]
fn an_observed_group_call_overrides_the_calendars_sole_invitee() {
    let interval = SpeakerContextInterval {
        start_ms: T0,
        end_ms: T0 + 60_000,
        active_call: true,
        calendar_call: false,
        mic_isolated: Some(true),
        shared_microphone: false,
        title: String::new(),
        self_names: vec![],
        participants: vec![RenderTranscriptHuman {
            human_id: "dana-person".into(),
            name: "Dana Park".into(),
        }],
    };
    let transcript = || RenderTranscriptInput {
        started_at: Some(T0),
        words: remote_run("w", 0, 0, 30_000),
        assignments: vec![],
    };
    let humans = [("dana-person", "Dana Park")];
    let render_with = |meetings| {
        render(
            SpeakerContext {
                intervals: vec![interval.clone()],
                meetings,
            },
            vec![transcript()],
            &humans,
        )
    };

    let calendar_only = render_with(vec![]);
    assert_eq!(calendar_only[0].speaker_label, "Dana Park");

    // Meet shows three other people and nobody lit: the invite list was not the call.
    let group = |_| (vec![me(), alice(), bob(), dana()], Some(4));
    let segments = render_with(vec![meet(group, &[], 30_000)]);
    assert!(segments.iter().all(|s| s.provisional_speaker.is_none()));

    // A 1:1 with the invitee keeps the invitee's provisional link.
    let solo = |_| (vec![me(), dana()], Some(2));
    let segments = render_with(vec![meet(solo, &[], 30_000)]);
    let label = segments[0].provisional_speaker.as_ref().unwrap();
    assert_eq!(label.human_id.as_deref(), Some("dana-person"));
    assert!(segments.iter().all(|s| s.speaker_label == "Dana Park"));
    assert!(
        segments
            .iter()
            .all(|s| s.key.channel == ChannelProfile::RemoteParty
                && s.key.speaker_human_id.is_none())
    );
}

#[test]
fn identity_assignment_types_stay_unchanged_by_meeting_names() {
    // Meeting names never become identity assignments: the render input's assignments are only
    // what the user and voiceprints recorded.
    let input = RenderTranscriptInput {
        started_at: Some(T0),
        words: remote_run("w", 0, 0, 20_000),
        assignments: vec![IdentityAssignment {
            human_id: "self".into(),
            scope: IdentityScope::Channel {
                channel: ChannelProfile::DirectMic,
            },
        }],
    };
    let segments = render(
        with_meetings(vec![meet(three_way, &[(0, Some(alice()))], 20_000)]),
        vec![input],
        &[],
    );
    assert!(segments.iter().all(|s| s.key.speaker_human_id.is_none()));
}
