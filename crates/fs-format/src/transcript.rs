use serde_json::{Map, Value};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptJson {
    pub transcripts: Vec<TranscriptWithData>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptWithData {
    pub id: String,
    pub user_id: String,
    pub created_at: String,
    pub session_id: String,
    pub started_at: f64,
    pub ended_at: Option<f64>,
    pub memo_md: String,
    pub words: Vec<TranscriptWord>,
    pub speaker_hints: Vec<TranscriptSpeakerHint>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptWord {
    pub id: String,
    pub text: String,
    pub start_ms: f64,
    pub end_ms: f64,
    pub channel: f64,
    pub speaker: Option<String>,
    pub metadata: Option<Map<String, Value>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptSpeakerHint {
    pub id: String,
    pub word_id: String,
    #[serde(rename = "type")]
    pub hint_type: String,
    pub value: Map<String, Value>,
}
