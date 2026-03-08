use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use hypr_version::Version;
use serde_json::Value;

use super::utils::{FileOp, apply_ops};
use super::version_from_name;
use crate::Result;

pub struct Migrate;

impl super::Migration for Migrate {
    fn introduced_in(&self) -> &'static Version {
        version_from_name!()
    }

    fn run<'a>(&self, base_dir: &'a Path) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(run_inner(base_dir))
    }
}

// The tracking_id format for recurring events changed from just the EKEvent eventIdentifier
// to "eventIdentifier:YYYY-MM-DD" (see apple-calendar/src/convert.rs). This migration updates
// existing ignored event entries and session embedded events to match the new format.
//
// We use system timezone as a best-effort approximation since we don't have access to EventKit's
// per-event timezone during migration.
async fn run_inner(base_dir: &Path) -> Result<()> {
    let mut ops = Vec::new();

    migrate_ignored_events(base_dir, &mut ops);
    migrate_session_metas(base_dir, &mut ops);

    apply_ops(ops)?;

    // events.json will be repopulated by the sync service on next app startup
    let _ = std::fs::remove_file(base_dir.join("events.json"));

    Ok(())
}

fn day_from_started_at_local(started_at: &str) -> String {
    let dt = match chrono::DateTime::parse_from_rfc3339(started_at) {
        Ok(dt) => dt,
        Err(_) => {
            if started_at.len() >= 10 {
                return started_at[..10].to_string();
            }
            return "1970-01-01".to_string();
        }
    };

    dt.with_timezone(&chrono::Local)
        .format("%Y-%m-%d")
        .to_string()
}

// store.json -> TinybaseValues -> ignored_events

fn migrate_ignored_events(base_dir: &Path, ops: &mut Vec<FileOp>) {
    let store_path = base_dir.join("store.json");
    if !store_path.exists() {
        return;
    }

    let content = match std::fs::read_to_string(&store_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut store: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return,
    };

    let desktop_str = match store.get("desktop").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return,
    };
    let mut desktop: Value = match serde_json::from_str(&desktop_str) {
        Ok(v) => v,
        Err(_) => return,
    };

    let tb_str = match desktop.get("TinybaseValues").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return,
    };
    let mut tb_values: Value = match serde_json::from_str(&tb_str) {
        Ok(v) => v,
        Err(_) => return,
    };

    let tb_obj = match tb_values.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    if !convert_ignored_events(tb_obj) {
        return;
    }

    let new_tb_str = match serde_json::to_string(&tb_values) {
        Ok(s) => s,
        Err(_) => return,
    };
    desktop["TinybaseValues"] = Value::String(new_tb_str);

    let new_desktop_str = match serde_json::to_string(&desktop) {
        Ok(s) => s,
        Err(_) => return,
    };
    store["desktop"] = Value::String(new_desktop_str);

    if let Ok(new_content) = serde_json::to_string_pretty(&store) {
        ops.push(FileOp::Write {
            path: store_path,
            content: new_content,
            force: true,
        });
    }
}

// Old format: {tracking_id: "<eventIdentifier>", is_recurrent: true, day: "YYYY-MM-DD", last_seen: "..."}
// New format: {tracking_id: "<eventIdentifier>:<day>", last_seen: "..."}
fn convert_ignored_events(tb_obj: &mut serde_json::Map<String, Value>) -> bool {
    let raw = match tb_obj.get("ignored_events").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return false,
    };

    let entries: Vec<Value> = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return false,
    };

    if entries.is_empty() {
        return false;
    }

    let needs_migration = entries.iter().any(|e| e.get("is_recurrent").is_some());
    if !needs_migration {
        return false;
    }

    let converted: Vec<Value> = entries
        .into_iter()
        .map(|entry| {
            let tracking_id = entry
                .get("tracking_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let is_recurrent = entry
                .get("is_recurrent")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let last_seen = entry
                .get("last_seen")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let new_tracking_id = if is_recurrent {
                let day = entry
                    .get("day")
                    .and_then(|v| v.as_str())
                    .unwrap_or("1970-01-01");
                format!("{}:{}", tracking_id, day)
            } else {
                tracking_id
            };

            serde_json::json!({
                "tracking_id": new_tracking_id,
                "last_seen": last_seen,
            })
        })
        .collect();

    let serialized = serde_json::to_string(&converted).unwrap_or_default();
    tb_obj.insert("ignored_events".to_string(), Value::String(serialized));
    true
}

// sessions/*/_meta.json

fn collect_meta_files(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return result,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let meta = path.join("_meta.json");
            if meta.exists() {
                result.push(meta);
            }
            result.extend(collect_meta_files(&path));
        }
    }
    result
}

fn migrate_session_metas(base_dir: &Path, ops: &mut Vec<FileOp>) {
    let sessions_dir = base_dir.join("sessions");

    for meta_path in collect_meta_files(&sessions_dir) {
        if let Ok(Some(op)) = try_migrate_meta(&meta_path) {
            ops.push(op);
        }
    }
}

// For recurring events, synthesize the new tracking_id from started_at since we don't
// have access to occurrence_date outside of EventKit.
fn try_migrate_meta(meta_path: &Path) -> Result<Option<FileOp>> {
    let content = std::fs::read_to_string(meta_path)?;
    let mut meta: Value = serde_json::from_str(&content)?;

    let obj = match meta.as_object_mut() {
        Some(o) => o,
        None => return Ok(None),
    };

    let event = match obj.get_mut("event").and_then(|v| v.as_object_mut()) {
        Some(e) => e,
        None => return Ok(None),
    };

    let has_recurrence = event
        .get("has_recurrence_rules")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !has_recurrence {
        return Ok(None);
    }

    let tracking_id = match event.get("tracking_id").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => return Ok(None),
    };

    let started_at = event
        .get("started_at")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let day = day_from_started_at_local(started_at);
    let new_tracking_id = format!("{}:{}", tracking_id, day);

    event.insert("tracking_id".to_string(), Value::String(new_tracking_id));

    let new_content = serde_json::to_string_pretty(&meta)?;
    Ok(Some(FileOp::Write {
        path: meta_path.to_path_buf(),
        content: new_content,
        force: true,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_json(path: &Path, value: &Value) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
    }

    fn read_json(path: &Path) -> Value {
        let content = std::fs::read_to_string(path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    fn make_store(tb_values: &Value) -> Value {
        let tb_str = serde_json::to_string(tb_values).unwrap();
        let desktop = serde_json::json!({"TinybaseValues": tb_str});
        let desktop_str = serde_json::to_string(&desktop).unwrap();
        serde_json::json!({"desktop": desktop_str})
    }

    fn read_tb_values(store: &Value) -> Value {
        let desktop_str = store["desktop"].as_str().unwrap();
        let desktop: Value = serde_json::from_str(desktop_str).unwrap();
        let tb_str = desktop["TinybaseValues"].as_str().unwrap();
        serde_json::from_str(tb_str).unwrap()
    }

    // ignored_events tests

    #[test]
    fn test_convert_recurrent_ignored_event() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let existing = serde_json::json!([
            {"tracking_id": "ABC:DEF", "is_recurrent": true, "day": "2024-03-15", "last_seen": "2024-03-20T00:00:00Z"},
        ]);
        let tb_values = serde_json::json!({
            "ignored_events": serde_json::to_string(&existing).unwrap(),
        });
        write_json(&base.join("store.json"), &make_store(&tb_values));

        let mut ops = Vec::new();
        migrate_ignored_events(base, &mut ops);
        apply_ops(ops).unwrap();

        let store = read_json(&base.join("store.json"));
        let tb = read_tb_values(&store);
        let parsed: Vec<Value> =
            serde_json::from_str(tb["ignored_events"].as_str().unwrap()).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["tracking_id"], "ABC:DEF:2024-03-15");
        assert_eq!(parsed[0]["last_seen"], "2024-03-20T00:00:00Z");
        assert!(parsed[0].get("is_recurrent").is_none());
        assert!(parsed[0].get("day").is_none());
    }

    #[test]
    fn test_convert_non_recurrent_ignored_event() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let existing = serde_json::json!([
            {"tracking_id": "ABC:DEF", "is_recurrent": false, "last_seen": "2024-03-20T00:00:00Z"},
        ]);
        let tb_values = serde_json::json!({
            "ignored_events": serde_json::to_string(&existing).unwrap(),
        });
        write_json(&base.join("store.json"), &make_store(&tb_values));

        let mut ops = Vec::new();
        migrate_ignored_events(base, &mut ops);
        apply_ops(ops).unwrap();

        let store = read_json(&base.join("store.json"));
        let tb = read_tb_values(&store);
        let parsed: Vec<Value> =
            serde_json::from_str(tb["ignored_events"].as_str().unwrap()).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["tracking_id"], "ABC:DEF");
        assert_eq!(parsed[0]["last_seen"], "2024-03-20T00:00:00Z");
        assert!(parsed[0].get("is_recurrent").is_none());
    }

    #[test]
    fn test_convert_mixed_ignored_events() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let existing = serde_json::json!([
            {"tracking_id": "id-1", "is_recurrent": true, "day": "2024-01-10", "last_seen": "t1"},
            {"tracking_id": "id-2", "is_recurrent": false, "last_seen": "t2"},
            {"tracking_id": "id-3", "is_recurrent": true, "day": "2024-06-20", "last_seen": "t3"},
        ]);
        let tb_values = serde_json::json!({
            "ignored_events": serde_json::to_string(&existing).unwrap(),
        });
        write_json(&base.join("store.json"), &make_store(&tb_values));

        let mut ops = Vec::new();
        migrate_ignored_events(base, &mut ops);
        apply_ops(ops).unwrap();

        let store = read_json(&base.join("store.json"));
        let tb = read_tb_values(&store);
        let parsed: Vec<Value> =
            serde_json::from_str(tb["ignored_events"].as_str().unwrap()).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0]["tracking_id"], "id-1:2024-01-10");
        assert_eq!(parsed[1]["tracking_id"], "id-2");
        assert_eq!(parsed[2]["tracking_id"], "id-3:2024-06-20");
    }

    #[test]
    fn test_already_migrated_ignored_events_skipped() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let existing = serde_json::json!([
            {"tracking_id": "id-1:2024-01-10", "last_seen": "t1"},
        ]);
        let tb_values = serde_json::json!({
            "ignored_events": serde_json::to_string(&existing).unwrap(),
        });
        write_json(&base.join("store.json"), &make_store(&tb_values));

        let mut ops = Vec::new();
        migrate_ignored_events(base, &mut ops);
        assert!(ops.is_empty());
    }

    #[test]
    fn test_no_ignored_events() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let tb_values = serde_json::json!({"user_id": "u1"});
        write_json(&base.join("store.json"), &make_store(&tb_values));

        let mut ops = Vec::new();
        migrate_ignored_events(base, &mut ops);
        assert!(ops.is_empty());
    }

    #[test]
    fn test_no_store_json() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let mut ops = Vec::new();
        migrate_ignored_events(base, &mut ops);
        assert!(ops.is_empty());
    }

    // _meta.json tests

    #[test]
    fn test_migrate_recurring_session_meta() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let meta = serde_json::json!({
            "id": "s1",
            "user_id": "u1",
            "created_at": "2024-01-01T00:00:00Z",
            "title": "Weekly Standup",
            "event": {
                "tracking_id": "ABC:DEF",
                "calendar_id": "cal-1",
                "title": "Weekly Standup",
                "started_at": "2024-03-15T14:00:00+00:00",
                "ended_at": "2024-03-15T15:00:00+00:00",
                "is_all_day": false,
                "has_recurrence_rules": true,
                "recurrence_series_id": "series-1"
            },
            "participants": [],
        });
        let meta_path = base.join("sessions/s1/_meta.json");
        write_json(&meta_path, &meta);

        let mut ops = Vec::new();
        migrate_session_metas(base, &mut ops);
        apply_ops(ops).unwrap();

        let result = read_json(&meta_path);
        let event = &result["event"];
        let tid = event["tracking_id"].as_str().unwrap();
        assert!(tid.starts_with("ABC:DEF:"));
        assert!(
            tid.ends_with("2024-03-15")
                || tid.ends_with("2024-03-14")
                || tid.ends_with("2024-03-16"),
            "day should be near 2024-03-15 depending on system timezone, got: {tid}"
        );
    }

    #[test]
    fn test_non_recurring_session_meta_unchanged() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let meta = serde_json::json!({
            "id": "s1",
            "user_id": "u1",
            "created_at": "2024-01-01T00:00:00Z",
            "title": "One-off Meeting",
            "event": {
                "tracking_id": "ABC:DEF",
                "calendar_id": "cal-1",
                "title": "One-off Meeting",
                "started_at": "2024-03-15T14:00:00+00:00",
                "ended_at": "2024-03-15T15:00:00+00:00",
                "is_all_day": false,
                "has_recurrence_rules": false,
            },
            "participants": [],
        });
        let meta_path = base.join("sessions/s1/_meta.json");
        write_json(&meta_path, &meta);

        let mut ops = Vec::new();
        migrate_session_metas(base, &mut ops);
        assert!(ops.is_empty());
    }

    #[test]
    fn test_session_meta_without_event_unchanged() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let meta = serde_json::json!({
            "id": "s1",
            "user_id": "u1",
            "created_at": "2024-01-01T00:00:00Z",
            "title": "No Event",
            "participants": [],
        });
        let meta_path = base.join("sessions/s1/_meta.json");
        write_json(&meta_path, &meta);

        let mut ops = Vec::new();
        migrate_session_metas(base, &mut ops);
        assert!(ops.is_empty());
    }

    #[test]
    fn test_session_meta_in_folder() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let meta = serde_json::json!({
            "id": "s1",
            "user_id": "u1",
            "created_at": "2024-01-01T00:00:00Z",
            "title": "Foldered Session",
            "event": {
                "tracking_id": "XYZ",
                "calendar_id": "cal-1",
                "title": "Foldered",
                "started_at": "2024-06-10T08:00:00+00:00",
                "ended_at": "2024-06-10T09:00:00+00:00",
                "is_all_day": false,
                "has_recurrence_rules": true,
            },
            "participants": [],
        });
        let meta_path = base.join("sessions/my-folder/s1/_meta.json");
        write_json(&meta_path, &meta);

        let mut ops = Vec::new();
        migrate_session_metas(base, &mut ops);
        apply_ops(ops).unwrap();

        let result = read_json(&meta_path);
        let tid = result["event"]["tracking_id"].as_str().unwrap();
        assert!(
            tid.starts_with("XYZ:2024-06-"),
            "should have day suffix, got: {tid}"
        );
    }

    // events.json deletion

    #[test]
    fn test_events_json_deleted() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let events = serde_json::json!({"e1": {"tracking_id_event": "id-1"}});
        write_json(&base.join("events.json"), &events);

        assert!(base.join("events.json").exists());
        let _ = std::fs::remove_file(base.join("events.json"));
        assert!(!base.join("events.json").exists());
    }

    #[test]
    fn test_events_json_missing_is_ok() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let _ = std::fs::remove_file(base.join("events.json"));
        // No panic
    }

    // day_from_started_at_local tests

    #[test]
    fn test_day_from_started_at_local_valid() {
        let day = day_from_started_at_local("2024-06-15T12:00:00+00:00");
        assert!(day.starts_with("2024-06-1"), "got: {day}");
    }

    #[test]
    fn test_day_from_started_at_local_invalid() {
        assert_eq!(day_from_started_at_local(""), "1970-01-01");
    }

    #[test]
    fn test_day_from_started_at_local_partial() {
        assert_eq!(day_from_started_at_local("2024-03-15 bad"), "2024-03-15");
    }

    // full integration

    #[tokio::test]
    async fn test_full_migration() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        // Set up events.json
        let events = serde_json::json!({"e1": {"tracking_id_event": "id-1"}});
        write_json(&base.join("events.json"), &events);

        // Set up store.json with ignored events
        let existing = serde_json::json!([
            {"tracking_id": "id-r", "is_recurrent": true, "day": "2024-02-20", "last_seen": "t1"},
            {"tracking_id": "id-n", "is_recurrent": false, "last_seen": "t2"},
        ]);
        let tb_values = serde_json::json!({
            "ignored_events": serde_json::to_string(&existing).unwrap(),
        });
        write_json(&base.join("store.json"), &make_store(&tb_values));

        // Set up session _meta.json with recurring event
        let meta = serde_json::json!({
            "id": "s1",
            "user_id": "u1",
            "created_at": "2024-01-01T00:00:00Z",
            "title": "Recurring",
            "event": {
                "tracking_id": "ev-id",
                "calendar_id": "cal-1",
                "title": "Recurring",
                "started_at": "2024-05-10T10:00:00+00:00",
                "ended_at": "2024-05-10T11:00:00+00:00",
                "is_all_day": false,
                "has_recurrence_rules": true,
            },
            "participants": [],
        });
        write_json(&base.join("sessions/s1/_meta.json"), &meta);

        run_inner(base).await.unwrap();

        // events.json deleted
        assert!(!base.join("events.json").exists());

        // store.json ignored_events converted
        let store = read_json(&base.join("store.json"));
        let tb = read_tb_values(&store);
        let parsed: Vec<Value> =
            serde_json::from_str(tb["ignored_events"].as_str().unwrap()).unwrap();
        assert_eq!(parsed[0]["tracking_id"], "id-r:2024-02-20");
        assert!(parsed[0].get("is_recurrent").is_none());
        assert_eq!(parsed[1]["tracking_id"], "id-n");

        // session _meta.json tracking_id updated
        let result = read_json(&base.join("sessions/s1/_meta.json"));
        let tid = result["event"]["tracking_id"].as_str().unwrap();
        assert!(
            tid.starts_with("ev-id:2024-05-"),
            "should have day suffix, got: {tid}"
        );
    }
}
