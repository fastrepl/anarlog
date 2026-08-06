use anlg_http::HttpClient;

use crate::error::Error;
use crate::types::{
    ListConversationsResponse, PostMessageRequest, PostMessageResponse, SlackResponse,
};

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

    pub async fn list_conversations(&self) -> Result<ListConversationsResponse, Error> {
        let bytes = self
            .http
            .get(
                "/api/conversations.list?exclude_archived=true&limit=200&types=public_channel,private_channel",
            )
            .await
            .map_err(Error::Http)?;
        let response: SlackResponse<ListConversationsResponse> = serde_json::from_slice(&bytes)?;
        response.into_result()
    }
}
