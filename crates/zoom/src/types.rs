use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListRecordingsResponse {
    #[serde(default)]
    pub meetings: Vec<RecordingMeeting>,
    #[serde(default)]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecordingMeeting {
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub share_url: Option<String>,
    #[serde(default)]
    pub recording_files: Vec<RecordingFile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecordingFile {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub file_type: Option<String>,
    #[serde(default)]
    pub file_extension: Option<String>,
    #[serde(default)]
    pub recording_type: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeetingSummary {
    #[serde(default)]
    pub meeting_host_email: Option<String>,
    #[serde(default)]
    pub summary_title: Option<String>,
    #[serde(default)]
    pub summary_overview: Option<String>,
    #[serde(default)]
    pub edited_summary: Option<String>,
    #[serde(default)]
    pub summary_details: Vec<SummaryDetail>,
    #[serde(default)]
    pub next_steps: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SummaryDetail {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TranscriptSegment {
    pub speaker: String,
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

impl RecordingMeeting {
    pub fn meeting_id(&self) -> Option<String> {
        match &self.id {
            Some(serde_json::Value::String(value)) if !value.is_empty() => Some(value.clone()),
            Some(serde_json::Value::Number(value)) => Some(value.to_string()),
            _ => None,
        }
    }

    pub fn external_id(&self) -> Option<String> {
        self.uuid
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| self.meeting_id())
    }

    pub fn transcript_file(&self) -> Option<&RecordingFile> {
        self.recording_files.iter().find(|file| {
            let file_type = file.file_type.as_deref().unwrap_or("");
            let recording_type = file.recording_type.as_deref().unwrap_or("");
            let extension = file.file_extension.as_deref().unwrap_or("");
            file_type.eq_ignore_ascii_case("TRANSCRIPT")
                || recording_type.eq_ignore_ascii_case("audio_transcript")
                || extension.eq_ignore_ascii_case("VTT")
        })
    }
}
