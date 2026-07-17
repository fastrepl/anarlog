use hypr_db_app::{LegacyImportBatch, LegacyImportRow, LegacyTemplate as ImportedTemplate};

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

pub(super) fn parse_legacy_templates(content: &str) -> Result<LegacyImportBatch, String> {
    let value =
        serde_json::from_str::<serde_json::Value>(content).map_err(|error| error.to_string())?;
    if !value.is_object() {
        return Err("JSON root is not an object".to_string());
    }

    let rows = parse_template_file(content)
        .into_iter()
        .map(|template| {
            LegacyImportRow::Template(ImportedTemplate {
                id: template.id,
                title: template.title,
                description: template.description,
                pinned: template.pinned,
                pin_order: template.pin_order,
                category: template.category,
                targets_json: template.targets_json,
                sections_json: template.sections_json,
            })
        })
        .collect();

    Ok(LegacyImportBatch {
        rows,
        ..Default::default()
    })
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
}
