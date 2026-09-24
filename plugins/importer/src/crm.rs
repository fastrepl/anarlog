use crate::connected_mcp::{
    self, AuthorizedService, ConnectedImportOAuthState, McpTarget, PreregisteredClient, call_tool,
    credentials_from_manager, find_tool, loopback_redirect_uri, schema_property,
};
use crate::types::{
    ConnectedImportAuthorization, ConnectedImportCredentials, CrmClientInput, CrmContact,
    CrmContactLookupResult, CrmContactQuery, CrmProviderInfo,
};
use rmcp::model::{JsonObject, Tool};
use serde_json::{Map, Value};

pub const MAX_CONTACT_RESULTS: usize = 10;

#[derive(Clone, Copy)]
pub enum ClientRegistration {
    /// The server supports OAuth dynamic client registration.
    Dynamic,
    /// The user brings an OAuth client from their CRM account; the loopback
    /// redirect must use a stable port so it can be registered upfront.
    Preregistered { redirect_port: u16 },
}

pub type SearchArguments = fn(&Tool, &CrmContactQuery) -> Option<JsonObject>;

#[derive(Clone, Copy)]
pub struct CrmProvider {
    pub target: McpTarget,
    pub client: ClientRegistration,
    pub search_tools: &'static [&'static str],
    pub search_arguments: Option<SearchArguments>,
}

pub const CRM_PROVIDERS: &[CrmProvider] =
    &[crate::crm_hubspot::PROVIDER, crate::crm_attio::PROVIDER];

impl CrmProvider {
    fn info(&self) -> CrmProviderInfo {
        let redirect_uri = match self.client {
            ClientRegistration::Dynamic => None,
            ClientRegistration::Preregistered { redirect_port } => {
                Some(loopback_redirect_uri(redirect_port))
            }
        };
        CrmProviderInfo {
            id: self.target.id.to_string(),
            name: self.target.name.to_string(),
            requires_client: redirect_uri.is_some(),
            redirect_uri,
        }
    }

    fn reconnect_message(&self) -> String {
        format!("Reconnect {} to look up contacts", self.target.name)
    }
}

fn provider(provider_id: &str) -> Result<CrmProvider, String> {
    CRM_PROVIDERS
        .iter()
        .copied()
        .find(|provider| provider.target.id == provider_id)
        .ok_or_else(|| "This CRM is not supported".to_string())
}

pub fn providers() -> Vec<CrmProviderInfo> {
    CRM_PROVIDERS.iter().map(CrmProvider::info).collect()
}

pub async fn begin_connection(
    provider_id: &str,
    client: Option<CrmClientInput>,
    state: &ConnectedImportOAuthState,
) -> Result<ConnectedImportAuthorization, String> {
    let provider = provider(provider_id)?;
    let client = match provider.client {
        ClientRegistration::Dynamic => None,
        ClientRegistration::Preregistered { redirect_port } => {
            let client = client
                .filter(|client| !client.client_id.trim().is_empty())
                .ok_or_else(|| {
                    format!(
                        "Enter the client ID of your {} app to connect",
                        provider.target.name
                    )
                })?;
            Some(PreregisteredClient {
                client_id: client.client_id.trim().to_string(),
                client_secret: client
                    .client_secret
                    .map(|secret| secret.trim().to_string())
                    .filter(|secret| !secret.is_empty()),
                redirect_port,
            })
        }
    };
    connected_mcp::begin_authorization(provider.target, client, state).await
}

pub async fn cancel_connection(
    provider_id: &str,
    state: &ConnectedImportOAuthState,
) -> Result<bool, String> {
    connected_mcp::cancel_authorization(provider(provider_id)?.target, state).await
}

pub async fn complete_connection(
    provider_id: &str,
    state: &ConnectedImportOAuthState,
) -> Result<ConnectedImportCredentials, String> {
    connected_mcp::complete_authorization(provider(provider_id)?.target, state).await
}

/// Confirms stored credentials still work and returns them, refreshed when
/// the access token was renewed.
pub async fn verify_connection(
    provider_id: &str,
    credentials: ConnectedImportCredentials,
) -> Result<ConnectedImportCredentials, String> {
    let provider = provider(provider_id)?;
    let AuthorizedService {
        service,
        manager,
        token_received_at,
        ..
    } = connected_mcp::connect_authorized(provider.target, &credentials, || {
        provider.reconnect_message()
    })
    .await?;
    let refreshed = credentials_from_manager(
        provider.target,
        &manager,
        credentials.client_secret,
        token_received_at,
    )
    .await;
    let _ = service.cancel().await;
    refreshed
}

pub async fn lookup_contacts(
    provider_id: &str,
    credentials: ConnectedImportCredentials,
    query: CrmContactQuery,
) -> Result<CrmContactLookupResult, String> {
    let provider = provider(provider_id)?;
    let query = CrmContactQuery {
        email: normalized_query_value(query.email),
        name: normalized_query_value(query.name),
    };
    if query.email.is_none() && query.name.is_none() {
        return Err("Add an email or name to the contact first".to_string());
    }

    let AuthorizedService {
        service,
        tools,
        manager,
        token_received_at,
    } = connected_mcp::connect_authorized(provider.target, &credentials, || {
        provider.reconnect_message()
    })
    .await?;
    let tool = find_tool(&tools, provider.search_tools).ok_or_else(|| {
        format!(
            "{} did not offer a supported contact search tool",
            provider.target.name
        )
    })?;
    let arguments = provider
        .search_arguments
        .and_then(|build| build(tool, &query))
        .unwrap_or_else(|| default_search_arguments(tool, &query));
    let payloads = call_tool(provider.target, service.peer(), tool, arguments).await?;
    let contacts = matching_contacts(&payloads, &query);

    let refreshed = credentials_from_manager(
        provider.target,
        &manager,
        credentials.client_secret,
        token_received_at,
    )
    .await?;
    let _ = service.cancel().await;
    Ok(CrmContactLookupResult {
        contacts,
        credentials: refreshed,
    })
}

fn normalized_query_value(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn default_search_arguments(tool: &Tool, query: &CrmContactQuery) -> JsonObject {
    let mut arguments = Map::new();
    let text = query
        .email
        .clone()
        .or_else(|| query.name.clone())
        .unwrap_or_default();
    if let Some(property) = schema_property(
        tool,
        &["objectType", "object_type", "objectName", "object", "type"],
    ) {
        arguments.insert(property, Value::String("contacts".to_string()));
    }
    if let Some(property) = schema_property(
        tool,
        &[
            "query",
            "search",
            "q",
            "text",
            "keyword",
            "searchQuery",
            "search_query",
        ],
    ) {
        arguments.insert(property, Value::String(text));
    }
    if let (Some(property), Some(email)) = (schema_property(tool, &["email"]), &query.email) {
        arguments.insert(property, Value::String(email.clone()));
    }
    if let (Some(property), Some(name)) = (schema_property(tool, &["name"]), &query.name) {
        arguments.insert(property, Value::String(name.clone()));
    }
    if let Some(property) = schema_property(tool, &["limit", "page_size", "pageSize"]) {
        arguments.insert(property, Value::Number(MAX_CONTACT_RESULTS.into()));
    }
    arguments
}

/// Keeps records that match the query, preferring exact email matches over
/// name matches so enrichment never picks an unrelated person.
pub fn matching_contacts(payloads: &[Value], query: &CrmContactQuery) -> Vec<CrmContact> {
    let contacts = extract_contacts(payloads);
    if let Some(email) = &query.email {
        let email = email.to_lowercase();
        let by_email = contacts
            .iter()
            .filter(|contact| {
                contact
                    .email
                    .as_deref()
                    .is_some_and(|candidate| candidate.to_lowercase() == email)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !by_email.is_empty() {
            return by_email;
        }
    }
    if let Some(name) = &query.name {
        let name = normalized_name(name);
        return contacts
            .into_iter()
            .filter(|contact| {
                contact
                    .name
                    .as_deref()
                    .is_some_and(|candidate| normalized_name(candidate) == name)
            })
            .take(MAX_CONTACT_RESULTS)
            .collect();
    }
    Vec::new()
}

fn normalized_name(name: &str) -> String {
    name.split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn extract_contacts(payloads: &[Value]) -> Vec<CrmContact> {
    let mut contacts = Vec::new();
    for payload in payloads {
        collect_contacts(payload, &mut contacts);
    }
    contacts
}

fn collect_contacts(value: &Value, contacts: &mut Vec<CrmContact>) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_contacts(value, contacts);
            }
        }
        Value::Object(record) => {
            if let Some(contact) = contact_from_record(record) {
                if !contacts.contains(&contact) {
                    contacts.push(contact);
                }
                return;
            }
            for value in record.values() {
                collect_contacts(value, contacts);
            }
        }
        _ => {}
    }
}

const PROPERTY_BAGS: &[&str] = &["properties", "values", "attributes", "fields", "data"];

fn contact_from_record(record: &Map<String, Value>) -> Option<CrmContact> {
    let mut fields = Map::new();
    for (key, value) in record {
        if PROPERTY_BAGS.contains(&key.as_str()) {
            if let Value::Object(bag) = value {
                for (key, value) in bag {
                    fields.insert(key.clone(), value.clone());
                }
            }
        } else {
            fields.insert(key.clone(), value.clone());
        }
    }

    let email = field(
        &fields,
        &[
            "email",
            "email_address",
            "emailAddress",
            "primary_email",
            "primaryEmail",
            "work_email",
            "email_addresses",
            "emails",
        ],
    )
    .filter(|value| value.contains('@'));
    let name = field(&fields, &["name", "full_name", "fullName", "display_name"]).or_else(|| {
        let first = field(&fields, &["firstname", "first_name", "firstName"]);
        let last = field(&fields, &["lastname", "last_name", "lastName"]);
        match (first, last) {
            (None, None) => None,
            (first, last) => Some(
                [first, last]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
        }
    });
    let id = match fields
        .get("id")
        .or_else(|| fields.get("record_id"))
        .or_else(|| fields.get("contact_id"))
    {
        Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(Value::Number(value)) => Some(value.to_string()),
        _ => None,
    };
    if email.is_none() && (name.is_none() || id.is_none()) {
        return None;
    }

    let linkedin_url = field(
        &fields,
        &[
            "hs_linkedin_url",
            "linkedin_url",
            "linkedinUrl",
            "linkedin",
            "linkedinbio",
            "linkedin_bio",
        ],
    )
    .filter(|value| value.contains("linkedin.com"));
    Some(CrmContact {
        id,
        name,
        email,
        company_name: field(
            &fields,
            &[
                "company",
                "company_name",
                "companyName",
                "organization",
                "organisation",
                "employer",
            ],
        ),
        job_title: field(
            &fields,
            &[
                "jobtitle",
                "job_title",
                "jobTitle",
                "title",
                "position",
                "role",
            ],
        ),
        phone: field(
            &fields,
            &[
                "phone",
                "phone_number",
                "phoneNumber",
                "mobilephone",
                "mobile",
                "phone_numbers",
            ],
        ),
        linkedin_url,
        url: field(
            &fields,
            &["url", "record_url", "recordUrl", "web_url", "webUrl"],
        )
        .filter(|value| value.starts_with("http") && !value.contains('{')),
    })
}

fn field(fields: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| fields.get(*key).and_then(string_value))
}

/// Reads a scalar, or the first value of a list, or the primary value of an
/// attribute object such as `{ "value": "..." }` / `{ "email_address": "..." }`.
fn string_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Number(value) => Some(value.to_string()),
        Value::Array(values) => values.iter().find_map(string_value),
        Value::Object(object) => [
            "value",
            "email_address",
            "full_name",
            "name",
            "original_phone_number",
            "phone_number",
            "url",
        ]
        .iter()
        .find_map(|key| object.get(*key).and_then(string_value)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn query(email: Option<&str>, name: Option<&str>) -> CrmContactQuery {
        CrmContactQuery {
            email: email.map(str::to_string),
            name: name.map(str::to_string),
        }
    }

    #[test]
    fn extracts_hubspot_style_records() {
        let payload = json!({
            "total": 1,
            "results": [{
                "id": "501",
                "properties": {
                    "email": "Ada@Example.com",
                    "firstname": "Ada",
                    "lastname": "Lovelace",
                    "jobtitle": "Engineer",
                    "company": "Analytical Engines",
                    "phone": "+44 20 7946 0000",
                    "hs_linkedin_url": "https://linkedin.com/in/ada"
                },
                "urlTemplate": "https://app.hubspot.com/contacts/{portalId}/record/0-1/{id}"
            }]
        });

        let contacts = matching_contacts(&[payload], &query(Some("ada@example.com"), None));
        assert_eq!(
            contacts,
            vec![CrmContact {
                id: Some("501".to_string()),
                name: Some("Ada Lovelace".to_string()),
                email: Some("Ada@Example.com".to_string()),
                company_name: Some("Analytical Engines".to_string()),
                job_title: Some("Engineer".to_string()),
                phone: Some("+44 20 7946 0000".to_string()),
                linkedin_url: Some("https://linkedin.com/in/ada".to_string()),
                url: None,
            }]
        );
    }

    #[test]
    fn extracts_attribute_list_records() {
        let payload = json!([{
            "id": { "record_id": "rec_1" },
            "values": {
                "name": [{ "full_name": "Grace Hopper" }],
                "email_addresses": [{ "email_address": "grace@navy.mil" }],
                "job_title": [{ "value": "Rear Admiral" }],
                "linkedin": [{ "value": "https://www.linkedin.com/in/grace" }]
            },
            "web_url": "https://app.attio.com/w/people/rec_1"
        }]);

        let contacts = extract_contacts(&[payload]);
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].name.as_deref(), Some("Grace Hopper"));
        assert_eq!(contacts[0].email.as_deref(), Some("grace@navy.mil"));
        assert_eq!(contacts[0].job_title.as_deref(), Some("Rear Admiral"));
        assert_eq!(
            contacts[0].url.as_deref(),
            Some("https://app.attio.com/w/people/rec_1")
        );
    }

    #[test]
    fn falls_back_to_name_match_and_drops_strangers() {
        let payload = json!({ "results": [
            { "id": 1, "properties": { "email": "a@x.io", "firstname": "Alan", "lastname": "Turing" } },
            { "id": 2, "properties": { "email": "b@x.io", "firstname": "Alonzo", "lastname": "Church" } }
        ]});
        let payloads = std::slice::from_ref(&payload);

        let by_name = matching_contacts(payloads, &query(None, Some("  alan   TURING ")));
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].email.as_deref(), Some("a@x.io"));

        let by_email = matching_contacts(payloads, &query(Some("B@x.io"), Some("Alan Turing")));
        assert_eq!(by_email.len(), 1);
        assert_eq!(by_email[0].id.as_deref(), Some("2"));

        assert!(matching_contacts(payloads, &query(Some("nobody@x.io"), None)).is_empty());
    }

    #[test]
    fn ignores_records_without_identity() {
        let payload = json!({ "results": [{ "properties": { "company": "Acme" } }] });
        assert!(extract_contacts(&[payload]).is_empty());
    }

    #[test]
    fn default_arguments_follow_the_tool_schema() {
        let tool = Tool::new(
            "search_records",
            "",
            serde_json::from_value::<JsonObject>(json!({
                "type": "object",
                "properties": {
                    "objectType": { "type": "string" },
                    "query": { "type": "string" },
                    "limit": { "type": "number" }
                }
            }))
            .unwrap(),
        );
        let arguments =
            default_search_arguments(&tool, &query(Some("ada@example.com"), Some("Ada")));
        assert_eq!(arguments["objectType"], "contacts");
        assert_eq!(arguments["query"], "ada@example.com");
        assert_eq!(arguments["limit"], 10);
    }
}
