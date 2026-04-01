use axum::{Json, http::StatusCode};

pub async fn callback(Json(_payload): Json<serde_json::Value>) -> StatusCode {
    StatusCode::OK
}
