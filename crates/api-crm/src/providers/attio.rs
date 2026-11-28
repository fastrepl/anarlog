use anlg_api_nango::NangoIntegrationId;
use anlg_nango::{OwnedNangoHttpClient, OwnedNangoProxy};
use futures_util::future::BoxFuture;
use serde_json::{Map, Value, json};

use crate::contacts::{
    CrmContact, CrmContactContext, CrmContactQuery, CrmDeal, CrmLoggedNote, CrmNote, CrmNoteInput,
    matching_contacts,
};
use crate::error::{CrmError, Result};

use super::{CrmProvider, keep_optional_result, send_json, truncate_chars};

pub const PROVIDER: CrmProvider = CrmProvider {
    id: "attio",
    name: "Attio",
    nango_integration_id: anlg_api_nango::Attio::ID,
    search,
    context,
    log_note,
};

async fn search_with_proxy(
    proxy: &OwnedNangoProxy,
    query: &CrmContactQuery,
    limit: usize,
) -> Result<Vec<CrmContact>> {
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

    let builder = proxy
        .post(
            "/v2/objects/people/records/query",
            serde_json::to_vec(&body).map_err(|e| CrmError::Internal(e.to_string()))?,
            "application/json",
        )
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let payload = send_json(builder).await?;

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
}

fn search(
    http: OwnedNangoHttpClient,
    query: CrmContactQuery,
    limit: usize,
) -> BoxFuture<'static, Result<Vec<CrmContact>>> {
    Box::pin(async move {
        let proxy = http.into_proxy();
        search_with_proxy(&proxy, &query, limit).await
    })
}

async fn fetch_notes(
    proxy: &OwnedNangoProxy,
    record_id: &str,
    limit: usize,
) -> Result<Vec<CrmNote>> {
    let builder = proxy
        .get(format!(
            "/v2/notes?parent_object=people&parent_record_id={}&limit={}",
            urlencoding::encode(record_id),
            limit.clamp(1, 20),
        ))
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let payload = send_json(builder).await?;
    Ok(payload["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|note| {
            let body = note
                .get("content_plaintext")
                .or_else(|| note.get("content_markdown"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            Some(CrmNote {
                id: note
                    .get("id")
                    .and_then(|id| id.get("note_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                title: note
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
                body: truncate_chars(body, 1000),
                created_at: note
                    .get("created_at")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect())
}

async fn fetch_deals(
    proxy: &OwnedNangoProxy,
    record_id: &str,
    limit: usize,
) -> Result<Vec<CrmDeal>> {
    let body = json!({
        "filter": {
            "associated_people": {
                "target_object": "people",
                "target_record_id": record_id,
            },
        },
        "limit": limit.clamp(1, 20),
    });
    let builder = proxy
        .post(
            "/v2/objects/deals/records/query",
            serde_json::to_vec(&body).map_err(|e| CrmError::Internal(e.to_string()))?,
            "application/json",
        )
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let payload = send_json(builder).await?;
    Ok(payload["data"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|record| {
            let values = record.get("values").and_then(Value::as_object);
            let name = values
                .and_then(|v| first_entry_field(v, "name", &["value"]))
                .map(str::to_string);
            let stage = values.and_then(|v| {
                attribute_entries(v, "stage").and_then(|entries| {
                    entries.iter().find_map(|entry| {
                        entry
                            .get("status")
                            .and_then(|s| s.get("title"))
                            .and_then(Value::as_str)
                            .or_else(|| entry.get("value").and_then(Value::as_str))
                            .map(str::to_string)
                    })
                })
            });
            let amount = values.and_then(|v| {
                attribute_entries(v, "value").and_then(|entries| {
                    entries.iter().find_map(|entry| {
                        entry.get("currency_value").and_then(|val| {
                            val.as_f64()
                                .map(|n| n.to_string())
                                .or_else(|| val.as_str().map(str::to_string))
                        })
                    })
                })
            });
            CrmDeal {
                id: record
                    .get("id")
                    .and_then(|id| id.get("record_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                name,
                stage,
                amount,
                close_date: None,
                url: record
                    .get("web_url")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            }
        })
        .collect())
}

fn context(
    http: OwnedNangoHttpClient,
    query: CrmContactQuery,
    limit: usize,
) -> BoxFuture<'static, Result<CrmContactContext>> {
    Box::pin(async move {
        let proxy = http.into_proxy();
        let candidates = search_with_proxy(&proxy, &query, limit.clamp(5, 25)).await?;
        let contact = matching_contacts(candidates, &query, 1).into_iter().next();
        let Some(record_id) = contact.as_ref().and_then(|c| c.id.clone()) else {
            return Ok(CrmContactContext {
                contact,
                deals: Vec::new(),
                notes: Vec::new(),
            });
        };

        let notes = keep_optional_result(fetch_notes(&proxy, &record_id, limit).await)?;
        let deals = keep_optional_result(fetch_deals(&proxy, &record_id, limit).await)?;
        Ok(CrmContactContext {
            contact,
            deals,
            notes,
        })
    })
}

fn log_note(
    http: OwnedNangoHttpClient,
    input: CrmNoteInput,
) -> BoxFuture<'static, Result<CrmLoggedNote>> {
    Box::pin(async move {
        let proxy = http.into_proxy();
        let (contact_id, contact_name) = match input
            .contact_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            Some(id) => (id.to_string(), None),
            None => {
                let candidates = search_with_proxy(&proxy, &input.query, 10).await?;
                matching_contacts(candidates, &input.query, 1)
                    .into_iter()
                    .next()
                    .and_then(|c| c.id.map(|id| (id, c.name)))
                    .ok_or_else(|| {
                        CrmError::NotFound("no matching Attio person found".to_string())
                    })?
            }
        };

        let title = match input.title.trim() {
            "" => "Meeting note".to_string(),
            raw => truncate_chars(raw, 200),
        };
        let body = json!({
            "data": {
                "parent_object": "people",
                "parent_record_id": contact_id,
                "title": title,
                "format": "markdown",
                "content": truncate_chars(input.body.trim(), 30_000),
            }
        });
        let builder = proxy
            .post(
                "/v2/notes",
                serde_json::to_vec(&body).map_err(|e| CrmError::Internal(e.to_string()))?,
                "application/json",
            )
            .map_err(|e| CrmError::Provider(e.to_string()))?;
        let created = send_json(builder).await?;

        Ok(CrmLoggedNote {
            note_id: created
                .get("data")
                .and_then(|d| d.get("id"))
                .and_then(|id| id.get("note_id"))
                .and_then(Value::as_str)
                .map(str::to_string),
            contact_id,
            contact_name,
            deal_id: None,
            url: None,
        })
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
