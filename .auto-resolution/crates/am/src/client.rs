use crate::{
    AmModel, Error, ErrorResponse, InitRequest, InitResponse, ResetResponse, Secret, ServerStatus,
    ShutdownResponse, UnloadResponse,
};
use reqwest::{Response, StatusCode};

#[derive(Clone)]
pub struct Client {
    client: reqwest::Client,
    base_url: String,
}

impl Client {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }

    pub fn with_client(client: reqwest::Client, base_url: impl Into<String>) -> Self {
        Self {
            client,
            base_url: base_url.into(),
        }
    }

    pub async fn status(&self) -> Result<ServerStatus, Error> {
        let url = format!("{}/status", self.base_url);
        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(self.handle_error_response(response).await)
        }
    }

    pub async fn wait_for_ready(
        &self,
        max_wait_time: Option<u32>,
        poll_interval: Option<u32>,
    ) -> Result<ServerStatus, Error> {
        let url = format!("{}/waitForReady", self.base_url);
        let mut request = self.client.get(&url);

        if let Some(max_wait) = max_wait_time {
            request = request.query(&[("maxWaitTime", max_wait)]);
        }
        if let Some(poll) = poll_interval {
            request = request.query(&[("pollInterval", poll)]);
        }

        let response = request.send().await?;

        match response.status() {
            StatusCode::OK => Ok(response.json().await?),
            StatusCode::BAD_REQUEST | StatusCode::REQUEST_TIMEOUT => {
                Err(self.handle_error_response(response).await)
            }
            _ => Err(Error::UnexpectedResponse),
        }
    }

    pub async fn init(&self, request: InitRequest) -> Result<InitResponse, Error> {
        if !request.api_key.expose().starts_with("ax_") {
            return Err(Error::InvalidApiKey);
        }

        let url = format!("{}/init", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;

        match response.status() {
            StatusCode::OK | StatusCode::BAD_REQUEST | StatusCode::CONFLICT => {
                Ok(response.json().await?)
            }
            _ => Err(Error::UnexpectedResponse),
        }
    }

    pub async fn reset(&self) -> Result<ResetResponse, Error> {
        let url = format!("{}/reset", self.base_url);
        let response = self.client.post(&url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(self.handle_error_response(response).await)
        }
    }

    pub async fn unload(&self) -> Result<UnloadResponse, Error> {
        let url = format!("{}/unload", self.base_url);
        let response = self.client.post(&url).send().await?;

        match response.status() {
            StatusCode::OK => Ok(response.json().await?),
            StatusCode::BAD_REQUEST => Err(self.handle_error_response(response).await),
            _ => Err(Error::UnexpectedResponse),
        }
    }

    pub async fn shutdown(&self) -> Result<ShutdownResponse, Error> {
        let url = format!("{}/shutdown", self.base_url);
        let response = self.client.post(&url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(self.handle_error_response(response).await)
        }
    }

    async fn handle_error_response(&self, response: Response) -> Error {
        if let Ok(error_response) = response.json::<ErrorResponse>().await {
            Error::ServerError {
                status: error_response.status.to_string(),
                message: error_response.message,
            }
        } else {
            Error::UnexpectedResponse
        }
    }
}

impl InitRequest {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: Secret::new(api_key),
            model: None,
            model_token: None,
            download_base: None,
            model_repo: None,
            model_folder: None,
            tokenizer_folder: None,
            fast_load: None,
            fast_load_encoder_compute_units: None,
            fast_load_decoder_compute_units: None,
            model_vad: None,
            verbose: None,
            custom_vocabulary: None,
            custom_vocabulary_model_folder: None,
        }
    }

    pub fn with_model(mut self, model: AmModel, base_dir: impl AsRef<std::path::Path>) -> Self {
        self.model = Some(model.model_dir().to_string());
        self.model_repo = Some(model.repo_name().to_string());
        self.model_folder = Some(
            base_dir
                .as_ref()
                .join(model.model_dir())
                .to_string_lossy()
                .to_string(),
        );

        match model {
            crate::AmModel::ParakeetV2 => {
                self.custom_vocabulary = Some(vec![]);
                self.custom_vocabulary_model_folder = Some(
                    base_dir
                        .as_ref()
                        .join("parakeet-tdt_ctc-110m")
                        .to_string_lossy()
                        .to_string(),
                );
            }
            _ => {
                self.custom_vocabulary = None;
            }
        }

        self
    }
}
