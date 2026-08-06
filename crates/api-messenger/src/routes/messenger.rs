use anlg_api_nango::{NangoConnection, Slack};
use anlg_slack_web::{PostMessageRequest, SlackWebClient};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::{MessengerError, Result};

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SlackSendRequest {
    pub channel: String,
    pub text: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SlackChannel {
    pub id: String,
    pub name: String,
    pub is_private: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SlackChannelsResponse {
    pub channels: Vec<SlackChannel>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SendMessageResponse {
    pub message_id: String,
    pub channel: String,
}

#[utoipa::path(
    get,
    path = "/slack/channels",
    operation_id = "list_slack_channels",
    responses(
        (status = 200, description = "Slack channels available to the connected account", body = SlackChannelsResponse),
        (status = 401, description = "Authentication required"),
        (status = 500, description = "Slack connection unavailable"),
    ),
    tag = "messenger",
)]
pub async fn list_slack_channels(
    nango: NangoConnection<Slack>,
) -> Result<Json<SlackChannelsResponse>> {
    let mut channels = SlackWebClient::new(nango.into_http())
        .list_conversations()
        .await?
        .channels
        .into_iter()
        .filter(|channel| channel.is_member)
        .map(|channel| SlackChannel {
            id: channel.id,
            name: channel.name,
            is_private: channel.is_private,
        })
        .collect::<Vec<_>>();
    channels.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(Json(SlackChannelsResponse { channels }))
}

#[utoipa::path(
    post,
    path = "/slack/messages",
    operation_id = "send_slack_message",
    request_body = SlackSendRequest,
    responses(
        (status = 200, description = "Slack message sent", body = SendMessageResponse),
        (status = 400, description = "Invalid Slack message"),
        (status = 401, description = "Authentication required"),
        (status = 500, description = "Slack delivery unavailable"),
    ),
    tag = "messenger",
)]
pub async fn send_slack_message(
    nango: NangoConnection<Slack>,
    Json(payload): Json<SlackSendRequest>,
) -> Result<Json<SendMessageResponse>> {
    let channel = required_text(&payload.channel, 80, "channel")?;
    let text = required_text(&payload.text, 40_000, "message")?;
    let response = SlackWebClient::new(nango.into_http())
        .post_message(PostMessageRequest {
            channel: channel.to_string(),
            text: Some(text.to_string()),
            blocks: None,
            attachments: None,
            thread_ts: None,
            reply_broadcast: None,
            mrkdwn: Some(true),
            unfurl_links: Some(false),
            unfurl_media: Some(false),
            metadata: None,
            username: None,
            icon_url: None,
            icon_emoji: None,
        })
        .await?;

    Ok(Json(SendMessageResponse {
        message_id: response.ts,
        channel: response.channel,
    }))
}

fn required_text<'a>(value: &'a str, max_bytes: usize, label: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > max_bytes
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(MessengerError::BadRequest(format!("invalid {label}")));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::required_text;

    #[test]
    fn validates_slack_delivery_fields() {
        assert_eq!(required_text(" C123 ", 80, "channel").unwrap(), "C123");
        assert!(required_text("", 80, "channel").is_err());
        assert!(required_text("hello\0world", 40_000, "message").is_err());
    }
}
