use serde::Deserialize;

use crate::error::Error;
use crate::types::{
    CreateStatusReportRequest, CreateStatusReportUpdateRequest, Incident, StatusReport,
    StatusReportUpdate, UpdateIncidentRequest,
};

#[derive(Debug, Deserialize)]
pub(crate) struct ApiErrorResponse {
    #[allow(dead_code)]
    pub code: String,
    pub message: String,
}

#[derive(Default)]
pub struct OpenStatusClientBuilder {
    api_key: Option<String>,
    api_base: Option<String>,
}

impl OpenStatusClientBuilder {
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = Some(api_base.into());
        self
    }

    pub fn build(self) -> OpenStatusClient {
        let mut headers = reqwest::header::HeaderMap::new();

        let api_key = self.api_key.expect("api_key is required");
        let mut key_value = reqwest::header::HeaderValue::from_str(&api_key).unwrap();
        key_value.set_sensitive(true);
        headers.insert("x-openstatus-key", key_value);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        let api_base = self
            .api_base
            .unwrap_or_else(|| "https://api.openstatus.dev/v1".to_string());

        OpenStatusClient {
            client,
            api_base: api_base.parse().unwrap(),
        }
    }
}

#[derive(Clone)]
pub struct OpenStatusClient {
    client: reqwest::Client,
    api_base: url::Url,
}

impl OpenStatusClient {
    pub fn builder() -> OpenStatusClientBuilder {
        OpenStatusClientBuilder::default()
    }

    pub fn api_base(&self) -> &url::Url {
        &self.api_base
    }

    pub async fn list_incidents(&self) -> Result<Vec<Incident>, Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("{}/incident", self.api_base.path()));

        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiErrorResponse = response.json().await?;
            Err(Error::ListIncidentsError(error.message))
        }
    }

    pub async fn get_incident(&self, id: i64) -> Result<Incident, Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("{}/incident/{}", self.api_base.path(), id));

        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiErrorResponse = response.json().await?;
            Err(Error::GetIncidentError(error.message))
        }
    }

    pub async fn update_incident(
        &self,
        id: i64,
        req: UpdateIncidentRequest,
    ) -> Result<Incident, Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("{}/incident/{}", self.api_base.path(), id));

        let response = self.client.put(url).json(&req).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiErrorResponse = response.json().await?;
            Err(Error::UpdateIncidentError(error.message))
        }
    }

    pub async fn acknowledge_incident(
        &self,
        id: i64,
        at: impl Into<String>,
    ) -> Result<Incident, Error> {
        self.update_incident(
            id,
            UpdateIncidentRequest {
                acknowledged_at: Some(at.into()),
                resolved_at: None,
            },
        )
        .await
    }

    pub async fn resolve_incident(
        &self,
        id: i64,
        at: impl Into<String>,
    ) -> Result<Incident, Error> {
        self.update_incident(
            id,
            UpdateIncidentRequest {
                acknowledged_at: None,
                resolved_at: Some(at.into()),
            },
        )
        .await
    }

    pub async fn list_status_reports(&self) -> Result<Vec<StatusReport>, Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("{}/status_report", self.api_base.path()));

        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiErrorResponse = response.json().await?;
            Err(Error::ListStatusReportsError(error.message))
        }
    }

    pub async fn get_status_report(&self, id: i64) -> Result<StatusReport, Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("{}/status_report/{}", self.api_base.path(), id));

        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiErrorResponse = response.json().await?;
            Err(Error::GetStatusReportError(error.message))
        }
    }

    pub async fn create_status_report(
        &self,
        req: CreateStatusReportRequest,
    ) -> Result<StatusReport, Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("{}/status_report", self.api_base.path()));

        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiErrorResponse = response.json().await?;
            Err(Error::CreateStatusReportError(error.message))
        }
    }

    pub async fn delete_status_report(&self, id: i64) -> Result<(), Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("{}/status_report/{}", self.api_base.path(), id));

        let response = self.client.delete(url).send().await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let error: ApiErrorResponse = response.json().await?;
            Err(Error::DeleteStatusReportError(error.message))
        }
    }

    pub async fn create_status_report_update(
        &self,
        req: CreateStatusReportUpdateRequest,
    ) -> Result<StatusReportUpdate, Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("{}/status_report_update", self.api_base.path()));

        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: ApiErrorResponse = response.json().await?;
            Err(Error::CreateStatusReportUpdateError(error.message))
        }
    }
}
