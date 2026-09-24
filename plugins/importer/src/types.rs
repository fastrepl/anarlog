use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportTextFile {
    pub path: String,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedImportAuthorization {
    pub provider_id: String,
    pub authorization_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedImportCredentials {
    pub provider_id: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub token_json: String,
    pub token_received_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CrmProviderInfo {
    pub id: String,
    pub name: String,
    /// The provider has no dynamic client registration, so the user supplies
    /// an OAuth client from their own CRM account.
    pub requires_client: bool,
    /// Redirect URL the user must register on the CRM side when
    /// `requires_client` is set.
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CrmClientInput {
    pub client_id: String,
    pub client_secret: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CrmContactQuery {
    pub email: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CrmContact {
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub company_name: Option<String>,
    pub job_title: Option<String>,
    pub phone: Option<String>,
    pub linkedin_url: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CrmContactLookupResult {
    pub contacts: Vec<CrmContact>,
    pub credentials: ConnectedImportCredentials,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedImportSyncResult {
    pub files: Vec<ImportTextFile>,
    pub credentials: ConnectedImportCredentials,
    pub warnings: Vec<String>,
}
