use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDriveFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parents: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_view_link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_content_link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trashed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starred: Option<bool>,
}

pub struct ListFilesRequest {
    pub q: Option<String>,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub order_by: Option<String>,
    pub fields: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilesResponse {
    pub kind: String,
    #[serde(default)]
    pub next_page_token: Option<String>,
    #[serde(default)]
    pub incomplete_search: Option<bool>,
    #[serde(default)]
    pub files: Vec<GoogleDriveFile>,
}

pub struct GetFileRequest {
    pub file_id: String,
    pub fields: Option<String>,
}

pub struct CreateFolderRequest {
    pub name: String,
    pub parent_id: Option<String>,
}

pub struct UploadFileRequest {
    pub name: String,
    pub parent_id: Option<String>,
    pub mime_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadataRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starred: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trashed: Option<bool>,
}
