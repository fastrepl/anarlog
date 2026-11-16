use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anlg_calendar::CalendarProviderType;
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError};
use futures::{
    SinkExt,
    channel::{mpsc, oneshot},
    future::BoxFuture,
};
use serde_json::json;

use crate::{
    meeting::{
        MeetingServices,
        capture::Phase,
        config::{CloudAccess, ProviderServices},
    },
    platform::permissions::NativePermissions,
    product::{
        cloud::{CloudConfig, CloudServices, auth::KeyringStore, host::NativeHost},
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
        profile: PathBuf,
        pointer: PathBuf,
        effects: Arc<Effects>,
        host: Arc<NativeHost>,
    ) -> Result<Self> {
        let api = std::env::var("VITE_API_URL").unwrap_or_else(|_| "https://api.anarlog.so".into());
        let cloud_config = config(&runtime, profile.clone(), &api).await;
        let cloud = match cloud_config {
            Ok(config) => match CloudServices::new(
                runtime.clone(),
                config,
                Arc::new(KeyringStore::new("com.anarlog.gpui.sandbox.secure-store")),
                host,
            )
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
            },
            Err(error) => CloudConnection {
                service: None,
                error,
            },
        };
        let resources = std::env::var_os("ANARLOG_NATIVE_RESOURCES")
            .map(PathBuf::from)
            .unwrap_or(
                std::env::current_exe()
                    .map_err(failure)?
                    .parent()
                    .ok_or(ServiceError::Closed)?
                    .join("resources"),
            );
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .ok_or_else(|| failure("No home directory configured"))?;
        let models = Arc::new(ModelService::new(
            profile.join("models"),
            EngineResources {
                argmax: resources.join(if cfg!(windows) {
                    "argmax.exe"
                } else {
                    "argmax"
                }),
                llama_server: resources.join(if cfg!(windows) {
                    "llama-server.exe"
                } else {
                    "llama-server"
                }),
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
                            models
                                .start(model, services.shutdown_requested.clone())
                                .await
                        })?
                        .receive()
                        .await
                })
            }),
            secret: crate::meeting::config::keyring_secrets("com.anarlog.gpui.sandbox".into()),
            secret_write: crate::meeting::subscription::keyring_writer(
                "com.anarlog.gpui.sandbox".into(),
            ),
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
                bundled_cli: resources.join(if cfg!(windows) {
                    "anarlog.exe"
                } else {
                    "anarlog"
                }),
                installed_cli: home.join(".local/bin").join(if cfg!(windows) {
                    "anarlog.exe"
                } else {
                    "anarlog"
                }),
                skills_bundle: resources.join("skills"),
                home,
            },
            storage_root: profile,
            storage_pointer: pointer,
        });
        Ok(Self {
            local,
            cloud,
            providers,
        })
    }
}

async fn config(runtime: &RuntimeHandle, profile: PathBuf, api: &str) -> Result<CloudConfig> {
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
    let fingerprint = runtime
        .service(move |_| async move {
            tokio::task::spawn_blocking(move || {
                let path = profile.join("device-id");
                if path.exists() {
                    return std::fs::read_to_string(path).map_err(failure);
                }
                let id = uuid::Uuid::new_v4().to_string();
                anlg_storage::fs::atomic_write(&path, &id).map_err(failure)?;
                Ok(id)
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
        callback: "anarlog-dev://auth/callback".parse().map_err(failure)?,
        credential_namespace: format!("gpui-{fingerprint}"),
        device_fingerprint: fingerprint,
        device_name: std::env::var("HOSTNAME").unwrap_or_else(|_| "Anarlog native desktop".into()),
    })
}
