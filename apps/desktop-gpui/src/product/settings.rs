use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};

use desktop_runtime::{QueryWatch, Reply, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;

const LEGACY_SETTINGS: &str = "legacy_settings_document";
const LEGACY_MAIN: &str = "legacy_main_values_document";
const BINDING: &str = "cloudsync_workspace_binding";
pub const MAX_VALUE_BYTES: usize = 512 * 1024;

#[derive(Debug)]
pub struct Definition {
    pub key: String,
    pub kind: String,
    pub path: [String; 2],
    pub default: Value,
    pub synced: bool,
}

pub fn definitions() -> &'static [Definition] {
    static DEFINITIONS: OnceLock<Vec<Definition>> = OnceLock::new();
    DEFINITIONS.get_or_init(|| {
        let value: Value =
            serde_json::from_str(include_str!("settings-schema.json")).expect("generated schema");
        value
            .as_object()
            .expect("schema object")
            .iter()
            .map(|(key, value)| Definition {
                key: key.clone(),
                kind: value["type"].as_str().expect("type").into(),
                path: [
                    value["path"][0].as_str().expect("section").into(),
                    value["path"][1].as_str().expect("field").into(),
                ],
                default: value["default"].clone(),
                synced: value["synced"] == true,
            })
            .collect()
    })
}

pub fn definition(key: &str) -> Result<&'static Definition> {
    definitions()
        .iter()
        .find(|definition| definition.key == key)
        .ok_or_else(|| ServiceError::Unsupported("Unknown preference".into()))
}

pub fn write_gate(key: &str) -> Option<&'static str> {
    match key {
        "autostart"
        | "automatic_updates"
        | "respect_dnd"
        | "ignored_platforms"
        | "included_platforms"
        | "mic_active_threshold"
        | "telemetry_consent"
        | "crash_reporting_consent"
        | "show_app_in_dock"
        | "show_tray_icon" => Some("Requires the native settings side-effect service."),
        "lock_app" => Some("Requires native authentication before changing device lock."),
        "cloud_sync_enabled" => Some("Requires the CloudSync lifecycle service."),
        "current_stt_provider" | "current_stt_model" | "local_stt_model_path" => {
            Some("Requires model inspection and successful engine startup before persistence.")
        }
        "current_llm_provider" | "current_llm_model" | "spoken_languages" => {
            Some("Requires provider validation and the consent-aware analytics service.")
        }
        _ => None,
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Revision {
    local: Option<String>,
    synced: Option<String>,
    context: Arc<RevisionContext>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct RevisionContext {
    binding: Option<String>,
    legacy_settings: Option<String>,
    legacy_main: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub value: Value,
    pub revision: Revision,
    pub source: &'static str,
}

pub type Snapshot = BTreeMap<String, Setting>;

fn raw<'a>(rows: &'a [Value], key: &str, rank: u64) -> Option<&'a str> {
    rows.iter()
        .find(|row| row["id"] == key && row["source_rank"] == rank)?
        .get("value_json")?
        .as_str()
}

fn parse(value: Option<&str>) -> Option<Value> {
    value.and_then(|value| serde_json::from_str(value).ok())
}

fn retention(value: &Value) -> Option<Value> {
    match value {
        Value::Bool(value) => Some(json!(if *value { "forever" } else { "none" })),
        Value::String(value)
            if [
                "none",
                "oneDay",
                "threeDays",
                "oneWeek",
                "oneMonth",
                "forever",
            ]
            .contains(&value.as_str()) =>
        {
            Some(json!(value))
        }
        _ => None,
    }
}

pub fn normalize(def: &Definition, value: Value, direct: bool) -> Option<Value> {
    if def.key == "audio_retention" {
        return retention(&value);
    }
    if [
        "spoken_languages",
        "personalization_dictionary_terms",
        "ignored_platforms",
        "included_platforms",
    ]
    .contains(&def.key.as_str())
    {
        if value.is_array() {
            return Some(json!(value.to_string()));
        }
        let text = value.as_str()?;
        if serde_json::from_str::<Value>(text).is_ok_and(|value| value.is_array()) {
            return Some(value);
        }
        if !direct && text.contains(',') {
            return Some(json!(
                json!(
                    text.split(',')
                        .map(str::trim)
                        .filter(|part| !part.is_empty())
                        .collect::<Vec<_>>()
                )
                .to_string()
            ));
        }
        return None;
    }
    match def.kind.as_str() {
        "boolean" if value.is_boolean() => Some(value),
        "number" if value.is_number() => Some(value),
        "string" if value.is_string() => Some(value),
        _ => None,
    }
}

fn legacy(def: &Definition, settings: &Value, main: &Value) -> Option<Value> {
    let mut value = settings
        .get(&def.path[0])
        .and_then(|section| section.get(&def.path[1]))
        .cloned();
    if value.is_none() && ["ai_language", "spoken_languages"].contains(&def.key.as_str()) {
        value = settings["general"].get(&def.key).cloned();
    }
    value.or_else(|| main.get(&def.key).cloned())
}

pub fn decode(rows: &[Value]) -> Result<Snapshot> {
    if rows.iter().any(|row| row["oversized"] == 1) {
        return Err(ServiceError::Unsupported(
            "A stored preference exceeds the 512 KiB read limit; it has not been changed.".into(),
        ));
    }
    let settings = parse(raw(rows, LEGACY_SETTINGS, 0)).unwrap_or(Value::Null);
    let main = parse(raw(rows, LEGACY_MAIN, 0)).unwrap_or(Value::Null);
    let context = Arc::new(RevisionContext {
        binding: raw(rows, BINDING, 0).map(str::to_owned),
        legacy_settings: raw(rows, LEGACY_SETTINGS, 0).map(str::to_owned),
        legacy_main: raw(rows, LEGACY_MAIN, 0).map(str::to_owned),
    });
    let mut snapshot = Snapshot::new();
    for def in definitions() {
        let local = raw(rows, &def.key, 0);
        let synced = raw(rows, &def.key, 1);
        let direct = parse(synced.or(local)).and_then(|value| normalize(def, value, true));
        let inherited = if def.key == "audio_retention" {
            [
                &settings["general"]["audio_retention"],
                &settings["general"]["saveAudioAfterMeeting"],
                &settings["general"]["save_recordings"],
                &main["audio_retention"],
                &main["save_recordings"],
            ]
            .into_iter()
            .find_map(retention)
        } else {
            legacy(def, &settings, &main).and_then(|value| normalize(def, value, false))
        };
        let source = if direct.is_some() {
            if synced.is_some() { "Synced" } else { "Device" }
        } else if inherited.is_some() {
            "Legacy"
        } else {
            "Default"
        };
        snapshot.insert(
            def.key.clone(),
            Setting {
                value: direct.or(inherited).unwrap_or_else(|| def.default.clone()),
                source,
                revision: Revision {
                    local: local.map(str::to_owned),
                    synced: synced.map(str::to_owned),
                    context: context.clone(),
                },
            },
        );
    }
    Ok(snapshot)
}

pub fn watch(runtime: &RuntimeHandle) -> Result<Reply<QueryWatch>> {
    let mut keys: Vec<&str> = definitions().iter().map(|def| def.key.as_str()).collect();
    keys.extend([LEGACY_SETTINGS, LEGACY_MAIN, BINDING]);
    runtime.watch_query(
        format!(
            "SELECT id, CASE WHEN length(CAST(value_json AS BLOB)) <= {MAX_VALUE_BYTES} THEN value_json END AS value_json,
             length(CAST(value_json AS BLOB)) > {MAX_VALUE_BYTES} AS oversized, 0 AS source_rank
             FROM app_settings WHERE id IN (SELECT value FROM json_each(?1))
             UNION ALL
             SELECT id, CASE WHEN length(CAST(value_json AS BLOB)) <= {MAX_VALUE_BYTES} THEN value_json END,
             length(CAST(value_json AS BLOB)) > {MAX_VALUE_BYTES}, 1
             FROM synced_preferences WHERE id IN (SELECT value FROM json_each(?1))
             ORDER BY id, source_rank"
        ),
        vec![json!(json!(keys).to_string())],
    )
}

pub fn save(
    runtime: &RuntimeHandle,
    key: String,
    base: Setting,
    value: Value,
) -> Result<Reply<Setting>> {
    let def = definition(&key)?;
    if let Some(reason) = write_gate(&key) {
        return Err(ServiceError::Unsupported(reason.into()));
    }
    if !def.synced && base.revision.synced.is_some() {
        return Err(ServiceError::Unsupported(
            "This device preference also has a synced row. Resolve its ownership before saving."
                .into(),
        ));
    }
    let value = normalize(def, value, true)
        .ok_or_else(|| ServiceError::Failed("Invalid preference value".into()))?;
    runtime.submit(move |services| async move {
        let serialized = value.to_string();
        if serialized.len() > MAX_VALUE_BYTES {
            return Err(ServiceError::Unsupported("Preference exceeds 512 KiB".into()));
        }
        let table = if def.synced { "synced_preferences" } else { "app_settings" };
        let (columns, workspace, update) = if def.synced {
            (", workspace_id", ", NULLIF(json_extract(?5, '$.workspace_id'), '')", ", workspace_id = excluded.workspace_id")
        } else {
            ("", "", "")
        };
        let rows = services.executor.execute(
            format!(
                "INSERT INTO {table} (id, value_json, updated_at{columns})
                 SELECT ?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'){workspace}
                 WHERE (SELECT value_json FROM app_settings WHERE id = ?1) IS ?3
                 AND (SELECT value_json FROM synced_preferences WHERE id = ?1) IS ?4
                 AND (SELECT value_json FROM app_settings WHERE id = 'cloudsync_workspace_binding') IS ?5
                 AND (SELECT value_json FROM app_settings WHERE id = 'legacy_settings_document') IS ?6
                 AND (SELECT value_json FROM app_settings WHERE id = 'legacy_main_values_document') IS ?7
                 ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json,
                    updated_at = excluded.updated_at{update}
                 RETURNING id"
            ),
            vec![json!(key), json!(serialized), json!(base.revision.local),
                json!(base.revision.synced), json!(base.revision.context.binding),
                json!(base.revision.context.legacy_settings), json!(base.revision.context.legacy_main)],
        ).await.map_err(|error| ServiceError::Failed(error.to_string().into()))?;
        if rows.is_empty() {
            return Err(ServiceError::Conflict);
        }
        let mut revision = base.revision;
        if def.synced {
            revision.synced = Some(serialized);
        } else {
            revision.local = Some(serialized);
        }
        Ok(Setting { value, revision, source: if def.synced { "Synced" } else { "Device" } })
    })
}

#[derive(Clone, Debug)]
pub struct Draft {
    pub base: Setting,
    pub latest: Setting,
    pub text: String,
    pub dirty: bool,
    pub saving: bool,
    pub error: Option<String>,
}

impl Draft {
    pub fn new(base: Setting) -> Self {
        Self {
            text: base
                .value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| base.value.to_string()),
            latest: base.clone(),
            base,
            dirty: false,
            saving: false,
            error: None,
        }
    }

    pub fn observe(&mut self, value: Setting) {
        self.latest = value.clone();
        if !self.dirty && !self.saving {
            *self = Self::new(value);
        }
    }

    pub fn restore(&mut self) {
        *self = Self::new(self.latest.clone());
    }

    pub fn saved(&mut self, value: Setting, submitted: &str) {
        if self.latest == self.base {
            self.latest = value.clone();
        }
        self.base = value;
        self.dirty = self.text != submitted;
        self.saving = false;
    }

    pub fn value(&self, def: &Definition) -> Result<Value> {
        let value = if def.kind == "string" {
            json!(self.text)
        } else {
            serde_json::from_str(&self.text)
                .map_err(|_| ServiceError::Failed(format!("Expected {}", def.kind).into()))?
        };
        normalize(def, value, true)
            .ok_or_else(|| ServiceError::Failed(format!("Expected {}", def.kind).into()))
    }
}
