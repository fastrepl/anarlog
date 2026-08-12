use anlg_http::HttpClient;

use crate::error::Error;
use crate::types::{
    ListConversationsResponse, PostMessageRequest, PostMessageResponse, SlackResponse,
};

// Nango's Slack provider base URL already includes `/api`.
const POST_MESSAGE_PATH: &str = "/chat.postMessage";
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

#[cfg(test)]
mod tests {
    use super::{LIST_CONVERSATIONS_PATH, POST_MESSAGE_PATH};

    #[test]
    fn endpoints_are_relative_to_nango_slack_api_base_url() {
        assert!(!POST_MESSAGE_PATH.starts_with("/api/"));
        assert!(!LIST_CONVERSATIONS_PATH.starts_with("/api/"));
    }
}
