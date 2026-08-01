use std::path::Path;
use std::str::FromStr;

use anlg_db_app::{
    LegacyActionItem, LegacyAppSetting, LegacyChatGroup, LegacyChatMessage, LegacyDailyNote,
    LegacyHuman, LegacyImportBatch, LegacyImportRow, LegacyOrganization,
};
use anlg_fs_sync_core::frontmatter::ParsedDocument;
use serde_json::Value;

use super::source_format::{
    append_warning, file_stem, json_field, parse_json_object, stable_id, value_bool, value_i64,
    value_string,
};

pub(super) fn parse_human(path: &Path, content: &str) -> Result<LegacyImportBatch, String> {
    let document = ParsedDocument::from_str(content).map_err(|error| error.to_string())?;
    let id = file_stem(path)?;
    let emails = match document.frontmatter.get("emails") {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(","),
        _ => value_string(document.frontmatter.get("email")).unwrap_or_default(),
    };

    Ok(LegacyImportBatch {
        rows: vec![LegacyImportRow::Human(LegacyHuman {
            id,
            owner_user_id: value_string(document.frontmatter.get("user_id")).unwrap_or_default(),
            organization_id: value_string(document.frontmatter.get("org_id")).unwrap_or_default(),
            name: value_string(document.frontmatter.get("name")).unwrap_or_default(),
            email: emails,
            phone: value_string(document.frontmatter.get("phone")).unwrap_or_default(),
            job_title: value_string(document.frontmatter.get("job_title")).unwrap_or_default(),
            linkedin_username: value_string(document.frontmatter.get("linkedin_username"))
                .unwrap_or_default(),
            memo: document.content.trim().to_string(),
            pinned: value_bool(document.frontmatter.get("pinned")),
            pin_order: value_i64(document.frontmatter.get("pin_order")),
            created_at: value_string(document.frontmatter.get("created_at")).unwrap_or_default(),
        })],
        ..Default::default()
    })
}

pub(super) fn parse_organization(path: &Path, content: &str) -> Result<LegacyImportBatch, String> {
    let document = ParsedDocument::from_str(content).map_err(|error| error.to_string())?;
    Ok(LegacyImportBatch {
        rows: vec![LegacyImportRow::Organization(LegacyOrganization {
            id: file_stem(path)?,
            owner_user_id: value_string(document.frontmatter.get("user_id")).unwrap_or_default(),
            name: value_string(document.frontmatter.get("name")).unwrap_or_default(),
            memo: document.content.trim().to_string(),
            pinned: value_bool(document.frontmatter.get("pinned")),
            pin_order: value_i64(document.frontmatter.get("pin_order")),
            created_at: value_string(document.frontmatter.get("created_at")).unwrap_or_default(),
        })],
        ..Default::default()
    })
}

pub(super) fn parse_tasks(content: &str) -> Result<LegacyImportBatch, String> {
    let table = parse_json_object(content)?;
    let mut batch = LegacyImportBatch::default();
    for (row_id, row) in table {
        let Some(row) = row.as_object() else {
            batch.skipped_count += 1;
            append_warning(&mut batch, format!("task {row_id} is not an object"));
            continue;
        };
        let id = value_string(row.get("task_id"))
            .filter(|id| !id.is_empty())
            .unwrap_or(row_id);
        let source_type = value_string(row.get("source_type")).unwrap_or_default();
        let source_id = value_string(row.get("source_id")).unwrap_or_default();
        let body = row
            .get("body")
            .cloned()
            .or_else(|| row.get("body_json").cloned())
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let body_json = match body {
            Value::String(value) if serde_json::from_str::<Value>(&value).is_ok() => value,
            value => value.to_string(),
        };
        batch
            .rows
            .push(LegacyImportRow::ActionItem(LegacyActionItem {
                id,
                owner_user_id: value_string(row.get("user_id")).unwrap_or_default(),
                session_id: if source_type == "session" {
                    source_id.clone()
                } else {
                    String::new()
                },
                source_type,
                source_id,
                source_order: value_i64(row.get("source_order"))
                    .or_else(|| value_i64(row.get("order")))
                    .unwrap_or(0),
                status: value_string(row.get("status")).unwrap_or_else(|| "todo".to_string()),
                text: value_string(row.get("text_preview"))
                    .or_else(|| value_string(row.get("text")))
                    .unwrap_or_default(),
                body_json,
                due_at: value_string(row.get("due_date")).unwrap_or_default(),
            }));
    }
    Ok(batch)
}

pub(super) fn parse_daily_notes(content: &str) -> Result<LegacyImportBatch, String> {
    let table = parse_json_object(content)?;
    let mut batch = LegacyImportBatch::default();
    for (id, row) in table {
        let Some(row) = row.as_object() else {
            batch.skipped_count += 1;
            append_warning(&mut batch, format!("daily note {id} is not an object"));
            continue;
        };
        batch.rows.push(LegacyImportRow::DailyNote(LegacyDailyNote {
            id,
            owner_user_id: value_string(row.get("user_id")).unwrap_or_default(),
            note_date: value_string(row.get("date")).unwrap_or_default(),
            body_format: "prosemirror_json".to_string(),
            body: value_string(row.get("content")).unwrap_or_default(),
        }));
    }
    Ok(batch)
}

pub(super) fn parse_chat(path: &Path, content: &str) -> Result<LegacyImportBatch, String> {
    let root = parse_json_object(content)?;
    let group = root
        .get("chat_group")
        .and_then(Value::as_object)
        .ok_or_else(|| "chat file has no chat_group object".to_string())?;
    let inferred_group_id = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let group_id = value_string(group.get("id"))
        .filter(|id| !id.is_empty())
        .unwrap_or(inferred_group_id);
    if group_id.is_empty() {
        return Err("chat group has no id".to_string());
    }

    let mut batch = LegacyImportBatch::default();
    batch.rows.push(LegacyImportRow::ChatGroup(LegacyChatGroup {
        id: group_id.clone(),
        owner_user_id: value_string(group.get("user_id")).unwrap_or_default(),
        title: value_string(group.get("title")).unwrap_or_default(),
        created_at: value_string(group.get("created_at")).unwrap_or_default(),
    }));

    if let Some(messages) = root.get("messages").and_then(Value::as_array) {
        for (index, message) in messages.iter().enumerate() {
            let Some(message) = message.as_object() else {
                batch.skipped_count += 1;
                append_warning(&mut batch, format!("chat message {index} is not an object"));
                continue;
            };
            let id = value_string(message.get("id"))
                .filter(|id| !id.is_empty())
                .unwrap_or_else(|| stable_id(&format!("{group_id}:message:{index}")));
            batch
                .rows
                .push(LegacyImportRow::ChatMessage(LegacyChatMessage {
                    id,
                    chat_group_id: value_string(message.get("chat_group_id"))
                        .filter(|id| !id.is_empty())
                        .unwrap_or_else(|| group_id.clone()),
                    owner_user_id: value_string(message.get("user_id")).unwrap_or_default(),
                    role: value_string(message.get("role")).unwrap_or_default(),
                    content: value_string(message.get("content")).unwrap_or_default(),
                    metadata_json: json_field(message.get("metadata"), "{}"),
                    parts_json: json_field(message.get("parts"), "[]"),
                    status: value_string(message.get("status"))
                        .unwrap_or_else(|| "ready".to_string()),
                    created_at: value_string(message.get("created_at")).unwrap_or_default(),
                }));
        }
    }

    Ok(batch)
}

pub(super) fn parse_settings(content: &str) -> Result<LegacyImportBatch, String> {
    serde_json::from_str::<Value>(content).map_err(|error| error.to_string())?;
    Ok(LegacyImportBatch {
        rows: vec![LegacyImportRow::AppSetting(LegacyAppSetting {
            id: "legacy_settings_document".to_string(),
            value_json: content.to_string(),
        })],
        ..Default::default()
    })
}
