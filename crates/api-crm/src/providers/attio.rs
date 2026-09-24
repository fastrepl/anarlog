use anlg_api_nango::NangoIntegrationId;
use anlg_nango::OwnedNangoHttpClient;
use futures_util::future::BoxFuture;
use serde_json::{Map, Value, json};

use crate::contacts::{CrmContact, CrmContactQuery};
use crate::error::{CrmError, Result};

use super::CrmProvider;

pub const PROVIDER: CrmProvider = CrmProvider {
    id: "attio",
    name: "Attio",
    nango_integration_id: anlg_api_nango::Attio::ID,
    search,
};

fn search(
    http: OwnedNangoHttpClient,
    query: CrmContactQuery,
    limit: usize,
) -> BoxFuture<'static, Result<Vec<CrmContact>>> {
    Box::pin(async move {
        let filter = if let Some(email) = &query.email {
            json!({
                "email_addresses": {
                    "email_address": { "$eq": email },
                },
            })
        } else if let Some(name) = &query.name {
            json!({
                "name": {
                    "full_name": { "$contains": name },
                },
            })
        } else {
            return Ok(Vec::new());
        };
        let body = json!({
            "filter": filter,
            "limit": limit.min(200),
        });

        let response = http
            .into_proxy()
            .post(
                "/v2/objects/people/records/query",
                serde_json::to_vec(&body).map_err(|e| CrmError::Internal(e.to_string()))?,
                "application/json",
            )
            .map_err(|e| CrmError::Provider(e.to_string()))?
            .send()
            .await
            .map_err(|e| CrmError::Provider(e.to_string()))?
            .error_for_status()
            .map_err(|e| CrmError::Provider(e.to_string()))?;

        let payload: Value = response
            .json()
            .await
            .map_err(|e| CrmError::Provider(e.to_string()))?;
        Ok(payload
            .get("data")
            .and_then(Value::as_array)
            .map(|records| {
                records
                    .iter()
                    .filter_map(|record| contact_from_record(record, query.email.as_deref()))
                    .collect()
            })
            .unwrap_or_default())
    })
}

fn attribute_entries<'a>(values: &'a Map<String, Value>, slug: &str) -> Option<&'a Vec<Value>> {
    values.get(slug).and_then(Value::as_array)
}

fn entry_field<'a>(entry: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        entry
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

fn first_entry_field<'a>(
    values: &'a Map<String, Value>,
    slug: &str,
    keys: &[&str],
) -> Option<&'a str> {
    attribute_entries(values, slug)?
        .iter()
        .find_map(|entry| entry_field(entry, keys))
}

fn contact_from_record(record: &Value, query_email: Option<&str>) -> Option<CrmContact> {
    let values = record.get("values")?.as_object()?;

    let name = first_entry_field(values, "name", &["full_name"])
        .map(str::to_string)
        .or_else(|| {
            let first = first_entry_field(values, "name", &["first_name"]);
            let last = first_entry_field(values, "name", &["last_name"]);
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

    Some(CrmContact {
        id: record
            .get("id")
            .and_then(|id| id.get("record_id"))
            .and_then(Value::as_str)
            .map(str::to_string),
        name,
        // Prefer the queried email when the match came through a secondary
        // address so `matching_contacts` keeps the record.
        email: attribute_entries(values, "email_addresses")
            .and_then(|entries| {
                let queried = query_email.and_then(|wanted| {
                    entries.iter().find_map(|entry| {
                        entry_field(entry, &["email_address"])
                            .filter(|email| email.eq_ignore_ascii_case(wanted))
                    })
                });
                queried.or_else(|| {
                    entries
                        .iter()
                        .find_map(|entry| entry_field(entry, &["email_address"]))
                })
            })
            .map(str::to_string),
        // Attio's `company` attribute is a record reference and does not carry
        // the company's name, so it is left unset rather than resolved with
        // extra requests per result.
        company_name: None,
        job_title: first_entry_field(values, "job_title", &["value"]).map(str::to_string),
        phone: first_entry_field(
            values,
            "phone_numbers",
            &["original_phone_number", "phone_number"],
        )
        .map(str::to_string),
        linkedin_url: None,
        url: record
            .get("web_url")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_attio_person_record() {
        let record = json!({
            "id": {
                "workspace_id": "ws",
                "object_id": "obj",
                "record_id": "rec_1",
            },
            "web_url": "https://app.attio.com/acme/person/rec_1",
            "values": {
                "name": [{
                    "first_name": "Ada",
                    "last_name": "Lovelace",
                    "full_name": "Ada Lovelace",
                }],
                "email_addresses": [{
                    "email_address": "ada@example.com",
                }],
                "phone_numbers": [{
                    "original_phone_number": "+15551234",
                }],
                "job_title": [{"value": "Engineer"}],
            },
        });

        let contact = contact_from_record(&record, None).unwrap();
        assert_eq!(contact.id.as_deref(), Some("rec_1"));
        assert_eq!(contact.name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(contact.email.as_deref(), Some("ada@example.com"));
        assert_eq!(contact.phone.as_deref(), Some("+15551234"));
        assert_eq!(contact.job_title.as_deref(), Some("Engineer"));
        assert_eq!(
            contact.url.as_deref(),
            Some("https://app.attio.com/acme/person/rec_1")
        );
    }

    #[test]
    fn joins_first_and_last_name_when_full_name_missing() {
        let record = json!({
            "id": {"record_id": "rec_2"},
            "values": {
                "name": [{"first_name": "Ada", "last_name": "Lovelace"}],
            },
        });

        let contact = contact_from_record(&record, None).unwrap();
        assert_eq!(contact.name.as_deref(), Some("Ada Lovelace"));
    }

    #[test]
    fn prefers_the_queried_email_over_the_first_entry() {
        let record = json!({
            "id": {"record_id": "rec_3"},
            "values": {
                "name": [{"full_name": "Ada Lovelace"}],
                "email_addresses": [
                    {"email_address": "ada@example.com"},
                    {"email_address": "ada.lovelace@work.com"},
                ],
            },
        });

        let contact = contact_from_record(&record, Some("ada.lovelace@work.com")).unwrap();
        assert_eq!(contact.email.as_deref(), Some("ada.lovelace@work.com"));

        let contact = contact_from_record(&record, Some("other@x.com")).unwrap();
        assert_eq!(contact.email.as_deref(), Some("ada@example.com"));
    }
}
