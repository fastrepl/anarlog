use axum::{
    body::Body,
    extract::{Path, Query, State},
    response::Response,
};
use serde::Deserialize;

use crate::chatwoot;
use crate::error::SupportError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ConversationEventsQuery {
    pub pubsub_token: String,
}

pub async fn conversation_events(
    State(state): State<AppState>,
    Path(_conversation_id): Path<i64>,
    Query(params): Query<ConversationEventsQuery>,
) -> Result<Response, SupportError> {
    let ws_url = chatwoot::ws_url(&state.config.chatwoot.chatwoot_base_url);

    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .map_err(|e| SupportError::Chatwoot(format!("websocket connect failed: {e}")))?;

    let (mut ws_write, mut ws_read) = futures_util::StreamExt::split(ws_stream);

    let identifier = chatwoot::action_cable_identifier(&params.pubsub_token);

    let subscribe_cmd = serde_json::json!({
        "command": "subscribe",
        "identifier": identifier,
    });

    use futures_util::SinkExt;
    ws_write
        .send(tokio_tungstenite::tungstenite::Message::Text(
            subscribe_cmd.to_string().into(),
        ))
        .await
        .map_err(|e| SupportError::Chatwoot(format!("websocket subscribe failed: {e}")))?;

    let ping_identifier = identifier.clone();
    let ws_write = std::sync::Arc::new(tokio::sync::Mutex::new(ws_write));
    let ping_writer = ws_write.clone();

    let ping_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let msg = serde_json::json!({
                "command": "message",
                "identifier": ping_identifier,
                "data": serde_json::json!({"action": "update_presence"}).to_string(),
            });
            let mut writer = ping_writer.lock().await;
            if writer
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    msg.to_string().into(),
                ))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let output_stream = async_stream::stream! {
        use futures_util::StreamExt;

        while let Some(msg_result) = ws_read.next().await {
            let text = match msg_result {
                Ok(tokio_tungstenite::tungstenite::Message::Text(t)) => t,
                Ok(_) => continue,
                Err(_) => break,
            };

            if let Ok(agent_msg) = text.parse::<chatwoot::AgentMessage>() {
                let sse_line = format!("data: {}\n\n", serde_json::to_string(&agent_msg).unwrap());
                yield Ok::<_, std::io::Error>(sse_line);
            }
        }

        ping_task.abort();
    };

    let body = Body::from_stream(output_stream);
    let response = Response::builder()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .body(body)
        .unwrap();

    Ok(response)
}
