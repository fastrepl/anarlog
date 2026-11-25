use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CrmContactQuery {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CrmContact {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linkedin_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CrmDeal {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CrmNote {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CrmContactContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<CrmContact>,
    pub deals: Vec<CrmDeal>,
    pub notes: Vec<CrmNote>,
}

#[derive(Debug, Clone)]
pub struct CrmNoteInput {
    pub contact_id: Option<String>,
    pub query: CrmContactQuery,
    pub title: String,
    pub body: String,
    pub occurred_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CrmLoggedNote {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_id: Option<String>,
    pub contact_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// A raw contact matches when the email is exactly the requested one, or —
/// with no email to check — the normalized name contains the query.
pub fn contact_matches(contact: &CrmContact, query: &CrmContactQuery) -> bool {
    let query_email = query
        .email
        .as_deref()
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(str::to_ascii_lowercase);
    let query_name = query
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_ascii_lowercase);

    if let Some(email) = &query_email {
        return contact
            .email
            .as_deref()
            .map(str::to_ascii_lowercase)
            .is_some_and(|candidate| candidate == *email);
    }
    if let Some(name) = &query_name {
        return contact
            .name
            .as_deref()
            .map(str::to_ascii_lowercase)
            .is_some_and(|candidate| candidate.contains(name));
    }
    false
}

pub fn matching_contacts(
    contacts: Vec<CrmContact>,
    query: &CrmContactQuery,
    limit: usize,
) -> Vec<CrmContact> {
    contacts
        .into_iter()
        .filter(|contact| contact_matches(contact, query))
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contact(email: Option<&str>, name: Option<&str>) -> CrmContact {
        CrmContact {
            id: None,
            name: name.map(str::to_string),
            email: email.map(str::to_string),
            company_name: None,
            job_title: None,
            phone: None,
            linkedin_url: None,
            url: None,
        }
    }

    #[test]
    fn matches_exact_email_case_insensitively() {
        let query = CrmContactQuery {
            email: Some("ada@example.com".to_string()),
            name: None,
        };
        assert!(contact_matches(
            &contact(Some("Ada@Example.com"), None),
            &query
        ));
        assert!(!contact_matches(
            &contact(Some("other@example.com"), None),
            &query
        ));
    }

    #[test]
    fn falls_back_to_name_match() {
        let query = CrmContactQuery {
            email: None,
            name: Some("lovelace".to_string()),
        };
        assert!(contact_matches(
            &contact(None, Some("Ada Lovelace")),
            &query
        ));
        assert!(!contact_matches(
            &contact(None, Some("Alan Turing")),
            &query
        ));
    }
}
