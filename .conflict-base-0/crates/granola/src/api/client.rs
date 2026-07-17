use crate::api::models::{Document, GranolaResponse};
use crate::api::token::extract_access_token;
use crate::error::{Error, Result};
use serde_json::json;
use std::time::Duration;

const API_URL: &str = "https://api.granola.ai/v2/get-documents";
const USER_AGENT: &str = "Granola/5.354.0";
const CLIENT_VERSION: &str = "5.354.0";
const PAGE_LIMIT: usize = 100;

pub struct GranolaClient {
    client: reqwest::Client,
    access_token: String,
}

impl GranolaClient {
    pub fn new(supabase_content: &[u8], timeout: Duration) -> Result<Self> {
        let access_token = extract_access_token(supabase_content)?;
        let client = reqwest::Client::builder().timeout(timeout).build()?;

        Ok(Self {
            client,
            access_token,
        })
    }

    pub async fn get_documents(&self) -> Result<Vec<Document>> {
        let mut all_documents = Vec::new();
        let mut offset = 0;

        loop {
            let request_body = json!({
                "limit": PAGE_LIMIT,
                "offset": offset,
                "include_last_viewed_panel": true
            });

            let response = self
                .client
                .post(API_URL)
                .header("Authorization", format!("Bearer {}", self.access_token))
                .header("User-Agent", USER_AGENT)
                .header("X-Client-Version", CLIENT_VERSION)
                .header("Content-Type", "application/json")
                .header("Accept", "*/*")
                .json(&request_body)
                .send()
                .await?;

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                let preview = if body.chars().count() > 200 {
                    let truncate_idx = body
                        .char_indices()
                        .nth(200)
                        .map(|(i, _)| i)
                        .unwrap_or(body.len());
                    format!("{}...", &body[..truncate_idx])
                } else {
                    body
                };
                return Err(Error::ApiStatus {
                    status: status.as_u16(),
                    body: preview,
                });
            }

            let response_text = response.text().await?;
            let granola_response: GranolaResponse =
                serde_json::from_str(&response_text).map_err(Error::ApiResponseParse)?;

            let docs_count = granola_response.docs.len();
            all_documents.extend(granola_response.docs);

            if docs_count < PAGE_LIMIT {
                break;
            }

            offset += PAGE_LIMIT;
        }

        Ok(all_documents)
    }
}
