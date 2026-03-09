use crate::client::{NangoClient, append_query, check_response, parse_response};
use crate::common_derives;

common_derives! {
    #[derive(strum::AsRefStr)]
    pub enum RecordFilter {
        #[serde(rename = "added")]
        #[strum(serialize = "added")]
        Added,
        #[serde(rename = "updated")]
        #[strum(serialize = "updated")]
        Updated,
        #[serde(rename = "deleted")]
        #[strum(serialize = "deleted")]
        Deleted,
    }
}

common_derives! {
    pub struct NangoRecordMetadata {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub deleted_at: Option<String>,
        pub last_action: String,
        pub first_seen_at: String,
        pub last_modified_at: String,
        pub cursor: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub pruned_at: Option<String>,
    }
}

common_derives! {
    pub struct GetRecordsParams {
        #[serde(skip)]
        pub connection_id: String,
        #[serde(skip)]
        pub provider_config_key: String,
        #[serde(skip)]
        pub model: String,
        #[serde(skip)]
        pub cursor: Option<String>,
        #[serde(skip)]
        pub limit: Option<i32>,
        #[serde(skip)]
        pub filter: Option<RecordFilter>,
        #[serde(skip)]
        pub modified_after: Option<String>,
        #[serde(skip)]
        pub ids: Option<Vec<String>>,
        #[serde(skip)]
        pub variant: Option<String>,
    }
}

common_derives! {
    pub struct GetRecordsResponse {
        pub records: Vec<serde_json::Value>,
        pub next_cursor: Option<String>,
    }
}

common_derives! {
    pub struct PruneRecordsParams {
        #[serde(skip)]
        pub connection_id: String,
        #[serde(skip)]
        pub provider_config_key: String,
        #[serde(skip)]
        pub model: String,
        #[serde(skip)]
        pub variant: Option<String>,
    }
}

common_derives! {
    pub enum SyncMode {
        #[serde(rename = "incremental")]
        Incremental,
        #[serde(rename = "full_refresh")]
        FullRefresh,
        #[serde(rename = "full_refresh_and_clear_cache")]
        FullRefreshAndClearCache,
    }
}

common_derives! {
    pub struct TriggerSyncRequest {
        pub provider_config_key: String,
        pub syncs: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub connection_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub sync_mode: Option<SyncMode>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub full_resync: Option<bool>,
    }
}

common_derives! {
    pub struct StartSyncRequest {
        pub provider_config_key: String,
        pub syncs: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub connection_id: Option<String>,
    }
}

common_derives! {
    pub struct PauseSyncRequest {
        pub provider_config_key: String,
        pub syncs: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub connection_id: Option<String>,
    }
}

common_derives! {
    pub struct SyncStatusParams {
        #[serde(skip)]
        pub provider_config_key: String,
        #[serde(skip)]
        pub syncs: String,
        #[serde(skip)]
        pub connection_id: Option<String>,
    }
}

common_derives! {
    pub struct SyncStatusResponse {
        pub syncs: Vec<SyncStatus>,
    }
}

common_derives! {
    pub struct SyncStatus {
        pub id: String,
        pub name: String,
        pub status: String,
        pub r#type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub frequency: Option<String>,
        #[serde(rename = "finishedAt")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub finished_at: Option<String>,
        #[serde(rename = "nextScheduledSyncAt")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub next_scheduled_sync_at: Option<String>,
        #[serde(rename = "latestResult")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub latest_result: Option<serde_json::Value>,
    }
}

common_derives! {
    pub struct UpdateConnectionFrequencyRequest {
        pub provider_config_key: String,
        pub connection_id: String,
        pub sync_name: String,
        pub frequency: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub sync_variant: Option<String>,
    }
}

common_derives! {
    pub struct UpdateConnectionFrequencyResponse {
        pub frequency: String,
    }
}

common_derives! {
    pub struct EnvironmentVariable {
        pub name: String,
        pub value: String,
    }
}

common_derives! {
    pub struct CreateVariantRequest {
        pub provider_config_key: String,
        pub connection_id: String,
    }
}

common_derives! {
    pub struct CreateVariantResponse {
        pub id: String,
        pub name: String,
        pub variant: String,
    }
}

common_derives! {
    pub struct DeleteVariantRequest {
        pub provider_config_key: String,
        pub connection_id: String,
    }
}

impl NangoClient {
    pub async fn get_records(
        &self,
        params: GetRecordsParams,
    ) -> Result<GetRecordsResponse, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/records");

        append_query(&mut url, "model", &params.model);
        if let Some(ref v) = params.cursor {
            append_query(&mut url, "cursor", v);
        }
        if let Some(v) = params.limit {
            append_query(&mut url, "limit", &v.to_string());
        }
        if let Some(ref v) = params.filter {
            append_query(&mut url, "filter", v.as_ref());
        }
        if let Some(ref v) = params.modified_after {
            append_query(&mut url, "modified_after", v);
        }
        if let Some(ref ids) = params.ids {
            for id in ids {
                append_query(&mut url, "ids", id);
            }
        }
        if let Some(ref v) = params.variant {
            append_query(&mut url, "variant", v);
        }

        let response = self
            .client
            .get(url)
            .header("Connection-Id", &params.connection_id)
            .header("Provider-Config-Key", &params.provider_config_key)
            .send()
            .await?;
        parse_response(response).await
    }

    pub async fn prune_records(&self, params: PruneRecordsParams) -> Result<(), crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/records/prune");

        append_query(&mut url, "model", &params.model);
        if let Some(ref v) = params.variant {
            append_query(&mut url, "variant", v);
        }

        let response = self
            .client
            .patch(url)
            .header("Connection-Id", &params.connection_id)
            .header("Provider-Config-Key", &params.provider_config_key)
            .send()
            .await?;
        check_response(response).await?;
        Ok(())
    }

    pub async fn trigger_sync(&self, req: TriggerSyncRequest) -> Result<(), crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/sync/trigger");

        let response = self.client.post(url).json(&req).send().await?;
        check_response(response).await?;
        Ok(())
    }

    pub async fn start_sync(&self, req: StartSyncRequest) -> Result<(), crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/sync/start");

        let response = self.client.post(url).json(&req).send().await?;
        check_response(response).await?;
        Ok(())
    }

    pub async fn pause_sync(&self, req: PauseSyncRequest) -> Result<(), crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/sync/pause");

        let response = self.client.post(url).json(&req).send().await?;
        check_response(response).await?;
        Ok(())
    }

    pub async fn sync_status(
        &self,
        params: SyncStatusParams,
    ) -> Result<SyncStatusResponse, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/sync/status");

        append_query(&mut url, "provider_config_key", &params.provider_config_key);
        append_query(&mut url, "syncs", &params.syncs);
        if let Some(ref v) = params.connection_id {
            append_query(&mut url, "connection_id", v);
        }

        let response = self.client.get(url).send().await?;
        parse_response(response).await
    }

    pub async fn update_connection_frequency(
        &self,
        req: UpdateConnectionFrequencyRequest,
    ) -> Result<UpdateConnectionFrequencyResponse, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/sync/update-connection-frequency");

        let response = self.client.put(url).json(&req).send().await?;
        parse_response(response).await
    }

    pub async fn get_environment_variables(
        &self,
    ) -> Result<Vec<EnvironmentVariable>, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/environment-variables");

        let response = self.client.get(url).send().await?;
        parse_response(response).await
    }

    pub async fn create_variant(
        &self,
        name: impl std::fmt::Display,
        variant: impl std::fmt::Display,
        req: CreateVariantRequest,
    ) -> Result<CreateVariantResponse, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("/sync/{}/variant/{}", name, variant));

        let response = self.client.post(url).json(&req).send().await?;
        parse_response(response).await
    }

    pub async fn delete_variant(
        &self,
        name: impl std::fmt::Display,
        variant: impl std::fmt::Display,
        req: DeleteVariantRequest,
    ) -> Result<(), crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("/sync/{}/variant/{}", name, variant));

        let response = self.client.delete(url).json(&req).send().await?;
        check_response(response).await?;
        Ok(())
    }
}
