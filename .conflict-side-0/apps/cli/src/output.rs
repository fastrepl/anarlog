use std::io::Write;
use std::path::Path;

use serde::Serialize;

use crate::{Error, Result};
use hypr_agent_access::Pagination;

pub const JSON_SCHEMA_VERSION: &str = "1";

#[derive(Serialize)]
struct JsonResponse<'a, T> {
    schema_version: &'static str,
    command: &'static str,
    data: &'a T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pagination: Option<&'a Pagination>,
}

pub fn json(
    command: &'static str,
    value: &impl Serialize,
    pagination: Option<&Pagination>,
) -> Result<String> {
    raw_json(&JsonResponse {
        schema_version: JSON_SCHEMA_VERSION,
        command,
        data: value,
        pagination,
    })
}

pub fn raw_json(value: &impl Serialize) -> Result<String> {
    serde_json::to_string_pretty(value)
        .map_err(|error| Error::operation("serialize output", error.to_string()))
}

pub fn emit(text: &str) {
    println!("{text}");
}

pub fn write_or_emit(text: &str, path: Option<&Path>, force: bool) -> Result<()> {
    match path {
        Some(path) => {
            if force {
                return std::fs::write(path, text)
                    .map_err(|error| Error::operation("write export", error.to_string()));
            }
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|error| {
                    if error.kind() == std::io::ErrorKind::AlreadyExists {
                        Error::OutputExists(path.to_path_buf())
                    } else {
                        Error::operation("write export", error.to_string())
                    }
                })?;
            file.write_all(text.as_bytes())
                .map_err(|error| Error::operation("write export", error.to_string()))
        }
        None => {
            emit(text);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_response_has_stable_version_and_pagination() {
        let pagination = Pagination {
            offset: 20,
            limit: 10,
            returned: 2,
            total: None,
            next_offset: None,
        };
        let response = json(
            "meetings.list",
            &serde_json::json!([{"id": "meeting-1"}]),
            Some(&pagination),
        )
        .unwrap();
        let response: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(response["schema_version"], "1");
        assert_eq!(response["command"], "meetings.list");
        assert_eq!(response["data"][0]["id"], "meeting-1");
        assert_eq!(response["pagination"]["offset"], 20);
        assert!(response["pagination"]["total"].is_null());
    }

    #[test]
    fn export_requires_force_before_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meeting.md");
        std::fs::write(&path, "existing").unwrap();

        let error = write_or_emit("replacement", Some(&path), false).unwrap_err();
        assert_eq!(error.code(), "output_exists");
        assert_eq!(error.exit_code(), 4);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "existing");

        write_or_emit("replacement", Some(&path), true).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "replacement");
    }
}
