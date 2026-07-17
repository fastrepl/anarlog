use serde::{Deserialize, Serialize};

// https://loops.so/docs/api-reference/send-event#body
#[derive(Debug, Deserialize, Serialize)]
pub struct Event {
    #[serde(rename = "eventName")]
    pub name: String,
    #[serde(rename = "eventProperties", skip_serializing_if = "Option::is_none")]
    pub properties: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Response {
    Success { success: bool },
    Error { success: bool, message: String },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteContactResponse {
    pub success: bool,
    #[serde(default)]
    pub message: Option<String>,
}
