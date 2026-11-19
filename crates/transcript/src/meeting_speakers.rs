//! What the meeting app showed about who was in the call and who was speaking, kept per capture,
//! and the names that evidence gives the call's voices.
//!
//! A reader of the meeting app (the Meet extension, or the capture helper's Accessibility reads of
//! Zoom, Teams and Webex) reports the roster it sees and, for Meet only, the participant it shows
//! speaking. A voice the provider diarized is named as a whole, by a vote over all of its speech,
//! never word by word, so a backchannel or a late indicator cannot flip a sentence. These names are
//! session evidence only: they carry no human id, and never enroll a voiceprint or create or merge
//! People. Explicit assignments (manual corrections and consented voiceprint identities) are
//! resolved before this runs and always win.
//!
//! Evidence times are capture milliseconds (`CaptureInstant::as_millis`), which share the clock of
//! the capture transcript's word offsets; `started_at_ms` is that transcript's `started_at_ms`.
//!
//! Provenance (LICENSING.md, third-party provenance): behavior and thresholds follow Char
//! (github.com/fastrepl/char at d4608723d8ac328ab91091803a08c4585a0dc244,
//! `char/apps/desktop/src/main/native-meeting/speaker-naming.ts` and `meeting-speakers.ts`;
//! Fastrepl, Inc.; GPL-3.0), used only as a behavioral reference. This module was written for
//! Anarlog's word and identity model; no Char source was copied.

use std::collections::{BTreeMap, HashMap};

use anlg_audio::meeting::{
    CallPlatform, CaptureInstant, MeetingCaptureEvent, ObservationSource, SpeakerObservation,
};
use serde_json::Value;

/// How far Meet's speaking outline trails the voice. Other apps have no speaking signal anyone has
/// read, so only Meet's cues are used.
const MEET_CUE_DELAY_MS: i64 = 500;
/// A voice is named only when one participant holds this share of its voted speech...
const NAME_SHARE: f64 = 0.6;
/// ...with at least this much voted speech once the capture is over...
const FINAL_MIN_VOTED_SPEECH_MS: i64 = 2_000;
/// ...or this much while it still records: a wrong name on screen is worse than none.
const PROVISIONAL_MIN_VOTED_SPEECH_MS: i64 = 6_000;
/// ...and the votes cover at least this share of everything the voice said.
const MIN_VOTED_COVERAGE: f64 = 0.25;
/// A roster showing one other person, unchanged this long, is a 1:1. Shorter is a join or leave.
const SOLO_ROSTER_MS: i64 = 15_000;
/// How long after a reader was last heard from its last report still holds.
const HEARD_GRACE_MS: i64 = 2_000;
/// A reader silent this long was lost; it reports at least every 5 s (Meet) or 1 s (helper).
const READER_LEASE_MS: i64 = 12_000;
const MAX_TRACK_EVENTS: usize = 10_000;
const MAX_ROSTER_ROWS: usize = 500;
const MAX_PARTICIPANTS: usize = 2_000;
const MAX_TEXT_CHARS: usize = 200;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[serde(rename_all = "snake_case")]
pub enum MeetingEvidencePlatform {
    GoogleMeet,
    Zoom,
    MicrosoftTeams,
    Webex,
}

impl From<CallPlatform> for MeetingEvidencePlatform {
    fn from(platform: CallPlatform) -> Self {
        match platform {
            CallPlatform::GoogleMeet => Self::GoogleMeet,
            CallPlatform::Zoom => Self::Zoom,
            CallPlatform::MicrosoftTeams => Self::MicrosoftTeams,
            CallPlatform::Webex => Self::Webex,
        }
    }
}

/// Capture time in which nothing a reader reported holds: a pause, a lost read, a silent reader.
/// `end_ms` is absent while it is still open.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MeetingEvidenceGap {
    pub start_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<i64>,
}

/// One participant as the app identified them. Equal display names stay distinct by `id`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MeetingEvidenceParticipant {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "is_false", default)]
    pub is_self: bool,
}

/// Who the app showed from `at_ms` until the next roster (indices into `participants`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MeetingRosterEvidence {
    pub at_ms: i64,
    pub participants: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Every row is present and identified: the app's count matches, each row has a name and says
    /// whether it is the local user, and exactly one is.
    #[serde(skip_serializing_if = "is_false", default)]
    pub complete: bool,
}

/// Whom the app showed speaking from `at_ms` until the next cue; absent means nobody.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MeetingCueEvidence {
    pub at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub participant: Option<u32>,
}

/// What one reader reported, compacted: unchanged rosters and cues are not repeated.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MeetingEvidenceTrack {
    pub platform: MeetingEvidencePlatform,
    #[serde(default)]
    pub participants: Vec<MeetingEvidenceParticipant>,
    #[serde(default)]
    pub rosters: Vec<MeetingRosterEvidence>,
    #[serde(default)]
    pub cues: Vec<MeetingCueEvidence>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub gaps: Vec<MeetingEvidenceGap>,
    pub last_heard_ms: i64,
}

/// One capture's platform evidence. The browser extension's track wins over the helper's.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MeetingSpeakerEvidence {
    pub capture_id: String,
    pub started_at_ms: i64,
    /// Set when the capture ended; names are final from then on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at_ms: Option<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub pauses: Vec<MeetingEvidenceGap>,
    /// Where the live transcription stream restarted; provider voice indices restart there.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub provider_restarts_ms: Vec<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<MeetingEvidenceTrack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<MeetingEvidenceTrack>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn millis(at: CaptureInstant) -> i64 {
    at.as_millis().round() as i64
}

fn clean(text: &str) -> Option<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then(|| text.chars().take(MAX_TEXT_CHARS).collect())
}

fn open_gap_at(gaps: &[MeetingEvidenceGap]) -> bool {
    gaps.last().is_some_and(|gap| gap.end_ms.is_none())
}

impl MeetingSpeakerEvidence {
    pub fn new(capture_id: impl Into<String>, started_at_ms: i64) -> Self {
        Self {
            capture_id: capture_id.into(),
            started_at_ms,
            ended_at_ms: None,
            pauses: Vec::new(),
            provider_restarts_ms: Vec::new(),
            browser: None,
            native: None,
        }
    }

    /// Every helper event of this capture.
    pub fn apply(&mut self, event: &MeetingCaptureEvent) {
        match event {
            MeetingCaptureEvent::Observation(observation) => self.observe(observation),
            MeetingCaptureEvent::ObservationUnavailable { at, .. } => {
                if let Some(track) = self.native.as_mut() {
                    track.lose(millis(*at));
                }
            }
            MeetingCaptureEvent::Paused { at } => self.pause(millis(*at)),
            MeetingCaptureEvent::Resumed { at } => {
                if let Some(pause) = self.pauses.last_mut()
                    && pause.end_ms.is_none()
                {
                    pause.end_ms = Some(millis(*at).max(pause.start_ms));
                }
            }
            MeetingCaptureEvent::Ended { at, .. } => self.close(*at),
            MeetingCaptureEvent::TrackStatus { .. }
            | MeetingCaptureEvent::MicProcessingChanged { .. } => {}
        }
    }

    /// A reader's report for this capture. Reports for another capture, after the end, or inside
    /// a pause are dropped: nothing is carried across a pause or into another recording.
    pub fn observe(&mut self, observation: &SpeakerObservation) {
        if observation.capture.capture_id != self.capture_id || self.ended_at_ms.is_some() {
            return;
        }
        let at = millis(observation.at);
        if self.paused_at(at) {
            return;
        }
        let browser = observation.source == ObservationSource::BrowserExtension;
        let platform = MeetingEvidencePlatform::from(observation.platform);
        let slot = if browser {
            &mut self.browser
        } else {
            &mut self.native
        };
        let track = slot.get_or_insert_with(|| MeetingEvidenceTrack {
            platform,
            participants: Vec::new(),
            rosters: Vec::new(),
            cues: Vec::new(),
            gaps: Vec::new(),
            last_heard_ms: at,
        });
        track.record(observation, at, platform, browser, &self.pauses);
    }

    /// The live transcription stream reconnected: its voice indices start over.
    pub fn provider_restarted(&mut self, at: CaptureInstant) {
        let at = millis(at);
        if self.ended_at_ms.is_none()
            && self
                .provider_restarts_ms
                .last()
                .is_none_or(|&last| at > last)
        {
            self.provider_restarts_ms.push(at);
        }
    }

    /// The capture ended, for any reason and whether or not live transcription ran.
    pub fn close(&mut self, at: CaptureInstant) {
        self.ended_at_ms.get_or_insert(millis(at));
    }

    fn pause(&mut self, at: i64) {
        if self.ended_at_ms.is_none() && !open_gap_at(&self.pauses) {
            self.pauses.push(MeetingEvidenceGap {
                start_ms: at,
                end_ms: None,
            });
        }
    }

    fn paused_at(&self, at: i64) -> bool {
        self.pauses
            .iter()
            .any(|pause| pause.start_ms <= at && pause.end_ms.is_none_or(|end| at < end))
    }

    pub(crate) fn voice_at(&self, capture_ms: i64, speaker_index: Option<i32>) -> VoiceKey {
        VoiceKey {
            generation: self
                .provider_restarts_ms
                .partition_point(|&restart| restart <= capture_ms),
            speaker_index,
        }
    }

    pub(crate) fn is_final(&self) -> bool {
        self.ended_at_ms.is_some()
    }

    /// The track that decides: Meet's explicit cues win over the helper's reads.
    fn decisive(&self) -> Option<(&MeetingEvidenceTrack, bool)> {
        match &self.browser {
            Some(track) if !track.rosters.is_empty() || !track.cues.is_empty() => {
                Some((track, true))
            }
            _ => self.native.as_ref().map(|track| (track, false)),
        }
    }

    /// The end of the stretch, starting at `at`, over which the track's report at `at` holds.
    fn trusted_until(&self, track: &MeetingEvidenceTrack, at: i64) -> Option<i64> {
        let mut until = track.last_heard_ms.saturating_add(HEARD_GRACE_MS);
        if let Some(ended) = self.ended_at_ms {
            until = until.min(ended);
        }
        for gap in track.gaps.iter().chain(&self.pauses) {
            if gap.start_ms <= at && gap.end_ms.is_none_or(|end| at < end) {
                return None;
            }
            if gap.start_ms > at {
                until = until.min(gap.start_ms);
            }
        }
        (until > at).then_some(until)
    }

    /// Whom Meet showed speaking, shifted back by its lag. The local user's own tile says nothing
    /// about the call's voices: their voice is the microphone.
    fn speaking_intervals(&self, track: &MeetingEvidenceTrack, browser: bool) -> Vec<Interval> {
        if !browser || track.platform != MeetingEvidencePlatform::GoogleMeet {
            return Vec::new();
        }
        let mut intervals = Vec::new();
        for (index, cue) in track.cues.iter().enumerate() {
            let Some(participant) = cue.participant.filter(|&p| !track.is_self(p)) else {
                continue;
            };
            let Some(until) = self.trusted_until(track, cue.at_ms) else {
                continue;
            };
            let start = cue.at_ms - MEET_CUE_DELAY_MS;
            let end = track
                .cues
                .get(index + 1)
                .map_or(until, |next| (next.at_ms - MEET_CUE_DELAY_MS).min(until));
            if end > start {
                intervals.push(Interval {
                    start,
                    end,
                    participant,
                });
            }
        }
        intervals
    }

    /// Unbroken stretches of at least `SOLO_ROSTER_MS` in which the roster showed exactly one other
    /// person. Meet's tiles qualify unless its own count says more people; a helper roster only
    /// when it is complete, which today only Webex's open participant panel has been shown to be.
    fn one_on_one_runs(&self, track: &MeetingEvidenceTrack, browser: bool) -> Vec<Interval> {
        let mut runs: Vec<Interval> = Vec::new();
        for (index, roster) in track.rosters.iter().enumerate() {
            let Some(until) = self.trusted_until(track, roster.at_ms) else {
                continue;
            };
            let end = track
                .rosters
                .get(index + 1)
                .map_or(until, |next| next.at_ms.min(until));
            let others = roster
                .participants
                .iter()
                .copied()
                .filter(|&participant| !track.is_self(participant))
                .collect::<Vec<_>>();
            let qualifies = if browser {
                track.platform == MeetingEvidencePlatform::GoogleMeet
                    && roster.count.is_none_or(|count| count <= 2)
            } else {
                track.platform == MeetingEvidencePlatform::Webex && roster.complete
            };
            let [other] = others[..] else { continue };
            if !qualifies || end <= roster.at_ms {
                continue;
            }
            match runs.last_mut() {
                Some(run) if run.participant == other && run.end == roster.at_ms => run.end = end,
                _ => runs.push(Interval {
                    start: roster.at_ms,
                    end,
                    participant: other,
                }),
            }
        }
        runs.retain(|run| run.end - run.start >= SOLO_ROSTER_MS);
        runs
    }

    /// Each voice's name, where one participant clearly leads its speech. Two voices may share a
    /// name (the diarizer split a person); a voice never gets two.
    pub(crate) fn name_voices(&self, words: &[VoiceWord]) -> HashMap<VoiceKey, NamedVoice> {
        let Some((track, browser)) = self.decisive() else {
            return HashMap::new();
        };
        let speaking = self.speaking_intervals(track, browser);
        let solo = self.one_on_one_runs(track, browser);
        let min_voted_ms = if self.is_final() {
            FINAL_MIN_VOTED_SPEECH_MS
        } else {
            PROVISIONAL_MIN_VOTED_SPEECH_MS
        };

        #[derive(Default)]
        struct Tally {
            spoken_ms: i64,
            solo: BTreeMap<u32, i64>,
            all: BTreeMap<u32, i64>,
        }
        let mut tallies: HashMap<VoiceKey, Tally> = HashMap::new();
        for word in words {
            let spoken_ms = word.end_ms - word.start_ms;
            if spoken_ms <= 0 {
                continue;
            }
            let tally = tallies.entry(word.voice).or_default();
            tally.spoken_ms += spoken_ms;
            let mut solo_ms = 0;
            for (participant, ms) in overlapping(word, &solo) {
                solo_ms += ms;
                *tally.solo.entry(participant).or_default() += ms;
                *tally.all.entry(participant).or_default() += ms;
            }
            // Speech a 1:1's roster already names does not vote again on cues.
            if solo_ms >= spoken_ms {
                continue;
            }
            for (participant, ms) in overlapping(word, &speaking) {
                *tally.all.entry(participant).or_default() += ms;
            }
        }

        tallies
            .into_iter()
            .filter_map(|(voice, tally)| {
                let spoken = tally.spoken_ms as f64;
                let (solo_leader, solo_ms, _) = leader(&tally.solo);
                let (participant, one_on_one) = match solo_leader {
                    Some(participant) if solo_ms as f64 >= NAME_SHARE * spoken => {
                        (participant, true)
                    }
                    // The 1:1's votes are weighed with the cues', so a voice the diarizer merged
                    // across a join stays unnamed when the two disagree.
                    _ => {
                        let (participant, ms, total) = leader(&tally.all);
                        let clear = total >= min_voted_ms
                            && total as f64 >= MIN_VOTED_COVERAGE * spoken
                            && ms as f64 >= NAME_SHARE * total as f64;
                        (participant.filter(|_| clear)?, false)
                    }
                };
                let name = track.participants.get(participant as usize)?.name.clone()?;
                Some((voice, NamedVoice { name, one_on_one }))
            })
            .collect()
    }

    /// Whether the decisive roster at `at` shows at least two people besides the local user. Rows
    /// are a lower bound (Accessibility only sees rows on screen), so any reader can prove this.
    pub(crate) fn group_call_at(&self, at: i64) -> bool {
        let Some((track, _)) = self.decisive() else {
            return false;
        };
        let index = track.rosters.partition_point(|roster| roster.at_ms <= at);
        let Some(roster) = index.checked_sub(1).and_then(|i| track.rosters.get(i)) else {
            return false;
        };
        if self
            .trusted_until(track, roster.at_ms)
            .is_none_or(|until| until <= at)
        {
            return false;
        }
        let rows = roster.participants.len();
        let selves = roster
            .participants
            .iter()
            .filter(|&&participant| track.is_self(participant))
            .count();
        let others = if selves == 0 {
            rows.saturating_sub(1)
        } else {
            rows - selves
        };
        let counted = roster
            .count
            .map_or(0, |count| (count as usize).saturating_sub(1));
        others.max(counted) >= 2
    }

    /// Stored evidence the naming can rely on: times in order and every index in range.
    fn is_well_formed(&self) -> bool {
        [&self.browser, &self.native]
            .into_iter()
            .flatten()
            .all(MeetingEvidenceTrack::is_well_formed)
            && is_sorted(self.pauses.iter().map(|gap| gap.start_ms))
            && is_sorted(self.provider_restarts_ms.iter().copied())
    }
}

impl MeetingEvidenceTrack {
    fn is_self(&self, participant: u32) -> bool {
        self.participants
            .get(participant as usize)
            .is_some_and(|entry| entry.is_self)
    }

    fn lose(&mut self, at: i64) {
        if !open_gap_at(&self.gaps) {
            self.gaps.push(MeetingEvidenceGap {
                start_ms: at.max(self.last_heard_ms),
                end_ms: None,
            });
        }
    }

    fn interrupted(&self, pauses: &[MeetingEvidenceGap], from: i64, to: i64) -> bool {
        self.gaps
            .iter()
            .chain(pauses)
            .any(|gap| gap.start_ms > from && gap.start_ms <= to)
    }

    fn participant(&mut self, id: &str, name: Option<String>, is_self: bool) -> Option<u32> {
        let id = clean(id)?;
        let index = match self.participants.iter().position(|entry| entry.id == id) {
            Some(index) => index,
            None if self.participants.len() < MAX_PARTICIPANTS => {
                self.participants.push(MeetingEvidenceParticipant {
                    id,
                    name: None,
                    is_self: false,
                });
                self.participants.len() - 1
            }
            None => return None,
        };
        let entry = &mut self.participants[index];
        if name.is_some() {
            entry.name = name;
        }
        entry.is_self |= is_self;
        Some(index as u32)
    }

    fn record(
        &mut self,
        observation: &SpeakerObservation,
        at: i64,
        platform: MeetingEvidencePlatform,
        browser: bool,
        pauses: &[MeetingEvidenceGap],
    ) {
        if platform != self.platform {
            return self.lose(at);
        }
        if at < self.last_heard_ms || self.rosters.len() + self.cues.len() >= MAX_TRACK_EVENTS {
            return;
        }
        if let Some(gap) = self.gaps.last_mut()
            && gap.end_ms.is_none()
        {
            gap.end_ms = Some(at.max(gap.start_ms));
        } else if at - self.last_heard_ms > READER_LEASE_MS {
            self.gaps.push(MeetingEvidenceGap {
                start_ms: self.last_heard_ms + HEARD_GRACE_MS,
                end_ms: Some(at),
            });
        }

        let rows = &observation.participants;
        let mut participants = Vec::new();
        let mut identified = rows.len() <= MAX_ROSTER_ROWS;
        let mut selves = 0;
        for row in rows.iter().take(MAX_ROSTER_ROWS) {
            let name = row.display_name.as_deref().and_then(clean);
            identified &= name.is_some() && row.is_self.is_some();
            let is_self = row.is_self == Some(true);
            selves += usize::from(is_self);
            match self.participant(&row.participant_id, name, is_self) {
                Some(index) if !participants.contains(&index) => participants.push(index),
                _ => identified = false,
            }
        }
        let count = observation.participant_count;
        let roster = MeetingRosterEvidence {
            at_ms: at,
            complete: observation.roster_complete
                && identified
                && selves == 1
                && count == Some(participants.len() as u32),
            participants,
            count,
        };
        let roster_changed = self.rosters.last().is_none_or(|last| {
            (&last.participants, last.count, last.complete)
                != (&roster.participants, roster.count, roster.complete)
                || self.interrupted(pauses, last.at_ms, at)
        });
        if roster_changed {
            self.rosters.push(roster);
        }

        // Accessibility shows no speaking signal on any app anyone has checked; ignore any it sends.
        if browser && let Some(speaking) = &observation.speaking {
            let last = self.cues.last().cloned();
            let fresh = last
                .as_ref()
                .filter(|last| !self.interrupted(pauses, last.at_ms, at));
            let lit = speaking
                .iter()
                .filter_map(|cue| {
                    let id = cue.participant_id.as_deref()?;
                    let name = cue.display_name.as_deref().and_then(clean);
                    self.participant(id, name, false)
                })
                .collect::<Vec<_>>();
            // While two are lit, whoever was already speaking stays.
            let current = fresh.and_then(|last| last.participant);
            let active = current
                .filter(|participant| lit.contains(participant))
                .or_else(|| lit.first().copied());
            if fresh.is_none_or(|last| last.participant != active) {
                self.cues.push(MeetingCueEvidence {
                    at_ms: at,
                    participant: active,
                });
            }
        }
        self.last_heard_ms = at;
    }

    fn is_well_formed(&self) -> bool {
        let known = self.participants.len();
        is_sorted(self.rosters.iter().map(|roster| roster.at_ms))
            && is_sorted(self.cues.iter().map(|cue| cue.at_ms))
            && is_sorted(self.gaps.iter().map(|gap| gap.start_ms))
            && self
                .rosters
                .iter()
                .flat_map(|roster| &roster.participants)
                .chain(self.cues.iter().filter_map(|cue| cue.participant.as_ref()))
                .all(|&participant| (participant as usize) < known)
    }
}

fn is_sorted(mut values: impl Iterator<Item = i64>) -> bool {
    let Some(mut previous) = values.next() else {
        return true;
    };
    values.all(|value| {
        let ordered = previous <= value;
        previous = value;
        ordered
    })
}

/// Stored `meetings` entries that parse and are well formed; anything else is dropped rather than
/// failing the whole speaker context.
pub(crate) fn parse_meeting_evidence(value: &Value) -> Vec<MeetingSpeakerEvidence> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| serde_json::from_value::<MeetingSpeakerEvidence>(entry.clone()).ok())
        .filter(MeetingSpeakerEvidence::is_well_formed)
        .collect()
}

pub(crate) fn deserialize_meeting_evidence<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<MeetingSpeakerEvidence>, D::Error> {
    let value = <Value as serde::Deserialize>::deserialize(deserializer)?;
    Ok(parse_meeting_evidence(&value))
}

/// The stored `meetings` array with `evidence` in place of its capture's entry. A snapshot taken
/// before a capture ended never replaces the ended one. `None` when nothing should be written.
pub fn upsert_meeting_evidence(stored: &Value, evidence: &MeetingSpeakerEvidence) -> Option<Value> {
    let mut meetings = parse_meeting_evidence(stored);
    match meetings
        .iter_mut()
        .find(|entry| entry.capture_id == evidence.capture_id)
    {
        Some(entry) if entry.ended_at_ms.is_some() && evidence.ended_at_ms.is_none() => {
            return None;
        }
        Some(entry) => *entry = evidence.clone(),
        None => meetings.push(evidence.clone()),
    }
    meetings.sort_by_key(|entry| entry.started_at_ms);
    serde_json::to_value(meetings).ok()
}

/// A provider voice within one capture: its diarization index within one stream generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct VoiceKey {
    generation: usize,
    speaker_index: Option<i32>,
}

/// One word of the call, in capture milliseconds.
#[derive(Debug, Clone)]
pub(crate) struct VoiceWord {
    pub voice: VoiceKey,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NamedVoice {
    pub name: String,
    pub one_on_one: bool,
}

struct Interval {
    start: i64,
    end: i64,
    participant: u32,
}

/// Each interval the word overlaps, with how much. Intervals are in time order and disjoint, so
/// the first candidate is found by bisection: a long call is named on every render.
fn overlapping<'a>(
    word: &'a VoiceWord,
    intervals: &'a [Interval],
) -> impl Iterator<Item = (u32, i64)> + 'a {
    let first = intervals.partition_point(|interval| interval.end <= word.start_ms);
    intervals[first..]
        .iter()
        .take_while(|interval| interval.start < word.end_ms)
        .map(|interval| {
            let ms = interval.end.min(word.end_ms) - interval.start.max(word.start_ms);
            (interval.participant, ms.max(0))
        })
}

fn leader(votes: &BTreeMap<u32, i64>) -> (Option<u32>, i64, i64) {
    let mut best = None;
    let mut best_ms = 0;
    let mut total = 0;
    for (&participant, &ms) in votes {
        total += ms;
        if ms > best_ms {
            best = Some(participant);
            best_ms = ms;
        }
    }
    (best, best_ms, total)
}

#[cfg(test)]
mod tests;
