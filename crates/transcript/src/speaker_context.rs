use std::collections::HashSet;

use crate::{ChannelProfile, RenderTranscriptHuman, RenderedTranscriptSegment};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SpeakerContext {
    pub intervals: Vec<SpeakerContextInterval>,
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
    fn interval_at(&self, start_ms: i64, end_ms: i64) -> Option<usize> {
        let mut matches = self.intervals.iter().enumerate().filter(|(_, interval)| {
            interval.start_ms <= start_ms
                && end_ms <= interval.end_ms
                && interval.start_ms < interval.end_ms
        });
        let (index, _) = matches.next()?;
        matches.next().is_none().then_some(index)
    }

    pub fn label_segments(
        &self,
        segments: Vec<RenderedTranscriptSegment>,
        started_at: i64,
        self_human_id: Option<&str>,
        humans: &[RenderTranscriptHuman],
    ) -> Vec<RenderedTranscriptSegment> {
        let mut counts = vec![(HashSet::new(), HashSet::new()); self.intervals.len()];
        for segment in &segments {
            for word in &segment.words {
                if let Some(index) = self.interval_at(
                    started_at.saturating_add(word.start_ms),
                    started_at.saturating_add(word.end_ms),
                ) {
                    match segment.key.channel {
                        ChannelProfile::DirectMic => {
                            counts[index].0.insert(segment.key.speaker_index);
                        }
                        ChannelProfile::RemoteParty => {
                            counts[index].1.insert(segment.key.speaker_index);
                        }
                        ChannelProfile::MixedCapture => {}
                    }
                }
            }
        }

        let mut result = Vec::new();
        for segment in segments {
            let mut groups: Vec<(Option<usize>, Vec<crate::SegmentWord>)> = Vec::new();
            for word in &segment.words {
                let interval = self.interval_at(
                    started_at.saturating_add(word.start_ms),
                    started_at.saturating_add(word.end_ms),
                );
                if let Some((last_interval, words)) = groups.last_mut()
                    && *last_interval == interval
                {
                    words.push(word.clone());
                } else {
                    groups.push((interval, vec![word.clone()]));
                }
            }
            let split = groups.len() > 1;
            for (interval, words) in groups {
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
                part.provisional_speaker = None;
                if part.key.speaker_human_id.is_none()
                    && let Some(index) = interval
                {
                    part.provisional_speaker = resolve_speaker(
                        &self.intervals[index],
                        part.key.channel,
                        counts[index].0.len(),
                        counts[index].1.len(),
                        self_human_id,
                        humans,
                    );
                }
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

fn resolve_speaker(
    context: &SpeakerContextInterval,
    source: ChannelProfile,
    local_voices: usize,
    remote_voices: usize,
    self_id: Option<&str>,
    humans: &[RenderTranscriptHuman],
) -> Option<ProvisionalSpeakerLabel> {
    let self_id = self_id.filter(|id| !id.trim().is_empty())?;
    let virtual_call = context.active_call || context.calendar_call;
    match source {
        ChannelProfile::DirectMic
            if !context.shared_microphone
                && local_voices <= 1
                && (virtual_call || context.mic_isolated == Some(true)) =>
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
                reason: if context.mic_isolated == Some(true) {
                    SpeakerResolutionReason::PersonalMicrophone
                } else {
                    SpeakerResolutionReason::VirtualMeetingMicrophone
                },
            })
        }
        ChannelProfile::RemoteParty if virtual_call && remote_voices == 1 => {
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
