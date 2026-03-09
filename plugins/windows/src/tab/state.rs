crate::common_derives! {
    #[serde(tag = "type")]
    pub enum EditorView {
        #[serde(rename = "raw")]
        Raw,
        #[serde(rename = "transcript")]
        Transcript,
        #[serde(rename = "enhanced")]
        Enhanced { id: String },
        #[serde(rename = "attachments")]
        Attachments,
    }
}

crate::common_derives! {
    pub struct SessionsState {
        pub view: Option<EditorView>,
        pub auto_start: Option<bool>,
    }
}

crate::common_derives! {
    #[serde(tag = "type")]
    pub enum ContactsSelection {
        #[serde(rename = "person")]
        Person { id: String },
        #[serde(rename = "organization")]
        Organization { id: String },
    }
}

crate::common_derives! {
    pub struct ContactsState {
        pub selected: Option<ContactsSelection>,
    }
}

crate::common_derives! {
    pub struct TemplatesState {
        pub show_homepage: Option<bool>,
        pub is_web_mode: Option<bool>,
        pub selected_mine_id: Option<String>,
        pub selected_web_index: Option<i32>,
    }
}

crate::common_derives! {
    pub struct PromptsState {
        pub selected_task: Option<String>,
    }
}

crate::common_derives! {
    pub struct ChatShortcutsState {
        pub is_web_mode: Option<bool>,
        pub selected_mine_id: Option<String>,
        pub selected_web_index: Option<i32>,
    }
}

crate::common_derives! {
    pub struct ExtensionsState {
        pub selected_extension: Option<String>,
    }
}

crate::common_derives! {
    pub struct ChangelogState {
        pub previous: Option<String>,
        pub current: String,
    }
}

crate::common_derives! {
    #[derive(Default)]
    pub enum AiTab {
        #[default]
        #[serde(rename = "transcription")]
        Transcription,
        #[serde(rename = "intelligence")]
        Intelligence,
        #[serde(rename = "templates")]
        Templates,
        #[serde(rename = "shortcuts")]
        Shortcuts,
        #[serde(rename = "prompts")]
        Prompts,
        #[serde(rename = "memory")]
        Memory,
    }
}

crate::common_derives! {
    #[derive(Default)]
    pub struct AiState {
        pub tab: Option<AiTab>,
    }
}

crate::common_derives! {
    #[derive(Default)]
    pub struct SearchState {
        pub selected_types: Option<Vec<String>>,
        pub initial_query: Option<String>,
    }
}

crate::common_derives! {
    #[derive(Default)]
    pub struct ChatState {
        pub group_id: Option<String>,
        pub initial_message: Option<String>,
    }
}
