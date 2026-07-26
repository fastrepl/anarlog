use hypr_http::HttpClient;

use crate::error::Error;
use crate::types::{SendMessageRequest, SendMessageResponse, parse_response};

pub struct TeamsClient<C> {
    http: C,
}

impl<C: HttpClient> TeamsClient<C> {
    pub fn new(http: C) -> Self {
        Self { http }
    }

    pub async fn send_message(
        &self,
        team_id: &str,
        channel_id: &str,
        req: SendMessageRequest,
    ) -> Result<SendMessageResponse, Error> {
        let path = format!("/v1.0/teams/{}/channels/{}/messages", team_id, channel_id);
        let body = serde_json::to_vec(&req)?;
        let bytes = self
            .http
            .post(&path, body, "application/json")
            .await
            .map_err(Error::Http)?;
        parse_response(&bytes)
    }
}
