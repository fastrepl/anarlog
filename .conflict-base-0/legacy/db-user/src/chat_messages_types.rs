use crate::user_common_derives;

user_common_derives! {
    #[derive(strum::EnumString, strum::Display)]
    pub enum ChatMessageRole {
        User,
        Assistant,
    }
}

user_common_derives! {
    #[derive(strum::EnumString, strum::Display)]
    pub enum ChatMessageType {
       #[serde(rename = "text-delta")]
       #[strum(serialize = "text-delta")]
       TextDelta,
       #[serde(rename = "tool-start")]
       #[strum(serialize = "tool-start")]
       ToolStart,
       #[serde(rename = "tool-result")]
       #[strum(serialize = "tool-result")]
       ToolResult,
       #[serde(rename = "tool-error")]
       #[strum(serialize = "tool-error")]
       ToolError,
    }
}

user_common_derives! {
    pub struct ChatMessage {
        pub id: String,
        pub group_id: String,
        pub created_at: chrono::DateTime<chrono::Utc>,
        pub role: ChatMessageRole,
        pub content: String,
        pub r#type: ChatMessageType,
        pub tool_details: Option<String>,
    }
}
