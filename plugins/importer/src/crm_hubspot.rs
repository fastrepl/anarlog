use crate::connected_mcp::McpTarget;
use crate::crm::{ClientRegistration, CrmProvider, MAX_CONTACT_RESULTS};
use crate::types::CrmContactQuery;
use rmcp::model::{JsonObject, Tool};
use serde_json::{Map, Value, json};

/// HubSpot MCP auth apps have no dynamic client registration, so the loopback
/// redirect must be stable to be registered on the app upfront.
pub const REDIRECT_PORT: u16 = 47691;

const CONTACT_PROPERTIES: &[&str] = &[
    "firstname",
    "lastname",
    "email",
    "company",
    "jobtitle",
    "phone",
    "mobilephone",
    "hs_linkedin_url",
];

pub const PROVIDER: CrmProvider = CrmProvider {
    target: McpTarget {
        id: "hubspot",
        name: "HubSpot",
        endpoint: "https://mcp.hubspot.com",
    },
    client: ClientRegistration::Preregistered {
        redirect_port: REDIRECT_PORT,
    },
    search_tools: &["search_crm_objects"],
    search_arguments: Some(search_arguments),
};

/// Builds a `search_crm_objects` call: an exact `email` filter when the
/// query has one, otherwise a free-text search on the name.
pub fn search_arguments(_tool: &Tool, query: &CrmContactQuery) -> Option<JsonObject> {
    let mut arguments = Map::new();
    arguments.insert("objectType".to_string(), json!("contacts"));
    arguments.insert("properties".to_string(), json!(CONTACT_PROPERTIES));
    arguments.insert("limit".to_string(), json!(MAX_CONTACT_RESULTS));
    match (&query.email, &query.name) {
        (Some(email), _) => {
            arguments.insert(
                "filterGroups".to_string(),
                json!([{
                    "filters": [{
                        "propertyName": "email",
                        "operator": "EQ",
                        "value": email.to_lowercase(),
                    }]
                }]),
            );
        }
        (None, Some(name)) => {
            arguments.insert("query".to_string(), Value::String(name.clone()));
        }
        (None, None) => return None,
    }
    Some(arguments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::Tool;
    use std::sync::Arc;

    fn tool() -> Tool {
        Tool::new(
            "search_crm_objects",
            "Search CRM records",
            Arc::new(Map::new()),
        )
    }

    #[test]
    fn filters_by_email_when_present() {
        let arguments = search_arguments(
            &tool(),
            &CrmContactQuery {
                email: Some("Ada@Example.com".to_string()),
                name: Some("Ada Lovelace".to_string()),
            },
        )
        .unwrap();
        assert_eq!(arguments["objectType"], json!("contacts"));
        assert_eq!(
            arguments["filterGroups"][0]["filters"][0]["value"],
            json!("ada@example.com")
        );
        assert!(arguments.get("query").is_none());
        assert!(
            arguments["properties"]
                .as_array()
                .unwrap()
                .contains(&json!("hs_linkedin_url"))
        );
    }

    #[test]
    fn falls_back_to_text_query_on_name() {
        let arguments = search_arguments(
            &tool(),
            &CrmContactQuery {
                email: None,
                name: Some("Ada Lovelace".to_string()),
            },
        )
        .unwrap();
        assert_eq!(arguments["query"], json!("Ada Lovelace"));
        assert!(arguments.get("filterGroups").is_none());
    }

    #[test]
    fn requires_email_or_name() {
        assert!(
            search_arguments(
                &tool(),
                &CrmContactQuery {
                    email: None,
                    name: None
                }
            )
            .is_none()
        );
    }
}
