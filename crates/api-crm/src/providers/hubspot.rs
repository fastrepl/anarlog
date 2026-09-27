use anlg_api_nango::NangoIntegrationId;
use anlg_nango::OwnedNangoHttpClient;
use futures_util::future::BoxFuture;
use serde_json::{Value, json};

use crate::contacts::{CrmContact, CrmContactQuery};
use crate::error::{CrmError, Result};

use super::CrmProvider;

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
};

fn search(
    http: OwnedNangoHttpClient,
    query: CrmContactQuery,
    limit: usize,
) -> BoxFuture<'static, Result<Vec<CrmContact>>> {
    Box::pin(async move {
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

        let response = http
            .into_proxy()
            .post(
                "/crm/v3/objects/contacts/search",
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
            .get("results")
            .and_then(Value::as_array)
            .map(|results| results.iter().filter_map(contact_from_result).collect())
            .unwrap_or_default())
    })
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
