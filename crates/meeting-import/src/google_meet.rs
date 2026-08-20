use anlg_http::HttpClient;
use serde::Deserialize;

use crate::error::Error;
use crate::json::{ImportedMeeting, TranscriptSegment, nonempty};
use crate::time::{duration_or_timestamp_to_ms, rfc3339_to_ms};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListConferenceRecordsResponse {
    #[serde(default, rename = "conferenceRecords")]
    pub conference_records: Vec<ConferenceRecord>,
    #[serde(default, rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConferenceRecord {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "startTime")]
    pub start_time: Option<String>,
    #[serde(default)]
    pub space: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListTranscriptsResponse {
    #[serde(default)]
    pub transcripts: Vec<MeetTranscript>,
    #[serde(default, rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MeetTranscript {
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListEntriesResponse {
    #[serde(default, rename = "transcriptEntries")]
    pub transcript_entries: Vec<TranscriptEntry>,
    #[serde(default, rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TranscriptEntry {
    #[serde(default)]
    pub participant: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default, rename = "startTime")]
    pub start_time: Option<String>,
    #[serde(default, rename = "endTime")]
    pub end_time: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListParticipantsResponse {
    #[serde(default)]
    pub participants: Vec<Participant>,
    #[serde(default, rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Participant {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "signedinUser")]
    pub signed_in_user: Option<NamedUser>,
    #[serde(default, rename = "anonymousUser")]
    pub anonymous_user: Option<NamedUser>,
    #[serde(default, rename = "phoneUser")]
    pub phone_user: Option<NamedUser>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NamedUser {
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Space {
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(default, rename = "meetingCode")]
    pub meeting_code: Option<String>,
}

pub struct GoogleMeetClient<C> {
    http: C,
}

impl<C: HttpClient> GoogleMeetClient<C> {
    pub fn new(http: C) -> Self {
        Self { http }
    }

    pub async fn list_conference_records(
        &self,
        page_token: Option<&str>,
    ) -> Result<ListConferenceRecordsResponse, Error> {
        let mut path = "/v2/conferenceRecords?pageSize=50".to_string();
        if let Some(token) = page_token.filter(|token| !token.is_empty()) {
            path.push_str("&pageToken=");
            path.push_str(&urlencoding::encode(token));
        }
        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn list_transcripts(
        &self,
        conference_name: &str,
        page_token: Option<&str>,
    ) -> Result<ListTranscriptsResponse, Error> {
        let mut path = format!("/v2/{conference_name}/transcripts?pageSize=50");
        if let Some(token) = page_token.filter(|token| !token.is_empty()) {
            path.push_str("&pageToken=");
            path.push_str(&urlencoding::encode(token));
        }
        match self.http.get(&path).await {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(_) => Ok(ListTranscriptsResponse::default()),
        }
    }

    pub async fn list_entries(
        &self,
        transcript_name: &str,
        page_token: Option<&str>,
    ) -> Result<ListEntriesResponse, Error> {
        let mut path = format!("/v2/{transcript_name}/entries?pageSize=100");
        if let Some(token) = page_token.filter(|token| !token.is_empty()) {
            path.push_str("&pageToken=");
            path.push_str(&urlencoding::encode(token));
        }
        match self.http.get(&path).await {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(_) => Ok(ListEntriesResponse::default()),
        }
    }

    pub async fn list_participants(
        &self,
        conference_name: &str,
        page_token: Option<&str>,
    ) -> Result<ListParticipantsResponse, Error> {
        let mut path = format!("/v2/{conference_name}/participants?pageSize=100");
        if let Some(token) = page_token.filter(|token| !token.is_empty()) {
            path.push_str("&pageToken=");
            path.push_str(&urlencoding::encode(token));
        }
        match self.http.get(&path).await {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(_) => Ok(ListParticipantsResponse::default()),
        }
    }

    pub async fn get_space(&self, space_name: &str) -> Result<Option<Space>, Error> {
        match self.http.get(&format!("/v2/{space_name}")).await {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(_) => Ok(None),
        }
    }
}

impl ConferenceRecord {
    pub fn resource_name(&self) -> Option<&str> {
        self.name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn imported(
        &self,
        title: String,
        transcript: Vec<TranscriptSegment>,
    ) -> Option<ImportedMeeting> {
        let id = self.resource_name()?.to_string();
        Some(ImportedMeeting {
            id,
            title,
            start_time: nonempty(self.start_time.as_deref()),
            url: None,
            summary: None,
            notes: None,
            transcript,
            action_items: Vec::new(),
        })
    }
}

impl Participant {
    pub fn display_name(&self) -> Option<String> {
        self.signed_in_user
            .as_ref()
            .and_then(|user| nonempty(user.display_name.as_deref()))
            .or_else(|| {
                self.anonymous_user
                    .as_ref()
                    .and_then(|user| nonempty(user.display_name.as_deref()))
            })
            .or_else(|| {
                self.phone_user
                    .as_ref()
                    .and_then(|user| nonempty(user.display_name.as_deref()))
            })
    }
}

pub fn entry_segment(
    entry: &TranscriptEntry,
    origin_ms: u64,
    speaker: String,
) -> Option<TranscriptSegment> {
    let text = nonempty(entry.text.as_deref())?;
    let start_ms = entry
        .start_time
        .as_deref()
        .map(|value| duration_or_timestamp_to_ms(value, origin_ms))
        .unwrap_or(0);
    let end_ms = entry
        .end_time
        .as_deref()
        .map(|value| duration_or_timestamp_to_ms(value, origin_ms))
        .unwrap_or(start_ms);
    Some(TranscriptSegment {
        speaker,
        text,
        start_ms,
        end_ms,
    })
}

pub fn conference_origin_ms(record: &ConferenceRecord) -> u64 {
    record
        .start_time
        .as_deref()
        .and_then(rfc3339_to_ms)
        .unwrap_or(0)
}
