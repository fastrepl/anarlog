#[allow(unused_imports)]
use hypr_askama_utils::filters;

use crate::{SessionContext, common_derives};

common_derives! {
    pub struct ToolSearchSessionItem {
        pub id: String,
        pub title: Option<String>,
        pub excerpt: Option<String>,
        pub score: f32,
        pub created_at: Option<u64>,
        pub session_context: Option<SessionContext>,
    }
}

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "tool.search-sessions.md.jinja")]
    pub struct ToolSearchSessions {
        pub query: String,
        pub results: Vec<ToolSearchSessionItem>,
    }
}
