use std::ops::Deref;

use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::complete_multipart_upload::CompleteMultipartUploadError;
use aws_sdk_s3::operation::create_bucket::CreateBucketError;
use aws_sdk_s3::operation::create_multipart_upload::CreateMultipartUploadError;
use aws_sdk_s3::operation::delete_object::DeleteObjectError;
use aws_sdk_s3::operation::delete_objects::DeleteObjectsError;
use aws_sdk_s3::operation::get_object::GetObjectError;
use aws_sdk_s3::operation::head_object::HeadObjectError;
use aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Error;
use aws_sdk_s3::operation::put_object::PutObjectError;
use aws_sdk_s3::operation::upload_part::UploadPartError;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::{AggregatedBytes, ByteStreamError};
use sha2::{Digest, Sha256};

#[derive(Default)]
pub struct ClientBuilder {
    endpoint_url: Option<String>,
    bucket: Option<String>,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
}

#[derive(Clone)]
pub struct Client {
    s3: aws_sdk_s3::Client,
    bucket: String,
}

pub struct UserClient<'a> {
    client: &'a Client,
    user_id: String,
}

pub struct Config {
    pub endpoint_url: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInfo {
    pub size_bytes: u64,
    pub content_type: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Error while deleting object: {0}")]
    DeleteObjectError(#[from] SdkError<DeleteObjectError>),
    #[error("Error while deleting objects: {0}")]
    DeleteObjectsError(#[from] SdkError<DeleteObjectsError>),
    #[error("Error while getting object: {0}")]
    GetObjectError(#[from] SdkError<GetObjectError>),
    #[error("Error while inspecting object: {0}")]
    HeadObjectError(#[from] SdkError<HeadObjectError>),
    #[error("Error while listing objects: {0}")]
    ListObjectsError(#[from] SdkError<ListObjectsV2Error>),
    #[error("R2 object prefix exceeded the deletion limit")]
    PrefixDeleteLimitExceeded,
    #[error("R2 object prefix deletion returned errors")]
    PrefixDeleteFailed,
    #[error("Error while putting object: {0}")]
    PutObjectError(#[from] SdkError<PutObjectError>),
    #[error("Error while creating bucket: {0}")]
    CreateBucketError(#[from] SdkError<CreateBucketError>),
    #[error("Error while collecting bytes: {0}")]
    CollectBytesError(#[from] ByteStreamError),
    #[error("Error while creating multipart upload: {0}")]
    CreateMultipartUploadError(#[from] SdkError<CreateMultipartUploadError>),
    #[error("Error while uploading part: {0}")]
    UploadPartError(#[from] SdkError<UploadPartError>),
    #[error("Error while completing multipart upload: {0}")]
    CompleteMultipartUploadError(#[from] SdkError<CompleteMultipartUploadError>),
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn endpoint_url(mut self, endpoint_url: impl Into<String>) -> Self {
        self.endpoint_url = Some(endpoint_url.into());
        self
    }

    pub fn bucket(mut self, bucket: impl Into<String>) -> Self {
        self.bucket = Some(bucket.into());
        self
    }

    pub fn credentials(mut self, id: impl Into<String>, secret: impl Into<String>) -> Self {
        self.access_key_id = Some(id.into());
        self.secret_access_key = Some(secret.into());
        self
    }

    pub async fn build(self) -> Client {
        let creds = aws_credential_types::Credentials::from_keys(
            self.access_key_id.unwrap(),
            self.secret_access_key.unwrap(),
            None,
        );

        let cfg = aws_config::from_env()
            .endpoint_url(self.endpoint_url.unwrap())
            // https://www.tigrisdata.com/docs/concepts/regions/
            .region(aws_config::Region::new("auto"))
            .credentials_provider(creds)
            .load()
            .await;

        let s3 = aws_sdk_s3::Client::new(&cfg);

        Client {
            s3,
            bucket: self.bucket.unwrap(),
        }
    }

    pub fn build_sync(self) -> Client {
        let creds = aws_credential_types::Credentials::from_keys(
            self.access_key_id.expect("S3 access key ID is configured"),
            self.secret_access_key
                .expect("S3 secret access key is configured"),
            None,
        );
        let config = aws_sdk_s3::Config::builder()
            .behavior_version_latest()
            .endpoint_url(self.endpoint_url.expect("S3 endpoint is configured"))
            .region(aws_config::Region::new("auto"))
            .credentials_provider(creds)
            .force_path_style(true)
            .build();
        Client {
            s3: aws_sdk_s3::Client::from_conf(config),
            bucket: self.bucket.expect("S3 bucket is configured"),
        }
    }
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub fn for_user(&self, user_id: impl Into<String>) -> UserClient<'_> {
        UserClient {
            client: self,
            user_id: user_id.into(),
        }
    }

    pub async fn get_bucket(&self) -> bool {
        self.s3
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .is_ok()
    }

    pub async fn create_bucket(&self) -> Result<(), ApiError> {
        let _ = self.s3.create_bucket().bucket(&self.bucket).send().await?;
        Ok(())
    }

    pub async fn presigned_put_object(&self, key: &str) -> Result<String, ApiError> {
        let config = PresigningConfig::builder()
            .expires_in(std::time::Duration::from_secs(60 * 60))
            .build()
            .unwrap();
        let url = self
            .s3
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type("application/octet-stream")
            .presigned(config)
            .await?
            .uri()
            .to_string();
        Ok(url)
    }

    pub async fn presigned_get_object(&self, key: &str) -> Result<String, ApiError> {
        let config = PresigningConfig::builder()
            .expires_in(std::time::Duration::from_secs(15 * 60))
            .build()
            .unwrap();
        let url = self
            .s3
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(config)
            .await?
            .uri()
            .to_string();
        Ok(url)
    }

    pub async fn head_object(&self, key: &str) -> Result<bool, ApiError> {
        match self
            .s3
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(error) if error.as_service_error().is_some_and(|e| e.is_not_found()) => Ok(false),
            Err(error) => Err(ApiError::HeadObjectError(error)),
        }
    }

    pub async fn object_info(&self, key: &str) -> Result<Option<ObjectInfo>, ApiError> {
        match self
            .s3
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(response) => Ok(Some(ObjectInfo {
                size_bytes: response.content_length().unwrap_or_default().max(0) as u64,
                content_type: response.content_type().map(ToString::to_string),
            })),
            Err(error) if error.as_service_error().is_some_and(|e| e.is_not_found()) => Ok(None),
            Err(error) => Err(ApiError::HeadObjectError(error)),
        }
    }

    pub async fn object_sha256(&self, key: &str) -> Result<String, ApiError> {
        let response = self
            .s3
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        let bytes = response.body.collect().await?;
        Ok(Sha256::digest(bytes.into_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    pub async fn delete_object(&self, key: &str) -> Result<(), ApiError> {
        self.s3
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        Ok(())
    }

    pub async fn delete_prefix(&self, prefix: &str, max_objects: usize) -> Result<usize, ApiError> {
        let mut continuation_token = None;
        let mut deleted = 0usize;
        loop {
            let response = self
                .s3
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix)
                .max_keys(1000)
                .set_continuation_token(continuation_token)
                .send()
                .await?;
            let objects = response.contents();
            deleted = deleted.saturating_add(objects.len());
            if deleted > max_objects {
                return Err(ApiError::PrefixDeleteLimitExceeded);
            }
            if !objects.is_empty() {
                let identifiers = objects
                    .iter()
                    .filter_map(|object| object.key())
                    .map(|key| {
                        aws_sdk_s3::types::ObjectIdentifier::builder()
                            .key(key)
                            .build()
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .expect("object keys are present in an S3 listing");
                let deletion = aws_sdk_s3::types::Delete::builder()
                    .set_objects(Some(identifiers))
                    .quiet(true)
                    .build()
                    .expect("delete request has object identifiers");
                let result = self
                    .s3
                    .delete_objects()
                    .bucket(&self.bucket)
                    .delete(deletion)
                    .send()
                    .await?;
                if !result.errors().is_empty() {
                    return Err(ApiError::PrefixDeleteFailed);
                }
            }
            if !response.is_truncated().unwrap_or(false) {
                return Ok(deleted);
            }
            continuation_token = response.next_continuation_token().map(ToString::to_string);
            if continuation_token.is_none() {
                return Err(ApiError::PrefixDeleteFailed);
            }
        }
    }
}

impl Deref for UserClient<'_> {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        self.client
    }
}

impl<'a> UserClient<'a> {
    fn folder(&self) -> String {
        format!("user_{}", self.user_id)
    }

    pub async fn create_multipart_upload(&self, file_name: &str) -> Result<String, ApiError> {
        let res = self
            .s3
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(format!("{}/{}", self.folder(), file_name))
            .send()
            .await?;

        let id = res.upload_id().unwrap().to_string();
        Ok(id)
    }

    pub async fn complete_multipart_upload(
        &self,
        file_name: &str,
        upload_id: &str,
        parts: Vec<String>,
    ) -> Result<(), ApiError> {
        let payload = aws_sdk_s3::types::CompletedMultipartUpload::builder()
            .set_parts(Some(
                parts
                    .into_iter()
                    .map(|p| aws_sdk_s3::types::CompletedPart::builder().e_tag(p).build())
                    .collect(),
            ))
            .build();

        let _res = self
            .s3
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(format!("{}/{}", self.folder(), file_name))
            .upload_id(upload_id)
            .multipart_upload(payload)
            .send()
            .await?;

        Ok(())
    }

    pub async fn presigned_url_for_upload(&self, file_name: &str) -> Result<String, ApiError> {
        let config = PresigningConfig::builder()
            .expires_in(std::time::Duration::from_secs(60 * 30))
            .build()
            .unwrap();

        let url = self
            .s3
            .put_object()
            .bucket(&self.bucket)
            .key(format!("{}/{}", self.folder(), file_name))
            .presigned(config)
            .await?
            .uri()
            .to_string();

        Ok(url)
    }

    pub async fn presigned_url_for_multipart_upload(
        &self,
        file_name: &str,
        upload_id: &str,
        num_parts: usize,
    ) -> Result<Vec<String>, ApiError> {
        let config = PresigningConfig::builder()
            .expires_in(std::time::Duration::from_secs(60 * 30))
            .build()
            .unwrap();

        let mut urls = Vec::with_capacity(num_parts);

        for part_number in 1..=num_parts {
            let presigned_req = self
                .s3
                .upload_part()
                .bucket(&self.bucket)
                .key(format!("{}/{}", self.folder(), file_name))
                .upload_id(upload_id)
                .checksum_algorithm(aws_sdk_s3::types::ChecksumAlgorithm::Crc32)
                .part_number(part_number as i32)
                .presigned(config.clone())
                .await?;

            urls.push(presigned_req.uri().to_string());
        }

        Ok(urls)
    }

    pub async fn presigned_url_for_download(&self, file_name: &str) -> Result<String, ApiError> {
        let config = PresigningConfig::builder()
            .expires_in(std::time::Duration::from_secs(60 * 30))
            .build()
            .unwrap();

        let url = self
            .s3
            .get_object()
            .bucket(&self.bucket)
            .key(format!("{}/{}", self.folder(), file_name))
            .presigned(config)
            .await?
            .uri()
            .to_string();

        Ok(url)
    }

    pub async fn get(&self, file_name: &str) -> Result<AggregatedBytes, ApiError> {
        let res = self
            .s3
            .get_object()
            .bucket(&self.bucket)
            .key(format!("{}/{}", self.folder(), file_name))
            .send()
            .await?;

        let data = res.body.collect().await?;
        Ok(data)
    }

    pub async fn put(
        &self,
        file_name: &str,
        content: impl Into<aws_sdk_s3::primitives::ByteStream>,
    ) -> Result<(), ApiError> {
        let _ = self
            .s3
            .put_object()
            .bucket(&self.bucket)
            .key(format!("{}/{}", self.folder(), file_name))
            .body(content.into())
            .content_type("audio/mpeg")
            .send()
            .await?;

        Ok(())
    }

    pub async fn delete(&self, file_name: &str) -> Result<(), ApiError> {
        let _ = self
            .s3
            .delete_object()
            .bucket(&self.bucket)
            .key(format!("{}/{}", self.folder(), file_name))
            .send()
            .await?;

        Ok(())
    }
}
