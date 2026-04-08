use std::path::PathBuf;
use std::pin::Pin;

use futures_util::Stream;
use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadError {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(flatten)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

impl ThreadItem {
    pub fn text(&self) -> Option<&str> {
        self.data.get("text")?.as_str()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ThreadEvent {
    #[serde(rename = "thread.started")]
    ThreadStarted { thread_id: String },
    #[serde(rename = "turn.started")]
    TurnStarted,
    #[serde(rename = "turn.completed")]
    TurnCompleted { usage: Usage },
    #[serde(rename = "turn.failed")]
    TurnFailed { error: ThreadError },
    #[serde(rename = "item.started")]
    ItemStarted { item: ThreadItem },
    #[serde(rename = "item.updated")]
    ItemUpdated { item: ThreadItem },
    #[serde(rename = "item.completed")]
    ItemCompleted { item: ThreadItem },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Clone)]
pub enum UserInput {
    Text(String),
    LocalImage(PathBuf),
}

impl From<&str> for UserInput {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for UserInput {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

#[derive(Debug, Clone)]
pub enum Input {
    Text(String),
    Items(Vec<UserInput>),
}

impl Input {
    pub(crate) fn normalize(self) -> (String, Vec<PathBuf>) {
        match self {
            Self::Text(text) => (text, Vec::new()),
            Self::Items(items) => {
                let mut prompt_parts = Vec::new();
                let mut images = Vec::new();
                for item in items {
                    match item {
                        UserInput::Text(text) => prompt_parts.push(text),
                        UserInput::LocalImage(path) => images.push(path),
                    }
                }
                (prompt_parts.join("\n\n"), images)
            }
        }
    }
}

impl From<String> for Input {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for Input {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<Vec<UserInput>> for Input {
    fn from(value: Vec<UserInput>) -> Self {
        Self::Items(value)
    }
}

#[derive(Debug, Clone)]
pub struct Turn {
    pub items: Vec<ThreadItem>,
    pub final_response: String,
    pub usage: Option<Usage>,
}

pub type EventStream = Pin<Box<dyn Stream<Item = Result<ThreadEvent, Error>> + Send>>;

pub struct RunStreamedResult {
    pub events: EventStream,
}
