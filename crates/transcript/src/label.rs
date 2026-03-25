use std::collections::HashMap;

use crate::{ChannelProfile, Segment, SegmentKey};

#[derive(Debug, Clone, Default)]
pub struct SpeakerLabelContext {
    pub self_human_id: Option<String>,
    pub human_name_by_id: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct SpeakerLabeler {
    unknown_speaker_map: HashMap<SegmentKey, usize>,
    next_index: usize,
}

impl SpeakerLabeler {
    pub fn new() -> Self {
        Self {
            unknown_speaker_map: HashMap::new(),
            next_index: 1,
        }
    }

    pub fn from_segments(segments: &[Segment], ctx: Option<&SpeakerLabelContext>) -> Self {
        let mut labeler = Self::new();
        for segment in segments {
            if !segment.key.is_known_speaker(ctx) {
                labeler.unknown_speaker_number(&segment.key);
            }
        }
        labeler
    }

    pub fn label_for(&mut self, key: &SegmentKey, ctx: Option<&SpeakerLabelContext>) -> String {
        render_speaker_label(key, ctx, Some(self))
    }

    pub fn unknown_speaker_number(&mut self, key: &SegmentKey) -> usize {
        if let Some(existing) = self.unknown_speaker_map.get(key) {
            return *existing;
        }

        let next = self.next_index;
        self.unknown_speaker_map.insert(key.clone(), next);
        self.next_index += 1;
        next
    }
}

impl SegmentKey {
    pub fn is_known_speaker(&self, ctx: Option<&SpeakerLabelContext>) -> bool {
        if self.speaker_human_id.is_some() {
            return true;
        }

        matches!(
            ctx,
            Some(SpeakerLabelContext {
                self_human_id: Some(_),
                ..
            }) if self.channel == ChannelProfile::DirectMic
        )
    }
}

pub fn render_speaker_label(
    key: &SegmentKey,
    ctx: Option<&SpeakerLabelContext>,
    mut labeler: Option<&mut SpeakerLabeler>,
) -> String {
    if let Some(ctx) = ctx {
        if let Some(human_id) = key.speaker_human_id.as_ref() {
            if let Some(name) = ctx.human_name_by_id.get(human_id) {
                return name.clone();
            }
            return human_id.clone();
        }

        if key.channel == ChannelProfile::DirectMic
            && let Some(self_human_id) = ctx.self_human_id.as_ref()
        {
            if let Some(name) = ctx.human_name_by_id.get(self_human_id) {
                return name.clone();
            }
            return "You".to_string();
        }
    } else if let Some(human_id) = key.speaker_human_id.as_ref() {
        return human_id.clone();
    }

    if let Some(labeler) = labeler.as_mut() {
        return format!("Speaker {}", labeler.unknown_speaker_number(key));
    }

    let channel_label = match key.channel {
        ChannelProfile::DirectMic => "A",
        ChannelProfile::RemoteParty => "B",
        ChannelProfile::MixedCapture => "C",
    };

    match key.speaker_index {
        Some(index) => format!("Speaker {}", index + 1),
        None => format!("Speaker {channel_label}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChannelProfile;

    fn direct_mic_key() -> SegmentKey {
        SegmentKey {
            channel: ChannelProfile::DirectMic,
            speaker_index: None,
            speaker_human_id: None,
        }
    }

    #[test]
    fn renders_direct_mic_as_you_when_self_exists_without_name() {
        let ctx = SpeakerLabelContext {
            self_human_id: Some("self".to_string()),
            human_name_by_id: HashMap::new(),
        };

        assert_eq!(
            render_speaker_label(&direct_mic_key(), Some(&ctx), None),
            "You"
        );
    }

    #[test]
    fn preserves_unknown_speaker_numbers_across_calls() {
        let mut labeler = SpeakerLabeler::new();
        let a = SegmentKey {
            channel: ChannelProfile::DirectMic,
            speaker_index: Some(7),
            speaker_human_id: None,
        };
        let b = SegmentKey {
            channel: ChannelProfile::RemoteParty,
            speaker_index: Some(9),
            speaker_human_id: None,
        };

        assert_eq!(labeler.label_for(&a, None), "Speaker 1");
        assert_eq!(labeler.label_for(&b, None), "Speaker 2");
        assert_eq!(labeler.label_for(&a, None), "Speaker 1");
    }
}
