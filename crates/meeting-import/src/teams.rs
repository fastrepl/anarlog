use anlg_http::HttpClient;
use serde::Deserialize;

use crate::error::Error;
use crate::json::{ImportedMeeting, nonempty};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GraphCollection<T> {
    #[serde(default)]
    pub value: Vec<T>,
    #[serde(default, rename = "@odata.nextLink")]
    pub next_link: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CalendarEvent {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub start: Option<DateTimeTimeZone>,
    #[serde(default, rename = "isOnlineMeeting")]
    pub is_online_meeting: Option<bool>,
    #[serde(default, rename = "onlineMeeting")]
    pub online_meeting: Option<OnlineMeetingInfo>,
    #[serde(default, rename = "webLink")]
    pub web_link: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DateTimeTimeZone {
    #[serde(default, rename = "dateTime")]
    pub date_time: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OnlineMeetingInfo {
    #[serde(default, rename = "joinUrl")]
    pub join_url: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OnlineMeeting {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default, rename = "joinWebUrl")]
    pub join_web_url: Option<String>,
    #[serde(default, rename = "startDateTime")]
    pub start_date_time: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CallTranscript {
    #[serde(default)]
    pub id: Option<String>,
}

pub struct TeamsClient<C> {
    http: C,
}

impl<C: HttpClient> TeamsClient<C> {
    pub fn new(http: C) -> Self {
        Self { http }
    }

    pub async fn list_calendar_view(
        &self,
        start: &str,
        end: &str,
        next_link: Option<&str>,
    ) -> Result<GraphCollection<CalendarEvent>, Error> {
        let path = if let Some(next_link) = next_link.filter(|value| !value.is_empty()) {
            graph_proxy_path(next_link).unwrap_or_else(|| calendar_view_path(start, end))
        } else {
            calendar_view_path(start, end)
        };
        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn find_online_meeting(
        &self,
        join_url: &str,
    ) -> Result<Option<OnlineMeeting>, Error> {
        let filter = format!("JoinWebUrl eq '{}'", join_url.replace('\'', "''"));
        let path = format!(
            "/v1.0/me/onlineMeetings?$filter={}",
            urlencoding::encode(&filter)
        );
        match self.http.get(&path).await {
            Ok(bytes) => {
                let page: GraphCollection<OnlineMeeting> = serde_json::from_slice(&bytes)?;
                Ok(page.value.into_iter().next())
            }
            Err(_) => Ok(None),
        }
    }

    pub async fn list_transcripts(&self, meeting_id: &str) -> Result<Vec<CallTranscript>, Error> {
        let path = format!(
            "/v1.0/me/onlineMeetings/{}/transcripts",
            urlencoding::encode(meeting_id)
        );
        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let page: GraphCollection<CallTranscript> = serde_json::from_slice(&bytes)?;
        Ok(page.value)
    }
}

impl CalendarEvent {
    pub fn join_url(&self) -> Option<&str> {
        self.online_meeting
            .as_ref()
            .and_then(|meeting| meeting.join_url.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn imported(
        &self,
        meeting: &OnlineMeeting,
        transcript: Vec<anlg_zoom::TranscriptSegment>,
    ) -> Option<ImportedMeeting> {
        let id = meeting
            .id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                self.id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            })?
            .to_string();
        let title = nonempty(self.subject.as_deref())
            .or_else(|| nonempty(meeting.subject.as_deref()))
            .unwrap_or_else(|| "Teams meeting".to_string());
        Some(ImportedMeeting {
            id,
            title,
            start_time: nonempty(meeting.start_date_time.as_deref()).or_else(|| {
                self.start
                    .as_ref()
                    .and_then(|start| nonempty(start.date_time.as_deref()))
            }),
            url: nonempty(meeting.join_web_url.as_deref())
                .or_else(|| nonempty(self.join_url()))
                .or_else(|| nonempty(self.web_link.as_deref())),
            summary: None,
            notes: None,
            transcript,
            action_items: Vec::new(),
        })
    }
}

fn calendar_view_path(start: &str, end: &str) -> String {
    format!(
        "/v1.0/me/calendarView?startDateTime={}&endDateTime={}&$top=50&$select=id,subject,start,isOnlineMeeting,onlineMeeting,webLink&$filter=isOnlineMeeting eq true",
        urlencoding::encode(start),
        urlencoding::encode(end)
    )
}

fn graph_proxy_path(next_link: &str) -> Option<String> {
    let url = url::Url::parse(next_link).ok()?;
    let path = url.path().to_string();
    match url.query() {
        Some(query) => Some(format!("{path}?{query}")),
        None => Some(path),
    }
}
