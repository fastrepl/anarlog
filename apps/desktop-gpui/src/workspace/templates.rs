use std::sync::Arc;

use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Section {
    pub title: Arc<str>,
    pub description: Arc<str>,
}

#[derive(Debug)]
pub struct StoredTemplate {
    pub sections: Vec<Section>,
    pub targets: Vec<Arc<str>>,
    pub warnings: Vec<Arc<str>>,
    pub raw_sections: Arc<str>,
    pub raw_targets: Arc<str>,
}

impl StoredTemplate {
    pub fn decode(raw_sections: Arc<str>, raw_targets: Arc<str>) -> Self {
        let sections = decode_sections(&raw_sections);
        let targets = decode_targets(&raw_targets);
        let mut warnings = Vec::new();
        if let Err(error) = &sections {
            warnings.push(error.clone());
        }
        if let Err(error) = &targets {
            warnings.push(error.clone());
        }
        Self {
            sections: sections.unwrap_or_default(),
            targets: targets.unwrap_or_default(),
            warnings,
            raw_sections,
            raw_targets,
        }
    }
}

fn decode_sections(raw: &str) -> Result<Vec<Section>, Arc<str>> {
    let value =
        super::ports::decode_json_text(raw).map_err(|error| Arc::<str>::from(error.to_string()))?;
    let rows = value
        .as_array()
        .ok_or_else(|| Arc::from("Sections must be an array; stored data is preserved."))?;
    let mut sections = Vec::new();
    for row in rows {
        if let Some(title) = row.as_str() {
            if !title.trim().is_empty() {
                sections.push(Section {
                    title: title.trim().into(),
                    description: "".into(),
                });
            }
        } else {
            let title = row.get("title").and_then(Value::as_str).ok_or_else(|| {
                Arc::from("A stored section has an invalid title; stored data is preserved.")
            })?;
            let description =
                match row.get("description") {
                    None => "",
                    Some(Value::String(description)) => description,
                    _ => return Err(
                        "A stored section has an invalid description; stored data is preserved."
                            .into(),
                    ),
                };
            sections.push(Section {
                title: title.trim().into(),
                description: description.into(),
            });
        }
    }
    Ok(sections)
}

fn decode_targets(raw: &str) -> Result<Vec<Arc<str>>, Arc<str>> {
    if raw.is_empty() || raw == "null" {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(raw)
        .map_err(|error| Arc::from(format!("Invalid stored targets: {error}")))?;
    if let Some(target) = value.as_str() {
        return Ok(if target.trim().is_empty() {
            vec![]
        } else {
            vec![target.trim().into()]
        });
    }
    let targets = value
        .as_array()
        .ok_or_else(|| Arc::from("Targets must be strings; stored data is preserved."))?;
    Ok(targets
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(Into::into)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_persisted_rows_keep_original_bytes_and_warning() {
        let template = StoredTemplate::decode("{broken".into(), "[false, \" Engineer \"]".into());
        assert!(template.sections.is_empty());
        assert!(!template.warnings.is_empty());
        assert_eq!(template.raw_sections.as_ref(), "{broken");
        assert_eq!(template.raw_targets.as_ref(), "[false, \" Engineer \"]");
        assert_eq!(template.targets, vec![Arc::<str>::from("Engineer")]);
    }

    #[test]
    fn legacy_strings_nested_json_and_blank_drafts_are_visible() {
        let raw =
            serde_json::to_string(r#"[" Legacy ",{"title":"","description":"draft"}]"#).unwrap();
        let template = StoredTemplate::decode(raw.into(), r#""Team""#.into());
        assert!(template.warnings.is_empty());
        assert_eq!(template.sections.len(), 2);
        assert_eq!(template.sections[0].title.as_ref(), "Legacy");
        assert_eq!(template.sections[1].description.as_ref(), "draft");
        assert_eq!(template.targets[0].as_ref(), "Team");
    }
}
