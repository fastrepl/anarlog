use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::OnceCell;
use uuid::Uuid;

use crate::{
    config::CloudsyncCleanupConfig,
    error::{Result, SubscriptionError},
};

const MAX_E2EE_WORKSPACES: usize = 1_000;

#[derive(Clone)]
pub(crate) struct CloudsyncCleanupClient {
    config: CloudsyncCleanupConfig,
    client: reqwest::Client,
    database_name: Arc<OnceCell<String>>,
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedDatabase {
    #[serde(default)]
    managed_database_id: Option<String>,
    database_name: String,
    project_id: String,
}

#[derive(Serialize)]
struct SqlRequest<'a> {
    database: &'a str,
    sql: &'a str,
}

impl CloudsyncCleanupClient {
    pub(crate) fn new(config: CloudsyncCleanupConfig) -> Self {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .expect("CloudSync cleanup HTTP client must build");
        Self {
            config,
            client,
            database_name: Arc::new(OnceCell::new()),
        }
    }

    pub(crate) async fn purge_and_confirm(&self, workspace_ids: &[String]) -> Result<()> {
        if workspace_ids.is_empty() || workspace_ids.len() > MAX_E2EE_WORKSPACES {
            return Err(invalid_response("CloudSync cleanup workspace scope"));
        }
        let mut previous_workspace_id: Option<&str> = None;
        for workspace_id in workspace_ids {
            let parsed = Uuid::parse_str(workspace_id)
                .map_err(|_| invalid_response("CloudSync cleanup workspace scope"))?;
            if parsed.to_string() != workspace_id.as_str()
                || previous_workspace_id.is_some_and(|previous| previous >= workspace_id.as_str())
            {
                return Err(invalid_response("CloudSync cleanup workspace scope"));
            }
            previous_workspace_id = Some(workspace_id.as_str());
        }
        let database_name = self
            .database_name
            .get_or_try_init(|| self.fetch_database_name())
            .await?;
        let quoted_ids = workspace_ids
            .iter()
            .map(|workspace_id| format!("'{workspace_id}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let delete_sql = format!("DELETE FROM e2ee_records WHERE workspace_id IN ({quoted_ids})");
        self.run_sql(database_name, &delete_sql).await?;

        let confirm_sql = format!(
            "SELECT COUNT(*) AS remaining FROM e2ee_records WHERE workspace_id IN ({quoted_ids})"
        );
        let rows = self.run_sql(database_name, &confirm_sql).await?;
        let remaining = rows
            .as_array()
            .filter(|rows| rows.len() == 1)
            .and_then(|rows| rows[0].get("remaining"))
            .and_then(parse_count)
            .ok_or_else(|| invalid_response("CloudSync cleanup confirmation"))?;
        if remaining != 0 {
            return Err(invalid_response("CloudSync cleanup confirmation"));
        }
        Ok(())
    }

    async fn fetch_database_name(&self) -> Result<String> {
        let response = self
            .client
            .get(format!(
                "{}/v1/databases/{}",
                self.config.management_url,
                urlencoding::encode(&self.config.managed_database_id)
            ))
            .bearer_auth(&self.config.management_api_key)
            .send()
            .await
            .map_err(request_error)?;
        if !response.status().is_success() {
            return Err(upstream_status("CloudSync management", response.status()));
        }
        let database = response
            .json::<Envelope<ManagedDatabase>>()
            .await
            .map_err(|_| invalid_response("CloudSync management"))?
            .data;
        if database
            .managed_database_id
            .as_deref()
            .is_some_and(|database_id| database_id != self.config.managed_database_id)
            || database.project_id != self.config.project_id
            || database.database_name.trim().is_empty()
            || database.database_name.len() > 255
            || database.database_name.chars().any(char::is_control)
        {
            return Err(invalid_response("CloudSync management"));
        }
        Ok(database.database_name)
    }

    async fn run_sql(&self, database_name: &str, sql: &str) -> Result<Value> {
        let response = self
            .client
            .post(format!("{}/v2/weblite/sql", self.config.project_url))
            .bearer_auth(&self.config.token_issuer_api_key)
            .json(&SqlRequest {
                database: database_name,
                sql,
            })
            .send()
            .await
            .map_err(request_error)?;
        if !response.status().is_success() {
            return Err(upstream_status("CloudSync Weblite", response.status()));
        }
        response
            .json::<Envelope<Value>>()
            .await
            .map(|envelope| envelope.data)
            .map_err(|_| invalid_response("CloudSync Weblite"))
    }
}

fn parse_count(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn request_error(error: reqwest::Error) -> SubscriptionError {
    tracing::warn!(error = %error.without_url(), "cloudsync_cleanup_request_failed");
    SubscriptionError::Internal("CloudSync cleanup request failed".to_string())
}

fn upstream_status(context: &str, status: reqwest::StatusCode) -> SubscriptionError {
    tracing::warn!(%status, "cloudsync_cleanup_upstream_rejected");
    SubscriptionError::Internal(format!("{context} request was rejected"))
}

fn invalid_response(context: &str) -> SubscriptionError {
    SubscriptionError::Internal(format!("Invalid {context} response"))
}

#[cfg(test)]
mod tests {
    use wiremock::MockServer;

    use super::*;

    #[tokio::test]
    async fn rejects_noncanonical_or_unsorted_workspace_scopes_before_network_io() {
        let server = MockServer::start().await;
        let client = CloudsyncCleanupClient::new(CloudsyncCleanupConfig::for_test(&server.uri()));

        for workspace_ids in [
            vec!["00000000-0000-4000-8000-000000000001' OR 1=1 --".to_string()],
            vec![
                "00000000-0000-4000-8000-000000000002".to_string(),
                "00000000-0000-4000-8000-000000000001".to_string(),
            ],
        ] {
            let error = client.purge_and_confirm(&workspace_ids).await.unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("Invalid CloudSync cleanup workspace scope response")
            );
        }

        assert!(server.received_requests().await.unwrap().is_empty());
    }
}
