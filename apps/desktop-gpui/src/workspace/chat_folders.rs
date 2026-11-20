//! Shipping's folder chat tools (`chat/tools/folders.ts`, `chat/tools/folder-materials.ts`),
//! on the workspace's folder catalog and vault.

use std::collections::HashSet;

use anlg_fs_sync_core::normalize_folder_path;
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};

use super::folders::{LIBRARY_SQL, decode_library, filesystem, move_notes};
use crate::meeting::model::failure;

const INVALID_PATH: &str = "Use a relative folder path without empty, . or .. segments";

async fn folder_paths(runtime: &RuntimeHandle) -> Result<Vec<String>> {
    let rows = runtime
        .read(CancellationToken::new(), |services| async move {
            services
                .executor
                .execute(LIBRARY_SQL.into(), vec![])
                .await
                .map_err(failure)
        })?
        .receive()
        .await?;
    Ok(decode_library(&rows)
        .paths
        .iter()
        .map(|path| path.to_string())
        .collect())
}

pub async fn list(
    runtime: &RuntimeHandle,
    query: String,
    limit: u32,
    offset: u32,
) -> Result<Value> {
    let needle = query.trim().to_lowercase();
    let paths = folder_paths(runtime)
        .await?
        .into_iter()
        .filter(|path| path.to_lowercase().contains(&needle))
        .collect::<Vec<_>>();
    let folders = paths
        .iter()
        .skip(offset as usize)
        .take(limit.clamp(1, 200) as usize)
        .cloned()
        .collect::<Vec<_>>();
    let end = offset as usize + folders.len();
    Ok(json!({
        "folders": folders,
        "total": paths.len(),
        "next_offset": (end < paths.len()).then_some(end),
    }))
}

pub async fn create(runtime: &RuntimeHandle, path: String) -> Result<Value> {
    let normalized = normalize_folder_path(&path).map_err(|_| failure(INVALID_PATH))?;
    if normalized.is_empty() {
        return Err(failure("A named folder is required"));
    }
    let created = super::folders::create(runtime, normalized.into())?
        .receive()
        .await?;
    Ok(json!({"status": "ok", "folder_path": created}))
}

/// Moves each meeting on its own, reporting moved, unchanged and failed meetings.
pub async fn move_meetings(
    runtime: &RuntimeHandle,
    ids: Vec<String>,
    path: String,
    cancellation: CancellationToken,
) -> Result<Value> {
    if ids.is_empty() || ids.len() > 200 {
        return Err(failure("Provide between 1 and 200 meeting ids"));
    }
    let path = normalize_folder_path(&path).map_err(|_| failure(INVALID_PATH))?;
    if !path.is_empty() && !folder_paths(runtime).await?.contains(&path) {
        return Ok(json!({
            "status": "error",
            "message": "Destination folder does not exist. Use list_folders or create_folder first.",
            "results": [],
        }));
    }
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for id in ids.into_iter().filter(|id| seen.insert(id.clone())) {
        if cancellation.is_cancelled() {
            return Err(ServiceError::Cancelled);
        }
        let (meeting_id, destination) = (id.clone(), path.clone());
        let outcome = runtime
            .submit(move |services| async move {
                let rows = services
                    .executor
                    .execute(
                        "SELECT title, folder_path FROM sessions WHERE id = ? AND deleted_at IS NULL".into(),
                        vec![json!(meeting_id)],
                    )
                    .await
                    .map_err(failure)?;
                let Some(row) = rows.first().cloned() else {
                    return Ok(None);
                };
                let previous = row["folder_path"].as_str().unwrap_or_default();
                let unchanged = normalize_folder_path(previous).ok().as_deref() == Some(&destination);
                if !unchanged {
                    move_notes(
                        &services.executor,
                        &services.vault,
                        &[json!(meeting_id)],
                        &destination,
                    )
                    .await?;
                }
                Ok(Some((row, unchanged)))
            })?
            .receive()
            .await;
        results.push(match outcome {
            Ok(Some((row, unchanged))) => json!({
                "meeting_id": id,
                "title": row["title"],
                "previous_folder_path": row["folder_path"],
                "status": if unchanged { "unchanged" } else { "moved" },
            }),
            Ok(None) => json!({"meeting_id": id, "status": "error", "message": "Meeting not found or deleted"}),
            Err(error) => json!({"meeting_id": id, "status": "error", "message": error.to_string()}),
        });
    }
    let count = |status: &str| {
        results
            .iter()
            .filter(|result| result["status"] == status)
            .count()
    };
    let failed = count("error");
    // Shipping's `toModelOutput` keeps only the failures in the model's copy.
    Ok(json!({
        "status": if failed == 0 { "ok" } else if failed == results.len() { "error" } else { "partial" },
        "folder_path": path,
        "moved": count("moved"),
        "unchanged": count("unchanged"),
        "failed": failed,
        "results": results.iter().filter(|result| result["status"] == "error").collect::<Vec<_>>(),
    }))
}

const DEFAULT_READ_CHARS: u32 = 16_000;
const MAX_READ_CHARS: u32 = 30_000;

fn readable_text(content_type: &str, filename: &str) -> bool {
    let extension = filename
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_lowercase())
        .unwrap_or_default();
    content_type.starts_with("text/")
        || matches!(
            content_type,
            "application/json" | "application/xml" | "application/javascript"
        )
        || matches!(
            extension.as_str(),
            "txt"
                | "md"
                | "markdown"
                | "csv"
                | "json"
                | "xml"
                | "html"
                | "css"
                | "js"
                | "ts"
                | "tsx"
        )
}

/// Reads a material from the mentioned folders. PDFs are reported as unreadable: the native
/// app has no PDF text extraction yet.
pub async fn read_material(
    runtime: &RuntimeHandle,
    folders: &[String],
    material_id: String,
    max_chars: Option<u32>,
) -> Result<Value> {
    if folders.is_empty() {
        return Ok(json!({"status": "error", "message": "No folder is selected"}));
    }
    let folders = folders.to_vec();
    let id = material_id.clone();
    let found = runtime
        .read(CancellationToken::new(), move |services| async move {
            for folder in &folders {
                let rows = services
                    .executor
                    .execute(
                        "SELECT id, filename, relative_path, content_type, size_bytes FROM folder_attachments WHERE folder_path = ? AND id = ? AND deleted_at IS NULL LIMIT 1".into(),
                        vec![json!(folder), json!(id)],
                    )
                    .await
                    .map_err(failure)?;
                if let Some(row) = rows.into_iter().next() {
                    let disk = row["relative_path"]
                        .as_str()
                        .and_then(|path| path.rsplit('/').next())
                        .unwrap_or_default()
                        .to_owned();
                    let bytes = filesystem(&services.vault)
                        .folder_attachment_read(folder, &disk)
                        .map_err(|error| error.to_string());
                    return Ok(Some((row, bytes)));
                }
            }
            Ok(None)
        })?
        .receive()
        .await?;
    let Some((row, bytes)) = found else {
        return Ok(json!({
            "status": "error",
            "message": format!("Could not find folder material {material_id}"),
            "materialId": material_id,
        }));
    };
    let filename = row["filename"].as_str().unwrap_or_default();
    let content_type = row["content_type"].as_str().unwrap_or_default();
    let bytes = match bytes {
        Ok(bytes) => bytes,
        Err(message) => {
            return Ok(
                json!({"status": "error", "message": message, "materialId": row["id"], "filename": filename}),
            );
        }
    };
    let base = json!({
        "status": "ok",
        "materialId": row["id"],
        "filename": filename,
        "contentType": content_type,
        "sizeBytes": row["size_bytes"],
    });
    let with = |mut base: Value, fields: Value| {
        if let (Some(base), Some(fields)) = (base.as_object_mut(), fields.as_object()) {
            base.extend(fields.clone());
        }
        base
    };
    let pdf = content_type == "application/pdf" || filename.to_lowercase().ends_with(".pdf");
    if pdf {
        return Ok(with(
            base,
            json!({
                "readable": false,
                "message": "Could not extract text from this PDF. Use the filename and folder notes.",
            }),
        ));
    }
    if !readable_text(content_type, filename) {
        return Ok(with(
            base,
            json!({
                "readable": false,
                "message": "This file is binary. Text extraction is not available yet; use the filename and folder notes.",
            }),
        ));
    }
    let limit = max_chars
        .unwrap_or(DEFAULT_READ_CHARS)
        .clamp(1_000, MAX_READ_CHARS) as usize;
    let text = String::from_utf8_lossy(&bytes);
    let truncated = text.chars().count() > limit;
    let text = if truncated {
        format!(
            "{}\n\n[Content truncated]",
            text.chars().take(limit).collect::<String>().trim_end()
        )
    } else {
        text.into_owned()
    };
    Ok(with(
        base,
        json!({"readable": true, "truncated": truncated, "contextText": text}),
    ))
}
