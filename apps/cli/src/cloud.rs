use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anlg_agent_access::{
    GetMeetingInput, GetMeetingTranscriptInput, GetRecurringMeetingHistoryInput, ListMeetingsInput,
    Meeting, MeetingExport, MeetingPage, TranscriptPage,
};
use anlg_supabase_auth::session::Session;
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::{Error, Result, commands::auth};

const API_URL: &str = "https://api.anarlog.so";

pub(crate) struct CloudClient {
    base_url: String,
    access_token: String,
    client: reqwest::Client,
}

impl CloudClient {
    pub(crate) fn current() -> Result<Self> {
        let session = auth::current_session()?.ok_or_else(|| {
            Error::cloud(
                "unauthorized",
                "Sign in with `anarlog auth login` before using --source cloud.",
            )
        })?;
        Self::from_session(API_URL, session)
    }

    fn from_session(base_url: &str, session: Session) -> Result<Self> {
        if session
            .expires_at
            .is_none_or(|expires_at| expires_at <= unix_timestamp())
        {
            return Err(Error::cloud(
                "unauthorized",
                "The Anarlog session has expired; open Anarlog or run `anarlog auth login` again.",
            ));
        }
        Self::new(base_url, session.access_token)
    }

    fn new(base_url: &str, access_token: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(format!("anarlog-cli/{}", crate::VERSION))
            .build()
            .map_err(|error| Error::operation("create Cloud API client", error.to_string()))?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            access_token,
            client,
        })
    }

    pub(crate) async fn list_meetings(&self, input: ListMeetingsInput) -> Result<MeetingPage> {
        self.get_json("/v1/meetings", Some(&input), "list Cloud meetings")
            .await
    }

    pub(crate) async fn get_meeting(&self, input: GetMeetingInput) -> Result<Meeting> {
        self.get_json(
            &format!("/v1/meetings/{}", encode_path(&input.meeting_id)),
            None::<&()>,
            "get Cloud meeting",
        )
        .await
    }

    pub(crate) async fn get_meeting_transcript(
        &self,
        input: GetMeetingTranscriptInput,
    ) -> Result<TranscriptPage> {
        self.get_json(
            &format!("/v1/meetings/{}/transcript", encode_path(&input.meeting_id)),
            Some(&TranscriptQuery {
                offset: input.offset,
                limit: input.limit,
            }),
            "get Cloud transcript",
        )
        .await
    }

    pub(crate) async fn get_recurring_meeting_history(
        &self,
        input: GetRecurringMeetingHistoryInput,
    ) -> Result<MeetingPage> {
        self.get_json(
            &format!("/v1/meetings/{}/history", encode_path(&input.meeting_id)),
            Some(&HistoryQuery {
                limit: input.limit,
                offset: input.offset,
            }),
            "get Cloud meeting history",
        )
        .await
    }

    pub(crate) async fn get_meeting_export(&self, meeting_id: String) -> Result<MeetingExport> {
        self.get_json(
            &format!("/v1/meetings/{}/export", encode_path(&meeting_id)),
            None::<&()>,
            "export Cloud meeting",
        )
        .await
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&impl serde::Serialize>,
        action: &'static str,
    ) -> Result<T> {
        let mut request = self
            .client
            .get(format!("{}{path}", self.base_url))
            .bearer_auth(&self.access_token);
        if let Some(query) = query {
            request = request.query(query);
        }
        let response = request
            .send()
            .await
            .map_err(|error| Error::operation(action, error.to_string()))?;
        decode(response, action).await
    }
}

#[derive(serde::Serialize)]
struct TranscriptQuery {
    offset: Option<u32>,
    limit: Option<u32>,
}

#[derive(serde::Serialize)]
struct HistoryQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(serde::Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(serde::Deserialize)]
struct ErrorBody {
    code: String,
    message: String,
}

async fn decode<T: DeserializeOwned>(response: Response, action: &'static str) -> Result<T> {
    let status = response.status();
    if status.is_success() {
        return response
            .json()
            .await
            .map_err(|error| Error::operation(action, error.to_string()));
    }

    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response.text().await.unwrap_or_default();
    if let Ok(envelope) = serde_json::from_str::<ErrorEnvelope>(&body) {
        if envelope.error.code == "not_found" {
            return Err(Error::NotFound(
                envelope
                    .error
                    .message
                    .strip_suffix(" not found")
                    .unwrap_or(&envelope.error.message)
                    .to_string(),
            ));
        }
        return Err(Error::cloud(
            cloud_error_code(&envelope.error.code),
            envelope.error.message,
        ));
    }

    if status == StatusCode::TOO_MANY_REQUESTS {
        let suffix = retry_after
            .map(|seconds| format!("; retry after {seconds} seconds"))
            .unwrap_or_default();
        return Err(Error::cloud(
            "rate_limited",
            format!("Cloud API rate limit exceeded{suffix}"),
        ));
    }

    Err(Error::cloud(
        "cloud_api_error",
        format!("Cloud API returned HTTP {}", status.as_u16()),
    ))
}

fn cloud_error_code(code: &str) -> &'static str {
    match code {
        "unauthorized" => "unauthorized",
        "insufficient_scope" => "insufficient_scope",
        "subscription_required" => "subscription_required",
        "cloud_api_not_enabled" => "cloud_api_not_enabled",
        "invalid_request" => "invalid_request",
        "internal_error" => "internal_error",
        _ => "cloud_api_error",
    }
}

fn encode_path(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path, query_param},
    };

    #[tokio::test]
    async fn list_uses_bearer_auth_and_preserves_query_pagination() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/meetings"))
            .and(header("authorization", "Bearer access-token"))
            .and(query_param("query", "planning"))
            .and(query_param("limit", "10"))
            .and(query_param("offset", "20"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "meetings": [{
                    "id": "meeting-1",
                    "title": "Planning",
                    "kind": "meeting",
                    "status": "completed",
                    "created_at": "2026-09-01",
                    "updated_at": "2026-09-01",
                    "started_at": "2026-09-01",
                    "ended_at": "",
                    "series_id": ""
                }],
                "pagination": {
                    "offset": 20,
                    "limit": 10,
                    "returned": 1,
                    "total": null,
                    "next_offset": null
                }
            })))
            .expect(1)
            .mount(&server)
            .await;
        let client = CloudClient::new(&server.uri(), "access-token".to_string()).unwrap();

        let page = client
            .list_meetings(ListMeetingsInput {
                query: Some("planning".to_string()),
                series_id: None,
                limit: Some(10),
                offset: Some(20),
            })
            .await
            .unwrap();

        assert_eq!(page.meetings[0].id, "meeting-1");
        assert_eq!(page.pagination.offset, 20);
    }

    #[tokio::test]
    async fn export_reads_the_shared_meeting_export_contract() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/meetings/meeting%2F1/export"))
            .respond_with(ResponseTemplate::new(200).set_body_json(export_json()))
            .expect(1)
            .mount(&server)
            .await;
        let client = CloudClient::new(&server.uri(), "access-token".to_string()).unwrap();

        let export = client
            .get_meeting_export("meeting/1".to_string())
            .await
            .unwrap();

        assert_eq!(export.meeting.id, "meeting/1");
        assert_eq!(export.transcripts[0].text, "Launch Friday");
    }

    #[tokio::test]
    async fn cloud_errors_keep_stable_codes_and_retry_guidance() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/meetings/missing"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "error": {
                    "code": "not_found",
                    "message": "meeting 'missing' not found"
                }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/meetings"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("retry-after", "2")
                    .set_body_string("rate limit exceeded"),
            )
            .mount(&server)
            .await;
        let client = CloudClient::new(&server.uri(), "access-token".to_string()).unwrap();

        let missing = client
            .get_meeting(GetMeetingInput {
                meeting_id: "missing".to_string(),
            })
            .await
            .unwrap_err();
        assert_eq!(missing.code(), "not_found");
        assert_eq!(missing.to_string(), "meeting 'missing' not found");

        let limited = client
            .list_meetings(ListMeetingsInput::default())
            .await
            .unwrap_err();
        assert_eq!(limited.code(), "rate_limited");
        assert!(limited.to_string().contains("retry after 2 seconds"));
    }

    fn export_json() -> serde_json::Value {
        serde_json::json!({
            "id": "meeting/1",
            "title": "Planning",
            "kind": "meeting",
            "status": "completed",
            "created_at": "2026-09-01",
            "updated_at": "2026-09-01",
            "started_at": "2026-09-01",
            "ended_at": "",
            "timezone": "Asia/Seoul",
            "language": "en",
            "series_id": "",
            "note": null,
            "summaries": [],
            "participants": [],
            "action_items": [],
            "transcripts": [{
                "id": "transcript-1",
                "source": "mic",
                "provider": "local",
                "model": "local",
                "language": "en",
                "started_at_ms": 0,
                "ended_at_ms": null,
                "memo": "",
                "text": "Launch Friday",
                "words": [],
                "speaker_hints": []
            }]
        })
    }
}
