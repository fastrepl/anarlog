use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionalEmail {
    pub email: String,
    pub transactional_id: String,
    pub data_variables: std::collections::HashMap<String, String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TransactionalSendOutcome {
    Sent,
    AlreadySent,
}

// https://loops.so/docs/api-reference/send-event#body
#[derive(Debug, Deserialize, Serialize)]
pub struct Event {
    #[serde(rename = "eventName")]
    pub name: String,
    #[serde(rename = "eventProperties", skip_serializing_if = "Option::is_none")]
    pub properties: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Response {
    pub success: bool,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteContactResponse {
    pub success: bool,
    #[serde(default)]
    pub message: Option<String>,
}
