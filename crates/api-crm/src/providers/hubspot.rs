use anlg_api_nango::NangoIntegrationId;
use anlg_nango::{OwnedNangoHttpClient, OwnedNangoProxy};
use futures_util::future::BoxFuture;
use serde_json::{Value, json};

use crate::contacts::{
    CrmContact, CrmContactContext, CrmContactQuery, CrmDeal, CrmLoggedNote, CrmNote, CrmNoteInput,
    matching_contacts,
};
use crate::error::{CrmError, Result};

use super::{
    CrmProvider, format_note_text, keep_optional_result, send_json, strip_html_tags,
    truncate_chars, unix_millis_string,
};

const PROPERTIES: &[&str] = &[
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
    id: "hubspot",
    name: "HubSpot",
    nango_integration_id: anlg_api_nango::HubSpot::ID,
    search,
    context,
    log_note,
};

async fn search_with_proxy(
    proxy: &OwnedNangoProxy,
    query: &CrmContactQuery,
    limit: usize,
) -> Result<Vec<CrmContact>> {
    let mut body = json!({
        "properties": PROPERTIES,
        "limit": limit.min(100),
    });
    if let Some(email) = &query.email {
        body["filterGroups"] = json!([{
            "filters": [{
                "propertyName": "email",
                "operator": "EQ",
                "value": email,
            }],
        }]);
    } else if let Some(name) = &query.name {
        body["query"] = json!(name);
    }

    let builder = proxy
        .post(
            "/crm/v3/objects/contacts/search",
            serde_json::to_vec(&body).map_err(|e| CrmError::Internal(e.to_string()))?,
            "application/json",
        )
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let payload = send_json(builder).await?;

    Ok(payload
        .get("results")
        .and_then(Value::as_array)
        .map(|results| results.iter().filter_map(contact_from_result).collect())
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

async fn associated_ids(
    proxy: &OwnedNangoProxy,
    contact_id: &str,
    object_type: &str,
    limit: usize,
) -> Result<Vec<String>> {
    let builder = proxy
        .get(format!(
            "/crm/v4/objects/contacts/{contact_id}/associations/{object_type}?limit={}",
            limit.clamp(1, 20)
        ))
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let payload = send_json(builder).await?;
    Ok(payload["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.get("toObjectId").and_then(|id| {
                id.as_str()
                    .map(str::to_string)
                    .or_else(|| id.as_i64().map(|n| n.to_string()))
            })
        })
        .collect())
}

async fn fetch_deals(proxy: &OwnedNangoProxy, contact_id: &str, limit: usize) -> Result<Vec<CrmDeal>> {
    let ids = associated_ids(proxy, contact_id, "deals", limit).await?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let inputs: Vec<Value> = ids.into_iter().map(|id| json!({"id": id})).collect();
    let body = json!({
        "properties": ["dealname", "dealstage", "amount", "closedate"],
        "inputs": inputs,
    });
    let builder = proxy
        .post(
            "/crm/v3/objects/deals/batch/read",
            serde_json::to_vec(&body).map_err(|e| CrmError::Internal(e.to_string()))?,
            "application/json",
        )
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let payload = send_json(builder).await?;
    Ok(payload["results"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            let props = &item["properties"];
            CrmDeal {
                id: item.get("id").and_then(Value::as_str).map(str::to_string),
                name: prop_text(props, "dealname"),
                stage: prop_text(props, "dealstage"),
                amount: prop_text(props, "amount"),
                close_date: prop_text(props, "closedate"),
                url: None,
            }
        })
        .collect())
}

async fn fetch_notes(proxy: &OwnedNangoProxy, contact_id: &str, limit: usize) -> Result<Vec<CrmNote>> {
    let ids = associated_ids(proxy, contact_id, "notes", limit).await?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let inputs: Vec<Value> = ids.into_iter().map(|id| json!({"id": id})).collect();
    let body = json!({
        "properties": ["hs_note_body", "hs_timestamp"],
        "inputs": inputs,
    });
    let builder = proxy
        .post(
            "/crm/v3/objects/notes/batch/read",
            serde_json::to_vec(&body).map_err(|e| CrmError::Internal(e.to_string()))?,
            "application/json",
        )
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let payload = send_json(builder).await?;
    let mut notes: Vec<CrmNote> = payload["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let props = &item["properties"];
            let raw = prop_text(props, "hs_note_body")?;
            let clean = strip_html_tags(&raw);
            if clean.is_empty() {
                return None;
            }
            Some(CrmNote {
                id: item.get("id").and_then(Value::as_str).map(str::to_string),
                title: None,
                body: truncate_chars(&clean, 1000),
                created_at: prop_text(props, "hs_timestamp"),
            })
        })
        .collect();
    notes.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    notes.truncate(limit);
    Ok(notes)
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
        let Some(contact_id) = contact.as_ref().and_then(|c| c.id.clone()) else {
            return Ok(CrmContactContext {
                contact,
                deals: Vec::new(),
                notes: Vec::new(),
            });
        };

        let deals = keep_optional_result(fetch_deals(&proxy, &contact_id, limit).await)?;
        let notes = keep_optional_result(fetch_notes(&proxy, &contact_id, limit).await)?;
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
                        CrmError::NotFound("no matching HubSpot contact found".to_string())
                    })?
            }
        };

        let deal_id = keep_optional_result(associated_ids(&proxy, &contact_id, "deals", 1).await)?
            .into_iter()
            .next();
        let company_id =
            keep_optional_result(associated_ids(&proxy, &contact_id, "companies", 1).await)?
                .into_iter()
                .next();

        let mut associations = vec![json!({
            "to": {"id": contact_id},
            "types": [{"associationCategory": "HUBSPOT_DEFINED", "associationTypeId": 202}],
        })];
        if let Some(ref deal) = deal_id {
            associations.push(json!({
                "to": {"id": deal},
                "types": [{"associationCategory": "HUBSPOT_DEFINED", "associationTypeId": 214}],
            }));
        }
        if let Some(ref company) = company_id {
            associations.push(json!({
                "to": {"id": company},
                "types": [{"associationCategory": "HUBSPOT_DEFINED", "associationTypeId": 190}],
            }));
        }

        let timestamp = input
            .occurred_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(unix_millis_string);

        let body = json!({
            "properties": {
                "hs_timestamp": timestamp,
                "hs_note_body": format_note_text(&input.title, &input.body),
            },
            "associations": associations,
        });
        let builder = proxy
            .post(
                "/crm/v3/objects/notes",
                serde_json::to_vec(&body).map_err(|e| CrmError::Internal(e.to_string()))?,
                "application/json",
            )
            .map_err(|e| CrmError::Provider(e.to_string()))?;
        let created = send_json(builder).await?;

        Ok(CrmLoggedNote {
            note_id: created
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string),
            contact_id,
            contact_name,
            deal_id,
            url: None,
        })
    })
}

fn prop_text(props: &Value, key: &str) -> Option<String> {
    props
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn contact_from_result(result: &Value) -> Option<CrmContact> {
    let properties = result.get("properties")?.as_object()?;
    let string = |key: &str| {
        properties
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };

    let first = string("firstname");
    let last = string("lastname");
    let name = match (first, last) {
        (None, None) => None,
        (first, last) => Some(
            [first, last]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" "),
        ),
    };

    Some(CrmContact {
        id: result.get("id").and_then(Value::as_str).map(str::to_string),
        name,
        email: string("email"),
        company_name: string("company"),
        job_title: string("jobtitle"),
        phone: string("phone").or_else(|| string("mobilephone")),
        linkedin_url: string("hs_linkedin_url"),
        url: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_hubspot_contact_result() {
        let result = json!({
            "id": "42",
            "properties": {
                "firstname": "Ada",
                "lastname": "Lovelace",
                "email": "ada@example.com",
                "company": "Acme",
                "jobtitle": "Engineer",
                "phone": null,
                "mobilephone": "+15551234",
                "hs_linkedin_url": "https://www.linkedin.com/in/ada",
            },
        });

        let contact = contact_from_result(&result).unwrap();
        assert_eq!(contact.id.as_deref(), Some("42"));
        assert_eq!(contact.name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(contact.email.as_deref(), Some("ada@example.com"));
        assert_eq!(contact.company_name.as_deref(), Some("Acme"));
        assert_eq!(contact.job_title.as_deref(), Some("Engineer"));
        assert_eq!(contact.phone.as_deref(), Some("+15551234"));
        assert_eq!(
            contact.linkedin_url.as_deref(),
            Some("https://www.linkedin.com/in/ada")
        );
    }
}
