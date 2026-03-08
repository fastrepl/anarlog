use hypr_http::HttpClient;

use crate::error::Error;
use crate::types::{PostMessageRequest, PostMessageResponse, SlackResponse};

pub struct SlackWebClient<C> {
    http: C,
}

impl<C: HttpClient> SlackWebClient<C> {
    pub fn new(http: C) -> Self {
        Self { http }
    }

    pub async fn post_message(
        &self,
        req: PostMessageRequest,
    ) -> Result<PostMessageResponse, Error> {
        let body = serde_json::to_vec(&req)?;
        let bytes = self
            .http
            .post("/api/chat.postMessage", body, "application/json")
            .await
            .map_err(Error::Http)?;
        let response: SlackResponse<PostMessageResponse> = serde_json::from_slice(&bytes)?;
        response.into_result()
    }
}
