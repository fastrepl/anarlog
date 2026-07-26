use std::collections::{BTreeSet, VecDeque};

use serde_json::Value;

/// BFS from every `$ref` found under `paths`, expanding through schema
/// definitions until the closure is complete.
pub(crate) fn transitively_referenced_schemas(spec: &Value) -> BTreeSet<String> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();

    if let Some(paths) = spec.get("paths") {
        collect_schema_refs(paths, &mut queue);
    }

    let schemas = spec
        .pointer("/components/schemas")
        .and_then(Value::as_object);

    while let Some(name) = queue.pop_front() {
        if !visited.insert(name.clone()) {
            continue;
        }
        if let Some(schema) = schemas.and_then(|s| s.get(&name)) {
            let mut nested = VecDeque::new();
            collect_schema_refs(schema, &mut nested);
            queue.extend(nested.into_iter().filter(|n| !visited.contains(n)));
        }
    }

    visited
}

fn collect_schema_refs(value: &Value, out: &mut VecDeque<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref")
                && let Some(name) = r.strip_prefix("#/components/schemas/")
            {
                out.push_back(name.to_string());
            }
            for v in map.values() {
                collect_schema_refs(v, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_schema_refs(v, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn collects_direct_refs_from_paths() {
        let spec = json!({
            "paths": {
                "/users": {
                    "get": {
                        "responses": {
                            "200": {
                                "content": {
                                    "application/json": {
                                        "schema": { "$ref": "#/components/schemas/User" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "components": {
                "schemas": {
                    "User": { "type": "object" }
                }
            }
        });

        let result = transitively_referenced_schemas(&spec);
        assert_eq!(result, BTreeSet::from(["User".to_string()]));
    }

    #[test]
    fn follows_transitive_refs() {
        let spec = json!({
            "paths": {
                "/orders": {
                    "get": {
                        "responses": {
                            "200": {
                                "content": {
                                    "application/json": {
                                        "schema": { "$ref": "#/components/schemas/Order" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "components": {
                "schemas": {
                    "Order": {
                        "type": "object",
                        "properties": {
                            "item": { "$ref": "#/components/schemas/Item" }
                        }
                    },
                    "Item": { "type": "object" },
                    "Unrelated": { "type": "string" }
                }
            }
        });

        let result = transitively_referenced_schemas(&spec);
        assert_eq!(
            result,
            BTreeSet::from(["Order".to_string(), "Item".to_string()])
        );
        assert!(!result.contains("Unrelated"));
    }

    #[test]
    fn returns_empty_when_no_paths() {
        let spec = json!({
            "components": {
                "schemas": {
                    "Orphan": { "type": "object" }
                }
            }
        });

        let result = transitively_referenced_schemas(&spec);
        assert!(result.is_empty());
    }

    #[test]
    fn handles_missing_components() {
        let spec = json!({
            "paths": {
                "/test": {
                    "get": {
                        "responses": {
                            "200": {
                                "content": {
                                    "application/json": {
                                        "schema": { "$ref": "#/components/schemas/Missing" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        let result = transitively_referenced_schemas(&spec);
        assert_eq!(result, BTreeSet::from(["Missing".to_string()]));
    }
}
