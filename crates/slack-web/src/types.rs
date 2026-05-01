use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Deserialize)]
pub(crate) struct SlackResponse<T> {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(flatten)]
    pub data: Option<T>,
}

impl<T> SlackResponse<T> {
    pub fn into_result(self) -> Result<T, Error> {
        if self.ok {
            self.data
                .ok_or_else(|| Error::SlackApi("missing response data".into()))
        } else {
            Err(Error::SlackApi(
                self.error.unwrap_or_else(|| "unknown error".into()),
            ))
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PostMessageRequest {
    pub channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_broadcast: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mrkdwn: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unfurl_links: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unfurl_media: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_emoji: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PostMessageResponse {
    pub channel: String,
    pub ts: String,
    pub message: Message,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub bot_id: Option<String>,
    #[serde(rename = "type")]
    #[serde(default)]
    pub message_type: Option<String>,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub ts: Option<String>,
}
