use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Deserialize)]
pub(crate) struct GraphErrorResponse {
    pub error: GraphError,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SendMessageRequest {
    pub body: MessageBody,
}

#[derive(Debug, Serialize)]
pub struct MessageBody {
    pub content: String,
    #[serde(rename = "contentType")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageResponse {
    pub id: String,
    #[serde(rename = "createdDateTime")]
    #[serde(default)]
    pub created_date_time: Option<String>,
    #[serde(rename = "from")]
    #[serde(default)]
    pub from: Option<IdentitySet>,
    #[serde(default)]
    pub body: Option<MessageBodyResponse>,
}

#[derive(Debug, Deserialize)]
pub struct IdentitySet {
    #[serde(default)]
    pub user: Option<Identity>,
    #[serde(default)]
    pub application: Option<Identity>,
}

#[derive(Debug, Deserialize)]
pub struct Identity {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "displayName")]
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageBodyResponse {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(rename = "contentType")]
    #[serde(default)]
    pub content_type: Option<String>,
}

pub(crate) fn parse_response(bytes: &[u8]) -> Result<SendMessageResponse, Error> {
    match serde_json::from_slice::<SendMessageResponse>(bytes) {
        Ok(response) => Ok(response),
        Err(_) => {
            let error_resp: GraphErrorResponse = serde_json::from_slice(bytes)?;
            Err(Error::TeamsApi {
                code: error_resp.error.code,
                message: error_resp.error.message,
            })
        }
    }
}
