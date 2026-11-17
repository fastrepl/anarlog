//! Write helpers shared by the workspace's data modules.

use anlg_db_execute::{DbExecutor, TransactionStatement};
use desktop_runtime::{Result, ServiceError};
use serde_json::Value;

pub(super) const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ','now')";

pub(super) fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

pub(super) fn statement(
    sql: String,
    params: Vec<Value>,
    expected: Option<u64>,
) -> TransactionStatement {
    TransactionStatement {
        sql,
        params,
        expected_rows_affected: expected,
    }
}

/// Runs `statements` atomically; an unexpected row count (a lost CAS) is a conflict.
pub(super) async fn transaction(
    executor: &DbExecutor,
    statements: Vec<TransactionStatement>,
) -> Result<()> {
    executor
        .execute_transaction(statements)
        .await
        .map(|_| ())
        .map_err(|error| match error {
            anlg_db_execute::Error::UnexpectedRowsAffected { .. } => ServiceError::Conflict,
            error => failure(error),
        })
}

/// A `WHERE` clause matching every column of the row as it was read.
pub(super) fn cas(base: &Value) -> Result<(String, Vec<Value>)> {
    let base = base.as_object().ok_or(ServiceError::Conflict)?;
    let mut clauses = Vec::new();
    let mut params = Vec::new();
    for (key, value) in base {
        if !key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            return Err(failure("Invalid stored column"));
        }
        clauses.push(format!("{key} IS ?"));
        params.push(value.clone());
    }
    Ok((clauses.join(" AND "), params))
}

/// Settings store strings as JSON strings, so JSON settings arrive encoded twice.
pub fn decode_json_text(text: &str) -> Result<Value> {
    let value: Value = serde_json::from_str(text)
        .map_err(|error| ServiceError::Failed(format!("Invalid stored JSON: {error}").into()))?;
    if let Value::String(nested) = value {
        serde_json::from_str(&nested).map_err(|error| {
            ServiceError::Failed(format!("Invalid nested stored JSON: {error}").into())
        })
    } else {
        Ok(value)
    }
}
