use hypr_http::HttpClient;

use crate::error::Error;
use crate::types::{
    CreateFolderRequest, GetFileRequest, GoogleDriveFile, ListFilesRequest, ListFilesResponse,
    UpdateMetadataRequest, UploadFileRequest,
};

const FOLDER_MIME_TYPE: &str = "application/vnd.google-apps.folder";

pub struct GoogleDriveClient<C> {
    http: C,
}

impl<C: HttpClient> GoogleDriveClient<C> {
    pub fn new(http: C) -> Self {
        Self { http }
    }

    pub async fn list_files(&self, req: ListFilesRequest) -> Result<ListFilesResponse, Error> {
        let mut query_parts: Vec<String> = Vec::new();

        if let Some(ref q) = req.q {
            query_parts.push(format!("q={}", urlencoding::encode(q)));
        }
        if let Some(page_size) = req.page_size {
            query_parts.push(format!("pageSize={page_size}"));
        }
        if let Some(ref page_token) = req.page_token {
            query_parts.push(format!("pageToken={}", urlencoding::encode(page_token)));
        }
        if let Some(ref order_by) = req.order_by {
            query_parts.push(format!("orderBy={}", urlencoding::encode(order_by)));
        }
        if let Some(ref fields) = req.fields {
            query_parts.push(format!("fields={}", urlencoding::encode(fields)));
        }

        let path = if query_parts.is_empty() {
            "/drive/v3/files".to_string()
        } else {
            format!("/drive/v3/files?{}", query_parts.join("&"))
        };

        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let response: ListFilesResponse = serde_json::from_slice(&bytes)?;
        Ok(response)
    }

    pub async fn get_file(&self, req: GetFileRequest) -> Result<GoogleDriveFile, Error> {
        let file_id = &req.file_id;
        let mut query_parts: Vec<String> = Vec::new();

        if let Some(ref fields) = req.fields {
            query_parts.push(format!("fields={}", urlencoding::encode(fields)));
        }

        let path = if query_parts.is_empty() {
            format!("/drive/v3/files/{file_id}")
        } else {
            format!("/drive/v3/files/{file_id}?{}", query_parts.join("&"))
        };

        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        let file: GoogleDriveFile = serde_json::from_slice(&bytes)?;
        Ok(file)
    }

    pub async fn download_file(&self, file_id: &str) -> Result<Vec<u8>, Error> {
        let path = format!("/drive/v3/files/{file_id}?alt=media");
        let bytes = self.http.get(&path).await.map_err(Error::Http)?;
        Ok(bytes)
    }

    pub async fn create_folder(&self, req: CreateFolderRequest) -> Result<GoogleDriveFile, Error> {
        let metadata = GoogleDriveFile {
            name: Some(req.name),
            mime_type: Some(FOLDER_MIME_TYPE.to_string()),
            parents: req.parent_id.map(|id| vec![id]),
            ..Default::default()
        };

        let body = serde_json::to_vec(&metadata)?;
        let bytes = self
            .http
            .post("/drive/v3/files", body, "application/json")
            .await
            .map_err(Error::Http)?;
        let file: GoogleDriveFile = serde_json::from_slice(&bytes)?;
        Ok(file)
    }

    pub async fn delete_file(&self, file_id: &str) -> Result<(), Error> {
        let path = format!("/drive/v3/files/{file_id}");
        self.http.delete(&path).await.map_err(Error::Http)?;
        Ok(())
    }

    pub async fn upload_file(&self, req: UploadFileRequest) -> Result<GoogleDriveFile, Error> {
        let metadata = GoogleDriveFile {
            name: Some(req.name),
            parents: req.parent_id.map(|id| vec![id]),
            ..Default::default()
        };
        let metadata_json = serde_json::to_vec(&metadata)?;

        let boundary = "drive_upload_boundary";
        let content_type = format!("multipart/related; boundary={boundary}");

        let mut body = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
        body.extend_from_slice(&metadata_json);
        body.extend_from_slice(format!("\r\n--{boundary}\r\n").as_bytes());
        body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", req.mime_type).as_bytes());
        body.extend_from_slice(&req.data);
        body.extend_from_slice(format!("\r\n--{boundary}--").as_bytes());

        let path = "/upload/drive/v3/files?uploadType=multipart";
        let bytes = self
            .http
            .post(path, body, &content_type)
            .await
            .map_err(Error::Http)?;
        let file: GoogleDriveFile = serde_json::from_slice(&bytes)?;
        Ok(file)
    }

    pub async fn update_metadata(
        &self,
        file_id: &str,
        req: UpdateMetadataRequest,
    ) -> Result<GoogleDriveFile, Error> {
        let path = format!("/drive/v3/files/{file_id}");
        let body = serde_json::to_vec(&req)?;
        let bytes = self.http.patch(&path, body).await.map_err(Error::Http)?;
        let file: GoogleDriveFile = serde_json::from_slice(&bytes)?;
        Ok(file)
    }
}
