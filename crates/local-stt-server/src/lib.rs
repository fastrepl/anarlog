use std::path::PathBuf;
use std::sync::Arc;

use axum::http::StatusCode;
use tower_http::cors::{self, CorsLayer};

mod axum_server;
pub mod events;
pub mod runtime;

pub use axum_server::LocalAxumServer;

use runtime::{LocalServerRuntime, NoopRuntime};

pub struct LocalSttServer {
    inner: LocalAxumServer,
}

impl LocalSttServer {
    pub async fn start(model_path: PathBuf) -> std::io::Result<Self> {
        Self::start_with_config(model_path, hypr_transcribe_cactus::CactusConfig::default()).await
    }

    pub async fn start_with_config(
        model_path: PathBuf,
        cactus_config: hypr_transcribe_cactus::CactusConfig,
    ) -> std::io::Result<Self> {
        Self::start_with_runtime(Arc::new(NoopRuntime), model_path, cactus_config).await
    }

    pub async fn start_with_runtime(
        runtime: Arc<dyn LocalServerRuntime>,
        model_path: PathBuf,
        cactus_config: hypr_transcribe_cactus::CactusConfig,
    ) -> std::io::Result<Self> {
        tracing::info!(model_path = %model_path.display(), "starting local STT server");

        let router = hypr_transcribe_cactus::TranscribeService::builder()
            .model_path(model_path)
            .cactus_config(cactus_config)
            .build()
            .into_router(|err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) })
            .layer(
                CorsLayer::new()
                    .allow_origin(cors::Any)
                    .allow_methods(cors::Any)
                    .allow_headers(cors::Any),
            );

        let inner = LocalAxumServer::start_with_runtime(
            runtime,
            router,
            hypr_transcribe_cactus::LISTEN_PATH,
        )
        .await?;

        tracing::info!(base_url = %inner.base_url(), "local STT server ready");

        Ok(Self { inner })
    }

    pub fn base_url(&self) -> &str {
        self.inner.base_url()
    }

    pub fn stop(&mut self) {
        self.inner.stop();
    }
}

impl Drop for LocalSttServer {
    fn drop(&mut self) {
        self.stop();
    }
}
