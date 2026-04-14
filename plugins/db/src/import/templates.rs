use std::path::Path;

use hypr_db_app::UpsertTemplate;
use sqlx::SqlitePool;

pub async fn import_legacy_templates_from_path(
    pool: &SqlitePool,
    path: &Path,
) -> crate::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let templates = read_template_file(path)?;
    for template in templates {
        hypr_db_app::insert_template_if_missing(
            pool,
            UpsertTemplate {
                id: &template.id,
                title: &template.title,
                description: &template.description,
                pinned: template.pinned,
                pin_order: template.pin_order,
                category: template.category.as_deref(),
                targets_json: template.targets_json.as_deref(),
                sections_json: &template.sections_json,
            },
        )
        .await?;
    }

    Ok(())
}

fn read_template_file(path: &Path) -> crate::Result<Vec<ParsedTemplate>> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_template_file(&content))
}

fn parse_template_file(content: &str) -> Vec<ParsedTemplate> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };
    let Some(object) = value.as_object() else {
        return Vec::new();
    };

    object
        .iter()
        .map(|(id, row)| ParsedTemplate {
            id: id.clone(),
            title: get_string(row, "title"),
            description: get_string(row, "description"),
            pinned: get_bool(row, "pinned"),
            pin_order: get_i64(row, "pin_order"),
            category: get_optional_string(row, "category"),
            targets_json: normalize_targets_json(row.get("targets")),
            sections_json: normalize_sections_json(row.get("sections")),
        })
        .collect()
}

fn get_string(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

fn get_optional_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn get_bool(value: &serde_json::Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn get_i64(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|value| value.as_i64())
}

fn normalize_targets_json(raw: Option<&serde_json::Value>) -> Option<String> {
    let normalized = normalize_string_array(raw?)?;
    serde_json::to_string(&normalized).ok()
}

fn normalize_sections_json(raw: Option<&serde_json::Value>) -> String {
    let normalized = normalize_template_sections(raw);
    serde_json::to_string(&normalized).unwrap_or_else(|_| "[]".to_string())
}

fn normalize_string_array(raw: &serde_json::Value) -> Option<Vec<String>> {
    let value = parse_nested_json_value(raw);

    match value {
        serde_json::Value::String(item) => {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(vec![trimmed.to_string()])
            }
        }
        serde_json::Value::Array(items) => {
            let normalized = items
                .into_iter()
                .filter_map(|item| item.as_str().map(str::trim).map(ToString::to_string))
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>();

            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        }
        _ => None,
    }
}

fn normalize_template_sections(raw: Option<&serde_json::Value>) -> Vec<serde_json::Value> {
    let Some(raw) = raw else {
        return Vec::new();
    };

    let value = parse_nested_json_value(raw);
    let items = match value {
        serde_json::Value::String(item) => vec![serde_json::Value::String(item)],
        serde_json::Value::Array(items) => items,
        _ => return Vec::new(),
    };

    items
        .into_iter()
        .filter_map(|item| match item {
            serde_json::Value::String(title) => {
                let title = title.trim();
                if title.is_empty() {
                    return None;
                }

                Some(serde_json::json!({
                    "title": title,
                    "description": "",
                }))
            }
            serde_json::Value::Object(section) => {
                let title = section
                    .get("title")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())?;
                let description = section
                    .get("description")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty());

                Some(serde_json::json!({
                    "title": title,
                    "description": description.unwrap_or(""),
                }))
            }
            _ => None,
        })
        .collect()
}

fn parse_nested_json_value(raw: &serde_json::Value) -> serde_json::Value {
    let Some(text) = raw.as_str() else {
        return raw.clone();
    };

    serde_json::from_str(text).unwrap_or_else(|_| raw.clone())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedTemplate {
    id: String,
    title: String,
    description: String,
    pinned: bool,
    pin_order: Option<i64>,
    category: Option<String>,
    targets_json: Option<String>,
    sections_json: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core2::Db3;

    async fn test_db() -> Db3 {
        let db = Db3::connect_memory_plain().await.unwrap();
        hypr_db_app::migrate(db.pool()).await.unwrap();
        db
    }

    #[test]
    fn parse_template_file_accepts_string_and_json_shapes() {
        let parsed = parse_template_file(
            r#"{
              "template-1": {

                "title": "Weekly",
                "description": "Agenda",
                "pinned": true,
                "pin_order": 4,
                "category": "meetings",
                "targets": ["eng"],
                "sections": [{"title":"Notes","description":"Capture"}]
              },
              "template-2": {

                "title": "1:1",
                "description": "",
                "pinned": false,
                "targets": "[\"exec\"]",
                "sections": "[{\"title\":\"Summary\",\"description\":\"Text\"}]"
              },
              "template-3": {

                "title": "Retro",
                "description": "",
                "targets": "manager",
                "sections": ["Wins", "Risks"]
              }
            }"#,
        );

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].targets_json.as_deref(), Some("[\"eng\"]"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&parsed[0].sections_json).unwrap(),
            serde_json::json!([{ "title": "Notes", "description": "Capture" }])
        );
        assert_eq!(parsed[1].targets_json.as_deref(), Some("[\"exec\"]"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&parsed[1].sections_json).unwrap(),
            serde_json::json!([{ "title": "Summary", "description": "Text" }])
        );
        assert_eq!(parsed[2].targets_json.as_deref(), Some("[\"manager\"]"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&parsed[2].sections_json).unwrap(),
            serde_json::json!([
                { "title": "Wins", "description": "" },
                { "title": "Risks", "description": "" }
            ])
        );
    }

    #[tokio::test]
    async fn import_legacy_templates_from_path_imports_missing_templates_without_overwriting() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(super::super::TEMPLATES_FILENAME);
        std::fs::write(
            &path,
            r#"{
              "template-1": {

                "title": "Weekly",
                "description": "Agenda",
                "pinned": true,
                "pin_order": 4,
                "category": "meetings",
                "targets": "[\"eng\"]",
                "sections": "[{\"title\":\"Notes\",\"description\":\"Capture\"}]"
              }
            }"#,
        )
        .unwrap();

        import_legacy_templates_from_path(db.pool(), &path)
            .await
            .unwrap();

        let row = hypr_db_app::get_template(db.pool(), "template-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.title, "Weekly");
        assert_eq!(row.targets_json.as_deref(), Some("[\"eng\"]"));

        std::fs::write(
            &path,
            r#"{
              "template-1": {

                "title": "Changed Existing",
                "description": "Should not overwrite",
                "sections": "[{\"title\":\"Ignored\",\"description\":\"Ignored\"}]"
              },
              "template-2": {

                "title": "Changed",
                "description": "Should import",
                "sections": "[]"
              }
            }"#,
        )
        .unwrap();

        import_legacy_templates_from_path(db.pool(), &path)
            .await
            .unwrap();

        let existing_row = hypr_db_app::get_template(db.pool(), "template-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(existing_row.title, "Weekly");
        assert_eq!(existing_row.description, "Agenda");

        let imported_row = hypr_db_app::get_template(db.pool(), "template-2")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(imported_row.title, "Changed");
        assert_eq!(imported_row.description, "Should import");
    }

    #[tokio::test]
    async fn import_legacy_templates_from_path_preserves_seeded_rows_and_adds_missing_ones() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(super::super::TEMPLATES_FILENAME);

        hypr_db_app::upsert_template(
            db.pool(),
            UpsertTemplate {
                id: "template-1",
                title: "Seeded",
                description: "Keep this",
                pinned: false,
                pin_order: None,
                category: None,
                targets_json: None,
                sections_json: "[]",
            },
        )
        .await
        .unwrap();

        std::fs::write(
            &path,
            r#"{
              "template-1": {

                "title": "Legacy Existing",
                "description": "Should stay ignored",
                "sections": "[{\"title\":\"Ignored\",\"description\":\"Ignored\"}]"
              },
              "template-2": {

                "title": "Legacy New",
                "description": "Should be inserted",
                "sections": "[{\"title\":\"Notes\",\"description\":\"Added\"}]"
              }
            }"#,
        )
        .unwrap();

        import_legacy_templates_from_path(db.pool(), &path)
            .await
            .unwrap();

        let seeded_row = hypr_db_app::get_template(db.pool(), "template-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(seeded_row.title, "Seeded");
        assert_eq!(seeded_row.description, "Keep this");

        let inserted_row = hypr_db_app::get_template(db.pool(), "template-2")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(inserted_row.title, "Legacy New");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&inserted_row.sections_json).unwrap(),
            serde_json::json!([{ "title": "Notes", "description": "Added" }])
        );
    }
}
