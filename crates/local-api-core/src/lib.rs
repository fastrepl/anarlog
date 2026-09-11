//! The desktop's local API surface shared by the Tauri `local-api` plugin
//! and the GPUI shell: outbound meeting webhooks, the markdown export
//! automation, and the cloud snapshot shape.

pub mod dispatch;
pub mod export;
pub mod types;

pub use types::*;

/// The JSON the plugin has always emitted: every object's keys in sorted
/// order, whichever `serde_json` map the build links, so a webhook body or a
/// run record is byte-identical from the desktop app and the shell.
pub fn sorted_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(&String, &serde_json::Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            let mut sorted = serde_json::Map::new();
            for (key, value) in entries {
                sorted.insert(key.clone(), sorted_json(value));
            }
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(sorted_json).collect())
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sorted_json_orders_every_level() {
        let value = serde_json::json!({
            "id": 1,
            "data": { "meeting": { "title": "t", "action_items": [] }, "transcript_text": "" },
            "created_at": "c",
        });
        assert_eq!(
            super::sorted_json(&value).to_string(),
            r#"{"created_at":"c","data":{"meeting":{"action_items":[],"title":"t"},"transcript_text":""},"id":1}"#
        );
    }
}
