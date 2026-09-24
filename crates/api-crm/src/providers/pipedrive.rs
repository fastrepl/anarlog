use anlg_api_nango::NangoIntegrationId;
use anlg_nango::OwnedNangoHttpClient;
use futures_util::future::BoxFuture;
use serde_json::Value;

use crate::contacts::{CrmContact, CrmContactQuery};
use crate::error::{CrmError, Result};

use super::CrmProvider;

pub const PROVIDER: CrmProvider = CrmProvider {
    id: "pipedrive",
    name: "Pipedrive",
    nango_integration_id: anlg_api_nango::Pipedrive::ID,
    search,
};

fn search(
    http: OwnedNangoHttpClient,
    query: CrmContactQuery,
    limit: usize,
) -> BoxFuture<'static, Result<Vec<CrmContact>>> {
    Box::pin(async move {
        let (term, fields) = if let Some(email) = &query.email {
            (email.clone(), "&fields=name,email&exact_match=true")
        } else if let Some(name) = &query.name {
            (name.clone(), "")
        } else {
            return Ok(Vec::new());
        };

        let path = format!(
            "/v1/persons/search?term={}{}&limit={}",
            urlencoding::encode(&term),
            fields,
            limit.min(500),
        );

        let response = http
            .into_proxy()
            .get(path)
            .map_err(|e| CrmError::Provider(e.to_string()))?
            .send()
            .await
            .map_err(|e| CrmError::Provider(e.to_string()))?
            .error_for_status()
            .map_err(|e| CrmError::Provider(e.to_string()))?;
        let body: Value = response
            .json()
            .await
            .map_err(|e| CrmError::Provider(e.to_string()))?;

        let contacts = body["data"]["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|hit| hit.get("item"))
            .map(contact_from_item)
            .collect();
        Ok(contacts)
    })
}

fn text(record: &Value, key: &str) -> Option<String> {
    record
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn first_entry(record: &Value, key: &str) -> Option<String> {
    record
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("value").and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_string)
}

fn contact_from_item(item: &Value) -> CrmContact {
    CrmContact {
        id: item
            .get("id")
            .and_then(Value::as_i64)
            .map(|id| id.to_string()),
        name: text(item, "name"),
        email: text(item, "primary_email").or_else(|| first_entry(item, "emails")),
        company_name: item
            .get("organization")
            .filter(|org| !org.is_null())
            .and_then(|org| text(org, "name")),
        job_title: None,
        phone: first_entry(item, "phones"),
        linkedin_url: None,
        url: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_a_search_item() {
        let item = json!({
            "id": 12,
            "name": "Jane Doe",
            "primary_email": "jane@acme.com",
            "emails": [{"value": "jane@acme.com", "primary": true}],
            "phones": [{"value": "+1 555 0100", "primary": true}],
            "organization": {"id": 5, "name": "Acme"},
        });
        let contact = contact_from_item(&item);
        assert_eq!(contact.id.as_deref(), Some("12"));
        assert_eq!(contact.name.as_deref(), Some("Jane Doe"));
        assert_eq!(contact.email.as_deref(), Some("jane@acme.com"));
        assert_eq!(contact.company_name.as_deref(), Some("Acme"));
        assert_eq!(contact.phone.as_deref(), Some("+1 555 0100"));
    }

    #[test]
    fn falls_back_to_the_first_email_entry() {
        let item = json!({
            "id": 7,
            "name": "Bob",
            "primary_email": null,
            "emails": [{"value": ""}, {"value": "bob@corp.io"}],
        });
        let contact = contact_from_item(&item);
        assert_eq!(contact.email.as_deref(), Some("bob@corp.io"));
    }
}
