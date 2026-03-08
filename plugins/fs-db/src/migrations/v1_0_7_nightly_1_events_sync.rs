use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::Path;
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

async fn run_inner(base_dir: &Path) -> Result<()> {
    let events = load_events(base_dir);
    let timezone = load_timezone(base_dir);
    let mut ops = Vec::new();

    migrate_session_metas(base_dir, &events, &mut ops);
    let ignored_series = load_ignored_recurring_series_ids(base_dir);
    let ignored = collect_ignored_events(&events, &ignored_series, timezone.as_deref());
    clean_events_json(base_dir, &mut ops);
    migrate_store_values(base_dir, &ignored, &mut ops);

    apply_ops(ops)?;
    Ok(())
}

// events.json

fn load_events(base_dir: &Path) -> HashMap<String, Value> {
    let events_path = base_dir.join("events.json");
    let content = match std::fs::read_to_string(&events_path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn load_timezone(base_dir: &Path) -> Option<String> {
    let settings_path = base_dir.join("settings.json");
    let content = std::fs::read_to_string(&settings_path).ok()?;
    let settings: Value = serde_json::from_str(&content).ok()?;
    settings
        .get("general")?
        .get("timezone")?
        .as_str()
        .map(|s| s.to_string())
}

fn day_from_started_at(started_at: &str, timezone: Option<&str>) -> String {
    let dt = match chrono::DateTime::parse_from_rfc3339(started_at) {
        Ok(dt) => dt,
        Err(_) => {
            if started_at.len() >= 10 {
                return started_at[..10].to_string();
            }
            return "1970-01-01".to_string();
        }
    };

    if let Some(tz_str) = timezone
        && let Ok(tz) = tz_str.parse::<chrono_tz::Tz>()
    {
        return dt.with_timezone(&tz).format("%Y-%m-%d").to_string();
    }

    dt.format("%Y-%m-%d").to_string()
}

fn load_ignored_recurring_series_ids(base_dir: &Path) -> HashSet<String> {
    let store_path = base_dir.join("store.json");
    let content = match std::fs::read_to_string(&store_path) {
        Ok(c) => c,
        Err(_) => return HashSet::new(),
    };
    let store: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return HashSet::new(),
    };
    let desktop_str = match store.get("desktop").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return HashSet::new(),
    };
    let desktop: Value = match serde_json::from_str(desktop_str) {
        Ok(v) => v,
        Err(_) => return HashSet::new(),
    };
    let tb_str = match desktop.get("TinybaseValues").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return HashSet::new(),
    };
    let tb: Value = match serde_json::from_str(tb_str) {
        Ok(v) => v,
        Err(_) => return HashSet::new(),
    };
    let raw = match tb.get("ignored_recurring_series").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return HashSet::new(),
    };
    let arr: Vec<Value> = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return HashSet::new(),
    };

    arr.iter()
        .filter_map(|v| {
            v.as_str().map(|s| s.to_string()).or_else(|| {
                v.get("id")
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
            })
        })
        .collect()
}

fn collect_ignored_events(
    events: &HashMap<String, Value>,
    ignored_series: &HashSet<String>,
    timezone: Option<&str>,
) -> Vec<Value> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut result = Vec::new();

    for event in events.values() {
        let ignored = event
            .get("ignored")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !ignored {
            continue;
        }

        if let Some(series_id) = event.get("recurrence_series_id").and_then(|v| v.as_str())
            && ignored_series.contains(series_id)
        {
            continue;
        }

        let tracking_id = event
            .get("tracking_id_event")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let has_recurrence_rules = event
            .get("has_recurrence_rules")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if has_recurrence_rules {
            let started_at = event
                .get("started_at")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let day = day_from_started_at(started_at, timezone);
            result.push(serde_json::json!({
                "tracking_id": tracking_id,
                "is_recurrent": true,
                "day": day,
                "last_seen": now,
            }));
        } else {
            result.push(serde_json::json!({
                "tracking_id": tracking_id,
                "is_recurrent": false,
                "last_seen": now,
            }));
        }
    }

    result
}

fn clean_events_json(base_dir: &Path, ops: &mut Vec<FileOp>) {
    let events_path = base_dir.join("events.json");
    if !events_path.exists() {
        return;
    }

    let content = match std::fs::read_to_string(&events_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut data: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return,
    };

    let map = match data.as_object_mut() {
        Some(m) => m,
        None => return,
    };

    let mut changed = false;
    for event in map.values_mut() {
        if let Some(obj) = event.as_object_mut()
            && obj.remove("ignored").is_some()
        {
            changed = true;
        }
    }

    if !changed {
        return;
    }

    if let Ok(new_content) = serde_json::to_string_pretty(&data) {
        ops.push(FileOp::Write {
            path: events_path,
            content: new_content,
            force: true,
        });
    }
}

// sessions/<session_id>/_meta.json

fn build_session_event(event: &Value) -> Value {
    let mut obj = serde_json::Map::new();

    obj.insert(
        "tracking_id".to_string(),
        Value::String(
            event
                .get("tracking_id_event")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        ),
    );

    for key in ["calendar_id", "title", "started_at", "ended_at"] {
        let v = event.get(key).and_then(|v| v.as_str()).unwrap_or("");
        obj.insert(key.to_string(), Value::String(v.to_string()));
    }

    for key in ["is_all_day", "has_recurrence_rules"] {
        let v = event.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
        obj.insert(key.to_string(), Value::Bool(v));
    }

    for key in [
        "location",
        "meeting_link",
        "description",
        "recurrence_series_id",
    ] {
        if let Some(v) = event.get(key).and_then(|v| v.as_str()) {
            obj.insert(key.to_string(), Value::String(v.to_string()));
        }
    }

    Value::Object(obj)
}

fn migrate_session_metas(base_dir: &Path, events: &HashMap<String, Value>, ops: &mut Vec<FileOp>) {
    let sessions_dir = base_dir.join("sessions");
    let entries = match std::fs::read_dir(&sessions_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let meta_path = entry.path().join("_meta.json");
        if !meta_path.exists() {
            continue;
        }
        if let Ok(Some(op)) = try_migrate_meta(&meta_path, events) {
            ops.push(op)
        }
    }
}

fn try_migrate_meta(meta_path: &Path, events: &HashMap<String, Value>) -> Result<Option<FileOp>> {
    let content = std::fs::read_to_string(meta_path)?;
    let mut meta: Value = serde_json::from_str(&content)?;

    let obj = match meta.as_object_mut() {
        Some(o) => o,
        None => return Ok(None),
    };

    if obj.contains_key("event") {
        return Ok(None);
    }

    let event_id = match obj.get("event_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Ok(None),
    };

    if let Some(event) = events.get(&event_id) {
        obj.insert("event".to_string(), build_session_event(event));
    }

    obj.remove("event_id");

    let new_content = serde_json::to_string_pretty(&meta)?;
    Ok(Some(FileOp::Write {
        path: meta_path.to_path_buf(),
        content: new_content,
        force: true,
    }))
}

// store.json (TinyBase values)
//
// Format: {"desktop": "<json string>"}
// Desktop scope: {"TinybaseValues": "<json string>", ...}
// TinybaseValues: {"ignored_recurring_series": "<json string>", "ignored_events": "<json string>", ...}

fn migrate_store_values(base_dir: &Path, ignored_events: &[Value], ops: &mut Vec<FileOp>) {
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

    let mut changed = false;

    if !ignored_events.is_empty() {
        changed |= merge_ignored_events(tb_obj, ignored_events);
    }

    changed |= migrate_ignored_recurring_series(tb_obj);

    if !changed {
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

fn merge_ignored_events(
    tb_obj: &mut serde_json::Map<String, Value>,
    new_entries: &[Value],
) -> bool {
    let mut existing: Vec<Value> = tb_obj
        .get("ignored_events")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let existing_keys: std::collections::HashSet<String> = existing
        .iter()
        .filter_map(|e| {
            let tid = e.get("tracking_id")?.as_str()?;
            let is_recurrent = e
                .get("is_recurrent")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if is_recurrent {
                let day = e.get("day")?.as_str()?;
                Some(format!("{tid}:{day}"))
            } else {
                Some(tid.to_string())
            }
        })
        .collect();

    let mut added = false;
    for entry in new_entries {
        let tid = match entry.get("tracking_id").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => continue,
        };
        let is_recurrent = entry
            .get("is_recurrent")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let key = if is_recurrent {
            match entry.get("day").and_then(|v| v.as_str()) {
                Some(day) => format!("{tid}:{day}"),
                None => continue,
            }
        } else {
            tid.to_string()
        };
        if !existing_keys.contains(&key) {
            existing.push(entry.clone());
            added = true;
        }
    }

    if !added {
        return false;
    }

    let serialized = serde_json::to_string(&existing).unwrap_or_default();
    tb_obj.insert("ignored_events".to_string(), Value::String(serialized));
    true
}

fn migrate_ignored_recurring_series(tb_obj: &mut serde_json::Map<String, Value>) -> bool {
    let raw = match tb_obj
        .get("ignored_recurring_series")
        .and_then(|v| v.as_str())
    {
        Some(s) => s.to_string(),
        None => return false,
    };

    let arr: Vec<Value> = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return false,
    };

    if arr.is_empty() {
        return false;
    }

    let already_migrated = arr.iter().all(|v| v.is_object());
    if already_migrated {
        return false;
    }

    let now = chrono::Utc::now().to_rfc3339();
    let migrated: Vec<Value> = arr
        .iter()
        .filter_map(|v| v.as_str())
        .map(|id| serde_json::json!({"id": id, "last_seen": now}))
        .collect();

    let serialized = serde_json::to_string(&migrated).unwrap_or_default();
    tb_obj.insert(
        "ignored_recurring_series".to_string(),
        Value::String(serialized),
    );
    true
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

    fn make_event(tracking_id: &str, title: &str) -> Value {
        serde_json::json!({
            "tracking_id_event": tracking_id,
            "calendar_id": "cal-1",
            "title": title,
            "started_at": "2024-01-15T10:00:00Z",
            "ended_at": "2024-01-15T11:00:00Z",
            "is_all_day": false,
            "has_recurrence_rules": false,
        })
    }

    fn make_meta(event_id: &str) -> Value {
        serde_json::json!({
            "id": "session-1",
            "user_id": "user-1",
            "created_at": "2024-01-01T00:00:00Z",
            "title": "Test Session",
            "event_id": event_id,
            "participants": [],
        })
    }

    /// Build a store.json with the triple-nested format:
    /// store.json -> {"desktop": "<desktop_json>"}
    /// desktop_json -> {"TinybaseValues": "<values_json>"}
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

    // _meta.json tests

    #[test]
    fn test_migrate_meta_converts_event_id_to_event() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let events = serde_json::json!({"row-1": make_event("track-1", "Meeting")});
        write_json(&base.join("events.json"), &events);

        let meta = make_meta("row-1");
        let meta_path = base.join("sessions/session-1/_meta.json");
        write_json(&meta_path, &meta);

        let events_map = load_events(base);
        let mut ops = Vec::new();
        migrate_session_metas(base, &events_map, &mut ops);
        apply_ops(ops).unwrap();

        let result = read_json(&meta_path);
        assert!(result.get("event_id").is_none());
        let event = result.get("event").unwrap();
        assert_eq!(event["tracking_id"], "track-1");
        assert_eq!(event["title"], "Meeting");
        assert_eq!(event["calendar_id"], "cal-1");
        assert_eq!(event["is_all_day"], false);
    }

    #[test]
    fn test_migrate_meta_no_match_when_event_id_differs_from_row_id() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let events = serde_json::json!({"different-row-id": make_event("track-1", "Not Matched")});
        write_json(&base.join("events.json"), &events);

        let meta = make_meta("track-1");
        let meta_path = base.join("sessions/session-1/_meta.json");
        write_json(&meta_path, &meta);

        let events_map = load_events(base);
        let mut ops = Vec::new();
        migrate_session_metas(base, &events_map, &mut ops);
        apply_ops(ops).unwrap();

        let result = read_json(&meta_path);
        assert!(result.get("event_id").is_none());
        assert!(result.get("event").is_none());
    }

    #[test]
    fn test_migrate_meta_clears_event_id_when_not_found() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let meta = make_meta("nonexistent");
        let meta_path = base.join("sessions/session-1/_meta.json");
        write_json(&meta_path, &meta);

        let events_map = load_events(base);
        let mut ops = Vec::new();
        migrate_session_metas(base, &events_map, &mut ops);
        apply_ops(ops).unwrap();

        let result = read_json(&meta_path);
        assert!(result.get("event_id").is_none());
        assert!(result.get("event").is_none());
    }

    #[test]
    fn test_migrate_meta_skips_already_migrated() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let meta = serde_json::json!({
            "id": "session-1",
            "event": { "tracking_id": "track-1" },
            "participants": [],
        });
        let meta_path = base.join("sessions/session-1/_meta.json");
        write_json(&meta_path, &meta);

        let events_map = load_events(base);
        let mut ops = Vec::new();
        migrate_session_metas(base, &events_map, &mut ops);

        assert!(ops.is_empty());
    }

    #[test]
    fn test_migrate_meta_skips_sessions_without_event() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let meta = serde_json::json!({"id": "session-1", "participants": []});
        let meta_path = base.join("sessions/session-1/_meta.json");
        write_json(&meta_path, &meta);

        let events_map = load_events(base);
        let mut ops = Vec::new();
        migrate_session_metas(base, &events_map, &mut ops);

        assert!(ops.is_empty());
    }

    #[test]
    fn test_migrate_multiple_sessions() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let events = serde_json::json!({
            "row-1": make_event("track-1", "Meeting 1"),
            "row-2": make_event("track-2", "Meeting 2"),
        });
        write_json(&base.join("events.json"), &events);

        let path1 = base.join("sessions/session-1/_meta.json");
        let path2 = base.join("sessions/session-2/_meta.json");
        write_json(&path1, &make_meta("row-1"));
        write_json(&path2, &make_meta("row-2"));

        let events_map = load_events(base);
        let mut ops = Vec::new();
        migrate_session_metas(base, &events_map, &mut ops);
        apply_ops(ops).unwrap();

        assert_eq!(read_json(&path1)["event"]["tracking_id"], "track-1");
        assert_eq!(read_json(&path2)["event"]["tracking_id"], "track-2");
    }

    #[test]
    fn test_migrate_meta_includes_optional_fields() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let mut event = make_event("track-1", "Full Event");
        event["location"] = Value::String("Room 42".to_string());
        event["meeting_link"] = Value::String("https://meet.example.com".to_string());
        event["description"] = Value::String("A meeting".to_string());
        event["recurrence_series_id"] = Value::String("series-1".to_string());

        write_json(
            &base.join("events.json"),
            &serde_json::json!({"row-1": event}),
        );
        let meta_path = base.join("sessions/session-1/_meta.json");
        write_json(&meta_path, &make_meta("row-1"));

        let events_map = load_events(base);
        let mut ops = Vec::new();
        migrate_session_metas(base, &events_map, &mut ops);
        apply_ops(ops).unwrap();

        let event = &read_json(&meta_path)["event"];
        assert_eq!(event["location"], "Room 42");
        assert_eq!(event["meeting_link"], "https://meet.example.com");
        assert_eq!(event["description"], "A meeting");
        assert_eq!(event["recurrence_series_id"], "series-1");
    }

    #[test]
    fn test_migrate_meta_omits_absent_optional_fields() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        write_json(
            &base.join("events.json"),
            &serde_json::json!({"row-1": make_event("track-1", "Basic")}),
        );
        let meta_path = base.join("sessions/session-1/_meta.json");
        write_json(&meta_path, &make_meta("row-1"));

        let events_map = load_events(base);
        let mut ops = Vec::new();
        migrate_session_metas(base, &events_map, &mut ops);
        apply_ops(ops).unwrap();

        let event = &read_json(&meta_path)["event"];
        assert!(event.get("location").is_none());
        assert!(event.get("meeting_link").is_none());
        assert!(event.get("description").is_none());
        assert!(event.get("recurrence_series_id").is_none());
    }

    // events.json tests

    #[test]
    fn test_clean_events_json_removes_ignored() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let events = serde_json::json!({
            "event-1": {"tracking_id_event": "track-1", "title": "Meeting", "ignored": true},
            "event-2": {"tracking_id_event": "track-2", "title": "Lunch", "ignored": false},
        });
        write_json(&base.join("events.json"), &events);

        let mut ops = Vec::new();
        clean_events_json(base, &mut ops);
        apply_ops(ops).unwrap();

        let result = read_json(&base.join("events.json"));
        assert!(result["event-1"].get("ignored").is_none());
        assert!(result["event-2"].get("ignored").is_none());
        assert_eq!(result["event-1"]["title"], "Meeting");
    }

    #[test]
    fn test_clean_events_json_skips_when_no_ignored() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        write_json(
            &base.join("events.json"),
            &serde_json::json!({"event-1": {"title": "Meeting"}}),
        );

        let mut ops = Vec::new();
        clean_events_json(base, &mut ops);
        assert!(ops.is_empty());
    }

    #[test]
    fn test_no_events_json() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let meta_path = base.join("sessions/session-1/_meta.json");
        write_json(&meta_path, &make_meta("nonexistent"));

        let events_map = load_events(base);
        let mut ops = Vec::new();
        migrate_session_metas(base, &events_map, &mut ops);
        apply_ops(ops).unwrap();

        let result = read_json(&meta_path);
        assert!(result.get("event_id").is_none());
        assert!(result.get("event").is_none());
    }

    // day_from_started_at tests

    #[test]
    fn test_day_from_started_at_utc() {
        assert_eq!(
            day_from_started_at("2024-01-15T10:00:00Z", None),
            "2024-01-15"
        );
    }

    #[test]
    fn test_day_from_started_at_with_timezone_same_day() {
        assert_eq!(
            day_from_started_at("2024-01-15T10:00:00Z", Some("America/New_York")),
            "2024-01-15"
        );
    }

    #[test]
    fn test_day_from_started_at_with_timezone_crosses_day() {
        assert_eq!(
            day_from_started_at("2024-01-15T02:00:00Z", Some("America/New_York")),
            "2024-01-14"
        );
    }

    #[test]
    fn test_day_from_started_at_with_timezone_crosses_day_forward() {
        assert_eq!(
            day_from_started_at("2024-01-14T23:00:00Z", Some("Asia/Tokyo")),
            "2024-01-15"
        );
    }

    #[test]
    fn test_day_from_started_at_invalid_timezone_falls_back() {
        assert_eq!(
            day_from_started_at("2024-01-15T10:00:00Z", Some("Invalid/Timezone")),
            "2024-01-15"
        );
    }

    #[test]
    fn test_day_from_started_at_invalid_date() {
        assert_eq!(day_from_started_at("", None), "1970-01-01");
    }

    #[test]
    fn test_day_from_started_at_non_rfc3339_with_date_prefix() {
        assert_eq!(day_from_started_at("2024-01-15 bad", None), "2024-01-15");
    }

    // load_timezone tests

    #[test]
    fn test_load_timezone_from_settings() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let settings = serde_json::json!({
            "general": { "timezone": "America/New_York" }
        });
        write_json(&base.join("settings.json"), &settings);

        assert_eq!(load_timezone(base), Some("America/New_York".to_string()));
    }

    #[test]
    fn test_load_timezone_missing_file() {
        let tmp = tempdir().unwrap();
        assert_eq!(load_timezone(tmp.path()), None);
    }

    #[test]
    fn test_load_timezone_missing_key() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        write_json(
            &base.join("settings.json"),
            &serde_json::json!({"general": {}}),
        );
        assert_eq!(load_timezone(base), None);
    }

    // collect_ignored_events tests

    #[test]
    fn test_collect_ignored_events() {
        let events: HashMap<String, Value> = serde_json::from_value(serde_json::json!({
            "e1": {"tracking_id_event": "track-1", "started_at": "2024-01-15T10:00:00Z", "ignored": true},
            "e2": {"tracking_id_event": "track-2", "started_at": "2024-02-01T09:00:00Z", "ignored": false},
            "e3": {"tracking_id_event": "track-3", "started_at": "2024-03-10T14:00:00Z", "ignored": true},
        }))
        .unwrap();

        let result = collect_ignored_events(&events, &HashSet::new(), None);
        assert_eq!(result.len(), 2);

        let tids: Vec<&str> = result
            .iter()
            .filter_map(|e| e.get("tracking_id").and_then(|v| v.as_str()))
            .collect();
        assert!(tids.contains(&"track-1"));
        assert!(tids.contains(&"track-3"));

        for entry in &result {
            assert_eq!(entry["is_recurrent"], false);
            assert!(entry.get("day").is_none());
        }
    }

    #[test]
    fn test_collect_ignored_events_recurrent_extracts_day() {
        let events: HashMap<String, Value> = serde_json::from_value(serde_json::json!({
            "e1": {"tracking_id_event": "track-1", "started_at": "2024-01-15T10:00:00Z", "ignored": true, "has_recurrence_rules": true},
        }))
        .unwrap();

        let result = collect_ignored_events(&events, &HashSet::new(), None);
        assert_eq!(result[0]["is_recurrent"], true);
        assert_eq!(result[0]["day"], "2024-01-15");
    }

    #[test]
    fn test_collect_ignored_events_recurrent_with_timezone() {
        let events: HashMap<String, Value> = serde_json::from_value(serde_json::json!({
            "e1": {"tracking_id_event": "track-1", "started_at": "2024-01-15T02:00:00Z", "ignored": true, "has_recurrence_rules": true},
        }))
        .unwrap();

        let result = collect_ignored_events(&events, &HashSet::new(), Some("America/New_York"));
        assert_eq!(result[0]["is_recurrent"], true);
        assert_eq!(result[0]["day"], "2024-01-14");
    }

    #[test]
    fn test_collect_ignored_events_recurrent_fallback_day() {
        let events: HashMap<String, Value> = serde_json::from_value(serde_json::json!({
            "e1": {"tracking_id_event": "track-1", "ignored": true, "has_recurrence_rules": true},
        }))
        .unwrap();

        let result = collect_ignored_events(&events, &HashSet::new(), None);
        assert_eq!(result[0]["is_recurrent"], true);
        assert_eq!(result[0]["day"], "1970-01-01");
    }

    #[test]
    fn test_collect_ignored_events_non_recurrent_no_day() {
        let events: HashMap<String, Value> = serde_json::from_value(serde_json::json!({
            "e1": {"tracking_id_event": "track-1", "started_at": "2024-01-15T10:00:00Z", "ignored": true},
        }))
        .unwrap();

        let result = collect_ignored_events(&events, &HashSet::new(), None);
        assert_eq!(result[0]["is_recurrent"], false);
        assert!(result[0].get("day").is_none());
    }

    #[test]
    fn test_collect_ignored_events_skips_already_ignored_series() {
        let events: HashMap<String, Value> = serde_json::from_value(serde_json::json!({
            "e1": {"tracking_id_event": "track-1", "started_at": "2024-01-15T10:00:00Z", "ignored": true, "recurrence_series_id": "series-1"},
            "e2": {"tracking_id_event": "track-2", "started_at": "2024-02-01T09:00:00Z", "ignored": true},
            "e3": {"tracking_id_event": "track-3", "started_at": "2024-03-10T14:00:00Z", "ignored": true, "recurrence_series_id": "series-2"},
        }))
        .unwrap();

        let ignored_series: HashSet<String> = ["series-1".to_string()].into();
        let result = collect_ignored_events(&events, &ignored_series, None);
        assert_eq!(result.len(), 2);

        let tids: Vec<&str> = result
            .iter()
            .filter_map(|e| e.get("tracking_id").and_then(|v| v.as_str()))
            .collect();
        assert!(!tids.contains(&"track-1"));
        assert!(tids.contains(&"track-2"));
        assert!(tids.contains(&"track-3"));
    }

    // store.json / ignored_events tests

    #[test]
    fn test_migrate_store_adds_ignored_events() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let tb_values = serde_json::json!({"user_id": "user-1"});
        write_json(&base.join("store.json"), &make_store(&tb_values));

        let ignored = vec![serde_json::json!({
            "tracking_id": "track-1", "is_recurrent": false, "last_seen": "2024-01-20T00:00:00Z"
        })];

        let mut ops = Vec::new();
        migrate_store_values(base, &ignored, &mut ops);
        apply_ops(ops).unwrap();

        let store = read_json(&base.join("store.json"));
        let tb = read_tb_values(&store);
        let ignored_str = tb["ignored_events"].as_str().unwrap();
        let parsed: Vec<Value> = serde_json::from_str(ignored_str).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["tracking_id"], "track-1");
        assert_eq!(parsed[0]["is_recurrent"], false);
    }

    #[test]
    fn test_migrate_store_merges_with_existing_ignored_events() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let existing = serde_json::json!([
            {"tracking_id": "track-1", "is_recurrent": false, "last_seen": "old"}
        ]);
        let tb_values = serde_json::json!({
            "user_id": "user-1",
            "ignored_events": serde_json::to_string(&existing).unwrap(),
        });
        write_json(&base.join("store.json"), &make_store(&tb_values));

        let ignored = vec![
            serde_json::json!({"tracking_id": "track-1", "is_recurrent": false, "last_seen": "new"}),
            serde_json::json!({"tracking_id": "track-2", "is_recurrent": true, "day": "2024-02-01", "last_seen": "new"}),
        ];

        let mut ops = Vec::new();
        migrate_store_values(base, &ignored, &mut ops);
        apply_ops(ops).unwrap();

        let store = read_json(&base.join("store.json"));
        let tb = read_tb_values(&store);
        let parsed: Vec<Value> =
            serde_json::from_str(tb["ignored_events"].as_str().unwrap()).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["last_seen"], "old");
        assert_eq!(parsed[1]["tracking_id"], "track-2");
        assert_eq!(parsed[1]["is_recurrent"], true);
        assert_eq!(parsed[1]["day"], "2024-02-01");
    }

    // store.json / ignored_recurring_series tests

    #[test]
    fn test_migrate_ignored_recurring_series_old_format() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let tb_values = serde_json::json!({
            "ignored_recurring_series": "[\"series-1\",\"series-2\"]",
        });
        write_json(&base.join("store.json"), &make_store(&tb_values));

        let mut ops = Vec::new();
        migrate_store_values(base, &[], &mut ops);
        apply_ops(ops).unwrap();

        let store = read_json(&base.join("store.json"));
        let tb = read_tb_values(&store);
        let parsed: Vec<Value> =
            serde_json::from_str(tb["ignored_recurring_series"].as_str().unwrap()).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["id"], "series-1");
        assert!(parsed[0].get("last_seen").is_some());
        assert_eq!(parsed[1]["id"], "series-2");
    }

    #[test]
    fn test_migrate_ignored_recurring_series_already_migrated() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let series = serde_json::json!([{"id": "series-1", "last_seen": "2024-01-01T00:00:00Z"}]);
        let tb_values = serde_json::json!({
            "ignored_recurring_series": serde_json::to_string(&series).unwrap(),
        });
        write_json(&base.join("store.json"), &make_store(&tb_values));

        let mut ops = Vec::new();
        migrate_store_values(base, &[], &mut ops);
        assert!(ops.is_empty());
    }

    #[test]
    fn test_migrate_store_skips_when_no_store_json() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let mut ops = Vec::new();
        migrate_store_values(base, &[], &mut ops);
        assert!(ops.is_empty());
    }

    #[test]
    fn test_migrate_store_skips_when_nothing_to_do() {
        let tmp = tempdir().unwrap();
        let base = tmp.path();

        let tb_values = serde_json::json!({"user_id": "user-1"});
        write_json(&base.join("store.json"), &make_store(&tb_values));

        let mut ops = Vec::new();
        migrate_store_values(base, &[], &mut ops);
        assert!(ops.is_empty());
    }
}
