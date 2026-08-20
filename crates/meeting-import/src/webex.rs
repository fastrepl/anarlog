use anlg_http::HttpClient;
use serde::Deserialize;

use crate::error::Error;
use crate::json::{ImportedMeeting, nonempty};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListTranscriptsResponse {
    #[serde(default)]
    pub items: Vec<WebexTranscript>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WebexTranscript {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub meeting_id: Option<String>,
    #[serde(rename = "meetingId")]
    #[serde(default)]
    pub meeting_id_camel: Option<String>,
    #[serde(default)]
    pub meeting_topic: Option<String>,
    #[serde(rename = "meetingTopic")]
    #[serde(default)]
    pub meeting_topic_camel: Option<String>,
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(rename = "startTime")]
    #[serde(default)]
    pub start_time_camel: Option<String>,
    #[serde(default)]
    pub vtt_download_link: Option<String>,
    #[serde(rename = "vttDownloadLink")]
    #[serde(default)]
    pub vtt_download_link_camel: Option<String>,
}

pub struct WebexClient<C> {
    http: C,
}

impl<C: HttpClient> WebexClient<C> {
    pub fn new(http: C) -> Self {
        Self { http }
    }

    pub async fn list_transcripts(&self, offset: u32) -> Result<ListTranscriptsResponse, Error> {
        let path = format!("/v1/meetingTranscripts?max=100&offset={offset}");
        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

impl WebexTranscript {
    pub fn transcript_id(&self) -> Option<&str> {
        self.id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn vtt_download_link(&self) -> Option<&str> {
        self.vtt_download_link
            .as_deref()
            .or(self.vtt_download_link_camel.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn imported(
        &self,
        transcript: Vec<anlg_zoom::TranscriptSegment>,
    ) -> Option<ImportedMeeting> {
        let id = self.transcript_id()?.to_string();
        let title = nonempty(self.meeting_topic.as_deref())
            .or_else(|| nonempty(self.meeting_topic_camel.as_deref()))
            .unwrap_or_else(|| "Webex meeting".to_string());
        Some(ImportedMeeting {
            id,
            title,
            start_time: nonempty(self.start_time.as_deref())
                .or_else(|| nonempty(self.start_time_camel.as_deref())),
            url: None,
            summary: None,
            notes: None,
            transcript,
            action_items: Vec::new(),
        })
    }
}
