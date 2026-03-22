mod state;
pub use state::*;

#[macro_export]
macro_rules! common_derives {
    ($item:item) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
        #[serde(rename_all = "camelCase")]
        $item
    };
}

common_derives! {
    #[serde(tag = "type")]
    pub enum TabInput {
        #[serde(rename = "sessions")]
        Sessions {
            id: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            state: Option<SessionsState>,
        },
        #[serde(rename = "contacts")]
        Contacts {
            #[serde(skip_serializing_if = "Option::is_none")]
            state: Option<ContactsState>,
        },
        #[serde(rename = "templates")]
        Templates {
            #[serde(skip_serializing_if = "Option::is_none")]
            state: Option<TemplatesState>,
        },
        #[serde(rename = "prompts")]
        Prompts {
            #[serde(skip_serializing_if = "Option::is_none")]
            state: Option<PromptsState>,
        },
        #[serde(rename = "chat_shortcuts")]
        ChatShortcuts {
            #[serde(skip_serializing_if = "Option::is_none")]
            state: Option<ChatShortcutsState>,
        },
        #[serde(rename = "extensions")]
        Extensions {
            #[serde(skip_serializing_if = "Option::is_none")]
            state: Option<ExtensionsState>,
        },
        #[serde(rename = "humans")]
        Humans { id: String },
        #[serde(rename = "organizations")]
        Organizations { id: String },
        #[serde(rename = "folders")]
        Folders { id: Option<String> },
        #[serde(rename = "empty")]
        Empty,
        #[serde(rename = "extension")]
        Extension {
            #[serde(rename = "extensionId")]
            extension_id: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            state: Option<serde_json::Map<String, serde_json::Value>>,
        },
        #[serde(rename = "calendar")]
        Calendar,
        #[serde(rename = "changelog")]
        Changelog { state: ChangelogState },
        #[serde(rename = "settings")]
        Settings,
        #[serde(rename = "ai")]
        Ai {
            #[serde(skip_serializing_if = "Option::is_none")]
            state: Option<AiState>,
        },
        #[serde(rename = "search")]
        Search {
            #[serde(skip_serializing_if = "Option::is_none")]
            state: Option<SearchState>,
        },
        #[serde(rename = "chat_support")]
        ChatSupport {
            #[serde(skip_serializing_if = "Option::is_none")]
            state: Option<ChatState>,
        },
        #[serde(rename = "onboarding")]
        Onboarding,
        #[serde(rename = "daily")]
        Daily,
        #[serde(rename = "edit")]
        Edit {
            #[serde(rename = "requestId")]
            request_id: String,
        },
    }
}
