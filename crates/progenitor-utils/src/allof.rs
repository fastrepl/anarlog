use serde_json::Value;

pub(crate) fn flatten_all_of_value(value: &mut Value) {
    let schemas = value
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    flatten_all_of_value_with_schemas(value, &schemas);
}

fn flatten_all_of_value_with_schemas(value: &mut Value, schemas: &serde_json::Map<String, Value>) {
    match value {
        Value::Object(map) => {
            if let Some(replacement) = flatten_allof_object(map, schemas) {
                *map = replacement;
            }
            for v in map.values_mut() {
                flatten_all_of_value_with_schemas(v, schemas);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                flatten_all_of_value_with_schemas(v, schemas);
            }
        }
        _ => {}
    }
}

fn flatten_allof_object(
    map: &serde_json::Map<String, Value>,
    schemas: &serde_json::Map<String, Value>,
) -> Option<serde_json::Map<String, Value>> {
    let items = map.get("allOf")?.as_array()?;
    if !items.iter().any(|item| item.get("$ref").is_none()) {
        let chosen_ref = items
            .iter()
            .rev()
            .find_map(|item| item.get("$ref")?.as_str().map(String::from))?;
        return Some(serde_json::Map::from_iter([(
            "$ref".into(),
            Value::String(chosen_ref),
        )]));
    }

    let mut merged = serde_json::Map::new();
    for item in items {
        let item = match item.get("$ref").and_then(Value::as_str) {
            Some(reference) => schemas.get(reference.strip_prefix("#/components/schemas/")?)?,
            None => item,
        };
        merge_schema(&mut merged, item.as_object()?);
    }
    for (key, value) in map {
        if key != "allOf" {
            merged.insert(key.clone(), value.clone());
        }
    }
    Some(merged)
}

fn merge_schema(
    target: &mut serde_json::Map<String, Value>,
    source: &serde_json::Map<String, Value>,
) {
    for (key, value) in source {
        match (key.as_str(), value) {
            ("properties", Value::Object(properties)) => {
                let target_properties = target
                    .entry(key.clone())
                    .or_insert_with(|| Value::Object(serde_json::Map::new()))
                    .as_object_mut()
                    .expect("schema properties must be an object");
                target_properties.extend(properties.clone());
            }
            ("required", Value::Array(required)) => {
                let target_required = target
                    .entry(key.clone())
                    .or_insert_with(|| Value::Array(Vec::new()))
                    .as_array_mut()
                    .expect("schema required must be an array");
                for field in required {
                    if !target_required.contains(field) {
                        target_required.push(field.clone());
                    }
                }
            }
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
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
    fn picks_last_ref_from_multi_ref_allof() {
        let mut value = json!({
            "allOf": [
                { "$ref": "#/components/schemas/First" },
                { "$ref": "#/components/schemas/Second" }
            ]
        });
        flatten_all_of_value(&mut value);
        assert_eq!(value, json!({ "$ref": "#/components/schemas/Second" }));
    }

    #[test]
    fn merges_referenced_and_inline_object_fields() {
        let mut value = json!({
            "components": {
                "schemas": {
                    "Base": {
                        "type": "object",
                        "required": ["id"],
                        "properties": {
                            "id": { "type": "string" }
                        }
                    },
                    "Extended": {
                        "allOf": [
                            { "$ref": "#/components/schemas/Base" },
                            {
                                "type": "object",
                                "required": ["secret"],
                                "properties": {
                                    "secret": { "type": "string" }
                                }
                            }
                        ]
                    }
                }
            }
        });
        flatten_all_of_value(&mut value);
        assert_eq!(
            value.pointer("/components/schemas/Extended"),
            Some(&json!({
                "type": "object",
                "required": ["id", "secret"],
                "properties": {
                    "id": { "type": "string" },
                    "secret": { "type": "string" }
                }
            }))
        );
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
