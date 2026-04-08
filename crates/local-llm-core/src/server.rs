#[cfg(target_arch = "aarch64")]
mod inner {
    use std::net::Ipv4Addr;
    use std::path::Path;

    use axum::http::StatusCode;
    use hypr_llm_cactus::{CompleteService, ModelManagerBuilder};
    use tokio::net::TcpListener;
    use tower_http::cors::CorsLayer;

    use crate::{Error, ModelSelection};

    pub struct LlmServer {
        base_url: String,
        shutdown_tx: tokio::sync::watch::Sender<()>,
        task: tokio::task::JoinHandle<()>,
    }

    impl LlmServer {
        pub async fn start(selection: &ModelSelection, models_dir: &Path) -> Result<Self, Error> {
            let models_base = models_dir.parent().unwrap_or(models_dir);
            let file_path = selection.install_path(models_base);
            let name = selection.display_name();

            Self::start_with_model_path(name, file_path).await
        }

        pub async fn start_with_resolver(
            selection: &ModelSelection,
            models_base: &Path,
            resolve_resource: impl FnMut(&str) -> Result<Option<std::path::PathBuf>, Error>,
        ) -> Result<Self, Error> {
            let file_path = selection.resolve_path(models_base, resolve_resource)?;
            let name = selection.display_name();

            Self::start_with_model_path(name, file_path).await
        }

        pub async fn start_with_model_path(
            name: String,
            file_path: impl AsRef<Path>,
        ) -> Result<Self, Error> {
            let file_path = file_path.as_ref().to_path_buf();
            if !file_path.exists() {
                return Err(Error::ModelNotDownloaded);
            }

            let manager = ModelManagerBuilder::default()
                .register(name.clone(), file_path)
                .default_model(name)
                .build();

            let router = CompleteService::new(manager)
                .into_router(handle_error)
                .layer(CorsLayer::permissive());

            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0u16)).await?;
            let addr = listener.local_addr()?;
            let base_url = format!("http://{}/v1", addr);

            let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(());

            let task = tokio::spawn(async move {
                axum::serve(listener, router)
                    .with_graceful_shutdown(async move {
                        let _ = shutdown_rx.changed().await;
                    })
                    .await
                    .ok();
            });

            tracing::info!(url = %base_url, "local LLM server started");

            Ok(Self {
                base_url,
                shutdown_tx,
                task,
            })
        }

        pub fn url(&self) -> &str {
            &self.base_url
        }

        pub async fn stop(self) {
            let _ = self.shutdown_tx.send(());
            let _ = self.task.await;
            tracing::info!("local LLM server stopped");
        }
    }

    async fn handle_error(err: hypr_llm_cactus::Error) -> (StatusCode, String) {
        (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    }
}

#[cfg(not(target_arch = "aarch64"))]
mod inner {
    use std::path::Path;

    use crate::{Error, ModelSelection};

    pub struct LlmServer {
        _private: (),
    }

    impl LlmServer {
        pub async fn start(_selection: &ModelSelection, _models_dir: &Path) -> Result<Self, Error> {
            Err(Error::Other(
                "Local LLM is not supported on this platform".to_string(),
            ))
        }

        pub async fn start_with_resolver(
            _selection: &ModelSelection,
            _models_base: &Path,
            _resolve_resource: impl FnMut(&str) -> Result<Option<std::path::PathBuf>, Error>,
        ) -> Result<Self, Error> {
            Err(Error::Other(
                "Local LLM is not supported on this platform".to_string(),
            ))
        }

        pub async fn start_with_model_path(
            _name: String,
            _file_path: impl AsRef<Path>,
        ) -> Result<Self, Error> {
            Err(Error::Other(
                "Local LLM is not supported on this platform".to_string(),
            ))
        }

        pub fn url(&self) -> &str {
            unreachable!()
        }

        pub async fn stop(self) {}
    }
}

pub use inner::LlmServer;
