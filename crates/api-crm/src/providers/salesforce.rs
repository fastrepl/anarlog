use anlg_api_nango::NangoIntegrationId;
use anlg_nango::{OwnedNangoHttpClient, OwnedNangoProxy};
use futures_util::future::BoxFuture;
use serde_json::{Value, json};

use crate::contacts::{
    CrmContact, CrmContactContext, CrmContactQuery, CrmDeal, CrmLoggedNote, CrmNote, CrmNoteInput,
    contact_matches,
};
use crate::error::{CrmError, Result};

use super::{CrmProvider, date_prefix, keep_optional_result, send_json, truncate_chars};

const FIELDS: &str = "Id, Name, Email, Phone, Title, AccountId, Account.Name";

pub const PROVIDER: CrmProvider = CrmProvider {
    id: "salesforce",
    name: "Salesforce",
    nango_integration_id: anlg_api_nango::Salesforce::ID,
    search,
    context,
    log_note,
};

fn soql_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

async fn run_soql(proxy: &OwnedNangoProxy, soql: &str) -> Result<Vec<Value>> {
    let builder = proxy
        .get(format!(
            "/services/data/v65.0/query?q={}",
            urlencoding::encode(soql)
        ))
        .map_err(|e| CrmError::Provider(e.to_string()))?;
    let body = send_json(builder).await?;
    Ok(body["records"].as_array().cloned().unwrap_or_default())
}

async fn search_records(
    proxy: &OwnedNangoProxy,
    query: &CrmContactQuery,
    limit: usize,
) -> Result<Vec<(CrmContact, Option<String>)>> {
    let condition = if let Some(email) = &query.email {
        format!("Email = '{}'", soql_escape(email))
    } else if let Some(name) = &query.name {
        format!("Name LIKE '%{}%'", soql_escape(name))
    } else {
        return Ok(Vec::new());
    };
    let soql = format!(
        "SELECT {FIELDS} FROM Contact WHERE {condition} LIMIT {}",
        limit.min(200),
    );
    let records = run_soql(proxy, &soql).await?;
    Ok(records
        .iter()
        .map(|record| (contact_from_record(record), text(record, "AccountId")))
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

fn map_opportunities(records: Vec<Value>) -> Vec<CrmDeal> {
    records
        .into_iter()
        .map(|record| CrmDeal {
            id: text(&record, "Id"),
            name: text(&record, "Name"),
            stage: text(&record, "StageName"),
            amount: record
                .get("Amount")
                .and_then(|v| v.as_f64().map(|n| format!("{n:.0}")).or_else(|| v.as_str().map(str::to_string))),
            close_date: text(&record, "CloseDate"),
            url: None,
        })
        .collect()
}

async fn fetch_opportunities(
    proxy: &OwnedNangoProxy,
    contact_id: &str,
    account_id: Option<&str>,
    limit: usize,
) -> Result<Vec<CrmDeal>> {
    let soql = format!(
        "SELECT Id, Name, StageName, Amount, CloseDate FROM Opportunity WHERE Id IN (SELECT OpportunityId FROM OpportunityContactRole WHERE ContactId = '{}') ORDER BY LastModifiedDate DESC LIMIT {}",
        soql_escape(contact_id),
        limit.clamp(1, 20),
    );
    let records = run_soql(proxy, &soql).await?;
    if !records.is_empty() {
        return Ok(map_opportunities(records));
    }
    let Some(account_id) = account_id else {
        return Ok(Vec::new());
    };
    let account_soql = format!(
        "SELECT Id, Name, StageName, Amount, CloseDate FROM Opportunity WHERE AccountId = '{}' ORDER BY LastModifiedDate DESC LIMIT {}",
        soql_escape(account_id),
        limit.clamp(1, 20),
    );
    Ok(map_opportunities(run_soql(proxy, &account_soql).await?))
}

async fn fetch_tasks(proxy: &OwnedNangoProxy, contact_id: &str, limit: usize) -> Result<Vec<CrmNote>> {
    let soql = format!(
        "SELECT Id, Subject, Description, ActivityDate, CreatedDate FROM Task WHERE WhoId = '{}' ORDER BY CreatedDate DESC LIMIT {}",
        soql_escape(contact_id),
        limit.clamp(1, 20),
    );
    let records = run_soql(proxy, &soql).await?;
    Ok(records
        .into_iter()
        .filter_map(|record| {
            let subject = text(&record, "Subject");
            let description = text(&record, "Description");
            let body = description.clone().or_else(|| subject.clone())?;
            Some(CrmNote {
                id: text(&record, "Id"),
                title: subject,
                body: truncate_chars(&body, 1000),
                created_at: text(&record, "CreatedDate").or_else(|| text(&record, "ActivityDate")),
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
        let Some((contact, account_id)) = matched else {
            return Ok(CrmContactContext {
                contact: None,
                deals: Vec::new(),
                notes: Vec::new(),
            });
        };
        let Some(ref contact_id) = contact.id else {
            return Ok(CrmContactContext {
                contact: Some(contact),
                deals: Vec::new(),
                notes: Vec::new(),
            });
        };
        let deals = keep_optional_result(
            fetch_opportunities(&proxy, contact_id, account_id.as_deref(), limit).await,
        )?;
        let notes = keep_optional_result(fetch_tasks(&proxy, contact_id, limit).await)?;
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
        let (contact_id, contact_name, account_id) = match input
            .contact_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            Some(id) => (id.to_string(), None, None),
            None => {
                let records = search_records(&proxy, &input.query, 10).await?;
                records
                    .into_iter()
                    .find(|(contact, _)| contact_matches(contact, &input.query))
                    .and_then(|(contact, account_id)| {
                        contact.id.map(|id| (id, contact.name, account_id))
                    })
                    .ok_or_else(|| {
                        CrmError::NotFound("no matching Salesforce contact found".to_string())
                    })?
            }
        };

        let open_opps_soql = format!(
            "SELECT Id, IsClosed FROM Opportunity WHERE Id IN (SELECT OpportunityId FROM OpportunityContactRole WHERE ContactId = '{}') ORDER BY IsClosed ASC, LastModifiedDate DESC LIMIT 1",
            soql_escape(&contact_id),
        );
        let deal_id = keep_optional_result(run_soql(&proxy, &open_opps_soql).await)?
            .into_iter()
            .next()
            .and_then(|record| text(&record, "Id"));

        let what_id = deal_id.clone().or(account_id);
        let subject = match input.title.trim() {
            "" => "Meeting note".to_string(),
            title => truncate_chars(title, 255),
        };
        let mut payload = json!({
            "WhoId": contact_id,
            "Subject": subject,
            "Description": truncate_chars(input.body.trim(), 30_000),
            "Status": "Completed",
            "TaskSubtype": "Call",
        });
        if let Some(what_id) = what_id {
            payload["WhatId"] = json!(what_id);
        }
        if let Some(date) = date_prefix(input.occurred_at.as_deref()) {
            payload["ActivityDate"] = json!(date);
        }

        let builder = proxy
            .post(
                "/services/data/v65.0/sobjects/Task",
                serde_json::to_vec(&payload).map_err(|e| CrmError::Internal(e.to_string()))?,
                "application/json",
            )
            .map_err(|e| CrmError::Provider(e.to_string()))?;
        let created = send_json(builder).await?;

        Ok(CrmLoggedNote {
            note_id: text(&created, "id"),
            contact_id,
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

fn contact_from_record(record: &Value) -> CrmContact {
    CrmContact {
        id: text(record, "Id"),
        name: text(record, "Name"),
        email: text(record, "Email"),
        company_name: record
            .get("Account")
            .filter(|account| !account.is_null())
            .and_then(|account| text(account, "Name")),
        job_title: text(record, "Title"),
        phone: text(record, "Phone"),
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
            "Id": "003abc",
            "Name": "Jane Doe",
            "Email": "jane@acme.com",
            "Phone": "+1 555 0100",
            "Title": "VP Sales",
            "Account": {"Name": "Acme"},
        });
        let contact = contact_from_record(&record);
        assert_eq!(contact.id.as_deref(), Some("003abc"));
        assert_eq!(contact.name.as_deref(), Some("Jane Doe"));
        assert_eq!(contact.email.as_deref(), Some("jane@acme.com"));
        assert_eq!(contact.company_name.as_deref(), Some("Acme"));
        assert_eq!(contact.job_title.as_deref(), Some("VP Sales"));
        assert_eq!(contact.phone.as_deref(), Some("+1 555 0100"));
    }

    #[test]
    fn tolerates_a_null_account_and_blank_fields() {
        let record = json!({
            "Id": "003def",
            "Name": "  ",
            "Account": null,
            "Title": "",
        });
        let contact = contact_from_record(&record);
        assert_eq!(contact.id.as_deref(), Some("003def"));
        assert_eq!(contact.name, None);
        assert_eq!(contact.company_name, None);
        assert_eq!(contact.job_title, None);
    }

    #[test]
    fn escapes_quotes_in_soql_values() {
        assert_eq!(soql_escape("o'brien\\x"), "o\\'brien\\\\x");
    }
}
