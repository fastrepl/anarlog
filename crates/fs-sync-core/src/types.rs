use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FolderInfo {
    pub name: String,
    pub parent_folder_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListFoldersResult {
    pub folders: HashMap<String, FolderInfo>,
    pub session_folder_map: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ScanResult {
    pub files: HashMap<String, String>,
    pub dirs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CleanupTarget {
    Files {
        subdir: String,
        extension: String,
    },
    Dirs {
        subdir: String,
        marker_file: String,
    },
    FilesRecursive {
        subdir: String,
        marker_file: String,
        extension: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentSaveResult {
    pub path: String,
    pub attachment_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentInfo {
    pub attachment_id: String,
    pub path: String,
    pub extension: String,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetaParticipant {
    pub id: String,
    pub user_id: String,
    pub session_id: String,
    pub human_id: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetaData {
    pub id: String,
    pub user_id: String,
    pub created_at: Option<String>,
    pub title: Option<String>,
    pub event: Option<serde_json::Value>,
    pub event_id: Option<String>,
    pub participants: Vec<SessionMetaParticipant>,
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionMetaDataSerde {
    id: String,
    user_id: String,
    created_at: Option<String>,
    title: Option<String>,
    event: Option<serde_json::Value>,
    event_id: Option<String>,
    participants: Option<Vec<SessionMetaParticipant>>,
    tags: Option<Vec<String>>,
}

impl<'de> Deserialize<'de> for SessionMetaData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = SessionMetaDataSerde::deserialize(deserializer)?;

        Ok(Self {
            id: value.id,
            user_id: value.user_id,
            created_at: value.created_at,
            title: value.title,
            event: value.event,
            event_id: value.event_id,
            participants: value.participants.unwrap_or_default(),
            tags: value.tags.unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TranscriptWord {
    pub id: Option<String>,
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub channel: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TranscriptSpeakerHint {
    #[serde(default)]
    pub id: Option<String>,
    pub word_id: String,
    #[serde(rename = "type")]
    pub hint_type: String,
    #[serde(default)]
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TranscriptEntry {
    pub id: String,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    pub session_id: String,
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub ended_at: Option<i64>,
    pub words: Vec<TranscriptWord>,
    #[serde(default)]
    pub speaker_hints: Vec<TranscriptSpeakerHint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TranscriptData {
    pub transcripts: Vec<TranscriptEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionNoteData {
    pub id: String,
    pub session_id: String,
    pub template_id: Option<String>,
    pub position: Option<i64>,
    pub title: Option<String>,
    pub tiptap_json: serde_json::Value,
    pub markdown: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionContentData {
    pub session_id: String,
    pub meta: Option<SessionMetaData>,
    pub raw_memo_tiptap_json: Option<serde_json::Value>,
    pub raw_memo_markdown: Option<String>,
    pub transcript: Option<TranscriptData>,
    pub notes: Vec<SessionNoteData>,
}
