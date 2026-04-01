use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Instant;

use crate::error::{Result, SubscriptionError};

fn url_encode(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

#[derive(Clone)]
pub struct SupabaseClient {
    base_url: String,
    anon_key: String,
    service_role_key: String,
    http_client: Client,
}

impl SupabaseClient {
    pub fn new(
        supabase_url: impl Into<String>,
        anon_key: impl Into<String>,
        service_role_key: impl Into<String>,
    ) -> Self {
        Self {
            base_url: supabase_url.into().trim_end_matches('/').to_string(),
            anon_key: anon_key.into(),
            service_role_key: service_role_key.into(),
            http_client: Client::new(),
        }
    }

    fn with_trace_context(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        hypr_observability::with_current_trace_context(builder)
    }

    pub async fn rpc<T: for<'de> Deserialize<'de>>(
        &self,
        function_name: &str,
        auth_token: &str,
        body: Option<Value>,
    ) -> Result<T> {
        let url = format!("{}/rest/v1/rpc/{}", self.base_url, function_name);

        let start = Instant::now();
        let response = self
            .with_trace_context(
                self.http_client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", auth_token))
                    .header("apikey", &self.anon_key)
                    .header("Content-Type", "application/json")
                    .json(&body.unwrap_or(Value::Object(Default::default()))),
            )
            .send()
            .await
            .map_err(|e| SubscriptionError::SupabaseRequest(e.to_string()))?;
        tracing::info!(
            service.peer.name = "supabase",
            hyprnote.supabase.operation = "rpc",
            hyprnote.supabase.function = %function_name,
            http.response.status_code = response.status().as_u16(),
            hyprnote.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            return Err(SubscriptionError::SupabaseRequest(format!(
                "RPC {} failed: {} - {}",
                function_name, status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| SubscriptionError::SupabaseRequest(e.to_string()))
    }

    pub async fn select<T: for<'de> Deserialize<'de>>(
        &self,
        table: &str,
        auth_token: &str,
        select: &str,
        filters: &[(&str, &str)],
    ) -> Result<Vec<T>> {
        let mut url = format!(
            "{}/rest/v1/{}?select={}",
            self.base_url,
            url_encode(table),
            url_encode(select)
        );
        for (key, value) in filters {
            url.push_str(&format!("&{}={}", url_encode(key), url_encode(value)));
        }

        let start = Instant::now();
        let response = self
            .with_trace_context(
                self.http_client
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", auth_token))
                    .header("apikey", &self.anon_key),
            )
            .send()
            .await
            .map_err(|e| SubscriptionError::SupabaseRequest(e.to_string()))?;
        tracing::info!(
            service.peer.name = "supabase",
            hyprnote.supabase.operation = "select",
            hyprnote.supabase.table = %table,
            http.response.status_code = response.status().as_u16(),
            hyprnote.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            return Err(SubscriptionError::SupabaseRequest(format!(
                "SELECT from {} failed: {} - {}",
                table, status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| SubscriptionError::SupabaseRequest(e.to_string()))
    }

    pub async fn update<T: Serialize>(
        &self,
        table: &str,
        auth_token: &str,
        filters: &[(&str, &str)],
        data: &T,
    ) -> Result<()> {
        let mut url = format!("{}/rest/v1/{}", self.base_url, url_encode(table));
        if !filters.is_empty() {
            url.push('?');
            for (i, (key, value)) in filters.iter().enumerate() {
                if i > 0 {
                    url.push('&');
                }
                url.push_str(&format!("{}={}", url_encode(key), url_encode(value)));
            }
        }

        let start = Instant::now();
        let response = self
            .with_trace_context(
                self.http_client
                    .patch(&url)
                    .header("Authorization", format!("Bearer {}", auth_token))
                    .header("apikey", &self.anon_key)
                    .header("Content-Type", "application/json")
                    .json(data),
            )
            .send()
            .await
            .map_err(|e| SubscriptionError::SupabaseRequest(e.to_string()))?;
        tracing::info!(
            service.peer.name = "supabase",
            hyprnote.supabase.operation = "update",
            hyprnote.supabase.table = %table,
            http.response.status_code = response.status().as_u16(),
            hyprnote.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            return Err(SubscriptionError::SupabaseRequest(format!(
                "UPDATE {} failed: {} - {}",
                table, status, body
            )));
        }

        Ok(())
    }

    pub async fn admin_get_stripe_customer_id(&self, user_id: &str) -> Result<Option<String>> {
        let url = format!(
            "{}/rest/v1/profiles?select=stripe_customer_id&id=eq.{}",
            self.base_url,
            url_encode(user_id)
        );

        let start = Instant::now();
        let response = self
            .with_trace_context(
                self.http_client
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", self.service_role_key))
                    .header("apikey", &self.service_role_key),
            )
            .send()
            .await
            .map_err(|e| SubscriptionError::SupabaseRequest(e.to_string()))?;
        tracing::info!(
            service.peer.name = "supabase",
            hyprnote.supabase.operation = "admin_get_stripe_customer_id",
            http.response.status_code = response.status().as_u16(),
            hyprnote.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            return Err(SubscriptionError::SupabaseRequest(format!(
                "GET stripe_customer_id for {} failed: {} - {}",
                user_id, status, body
            )));
        }

        #[derive(Deserialize)]
        struct Row {
            stripe_customer_id: Option<String>,
        }

        let rows: Vec<Row> = response
            .json()
            .await
            .map_err(|e| SubscriptionError::SupabaseRequest(e.to_string()))?;

        Ok(rows.into_iter().next().and_then(|r| r.stripe_customer_id))
    }

    pub async fn admin_delete_storage_objects(&self, bucket: &str, user_id: &str) -> Result<()> {
        // List objects in the user's folder
        let list_url = format!("{}/storage/v1/object/list/{}", self.base_url, bucket);

        let start = Instant::now();
        let response = self
            .with_trace_context(
                self.http_client
                    .post(&list_url)
                    .header("Authorization", format!("Bearer {}", self.service_role_key))
                    .header("apikey", &self.service_role_key)
                    .json(&serde_json::json!({
                        "prefix": format!("{}/", user_id),
                        "limit": 1000
                    })),
            )
            .send()
            .await
            .map_err(|e| SubscriptionError::SupabaseRequest(e.to_string()))?;
        tracing::info!(
            service.peer.name = "supabase",
            hyprnote.supabase.operation = "admin_delete_storage_objects.list",
            http.response.status_code = response.status().as_u16(),
            hyprnote.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            // Not critical — storage may be empty
            tracing::warn!(
                enduser.id = %user_id,
                "failed to list storage objects, skipping cleanup"
            );
            return Ok(());
        }

        #[derive(Deserialize)]
        struct StorageObject {
            name: String,
        }

        let objects: Vec<StorageObject> = response.json().await.unwrap_or_default();

        if objects.is_empty() {
            return Ok(());
        }

        // Delete all objects in the user's folder
        let delete_url = format!("{}/storage/v1/object/{}", self.base_url, bucket);
        let prefixes: Vec<String> = objects
            .into_iter()
            .map(|o| format!("{}/{}", user_id, o.name))
            .collect();

        let start = Instant::now();
        let response = self
            .with_trace_context(
                self.http_client
                    .delete(&delete_url)
                    .header("Authorization", format!("Bearer {}", self.service_role_key))
                    .header("apikey", &self.service_role_key)
                    .json(&serde_json::json!({ "prefixes": prefixes })),
            )
            .send()
            .await
            .map_err(|e| SubscriptionError::SupabaseRequest(e.to_string()))?;
        tracing::info!(
            service.peer.name = "supabase",
            hyprnote.supabase.operation = "admin_delete_storage_objects.delete",
            http.response.status_code = response.status().as_u16(),
            hyprnote.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        Ok(())
    }

    pub async fn admin_delete_user(&self, user_id: &str) -> Result<()> {
        let url = format!(
            "{}/auth/v1/admin/users/{}",
            self.base_url,
            url_encode(user_id)
        );

        let start = Instant::now();
        let response = self
            .with_trace_context(
                self.http_client
                    .delete(&url)
                    .header("Authorization", format!("Bearer {}", self.service_role_key))
                    .header("apikey", &self.service_role_key),
            )
            .send()
            .await
            .map_err(|e| SubscriptionError::SupabaseRequest(e.to_string()))?;
        tracing::info!(
            service.peer.name = "supabase",
            hyprnote.supabase.operation = "admin_delete_user",
            http.response.status_code = response.status().as_u16(),
            hyprnote.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            return Err(SubscriptionError::SupabaseRequest(format!(
                "DELETE user {} failed: {} - {}",
                user_id, status, body
            )));
        }

        Ok(())
    }

    pub async fn get_user_email(&self, auth_token: &str) -> Result<Option<String>> {
        let url = format!("{}/auth/v1/user", self.base_url);

        let response = self
            .with_trace_context(
                self.http_client
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", auth_token))
                    .header("apikey", &self.anon_key),
            )
            .send()
            .await
            .map_err(|e| SubscriptionError::SupabaseRequest(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            return Err(SubscriptionError::SupabaseRequest(format!(
                "GET user failed: {} - {}",
                status, body
            )));
        }

        #[derive(Deserialize)]
        struct UserResponse {
            email: Option<String>,
        }

        let user: UserResponse = response
            .json()
            .await
            .map_err(|e| SubscriptionError::SupabaseRequest(e.to_string()))?;

        Ok(user.email)
    }
}
