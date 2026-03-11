#[derive(serde::Deserialize)]
pub(crate) struct NangoConnectionRow {
    #[serde(default)]
    pub integration_id: String,
    pub connection_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub last_error_type: Option<String>,
    #[serde(default)]
    pub last_error_description: Option<String>,
    #[serde(default)]
    pub last_error_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(serde::Deserialize)]
pub(crate) struct LookupConnectionRow {
    pub connection_id: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Clone)]
pub(crate) struct SupabaseClient {
    supabase_url: String,
    supabase_anon_key: String,
    supabase_service_role_key: Option<String>,
    http_client: reqwest::Client,
}

impl SupabaseClient {
    pub(crate) fn new(
        supabase_url: impl Into<String>,
        supabase_anon_key: impl Into<String>,
        supabase_service_role_key: Option<String>,
    ) -> Self {
        Self {
            supabase_url: supabase_url.into().trim_end_matches('/').to_string(),
            supabase_anon_key: supabase_anon_key.into(),
            supabase_service_role_key,
            http_client: reqwest::Client::new(),
        }
    }

    pub(crate) fn is_configured(&self) -> bool {
        self.supabase_service_role_key.is_some()
    }

    pub(crate) async fn anon_query(
        &self,
        url: &str,
        auth_token: &str,
    ) -> Result<reqwest::Response, crate::error::NangoError> {
        self.http_client
            .get(url)
            .header("Authorization", format!("Bearer {}", auth_token))
            .header("apikey", &self.supabase_anon_key)
            .send()
            .await
            .map_err(|e| crate::error::NangoError::Internal(e.to_string()))
    }

    fn service_role_key(&self) -> Result<&str, crate::error::NangoError> {
        self.supabase_service_role_key.as_deref().ok_or_else(|| {
            crate::error::NangoError::Internal(
                "supabase_service_role_key not configured".to_string(),
            )
        })
    }

    pub(crate) async fn verify_connection_ownership(
        &self,
        auth_token: &str,
        user_id: &str,
        connection_id: &str,
        integration_id: &str,
    ) -> Result<bool, crate::error::NangoError> {
        let encoded_user_id = urlencoding::encode(user_id);
        let encoded_connection_id = urlencoding::encode(connection_id);
        let encoded_integration_id = urlencoding::encode(integration_id);
        let url = format!(
            "{}/rest/v1/nango_connections?select=connection_id&user_id=eq.{}&connection_id=eq.{}&integration_id=eq.{}",
            self.supabase_url, encoded_user_id, encoded_connection_id, encoded_integration_id,
        );

        let response = self.anon_query(&url, auth_token).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::NangoError::Internal(format!(
                "ownership check failed: {} - {}",
                status, body
            )));
        }

        let rows: Vec<LookupConnectionRow> = response
            .json()
            .await
            .map_err(|e| crate::error::NangoError::Internal(e.to_string()))?;

        Ok(!rows.is_empty())
    }

    pub(crate) async fn lookup_connection(
        &self,
        auth_token: &str,
        user_id: &str,
        integration_id: &str,
    ) -> Result<Option<LookupConnectionRow>, crate::error::NangoError> {
        let encoded_user_id = urlencoding::encode(user_id);
        let encoded_integration_id = urlencoding::encode(integration_id);
        let url = format!(
            "{}/rest/v1/nango_connections?select=connection_id,status&user_id=eq.{}&integration_id=eq.{}",
            self.supabase_url, encoded_user_id, encoded_integration_id,
        );

        let response = self.anon_query(&url, auth_token).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::NangoError::Internal(format!(
                "lookup query failed: {} - {}",
                status, body
            )));
        }

        let rows: Vec<LookupConnectionRow> = response
            .json()
            .await
            .map_err(|e| crate::error::NangoError::Internal(e.to_string()))?;

        Ok(rows.into_iter().next())
    }

    pub(crate) async fn list_user_connections(
        &self,
        auth_token: &str,
        user_id: &str,
    ) -> Result<Vec<NangoConnectionRow>, crate::error::NangoError> {
        let encoded_user_id = urlencoding::encode(user_id);
        let url = format!(
            "{}/rest/v1/nango_connections?select=integration_id,connection_id,status,last_error_type,last_error_description,last_error_at,updated_at&user_id=eq.{}",
            self.supabase_url, encoded_user_id,
        );

        let response = self.anon_query(&url, auth_token).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::NangoError::Internal(format!(
                "query failed: {} - {}",
                status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| crate::error::NangoError::Internal(e.to_string()))
    }

    pub(crate) async fn upsert_connection(
        &self,
        user_id: &str,
        integration_id: &str,
        connection_id: &str,
        provider: &str,
    ) -> Result<(), crate::error::NangoError> {
        let service_role_key = self.service_role_key()?;

        let url = format!(
            "{}/rest/v1/nango_connections?on_conflict=user_id,integration_id",
            self.supabase_url,
        );

        let body = serde_json::json!({
            "user_id": user_id,
            "integration_id": integration_id,
            "connection_id": connection_id,
            "provider": provider,
            "status": "connected",
            "last_error_type": serde_json::Value::Null,
            "last_error_description": serde_json::Value::Null,
            "last_error_at": serde_json::Value::Null,
            "updated_at": chrono::Utc::now().to_rfc3339(),
        });

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", service_role_key))
            .header("apikey", service_role_key)
            .header("Content-Type", "application/json")
            .header("Prefer", "resolution=merge-duplicates")
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::error::NangoError::Internal(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::NangoError::Internal(format!(
                "upsert failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    pub(crate) async fn delete_connection(
        &self,
        user_id: &str,
        integration_id: &str,
    ) -> Result<(), crate::error::NangoError> {
        let service_role_key = self.service_role_key()?;

        let encoded_user_id = urlencoding::encode(user_id);
        let encoded_integration_id = urlencoding::encode(integration_id);
        let url = format!(
            "{}/rest/v1/nango_connections?user_id=eq.{}&integration_id=eq.{}",
            self.supabase_url, encoded_user_id, encoded_integration_id,
        );

        let response = self
            .http_client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", service_role_key))
            .header("apikey", service_role_key)
            .send()
            .await
            .map_err(|e| crate::error::NangoError::Internal(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::NangoError::Internal(format!(
                "delete failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    pub(crate) async fn delete_connection_by_connection(
        &self,
        integration_id: &str,
        connection_id: &str,
    ) -> Result<(), crate::error::NangoError> {
        let service_role_key = self.service_role_key()?;

        let encoded_integration_id = urlencoding::encode(integration_id);
        let encoded_connection_id = urlencoding::encode(connection_id);
        let url = format!(
            "{}/rest/v1/nango_connections?integration_id=eq.{}&connection_id=eq.{}",
            self.supabase_url, encoded_integration_id, encoded_connection_id,
        );

        let response = self
            .http_client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", service_role_key))
            .header("apikey", service_role_key)
            .send()
            .await
            .map_err(|e| crate::error::NangoError::Internal(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::NangoError::Internal(format!(
                "delete by connection failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    pub(crate) async fn mark_connection_refresh_failed(
        &self,
        integration_id: &str,
        connection_id: &str,
        error_type: Option<&str>,
        error_description: Option<&str>,
    ) -> Result<(), crate::error::NangoError> {
        let service_role_key = self.service_role_key()?;

        let encoded_integration_id = urlencoding::encode(integration_id);
        let encoded_connection_id = urlencoding::encode(connection_id);
        let url = format!(
            "{}/rest/v1/nango_connections?integration_id=eq.{}&connection_id=eq.{}",
            self.supabase_url, encoded_integration_id, encoded_connection_id,
        );

        let body = serde_json::json!({
            "status": "reconnect_required",
            "last_error_type": error_type,
            "last_error_description": error_description,
            "last_error_at": chrono::Utc::now().to_rfc3339(),
            "updated_at": chrono::Utc::now().to_rfc3339(),
        });

        let response = self
            .http_client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", service_role_key))
            .header("apikey", service_role_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::error::NangoError::Internal(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::NangoError::Internal(format!(
                "mark refresh failed failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }
}
