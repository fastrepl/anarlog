//! Templates as the shipping templates route sees them (`apps/desktop/src/templates`): the
//! live library, its sidebar ordering and the writes behind create, edit, favorite, default,
//! duplicate and delete.

use std::{cmp::Ordering, sync::Arc};

use desktop_runtime::{Reply, Result, RuntimeHandle};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    mutations::{failure, statement, transaction},
    resource_list::Glyph,
};

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

fn decode_sections(raw: &str) -> std::result::Result<Vec<Section>, Arc<str>> {
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

fn decode_targets(raw: &str) -> std::result::Result<Vec<Arc<str>>, Arc<str>> {
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

/// Shipping stamps template rows with second precision.
const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%SZ','now')";

#[derive(Clone, Debug, PartialEq)]
pub struct Template {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub description: Arc<str>,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    pub category: Arc<str>,
    pub icon: Glyph,
    pub sections: Vec<Section>,
    pub targets: Vec<Arc<str>>,
    /// Stored sections or targets that could not be decoded; editing them is disabled so the
    /// original bytes survive.
    pub warnings: Vec<Arc<str>>,
}

impl Template {
    pub fn display_title(&self) -> &str {
        match self.title.trim() {
            "" => "Untitled",
            title => title,
        }
    }
}

/// The templates in id order (shipping's live query) plus the two settings the route reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    pub templates: Vec<Template>,
    pub default_id: Arc<str>,
    pub auto_customized: bool,
}

pub const LIBRARY_SQL: &str =
    "SELECT 'template' AS kind, id, title, description, pinned, pin_order,
        category, icon_json, targets_json, sections_json FROM templates
    UNION ALL
    SELECT 'setting', id, value_json, NULL, NULL, NULL, NULL, NULL, NULL, NULL
        FROM app_settings WHERE id IN ('selected_template_id', 'auto_summary_prompt')
    ORDER BY 2 LIMIT 2000";

fn text(row: &Value, key: &str) -> Arc<str> {
    row[key].as_str().unwrap_or_default().into()
}

fn setting(rows: &[Value], id: &str) -> String {
    rows.iter()
        .find(|row| row["kind"] == "setting" && row["id"] == id)
        .and_then(|row| row["title"].as_str())
        .and_then(|raw| serde_json::from_str::<String>(raw).ok())
        .unwrap_or_default()
}

pub fn decode_library(rows: &[Value]) -> Library {
    let templates = rows
        .iter()
        .filter(|row| row["kind"] == "template")
        .map(|row| {
            let stored =
                StoredTemplate::decode(text(row, "sections_json"), text(row, "targets_json"));
            Template {
                id: text(row, "id"),
                title: text(row, "title"),
                description: text(row, "description"),
                pinned: row["pinned"].as_i64().is_some_and(|pinned| pinned != 0)
                    || row["pinned"] == true,
                pin_order: row["pin_order"].as_i64(),
                category: text(row, "category"),
                icon: Glyph::template(row["icon_json"].as_str().unwrap_or_default()),
                sections: stored.sections,
                targets: stored.targets,
                warnings: stored.warnings,
            }
        })
        .collect();
    Library {
        templates,
        default_id: setting(rows, "selected_template_id").into(),
        auto_customized: !setting(rows, "auto_summary_prompt").trim().is_empty(),
    }
}

/// `localeCompare` for titles: case-insensitive first, lowercase before uppercase on ties.
fn locale_cmp(a: &str, b: &str) -> Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| b.cmp(a))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Sort {
    #[default]
    AToZ,
    ZToA,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Selection {
    Auto,
    Mine(Arc<str>),
}

/// The sidebar list: Auto, favorites by pin order, then the rest by title, filtered by search.
pub fn visible(library: &Library, search: &str, sort: Sort) -> Vec<Selection> {
    let mut favorites = library
        .templates
        .iter()
        .filter(|template| template.pinned)
        .collect::<Vec<_>>();
    favorites.sort_by(|a, b| {
        let order = |template: &Template| template.pin_order.unwrap_or(i64::MAX);
        order(a)
            .cmp(&order(b))
            .then_with(|| locale_cmp(&a.title, &b.title))
    });
    let mut others = library
        .templates
        .iter()
        .filter(|template| !template.pinned)
        .collect::<Vec<_>>();
    others.sort_by(|a, b| match sort {
        Sort::AToZ => locale_cmp(&a.title, &b.title),
        Sort::ZToA => locale_cmp(&b.title, &a.title),
    });
    let trimmed = search.trim().to_lowercase();
    // Shipping lowercases but does not trim the query for the user's templates.
    let query = search.to_lowercase();
    let matches = |template: &Template| {
        trimmed.is_empty()
            || [&template.title, &template.description, &template.category]
                .iter()
                .any(|field| field.to_lowercase().contains(&query))
            || template
                .targets
                .iter()
                .any(|target| target.to_lowercase().contains(&query))
    };
    let auto = (trimmed.is_empty() || "auto".contains(&trimmed)).then_some(Selection::Auto);
    auto.into_iter()
        .chain(
            favorites
                .into_iter()
                .chain(others)
                .filter(|template| matches(template))
                .map(|template| Selection::Mine(template.id.clone())),
        )
        .collect()
}

/// `resolveTemplateTabSelection`: a missing selection falls back to the first template by id,
/// then to Auto.
pub fn resolve(library: &Library, selected: Option<&Selection>) -> Selection {
    match selected {
        Some(Selection::Auto) => Selection::Auto,
        Some(Selection::Mine(id)) if library.templates.iter().any(|t| &t.id == id) => {
            Selection::Mine(id.clone())
        }
        _ => library
            .templates
            .first()
            .map_or(Selection::Auto, |template| {
                Selection::Mine(template.id.clone())
            }),
    }
}

pub fn copy_title(title: &str) -> String {
    match title.trim() {
        "" => "Untitled (Copy)".into(),
        title if title.ends_with("(Copy)") => title.into(),
        title => format!("{title} (Copy)"),
    }
}

/// Columns the template form writes; `None` leaves a column untouched.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    pub title: Option<String>,
    pub description: Option<String>,
    pub icon: Option<Glyph>,
    pub targets: Option<Vec<Arc<str>>>,
    pub sections: Option<Vec<Section>>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn merge(&mut self, newer: Self) {
        self.title = newer.title.or(self.title.take());
        self.description = newer.description.or(self.description.take());
        self.icon = newer.icon.or(self.icon.take());
        self.targets = newer.targets.or(self.targets.take());
        self.sections = newer.sections.or(self.sections.take());
    }

    fn columns(self) -> Vec<(&'static str, Value)> {
        let mut columns = Vec::new();
        if let Some(title) = self.title {
            columns.push(("title", json!(title)));
        }
        if let Some(description) = self.description {
            columns.push(("description", json!(description)));
        }
        if let Some(icon) = self.icon {
            columns.push(("icon_json", json!(icon.to_json())));
        }
        if let Some(targets) = self.targets {
            columns.push(("targets_json", json!(json!(targets).to_string())));
        }
        if let Some(sections) = self.sections {
            let sections = sections
                .iter()
                .map(|section| json!({"title": section.title, "description": section.description}))
                .collect::<Vec<_>>();
            columns.push(("sections_json", json!(json!(sections).to_string())));
        }
        columns
    }
}

pub fn save(runtime: &RuntimeHandle, id: Arc<str>, changes: Changes) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let columns = changes.columns();
        if columns.is_empty() {
            return Ok(());
        }
        let assignments = columns
            .iter()
            .map(|(column, _)| format!("{column}=?"))
            .collect::<Vec<_>>()
            .join(",");
        let mut params = columns
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        params.push(json!(id));
        transaction(
            &services.executor,
            vec![statement(
                format!("UPDATE templates SET {assignments},updated_at={NOW} WHERE id=?"),
                params,
                Some(1),
            )],
        )
        .await
    })
}

/// `createDefaultTemplate`: an empty "New Template" with the default icon.
pub fn create(runtime: &RuntimeHandle) -> Result<Reply<Arc<str>>> {
    runtime.submit(move |services| async move {
        let id: Arc<str> = Uuid::new_v4().to_string().into();
        transaction(
            &services.executor,
            vec![statement(
                format!(
                    "INSERT INTO templates (id,title,description,pinned,icon_json,targets_json,sections_json,created_at,updated_at)
                    VALUES (?,'New Template','',0,?,NULL,'[]',{NOW},{NOW})"
                ),
                vec![json!(id), json!(Glyph::template_default().to_json())],
                Some(1),
            )],
        )
        .await?;
        Ok(id)
    })
}

/// Copies a template under a "(Copy)" title, keeping its stored JSON as is.
pub fn duplicate(runtime: &RuntimeHandle, source: Arc<str>) -> Result<Reply<Arc<str>>> {
    runtime.submit(move |services| async move {
        let rows = services
            .executor
            .execute("SELECT title FROM templates WHERE id=?".into(), vec![json!(source)])
            .await
            .map_err(failure)?;
        let title = rows
            .first()
            .and_then(|row| row["title"].as_str())
            .ok_or(desktop_runtime::ServiceError::Conflict)?;
        let id: Arc<str> = Uuid::new_v4().to_string().into();
        transaction(
            &services.executor,
            vec![statement(
                format!(
                    "INSERT INTO templates (id,title,description,pinned,pin_order,category,icon_json,targets_json,sections_json,created_at,updated_at)
                    SELECT ?,?,description,0,NULL,category,icon_json,targets_json,sections_json,{NOW},{NOW}
                    FROM templates WHERE id=?"
                ),
                vec![json!(id), json!(copy_title(title)), json!(source)],
                Some(1),
            )],
        )
        .await?;
        Ok(id)
    })
}

/// Deletes the row and stops pointing the default selection at it.
pub fn delete(runtime: &RuntimeHandle, id: Arc<str>) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        transaction(
            &services.executor,
            vec![
                statement(
                    "DELETE FROM templates WHERE id=?".into(),
                    vec![json!(id)],
                    Some(1),
                ),
                statement(
                    "UPDATE app_settings SET value_json='\"\"',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id='selected_template_id' AND value_json=?".into(),
                    vec![json!(json!(id).to_string())],
                    None,
                ),
            ],
        )
        .await
    })
}

/// Favoriting is not a content edit, so `updated_at` stays untouched.
pub fn toggle_favorite(runtime: &RuntimeHandle, id: Arc<str>) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        transaction(
            &services.executor,
            vec![statement(
                "UPDATE templates SET pinned=NOT pinned, pin_order=CASE WHEN pinned THEN 0
                    ELSE COALESCE((SELECT MAX(pin_order) FROM templates WHERE id<>?1),0)+1 END
                    WHERE id=?1"
                    .into(),
                vec![json!(id)],
                Some(1),
            )],
        )
        .await
    })
}

/// Stores `selected_template_id`; an empty id makes Auto the default.
pub fn set_default(runtime: &RuntimeHandle, id: Arc<str>) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        transaction(
            &services.executor,
            vec![statement(
                "INSERT INTO app_settings (id,value_json,updated_at) VALUES ('selected_template_id',?,strftime('%Y-%m-%dT%H:%M:%fZ','now'))
                    ON CONFLICT(id) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at"
                    .into(),
                vec![json!(json!(id).to_string())],
                Some(1),
            )],
        )
        .await
    })
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

    fn template(id: &str, title: &str, pinned: Option<i64>) -> Value {
        json!({"kind": "template", "id": id, "title": title, "description": "",
            "pinned": i64::from(pinned.is_some()), "pin_order": pinned,
            "sections_json": "[]", "targets_json": null, "icon_json": ""})
    }

    #[test]
    fn sidebar_lists_auto_then_favorites_then_titles_and_selects_the_first_by_id() {
        let library = decode_library(&[
            template("a", "zeta", None),
            template("b", "Alpha", None),
            template("c", "Mid", Some(2)),
            template("d", "Beta", Some(1)),
            json!({"kind": "setting", "id": "selected_template_id", "title": "\"b\""}),
            json!({"kind": "setting", "id": "auto_summary_prompt", "title": "\" \""}),
        ]);
        let ids = |list: Vec<Selection>| {
            list.into_iter()
                .map(|item| match item {
                    Selection::Auto => "auto".to_owned(),
                    Selection::Mine(id) => id.to_string(),
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ids(visible(&library, "", Sort::AToZ)),
            ["auto", "d", "c", "b", "a"]
        );
        assert_eq!(
            ids(visible(&library, "", Sort::ZToA)),
            ["auto", "d", "c", "a", "b"]
        );
        assert_eq!(ids(visible(&library, "ALP", Sort::AToZ)), ["b"]);
        assert_eq!(ids(visible(&library, "au", Sort::AToZ)), ["auto"]);
        assert_eq!(library.default_id.as_ref(), "b");
        assert!(!library.auto_customized);
        assert_eq!(resolve(&library, None), Selection::Mine("a".into()));
        assert_eq!(
            resolve(&library, Some(&Selection::Mine("gone".into()))),
            Selection::Mine("a".into())
        );
        assert_eq!(resolve(&Library::default(), None), Selection::Auto);
    }

    #[test]
    fn copies_are_titled_once() {
        assert_eq!(copy_title(" Sync "), "Sync (Copy)");
        assert_eq!(copy_title("Sync (Copy)"), "Sync (Copy)");
        assert_eq!(copy_title(""), "Untitled (Copy)");
    }
}
