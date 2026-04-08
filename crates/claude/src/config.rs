use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEntry {
    #[serde(rename = "type")]
    pub hook_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub hooks: Vec<HookEntry>,
}

pub type HooksConfig = HashMap<String, Vec<HookMatcher>>;

pub fn settings_path() -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".claude")
        .join("settings.json")
}

pub fn read_settings(path: &Path) -> Result<serde_json::Value, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents)
            .map_err(|e| format!("failed to parse {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(serde_json::Value::Object(serde_json::Map::new()))
        }
        Err(e) => Err(format!("failed to read {}: {e}", path.display())),
    }
}

pub fn write_settings(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    let contents = serde_json::to_string_pretty(value)
        .map_err(|e| format!("failed to serialize settings: {e}"))?;
    std::fs::write(path, contents).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

pub fn upsert_command_hook(
    settings: &mut serde_json::Value,
    event_name: &str,
    command: &str,
) -> Result<(), String> {
    let hooks = hooks_object_mut(settings)?;
    let event_hooks = hooks
        .entry(event_name.to_string())
        .or_insert_with(|| serde_json::Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| format!("hooks.{event_name} must be an array"))?;

    if event_hooks
        .iter()
        .any(|matcher| hook_matcher_has_command(matcher, command))
    {
        return Ok(());
    }

    event_hooks.push(serde_json::json!({
        "hooks": [{ "type": "command", "command": command }]
    }));

    Ok(())
}

pub fn has_command_hook(settings: &serde_json::Value, event_name: &str, command: &str) -> bool {
    settings
        .get("hooks")
        .and_then(serde_json::Value::as_object)
        .and_then(|hooks| hooks.get(event_name))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|event_hooks| {
            event_hooks
                .iter()
                .any(|matcher| hook_matcher_has_command(matcher, command))
        })
}

pub fn remove_command_hook(
    settings: &mut serde_json::Value,
    event_name: &str,
    command: &str,
) -> Result<(), String> {
    let Some(hooks) = settings
        .as_object_mut()
        .and_then(|obj| obj.get_mut("hooks"))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Ok(());
    };

    let Some(event_hooks) = hooks
        .get_mut(event_name)
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Ok(());
    };

    event_hooks.retain_mut(|matcher| {
        if let Some(entries) = matcher
            .get_mut("hooks")
            .and_then(serde_json::Value::as_array_mut)
        {
            entries.retain(|entry| !hook_entry_matches_command(entry, command));
            !entries.is_empty()
        } else {
            true
        }
    });

    if event_hooks.is_empty() {
        hooks.remove(event_name);
    }

    Ok(())
}

fn hooks_object_mut(
    settings: &mut serde_json::Value,
) -> Result<&mut serde_json::Map<String, serde_json::Value>, String> {
    let obj = settings
        .as_object_mut()
        .ok_or_else(|| "expected settings to be a JSON object".to_string())?;

    obj.entry("hooks")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "hooks must be an object".to_string())
}

fn hook_matcher_has_command(matcher: &serde_json::Value, command: &str) -> bool {
    matcher
        .get("hooks")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| hook_entry_matches_command(entry, command))
        })
}

fn hook_entry_matches_command(entry: &serde_json::Value, command: &str) -> bool {
    entry.get("type").and_then(serde_json::Value::as_str) == Some("command")
        && entry.get("command").and_then(serde_json::Value::as_str) == Some(command)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{has_command_hook, remove_command_hook, upsert_command_hook};

    #[test]
    fn upsert_command_hook_is_idempotent() {
        let mut settings = json!({});

        upsert_command_hook(&mut settings, "Stop", "char claude notify").expect("upsert");
        upsert_command_hook(&mut settings, "Stop", "char claude notify").expect("upsert");

        assert_eq!(
            settings,
            json!({
                "hooks": {
                    "Stop": [
                        {
                            "hooks": [
                                { "type": "command", "command": "char claude notify" }
                            ]
                        }
                    ]
                }
            })
        );
    }

    #[test]
    fn remove_command_hook_only_removes_matching_command() {
        let mut settings = json!({
            "hooks": {
                "Stop": [
                    {
                        "hooks": [
                            { "type": "command", "command": "char claude notify" },
                            { "type": "command", "command": "other command" }
                        ]
                    },
                    {
                        "hooks": [
                            { "type": "url", "url": "https://example.com/hook" }
                        ]
                    }
                ]
            }
        });

        remove_command_hook(&mut settings, "Stop", "char claude notify").expect("remove");

        assert_eq!(
            settings,
            json!({
                "hooks": {
                    "Stop": [
                        {
                            "hooks": [
                                { "type": "command", "command": "other command" }
                            ]
                        },
                        {
                            "hooks": [
                                { "type": "url", "url": "https://example.com/hook" }
                            ]
                        }
                    ]
                }
            })
        );
    }

    #[test]
    fn detects_matching_command_hook() {
        let settings = json!({
            "hooks": {
                "Stop": [
                    {
                        "hooks": [
                            { "type": "command", "command": "char claude notify" }
                        ]
                    }
                ]
            }
        });

        assert!(has_command_hook(&settings, "Stop", "char claude notify"));
        assert!(!has_command_hook(&settings, "Stop", "other command"));
    }
}
