#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum BridgeError {
    #[error("bridge is closed")]
    Closed,
    #[error("invalid params json: {reason}")]
    InvalidParamsJson { reason: String },
    #[error("invalid cloudsync config json: {reason}")]
    InvalidCloudsyncConfigJson { reason: String },
    #[error("params json must encode an array")]
    ParamsMustBeArray,
    #[error("failed to open database: {reason}")]
    OpenFailed { reason: String },
    #[error("query failed: {reason}")]
    QueryFailed { reason: String },
    #[error("cloudsync failed: {reason}")]
    CloudsyncFailed { reason: String },
    #[error("failed to serialize payload: {reason}")]
    SerializationFailed { reason: String },
}

pub(crate) fn parse_params_json(params_json: &str) -> Result<Vec<serde_json::Value>, BridgeError> {
    if params_json.trim().is_empty() {
        return Ok(Vec::new());
    }

    let value: serde_json::Value =
        serde_json::from_str(params_json).map_err(|error| BridgeError::InvalidParamsJson {
            reason: error.to_string(),
        })?;
    match value {
        serde_json::Value::Array(values) => Ok(values),
        _ => Err(BridgeError::ParamsMustBeArray),
    }
}

pub(crate) fn runtime_error(error: hypr_db_live_query::Error) -> BridgeError {
    BridgeError::QueryFailed {
        reason: error.to_string(),
    }
}

pub(crate) fn cloudsync_error(error: hypr_db_core2::Error) -> BridgeError {
    BridgeError::CloudsyncFailed {
        reason: error.to_string(),
    }
}

pub(crate) fn cloudsync_runtime_error(error: hypr_db_core2::CloudsyncRuntimeError) -> BridgeError {
    BridgeError::CloudsyncFailed {
        reason: error.to_string(),
    }
}

pub(crate) fn serialization_error(error: serde_json::Error) -> BridgeError {
    BridgeError::SerializationFailed {
        reason: error.to_string(),
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum OpenAppDbError {
    #[error(transparent)]
    Open(#[from] hypr_db_core2::DbOpenError),
    #[error(transparent)]
    Migrate(#[from] hypr_db_migrate::MigrateError),
}
