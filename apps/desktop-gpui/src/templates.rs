//! Templates tab data: `templates/queries.ts` (`UserTemplate`, create / save /
//! delete / favourite) and `templates/codec.ts`'s lenient stored-row parsers.

use sqlx::SqlitePool;

use crate::db::TemplateIcon;

pub const AUTO_TEMPLATE_ID: &str = "__auto__";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserTemplate {
    pub id: String,
    pub title: String,
    pub description: String,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    pub category: Option<String>,
    pub icon: TemplateIcon,
    pub targets: Option<Vec<String>>,
    pub sections: Vec<Section>,
}

/// `UserTemplateDraft`
#[derive(Debug, Clone, Default)]
pub struct Draft {
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub icon: Option<TemplateIcon>,
    pub targets: Option<Vec<String>>,
    pub sections: Vec<Section>,
}

const LIST_SQL: &str = "
  SELECT id, title, description, pinned, pin_order, category, icon_json, targets_json, sections_json
  FROM templates
  ORDER BY id
";

const GET_SQL: &str = "
  SELECT id, title, description, pinned, pin_order, category, icon_json, targets_json, sections_json
  FROM templates
  WHERE id = ?
  LIMIT 1
";

type Row = (
    String,
    String,
    String,
    i64,
    Option<i64>,
    Option<String>,
    String,
    Option<String>,
    String,
);

/// `parseStoredTemplateSections` with `allowBareStrings`, blank drafts kept,
/// description optional; an unparseable column yields no sections.
pub fn parse_sections(json: &str) -> Vec<Section> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(json)
    else {
        return Vec::new();
    };
    let mut sections = Vec::new();
    for item in items {
        match item {
            serde_json::Value::String(title) => {
                let title = title.trim();
                if !title.is_empty() {
                    sections.push(Section {
                        title: title.to_string(),
                        description: String::new(),
                    });
                }
            }
            serde_json::Value::Object(map) => {
                let Some(title) = map.get("title").and_then(serde_json::Value::as_str) else {
                    return Vec::new();
                };
                let description = match map.get("description") {
                    None | Some(serde_json::Value::Null) => "",
                    Some(serde_json::Value::String(text)) => text.as_str(),
                    Some(_) => return Vec::new(),
                };
                sections.push(Section {
                    title: title.trim().to_string(),
                    description: description.to_string(),
                });
            }
            _ => return Vec::new(),
        }
    }
    sections
}

/// `parseStoredTemplateTargets` (`lenient`): trimmed non-empty strings.
pub fn parse_targets(json: Option<&str>) -> Option<Vec<String>> {
    let json = json?;
    let serde_json::Value::Array(items) = serde_json::from_str::<serde_json::Value>(json).ok()?
    else {
        return None;
    };
    Some(
        items
            .into_iter()
            .filter_map(|item| item.as_str().map(str::trim).map(str::to_string))
            .filter(|target| !target.is_empty())
            .collect(),
    )
}

fn from_row(
    (id, title, description, pinned, pin_order, category, icon_json, targets_json, sections_json): Row,
) -> UserTemplate {
    UserTemplate {
        icon: serde_json::from_str::<serde_json::Value>(&icon_json)
            .ok()
            .and_then(|icon| TemplateIcon::from_json(&icon))
            .unwrap_or_else(TemplateIcon::default_template),
        targets: parse_targets(targets_json.as_deref()),
        sections: parse_sections(&sections_json),
        id,
        title,
        description,
        pinned: pinned != 0,
        pin_order,
        category,
    }
}

fn sections_json(sections: &[Section]) -> String {
    serde_json::Value::Array(
        sections
            .iter()
            .map(|section| {
                serde_json::json!({ "title": section.title, "description": section.description })
            })
            .collect(),
    )
    .to_string()
}

fn targets_json(targets: Option<&[String]>) -> Option<String> {
    targets.map(|targets| serde_json::Value::from(targets.to_vec()).to_string())
}

/// `useUserTemplates`: `ORDER BY id`.
pub async fn list(pool: &SqlitePool) -> anyhow::Result<Vec<UserTemplate>> {
    let rows: Vec<Row> = sqlx::query_as(LIST_SQL).fetch_all(pool).await?;
    Ok(rows.into_iter().map(from_row).collect())
}

pub async fn get(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<UserTemplate>> {
    let row: Option<Row> = sqlx::query_as(GET_SQL)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(from_row))
}

/// `useCreateTemplate`
pub async fn create(pool: &SqlitePool, draft: &Draft) -> anyhow::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let icon = draft
        .icon
        .clone()
        .unwrap_or_else(TemplateIcon::default_template);
    sqlx::query(
        "INSERT INTO templates (id, title, description, pinned, category, icon_json, targets_json, sections_json, created_at, updated_at)
         VALUES (?, ?, ?, 0, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
    )
    .bind(&id)
    .bind(&draft.title)
    .bind(&draft.description)
    .bind(&draft.category)
    .bind(icon.to_json().to_string())
    .bind(targets_json(draft.targets.as_deref()))
    .bind(sections_json(&draft.sections))
    .execute(pool)
    .await?;
    Ok(id)
}

/// `useSaveTemplate`
pub async fn save(pool: &SqlitePool, template: &UserTemplate) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE templates
         SET title = ?, description = ?, pinned = ?, pin_order = ?, category = ?, icon_json = ?,
             targets_json = ?, sections_json = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE id = ?",
    )
    .bind(&template.title)
    .bind(&template.description)
    .bind(template.pinned as i64)
    .bind(template.pin_order)
    .bind(&template.category)
    .bind(template.icon.to_json().to_string())
    .bind(targets_json(template.targets.as_deref()))
    .bind(sections_json(&template.sections))
    .bind(&template.id)
    .execute(pool)
    .await?;
    Ok(())
}

/// `useDeleteTemplate`
pub async fn delete(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM templates WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// `useToggleTemplateFavorite`: unpin resets `pin_order` to 0, pin takes
/// `max(pin_order) + 1` over the other templates.
pub async fn toggle_favorite(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    let Some(mut template) = get(pool, id).await? else {
        return Ok(());
    };
    if template.pinned {
        template.pinned = false;
        template.pin_order = Some(0);
    } else {
        let (max_order,): (Option<i64>,) =
            sqlx::query_as("SELECT MAX(pin_order) FROM templates WHERE id != ?")
                .bind(id)
                .fetch_one(pool)
                .await?;
        template.pinned = true;
        template.pin_order = Some(max_order.unwrap_or(0) + 1);
    }
    save(pool, &template).await
}

/// `getTemplateCopyTitle`
pub fn copy_title(title: &str) -> String {
    let value = title.trim();
    if value.is_empty() {
        return "Untitled (Copy)".to_string();
    }
    if value.ends_with("(Copy)") {
        return value.to_string();
    }
    format!("{value} (Copy)")
}

/// `templateCommands.getTemplateSource("enhanceFormat")`
pub fn default_auto_format() -> &'static str {
    anlg_template_app::template_source(anlg_template_app::EditableTemplate::EnhanceFormat)
}

/// `normalizeFormat`
pub fn normalize_format(value: &str) -> String {
    value.replace("\r\n", "\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_sections_accept_bare_strings_and_blank_drafts() {
        let sections = parse_sections(
            r#"["Agenda", " ", {"title": "", "description": "draft"}, {"title": "Notes"}]"#,
        );
        assert_eq!(
            sections,
            vec![
                Section {
                    title: "Agenda".into(),
                    description: String::new()
                },
                Section {
                    title: String::new(),
                    description: "draft".into()
                },
                Section {
                    title: "Notes".into(),
                    description: String::new()
                },
            ]
        );
        assert!(parse_sections("not json").is_empty());
        assert!(parse_sections(r#"[{"title": 3}]"#).is_empty());
    }

    #[test]
    fn stored_targets_are_lenient() {
        assert_eq!(parse_targets(None), None);
        assert_eq!(
            parse_targets(Some(r#"["Founder", " CEO ", "", 4]"#)),
            Some(vec!["Founder".to_string(), "CEO".to_string()])
        );
    }

    #[test]
    fn copy_titles_match_the_app() {
        assert_eq!(copy_title(""), "Untitled (Copy)");
        assert_eq!(copy_title("Board (Copy)"), "Board (Copy)");
        assert_eq!(copy_title(" Board "), "Board (Copy)");
    }

    #[test]
    fn default_auto_format_is_the_enhance_format_template() {
        assert!(!default_auto_format().trim().is_empty());
    }
}
