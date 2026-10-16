use anlg_http::HttpClient;

use crate::error::Error;
use crate::types::{
    ListConversationsResponse, PostMessageRequest, PostMessageResponse, SlackResponse,
    UpdateMessageRequest, UpdateMessageResponse,
};

// Nango's Slack provider base URL already includes `/api`.
const POST_MESSAGE_PATH: &str = "/chat.postMessage";
const UPDATE_MESSAGE_PATH: &str = "/chat.update";
const LIST_CONVERSATIONS_PATH: &str =
    "/conversations.list?exclude_archived=true&limit=200&types=public_channel,private_channel";

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
            .post(POST_MESSAGE_PATH, body, "application/json")
            .await
            .map_err(Error::Http)?;
        let response: SlackResponse<PostMessageResponse> = serde_json::from_slice(&bytes)?;
        response.into_result()
    }

    pub async fn update_message(
        &self,
        req: UpdateMessageRequest,
    ) -> Result<UpdateMessageResponse, Error> {
        let body = serde_json::to_vec(&req)?;
        let bytes = self
            .http
            .post(UPDATE_MESSAGE_PATH, body, "application/json")
            .await
            .map_err(Error::Http)?;
        let response: SlackResponse<UpdateMessageResponse> = serde_json::from_slice(&bytes)?;
        response.into_result()
    }

    pub async fn list_conversations(&self) -> Result<ListConversationsResponse, Error> {
        let bytes = self
            .http
            .get(LIST_CONVERSATIONS_PATH)
            .await
            .map_err(Error::Http)?;
        let response: SlackResponse<ListConversationsResponse> = serde_json::from_slice(&bytes)?;
        response.into_result()
    }
}
