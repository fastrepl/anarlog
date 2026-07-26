use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

use crate::error::Error;
use crate::types::{Bot, CreateBotRequest, OutputMedia};

const BASE_URL: &str = "https://us-east-1.recall.ai/api/v1";

#[derive(Clone)]
pub struct RecallClient {
    http: reqwest::Client,
}

impl RecallClient {
    pub fn new(api_key: &str) -> Result<Self, Error> {
        let mut headers = HeaderMap::new();
        let auth_value =
            HeaderValue::from_str(&format!("Token {api_key}")).map_err(|e| Error::Api {
                status: 0,
                body: e.to_string(),
            })?;
        headers.insert(AUTHORIZATION, auth_value);

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self { http })
    }

    pub async fn create_bot(&self, req: CreateBotRequest) -> Result<Bot, Error> {
        let resp = self
            .http
            .post(format!("{BASE_URL}/bot"))
            .json(&req)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Api { status, body });
        }

        Ok(resp.json::<Bot>().await?)
    }

    pub async fn get_bot(&self, bot_id: &str) -> Result<Bot, Error> {
        let resp = self
            .http
            .get(format!("{BASE_URL}/bot/{bot_id}"))
            .send()
            .await?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Api { status, body });
        }

        Ok(resp.json::<Bot>().await?)
    }

    pub async fn output_media(&self, bot_id: &str, req: OutputMedia) -> Result<(), Error> {
        let resp = self
            .http
            .post(format!("{BASE_URL}/bot/{bot_id}/output_media"))
            .json(&req)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Api { status, body });
        }

        Ok(())
    }

    pub async fn remove_bot(&self, bot_id: &str) -> Result<(), Error> {
        let resp = self
            .http
            .post(format!("{BASE_URL}/bot/{bot_id}/leave_call"))
            .send()
            .await?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Api { status, body });
        }

        Ok(())
    }
}
