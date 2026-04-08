use std::{collections::HashMap, future::Future, path::PathBuf, sync::Arc};

use tauri::{Manager, Runtime, ipc::Channel};
use tauri_plugin_store2::Store2PluginExt;

use hypr_model_downloader::{DownloadableModel, ModelDownloadManager, ModelDownloaderRuntime};

use crate::store::TauriModelStore;

struct TauriModelRuntime<R: Runtime> {
    app_handle: tauri::AppHandle<R>,
    channels: Arc<std::sync::Mutex<HashMap<String, Channel<i8>>>>,
}

impl<R: Runtime> ModelDownloaderRuntime<crate::SupportedModel> for TauriModelRuntime<R> {
    fn models_base(&self) -> Result<PathBuf, hypr_model_downloader::Error> {
        use tauri_plugin_settings::SettingsPluginExt;
        Ok(self
            .app_handle
            .settings()
            .global_base()
            .map(|base| base.join("models").into_std_path_buf())
            .unwrap_or_else(|_| dirs::data_dir().unwrap_or_default().join("models")))
    }

    fn emit_progress(
        &self,
        model: &crate::SupportedModel,
        status: hypr_model_downloader::DownloadStatus,
    ) {
        use hypr_model_downloader::DownloadStatus;

        let progress: i8 = match &status {
            DownloadStatus::Downloading(p) => *p as i8,
            DownloadStatus::Completed => 100,
            DownloadStatus::Failed(_) => -1,
        };

        let key = model.download_key();
        let mut guard = self.channels.lock().unwrap();

        let Some(channel) = guard.get(&key) else {
            return;
        };

        let send_result = channel.send(progress);
        let is_terminal = matches!(
            status,
            DownloadStatus::Completed | DownloadStatus::Failed(_)
        );
        if send_result.is_err() || is_terminal {
            guard.remove(&key);
        }
    }
}

pub fn create_model_downloader<R: Runtime>(
    app_handle: &tauri::AppHandle<R>,
    channels: Arc<std::sync::Mutex<HashMap<String, Channel<i8>>>>,
) -> ModelDownloadManager<crate::SupportedModel> {
    let runtime = Arc::new(TauriModelRuntime {
        app_handle: app_handle.clone(),
        channels,
    });
    ModelDownloadManager::new(runtime)
}

fn models_base<R: Runtime, T: Manager<R>>(manager: &T) -> PathBuf {
    use tauri_plugin_settings::SettingsPluginExt;

    manager
        .settings()
        .global_base()
        .map(|base| base.join("models").into_std_path_buf())
        .unwrap_or_else(|_| dirs::data_dir().unwrap_or_default().join("models"))
}

fn bundled_resource_candidates(relative_path: &str) -> Vec<String> {
    let mut candidates = vec![relative_path.to_string()];
    if cfg!(debug_assertions) {
        candidates.push(format!("resources/{relative_path}"));
    }
    candidates
}

fn resolve_resource_path<R: Runtime, T: Manager<R>>(
    manager: &T,
    relative_path: &str,
) -> Result<Option<PathBuf>, hypr_local_llm_core::Error> {
    use tauri::path::BaseDirectory;

    for candidate in bundled_resource_candidates(relative_path) {
        let path = manager
            .path()
            .resolve(&candidate, BaseDirectory::Resource)
            .map_err(|error| hypr_local_llm_core::Error::Other(error.to_string()))?;
        if path.exists() {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

pub trait LocalLlmPluginExt<R: Runtime> {
    fn local_llm_store(&self) -> tauri_plugin_store2::ScopedStore<R, crate::StoreKey>;

    fn models_dir(&self) -> PathBuf;

    fn list_downloaded_model(
        &self,
    ) -> impl Future<Output = Result<Vec<crate::SupportedModel>, crate::Error>>;

    fn list_custom_models(
        &self,
    ) -> impl Future<Output = Result<Vec<crate::CustomModelInfo>, crate::Error>>;
    fn get_current_model(&self) -> Result<crate::SupportedModel, crate::Error>;
    fn set_current_model(&self, model: crate::SupportedModel) -> Result<(), crate::Error>;
    fn get_current_model_selection(&self) -> Result<crate::ModelSelection, crate::Error>;
    fn set_current_model_selection(&self, model: crate::ModelSelection)
    -> Result<(), crate::Error>;

    fn download_model(
        &self,
        model: crate::SupportedModel,
        channel: Channel<i8>,
    ) -> impl Future<Output = Result<(), crate::Error>>;
    fn cancel_download(
        &self,
        model: crate::SupportedModel,
    ) -> impl Future<Output = Result<bool, crate::Error>>;
    fn delete_model(
        &self,
        model: &crate::SupportedModel,
    ) -> impl Future<Output = Result<(), crate::Error>>;
    fn is_model_downloading(&self, model: &crate::SupportedModel) -> impl Future<Output = bool>;
    fn is_model_downloaded(
        &self,
        model: &crate::SupportedModel,
    ) -> impl Future<Output = Result<bool, crate::Error>>;

    fn start_server(&self) -> impl Future<Output = Result<String, crate::Error>>;
    fn stop_server(&self) -> impl Future<Output = Result<(), crate::Error>>;
    fn server_url(&self) -> impl Future<Output = Result<Option<String>, crate::Error>>;
}

impl<R: Runtime, T: Manager<R>> LocalLlmPluginExt<R> for T {
    fn local_llm_store(&self) -> tauri_plugin_store2::ScopedStore<R, crate::StoreKey> {
        self.store2().scoped_store(crate::PLUGIN_NAME).unwrap()
    }

    fn models_dir(&self) -> PathBuf {
        hypr_local_llm_core::llm_models_dir(&models_base(self))
    }

    #[tracing::instrument(skip_all)]
    async fn is_model_downloading(&self, model: &crate::SupportedModel) -> bool {
        let downloader = {
            let state = self.state::<crate::SharedState>();
            let guard = state.lock().await;
            guard.model_downloader.clone()
        };

        downloader.is_downloading(model).await
    }

    #[tracing::instrument(skip_all)]
    async fn is_model_downloaded(
        &self,
        model: &crate::SupportedModel,
    ) -> Result<bool, crate::Error> {
        let downloader = {
            let state = self.state::<crate::SharedState>();
            let guard = state.lock().await;
            guard.model_downloader.clone()
        };
        Ok(downloader.is_downloaded(model).await?)
    }

    #[tracing::instrument(skip_all)]
    async fn download_model(
        &self,
        model: crate::SupportedModel,
        channel: Channel<i8>,
    ) -> Result<(), crate::Error> {
        let download_model = model;
        let key = download_model.download_key();

        let (downloader, channels) = {
            let state = self.state::<crate::SharedState>();
            let guard = state.lock().await;
            (
                guard.model_downloader.clone(),
                guard.download_channels.clone(),
            )
        };

        downloader.cancel_download(&download_model).await?;

        {
            let mut guard = channels.lock().unwrap();
            if let Some(existing) = guard.insert(key.clone(), channel) {
                let _ = existing.send(-1);
            }
        }

        if let Err(e) = downloader.download(&download_model).await {
            let mut guard = channels.lock().unwrap();
            if let Some(channel) = guard.remove(&key) {
                let _ = channel.send(-1);
            }
            return Err(e.into());
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn cancel_download(&self, model: crate::SupportedModel) -> Result<bool, crate::Error> {
        let downloader = {
            let state = self.state::<crate::SharedState>();
            let guard = state.lock().await;
            guard.model_downloader.clone()
        };

        Ok(downloader.cancel_download(&model).await?)
    }

    #[tracing::instrument(skip_all)]
    async fn delete_model(&self, model: &crate::SupportedModel) -> Result<(), crate::Error> {
        let downloader = {
            let state = self.state::<crate::SharedState>();
            let guard = state.lock().await;
            guard.model_downloader.clone()
        };

        downloader.delete(model).await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn list_downloaded_model(&self) -> Result<Vec<crate::SupportedModel>, crate::Error> {
        Ok(hypr_local_llm_core::list_downloaded_models(
            &self.models_dir(),
        )?)
    }

    #[tracing::instrument(skip_all)]
    fn get_current_model(&self) -> Result<crate::SupportedModel, crate::Error> {
        let store = self.local_llm_store();
        let tauri_store = TauriModelStore::new(&store);
        Ok(hypr_local_llm_core::get_current_model(
            &tauri_store,
            &self.models_dir(),
        )?)
    }

    #[tracing::instrument(skip_all)]
    fn set_current_model(&self, model: crate::SupportedModel) -> Result<(), crate::Error> {
        let store = self.local_llm_store();
        let tauri_store = TauriModelStore::new(&store);
        Ok(hypr_local_llm_core::set_current_model(&tauri_store, model)?)
    }

    #[tracing::instrument(skip_all)]
    async fn list_custom_models(&self) -> Result<Vec<crate::CustomModelInfo>, crate::Error> {
        Ok(hypr_local_llm_core::list_custom_models()?)
    }

    #[tracing::instrument(skip_all)]
    fn get_current_model_selection(&self) -> Result<crate::ModelSelection, crate::Error> {
        let store = self.local_llm_store();
        let tauri_store = TauriModelStore::new(&store);
        Ok(hypr_local_llm_core::get_current_model_selection(
            &tauri_store,
            &self.models_dir(),
        )?)
    }

    #[tracing::instrument(skip_all)]
    fn set_current_model_selection(
        &self,
        model: crate::ModelSelection,
    ) -> Result<(), crate::Error> {
        let store = self.local_llm_store();
        let tauri_store = TauriModelStore::new(&store);
        Ok(hypr_local_llm_core::set_current_model_selection(
            &tauri_store,
            model,
        )?)
    }

    #[tracing::instrument(skip_all)]
    async fn start_server(&self) -> Result<String, crate::Error> {
        let state = self.state::<crate::SharedState>();

        let existing_server = {
            let mut guard = state.lock().await;
            guard.server.take()
        };

        if let Some(server) = existing_server {
            server.stop().await;
        }

        let selection = self.get_current_model_selection()?;
        let models_base = models_base(self);
        let server = hypr_local_llm_core::LlmServer::start_with_resolver(
            &selection,
            &models_base,
            |relative_path| resolve_resource_path(self, relative_path),
        )
        .await?;
        let url = server.url().to_string();

        {
            let mut guard = state.lock().await;
            guard.server = Some(server);
        }

        Ok(url)
    }

    #[tracing::instrument(skip_all)]
    async fn stop_server(&self) -> Result<(), crate::Error> {
        let state = self.state::<crate::SharedState>();

        let existing_server = {
            let mut guard = state.lock().await;
            guard.server.take()
        };

        if let Some(server) = existing_server {
            server.stop().await;
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn server_url(&self) -> Result<Option<String>, crate::Error> {
        let state = self.state::<crate::SharedState>();
        let guard = state.lock().await;

        Ok(guard.server.as_ref().map(|s| s.url().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_resource_candidates_include_dev_fallback() {
        let candidates = bundled_resource_candidates("models/cactus/char-vlm/weight");

        assert_eq!(candidates[0], "models/cactus/char-vlm/weight");
        if cfg!(debug_assertions) {
            assert_eq!(candidates[1], "resources/models/cactus/char-vlm/weight");
        }
    }
}
