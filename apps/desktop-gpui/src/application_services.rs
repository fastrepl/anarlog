use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anlg_calendar::CalendarProviderType;
use anlg_desktop_identity::{Channel, Locations, auth::AuthPaths};
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError};
use futures::{
    SinkExt,
    channel::{mpsc, oneshot},
    future::BoxFuture,
};
use serde_json::json;

use crate::{
    launch::Storage,
    meeting::{
        MeetingServices,
        capture::Phase,
        config::{CloudAccess, ProviderServices},
    },
    platform::permissions::NativePermissions,
    product::{
        cloud::{
            ChannelStore, CloudConfig, CloudServices, SecretNames,
            auth::{KeyringStore, SecureStore, SessionStore, ShippingSession},
            host::NativeHost,
        },
        local::{
            HostEffects, LocalServices,
            calendar::{AccessToken, CalendarAuth, CalendarService},
            developers::DeveloperTools,
            failure,
            models::{EngineResources, ModelService, NativeEngine},
        },
        services::{Mutation, Outcome, Panel, ProductServices, Request, Surface},
    },
};

const SANDBOX_IDENTIFIER: &str = "com.anarlog.gpui.sandbox";

pub enum HostCommand {
    Flush(oneshot::Sender<Result<()>>),
    Pause(oneshot::Sender<Result<()>>),
    Resume(oneshot::Sender<Result<()>>),
    Relaunch(oneshot::Sender<Result<()>>),
}

pub struct Effects {
    sender: mpsc::Sender<HostCommand>,
    runtime: RuntimeHandle,
    capture: Mutex<Option<MeetingServices>>,
    native: Arc<Mutex<Option<NativeEngine>>>,
}

impl Effects {
    pub fn channel(runtime: RuntimeHandle) -> (Arc<Self>, mpsc::Receiver<HostCommand>) {
        let (sender, receiver) = mpsc::channel(8);
        (
            Arc::new(Self {
                sender,
                runtime,
                capture: Mutex::new(None),
                native: Arc::default(),
            }),
            receiver,
        )
    }

    pub fn attach(&self, services: MeetingServices) {
        *self.capture.lock().unwrap_or_else(|e| e.into_inner()) = Some(services);
    }

    fn idle(&self) -> Result<()> {
        let services = self.capture.lock().map_err(failure)?.clone();
        if services.is_some_and(|s| {
            matches!(
                s.capture.take_update(u64::MAX).phase,
                Phase::Loading | Phase::Listening | Phase::Finalizing
            )
        }) {
            return Err(failure("Finish the recording before changing models"));
        }
        Ok(())
    }

    fn request(
        &self,
        command: fn(oneshot::Sender<Result<()>>) -> HostCommand,
    ) -> BoxFuture<'static, Result<()>> {
        let mut sender = self.sender.clone();
        Box::pin(async move {
            let (reply, receive) = oneshot::channel();
            sender
                .send(command(reply))
                .await
                .map_err(|_| ServiceError::Closed)?;
            receive.await.map_err(|_| ServiceError::Closed)?
        })
    }
}

impl HostEffects for Effects {
    fn flush_drafts(&self) -> BoxFuture<'static, Result<()>> {
        self.request(HostCommand::Flush)
    }
    fn pause_writers(&self) -> BoxFuture<'static, Result<()>> {
        self.request(HostCommand::Pause)
    }
    fn resume_writers(&self) -> BoxFuture<'static, Result<()>> {
        self.request(HostCommand::Resume)
    }
    fn relaunch(&self) -> BoxFuture<'static, Result<()>> {
        self.request(HostCommand::Relaunch)
    }

    fn use_model(
        &self,
        model: anlg_local_model::LocalModel,
        _: String,
        native: Option<NativeEngine>,
    ) -> BoxFuture<'static, Result<()>> {
        let idle = self.idle();
        let runtime = self.runtime.clone();
        let current = self.native.clone();
        Box::pin(async move {
            idle?;
            let kind = if matches!(model, anlg_local_model::LocalModel::GgufLlm(_)) {
                "llm"
            } else {
                "stt"
            };
            let result = runtime.submit(move |services| async move {
                let statements = [
                    (format!("current_{kind}_provider"), if kind == "llm" { "local" } else { "anarlog" }.to_owned()),
                    (format!("current_{kind}_model"), model.cli_name().to_owned()),
                ].into_iter().map(|(key, value)| anlg_db_execute::TransactionStatement {
                    sql: "INSERT INTO app_settings (id,value_json,updated_at) VALUES (?,?,strftime('%Y-%m-%dT%H:%M:%fZ','now')) ON CONFLICT(id) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at".into(),
                    params: vec![json!(key), json!(json!(value).to_string())], expected_rows_affected: Some(1),
                }).collect();
                services.executor.execute_transaction(statements).await.map_err(failure)?;
                Ok(())
            })?.receive().await;
            if result.is_ok() {
                if let Some(native) = native {
                    let previous = current.lock().map_err(failure)?.replace(native);
                    if let Some(previous) = previous {
                        tokio::task::spawn_blocking(move || previous.stop())
                            .await
                            .map_err(failure)??;
                    }
                }
            } else if let Some(native) = native {
                tokio::task::spawn_blocking(move || native.stop())
                    .await
                    .map_err(failure)??;
            }
            result
        })
    }

    fn stop_model(&self) -> BoxFuture<'static, Result<()>> {
        let idle = self.idle();
        let native = self.native.clone();
        Box::pin(async move {
            idle?;
            let session = native.lock().map_err(failure)?.take();
            if let Some(session) = session {
                tokio::task::spawn_blocking(move || session.stop())
                    .await
                    .map_err(failure)??;
            }
            Ok(())
        })
    }
}

#[derive(Clone)]
pub struct CloudConnection {
    pub service: Option<CloudServices>,
    pub error: ServiceError,
}

impl CloudConnection {
    fn configured(&self) -> Result<CloudServices> {
        self.service.clone().ok_or_else(|| self.error.clone())
    }
}

impl ProductServices for CloudConnection {
    fn load(&self, surface: Surface, request: Request) -> BoxFuture<'static, Result<Panel>> {
        let service = self.configured();
        Box::pin(async move { service?.load(surface, request).await })
    }
    fn perform(
        &self,
        surface: Surface,
        request: Request,
        mutation: Mutation,
    ) -> BoxFuture<'static, Result<Outcome>> {
        let service = self.configured();
        Box::pin(async move { service?.perform(surface, request, mutation).await })
    }
}

impl CalendarAuth for CloudConnection {
    fn token(&self, cancel: CancellationToken) -> BoxFuture<'static, Result<AccessToken>> {
        let service = self.configured();
        Box::pin(async move { service?.token(cancel).await })
    }
    fn connect(
        &self,
        provider: CalendarProviderType,
        cancel: CancellationToken,
    ) -> BoxFuture<'static, Result<()>> {
        let service = self.configured();
        Box::pin(async move { service?.connect(provider, cancel).await })
    }
    fn disconnect(
        &self,
        provider: CalendarProviderType,
        connection: String,
        cancel: CancellationToken,
    ) -> BoxFuture<'static, Result<()>> {
        let service = self.configured();
        Box::pin(async move { service?.disconnect(provider, connection, cancel).await })
    }
}

pub struct NativeServices {
    pub local: Arc<LocalServices>,
    pub cloud: CloudConnection,
    pub providers: ProviderServices,
}

impl NativeServices {
    pub async fn new(
        runtime: RuntimeHandle,
        storage: Storage,
        effects: Arc<Effects>,
        host: Arc<NativeHost>,
    ) -> Result<Self> {
        let api = std::env::var("VITE_API_URL").unwrap_or_else(|_| "https://api.anarlog.so".into());
        let cloud_config = config(&runtime, &storage, &api).await;
        let shipping_store = storage.shipping_store();
        let profile = storage.root;
        // A channel run uses the shipping channel's secure store and names, so the secrets the
        // shipping app saved, the recovery key among them, are found in place.
        let (identifier, store): (&str, Arc<dyn SecureStore>) = match storage.channel {
            Some(channel) => (
                channel.identifier(),
                Arc::new(ChannelStore::keyring(channel.identifier())),
            ),
            None => (
                SANDBOX_IDENTIFIER,
                Arc::new(KeyringStore::new("com.anarlog.gpui.sandbox.secure-store")),
            ),
        };
        let cloud = match cloud_config.and_then(|config| {
            let session = storage
                .channel
                .map(|channel| shipping_session(channel, &config));
            Ok((config, session.transpose()?))
        }) {
            Ok((config, session)) => {
                match CloudServices::with_session(runtime.clone(), config, store, session, host)
                    .await
                {
                    Ok(service) => CloudConnection {
                        service: Some(service),
                        error: ServiceError::Closed,
                    },
                    Err(error) => CloudConnection {
                        service: None,
                        error,
                    },
                }
            }
            Err(error) => CloudConnection {
                service: None,
                error,
            },
        };
        let resources = crate::platform::package::Resources::current();
        let executable = |name| {
            resources
                .as_ref()
                .map(|resources| resources.executable(name))
                .unwrap_or_default()
        };
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .ok_or_else(|| failure("No home directory configured"))?;
        let models = Arc::new(ModelService::new(
            profile.join("models"),
            EngineResources {
                argmax: executable("argmax"),
                llama_server: executable("llama-server"),
                argmax_api_key: std::env::var("ARGMAX_API_KEY").unwrap_or_default(),
            },
        ));
        let model_runtime = runtime.clone();
        let model_service = models.clone();
        let provider_cloud = cloud.clone();
        let providers = ProviderServices {
            runtime: runtime.clone(),
            api_url: api.clone(),
            cloud: Arc::new(move || {
                let cloud = provider_cloud.configured();
                Box::pin(async move { cloud?.provider_access().await })
                    as BoxFuture<'static, Result<CloudAccess>>
            }),
            local: Arc::new(move |request| {
                let models = model_service.clone();
                let runtime = model_runtime.clone();
                Box::pin(async move {
                    runtime
                        .service(move |services| async move {
                            if let Some(path) = request.file {
                                if !path.is_file() {
                                    return Err(failure(
                                        "The selected local model file does not exist",
                                    ));
                                }
                                return Ok(path.to_string_lossy().into_owned());
                            }
                            let model = ModelService::resolve(&request.model)?;
                            let cancel = services.shutdown_requested.clone();
                            if request.batch {
                                models.batch(model, cancel).await
                            } else {
                                models.start(model, cancel).await
                            }
                        })?
                        .receive()
                        .await
                })
            }),
            secret: crate::meeting::config::keyring_secrets(identifier.into()),
            secret_write: crate::meeting::subscription::keyring_writer(identifier.into()),
        };
        let stop_models = models.clone();
        let stop_effects = effects.clone();
        runtime
            .register_shutdown(
                desktop_runtime::ShutdownPhase::StopServices,
                Box::new(move || {
                    Box::pin(async move {
                        let (native, engines) =
                            tokio::join!(stop_effects.stop_model(), stop_models.stop());
                        native.and(engines)
                    })
                }),
            )?
            .receive()
            .await?;
        let local = Arc::new(LocalServices {
            runtime,
            cloud: Arc::new(cloud.clone()),
            host: effects,
            permissions: NativePermissions::new(Arc::new(anlg_audio_actual::ActualAudio)),
            models,
            calendar: CalendarService::new(api.into(), Arc::new(cloud.clone())),
            developers: DeveloperTools {
                // Shipping's `externalBin` sidecar, beside the app's executable.
                bundled_cli: executable("anarlog-cli"),
                home,
                command_name: crate::platform::package::channel()
                    .map_or("anarlog", anlg_desktop_identity::Channel::executable),
                version: crate::platform::package::VERSION,
            },
            storage_root: profile,
            database_dir: storage.database_dir.clone(),
            storage_pointer: storage.pointer,
            shipping_store,
        });
        Ok(Self {
            local,
            cloud,
            providers,
        })
    }
}

async fn config(runtime: &RuntimeHandle, storage: &Storage, api: &str) -> Result<CloudConfig> {
    let supabase = std::env::var("VITE_SUPABASE_URL").or_else(|_| option_env!("VITE_SUPABASE_URL").map(str::to_owned).ok_or(std::env::VarError::NotPresent))
        .map_err(|_| failure("Set VITE_SUPABASE_URL and VITE_SUPABASE_ANON_KEY for native cloud sign-in, then restart"))?;
    let anon_key = std::env::var("VITE_SUPABASE_ANON_KEY")
        .or_else(|_| {
            option_env!("VITE_SUPABASE_ANON_KEY")
                .map(str::to_owned)
                .ok_or(std::env::VarError::NotPresent)
        })
        .map_err(|_| {
            failure("Set VITE_SUPABASE_ANON_KEY for native cloud sign-in, then restart")
        })?;
    let channel = storage.channel;
    let profile = storage.root.clone();
    let (fingerprint, device_name) = runtime
        .service(move |_| async move {
            tokio::task::spawn_blocking(move || {
                let device_name = anlg_desktop_identity::device::device_name()
                    .unwrap_or_else(|| "Anarlog native desktop".into());
                // Shipping's machine-derived fingerprint keeps this device's sync slot. A
                // sandbox profile is a separate device.
                if channel.is_some() {
                    return Ok((anlg_desktop_identity::device::fingerprint(), device_name));
                }
                let path = profile.join("device-id");
                if path.exists() {
                    let id = std::fs::read_to_string(path).map_err(failure)?;
                    return Ok((id, device_name));
                }
                let id = uuid::Uuid::new_v4().to_string();
                anlg_storage::fs::atomic_write(&path, &id).map_err(failure)?;
                Ok((id, device_name))
            })
            .await
            .map_err(failure)?
        })?
        .receive()
        .await?;
    Ok(CloudConfig {
        supabase: supabase.parse().map_err(failure)?,
        api: api.parse().map_err(failure)?,
        web: std::env::var("VITE_WEB_APP_URL")
            .unwrap_or_else(|_| "https://anarlog.so".into())
            .parse()
            .map_err(failure)?,
        anon_key,
        callback: sign_in_callback(channel).parse().map_err(failure)?,
        secrets: match channel {
            Some(_) => SecretNames::Channel,
            None => SecretNames::Sandbox(format!("gpui-{fingerprint}")),
        },
        device_fingerprint: fingerprint,
        device_name,
    })
}

/// Shipping's web sign-in returns to `<scheme>://auth/callback`, with the scheme the app passes.
fn sign_in_callback(channel: Option<Channel>) -> String {
    let scheme = channel.map_or("anarlog-dev", |channel| channel.schemes()[0]);
    format!("{scheme}://auth/callback")
}

/// A channel run keeps the sign-in where the shipping app does, so the update keeps the user
/// signed in and a rollback to the Tauri app still finds the session.
fn shipping_session(channel: Channel, config: &CloudConfig) -> Result<Arc<dyn SessionStore>> {
    let locations = Locations::resolve(channel.identifier())
        .ok_or_else(|| failure("The application data directory is unavailable"))?;
    let store = anlg_desktop_identity::session::SessionStore::for_platform(
        AuthPaths::new(&locations),
        channel.identifier(),
    );
    Ok(Arc::new(ShippingSession::new(store, &config.supabase)))
}

#[cfg(test)]
mod tests {
    /// `crates/desktop-identity` pins the same values on the shipping toolchain; a
    /// `DefaultHasher` change would make every user a new sync device.
    #[test]
    fn fingerprint_matches_the_shipping_toolchain() {
        for (machine_uid, expected) in [
            ("", "30406ea523c53def"),
            ("00000000-0000-0000-0000-000000000000", "c104447f85eea341"),
            ("6F8E2A0C-3B1D-4E5F-9A7B-1C2D3E4F5A6B", "5681594ddfa3a7"),
            ("{6F8E2A0C-3B1D-4E5F-9A7B-1C2D3E4F5A6B}", "8136fff4c7357ff5"),
            ("0123456789abcdef0123456789abcdef", "b7fc4b7cafcbcea0"),
        ] {
            assert_eq!(
                anlg_desktop_identity::device::fingerprint_from(machine_uid),
                expected,
                "{machine_uid:?}"
            );
        }
    }
}
