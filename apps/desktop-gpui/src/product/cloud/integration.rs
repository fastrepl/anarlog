use std::sync::Arc;

use anlg_calendar::CalendarProviderType;
use desktop_runtime::{CancellationToken, Result, ServiceError};
use futures::future::BoxFuture;
use reqwest::Method;
use serde_json::json;

use super::{CloudServices, auth::SecureAuthProvider, transport::failure};
use crate::{
    meeting::config::CloudAccess,
    product::local::calendar::{AccessToken, CalendarAuth},
    workspace::automation_runner::AutomationClient,
};

impl CloudServices {
    pub fn provider_access(&self) -> BoxFuture<'static, Result<CloudAccess>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .clone()
                .service(move |_| async move {
                    let account = this
                        .identity()
                        .account_id
                        .ok_or_else(|| failure("Sign in to use Anarlog AI"))?;
                    let lease = this.core.auth.lease(&account, false).await?;
                    let claims = this.core.auth.claims(&account).await?;
                    Ok(CloudAccess {
                        access_token: lease.access_token()?.to_owned(),
                        user_id: account.to_string(),
                        is_paid: claims.is_paid() || claims.has_active_trial(),
                    })
                })?
                .receive()
                .await
        })
    }

    pub fn automation_client(&self) -> BoxFuture<'static, Result<AutomationClient>> {
        let this = self.clone();
        Box::pin(async move {
            let identity = this.identity();
            let account = identity
                .account_id
                .ok_or_else(|| failure("Sign in to run automations"))?;
            let lease = this.credential(account.clone()).await?;
            AutomationClient::new(
                this.core.auth.transport.config.api.as_str(),
                lease.access_token()?.into(),
                identity.email.unwrap_or(account),
            )
        })
    }

    /// The account's integration connections with their status (shipping `useConnections`).
    pub fn integration_connections(&self) -> BoxFuture<'static, Result<Vec<serde_json::Value>>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .clone()
                .service(move |_| async move {
                    let account = this
                        .identity()
                        .account_id
                        .ok_or_else(|| failure("Sign in to connect a calendar"))?;
                    let lease = this.core.auth.lease(&account, false).await?;
                    let response = this
                        .core
                        .api(&lease, Method::GET, "/nango/connections", None)
                        .await?;
                    Ok(response["connections"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default())
                })?
                .receive()
                .await
        })
    }

    /// Opens the web integration page like shipping's `openIntegrationUrl`: connect and
    /// reconnect hand over a Nango session; disconnect confirms on the page itself.
    pub fn open_integration(
        &self,
        integration: String,
        action: &'static str,
        connection: Option<String>,
        return_to: Option<&'static str>,
    ) -> BoxFuture<'static, Result<()>> {
        let this = self.clone();
        Box::pin(async move {
            let host = this.core.host.clone();
            let handoff = super::host::Handoff::Integration(integration.clone());
            let url = this
                .runtime
                .clone()
                .service(move |_| async move {
                    let mut url = this
                        .core
                        .auth
                        .transport
                        .config
                        .web_flow("/app/integration")?;
                    url.query_pairs_mut()
                        .append_pair("action", action)
                        .append_pair("integration_id", &integration);
                    if let Some(return_to) = return_to {
                        url.query_pairs_mut().append_pair("return_to", return_to);
                    }
                    if let Some(connection) = &connection {
                        url.query_pairs_mut()
                            .append_pair("connection_id", connection);
                    }
                    if action != "disconnect" {
                        let account = this
                            .identity()
                            .account_id
                            .ok_or_else(|| failure("Sign in to connect a calendar"))?;
                        let lease = this.core.auth.lease(&account, false).await?;
                        let response = this
                            .core
                            .api(
                                &lease,
                                Method::POST,
                                "/nango/session",
                                Some(&json!({
                                    "integration_id": integration,
                                    "mode": action,
                                    "connection_id": connection,
                                })),
                            )
                            .await?;
                        let token = response["token"]
                            .as_str()
                            .ok_or_else(|| failure("Integration handoff is missing"))?;
                        url.query_pairs_mut().append_pair("handoff", "nango");
                        url.set_fragment(Some(
                            &url::form_urlencoded::Serializer::new(String::new())
                                .append_pair("session_token", token)
                                .finish(),
                        ));
                    }
                    Ok(url)
                })?
                .receive()
                .await?;
            host.hand_off(url, handoff).await
        })
    }

    fn calendar_flow(
        &self,
        provider: CalendarProviderType,
        connection: Option<String>,
        cancel: CancellationToken,
    ) -> BoxFuture<'static, Result<()>> {
        let this = self.clone();
        Box::pin(async move {
            let integration = match provider {
                CalendarProviderType::Google => "google-calendar",
                CalendarProviderType::Outlook => "outlook",
                CalendarProviderType::Apple => {
                    return Err(failure("Use the native Calendar permission"));
                }
            };
            let account = this
                .identity()
                .account_id
                .ok_or_else(|| failure("Sign in to connect a calendar"))?;
            let lease = this.core.auth.lease(&account, false).await?;
            if cancel.is_cancelled() {
                return Err(ServiceError::Cancelled);
            }
            if let Some(connection) = connection {
                let response = this
                    .core
                    .api(
                        &lease,
                        Method::DELETE,
                        "/nango/connections",
                        Some(&json!({"integration_id":integration,"connection_id":connection})),
                    )
                    .await?;
                if response["status"] != "ok" {
                    return Err(failure("Calendar disconnect was not confirmed"));
                }
                return Ok(());
            }
            let mut url = this
                .core
                .auth
                .transport
                .config
                .web_flow("/app/integration")?;
            url.query_pairs_mut()
                .append_pair("integration_id", integration)
                .append_pair("action", "connect");
            {
                let response = this
                    .core
                    .api(
                        &lease,
                        Method::POST,
                        "/nango/session",
                        Some(&json!({"integration_id": integration, "mode":"connect"})),
                    )
                    .await?;
                let token = response["token"]
                    .as_str()
                    .ok_or_else(|| failure("Integration handoff is missing"))?;
                url.query_pairs_mut().append_pair("handoff", "nango");
                url.set_fragment(Some(
                    &url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("session_token", token)
                        .finish(),
                ));
            }
            if cancel.is_cancelled() {
                return Err(ServiceError::Cancelled);
            }
            this.core.host.open_url(url).await
        })
    }
}

impl CalendarAuth for CloudServices {
    fn token(&self, cancel: CancellationToken) -> BoxFuture<'static, Result<AccessToken>> {
        let this = self.clone();
        Box::pin(async move {
            let account: Arc<str> = this
                .identity()
                .account_id
                .ok_or_else(|| failure("Sign in to connect a calendar"))?;
            let lease = this.core.auth.lease(&account, false).await?;
            if cancel.is_cancelled() {
                return Err(ServiceError::Cancelled);
            }
            Ok(AccessToken::new(lease.access_token()?.to_owned()))
        })
    }

    fn connect(
        &self,
        provider: CalendarProviderType,
        cancel: CancellationToken,
    ) -> BoxFuture<'static, Result<()>> {
        self.calendar_flow(provider, None, cancel)
    }

    fn disconnect(
        &self,
        provider: CalendarProviderType,
        connection: String,
        cancel: CancellationToken,
    ) -> BoxFuture<'static, Result<()>> {
        self.calendar_flow(provider, Some(connection), cancel)
    }
}

impl crate::product::integrations::Integrations for CloudServices {
    fn connections(
        &self,
    ) -> BoxFuture<'static, Result<Vec<crate::product::integrations::Connection>>> {
        let connections = self.integration_connections();
        Box::pin(async move {
            Ok(connections
                .await?
                .iter()
                .filter_map(crate::product::integrations::Connection::from_value)
                .collect())
        })
    }

    fn open(
        &self,
        integration: &'static str,
        action: &'static str,
        connection: Option<String>,
        return_to: &'static str,
    ) -> BoxFuture<'static, Result<()>> {
        self.open_integration(integration.into(), action, connection, Some(return_to))
    }

    fn post(
        &self,
        path: String,
        body: serde_json::Value,
    ) -> BoxFuture<'static, Result<serde_json::Value>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .clone()
                .service(move |_| async move {
                    let account = this
                        .identity()
                        .account_id
                        .ok_or_else(|| failure("Sign in to continue"))?;
                    let lease = this.core.auth.lease(&account, false).await?;
                    this.core
                        .api(&lease, Method::POST, &path, Some(&body))
                        .await
                })?
                .receive()
                .await
        })
    }
}
