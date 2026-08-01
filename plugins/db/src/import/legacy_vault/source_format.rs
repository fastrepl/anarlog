use std::fmt::Write;
use std::path::Path;

use anlg_db_app::LegacyImportBatch;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub(super) fn parse_json_object(content: &str) -> Result<Map<String, Value>, String> {
    serde_json::from_str::<Value>(content)
        .map_err(|error| error.to_string())?
        .as_object()
        .cloned()
        .ok_or_else(|| "JSON root is not an object".to_string())
}

pub(super) fn infer_session_id_and_folder(
    vault_base: &Path,
    path: &Path,
) -> Result<(String, String), String> {
    let relative = path
        .strip_prefix(vault_base)
        .map_err(|_| "session source is outside the vault".to_string())?;
    let parts = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    if parts.first().map(String::as_str) != Some("sessions") || parts.len() < 3 {
        return Err("invalid session source path".to_string());
    }
    let session_id = parts[parts.len() - 2].clone();
    let folder_path = parts[1..parts.len() - 2].join("/");
    Ok((session_id, folder_path))
}

pub(super) fn normalized_relative_path(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn file_stem(path: &Path) -> Result<String, String> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| "source filename has no valid id".to_string())
}

pub(super) fn value_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToString::to_string)
}

pub(super) fn value_i64(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    value
        .as_i64()
        .or_else(|| value.as_f64().map(|number| number.round() as i64))
}

pub(super) fn value_bool(value: Option<&Value>) -> bool {
    value.and_then(Value::as_bool).unwrap_or(false)
}

pub(super) fn json_field(value: Option<&Value>, fallback: &str) -> String {
    match value {
        Some(Value::String(value)) if serde_json::from_str::<Value>(value).is_ok() => value.clone(),
        Some(value) => value.to_string(),
        None => fallback.to_string(),
    }
}

pub(super) fn append_warning(batch: &mut LegacyImportBatch, warning: String) {
    if !batch.warning.is_empty() {
        batch.warning.push_str("; ");
    }
    batch.warning.push_str(&warning);
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
            output
        })
}

pub(super) fn stable_id(value: &str) -> String {
    sha256(value.as_bytes())
}

pub(super) fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("aac") => "audio/aac",
        Some("m4a") => "audio/mp4",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("webm") => "audio/webm",
        Some("gif") => "image/gif",
        Some("jpeg" | "jpg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        Some("md") => "text/markdown",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}
