use std::path::PathBuf;
use std::pin::Pin;

use futures_util::Stream;
use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(flatten)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

impl Event {
    pub fn string(&self, key: &str) -> Option<&str> {
        self.data.get(key)?.as_str()
    }

    pub fn session_id(&self) -> Option<&str> {
        self.string("sessionID")
            .or_else(|| self.string("sessionId"))
            .or_else(|| self.string("session_id"))
    }
}

#[derive(Debug, Clone)]
pub enum UserInput {
    Text(String),
    LocalFile(PathBuf),
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

impl From<PathBuf> for UserInput {
    fn from(value: PathBuf) -> Self {
        Self::LocalFile(value)
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
                let mut files = Vec::new();
                for item in items {
                    match item {
                        UserInput::Text(text) => prompt_parts.push(text),
                        UserInput::LocalFile(path) => files.push(path),
                    }
                }
                (prompt_parts.join("\n\n"), files)
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

pub type EventStream = Pin<Box<dyn Stream<Item = Result<Event, Error>> + Send>>;

pub struct RunStreamedResult {
    pub events: EventStream,
}

#[derive(Debug, Clone)]
pub struct SessionTurn {
    pub events: Vec<Event>,
}
