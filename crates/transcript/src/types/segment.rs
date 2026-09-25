#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[repr(i32)]
pub enum ChannelProfile {
    DirectMic = 0,
    RemoteParty = 1,
    MixedCapture = 2,
}

impl From<i32> for ChannelProfile {
    fn from(value: i32) -> Self {
        match value {
            0 => ChannelProfile::DirectMic,
            1 => ChannelProfile::RemoteParty,
            2 => ChannelProfile::MixedCapture,
            _ => ChannelProfile::MixedCapture,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SegmentKey {
    pub channel: ChannelProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_human_id: Option<String>,
}

impl SegmentKey {
    pub fn has_speaker_identity(&self) -> bool {
        self.speaker_index.is_some() || self.speaker_human_id.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SegmentWord {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub channel: ChannelProfile,
    pub is_final: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Segment {
    pub key: SegmentKey,
    pub words: Vec<SegmentWord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SegmentBuilderOptions {
    pub max_gap_ms: Option<i64>,
    pub complete_channels: Option<Vec<ChannelProfile>>,
    pub min_segment_words: Option<usize>,
    pub min_segment_ms: Option<i64>,
    /// `(start_ms, end_ms)` ranges, in the same clock as word timestamps, where the
    /// microphone was isolated and provider diarization was off. Index-less mic
    /// words inside a range may inherit the channel's only named speaker; words
    /// outside it cannot, so voices from shared-mic intervals stay unnamed.
    pub isolated_mic_ranges: Option<Vec<(i64, i64)>>,
}

impl Default for SegmentBuilderOptions {
    fn default() -> Self {
        Self {
            max_gap_ms: None,
            complete_channels: Some(vec![ChannelProfile::DirectMic]),
            min_segment_words: Some(3),
            min_segment_ms: Some(1500),
            isolated_mic_ranges: None,
        }
    }
}
