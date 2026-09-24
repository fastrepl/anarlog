use crate::connected_mcp::{McpTarget, schema_property};
use crate::crm::{ClientRegistration, CrmProvider, MAX_CONTACT_RESULTS};
use crate::types::CrmContactQuery;
use rmcp::model::{JsonObject, Tool};
use serde_json::{Map, Value, json};

/// Attio stores person records under the `people` object.
const PEOPLE_OBJECT: &str = "people";

/// Attio's hosted MCP server supports OAuth dynamic client registration, so
/// connecting needs no user-supplied app credentials.
pub const PROVIDER: CrmProvider = CrmProvider {
    target: McpTarget {
        id: "attio",
        name: "Attio",
        endpoint: "https://mcp.attio.com/mcp",
    },
    client: ClientRegistration::Dynamic,
    search_tools: &["search-records", "search_records"],
    search_arguments: Some(search_arguments),
};

/// Attio needs its own builder because the generic one targets a `contacts`
/// object, which Attio rejects: people live under `people`, and the object
/// parameter's name and shape (slug vs. slug list) are only known from the
/// live tool schema.
pub fn search_arguments(tool: &Tool, query: &CrmContactQuery) -> Option<JsonObject> {
    let text = query.email.clone().or_else(|| query.name.clone())?;
    let mut arguments = Map::new();
    if let Some(property) = schema_property(
        tool,
        &[
            "objects",
            "object_slugs",
            "objectSlugs",
            "object",
            "object_slug",
            "objectSlug",
            "object_type",
            "objectType",
        ],
    ) {
        let value = if schema_is_array(tool, &property) {
            json!([PEOPLE_OBJECT])
        } else {
            json!(PEOPLE_OBJECT)
        };
        arguments.insert(property, value);
    }
    let query_property = schema_property(tool, &["query", "search", "q", "text"])
        .unwrap_or_else(|| "query".to_string());
    arguments.insert(query_property, Value::String(text));
    if let Some(property) = schema_property(tool, &["limit", "page_size", "pageSize"]) {
        arguments.insert(property, json!(MAX_CONTACT_RESULTS));
    }
    Some(arguments)
}

fn schema_is_array(tool: &Tool, property: &str) -> bool {
    let schema = tool
        .input_schema
        .get("properties")
        .and_then(|properties| properties.get(property));
    let Some(schema) = schema else {
        return property.ends_with('s');
    };
    match schema.get("type") {
        Some(Value::String(kind)) => kind == "array",
        Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind == "array"),
        _ => schema.get("items").is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn tool(schema: Value) -> Tool {
        Tool::new(
            "search-records",
            "search",
            Arc::new(schema.as_object().cloned().unwrap_or_default()),
        )
    }

    #[test]
    fn selects_people_in_a_slug_list() {
        let tool = tool(json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "objects": { "type": "array", "items": { "type": "string" } },
                "limit": { "type": "integer" }
            }
        }));
        let arguments = search_arguments(
            &tool,
            &CrmContactQuery {
                email: Some("ada@example.com".to_string()),
                name: Some("Ada Lovelace".to_string()),
            },
        )
        .unwrap();
        assert_eq!(arguments["objects"], json!(["people"]));
        assert_eq!(arguments["query"], json!("ada@example.com"));
        assert_eq!(arguments["limit"], json!(MAX_CONTACT_RESULTS));
    }

    #[test]
    fn selects_people_as_a_single_slug() {
        let tool = tool(json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "object": { "type": "string" }
            }
        }));
        let arguments = search_arguments(
            &tool,
            &CrmContactQuery {
                email: None,
                name: Some("Ada Lovelace".to_string()),
            },
        )
        .unwrap();
        assert_eq!(arguments["object"], json!("people"));
        assert_eq!(arguments["query"], json!("Ada Lovelace"));
        assert!(arguments.get("limit").is_none());
    }

    #[test]
    fn falls_back_to_query_without_a_schema() {
        let tool = tool(json!({ "type": "object" }));
        let arguments = search_arguments(
            &tool,
            &CrmContactQuery {
                email: Some("ada@example.com".to_string()),
                name: None,
            },
        )
        .unwrap();
        assert_eq!(
            arguments,
            json!({ "query": "ada@example.com" })
                .as_object()
                .cloned()
                .unwrap()
        );
    }

    #[test]
    fn requires_an_email_or_name() {
        let tool = tool(json!({ "type": "object" }));
        assert!(
            search_arguments(
                &tool,
                &CrmContactQuery {
                    email: None,
                    name: None
                }
            )
            .is_none()
        );
    }
}
