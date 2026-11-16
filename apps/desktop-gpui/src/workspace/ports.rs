use std::sync::Arc;

use desktop_runtime::{CancellationToken, Reply, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};

pub const PAGE_SIZE: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Catalog {
    Contacts,
    Folders,
    Templates,
    Automations,
}

impl Catalog {
    pub fn label(self) -> &'static str {
        match self {
            Self::Contacts => "Contacts",
            Self::Folders => "Folders",
            Self::Templates => "Templates",
            Self::Automations => "Automations",
        }
    }

    pub fn limitation(self) -> &'static str {
        match self {
            Self::Contacts => "Local contact create, edit, pin and merge are available.",
            Self::Folders => {
                "Folder create, rename, move, deletion and materials use the local vault."
            }
            Self::Templates => {
                "Local template editing, duplication, deletion and pinning are available."
            }
            Self::Automations => {
                "Local Markdown export runs directly; provider steps require a signed-in connection."
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogRow {
    pub id: Arc<str>,
    pub kind: Arc<str>,
    pub title: Arc<str>,
    pub subtitle: Arc<str>,
    pub pinned: bool,
    pub self_contact: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CatalogPage {
    pub rows: Arc<[CatalogRow]>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Detail {
    pub title: Arc<str>,
    pub fields: Arc<[(Arc<str>, Arc<str>)]>,
    pub warnings: Arc<[Arc<str>]>,
}

#[derive(Clone)]
pub struct CatalogQuery {
    pub catalog: Catalog,
    pub search: Arc<str>,
    pub offset: u32,
}

impl CatalogQuery {
    pub fn sql(&self) -> (String, Vec<Value>) {
        self.sql_for_viewer(None)
    }

    pub(super) fn sql_for_viewer(&self, viewer: Option<&str>) -> (String, Vec<Value>) {
        let search = format!("%{}%", escape_like(&self.search));
        let sql = match self.catalog {
            Catalog::Contacts => "SELECT id, kind, substr(title,1,4096) AS title,
                substr(subtitle,1,4096) AS subtitle, pinned, self_contact FROM (
                SELECT id, 'human' AS kind, name AS title, email AS subtitle, pinned,
                    pin_order, CASE WHEN id = ?4 THEN 2
                        WHEN id = '00000000-0000-0000-0000-000000000000' THEN 1
                        ELSE 0 END AS self_contact
                FROM humans WHERE deleted_at IS NULL
                UNION ALL SELECT id, 'organization' AS kind, name AS title,
                    'Organization' AS subtitle, pinned, pin_order, 0 AS self_contact
                FROM organizations WHERE deleted_at IS NULL
                ) WHERE self_contact OR title LIKE ?1 ESCAPE '\\' OR subtitle LIKE ?1 ESCAPE '\\'
                ORDER BY self_contact DESC, pinned DESC, pin_order IS NULL, pin_order,
                    title COLLATE NOCASE, kind, id LIMIT ?2 OFFSET ?3",
            Catalog::Folders => "SELECT id, 'folder' AS kind, substr(path,1,4096) AS title,
                '' AS subtitle, 0 AS pinned, 0 AS self_contact
                FROM folders WHERE deleted_at IS NULL AND path LIKE ?1 ESCAPE '\\'
                ORDER BY path COLLATE NOCASE, id LIMIT ?2 OFFSET ?3",
            Catalog::Templates => "SELECT id, 'template' AS kind, substr(title,1,4096) AS title,
                substr(COALESCE(category,''),1,4096) AS subtitle, pinned, 0 AS self_contact
                FROM templates WHERE title LIKE ?1 ESCAPE '\\'
                ORDER BY pinned DESC, pin_order IS NULL, pin_order, title COLLATE NOCASE, id
                LIMIT ?2 OFFSET ?3",
            Catalog::Automations => "WITH setting AS (
                SELECT value_json FROM app_settings WHERE id = 'automation_workflows'
                ), decoded AS (
                SELECT CASE WHEN json_valid(value_json) THEN
                    CASE WHEN json_type(value_json) = 'text' THEN json_extract(value_json,'$')
                    ELSE value_json END ELSE json(value_json) END AS body FROM setting
                )
                SELECT json_extract(value,'$.id') AS id, 'workflow' AS kind,
                    substr(COALESCE(json_extract(value,'$.title'),'Untitled automation'),1,4096) AS title,
                    CASE WHEN json_extract(value,'$.enabled') = 1 THEN 'Enabled' ELSE 'Disabled' END AS subtitle,
                    0 AS pinned, 0 AS self_contact
                FROM decoded, json_each(json(body))
                WHERE json_type(value,'$.id') = 'text'
                    AND COALESCE(json_extract(value,'$.title'),'Untitled automation') LIKE ?1 ESCAPE '\\'
                ORDER BY title COLLATE NOCASE, id LIMIT ?2 OFFSET ?3",
        };
        let mut params = vec![json!(search), json!(PAGE_SIZE + 1), json!(self.offset)];
        if self.catalog == Catalog::Contacts {
            params.push(json!(viewer));
        }
        (sql.into(), params)
    }
}

pub fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn text(row: &Value, key: &str) -> Arc<str> {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}

fn flag(row: &Value, key: &str) -> bool {
    row.get(key)
        .is_some_and(|value| value == true || value.as_i64().is_some_and(|number| number > 0))
}

pub fn decode_catalog(rows: &[Value]) -> Result<CatalogPage> {
    let mut items = Vec::with_capacity(rows.len().min(PAGE_SIZE));
    for row in rows.iter().take(PAGE_SIZE) {
        let id = text(row, "id");
        if id.is_empty() {
            return Err(ServiceError::Failed(
                "A catalog row has no stable ID.".into(),
            ));
        }
        items.push(CatalogRow {
            id,
            kind: text(row, "kind"),
            title: text(row, "title"),
            subtitle: text(row, "subtitle"),
            pinned: flag(row, "pinned"),
            self_contact: flag(row, "self_contact"),
        });
    }
    Ok(CatalogPage {
        rows: items.into(),
        has_more: rows.len() > PAGE_SIZE,
    })
}

pub fn catalog_page(
    runtime: &RuntimeHandle,
    query: CatalogQuery,
    cancel: CancellationToken,
) -> Result<Reply<CatalogPage>> {
    runtime.read(cancel, move |services| async move {
        let (sql, params) = query.sql();
        let rows = services
            .executor
            .execute(sql, params)
            .await
            .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
        decode_catalog(&rows)
    })
}

pub fn catalog_detail(
    runtime: &RuntimeHandle,
    row: CatalogRow,
    cancel: CancellationToken,
) -> Result<Reply<Detail>> {
    runtime.read(cancel, move |services| async move {
        let sql = match row.kind.as_ref() {
            "human" => {
                "SELECT substr(name,1,4096) AS title, substr(email,1,4096) AS Email,
                substr(phone,1,4096) AS Phone, substr(job_title,1,4096) AS Role,
                substr(memo,1,65536) AS Notes, organization_id AS Organization
                FROM humans WHERE id = ? AND deleted_at IS NULL"
            }
            "organization" => {
                "SELECT substr(name,1,4096) AS title, substr(memo,1,65536) AS Notes
                FROM organizations WHERE id = ? AND deleted_at IS NULL"
            }
            "folder" => {
                "SELECT substr(path,1,4096) AS title, substr(instructions,1,65536) AS Instructions
                FROM folders WHERE id = ? AND deleted_at IS NULL"
            }
            "template" => {
                "SELECT substr(title,1,4096) AS title,
                substr(description,1,65536) AS Description, substr(category,1,4096) AS Category,
                substr(targets_json,1,65536) AS targets_json,
                substr(sections_json,1,65536) AS sections_json
                FROM templates WHERE id = ?"
            }
            "workflow" => {
                "WITH setting AS (
                SELECT value_json FROM app_settings WHERE id = 'automation_workflows'
                ), decoded AS (
                SELECT CASE WHEN json_valid(value_json) THEN
                    CASE WHEN json_type(value_json) = 'text' THEN json_extract(value_json,'$')
                    ELSE value_json END ELSE '[]' END AS body FROM setting
                ) SELECT substr(value,1,65536) AS workflow
                FROM decoded, json_each(CASE WHEN json_valid(body) THEN body ELSE '[]' END)
                WHERE json_extract(value,'$.id') = ? LIMIT 1"
            }
            _ => return Err(ServiceError::Unsupported("Unknown catalog resource".into())),
        };
        let rows = services
            .executor
            .execute(sql.into(), vec![json!(row.id)])
            .await
            .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
        let value = rows.first().ok_or(ServiceError::Conflict)?;
        let mut fields = Vec::new();
        let mut warnings: Vec<Arc<str>> = Vec::new();
        let title = if row.kind.as_ref() == "workflow" {
            let workflow = decode_json_text(&text(value, "workflow"))?;
            fields.push(("Trigger".into(), text(&workflow, "trigger")));
            fields.push((
                "Configuration".into(),
                if super::automations::workflow_configured(&workflow) {
                    "All steps are configured. Provider steps require a signed-in connection."
                        .into()
                } else {
                    "One or more steps need configuration.".into()
                },
            ));
            fields.push(("Chat group".into(), text(&workflow, "chatGroupId")));
            fields.push((
                "Status".into(),
                if flag(&workflow, "enabled") {
                    "Enabled".into()
                } else {
                    "Disabled".into()
                },
            ));
            let steps = workflow.get("steps").and_then(Value::as_array);
            fields.push((
                "Steps".into(),
                steps
                    .map(|steps| {
                        steps
                            .iter()
                            .map(|step| text(step, "type").to_string())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default()
                    .into(),
            ));
            if let Some(run) = workflow.get("lastRun").filter(|run| !run.is_null()) {
                fields.push((
                    "Last run".into(),
                    format!(
                        "{} — {}\n{}",
                        text(run, "at"),
                        text(run, "status"),
                        text(run, "detail")
                    )
                    .into(),
                ));
            }
            text(&workflow, "title")
        } else {
            if let Some(object) = value.as_object() {
                for (key, value) in object {
                    if key != "title" && !key.ends_with("_json") {
                        fields.push((
                            key.as_str().into(),
                            value.as_str().unwrap_or_default().into(),
                        ));
                    }
                }
            }
            if row.kind.as_ref() == "template" {
                let template = super::templates::StoredTemplate::decode(
                    text(value, "sections_json"),
                    text(value, "targets_json"),
                );
                for section in template.sections {
                    fields.push((section.title, section.description));
                }
                fields.push((
                    "Targets".into(),
                    template
                        .targets
                        .iter()
                        .map(AsRef::as_ref)
                        .collect::<Vec<_>>()
                        .join(", ")
                        .into(),
                ));
                warnings.extend(template.warnings);
            }
            text(value, "title")
        };
        if value.as_object().is_some_and(|object| {
            object.values().any(|value| {
                value
                    .as_str()
                    .is_some_and(|text| text.chars().count() >= 65536)
            })
        }) {
            warnings.push(
                "Large fields are truncated for display; original stored data is unchanged.".into(),
            );
        }
        Ok(Detail {
            title,
            fields: fields.into(),
            warnings: warnings.into(),
        })
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_title_filters_escape_sql_wildcards() {
        assert_eq!(escape_like("50%_\\notes"), "50\\%\\_\\\\notes");
    }

    #[test]
    fn catalog_limit_is_explicit_and_not_silent() {
        let rows = (0..101)
            .map(|index| {
                json!({
                    "id":index.to_string(), "kind":"template", "title":"x"
                })
            })
            .collect::<Vec<_>>();
        let page = decode_catalog(&rows).unwrap();
        assert_eq!(page.rows.len(), 100);
        assert!(page.has_more);
        assert!(decode_catalog(&[json!({"title":"No ID"})]).is_err());
    }
}
