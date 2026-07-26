use crate::user_common_derives;

user_common_derives! {
    #[derive(strum::EnumString, strum::Display)]
    pub enum ChatMessageV2Role {
        #[serde(rename = "system")]
        #[strum(serialize = "system")]
        System,
        #[serde(rename = "user")]
        #[strum(serialize = "user")]
        User,
        #[serde(rename = "assistant")]
        #[strum(serialize = "assistant")]
        Assistant,
    }
}

user_common_derives! {
    pub struct ChatMessageV2 {
        pub id: String,
        pub conversation_id: String,
        pub role: ChatMessageV2Role,
        pub parts: String, // JSON string
        pub metadata: Option<String>, // JSON string
        pub created_at: chrono::DateTime<chrono::Utc>,
        pub updated_at: chrono::DateTime<chrono::Utc>,
    }
}
