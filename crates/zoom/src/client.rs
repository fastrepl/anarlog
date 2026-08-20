use anlg_http::HttpClient;
use chrono::NaiveDate;

use crate::error::Error;
use crate::types::{ListRecordingsResponse, MeetingSummary};

pub struct ZoomClient<C> {
    http: C,
}

impl<C: HttpClient> ZoomClient<C> {
    pub fn new(http: C) -> Self {
        Self { http }
    }

    pub async fn list_user_recordings(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        next_page_token: Option<&str>,
    ) -> Result<ListRecordingsResponse, Error> {
        let mut path = format!("/users/me/recordings?page_size=300&from={from}&to={to}");
        if let Some(token) = next_page_token.filter(|token| !token.is_empty()) {
            path.push_str("&next_page_token=");
            path.push_str(&urlencoding::encode(token));
        }

        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn get_meeting_summary(
        &self,
        meeting_id: &str,
    ) -> Result<Option<MeetingSummary>, Error> {
        let path = format!(
            "/meetings/{}/meeting_summary",
            urlencoding::encode(meeting_id)
        );
        match self.http.get(&path).await {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(_) => Ok(None),
        }
    }
}
