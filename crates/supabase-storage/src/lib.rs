use std::time::Duration;

use futures_util::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const MAX_STORAGE_RESPONSE_BYTES: usize = 64 * 1024;
const OBJECT_VERIFICATION_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("the storage object path is invalid")]
    InvalidPath,
    #[error("{0}")]
    Api(String),
}

#[derive(Clone)]
pub struct SupabaseStorage {
    client: reqwest::Client,
    base_url: String,
    service_role_key: String,
}

#[derive(Deserialize)]
struct SignedUrlResponse {
    #[serde(alias = "signedURL")]
    signed_url: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SignedUpload {
    pub signed_url: String,
    pub token: String,
}

impl std::fmt::Debug for SignedUpload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SignedUpload")
            .field("signed_url", &"[REDACTED]")
            .field("token", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInfo {
    pub size_bytes: u64,
    pub content_type: String,
    pub ciphertext_sha256: String,
    pub format_version: u8,
}

#[derive(Deserialize)]
struct SignedUploadResponse {
    url: String,
}

#[derive(Deserialize)]
struct ObjectInfoResponse {
    size: Option<u64>,
    content_type: Option<String>,
    metadata: Option<ObjectMetadata>,
    user_metadata: Option<ObjectUserMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ObjectMetadata {
    size: Option<u64>,
    mimetype: Option<String>,
    ciphertext_sha256: Option<String>,
    format_version: Option<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ObjectUserMetadata {
    ciphertext_sha256: Option<String>,
    format_version: Option<u8>,
}

#[derive(Deserialize)]
struct StorageErrorResponse {
    code: Option<String>,
}

impl SupabaseStorage {
    pub fn new(client: reqwest::Client, supabase_url: &str, service_role_key: &str) -> Self {
        Self {
            client,
            base_url: supabase_url.trim_end_matches('/').to_string(),
            service_role_key: service_role_key.to_string(),
        }
    }

    fn auth_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        hypr_observability::with_current_trace_context(
            builder
                .header("Authorization", format!("Bearer {}", self.service_role_key))
                .header("apikey", &self.service_role_key),
        )
    }

    async fn read_response(
        response: reqwest::Response,
        operation: &str,
    ) -> Result<(reqwest::StatusCode, Vec<u8>), Error> {
        let status = response.status();
        if response
            .content_length()
            .is_some_and(|length| length > MAX_STORAGE_RESPONSE_BYTES as u64)
        {
            return Err(Error::Api(format!(
                "{operation} response from Supabase Storage was too large"
            )));
        }

        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if body.len().saturating_add(chunk.len()) > MAX_STORAGE_RESPONSE_BYTES {
                return Err(Error::Api(format!(
                    "{operation} response from Supabase Storage was too large"
                )));
            }
            body.extend_from_slice(&chunk);
        }
        Ok((status, body))
    }

    fn object_url(
        &self,
        operation: &str,
        bucket: &str,
        object_path: &str,
    ) -> Result<String, Error> {
        if bucket.is_empty()
            || bucket.len() > 100
            || bucket.contains(['/', '\\'])
            || bucket.chars().any(char::is_control)
            || object_path.is_empty()
            || object_path.len() > 1024
            || object_path.contains('\\')
            || object_path.chars().any(char::is_control)
        {
            return Err(Error::InvalidPath);
        }

        let segments = object_path
            .split('/')
            .map(|segment| {
                if segment.is_empty() || matches!(segment, "." | "..") {
                    return Err(Error::InvalidPath);
                }
                Ok(urlencoding::encode(segment))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(format!(
            "{}/storage/v1/{operation}/{}/{}",
            self.base_url,
            urlencoding::encode(bucket),
            segments.join("/")
        ))
    }

    fn validate_signed_object_url(
        &self,
        raw_url: &str,
        operation: &str,
        bucket: &str,
        object_path: &str,
        require_token: bool,
    ) -> Result<String, Error> {
        let expected = reqwest::Url::parse(&self.object_url(operation, bucket, object_path)?)
            .map_err(|_| Error::Api("Supabase Storage URL is invalid".to_string()))?;
        let absolute = if raw_url.starts_with('/') {
            format!("{}/storage/v1{raw_url}", self.base_url)
        } else {
            raw_url.to_string()
        };
        let signed = reqwest::Url::parse(&absolute)
            .map_err(|_| Error::Api("signed URL was not returned by Supabase Storage".into()))?;
        let has_token = signed
            .query_pairs()
            .any(|(name, value)| name == "token" && !value.is_empty());

        if signed.scheme() != expected.scheme()
            || signed.host_str() != expected.host_str()
            || signed.port_or_known_default() != expected.port_or_known_default()
            || !signed.username().is_empty()
            || signed.password().is_some()
            || signed.path() != expected.path()
            || signed.fragment().is_some()
            || (require_token && !has_token)
        {
            return Err(Error::Api(
                "signed URL was not returned by Supabase Storage".to_string(),
            ));
        }

        Ok(absolute)
    }

    pub async fn create_signed_url(
        &self,
        bucket: &str,
        object_path: &str,
        expires_in_seconds: u64,
    ) -> Result<String, Error> {
        let url = self.object_url("object/sign", bucket, object_path)?;
        let response = self
            .auth_headers(self.client.post(url))
            .json(&serde_json::json!({ "expiresIn": expires_in_seconds }))
            .send()
            .await?;

        let (status, body) = Self::read_response(response, "signed URL").await?;
        if !status.is_success() {
            return Err(Error::Api(format!("failed to create signed URL: {status}")));
        }

        let data: SignedUrlResponse = serde_json::from_slice(&body)
            .map_err(|_| Error::Api("signed URL response was invalid".to_string()))?;
        let raw_url = data
            .signed_url
            .ok_or_else(|| Error::Api("signed URL not returned from Supabase".into()))?;
        self.validate_signed_object_url(&raw_url, "object/sign", bucket, object_path, true)
    }

    pub async fn create_signed_upload(
        &self,
        bucket: &str,
        object_path: &str,
    ) -> Result<SignedUpload, Error> {
        let url = self.object_url("object/upload/sign", bucket, object_path)?;
        let response = self
            .auth_headers(self.client.post(url))
            .json(&serde_json::json!({}))
            .send()
            .await?;

        let (status, body) = Self::read_response(response, "signed upload").await?;
        if !status.is_success() {
            return Err(Error::Api(format!(
                "failed to create signed upload: {status}"
            )));
        }

        let data: SignedUploadResponse = serde_json::from_slice(&body)
            .map_err(|_| Error::Api("signed upload response was invalid".to_string()))?;
        let signed_url = self
            .validate_signed_object_url(&data.url, "object/upload/sign", bucket, object_path, false)
            .map_err(|_| {
                Error::Api("signed upload URL was not returned by Supabase Storage".to_string())
            })?;
        let token = reqwest::Url::parse(&signed_url)
            .ok()
            .and_then(|url| {
                url.query_pairs()
                    .find(|(name, _)| name == "token")
                    .map(|(_, value)| value.into_owned())
            })
            .filter(|token| !token.is_empty())
            .ok_or_else(|| Error::Api("signed upload token was not returned".to_string()))?;

        Ok(SignedUpload { signed_url, token })
    }

    pub async fn object_info(&self, bucket: &str, object_path: &str) -> Result<ObjectInfo, Error> {
        let url = self.object_url("object/info", bucket, object_path)?;
        let response = self.auth_headers(self.client.get(url)).send().await?;
        let (status, body) = Self::read_response(response, "object info").await?;
        if !status.is_success() {
            return Err(Error::Api(format!("failed to read object info: {status}")));
        }

        let data: ObjectInfoResponse = serde_json::from_slice(&body)
            .map_err(|_| Error::Api("storage object info response was invalid".to_string()))?;
        let size_bytes = data
            .size
            .or_else(|| data.metadata.as_ref().and_then(|metadata| metadata.size))
            .ok_or_else(|| Error::Api("storage object size was not returned".to_string()))?;
        let content_type = data
            .content_type
            .or_else(|| {
                data.metadata
                    .as_ref()
                    .and_then(|metadata| metadata.mimetype.clone())
            })
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                Error::Api("storage object content type was not returned".to_string())
            })?;
        let ciphertext_sha256 = data
            .user_metadata
            .as_ref()
            .and_then(|metadata| metadata.ciphertext_sha256.clone())
            .or_else(|| {
                data.metadata
                    .as_ref()
                    .and_then(|metadata| metadata.ciphertext_sha256.clone())
            })
            .filter(|value| {
                value.len() == 64
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            })
            .ok_or_else(|| {
                Error::Api("storage object ciphertext checksum was not returned".to_string())
            })?;
        let format_version = data
            .user_metadata
            .as_ref()
            .and_then(|metadata| metadata.format_version)
            .or_else(|| {
                data.metadata
                    .as_ref()
                    .and_then(|metadata| metadata.format_version)
            })
            .filter(|version| *version == 1)
            .ok_or_else(|| {
                Error::Api("storage object format version was not returned".to_string())
            })?;

        Ok(ObjectInfo {
            size_bytes,
            content_type,
            ciphertext_sha256,
            format_version,
        })
    }

    pub async fn object_sha256(
        &self,
        bucket: &str,
        object_path: &str,
        expected_size_bytes: u64,
    ) -> Result<String, Error> {
        let url = self.object_url("object/authenticated", bucket, object_path)?;
        let response = self
            .auth_headers(self.client.get(url))
            .timeout(OBJECT_VERIFICATION_TIMEOUT)
            .send()
            .await?;
        if !response.status().is_success() {
            let (status, _) = Self::read_response(response, "object verification").await?;
            return Err(Error::Api(format!(
                "failed to verify storage object: {status}"
            )));
        }
        if response
            .content_length()
            .is_some_and(|size| size != expected_size_bytes)
        {
            return Err(Error::Api(
                "storage object size did not match its reservation".to_string(),
            ));
        }

        let mut size_bytes = 0_u64;
        let mut digest = Sha256::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            size_bytes = size_bytes
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| Error::Api("storage object was too large".to_string()))?;
            if size_bytes > expected_size_bytes {
                return Err(Error::Api(
                    "storage object size did not match its reservation".to_string(),
                ));
            }
            digest.update(&chunk);
        }
        if size_bytes != expected_size_bytes {
            return Err(Error::Api(
                "storage object size did not match its reservation".to_string(),
            ));
        }

        let checksum = digest.finalize();
        let mut encoded = String::with_capacity(checksum.len() * 2);
        for byte in checksum {
            encoded.push(char::from(b"0123456789abcdef"[(byte >> 4) as usize]));
            encoded.push(char::from(b"0123456789abcdef"[(byte & 0x0f) as usize]));
        }
        Ok(encoded)
    }

    pub async fn delete_file(&self, bucket: &str, object_path: &str) -> Result<(), Error> {
        let url = self.object_url("object", bucket, object_path)?;
        let response = self.auth_headers(self.client.delete(url)).send().await?;
        let (status, body) = Self::read_response(response, "delete").await?;
        if !status.is_success() {
            let code = serde_json::from_slice::<StorageErrorResponse>(&body)
                .ok()
                .and_then(|error| error.code);
            if status == reqwest::StatusCode::NOT_FOUND && code.as_deref() == Some("NoSuchKey") {
                return Ok(());
            }
            return Err(Error::Api(format!("failed to delete file: {status}")));
        }

        Ok(())
    }
}
