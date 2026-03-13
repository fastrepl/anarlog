use serde_json::Value;

pub(crate) fn flatten_all_of_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(chosen_ref) = extract_allof_ref(map) {
                map.clear();
                map.insert("$ref".into(), Value::String(chosen_ref));
                return;
            }
            for v in map.values_mut() {
                flatten_all_of_value(v);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                flatten_all_of_value(v);
            }
        }
        _ => {}
    }
}

fn extract_allof_ref(map: &serde_json::Map<String, Value>) -> Option<String> {
    let items = map.get("allOf")?.as_array()?;
    items
        .iter()
        .rev()
        .find_map(|item| item.get("$ref")?.as_str().map(String::from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn replaces_allof_with_last_ref() {
        let mut value = json!({
            "allOf": [
                { "$ref": "#/components/schemas/GenericId" },
                { "$ref": "#/components/schemas/Contact" }
            ]
        });
        flatten_all_of_value(&mut value);
        assert_eq!(value, json!({ "$ref": "#/components/schemas/Contact" }));
    }

    #[test]
    fn picks_last_ref_skipping_non_ref_entries() {
        let mut value = json!({
            "allOf": [
                { "$ref": "#/components/schemas/First" },
                { "type": "object", "properties": {} },
                { "$ref": "#/components/schemas/Second" }
            ]
        });
        flatten_all_of_value(&mut value);
        assert_eq!(value, json!({ "$ref": "#/components/schemas/Second" }));
    }

    #[test]
    fn recurses_into_nested_objects() {
        let mut value = json!({
            "schema": {
                "allOf": [
                    { "$ref": "#/components/schemas/Inner" }
                ]
            }
        });
        flatten_all_of_value(&mut value);
        assert_eq!(
            value,
            json!({ "schema": { "$ref": "#/components/schemas/Inner" } })
        );
    }

    #[test]
    fn leaves_non_allof_objects_unchanged() {
        let original = json!({ "type": "string" });
        let mut value = original.clone();
        flatten_all_of_value(&mut value);
        assert_eq!(value, original);
    }
}
