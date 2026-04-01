use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct CreateBotRequest {
    pub meeting_url: String,
    pub bot_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcription_options: Option<TranscriptionOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real_time_transcription: Option<RealTimeTranscriptionConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_media: Option<OutputMedia>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_leave: Option<AutomaticLeaveConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording_config: Option<RecordingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<BotVariant>,
}

#[derive(Debug, Serialize)]
pub struct RecordingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_recording_on: Option<StartRecordingOn>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartRecordingOn {
    CallJoin,
    ParticipantJoin,
    ParticipantSpeak,
    Manual,
}

#[derive(Debug, Serialize, Default)]
pub struct BotVariant {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom: Option<VariantKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google_meet: Option<VariantKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub microsoft_teams: Option<VariantKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webex: Option<VariantKind>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantKind {
    Web,
    Web4Core,
    WebGpu,
}

#[derive(Debug, Serialize, Default)]
pub struct AutomaticLeaveConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_room_timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub noone_joined_timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub everyone_left_timeout: Option<EveryoneLeftConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_call_not_recording_timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_call_recording_timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording_permission_denied_timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_detection: Option<SilenceDetectionConfig>,
}

#[derive(Debug, Serialize)]
pub struct EveryoneLeftConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activate_after: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct SilenceDetectionConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
    /// Seconds after recording starts before silence detection activates.
    /// Min: 1. Set low so silence is detected promptly after video ends.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activate_after: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct OutputMedia {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<OutputMediaConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshare: Option<OutputMediaConfig>,
}

#[derive(Debug, Serialize)]
pub struct OutputMediaConfig {
    pub kind: OutputMediaKind,
    pub config: OutputMediaWebpageConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputMediaKind {
    Webpage,
}

#[derive(Debug, Serialize)]
pub struct OutputMediaWebpageConfig {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct TranscriptionOptions {
    pub provider: TranscriptionProvider,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionProvider {
    MeetingCaptions,
    Gladia,
    AssemblyAi,
    Deepgram,
}

#[derive(Debug, Serialize)]
pub struct RealTimeTranscriptionConfig {
    pub destination_url: String,
    pub partial_results: bool,
}

#[derive(Debug, Deserialize)]
pub struct Bot {
    pub id: String,
    pub status: BotStatus,
    #[serde(default)]
    pub metadata: Option<HashMap<String, Value>>,
}

#[derive(Debug, Deserialize)]
pub struct BotStatus {
    pub code: BotStatusCode,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BotStatusCode {
    Ready,
    JoiningCall,
    InCallNotRecording,
    InCallRecording,
    CallEnded,
    Fatal,
    #[serde(other)]
    Unknown,
}

// Webhook event sent to your status change webhook URL.
#[derive(Debug, Deserialize)]
pub struct BotStatusWebhook {
    pub event: String,
    pub data: BotStatusWebhookData,
}

#[derive(Debug, Deserialize)]
pub struct BotStatusWebhookData {
    pub bot_id: String,
    pub status: BotStatus,
}

// Payload sent to the real_time_transcription.destination_url.
#[derive(Debug, Deserialize)]
pub struct TranscriptWebhook {
    pub bot_id: String,
    pub transcript: TranscriptSegment,
}

#[derive(Debug, Deserialize)]
pub struct TranscriptSegment {
    pub speaker: String,
    pub words: Vec<TranscriptWord>,
    pub is_final: bool,
    pub original_transcript_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TranscriptWord {
    pub text: String,
    pub start_time: f64,
    pub end_time: f64,
}
