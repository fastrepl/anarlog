use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
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

    pub async fn create_signed_url(
        &self,
        bucket: &str,
        object_path: &str,
        expires_in_seconds: u64,
    ) -> Result<String, Error> {
        let response = self
            .auth_headers(self.client.post(format!(
                "{}/storage/v1/object/sign/{bucket}/{object_path}",
                self.base_url
            )))
            .json(&serde_json::json!({ "expiresIn": expires_in_seconds }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Api(format!(
                "failed to create signed URL: {status} {body}"
            )));
        }

        let data: SignedUrlResponse = response.json().await?;
        let signed_url = data
            .signed_url
            .ok_or_else(|| Error::Api("signed URL not returned from Supabase".into()))?;

        if signed_url.starts_with("http") {
            Ok(signed_url)
        } else {
            Ok(format!("{}/storage/v1{signed_url}", self.base_url))
        }
    }

    pub async fn delete_file(&self, bucket: &str, object_path: &str) -> Result<(), Error> {
        let response = self
            .auth_headers(self.client.delete(format!(
                "{}/storage/v1/object/{bucket}/{object_path}",
                self.base_url
            )))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Api(format!(
                "failed to delete file: {status} {body}"
            )));
        }

        Ok(())
    }
}
