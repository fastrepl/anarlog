#![forbid(unsafe_code)]

mod error;
mod query;

use std::sync::Arc;

pub use error::{Error, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyQueryMethod {
    Run,
    All,
    Get,
    Values,
}

impl std::str::FromStr for ProxyQueryMethod {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
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

#[derive(Clone)]
pub struct DbExecutor {
    db: Arc<hypr_db_core::Db>,
}

impl DbExecutor {
    pub fn new(db: Arc<hypr_db_core::Db>) -> Self {
        Self { db }
    }

    pub async fn execute(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        query::run_query(self.db.as_ref(), &sql, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn execute_proxy(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        method: ProxyQueryMethod,
    ) -> Result<ProxyQueryResult> {
        query::run_query_proxy(self.db.as_ref(), &sql, &params, method)
            .await
            .map_err(Into::into)
    }
}
