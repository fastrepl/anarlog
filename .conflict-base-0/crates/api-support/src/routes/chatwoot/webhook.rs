use axum::{Json, http::StatusCode};

fn message_id(payload: &serde_json::Value) -> Option<String> {
    payload.get("id").and_then(|value| {
        value
            .as_str()
            .map(ToOwned::to_owned)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
            .or_else(|| value.as_u64().map(|value| value.to_string()))
    })
}

async fn handle(route: &'static str, payload: serde_json::Value) -> StatusCode {
    tracing::info!(
        route,
        event = payload.get("event").and_then(serde_json::Value::as_str),
        message_id = message_id(&payload),
        "chatwoot webhook received"
    );

    StatusCode::OK
}

pub async fn webhook(Json(payload): Json<serde_json::Value>) -> StatusCode {
    handle("webhook", payload).await
}

pub async fn callback(Json(payload): Json<serde_json::Value>) -> StatusCode {
    handle("callback", payload).await
}
