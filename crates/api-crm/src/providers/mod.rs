mod attio;
mod close;
mod hubspot;
mod pipedrive;
mod salesforce;

use anlg_api_nango::is_provider_auth_failure;
use anlg_nango::OwnedNangoHttpClient;
use futures_util::future::BoxFuture;
use serde_json::Value;

use crate::contacts::{
    CrmContact, CrmContactContext, CrmContactQuery, CrmLoggedNote, CrmNoteInput,
};
use crate::error::{CrmError, Result};

pub const MAX_CONTACT_RESULTS: usize = 5;
const MAX_NOTE_CHARS: usize = 30_000;

pub type SearchFn =
    fn(OwnedNangoHttpClient, CrmContactQuery, usize) -> BoxFuture<'static, Result<Vec<CrmContact>>>;
pub type ContextFn =
    fn(OwnedNangoHttpClient, CrmContactQuery, usize) -> BoxFuture<'static, Result<CrmContactContext>>;
pub type LogNoteFn =
    fn(OwnedNangoHttpClient, CrmNoteInput) -> BoxFuture<'static, Result<CrmLoggedNote>>;

/// A CRM reachable through a Nango integration. `nango_integration_id` must
/// match the integration configured in the Nango dashboard; `search` and
/// `context` query the CRM via the Nango proxy, and `log_note` attaches a
/// meeting note/activity to a matched contact.
pub struct CrmProvider {
    pub id: &'static str,
    pub name: &'static str,
    pub nango_integration_id: &'static str,
    pub search: SearchFn,
    pub context: ContextFn,
    pub log_note: LogNoteFn,
}

pub fn resolve(provider_id: &str) -> Option<&'static CrmProvider> {
    PROVIDERS
        .iter()
        .find(|provider| provider.id == provider_id)
        .copied()
}

pub static PROVIDERS: &[&CrmProvider] = &[
    &hubspot::PROVIDER,
    &attio::PROVIDER,
    &salesforce::PROVIDER,
    &pipedrive::PROVIDER,
    &close::PROVIDER,
];

pub(super) async fn send_json(builder: reqwest::RequestBuilder) -> Result<Value> {
    let response = builder
        .send()
        .await
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    if !status.is_success() {
        let body = String::from_utf8_lossy(&bytes);
        let snippet: String = body.chars().take(512).collect();
        return Err(CrmError::Provider(format!(
            "API error (status {}): {}",
            status.as_u16(),
            snippet
        )));
    }
    if bytes.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&bytes).map_err(|e| CrmError::Provider(e.to_string()))
}

pub(super) fn keep_optional_result<T: Default>(res: Result<T>) -> Result<T> {
    match res {
        Ok(value) => Ok(value),
        Err(CrmError::Provider(message)) if is_provider_auth_failure(&message) => {
            Err(CrmError::Provider(message))
        }
        Err(_) => Ok(T::default()),
    }
}

pub(super) fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let mut out: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        out.push('…');
    }
    out
}

pub(super) fn format_note_text(title: &str, body: &str) -> String {
    let title = title.trim();
    let body = truncate_chars(body.trim(), MAX_NOTE_CHARS);
    match (title.is_empty(), body.is_empty()) {
        (true, true) => String::new(),
        (false, true) => title.to_string(),
        (true, false) => body,
        (false, false) => format!("{title}\n\n{body}"),
    }
}

pub(super) fn date_prefix(occurred_at: Option<&str>) -> Option<&str> {
    let raw = occurred_at?.trim();
    let date = raw.get(..10)?;
    let bytes = date.as_bytes();
    let valid = bytes.len() == 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit);
    valid.then_some(date)
}

pub(super) fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => {
                in_tag = true;
                if !out.ends_with([' ', '\n']) {
                    out.push(' ');
                }
            }
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn unix_millis_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_nango::{NangoClient, OwnedNangoProxy};
    use serde_json::json;
    use wiremock::matchers::{body_json, body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_http(server: &MockServer, integration: &str) -> OwnedNangoHttpClient {
        let client = NangoClient::builder()
            .api_key("test-key")
            .api_base(server.uri())
            .build()
            .unwrap();
        OwnedNangoHttpClient::new(OwnedNangoProxy::new(
            &client,
            integration.to_string(),
            "conn-1".to_string(),
        ))
    }

    #[tokio::test]
    async fn hubspot_context_returns_deals_and_survives_notes_failure() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/proxy/crm/v3/objects/contacts/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "results": [{
                    "id": "101",
                    "properties": {
                        "firstname": "Ada",
                        "lastname": "Lovelace",
                        "email": "ada@acme.com",
                        "company": "Acme",
                        "jobtitle": "CTO",
                    }
                }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/proxy/crm/v4/objects/contacts/101/associations/deals"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "results": [{"toObjectId": 501}]
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/proxy/crm/v3/objects/deals/batch/read"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "results": [{
                    "id": "501",
                    "properties": {
                        "dealname": "Acme Enterprise",
                        "dealstage": "qualifiedtobuy",
                        "amount": "24000",
                        "closedate": "2026-12-31"
                    }
                }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/proxy/crm/v4/objects/contacts/101/associations/notes"))
            .respond_with(ResponseTemplate::new(500).set_body_string("temporary error"))
            .mount(&server)
            .await;

        let ctx = (hubspot::PROVIDER.context)(
            test_http(&server, "hubspot"),
            CrmContactQuery {
                email: Some("ada@acme.com".to_string()),
                name: None,
            },
            5,
        )
        .await
        .unwrap();

        assert_eq!(
            ctx.contact.as_ref().and_then(|c| c.name.as_deref()),
            Some("Ada Lovelace")
        );
        assert_eq!(ctx.deals.len(), 1);
        assert_eq!(ctx.deals[0].name.as_deref(), Some("Acme Enterprise"));
        assert!(ctx.notes.is_empty());
    }

    #[tokio::test]
    async fn hubspot_log_note_associates_contact_deal_and_company() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/proxy/crm/v3/objects/contacts/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "results": [{
                    "id": "101",
                    "properties": {"firstname": "Ada", "lastname": "Lovelace", "email": "ada@acme.com"}
                }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/proxy/crm/v4/objects/contacts/101/associations/deals"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "results": [{"toObjectId": 501}]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/proxy/crm/v4/objects/contacts/101/associations/companies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "results": [{"toObjectId": 701}]
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/proxy/crm/v3/objects/notes"))
            .and(body_string_contains("\"associationTypeId\":202"))
            .and(body_string_contains("\"associationTypeId\":214"))
            .and(body_string_contains("\"associationTypeId\":190"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({"id": "note-9"})))
            .expect(1)
            .mount(&server)
            .await;

        let logged = (hubspot::PROVIDER.log_note)(
            test_http(&server, "hubspot"),
            CrmNoteInput {
                contact_id: None,
                query: CrmContactQuery {
                    email: Some("ada@acme.com".to_string()),
                    name: None,
                },
                title: "Q4 Sync".to_string(),
                body: "Agreed on pilot scope.".to_string(),
                occurred_at: Some("2026-11-25T10:00:00Z".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(logged.note_id.as_deref(), Some("note-9"));
        assert_eq!(logged.contact_id, "101");
        assert_eq!(logged.deal_id.as_deref(), Some("501"));
    }

    #[tokio::test]
    async fn attio_context_and_log_note_work() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/proxy/v2/objects/people/records/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "id": {"record_id": "rec_1"},
                    "values": {
                        "name": [{"full_name": "Ada Lovelace"}],
                        "email_addresses": [{"email_address": "ada@acme.com"}]
                    }
                }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/proxy/v2/notes"))
            .and(query_param("parent_object", "people"))
            .and(query_param("parent_record_id", "rec_1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "id": {"note_id": "n_1"},
                    "title": "Intro",
                    "content_plaintext": "Wants SOC2 report",
                    "created_at": "2026-11-20T09:00:00Z"
                }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/proxy/v2/objects/deals/records/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "id": {"record_id": "deal_1"},
                    "values": {
                        "name": [{"value": "Annual Plan"}],
                        "stage": [{"status": {"title": "Evaluation"}}],
                        "value": [{"currency_value": 12000}]
                    }
                }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/proxy/v2/notes"))
            .and(body_json(json!({
                "data": {
                    "parent_object": "people",
                    "parent_record_id": "rec_1",
                    "title": "Demo Call",
                    "format": "markdown",
                    "content": "Send contract"
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {"id": {"note_id": "n_2"}}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let ctx = (attio::PROVIDER.context)(
            test_http(&server, "attio"),
            CrmContactQuery {
                email: Some("ada@acme.com".to_string()),
                name: None,
            },
            5,
        )
        .await
        .unwrap();
        assert_eq!(ctx.notes.len(), 1);
        assert_eq!(ctx.deals.len(), 1);
        assert_eq!(ctx.deals[0].stage.as_deref(), Some("Evaluation"));

        let logged = (attio::PROVIDER.log_note)(
            test_http(&server, "attio"),
            CrmNoteInput {
                contact_id: None,
                query: CrmContactQuery {
                    email: Some("ada@acme.com".to_string()),
                    name: None,
                },
                title: "Demo Call".to_string(),
                body: "Send contract".to_string(),
                occurred_at: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(logged.note_id.as_deref(), Some("n_2"));
    }

    #[tokio::test]
    async fn salesforce_log_note_returns_not_found_when_unmatched() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/proxy/services/data/v65.0/query"))
            .and(header("Provider-Config-Key", "salesforce"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"records": []})))
            .mount(&server)
            .await;

        let err = (salesforce::PROVIDER.log_note)(
            test_http(&server, "salesforce"),
            CrmNoteInput {
                contact_id: None,
                query: CrmContactQuery {
                    email: Some("nobody@acme.com".to_string()),
                    name: None,
                },
                title: "Sync".to_string(),
                body: "Summary".to_string(),
                occurred_at: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, CrmError::NotFound(_)));
    }

    #[tokio::test]
    async fn pipedrive_and_close_log_note_work() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/proxy/v1/persons/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "items": [{
                        "item": {
                            "id": 77,
                            "name": "Ada Lovelace",
                            "primary_email": "ada@acme.com",
                            "organization": {"id": 9, "name": "Acme"}
                        }
                    }]
                }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/proxy/v1/persons/77/deals"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{"id": 33, "title": "Pilot", "status": "open"}]
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/proxy/v1/notes"))
            .and(body_string_contains("\"person_id\":77"))
            .and(body_string_contains("\"deal_id\":33"))
            .and(body_string_contains("\"org_id\":9"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({"data": {"id": 500}})))
            .mount(&server)
            .await;

        let pipedrive_logged = (pipedrive::PROVIDER.log_note)(
            test_http(&server, "pipedrive"),
            CrmNoteInput {
                contact_id: None,
                query: CrmContactQuery {
                    email: Some("ada@acme.com".to_string()),
                    name: None,
                },
                title: "Call".to_string(),
                body: "Next steps".to_string(),
                occurred_at: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(pipedrive_logged.note_id.as_deref(), Some("500"));
        assert_eq!(pipedrive_logged.deal_id.as_deref(), Some("33"));

        Mock::given(method("GET"))
            .and(path("/proxy/api/v1/contact"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "id": "cont_1",
                    "lead_id": "lead_1",
                    "name": "Ada Lovelace",
                    "emails": [{"email": "ada@acme.com"}]
                }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/proxy/api/v1/activity/note"))
            .and(body_string_contains("\"lead_id\":\"lead_1\""))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({"id": "note_c1"})))
            .mount(&server)
            .await;

        let close_logged = (close::PROVIDER.log_note)(
            test_http(&server, "close"),
            CrmNoteInput {
                contact_id: None,
                query: CrmContactQuery {
                    email: Some("ada@acme.com".to_string()),
                    name: None,
                },
                title: "Call".to_string(),
                body: "Next steps".to_string(),
                occurred_at: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(close_logged.note_id.as_deref(), Some("note_c1"));
    }
}
