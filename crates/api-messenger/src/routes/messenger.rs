use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::{MessengerError, Result};

#[derive(Debug, Deserialize)]
#[serde(tag = "platform", rename_all = "lowercase")]
pub enum SendMessageRequest {
    Slack(SlackSendRequest),
    Teams(TeamsSendRequest),
}

#[derive(Debug, Deserialize)]
pub struct SlackSendRequest {
    pub channel: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub blocks: Option<serde_json::Value>,
    #[serde(default)]
    pub thread_ts: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TeamsSendRequest {
    pub team_id: String,
    pub channel_id: String,
    pub content: String,
    #[serde(default)]
    pub content_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub platform: String,
    pub message_id: String,
    pub channel: String,
}

pub async fn send_message(
    Json(payload): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>> {
    match payload {
        SendMessageRequest::Slack(req) => {
            if req.text.is_none() && req.blocks.is_none() {
                return Err(MessengerError::BadRequest(
                    "either text or blocks must be provided".into(),
                ));
            }

            let slack_req = hypr_slack_web::PostMessageRequest {
                channel: req.channel.clone(),
                text: req.text,
                blocks: req.blocks,
                attachments: None,
                thread_ts: req.thread_ts,
                reply_broadcast: None,
                mrkdwn: None,
                unfurl_links: None,
                unfurl_media: None,
                metadata: None,
                username: None,
                icon_url: None,
                icon_emoji: None,
            };

            let _ = slack_req;

            Err(MessengerError::Internal(
                "slack client not configured in app state".into(),
            ))
        }
        SendMessageRequest::Teams(req) => {
            let teams_req = hypr_teems::SendMessageRequest {
                body: hypr_teems::MessageBody {
                    content: req.content,
                    content_type: req.content_type,
                },
            };

            let _ = teams_req;

            Err(MessengerError::Internal(
                "teams client not configured in app state".into(),
            ))
        }
    }
}
