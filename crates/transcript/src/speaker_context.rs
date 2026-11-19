use std::collections::{HashMap, HashSet};

use crate::meeting_speakers::{MeetingSpeakerEvidence, NamedVoice, VoiceKey, VoiceWord};
use crate::{ChannelProfile, RenderTranscriptHuman, RenderedTranscriptSegment, SegmentWord};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SpeakerContext {
    pub intervals: Vec<SpeakerContextInterval>,
    /// Platform evidence per capture of this session. Malformed entries are dropped on read.
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        default,
        deserialize_with = "crate::meeting_speakers::deserialize_meeting_evidence"
    )]
    pub meetings: Vec<MeetingSpeakerEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SpeakerContextInterval {
    pub start_ms: i64,
    pub end_ms: i64,
    pub active_call: bool,
    pub calendar_call: bool,
    pub mic_isolated: Option<bool>,
    pub shared_microphone: bool,
    pub title: String,
    pub self_names: Vec<String>,
    pub participants: Vec<RenderTranscriptHuman>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerResolutionReason {
    PersonalMicrophone,
    VirtualMeetingMicrophone,
    SoleRemoteParticipant,
    OneOnOneTitle,
    /// The meeting app showed this participant speaking over most of the voice.
    MeetingActiveSpeaker,
    /// The meeting app's roster showed exactly one other person while the voice spoke.
    MeetingOneOnOne,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ProvisionalSpeakerLabel {
    pub name: String,
    pub human_id: Option<String>,
    pub reason: SpeakerResolutionReason,
}

pub fn segment_options_for_assignments(
    assignments: &[crate::IdentityAssignment],
) -> crate::SegmentBuilderOptions {
    crate::SegmentBuilderOptions {
        complete_channels: Some(
            assignments
                .iter()
                .filter_map(|assignment| match assignment.scope {
                    crate::IdentityScope::Channel { channel } => Some(channel),
                    _ => None,
                })
                .collect(),
        ),
        min_segment_words: Some(0),
        min_segment_ms: Some(0),
        ..Default::default()
    }
}

impl SpeakerContext {
    /// A stored `speaker_context` value, keeping what is valid: malformed or empty intervals and
    /// malformed meeting evidence are dropped instead of failing the whole context.
    pub fn from_stored_value(value: &serde_json::Value) -> Self {
        let intervals = value
            .get("intervals")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|interval| {
                serde_json::from_value::<SpeakerContextInterval>(interval.clone()).ok()
            })
            .filter(|interval| interval.start_ms < interval.end_ms)
            .collect();
        let meetings = value
            .get("meetings")
            .map(crate::meeting_speakers::parse_meeting_evidence)
            .unwrap_or_default();
        Self {
            intervals,
            meetings,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty() && self.meetings.is_empty()
    }

    fn interval_at(&self, start_ms: i64, end_ms: i64) -> Option<usize> {
        let mut matches = self.intervals.iter().enumerate().filter(|(_, interval)| {
            interval.start_ms <= start_ms
                && end_ms <= interval.end_ms
                && interval.start_ms < interval.end_ms
        });
        if let Some((index, _)) = matches.next() {
            return matches.next().is_none().then_some(index);
        }
        self.isolated_run_at(start_ms, end_ms)
    }

    // Call-evidence changes split context intervals even while `mic_isolated`
    // stays true on both sides, so adjacent isolated intervals form one
    // verified isolated period; a word straddling that seam still belongs to
    // the run's first interval.
    fn isolated_run_at(&self, start_ms: i64, end_ms: i64) -> Option<usize> {
        let mut run: Option<(usize, i64, i64)> = None;
        for (index, interval) in self.intervals.iter().enumerate() {
            if interval.mic_isolated != Some(true) {
                continue;
            }
            match &mut run {
                Some((_, _, run_end)) if interval.start_ms <= *run_end => {
                    *run_end = (*run_end).max(interval.end_ms);
                }
                _ => run = Some((index, interval.start_ms, interval.end_ms)),
            }
            if let Some((first, run_start, run_end)) = run
                && run_start <= start_ms
                && end_ms <= run_end
                && run_start < run_end
            {
                return Some(first);
            }
        }
        None
    }

    /// `transcript_starts` are the epoch-ms starts of every transcript in the render; a later one
    /// bounds a capture whose evidence never closed, so it cannot name the next recording.
    pub fn label_segments(
        &self,
        segments: Vec<RenderedTranscriptSegment>,
        started_at: i64,
        transcript_starts: &[i64],
        self_human_id: Option<&str>,
        humans: &[RenderTranscriptHuman],
    ) -> Vec<RenderedTranscriptSegment> {
        let meetings = MeetingNames::new(&self.meetings, transcript_starts, &segments, started_at);
        // Mic words per diarization voice per interval. Remote voices are not counted: diarization
        // indices restart with every stream refresh, so their number says nothing about how
        // many people are on the far end of a call.
        let mut local_voices = vec![HashMap::<Option<i32>, usize>::new(); self.intervals.len()];
        for segment in &segments {
            if segment.key.channel != ChannelProfile::DirectMic {
                continue;
            }
            for word in &segment.words {
                if let Some(index) = self.interval_at(
                    started_at.saturating_add(word.start_ms),
                    started_at.saturating_add(word.end_ms),
                ) {
                    *local_voices[index]
                        .entry(segment.key.speaker_index)
                        .or_default() += 1;
                }
            }
        }

        let mut result = Vec::new();
        for segment in segments {
            // Context boundaries only affect inferred names, not explicit assignments.
            if segment.key.speaker_human_id.is_some() {
                result.push(RenderedTranscriptSegment {
                    provisional_speaker: None,
                    ..segment
                });
                continue;
            }

            let mut groups: Vec<(Option<usize>, MeetingLabel, Vec<SegmentWord>)> = Vec::new();
            for word in &segment.words {
                let interval = self.interval_at(
                    started_at.saturating_add(word.start_ms),
                    started_at.saturating_add(word.end_ms),
                );
                let mut meeting = meetings.label(&segment.key, word);
                // An observed group call only matters where a calendar guess could apply; keep it
                // from splitting turns elsewhere.
                meeting.group_call &= meeting.name.is_none()
                    && interval.is_some_and(|index| {
                        self.intervals[index].active_call || self.intervals[index].calendar_call
                    });
                if let Some((last_interval, last_meeting, words)) = groups.last_mut()
                    && *last_interval == interval
                    && *last_meeting == meeting
                {
                    words.push(word.clone());
                } else {
                    groups.push((interval, meeting, vec![word.clone()]));
                }
            }
            let split = groups.len() > 1;
            for (interval, meeting, words) in groups {
                let mut part = RenderedTranscriptSegment {
                    id: segment.id.clone(),
                    key: segment.key.clone(),
                    speaker_label: segment.speaker_label.clone(),
                    provisional_speaker: None,
                    start_ms: words[0].start_ms,
                    end_ms: words.last().unwrap().end_ms,
                    text: String::new(),
                    words: Vec::new(),
                };
                if split {
                    part.id = format!("{}:{}", part.id, part.start_ms);
                }
                part.text = words
                    .iter()
                    .map(|word| word.text.as_str())
                    .collect::<String>()
                    .trim()
                    .to_owned();
                part.words = words;
                let inferred = interval.and_then(|index| {
                    resolve_speaker(
                        &self.intervals[index],
                        part.key.channel,
                        is_owner_voice(&local_voices[index], part.key.speaker_index),
                        self_human_id,
                        humans,
                    )
                });
                part.provisional_speaker = meeting.resolve(inferred);
                if let Some(label) = &part.provisional_speaker {
                    part.speaker_label = label.name.clone();
                }
                result.push(part);
            }
        }

        // Provisional names do not consume anonymous speaker numbers or become identity hints.
        let mut unknown = std::collections::HashMap::new();
        for segment in &mut result {
            if segment.key.speaker_human_id.is_none() && segment.provisional_speaker.is_none() {
                let next = unknown.len() + 1;
                let number = unknown
                    .entry((segment.key.channel, segment.key.speaker_index))
                    .or_insert(next);
                segment.speaker_label = format!("Speaker {number}");
            }
        }
        result
    }
}

/// A later transcript within this of a capture's start is that capture's own transcript.
const CAPTURE_START_TOLERANCE_MS: i64 = 1_000;

/// The meeting-evidence names of one render's remote voices.
struct MeetingNames<'a> {
    meetings: &'a [MeetingSpeakerEvidence],
    transcript_starts: &'a [i64],
    started_at: i64,
    names: Vec<HashMap<VoiceKey, NamedVoice>>,
}

/// What meeting evidence says about one remote word.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MeetingLabel {
    name: Option<NamedVoice>,
    group_call: bool,
}

impl<'a> MeetingNames<'a> {
    fn new(
        meetings: &'a [MeetingSpeakerEvidence],
        transcript_starts: &'a [i64],
        segments: &[RenderedTranscriptSegment],
        started_at: i64,
    ) -> Self {
        let mut names = Self {
            meetings,
            transcript_starts,
            started_at,
            names: Vec::new(),
        };
        if meetings.is_empty() {
            return names;
        }
        let mut words: Vec<Vec<VoiceWord>> = meetings.iter().map(|_| Vec::new()).collect();
        for segment in segments {
            if !is_unassigned_remote(&segment.key) {
                continue;
            }
            for word in &segment.words {
                if let Some((index, start_ms)) = names.capture_of(word) {
                    words[index].push(VoiceWord {
                        voice: meetings[index].voice_at(start_ms, segment.key.speaker_index),
                        start_ms,
                        end_ms: start_ms + (word.end_ms - word.start_ms),
                    });
                }
            }
        }
        names.names = meetings
            .iter()
            .zip(&words)
            .map(|(evidence, words)| evidence.name_voices(words))
            .collect();
        names
    }

    /// The capture `word` was recorded in, with its capture time: the latest capture started by
    /// then that had neither ended nor been followed by another transcript. Ties name nobody.
    fn capture_of(&self, word: &SegmentWord) -> Option<(usize, i64)> {
        let at = self.started_at.saturating_add(word.start_ms);
        let mut best: Option<usize> = None;
        let mut tied = false;
        for (index, evidence) in self.meetings.iter().enumerate() {
            if evidence.started_at_ms > at {
                continue;
            }
            match best.map(|best| self.meetings[best].started_at_ms) {
                Some(start) if start > evidence.started_at_ms => {}
                Some(start) if start == evidence.started_at_ms => tied = true,
                _ => {
                    best = Some(index);
                    tied = false;
                }
            }
        }
        let index = best.filter(|_| !tied)?;
        let evidence = &self.meetings[index];
        let capture_ms = at - evidence.started_at_ms;
        let next_recording = self.transcript_starts.iter().any(|&start| {
            start > evidence.started_at_ms + CAPTURE_START_TOLERANCE_MS && start <= at
        });
        let ended = evidence.ended_at_ms.is_some_and(|end| capture_ms > end);
        (!ended && !next_recording).then_some((index, capture_ms))
    }

    fn label(&self, key: &crate::SegmentKey, word: &SegmentWord) -> MeetingLabel {
        if self.names.is_empty() || !is_unassigned_remote(key) {
            return MeetingLabel::default();
        }
        let Some((index, capture_ms)) = self.capture_of(word) else {
            return MeetingLabel::default();
        };
        let evidence = &self.meetings[index];
        MeetingLabel {
            name: self.names[index]
                .get(&evidence.voice_at(capture_ms, key.speaker_index))
                .cloned(),
            group_call: evidence.group_call_at(capture_ms),
        }
    }
}

fn is_unassigned_remote(key: &crate::SegmentKey) -> bool {
    key.channel == ChannelProfile::RemoteParty && key.speaker_human_id.is_none()
}

impl MeetingLabel {
    /// Meeting evidence outranks calendar guesses; explicit assignments never reach here. A sole
    /// invitee the app names the same keeps the invitee's provisional link.
    fn resolve(
        &self,
        inferred: Option<ProvisionalSpeakerLabel>,
    ) -> Option<ProvisionalSpeakerLabel> {
        match &self.name {
            Some(named) => Some(match inferred {
                Some(label)
                    if label.reason == SpeakerResolutionReason::SoleRemoteParticipant
                        && label.name.to_lowercase() == named.name.to_lowercase() =>
                {
                    label
                }
                _ => ProvisionalSpeakerLabel {
                    name: named.name.clone(),
                    human_id: None,
                    reason: if named.one_on_one {
                        SpeakerResolutionReason::MeetingOneOnOne
                    } else {
                        SpeakerResolutionReason::MeetingActiveSpeaker
                    },
                },
            }),
            // The app shows more people than a calendar 1:1 guess allows.
            None if self.group_call => inferred.filter(|label| {
                !matches!(
                    label.reason,
                    SpeakerResolutionReason::SoleRemoteParticipant
                        | SpeakerResolutionReason::OneOnOneTitle
                )
            }),
            None => inferred,
        }
    }
}

// A room mic on a call also catches nearby chatter and diarization splits of the owner's own
// voice, so the voice carrying at least this share of the interval's mic words is the owner.
const OWNER_VOICE_SHARE_PERCENT: usize = 80;

fn is_owner_voice(voices: &HashMap<Option<i32>, usize>, speaker_index: Option<i32>) -> bool {
    if voices.len() <= 1 {
        return true;
    }
    // Unindexed words carry no voice identity once diarized voices are present.
    if speaker_index.is_none() {
        return false;
    }
    let total: usize = voices.values().sum();
    voices
        .get(&speaker_index)
        .is_some_and(|count| count * 100 >= total * OWNER_VOICE_SHARE_PERCENT)
}

fn resolve_speaker(
    context: &SpeakerContextInterval,
    source: ChannelProfile,
    owner_voice: bool,
    self_id: Option<&str>,
    humans: &[RenderTranscriptHuman],
) -> Option<ProvisionalSpeakerLabel> {
    let self_id = self_id.filter(|id| !id.trim().is_empty())?;
    let virtual_call = context.active_call || context.calendar_call;
    // A headset only hears its wearer however diarization splits the voice; a room microphone
    // on a call is the owner only for the voice that dominates it.
    let personal_microphone = context.mic_isolated == Some(true);
    match source {
        ChannelProfile::DirectMic
            if !context.shared_microphone
                && (personal_microphone || (virtual_call && owner_voice)) =>
        {
            let name = humans
                .iter()
                .find(|human| human.human_id == self_id)
                .map(|human| human.name.trim())
                .filter(|name| !name.is_empty())
                .unwrap_or("You");
            Some(ProvisionalSpeakerLabel {
                name: name.to_owned(),
                human_id: Some(self_id.to_owned()),
                reason: if personal_microphone {
                    SpeakerResolutionReason::PersonalMicrophone
                } else {
                    SpeakerResolutionReason::VirtualMeetingMicrophone
                },
            })
        }
        // The far end of a call is whoever else was invited, regardless of how many voices
        // diarization reports for it.
        ChannelProfile::RemoteParty if virtual_call => {
            let remotes = context
                .participants
                .iter()
                .filter(|person| !person.human_id.is_empty() && person.human_id != self_id)
                .map(|person| person.human_id.as_str())
                .collect::<HashSet<_>>();
            if remotes.len() == 1 {
                let person = context
                    .participants
                    .iter()
                    .find(|person| remotes.contains(person.human_id.as_str()))?;
                if !person.name.trim().is_empty() {
                    return Some(ProvisionalSpeakerLabel {
                        name: person.name.trim().to_owned(),
                        human_id: Some(person.human_id.clone()),
                        reason: SpeakerResolutionReason::SoleRemoteParticipant,
                    });
                }
            }
            if !remotes.is_empty() {
                return None;
            }
            remote_name_from_title(&context.title, &context.self_names).map(|name| {
                ProvisionalSpeakerLabel {
                    name,
                    human_id: None,
                    reason: SpeakerResolutionReason::OneOnOneTitle,
                }
            })
        }
        _ => None,
    }
}

pub fn remote_name_from_title(title: &str, self_names: &[String]) -> Option<String> {
    let title = title
        .trim()
        .trim_matches(|c| matches!(c, '[' | ']' | '(' | ')' | '{' | '}' | '【' | '】'));
    let mut normalized = title.replace(['🤝', '×', '/', '&', '+', '|', '·', '、', '↔'], " x ");
    for separator in ["<->", "<>", "][", "] [", ")(", ") ("] {
        normalized = normalized.replace(separator, " x ");
    }
    let names = if normalized.contains(" x ") || normalized.contains(" X ") {
        normalized
            .split(" x ")
            .flat_map(|part| part.split(" X "))
            .map(str::trim)
            .collect::<Vec<_>>()
    } else {
        // Unseparated Korean first-name pairs are common; arbitrary multi-word titles are not names.
        let words = title.split_whitespace().collect::<Vec<_>>();
        if words.len() != 2
            || !words.iter().all(|word| {
                (2..=4).contains(&word.chars().count())
                    && word.chars().all(|c| ('가'..='힣').contains(&c))
            })
        {
            return None;
        }
        words
    };
    if names.len() != 2
        || names.iter().any(|name| {
            name.is_empty()
                || name.chars().count() > 60
                || !name
                    .chars()
                    .all(|c| c.is_alphabetic() || matches!(c, ' ' | '-' | '\'' | '.'))
        })
    {
        return None;
    }
    let is_self = |name: &str| {
        self_names.iter().any(|alias| {
            !alias.trim().is_empty() && alias.trim().to_lowercase() == name.to_lowercase()
        })
    };
    match (is_self(names[0]), is_self(names[1])) {
        (true, false) => Some(names[1].to_owned()),
        (false, true) => Some(names[0].to_owned()),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
