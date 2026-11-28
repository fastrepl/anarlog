use anlg_api_nango::NangoIntegrationId;
use anlg_nango::{OwnedNangoHttpClient, OwnedNangoProxy};
use futures_util::future::BoxFuture;
use serde_json::{Value, json};

use crate::contacts::{
    CrmContact, CrmContactContext, CrmContactQuery, CrmDeal, CrmLoggedNote, CrmNote, CrmNoteInput,
    contact_matches,
};
use crate::error::{CrmError, Result};

use super::{CrmProvider, format_note_text, keep_optional_result, send_json, truncate_chars};

pub const PROVIDER: CrmProvider = CrmProvider {
    id: "close",
    name: "Close",
    nango_integration_id: anlg_api_nango::Close::ID,
    search,
    context,
    log_note,
};

async fn search_records(
    proxy: &OwnedNangoProxy,
    query: &CrmContactQuery,
    limit: usize,
) -> Result<Vec<(CrmContact, Option<String>)>> {
    let term = query.email.clone().or(query.name.clone());
    let Some(term) = term else {
        return Ok(Vec::new());
    };

    let builder = proxy
        .get(format!(
            "/api/v1/contact/?query={}&_limit={}",
            urlencoding::encode(&term),
            limit.min(100),
        ))
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let body = send_json(builder).await?;

    Ok(body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|record| (contact_from_record(record), text(record, "lead_id")))
        .collect())
}

fn search(
    http: OwnedNangoHttpClient,
    query: CrmContactQuery,
    limit: usize,
) -> BoxFuture<'static, Result<Vec<CrmContact>>> {
    Box::pin(async move {
        let proxy = http.into_proxy();
        let records = search_records(&proxy, &query, limit).await?;
        Ok(records.into_iter().map(|(contact, _)| contact).collect())
    })
}

async fn fetch_opportunities(
    proxy: &OwnedNangoProxy,
    lead_id: &str,
    limit: usize,
) -> Result<Vec<CrmDeal>> {
    let builder = proxy
        .get(format!(
            "/api/v1/opportunity/?lead_id={}&_limit={}",
            urlencoding::encode(lead_id),
            limit.clamp(1, 20),
        ))
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let body = send_json(builder).await?;
    Ok(body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|opp| CrmDeal {
            id: text(opp, "id"),
            name: text(opp, "note").or_else(|| text(opp, "status_label")),
            stage: text(opp, "status_label").or_else(|| text(opp, "status_type")),
            amount: text(opp, "value_formatted"),
            close_date: text(opp, "date_won"),
            url: None,
        })
        .collect())
}

async fn fetch_notes(proxy: &OwnedNangoProxy, lead_id: &str, limit: usize) -> Result<Vec<CrmNote>> {
    let builder = proxy
        .get(format!(
            "/api/v1/activity/note/?lead_id={}&_limit={}",
            urlencoding::encode(lead_id),
            limit.clamp(1, 20),
        ))
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let body = send_json(builder).await?;
    Ok(body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|note| {
            let text_body = text(note, "note")?;
            Some(CrmNote {
                id: text(note, "id"),
                title: None,
                body: truncate_chars(&text_body, 1000),
                created_at: text(note, "date_created"),
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
        let records = search_records(&proxy, &query, limit.clamp(5, 25)).await?;
        let matched = records
            .into_iter()
            .find(|(contact, _)| contact_matches(contact, &query));
        let Some((contact, lead_id)) = matched else {
            return Ok(CrmContactContext {
                contact: None,
                deals: Vec::new(),
                notes: Vec::new(),
            });
        };
        let Some(lead_id) = lead_id else {
            return Ok(CrmContactContext {
                contact: Some(contact),
                deals: Vec::new(),
                notes: Vec::new(),
            });
        };

        let deals = keep_optional_result(fetch_opportunities(&proxy, &lead_id, limit).await)?;
        let notes = keep_optional_result(fetch_notes(&proxy, &lead_id, limit).await)?;
        Ok(CrmContactContext {
            contact: Some(contact),
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
        let (contact_id, contact_name, lead_id) = match input
            .contact_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            Some(id) => {
                let builder = proxy
                    .get(format!("/api/v1/contact/{}/", urlencoding::encode(id)))
                    .map_err(|e| CrmError::Provider(e.to_string()))?;
                let record = send_json(builder).await?;
                let lead_id = text(&record, "lead_id").ok_or_else(|| {
                    CrmError::NotFound("Close contact has no associated lead".to_string())
                })?;
                (id.to_string(), text(&record, "name"), lead_id)
            }
            None => {
                let records = search_records(&proxy, &input.query, 10).await?;
                records
                    .into_iter()
                    .find(|(contact, _)| contact_matches(contact, &input.query))
                    .and_then(|(contact, lead_id)| Some((contact.id?, contact.name, lead_id?)))
                    .ok_or_else(|| {
                        CrmError::NotFound("no matching Close contact found".to_string())
                    })?
            }
        };

        let payload = json!({
            "lead_id": lead_id,
            "note": format_note_text(&input.title, &input.body),
        });
        let builder = proxy
            .post(
                "/api/v1/activity/note/",
                serde_json::to_vec(&payload).map_err(|e| CrmError::Internal(e.to_string()))?,
                "application/json",
            )
            .map_err(|e| CrmError::Provider(e.to_string()))?;
        let created = send_json(builder).await?;

        Ok(CrmLoggedNote {
            note_id: text(&created, "id"),
            contact_id,
            contact_name,
            deal_id: None,
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
