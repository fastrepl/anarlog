use anlg_api_nango::NangoIntegrationId;
use anlg_nango::OwnedNangoHttpClient;
use futures_util::future::BoxFuture;
use serde_json::Value;

use crate::contacts::{CrmContact, CrmContactQuery};
use crate::error::{CrmError, Result};

use super::CrmProvider;

const FIELDS: &str = "Id, Name, Email, Phone, Title, Account.Name";

pub const PROVIDER: CrmProvider = CrmProvider {
    id: "salesforce",
    name: "Salesforce",
    nango_integration_id: anlg_api_nango::Salesforce::ID,
    search,
};

fn soql_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

fn search(
    http: OwnedNangoHttpClient,
    query: CrmContactQuery,
    limit: usize,
) -> BoxFuture<'static, Result<Vec<CrmContact>>> {
    Box::pin(async move {
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

        let response = http
            .into_proxy()
            .get(format!(
                "/services/data/v65.0/query?q={}",
                urlencoding::encode(&soql)
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

        let contacts = body["records"]
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
