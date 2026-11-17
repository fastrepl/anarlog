//! Account integrations through the Anarlog API: shipping's `useConnections`,
//! `openIntegrationUrl` and `waitForNangoConnection`. Imports, CRM and Ticket share them; the
//! trait keeps views and tests away from the network.

use std::{sync::Arc, time::Duration};

use desktop_runtime::{CancellationToken, Result, ServiceError};
use futures::future::BoxFuture;
use gpui::{App, AsyncApp, Global};
use serde_json::Value;

/// `integrationSetupError`.
pub const SETUP_ERROR: &str = "Could not start the integration setup. Try again.";
pub const POLL: Duration = Duration::from_secs(2);
pub const TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connection {
    pub integration_id: String,
    pub connection_id: String,
    pub status: Option<String>,
}

impl Connection {
    pub fn from_value(value: &Value) -> Option<Self> {
        Some(Self {
            integration_id: value["integration_id"].as_str()?.to_owned(),
            connection_id: value["connection_id"].as_str()?.to_owned(),
            status: value["status"].as_str().map(str::to_owned),
        })
    }

    /// `nangoConnectionIsReady`.
    pub fn ready(&self) -> bool {
        self.status.as_deref() != Some("reconnect_required")
    }
}

/// `findCrmConnection`: the first ready connection for the integration, else the first one.
pub fn find<'a>(connections: &'a [Connection], integration: &str) -> Option<&'a Connection> {
    let mut matches = connections
        .iter()
        .filter(|connection| connection.integration_id == integration);
    let first = matches.clone().next();
    matches.find(|connection| connection.ready()).or(first)
}

pub trait Integrations: Send + Sync {
    fn connections(&self) -> BoxFuture<'static, Result<Vec<Connection>>>;
    /// Opens the web integration page; `action` is connect, reconnect or disconnect.
    fn open(
        &self,
        integration: &'static str,
        action: &'static str,
        connection: Option<String>,
        return_to: &'static str,
    ) -> BoxFuture<'static, Result<()>>;
    /// An authenticated `POST` to the Anarlog API.
    fn post(&self, path: String, body: Value) -> BoxFuture<'static, Result<Value>>;
}

/// Replaces the signed-in cloud service (tests and screenshots).
#[derive(Clone)]
pub struct Installed(pub Arc<dyn Integrations>);

impl Global for Installed {}

pub fn installed(cx: &App) -> Option<Arc<dyn Integrations>> {
    if let Some(installed) = cx.try_global::<Installed>() {
        return Some(installed.0.clone());
    }
    cx.try_global::<super::sharing::SharingCloud>()
        .map(|cloud| Arc::new(cloud.0.clone()) as Arc<dyn Integrations>)
}

/// `waitForNangoConnection`: polls until the integration has a ready connection.
pub async fn wait_for_connection(
    service: Arc<dyn Integrations>,
    name: &str,
    integration: &str,
    cancel: CancellationToken,
    cx: &mut AsyncApp,
) -> Result<Connection> {
    let deadline = std::time::Instant::now() + TIMEOUT;
    loop {
        if cancel.is_cancelled() {
            return Err(ServiceError::Cancelled);
        }
        let connections = service
            .connections()
            .await
            .map_err(|_| failure("Failed to load integrations"))?;
        if let Some(connection) = connections
            .into_iter()
            .find(|item| item.integration_id == integration && item.ready())
        {
            return Ok(connection);
        }
        if std::time::Instant::now() >= deadline {
            return Err(failure(format!("{name} sign-in timed out. Try again.")));
        }
        cx.background_executor().timer(POLL).await;
    }
}

pub fn failure(message: impl Into<Arc<str>>) -> ServiceError {
    ServiceError::Failed(message.into())
}

/// A scripted service for tests and screenshots.
pub mod fake {
    use std::sync::Mutex;

    use super::*;

    /// An opened integration page: integration, action, connection and return page.
    pub type Opened = (String, String, Option<String>, String);

    /// Records opened pages and answers posts from canned responses keyed by path.
    #[derive(Default)]
    pub struct FakeIntegrations {
        pub connections: Mutex<Vec<Connection>>,
        pub opened: Mutex<Vec<Opened>>,
        pub posts: Mutex<Vec<(String, Value)>>,
        pub responses: Mutex<Vec<(String, Result<Value>)>>,
        /// Connections that appear once a connect page opens.
        pub on_connect: Mutex<Vec<Connection>>,
    }

    impl FakeIntegrations {
        pub fn connection(integration: &str, id: &str, status: Option<&str>) -> Connection {
            Connection {
                integration_id: integration.into(),
                connection_id: id.into(),
                status: status.map(Into::into),
            }
        }
    }

    impl Integrations for FakeIntegrations {
        fn connections(&self) -> BoxFuture<'static, Result<Vec<Connection>>> {
            let connections = self.connections.lock().unwrap().clone();
            Box::pin(async move { Ok(connections) })
        }

        fn open(
            &self,
            integration: &'static str,
            action: &'static str,
            connection: Option<String>,
            return_to: &'static str,
        ) -> BoxFuture<'static, Result<()>> {
            self.opened.lock().unwrap().push((
                integration.into(),
                action.into(),
                connection.clone(),
                return_to.into(),
            ));
            let mut connections = self.connections.lock().unwrap();
            match action {
                "disconnect" => {
                    connections.retain(|item| Some(&item.connection_id) != connection.as_ref())
                }
                _ => connections.extend(self.on_connect.lock().unwrap().drain(..)),
            }
            Box::pin(async { Ok(()) })
        }

        fn post(&self, path: String, body: Value) -> BoxFuture<'static, Result<Value>> {
            self.posts.lock().unwrap().push((path.clone(), body));
            let mut responses = self.responses.lock().unwrap();
            let response = responses
                .iter()
                .position(|(key, _)| *key == path)
                .map(|index| responses.remove(index).1)
                .unwrap_or_else(|| Err(failure("no response")));
            Box::pin(async move { response })
        }
    }
}
