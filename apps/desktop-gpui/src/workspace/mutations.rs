use std::{collections::BTreeMap, sync::Arc};

use anlg_db_execute::{DbExecutor, TransactionStatement};
use desktop_runtime::{CancellationToken, Reply, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};
use uuid::Uuid;

use super::ports::{Catalog, CatalogRow, decode_json_text};

pub(super) const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ','now')";
const LOCAL_OWNER: &str = "00000000-0000-0000-0000-000000000000";
const WORKSPACE: &str = "NULLIF((SELECT json_extract(value_json,'$.workspace_id') FROM app_settings WHERE id='cloudsync_workspace_binding'),'')";

#[derive(Clone, Debug)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct Draft {
    pub row: CatalogRow,
    pub fields: Vec<Field>,
    pub base: Option<Value>,
    pub groups: Option<Vec<Group>>,
    pub groups_dirty: bool,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub kind: String,
    pub fields: Vec<Field>,
    pub base: Value,
}

pub fn new_group(kind: &str) -> Group {
    group(json!({"id":Uuid::new_v4().to_string(),"type":kind}))
}

fn group(value: Value) -> Group {
    let kind = value["type"].as_str().unwrap_or("section").to_owned();
    let keys: &[(&str, &str)] = match kind.as_str() {
        "section" => &[("title", "Section title"), ("description", "Instructions")],
        "markdown_export" => &[
            ("directory", "Export directory"),
            ("filename", "Filename pattern"),
        ],
        _ => &[("target_id", "Target ID"), ("target_name", "Target name")],
    };
    let fields = keys
        .iter()
        .map(|&(key, label)| {
            let text = match key {
                "target_id" => &value["target"]["id"],
                "target_name" => &value["target"]["name"],
                "filename" => &value["options"]["filename"],
                _ => &value[key],
            };
            Field {
                key,
                label,
                value: text.as_str().unwrap_or("").into(),
            }
        })
        .collect();
    Group {
        kind,
        fields,
        base: value,
    }
}

fn groups(kind: &str, value: &Value) -> Option<Vec<Group>> {
    let list = match kind {
        "template" => serde_json::from_str::<Value>(value["sections_json"].as_str()?).ok()?,
        "workflow" => value["steps"].clone(),
        _ => return None,
    };
    let list = list.as_array()?;
    if list.len() > 64 || list.iter().any(|item| !item.is_object()) {
        return None;
    }
    if list.iter().any(|item| {
        ["title", "description", "directory"]
            .iter()
            .any(|key| !item[key].is_null() && !item[key].is_string())
            || (!item["target"].is_null() && !item["target"].is_object())
            || (!item["options"].is_null() && !item["options"].is_object())
    }) {
        return None;
    }
    Some(list.iter().cloned().map(group).collect())
}

fn hydrate_groups(draft: &mut Draft) -> Result<()> {
    if !draft.groups_dirty {
        return Ok(());
    }
    let groups = draft
        .groups
        .as_ref()
        .ok_or_else(|| failure("Invalid structured fields"))?;
    let values = groups
        .iter()
        .map(|group| {
            let mut value = group.base.clone();
            for field in &group.fields {
                let original = match field.key {
                    "target_id" => &value["target"]["id"],
                    "target_name" => &value["target"]["name"],
                    "filename" => &value["options"]["filename"],
                    key => &value[key],
                };
                if original.as_str().unwrap_or("") == field.value {
                    continue;
                }
                match field.key {
                    "target_id" | "target_name" => {
                        if !value["target"].is_object() {
                            value["target"] = json!({});
                        }
                        value["target"][if field.key == "target_id" {
                            "id"
                        } else {
                            "name"
                        }] = json!(field.value);
                    }
                    "filename" => {
                        if !value["options"].is_object() {
                            value["options"] = json!({});
                        }
                        value["options"]["filename"] = json!(field.value);
                    }
                    _ => value[field.key] = json!(field.value),
                }
            }
            value
        })
        .collect::<Vec<_>>();
    let key = if draft.row.kind.as_ref() == "template" {
        "sections_json"
    } else {
        "steps"
    };
    draft
        .fields
        .iter_mut()
        .find(|field| field.key == key)
        .ok_or_else(|| failure("Missing structured field"))?
        .value = serde_json::to_string(&values).map_err(failure)?;
    Ok(())
}

#[derive(Clone, Debug)]
pub enum Command {
    Save(Draft),
    Duplicate(Draft),
    Delete(Draft),
    Pin(Draft),
    SelectDefault(Draft),
    Merge { primary: Draft, duplicate: Arc<str> },
}

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

fn table(kind: &str) -> Result<&'static str> {
    match kind {
        "human" => Ok("humans"),
        "organization" => Ok("organizations"),
        "folder" => Ok("folders"),
        "template" => Ok("templates"),
        "workflow" => Ok("app_settings"),
        _ => Err(failure("Unknown resource kind")),
    }
}

fn fields(kind: &str) -> &'static [(&'static str, &'static str)] {
    match kind {
        "human" => &[
            ("name", "Name"),
            ("email", "Email"),
            ("phone", "Phone"),
            ("job_title", "Job title"),
            ("linkedin_username", "LinkedIn"),
            ("organization_id", "Organization ID"),
            ("memo", "Notes"),
        ],
        "organization" => &[("name", "Name"), ("memo", "Notes")],
        "folder" => &[
            ("path", "Folder"),
            ("instructions", "Context & instructions"),
        ],
        "template" => &[
            ("title", "Title"),
            ("description", "Description"),
            ("category", "Category"),
            ("sections_json", "Sections (JSON)"),
            ("targets_json", "Targets (JSON)"),
        ],
        "workflow" => &[
            ("title", "Title"),
            ("trigger", "Trigger"),
            ("enabled", "Enabled"),
            ("steps", "Steps (JSON)"),
        ],
        _ => &[],
    }
}

pub fn blank(catalog: Catalog) -> Draft {
    let (kind, title) = match catalog {
        Catalog::Contacts => ("human", "New contact"),
        Catalog::Folders => ("folder", "New folder"),
        Catalog::Templates => ("template", "Untitled template"),
        Catalog::Automations => ("workflow", "Untitled automation"),
    };
    let value = json!({"title":title,"name":"","path":"","instructions":"","sections_json":"[]","targets_json":"null","trigger":"note_enhanced","enabled":false,"steps":[]});
    Draft {
        row: CatalogRow {
            id: Uuid::new_v4().to_string().into(),
            kind: kind.into(),
            title: title.into(),
            subtitle: "".into(),
            pinned: false,
            self_contact: false,
        },
        fields: editable(kind, &value),
        base: None,
        groups: groups(kind, &value),
        groups_dirty: false,
    }
}

fn editable(kind: &str, value: &Value) -> Vec<Field> {
    fields(kind)
        .iter()
        .map(|&(key, label)| Field {
            key,
            label,
            value: match &value[key] {
                Value::String(text) => text.clone(),
                Value::Null => String::new(),
                value => value.to_string(),
            },
        })
        .collect()
}

pub fn load(
    runtime: &RuntimeHandle,
    row: CatalogRow,
    cancel: CancellationToken,
) -> Result<Reply<Draft>> {
    runtime.read(cancel, move |services| async move {
        let base = load_base(&services.executor, &row).await?;
        let value = if row.kind.as_ref() == "workflow" {
            workflow(&base, &row.id)?
        } else {
            base.clone()
        };
        if value.to_string().len() > 256 * 1024 {
            return Err(failure(
                "This record exceeds the 256 KiB editor limit. Stored content is unchanged.",
            ));
        }
        Ok(Draft {
            groups: groups(&row.kind, &value),
            groups_dirty: false,
            fields: editable(&row.kind, &value),
            row,
            base: Some(base),
        })
    })
}

async fn load_base(executor: &DbExecutor, row: &CatalogRow) -> Result<Value> {
    let table = table(&row.kind)?;
    let id = if row.kind.as_ref() == "workflow" {
        "automation_workflows"
    } else {
        &row.id
    };
    executor
        .execute(
            format!("SELECT * FROM {table} WHERE id = ?"),
            vec![json!(id)],
        )
        .await
        .map_err(failure)?
        .into_iter()
        .next()
        .ok_or(ServiceError::Conflict)
}

fn workflow(base: &Value, id: &str) -> Result<Value> {
    let list = decode_json_text(base["value_json"].as_str().unwrap_or(""))?;
    list.as_array()
        .ok_or_else(|| failure("Workflows must be an array"))?
        .iter()
        .find(|value| value["id"] == id)
        .cloned()
        .ok_or(ServiceError::Conflict)
}

pub fn dispatch(
    runtime: &RuntimeHandle,
    command: Command,
    viewer: Option<Arc<str>>,
) -> Result<Reply<CatalogRow>> {
    runtime.submit(move |services| async move {
        execute(&services.executor, command, viewer.as_deref()).await
    })
}

async fn execute(
    executor: &DbExecutor,
    command: Command,
    viewer: Option<&str>,
) -> Result<CatalogRow> {
    match command {
        Command::SelectDefault(draft) => {
            if draft.row.kind.as_ref() != "template" {
                return Err(failure("Only templates can be selected as default"));
            }
            let rows = executor
                .execute(
                    "SELECT value_json FROM app_settings WHERE id='selected_template_id'".into(),
                    vec![],
                )
                .await
                .map_err(failure)?;
            let raw = rows.first().and_then(|row| row["value_json"].as_str());
            let current = raw
                .map(serde_json::from_str::<String>)
                .transpose()
                .map_err(failure)?
                .unwrap_or_default();
            let selected = if current == draft.row.id.as_ref() {
                ""
            } else {
                draft.row.id.as_ref()
            };
            let encoded = json!(selected).to_string();
            let (predicate, params) = cas(&draft)?;
            let update=match raw {
                Some(raw)=>statement(format!("UPDATE app_settings SET value_json=?,updated_at={NOW} WHERE id='selected_template_id' AND value_json=?"),vec![json!(encoded),json!(raw)],Some(1)),
                None=>statement("INSERT INTO app_settings(id,value_json) SELECT 'selected_template_id',? WHERE NOT EXISTS(SELECT 1 FROM app_settings WHERE id='selected_template_id')".into(),vec![json!(encoded)],Some(1)),
            };
            transaction(
                executor,
                vec![
                    statement(
                        format!("UPDATE templates SET id=id WHERE {predicate}"),
                        params,
                        Some(1),
                    ),
                    update,
                ],
            )
            .await?;
            Ok(draft.row)
        }
        Command::Save(mut draft) => {
            hydrate_groups(&mut draft)?;
            save(executor, draft, viewer).await
        }
        Command::Duplicate(mut draft) => {
            if !matches!(draft.row.kind.as_ref(), "template" | "workflow") {
                return Err(failure(
                    "Only templates and automations can be duplicated here",
                ));
            }
            let old = draft.clone();
            draft.row.id = Uuid::new_v4().to_string().into();
            draft.row.pinned = false;
            for field in &mut draft.fields {
                if field.key == "title" && !field.value.ends_with("(Copy)") {
                    field.value = format!("{} (Copy)", field.value.trim());
                }
            }
            if draft.row.kind.as_ref() == "template" {
                // Copy stored extension fields without interpreting their contents.
                let base = old.base.ok_or(ServiceError::Conflict)?;
                let mut columns = Vec::new();
                let mut values = Vec::new();
                for (key, value) in base.as_object().ok_or(ServiceError::Conflict)? {
                    columns.push(key.as_str());
                    values.push(match key.as_str() {
                        "id" => json!(draft.row.id),
                        "title" => json!(draft.fields[0].value),
                        "pinned" => json!(0),
                        "pin_order" => Value::Null,
                        _ => value.clone(),
                    });
                }
                let placeholders = vec!["?"; values.len()].join(",");
                transaction(
                    executor,
                    vec![statement(
                        format!(
                            "INSERT INTO templates ({}) VALUES ({placeholders})",
                            columns.join(",")
                        ),
                        values,
                        Some(1),
                    )],
                )
                .await?;
                draft.row.title = draft.fields[0].value.as_str().into();
                return Ok(draft.row);
            }
            save_workflow(executor, draft, Some(old.row.id), false).await
        }
        Command::Delete(draft) => {
            if draft.row.kind.as_ref() == "workflow" {
                return save_workflow(executor, draft, None, true).await;
            }
            protect_self(&draft, viewer)?;
            if draft.row.kind.as_ref() == "folder" {
                return super::folders::remove(executor, draft).await;
            }
            let (predicate, params) = cas(&draft)?;
            let table = table(&draft.row.kind)?;
            let sql = if table == "templates" {
                format!("DELETE FROM {table} WHERE {predicate}")
            } else {
                format!("UPDATE {table} SET deleted_at={NOW},updated_at={NOW} WHERE {predicate}")
            };
            let mut statements = vec![statement(sql, params, Some(1))];
            if table == "templates" {
                statements.push(statement(format!("UPDATE app_settings SET value_json='\"\"',updated_at={NOW} WHERE id='selected_template_id' AND value_json=?"),vec![json!(json!(draft.row.id).to_string())],None));
            }
            transaction(executor, statements).await?;
            Ok(draft.row)
        }
        Command::Pin(draft) => {
            let table = table(&draft.row.kind)?;
            if !matches!(table, "humans" | "organizations" | "templates") {
                return Err(failure("This resource has no pin field"));
            }
            let (predicate, params) = cas(&draft)?;
            transaction(executor, vec![statement(format!("UPDATE {table} SET pinned=NOT pinned,pin_order=CASE WHEN pinned THEN NULL ELSE COALESCE((SELECT MAX(pin_order) FROM {table}),0)+1 END,updated_at={NOW} WHERE {predicate}"), params, Some(1))]).await?;
            Ok(draft.row)
        }
        Command::Merge { primary, duplicate } => merge(executor, primary, duplicate, viewer).await,
    }
}

fn protect_self(draft: &Draft, viewer: Option<&str>) -> Result<()> {
    if draft.row.kind.as_ref() == "human"
        && (draft.row.self_contact
            || Some(draft.row.id.as_ref()) == viewer
            || draft.row.id.as_ref() == LOCAL_OWNER
            || draft
                .base
                .as_ref()
                .is_some_and(|base| base["owner_user_id"] == draft.row.id.as_ref()))
    {
        return Err(failure("Your own contact cannot be deleted"));
    }
    Ok(())
}

pub(super) fn cas(draft: &Draft) -> Result<(String, Vec<Value>)> {
    let base = draft
        .base
        .as_ref()
        .and_then(Value::as_object)
        .ok_or(ServiceError::Conflict)?;
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

async fn save(executor: &DbExecutor, mut draft: Draft, viewer: Option<&str>) -> Result<CatalogRow> {
    let kind = draft.row.kind.as_ref();
    if kind == "workflow" {
        return save_workflow(executor, draft, None, false).await;
    }
    let mut values = BTreeMap::new();
    for field in &draft.fields {
        if !fields(kind).iter().any(|(key, _)| *key == field.key) {
            return Err(failure("Invalid field"));
        }
        if field.value.len() > 65536 {
            return Err(failure("Field exceeds 64 KiB"));
        }
        values.insert(field.key, field.value.clone());
    }
    let title_key = match kind {
        "human" | "organization" => "name",
        "folder" => "path",
        _ => "title",
    };
    let title = values
        .get(title_key)
        .ok_or_else(|| failure("Name is required"))?
        .trim()
        .to_owned();
    if title.is_empty() {
        return Err(failure("Name cannot be empty"));
    }
    values.insert(title_key, title.clone());
    if kind == "folder" {
        return super::folders::save(executor, draft).await;
    }
    if kind == "template" {
        for key in ["sections_json", "targets_json"] {
            let value = values.get(key).map(String::as_str).unwrap_or("");
            if draft
                .base
                .as_ref()
                .is_some_and(|base| base[key].as_str().unwrap_or("") == value)
            {
                continue;
            }
            let parsed: Value = serde_json::from_str(if value.is_empty() { "null" } else { value })
                .map_err(failure)?;
            if !(parsed.is_array() || key == "targets_json" && parsed.is_null()) {
                return Err(failure(format!("{key} must be an array")));
            }
        }
    }
    let table = table(kind)?;
    let mut params: Vec<Value> = values.values().map(|value| json!(value)).collect();
    let sql = if draft.base.is_some() {
        let (predicate, old) = cas(&draft)?;
        params.extend(old);
        format!(
            "UPDATE {table} SET {},updated_at={NOW} WHERE {predicate}",
            values
                .keys()
                .map(|key| format!("{key}=?"))
                .collect::<Vec<_>>()
                .join(",")
        )
    } else {
        params.push(json!(draft.row.id));
        let owner_columns = if matches!(kind, "human" | "organization") {
            ",workspace_id,owner_user_id"
        } else {
            ""
        };
        let owner_values = if owner_columns.is_empty() {
            String::new()
        } else {
            params.push(json!(viewer.unwrap_or(LOCAL_OWNER)));
            format!(
                ",{WORKSPACE},COALESCE((SELECT library_workspace_id FROM local_library_connections WHERE active=1),NULLIF(NULLIF(?,''),'{LOCAL_OWNER}'),{WORKSPACE},'{LOCAL_OWNER}')"
            )
        };
        format!(
            "INSERT INTO {table} ({},id,created_at,updated_at{owner_columns}) VALUES ({},?,{NOW},{NOW}{owner_values})",
            values.keys().copied().collect::<Vec<_>>().join(","),
            vec!["?"; values.len()].join(",")
        )
    };
    transaction(executor, vec![statement(sql, params, Some(1))]).await?;
    draft.row.title = title.into();
    Ok(draft.row)
}

async fn save_workflow(
    executor: &DbExecutor,
    mut draft: Draft,
    duplicate: Option<Arc<str>>,
    delete: bool,
) -> Result<CatalogRow> {
    let current = executor
        .execute(
            "SELECT * FROM app_settings WHERE id='automation_workflows'".into(),
            vec![],
        )
        .await
        .map_err(failure)?
        .into_iter()
        .next();
    if draft.base.is_some() && draft.base != current {
        return Err(ServiceError::Conflict);
    }
    let raw = current
        .as_ref()
        .and_then(|value| value["value_json"].as_str());
    let mut list = match raw {
        Some(raw) => decode_json_text(raw)?,
        None => json!([]),
    };
    let list = list
        .as_array_mut()
        .ok_or_else(|| failure("Workflows must be an array"))?;
    let source_id = duplicate.as_deref().unwrap_or(&draft.row.id);
    let mut value = list.iter().find(|value| value["id"] == source_id).cloned().unwrap_or_else(|| json!({
        "id":draft.row.id,"title":"Untitled automation","enabled":false,"trigger":"note_enhanced","steps":[],"lastRun":null,"processedSessionIds":[],"chatGroupId":null
    }));
    if duplicate.is_some() {
        value["id"] = json!(draft.row.id);
        value["lastRun"] = Value::Null;
        value["processedSessionIds"] = json!([]);
    }
    for field in &draft.fields {
        match field.key {
            "title" => {
                if field.value.trim().is_empty() {
                    return Err(failure("Title cannot be empty"));
                }
                value["title"] = json!(field.value.trim());
            }
            "trigger" if matches!(field.value.as_str(), "note_enhanced" | "meeting_completed") => {
                value["trigger"] = json!(field.value)
            }
            "enabled" => value["enabled"] = json!(field.value.parse::<bool>().map_err(failure)?),
            "steps" => {
                let steps: Value = serde_json::from_str(&field.value).map_err(failure)?;
                if !steps.is_array() {
                    return Err(failure("Steps must be an array"));
                }
                value["steps"] = steps;
            }
            _ => return Err(failure("Invalid automation field or trigger")),
        }
    }
    list.retain(|item| item["id"] != draft.row.id.as_ref());
    if !delete {
        list.push(value.clone());
    }
    let serialized = serde_json::to_string(list).map_err(failure)?;
    let stored = if raw
        .is_some_and(|raw| serde_json::from_str::<Value>(raw).is_ok_and(|value| value.is_string()))
    {
        json!(serialized).to_string()
    } else {
        serialized
    };
    let stmt = if let Some(raw) = raw {
        statement(
            format!(
                "UPDATE app_settings SET value_json=?,updated_at={NOW} WHERE id='automation_workflows' AND value_json=?"
            ),
            vec![json!(stored), json!(raw)],
            Some(1),
        )
    } else {
        statement(
            "INSERT INTO app_settings(id,value_json) VALUES ('automation_workflows',?)".into(),
            vec![json!(stored)],
            Some(1),
        )
    };
    transaction(executor, vec![stmt]).await?;
    draft.row.title = value["title"].as_str().unwrap_or("").into();
    Ok(draft.row)
}

async fn merge(
    executor: &DbExecutor,
    mut primary: Draft,
    duplicate_id: Arc<str>,
    viewer: Option<&str>,
) -> Result<CatalogRow> {
    if primary.row.kind.as_ref() != "human" || primary.row.id == duplicate_id {
        return Err(failure("Choose a different person"));
    }
    let mut duplicate = primary.clone();
    duplicate.row.id = duplicate_id;
    duplicate.row.self_contact = false;
    duplicate.base = Some(load_base(executor, &duplicate.row).await?);
    if protect_self(&duplicate, viewer).is_err() {
        std::mem::swap(&mut primary, &mut duplicate);
    }
    protect_self(&duplicate, viewer)?;
    let (primary_where, primary_params) = cas(&primary)?;
    let (duplicate_where, duplicate_params) = cas(&duplicate)?;
    let left = primary.base.as_ref().ok_or(ServiceError::Conflict)?;
    let right = duplicate.base.as_ref().ok_or(ServiceError::Conflict)?;
    let mut assignments = Vec::new();
    let mut params = Vec::new();
    for key in [
        "job_title",
        "linkedin_username",
        "phone",
        "memo",
        "organization_id",
    ] {
        let a = left[key].as_str().unwrap_or("");
        let b = right[key].as_str().unwrap_or("");
        let merged = if a.is_empty() {
            b.to_owned()
        } else if b.is_empty() || key == "organization_id" {
            a.to_owned()
        } else {
            format!("{a}, {b}")
        };
        assignments.push(format!("{key}=?"));
        params.push(json!(merged));
    }
    params.extend(primary_params);
    transaction(executor, vec![
        statement(format!("UPDATE humans SET {},updated_at={NOW} WHERE {primary_where}", assignments.join(",")), params, Some(1)),
        statement(format!("UPDATE humans SET deleted_at={NOW},updated_at={NOW} WHERE {duplicate_where}"), duplicate_params, Some(1)),
        statement(format!("UPDATE session_participants AS d SET deleted_at={NOW},updated_at={NOW} WHERE human_id=? AND deleted_at IS NULL AND EXISTS (SELECT 1 FROM session_participants p WHERE p.session_id=d.session_id AND p.human_id=? AND p.deleted_at IS NULL)"), vec![json!(duplicate.row.id),json!(primary.row.id)],None),
        statement(format!("UPDATE session_participants SET human_id=?,updated_at={NOW} WHERE human_id=? AND deleted_at IS NULL"),vec![json!(primary.row.id),json!(duplicate.row.id)],None),
    ]).await?;
    Ok(primary.row)
}
