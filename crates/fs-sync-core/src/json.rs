use serde_json::Value;
use std::collections::BTreeMap;

fn sort_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<String, Value> =
                map.into_iter().map(|(k, v)| (k, sort_keys(v))).collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(sort_keys).collect()),
        other => other,
    }
}

pub fn serialize(json: Value) -> Result<String, String> {
    let sorted = sort_keys(json);
    serde_json::to_string_pretty(&sorted).map_err(|e| format!("JSON serialize: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_sorts_keys_recursively_and_preserves_values() {
        let input = json!({"z": {"b": 1, "a": [{"d": true, "c": null}]}, "m": "s", "a": 4.5});
        let result = serialize(input).unwrap();

        assert_eq!(
            result,
            "{\n  \"a\": 4.5,\n  \"m\": \"s\",\n  \"z\": {\n    \"a\": [\n      {\n        \"c\": null,\n        \"d\": true\n      }\n    ],\n    \"b\": 1\n  }\n}"
        );
    }
}
