use anlg_api_nango::NangoIntegrationId;
use anlg_nango::OwnedNangoHttpClient;
use futures_util::future::BoxFuture;
use serde_json::Value;

use crate::contacts::{CrmContact, CrmContactQuery};
use crate::error::{CrmError, Result};

use super::CrmProvider;

pub const PROVIDER: CrmProvider = CrmProvider {
    id: "close",
    name: "Close",
    nango_integration_id: anlg_api_nango::Close::ID,
    search,
};

fn search(
    http: OwnedNangoHttpClient,
    query: CrmContactQuery,
    limit: usize,
) -> BoxFuture<'static, Result<Vec<CrmContact>>> {
    Box::pin(async move {
        let term = query.email.clone().or(query.name.clone());
        let Some(term) = term else {
            return Ok(Vec::new());
        };

        let response = http
            .into_proxy()
            .get(format!(
                "/api/v1/contact/?query={}&_limit={}",
                urlencoding::encode(&term),
                limit.min(100),
            ))
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

        let contacts = body["data"]
            .as_array()
            .into_iter()
            .flatten()
            .map(contact_from_record)
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

fn first_entry(record: &Value, list: &str, field: &str) -> Option<String> {
    record
        .get(list)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get(field).and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_string)
}

fn contact_from_record(record: &Value) -> CrmContact {
    CrmContact {
        id: text(record, "id"),
        name: text(record, "name"),
        email: first_entry(record, "emails", "email"),
        company_name: record
            .get("organizations")
            .and_then(Value::as_array)
            .and_then(|orgs| orgs.first())
            .and_then(|org| text(org, "name")),
        job_title: text(record, "title"),
        phone: first_entry(record, "phones", "phone"),
        linkedin_url: None,
        url: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_a_contact_record() {
        let record = json!({
            "id": "cont_abc",
            "name": "Jane Doe",
            "title": "VP Sales",
            "emails": [{"type": "office", "email": "jane@acme.com"}],
            "phones": [{"type": "office", "phone": "+1 555 0100"}],
            "organizations": [{"id": "orga_x", "name": "Acme"}],
        });
        let contact = contact_from_record(&record);
        assert_eq!(contact.id.as_deref(), Some("cont_abc"));
        assert_eq!(contact.name.as_deref(), Some("Jane Doe"));
        assert_eq!(contact.email.as_deref(), Some("jane@acme.com"));
        assert_eq!(contact.company_name.as_deref(), Some("Acme"));
        assert_eq!(contact.job_title.as_deref(), Some("VP Sales"));
        assert_eq!(contact.phone.as_deref(), Some("+1 555 0100"));
    }

    #[test]
    fn skips_blank_entries() {
        let record = json!({
            "id": "cont_def",
            "emails": [{"email": ""}, {"email": "bob@corp.io"}],
        });
        let contact = contact_from_record(&record);
        assert_eq!(contact.email.as_deref(), Some("bob@corp.io"));
        assert_eq!(contact.company_name, None);
    }
}
