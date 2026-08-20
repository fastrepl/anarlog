use anlg_http::HttpClient;
use serde::Deserialize;

use crate::error::Error;
use crate::json::{ImportedMeeting, TranscriptSegment, nonempty};
use crate::time::hhmmss_to_ms;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListMeetingsResponse {
    #[serde(default)]
    pub items: Vec<FathomMeeting>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FathomMeeting {
    #[serde(default)]
    pub recording_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub meeting_title: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub share_url: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub scheduled_start_time: Option<String>,
    #[serde(default)]
    pub recording_start_time: Option<String>,
    #[serde(default)]
    pub recorded_by: Option<FathomUser>,
    #[serde(default)]
    pub action_items: Vec<FathomActionItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FathomUser {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FathomActionItem {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TranscriptResponse {
    #[serde(default)]
    transcript: Vec<TranscriptItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TranscriptItem {
    #[serde(default)]
    speaker: Option<TranscriptSpeaker>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TranscriptSpeaker {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SummaryResponse {
    #[serde(default)]
    markdown: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    default_summary: Option<DefaultSummary>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct DefaultSummary {
    #[serde(default)]
    markdown: Option<String>,
}

pub struct FathomClient<C> {
    http: C,
}

impl<C: HttpClient> FathomClient<C> {
    pub fn new(http: C) -> Self {
        Self { http }
    }

    pub async fn list_meetings(
        &self,
        created_after: &str,
        cursor: Option<&str>,
    ) -> Result<ListMeetingsResponse, Error> {
        let mut path = format!(
            "/external/v1/meetings?created_after={}",
            urlencoding::encode(created_after)
        );
        if let Some(cursor) = cursor.filter(|cursor| !cursor.is_empty()) {
            path.push_str("&cursor=");
            path.push_str(&urlencoding::encode(cursor));
        }
        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn get_transcript(
        &self,
        recording_id: &str,
    ) -> Result<Vec<TranscriptSegment>, Error> {
        let path = format!(
            "/external/v1/recordings/{}/transcript",
            urlencoding::encode(recording_id)
        );
        match self.http.get(&path).await {
            Ok(bytes) => Ok(parse_transcript(&serde_json::from_slice(&bytes)?)),
            Err(_) => Ok(Vec::new()),
        }
    }

    pub async fn get_summary(&self, recording_id: &str) -> Result<Option<String>, Error> {
        let path = format!(
            "/external/v1/recordings/{}/summary",
            urlencoding::encode(recording_id)
        );
        match self.http.get(&path).await {
            Ok(bytes) => Ok(summary_text(&serde_json::from_slice(&bytes)?)),
            Err(_) => Ok(None),
        }
    }
}

impl FathomMeeting {
    pub fn recording_id(&self) -> Option<&str> {
        self.recording_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn imported(
        &self,
        summary: Option<String>,
        transcript: Vec<TranscriptSegment>,
    ) -> Option<ImportedMeeting> {
        let id = self.recording_id()?.to_string();
        let title = nonempty(self.title.as_deref())
            .or_else(|| nonempty(self.meeting_title.as_deref()))
            .unwrap_or_else(|| "Fathom meeting".to_string());
        Some(ImportedMeeting {
            id,
            title,
            start_time: nonempty(self.recording_start_time.as_deref())
                .or_else(|| nonempty(self.scheduled_start_time.as_deref()))
                .or_else(|| nonempty(self.created_at.as_deref())),
            url: nonempty(self.share_url.as_deref()).or_else(|| nonempty(self.url.as_deref())),
            summary,
            notes: None,
            transcript,
            action_items: self
                .action_items
                .iter()
                .filter_map(|item| {
                    nonempty(item.description.as_deref()).or_else(|| nonempty(item.text.as_deref()))
                })
                .collect(),
        })
    }
}

fn parse_transcript(response: &TranscriptResponse) -> Vec<TranscriptSegment> {
    response
        .transcript
        .iter()
        .filter_map(|item| {
            let text = nonempty(item.text.as_deref())?;
            let start_ms = item.timestamp.as_deref().map(hhmmss_to_ms).unwrap_or(0);
            Some(TranscriptSegment {
                speaker: item
                    .speaker
                    .as_ref()
                    .and_then(|speaker| {
                        nonempty(speaker.display_name.as_deref())
                            .or_else(|| nonempty(speaker.name.as_deref()))
                    })
                    .unwrap_or_default(),
                text,
                start_ms,
                end_ms: start_ms,
            })
        })
        .collect()
}

fn summary_text(response: &SummaryResponse) -> Option<String> {
    nonempty(response.markdown.as_deref())
        .or_else(|| {
            response
                .default_summary
                .as_ref()
                .and_then(|summary| nonempty(summary.markdown.as_deref()))
        })
        .or_else(|| nonempty(response.summary.as_deref()))
}
