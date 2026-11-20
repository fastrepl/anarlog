//! Shipping's "auto-add future meetings" rules (`calendar/series-folders.ts`): notes of a
//! recurring meeting are filed into a folder automatically.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anlg_db_execute::TransactionStatement;
use desktop_runtime::{Reply, Result, RuntimeHandle};
use serde_json::{Value, json};

use super::mutations::failure;

pub const RULES_ID: &str = "auto_add_series_folders";
pub const DECLINED_ID: &str = "auto_add_series_folders_declined";
pub const QUERY: &str = "SELECT id, value_json FROM app_settings WHERE id IN ('auto_add_series_folders','auto_add_series_folders_declined')";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeriesRules {
    /// Folder for each recurring series.
    pub folders: HashMap<Arc<str>, Arc<str>>,
    /// Series whose auto-add prompt was answered "No".
    pub declined: HashSet<Arc<str>>,
}

impl SeriesRules {
    pub fn from_rows(rows: &[Value]) -> Self {
        let value = |id: &str| {
            rows.iter()
                .find(|row| row["id"] == id)
                .and_then(|row| row["value_json"].as_str())
        };
        Self {
            folders: parse_rules(value(RULES_ID))
                .into_iter()
                .map(|(series, folder)| (series.into(), folder.into()))
                .collect(),
            declined: parse_declined(value(DECLINED_ID))
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }

    pub fn folder(&self, series: &str) -> Option<&Arc<str>> {
        self.folders.get(series)
    }
}

/// Shipping's `normalizeFolderPath`; invalid and empty paths are `None`.
fn normalize(path: &str) -> Option<String> {
    Some(super::timeline::folder_label(path)).filter(|path| !path.is_empty())
}

/// Valid `{series_id, folder_path}` entries with normalized paths, in stored order.
pub fn parse_rules(value: Option<&str>) -> Vec<(String, String)> {
    let Some(Ok(Value::Array(entries))) = value.map(serde_json::from_str::<Value>) else {
        return vec![];
    };
    entries
        .iter()
        .filter_map(|entry| {
            let series = entry["series_id"].as_str().filter(|id| !id.is_empty())?;
            Some((
                series.to_owned(),
                normalize(entry["folder_path"].as_str()?)?,
            ))
        })
        .collect()
}

pub fn parse_declined(value: Option<&str>) -> Vec<String> {
    let Some(Ok(Value::Array(entries))) = value.map(serde_json::from_str::<Value>) else {
        return vec![];
    };
    entries
        .iter()
        .filter_map(|entry| entry.as_str().filter(|id| !id.is_empty()))
        .map(str::to_owned)
        .collect()
}

#[derive(Clone, Debug)]
pub enum RuleChange {
    /// Files future notes of the series into this folder and forgets a declined prompt.
    Set(Arc<str>),
    Decline,
    Clear,
}

/// The settings writes for a change, given the stored values; empty when nothing changes.
pub fn plan(
    rules: Option<&str>,
    declined: Option<&str>,
    series: &str,
    change: &RuleChange,
) -> Vec<(&'static str, String)> {
    let mut rules = parse_rules(rules);
    let mut declined = parse_declined(declined);
    let mut writes = vec![];
    match change {
        RuleChange::Set(folder) => {
            let Some(folder) = normalize(folder) else {
                return writes;
            };
            rules.retain(|(id, _)| id != series);
            rules.push((series.to_owned(), folder));
            writes.push((RULES_ID, encode_rules(&rules)));
            if declined.iter().any(|id| id == series) {
                declined.retain(|id| id != series);
                writes.push((DECLINED_ID, json!(declined).to_string()));
            }
        }
        RuleChange::Decline => {
            if !declined.iter().any(|id| id == series) {
                declined.push(series.to_owned());
                writes.push((DECLINED_ID, json!(declined).to_string()));
            }
        }
        RuleChange::Clear => {
            let before = rules.len();
            rules.retain(|(id, _)| id != series);
            if rules.len() != before {
                writes.push((RULES_ID, encode_rules(&rules)));
            }
        }
    }
    writes
}

/// Serialized like shipping's `JSON.stringify` of `{series_id, folder_path}` entries.
fn encode_rules(rules: &[(String, String)]) -> String {
    #[derive(serde::Serialize)]
    struct Rule<'a> {
        series_id: &'a str,
        folder_path: &'a str,
    }
    serde_json::to_string(
        &rules
            .iter()
            .map(|(series_id, folder_path)| Rule {
                series_id,
                folder_path,
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_default()
}

/// Shipping's `remapSeriesFolderRules` and `clearSeriesFolderRulesForFolder`: rules filing into
/// `from` or one of its subfolders follow a rename to `to`, or are dropped when it is deleted
/// (`None`). The new stored rules, when any rule changed.
pub fn folder_moved(rules: Option<&str>, from: &str, to: Option<&str>) -> Option<String> {
    let rules = parse_rules(rules);
    let inside = |path: &str| {
        path.strip_prefix(from)
            .filter(|rest| rest.is_empty() || rest.starts_with('/'))
            .map(str::to_owned)
    };
    if !rules.iter().any(|(_, path)| inside(path).is_some()) {
        return None;
    }
    let moved = rules
        .into_iter()
        .filter_map(|(series, path)| match (inside(&path), to) {
            (None, _) => Some((series, path)),
            (Some(rest), Some(to)) => Some((series, format!("{to}{rest}"))),
            (Some(_), None) => None,
        })
        .collect::<Vec<_>>();
    Some(encode_rules(&moved))
}

pub fn update(runtime: &RuntimeHandle, series: Arc<str>, change: RuleChange) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let rows = services
            .executor
            .execute(QUERY.into(), vec![])
            .await
            .map_err(failure)?;
        let value = |id: &str| {
            rows.iter()
                .find(|row| row["id"] == id)
                .and_then(|row| row["value_json"].as_str())
        };
        let writes = plan(value(RULES_ID), value(DECLINED_ID), &series, &change);
        if writes.is_empty() {
            return Ok(());
        }
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        services
            .executor
            .execute_transaction(
                writes
                    .into_iter()
                    .map(|(id, value)| TransactionStatement {
                        sql: "INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, ?)
                            ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at"
                            .into(),
                        params: vec![json!(id), json!(value), json!(now)],
                        expected_rows_affected: None,
                    })
                    .collect(),
            )
            .await
            .map_err(failure)?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rule_changes_round_trip_through_app_settings() {
        let root = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: root.path().join("rules.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        let read = || {
            runtime
                .submit(|services| async move {
                    let rows = services
                        .executor
                        .execute(QUERY.into(), vec![])
                        .await
                        .map_err(failure)?;
                    Ok(SeriesRules::from_rows(&rows))
                })
                .unwrap()
                .receive()
        };
        update(&runtime, "s".into(), RuleChange::Decline)
            .unwrap()
            .receive()
            .await
            .unwrap();
        update(&runtime, "s".into(), RuleChange::Set("Weekly".into()))
            .unwrap()
            .receive()
            .await
            .unwrap();
        let rules = read().await.unwrap();
        assert_eq!(rules.folder("s").map(|folder| &**folder), Some("Weekly"));
        assert!(rules.declined.is_empty());
        update(&runtime, "s".into(), RuleChange::Clear)
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(read().await.unwrap().folder("s"), None);
        runtime.shutdown().await.unwrap();
    }

    #[test]
    fn stored_rules_are_validated_and_normalized_like_shipping() {
        let rules = parse_rules(Some(
            r#"[{"series_id":"a","folder_path":" Work/Weekly/ "},{"series_id":"","folder_path":"X"},
                {"series_id":"b","folder_path":"../x"},{"series_id":"c"},"junk"]"#,
        ));
        assert_eq!(rules, [("a".to_owned(), "Work/Weekly".to_owned())]);
        assert!(parse_rules(Some("not json")).is_empty());
        assert_eq!(parse_declined(Some(r#"["a","",3,"b"]"#)), ["a", "b"]);
    }

    #[test]
    fn choosing_a_folder_replaces_the_rule_and_forgets_a_decline() {
        let rules =
            r#"[{"series_id":"a","folder_path":"Old"},{"series_id":"b","folder_path":"B"}]"#;
        let writes = plan(
            Some(rules),
            Some(r#"["a","c"]"#),
            "a",
            &RuleChange::Set("New".into()),
        );
        assert_eq!(
            writes,
            [
                (
                    RULES_ID,
                    r#"[{"series_id":"b","folder_path":"B"},{"series_id":"a","folder_path":"New"}]"#
                        .to_owned()
                ),
                (DECLINED_ID, r#"["c"]"#.to_owned()),
            ]
        );
        assert!(plan(Some(rules), None, "z", &RuleChange::Clear).is_empty());
        assert!(plan(None, Some(r#"["a"]"#), "a", &RuleChange::Decline).is_empty());
        assert_eq!(
            plan(None, None, "a", &RuleChange::Decline),
            [(DECLINED_ID, r#"["a"]"#.to_owned())]
        );
    }
}
