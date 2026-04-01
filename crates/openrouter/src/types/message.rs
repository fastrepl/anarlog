use serde::{Deserialize, Serialize};

use super::content::Content;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Developer,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub refusal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reasoning_details: Option<Vec<ReasoningDetail>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub images: Option<Vec<ResponseImage>>,
}

impl ChatMessage {
    pub fn new(role: Role, content: impl Into<Content>) -> Self {
        Self {
            role,
            content: Some(content.into()),
            name: None,
            tool_call_id: None,
            tool_calls: None,
            refusal: None,
            reasoning: None,
            reasoning_details: None,
            images: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none", default)]
    pub r#type: Option<String>,
    pub function: FunctionCall,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub index: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ReasoningDetail {
    #[serde(rename = "reasoning.summary")]
    Summary {
        summary: String,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        format: Option<String>,
        #[serde(default)]
        index: Option<f64>,
    },
    #[serde(rename = "reasoning.encrypted")]
    Encrypted {
        data: String,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        format: Option<String>,
        #[serde(default)]
        index: Option<f64>,
    },
    #[serde(rename = "reasoning.text")]
    Text {
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        signature: Option<String>,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        format: Option<String>,
        #[serde(default)]
        index: Option<f64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseImage {
    pub image_url: ResponseImageUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseImageUrl {
    pub url: String,
}
