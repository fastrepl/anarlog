//! The calendar's account services, provided by the host: cloud calendar connections,
//! shipping's browser connect/reconnect/disconnect flows, Apple Calendar access and the
//! calendar sync. `CalendarSync` shares the sync status and the account list between the
//! calendar and its sidebar, like shipping's `SyncProvider` and `useConnections`.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anlg_calendar::CalendarProviderType;
use desktop_runtime::{Result, RuntimeHandle, ServiceError};
use futures::future::BoxFuture;
use gpui::{Context, Task};
use serde_json::Value;

use super::{calendar_sidebar::Provider, mutations::failure};
use crate::{
    platform::permissions::{Permission, Status},
    product::{cloud::CloudServices, local::LocalServices},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    pub provider: Provider,
    pub id: Arc<str>,
    pub reconnect_required: bool,
    pub error: Option<Arc<str>>,
}

/// Parses `GET /nango/connections` entries; integrations other than calendars are skipped.
pub fn connections(entries: &[Value]) -> Vec<Connection> {
    entries
        .iter()
        .filter_map(|entry| {
            let provider = match entry["integration_id"].as_str()? {
                "google-calendar" => Provider::Google,
                "outlook" => Provider::Outlook,
                _ => return None,
            };
            Some(Connection {
                provider,
                id: entry["connection_id"].as_str()?.into(),
                reconnect_required: entry["status"] == "reconnect_required",
                error: entry["last_error_description"]
                    .as_str()
                    .filter(|error| !error.is_empty())
                    .map(Into::into),
            })
        })
        .collect()
}

/// Shipping's `getCalendarConnectionKey`: which calendar accounts exist.
pub fn connection_key(connections: &[Connection]) -> Vec<Arc<str>> {
    let mut key = connections
        .iter()
        .map(|connection| connection.id.clone())
        .collect::<Vec<_>>();
    key.sort();
    key
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Connect,
    Reconnect,
    Disconnect,
}

impl Flow {
    fn action(self) -> &'static str {
        match self {
            Self::Connect => "connect",
            Self::Reconnect => "reconnect",
            Self::Disconnect => "disconnect",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppleAccess {
    Authorized,
    Denied,
    NotDetermined,
}

pub trait CalendarAccounts: Send + Sync {
    fn connections(&self) -> BoxFuture<'static, Result<Vec<Connection>>>;
    fn open(
        &self,
        provider: Provider,
        flow: Flow,
        connection: Option<Arc<str>>,
    ) -> BoxFuture<'static, Result<()>>;
    fn apple_access(&self) -> AppleAccess;
    /// Shows the system prompt when access was never requested.
    fn request_apple(&self) -> BoxFuture<'static, Result<()>>;
    fn open_apple_settings(&self) -> Result<()>;
    fn disconnect_apple(&self) -> BoxFuture<'static, Result<()>>;
    /// Discovers every connected account's calendars and refreshes the enabled ones.
    fn sync(&self) -> BoxFuture<'static, Result<()>>;
}

pub struct HostCalendarAccounts {
    runtime: RuntimeHandle,
    local: Arc<LocalServices>,
    cloud: Option<CloudServices>,
    /// Shipping resets the Calendar permission on disconnect; the native app cannot, so it
    /// stops syncing Apple Calendar and offers to connect again until the next connect.
    apple_disconnected: Arc<AtomicBool>,
}

impl HostCalendarAccounts {
    pub fn new(
        runtime: RuntimeHandle,
        local: Arc<LocalServices>,
        cloud: Option<CloudServices>,
    ) -> Self {
        Self {
            runtime,
            local,
            cloud,
            apple_disconnected: Arc::default(),
        }
    }
}

fn system_apple_access() -> AppleAccess {
    #[cfg(target_os = "macos")]
    {
        match anlg_apple_calendar::Handle::authorization_status() {
            anlg_apple_calendar::CalendarAuthStatus::Authorized => AppleAccess::Authorized,
            anlg_apple_calendar::CalendarAuthStatus::Denied => AppleAccess::Denied,
            anlg_apple_calendar::CalendarAuthStatus::NotDetermined => AppleAccess::NotDetermined,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        AppleAccess::NotDetermined
    }
}

impl CalendarAccounts for HostCalendarAccounts {
    fn connections(&self) -> BoxFuture<'static, Result<Vec<Connection>>> {
        let cloud = self.cloud.clone();
        Box::pin(async move {
            let cloud = cloud.ok_or_else(|| failure("Cloud sign-in is not configured"))?;
            Ok(connections(&cloud.integration_connections().await?))
        })
    }

    fn open(
        &self,
        provider: Provider,
        flow: Flow,
        connection: Option<Arc<str>>,
    ) -> BoxFuture<'static, Result<()>> {
        let cloud = self.cloud.clone();
        Box::pin(async move {
            let integration = match provider {
                Provider::Google => "google-calendar",
                Provider::Outlook => "outlook",
                Provider::Apple => return Err(ServiceError::Conflict),
            };
            cloud
                .ok_or_else(|| failure("Cloud sign-in is not configured"))?
                .open_integration(
                    integration,
                    flow.action(),
                    connection.map(|id| id.to_string()),
                    "calendar",
                )
                .await
        })
    }

    fn apple_access(&self) -> AppleAccess {
        if self.apple_disconnected.load(Ordering::Acquire) {
            AppleAccess::NotDetermined
        } else {
            system_apple_access()
        }
    }

    fn request_apple(&self) -> BoxFuture<'static, Result<()>> {
        self.apple_disconnected.store(false, Ordering::Release);
        let permissions = self.local.permissions.clone();
        let job = self.runtime.service(move |_| async move {
            let status =
                tokio::task::spawn_blocking(move || permissions.request(Permission::Calendar))
                    .await
                    .map_err(failure)?;
            match status {
                Status::Denied(error) => Err(failure(error)),
                _ => Ok(()),
            }
        });
        Box::pin(async move { job?.receive().await })
    }

    fn open_apple_settings(&self) -> Result<()> {
        self.local.permissions.open_settings(Permission::Calendar)
    }

    fn disconnect_apple(&self) -> BoxFuture<'static, Result<()>> {
        self.apple_disconnected.store(true, Ordering::Release);
        // Shipping's `tombstoneCalendarConnection("apple", "apple")`.
        let job = self.runtime.submit(|services| async move {
            services
                .executor
                .execute_transaction(vec![
                    anlg_db_execute::TransactionStatement {
                        sql: "UPDATE events SET deleted_at=strftime('%Y-%m-%dT%H:%M:%SZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE calendar_id IN (SELECT id FROM calendars WHERE provider='apple') AND deleted_at IS NULL".into(),
                        params: vec![],
                        expected_rows_affected: None,
                    },
                    anlg_db_execute::TransactionStatement {
                        sql: "UPDATE calendars SET enabled=0,deleted_at=strftime('%Y-%m-%dT%H:%M:%SZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE provider='apple' AND deleted_at IS NULL".into(),
                        params: vec![],
                        expected_rows_affected: None,
                    },
                ])
                .await
                .map_err(failure)?;
            Ok(())
        });
        Box::pin(async move { job?.receive().await })
    }

    fn sync(&self) -> BoxFuture<'static, Result<()>> {
        let calendar = self.local.calendar.clone();
        let runtime = self.runtime.clone();
        let cloud = self.cloud.is_some();
        let apple = !self.apple_disconnected.load(Ordering::Acquire)
            && system_apple_access() == AppleAccess::Authorized;
        let job = self.runtime.service(move |services| async move {
            let cancel = services.shutdown_requested;
            let mut accounts = Vec::new();
            if apple {
                accounts.push((CalendarProviderType::Apple, "apple".to_owned()));
            }
            // Without a signed-in account only Apple Calendar syncs, as in shipping.
            if cloud && let Ok(connections) = calendar.connections(cancel.clone()).await {
                for entry in connections {
                    accounts.extend(
                        entry
                            .connection_ids
                            .into_iter()
                            .map(|id| (entry.provider, id)),
                    );
                }
            }
            // Like shipping, one account failing does not stop the others.
            for (provider, connection) in accounts {
                let Ok(calendars) = calendar
                    .discover(&runtime, provider, connection, cancel.clone())
                    .await
                else {
                    continue;
                };
                for entry in calendars {
                    if entry["enabled"].as_bool() == Some(true)
                        && let Some(id) = entry["id"].as_str()
                    {
                        let _ = calendar.refresh(&runtime, id.into(), cancel.clone()).await;
                    }
                }
                if cancel.is_cancelled() {
                    return Err(ServiceError::Cancelled);
                }
            }
            Ok(())
        });
        Box::pin(async move { job?.receive().await })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SyncStatus {
    #[default]
    Idle,
    Scheduled,
    Syncing,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Accounts {
    #[default]
    Loading,
    Ready(Arc<[Connection]>),
    Failed,
}

const DEBOUNCE: Duration = Duration::from_secs(5);
const POLL_EVERY: Duration = Duration::from_millis(1500);
const POLL_FOR: Duration = Duration::from_secs(45);

pub struct CalendarSync {
    services: Option<Arc<dyn CalendarAccounts>>,
    syncing: bool,
    debounce: Option<Task<()>>,
    pub accounts: Accounts,
    poll: Option<Task<()>>,
}

impl CalendarSync {
    pub fn new() -> Self {
        Self {
            services: None,
            syncing: false,
            debounce: None,
            accounts: Accounts::Loading,
            poll: None,
        }
    }

    pub fn services(&self) -> Option<Arc<dyn CalendarAccounts>> {
        self.services.clone()
    }

    pub fn set_services(
        &mut self,
        services: Option<Arc<dyn CalendarAccounts>>,
        cx: &mut Context<Self>,
    ) {
        self.services = services;
        self.accounts = Accounts::Loading;
        cx.notify();
    }

    pub fn status(&self) -> SyncStatus {
        if self.syncing {
            SyncStatus::Syncing
        } else if self.debounce.is_some() {
            SyncStatus::Scheduled
        } else {
            SyncStatus::Idle
        }
    }

    /// Starts a sync unless one is already running (shipping `scheduleCalendarSync`).
    pub fn schedule(&mut self, cx: &mut Context<Self>) {
        let Some(services) = self.services.clone() else {
            return;
        };
        if self.syncing {
            return;
        }
        self.syncing = true;
        cx.notify();
        let job = services.sync();
        cx.spawn(async move |this, cx| {
            let _ = job.await;
            let _ = this.update(cx, |this, cx| {
                this.syncing = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// Syncs five seconds after the last calendar toggle.
    pub fn schedule_debounced(&mut self, cx: &mut Context<Self>) {
        self.debounce = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            let _ = this.update(cx, |this, cx| {
                this.debounce = None;
                this.schedule(cx);
            });
        }));
        cx.notify();
    }

    pub fn cancel_debounced(&mut self, cx: &mut Context<Self>) {
        if self.debounce.take().is_some() {
            cx.notify();
        }
    }

    pub fn refresh_accounts(&mut self, cx: &mut Context<Self>) {
        let Some(services) = self.services.clone() else {
            return;
        };
        let job = services.connections();
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| this.replace_accounts(result, cx));
        })
        .detach();
    }

    fn replace_accounts(&mut self, result: Result<Vec<Connection>>, cx: &mut Context<Self>) {
        let next = match result {
            Ok(connections) => Accounts::Ready(connections.into()),
            Err(_) => Accounts::Failed,
        };
        // A connected or removed account syncs right away (shipping
        // `useSyncWhenCalendarConnectionsChange`).
        if let (Accounts::Ready(before), Accounts::Ready(after)) = (&self.accounts, &next)
            && connection_key(before) != connection_key(after)
        {
            self.schedule(cx);
        }
        if self.accounts != next {
            self.accounts = next;
            cx.notify();
        }
    }

    /// After a browser flow starts, checks for the new account every 1.5 seconds for up to
    /// 45 seconds, like shipping's connection polling.
    pub fn watch_for_new_connection(&mut self, cx: &mut Context<Self>) {
        let Some(services) = self.services.clone() else {
            return;
        };
        let start = match &self.accounts {
            Accounts::Ready(connections) => Some(connection_key(connections)),
            _ => None,
        };
        let until = Instant::now() + POLL_FOR;
        self.poll = Some(cx.spawn(async move |this, cx| {
            while Instant::now() < until {
                cx.background_executor().timer(POLL_EVERY).await;
                let result = services.connections().await;
                let changed = result
                    .as_ref()
                    .is_ok_and(|connections| Some(connection_key(connections)) != start);
                if this
                    .update(cx, |this, cx| this.replace_accounts(result, cx))
                    .is_err()
                    || changed
                {
                    break;
                }
            }
        }));
    }
}

impl Default for CalendarSync {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn calendar_accounts_keep_only_calendar_integrations_and_their_status() {
        let parsed = connections(&[
            json!({"integration_id":"google-calendar","connection_id":"g1","status":"reconnect_required","last_error_description":"Token expired"}),
            json!({"integration_id":"outlook","connection_id":"o1","status":"ok","last_error_description":""}),
            json!({"integration_id":"slack","connection_id":"s1"}),
            json!({"integration_id":"google-calendar"}),
        ]);
        assert_eq!(
            parsed,
            [
                Connection {
                    provider: Provider::Google,
                    id: "g1".into(),
                    reconnect_required: true,
                    error: Some("Token expired".into()),
                },
                Connection {
                    provider: Provider::Outlook,
                    id: "o1".into(),
                    reconnect_required: false,
                    error: None,
                },
            ]
        );
        assert_eq!(
            connection_key(&parsed),
            connection_key(&[parsed[1].clone(), parsed[0].clone()])
        );
    }
}
