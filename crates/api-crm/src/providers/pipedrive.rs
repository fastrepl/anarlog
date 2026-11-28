use anlg_api_nango::NangoIntegrationId;
use anlg_nango::{OwnedNangoHttpClient, OwnedNangoProxy};
use futures_util::future::BoxFuture;
use serde_json::{Value, json};

use crate::contacts::{
    CrmContact, CrmContactContext, CrmContactQuery, CrmDeal, CrmLoggedNote, CrmNote, CrmNoteInput,
    contact_matches,
};
use crate::error::{CrmError, Result};

use super::{
    CrmProvider, format_note_text, keep_optional_result, send_json, strip_html_tags,
    truncate_chars,
};

pub const PROVIDER: CrmProvider = CrmProvider {
    id: "pipedrive",
    name: "Pipedrive",
    nango_integration_id: anlg_api_nango::Pipedrive::ID,
    search,
    context,
    log_note,
};

async fn search_items(
    proxy: &OwnedNangoProxy,
    query: &CrmContactQuery,
    limit: usize,
) -> Result<Vec<(CrmContact, Option<i64>)>> {
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
    let builder = proxy
        .get(path)
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let body = send_json(builder).await?;

    Ok(body["data"]["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|hit| hit.get("item"))
        .map(|item| {
            let org_id = item
                .get("organization")
                .and_then(|org| org.get("id"))
                .and_then(Value::as_i64);
            (contact_from_item(item), org_id)
        })
        .collect())
}

fn search(
    http: OwnedNangoHttpClient,
    query: CrmContactQuery,
    limit: usize,
) -> BoxFuture<'static, Result<Vec<CrmContact>>> {
    Box::pin(async move {
        let proxy = http.into_proxy();
        let items = search_items(&proxy, &query, limit).await?;
        Ok(items.into_iter().map(|(contact, _)| contact).collect())
    })
}

async fn fetch_deals(
    proxy: &OwnedNangoProxy,
    person_id: &str,
    status: Option<&str>,
    limit: usize,
) -> Result<Vec<CrmDeal>> {
    let mut path = format!("/v1/persons/{person_id}/deals?limit={}", limit.clamp(1, 20));
    if let Some(status) = status {
        path.push_str(&format!("&status={status}"));
    }
    let builder = proxy
        .get(path)
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let body = send_json(builder).await?;
    Ok(body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|deal| CrmDeal {
            id: deal
                .get("id")
                .and_then(Value::as_i64)
                .map(|id| id.to_string()),
            name: text(deal, "title"),
            stage: text(deal, "status"),
            amount: text(deal, "formatted_value").or_else(|| {
                deal.get("value")
                    .and_then(|v| v.as_f64().map(|n| format!("{n:.0}")))
            }),
            close_date: text(deal, "expected_close_date"),
            url: None,
        })
        .collect())
}

async fn fetch_notes(proxy: &OwnedNangoProxy, person_id: &str, limit: usize) -> Result<Vec<CrmNote>> {
    let builder = proxy
        .get(format!(
            "/v1/notes?person_id={person_id}&limit={}&sort=add_time%20DESC",
            limit.clamp(1, 20)
        ))
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let body = send_json(builder).await?;
    Ok(body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|note| {
            let raw = text(note, "content")?;
            let clean = strip_html_tags(&raw);
            if clean.is_empty() {
                return None;
            }
            Some(CrmNote {
                id: note
                    .get("id")
                    .and_then(Value::as_i64)
                    .map(|id| id.to_string()),
                title: None,
                body: truncate_chars(&clean, 1000),
                created_at: text(note, "add_time"),
            })
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
        let items = search_items(&proxy, &query, limit.clamp(5, 25)).await?;
        let contact = items
            .into_iter()
            .map(|(contact, _)| contact)
            .find(|contact| contact_matches(contact, &query));
        let Some(ref person_id) = contact.as_ref().and_then(|c| c.id.clone()) else {
            return Ok(CrmContactContext {
                contact,
                deals: Vec::new(),
                notes: Vec::new(),
            });
        };
        let deals =
            keep_optional_result(fetch_deals(&proxy, person_id, None, limit).await)?;
        let notes = keep_optional_result(fetch_notes(&proxy, person_id, limit).await)?;
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
        let (person_id_str, contact_name, org_id) = match input
            .contact_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            Some(id) => (id.to_string(), None, None),
            None => {
                let items = search_items(&proxy, &input.query, 10).await?;
                items
                    .into_iter()
                    .find(|(contact, _)| contact_matches(contact, &input.query))
                    .and_then(|(contact, org_id)| contact.id.map(|id| (id, contact.name, org_id)))
                    .ok_or_else(|| {
                        CrmError::NotFound("no matching Pipedrive contact found".to_string())
                    })?
            }
        };

        let person_id: i64 = person_id_str
            .parse()
            .map_err(|_| CrmError::NotFound("invalid Pipedrive person id".to_string()))?;
        let deal_id = keep_optional_result(fetch_deals(&proxy, &person_id_str, Some("open"), 1).await)?
            .into_iter()
            .next()
            .and_then(|d| d.id);

        let mut payload = json!({
            "content": format_note_text(&input.title, &input.body),
            "person_id": person_id,
        });
        if let Some(deal_id_num) = deal_id.as_deref().and_then(|id| id.parse::<i64>().ok()) {
            payload["deal_id"] = json!(deal_id_num);
        }
        if let Some(org_id) = org_id {
            payload["org_id"] = json!(org_id);
        }

        let builder = proxy
            .post(
                "/v1/notes",
                serde_json::to_vec(&payload).map_err(|e| CrmError::Internal(e.to_string()))?,
                "application/json",
            )
            .map_err(|e| CrmError::Provider(e.to_string()))?;
        let created = send_json(builder).await?;

        Ok(CrmLoggedNote {
            note_id: created["data"]["id"]
                .as_i64()
                .map(|id| id.to_string()),
            contact_id: person_id_str,
            contact_name,
            deal_id,
            url: None,
        })
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
