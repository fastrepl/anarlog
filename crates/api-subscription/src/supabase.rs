use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::time::{Duration, Instant};

use crate::error::{Result, SubscriptionError};

const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const ADMIN_RPC_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_ADMIN_RPC_RESPONSE_BYTES: usize = 256 * 1024;

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
            http_client: Client::builder()
                .timeout(HTTP_TIMEOUT)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("valid Supabase HTTP client"),
        }
    }

    fn with_trace_context(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        hypr_observability::with_current_trace_context(builder)
    }

    pub(crate) fn storage(&self) -> hypr_supabase_storage::SupabaseStorage {
        hypr_supabase_storage::SupabaseStorage::new(
            self.http_client.clone(),
            &self.base_url,
            &self.service_role_key,
        )
    }

    pub(crate) async fn admin_rpc<RequestBody, ResponseBody>(
        &self,
        function_name: &str,
        body: &RequestBody,
    ) -> Result<ResponseBody>
    where
        RequestBody: Serialize + ?Sized,
        ResponseBody: DeserializeOwned,
    {
        let url = format!("{}/rest/v1/rpc/{}", self.base_url, function_name);
        let start = Instant::now();
        let response = self
            .with_trace_context(
                self.http_client
                    .post(url)
                    .header("Authorization", format!("Bearer {}", self.service_role_key))
                    .header("apikey", &self.service_role_key)
                    .timeout(ADMIN_RPC_TIMEOUT)
                    .json(body),
            )
            .send()
            .await
            .map_err(|_| {
                SubscriptionError::SupabaseRequest(format!("RPC {function_name} request failed"))
            })?;
        let status = response.status();
        tracing::info!(
            service.peer.name = "supabase",
            hyprnote.supabase.operation = "admin_rpc",
            hyprnote.supabase.function = %function_name,
            http.response.status_code = status.as_u16(),
            hyprnote.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );
        if response
            .content_length()
            .is_some_and(|length| length > MAX_ADMIN_RPC_RESPONSE_BYTES as u64)
        {
            return Err(SubscriptionError::SupabaseRequest(format!(
                "RPC {function_name} response was too large"
            )));
        }

        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| {
                SubscriptionError::SupabaseRequest(format!(
                    "RPC {function_name} response could not be read"
                ))
            })?;
            if bytes.len().saturating_add(chunk.len()) > MAX_ADMIN_RPC_RESPONSE_BYTES {
                return Err(SubscriptionError::SupabaseRequest(format!(
                    "RPC {function_name} response was too large"
                )));
            }
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            return Err(SubscriptionError::SupabaseRequest(format!(
                "RPC {function_name} failed: {status}"
            )));
        }

        serde_json::from_slice(&bytes).map_err(|_| {
            SubscriptionError::SupabaseRequest(format!("RPC {function_name} response was invalid"))
        })
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

    pub async fn begin_account_deletion(&self, user_id: &str) -> Result<()> {
        #[derive(Serialize)]
        struct Request<'a> {
            p_owner_user_id: &'a str,
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Row {
            owner_user_id: String,
            final_sweep_not_before: String,
            was_created: bool,
        }

        let mut rows: Vec<Row> = self
            .admin_rpc(
                "begin_account_deletion",
                &Request {
                    p_owner_user_id: user_id,
                },
            )
            .await?;
        if rows.len() != 1 {
            return Err(SubscriptionError::SupabaseRequest(
                "Account deletion RPC returned an invalid row count".to_string(),
            ));
        }
        let row = rows.pop().expect("row count was checked");
        if row.owner_user_id != user_id
            || chrono::DateTime::parse_from_rfc3339(&row.final_sweep_not_before).is_err()
        {
            return Err(SubscriptionError::SupabaseRequest(
                "Account deletion RPC returned an invalid response".to_string(),
            ));
        }
        let _ = row.was_created;
        Ok(())
    }

    pub(crate) async fn assign_profile_stripe_customer(
        &self,
        auth_token: &str,
        user_id: &str,
        customer_id: &str,
    ) -> Result<Option<String>> {
        #[derive(Serialize)]
        struct Request<'a> {
            p_owner_user_id: &'a str,
            p_stripe_customer_id: &'a str,
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Row {
            assigned_customer_id: Option<String>,
        }

        let result: Result<Vec<Row>> = self
            .admin_rpc(
                "assign_profile_stripe_customer",
                &Request {
                    p_owner_user_id: user_id,
                    p_stripe_customer_id: customer_id,
                },
            )
            .await;
        let mut rows = match result {
            Ok(rows) => rows,
            Err(SubscriptionError::SupabaseRequest(message))
                if message.contains("404 Not Found") =>
            {
                #[derive(Serialize)]
                struct UpdateData<'a> {
                    stripe_customer_id: &'a str,
                }

                let url = format!(
                    "{}/rest/v1/profiles?id=eq.{}&stripe_customer_id=is.null",
                    self.base_url,
                    url_encode(user_id)
                );
                let response = self
                    .with_trace_context(
                        self.http_client
                            .patch(url)
                            .header("Authorization", format!("Bearer {auth_token}"))
                            .header("apikey", &self.anon_key)
                            .json(&UpdateData {
                                stripe_customer_id: customer_id,
                            }),
                    )
                    .send()
                    .await
                    .map_err(|_| {
                        SubscriptionError::SupabaseRequest(
                            "Legacy Stripe customer assignment request failed".to_string(),
                        )
                    })?;
                if !response.status().is_success() {
                    return Err(SubscriptionError::SupabaseRequest(format!(
                        "Legacy Stripe customer assignment failed: {}",
                        response.status()
                    )));
                }

                #[derive(Deserialize)]
                struct Profile {
                    stripe_customer_id: Option<String>,
                }

                let profiles: Vec<Profile> = self
                    .select(
                        "profiles",
                        auth_token,
                        "stripe_customer_id",
                        &[("id", &format!("eq.{user_id}"))],
                    )
                    .await?;
                return Ok(profiles
                    .first()
                    .and_then(|profile| profile.stripe_customer_id.clone()));
            }
            Err(error) => return Err(error),
        };
        if rows.len() != 1 {
            return Err(SubscriptionError::SupabaseRequest(
                "Stripe customer assignment RPC returned an invalid row count".to_string(),
            ));
        }

        Ok(rows
            .pop()
            .expect("row count was checked")
            .assigned_customer_id)
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

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }
        if !response.status().is_success() {
            let status = response.status();
            return Err(SubscriptionError::SupabaseRequest(format!(
                "DELETE user failed: {status}"
            )));
        }

        Ok(())
    }

    pub(crate) async fn admin_get_user_email(&self, user_id: &str) -> Result<Option<String>> {
        let url = format!(
            "{}/auth/v1/admin/users/{}",
            self.base_url,
            url_encode(user_id)
        );
        let response = self
            .with_trace_context(
                self.http_client
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", self.service_role_key))
                    .header("apikey", &self.service_role_key),
            )
            .send()
            .await
            .map_err(|_| {
                SubscriptionError::SupabaseRequest("Admin user lookup request failed".to_string())
            })?;
        if !response.status().is_success() {
            return Err(SubscriptionError::SupabaseRequest(format!(
                "Admin user lookup failed: {}",
                response.status()
            )));
        }

        #[derive(Deserialize)]
        struct UserResponse {
            email: Option<String>,
        }

        response
            .json::<UserResponse>()
            .await
            .map(|user| user.email)
            .map_err(|_| {
                SubscriptionError::SupabaseRequest(
                    "Admin user lookup returned an invalid response".to_string(),
                )
            })
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

    pub async fn revoke_user_sessions(&self, auth_token: &str) -> Result<()> {
        let url = format!("{}/auth/v1/logout?scope=global", self.base_url);
        let response = self
            .with_trace_context(
                self.http_client
                    .post(url)
                    .header("Authorization", format!("Bearer {auth_token}"))
                    .header("apikey", &self.anon_key),
            )
            .send()
            .await
            .map_err(|_| {
                SubscriptionError::SupabaseRequest("Session revocation request failed".to_string())
            })?;
        if !response.status().is_success() {
            return Err(SubscriptionError::SupabaseRequest(format!(
                "Session revocation failed: {}",
                response.status()
            )));
        }
        Ok(())
    }
}
