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
    fn sorts_top_level_keys_alphabetically() {
        let input = json!({"zebra": 1, "apple": 2, "mango": 3});
        let result = serialize(input).unwrap();

        assert!(result.find("apple").unwrap() < result.find("mango").unwrap());
        assert!(result.find("mango").unwrap() < result.find("zebra").unwrap());
    }

    #[test]
    fn sorts_nested_object_keys() {
        let input = json!({"outer": {"z": 1, "a": 2}});
        let result = serialize(input).unwrap();

        assert!(result.find("\"a\"").unwrap() < result.find("\"z\"").unwrap());
    }

    #[test]
    fn sorts_keys_in_array_objects() {
        let input = json!([{"b": 1, "a": 2}]);
        let result = serialize(input).unwrap();

        assert!(result.find("\"a\"").unwrap() < result.find("\"b\"").unwrap());
    }

    #[test]
    fn preserves_primitive_values() {
        let input = json!({"str": "hello", "num": 42, "bool": true, "null": null});
        let result = serialize(input).unwrap();

        assert!(result.contains("\"hello\""));
        assert!(result.contains("42"));
        assert!(result.contains("true"));
        assert!(result.contains("null"));
    }
}
