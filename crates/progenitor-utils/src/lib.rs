mod allof;
mod refs;

use std::path::Path;

use serde_json::Value;

pub struct OpenApiSpec {
    inner: Value,
}

impl OpenApiSpec {
    pub fn from_path(path: &str) -> Self {
        let raw = std::fs::read_to_string(path).expect("failed to read OpenAPI spec");
        let inner: Value = serde_json::from_str(&raw).expect("invalid JSON");
        Self { inner }
    }

    pub fn retain_paths(&mut self, prefixes: &[&str]) -> &mut Self {
        let Some(paths) = self.inner.get_mut("paths").and_then(Value::as_object_mut) else {
            return self;
        };
        paths.retain(|key, _| prefixes.iter().any(|prefix| key.starts_with(prefix)));
        self
    }

    /// Progenitor requires at most one typed response per operation.
    /// - Strip content bodies from 204 responses (shouldn't carry a body).
    /// - Strip content bodies from all error (non-2xx) responses so that
    ///   inconsistent error schemas don't trip the assertion.
    pub fn normalize_responses(&mut self) -> &mut Self {
        for_each_operation(&mut self.inner, |op| {
            let Some(responses) = op.get_mut("responses").and_then(Value::as_object_mut) else {
                return;
            };
            for (code, resp) in responses.iter_mut() {
                let dominated = code == "204" || !code.starts_with('2');
                if dominated && let Some(obj) = resp.as_object_mut() {
                    obj.remove("content");
                }
            }
        });
        self
    }

    /// Progenitor cannot handle `allOf` compositions (it panics with
    /// "response_types.len() <= 1"). Walk the entire tree and replace every
    /// `allOf` node with its last `$ref` member, which is typically the primary
    /// type (the first tends to be `generic_id` or similar).
    pub fn flatten_all_of(&mut self) -> &mut Self {
        allof::flatten_all_of_value(&mut self.inner);
        self
    }

    pub fn remove_unreferenced_schemas(&mut self) -> &mut Self {
        let referenced = refs::transitively_referenced_schemas(&self.inner);
        let Some(schemas) = self
            .inner
            .pointer_mut("/components/schemas")
            .and_then(Value::as_object_mut)
        else {
            return self;
        };
        schemas.retain(|key, _| referenced.contains(key));
        self
    }

    pub fn write_filtered(&self, path: impl AsRef<Path>) -> &Self {
        let json = serde_json::to_string_pretty(&self.inner).unwrap();
        std::fs::write(path, json).unwrap();
        self
    }

    pub fn convert_31_to_30(&mut self) -> &mut Self {
        let json = serde_json::to_string(&self.inner).expect("serialize to JSON");
        let yaml_30 = openapi31to30::convert(&json).expect("3.1 -> 3.0 conversion failed");
        self.inner = serde_json::to_value(
            serde_yaml::from_str::<serde_yaml::Value>(&yaml_30).expect("parse converted YAML"),
        )
        .expect("convert YAML value to JSON value");
        self
    }

    pub fn generate(&self, filename: &str) {
        self.generate_with_replacements(filename, &[]);
    }

    pub fn generate_with_replacements(&self, filename: &str, replacements: &[(&str, &str)]) {
        let openapi: openapiv3::OpenAPI =
            serde_json::from_value(self.inner.clone()).expect("filtered spec is not valid OpenAPI");

        let mut settings = progenitor::GenerationSettings::new();
        for (type_name, replace_name) in replacements {
            settings.with_replacement(*type_name, *replace_name, std::iter::empty());
        }

        let tokens = progenitor::Generator::new(&settings)
            .generate_tokens(&openapi)
            .expect("progenitor code generation failed");

        let ast = syn::parse2(tokens).expect("generated code failed to parse");
        let content = prettyplease::unparse(&ast);
        let out_path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join(filename);
        std::fs::write(&out_path, content).unwrap();
    }
}

fn for_each_operation(spec: &mut Value, mut f: impl FnMut(&mut Value)) {
    let Some(paths) = spec.get_mut("paths").and_then(Value::as_object_mut) else {
        return;
    };
    for item in paths.values_mut() {
        let Some(item) = item.as_object_mut() else {
            continue;
        };
        for method in ["get", "post", "put", "patch", "delete"] {
            if let Some(op) = item.get_mut(method) {
                f(op);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn retain_paths_keeps_matching_prefixes() {
        let mut spec = OpenApiSpec {
            inner: json!({
                "paths": {
                    "/api/v1/users": {},
                    "/api/v1/orders": {},
                    "/api/v2/users": {},
                    "/internal/health": {}
                }
            }),
        };
        spec.retain_paths(&["/api/v1"]);

        let paths = spec.inner["paths"].as_object().unwrap();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains_key("/api/v1/users"));
        assert!(paths.contains_key("/api/v1/orders"));
    }

    #[test]
    fn normalize_responses_strips_204_and_error_content() {
        let mut spec = OpenApiSpec {
            inner: json!({
                "paths": {
                    "/test": {
                        "delete": {
                            "responses": {
                                "200": { "content": { "application/json": {} } },
                                "204": { "content": { "application/json": {} } },
                                "400": { "content": { "application/json": {} } },
                                "500": { "content": { "application/json": {} } }
                            }
                        }
                    }
                }
            }),
        };
        spec.normalize_responses();

        let responses = &spec.inner["paths"]["/test"]["delete"]["responses"];
        assert!(responses["200"].get("content").is_some());
        assert!(responses["204"].get("content").is_none());
        assert!(responses["400"].get("content").is_none());
        assert!(responses["500"].get("content").is_none());
    }

    #[test]
    fn for_each_operation_visits_all_methods() {
        let mut spec = json!({
            "paths": {
                "/a": { "get": { "id": "a_get" }, "post": { "id": "a_post" } },
                "/b": { "delete": { "id": "b_delete" } }
            }
        });

        let mut visited = vec![];
        for_each_operation(&mut spec, |op| {
            if let Some(id) = op.get("id").and_then(Value::as_str) {
                visited.push(id.to_string());
            }
        });

        visited.sort();
        assert_eq!(visited, vec!["a_get", "a_post", "b_delete"]);
    }

    #[test]
    fn remove_unreferenced_schemas_prunes_unused() {
        let mut spec = OpenApiSpec {
            inner: json!({
                "paths": {
                    "/items": {
                        "get": {
                            "responses": {
                                "200": {
                                    "content": {
                                        "application/json": {
                                            "schema": { "$ref": "#/components/schemas/Item" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                "components": {
                    "schemas": {
                        "Item": { "type": "object" },
                        "Unused": { "type": "string" }
                    }
                }
            }),
        };
        spec.remove_unreferenced_schemas();

        let schemas = spec.inner["components"]["schemas"].as_object().unwrap();
        assert!(schemas.contains_key("Item"));
        assert!(!schemas.contains_key("Unused"));
    }
}
