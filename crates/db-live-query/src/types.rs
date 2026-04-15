use std::collections::HashSet;

use crate::error::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyQueryMethod {
    Run,
    All,
    Get,
    Values,
}

impl std::str::FromStr for ProxyQueryMethod {
    type Err = Error;

    fn from_str(s: &str) -> crate::error::Result<Self> {
        match s {
            "run" => Ok(Self::Run),
            "all" => Ok(Self::All),
            "get" => Ok(Self::Get),
            "values" => Ok(Self::Values),
            _ => Err(Error::InvalidQueryMethod(s.to_string())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ProxyQueryResult {
    pub rows: Vec<serde_json::Value>,
}

pub trait QueryEventSink: Clone + Send + 'static {
    fn send_result(&self, rows: Vec<serde_json::Value>) -> std::result::Result<(), String>;
    fn send_error(&self, error: String) -> std::result::Result<(), String>;
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum DependencyTarget {
    Table(String),
    VirtualTable(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyAnalysis {
    Reactive { targets: HashSet<DependencyTarget> },
    NonReactive { reason: String },
}

pub struct SubscriptionRegistration {
    pub id: String,
    pub analysis: DependencyAnalysis,
}
