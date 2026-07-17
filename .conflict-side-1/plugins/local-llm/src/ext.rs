use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tauri::{Manager, Runtime, ipc::Channel};

use hypr_model_downloader::{DownloadableModel, ModelDownloadManager, ModelDownloaderRuntime};

struct TauriModelRuntime<R: Runtime> {
    app_handle: tauri::AppHandle<R>,
    channels: Arc<std::sync::Mutex<HashMap<String, Channel<i8>>>>,
}

impl<R: Runtime> ModelDownloaderRuntime<crate::SupportedModel> for TauriModelRuntime<R> {
    fn models_base(&self) -> Result<PathBuf, hypr_model_downloader::Error> {
        Ok(models_base(&self.app_handle))
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

async fn downloader<R: Runtime>(
    manager: &impl Manager<R>,
) -> ModelDownloadManager<crate::SupportedModel> {
    let state = manager.state::<crate::SharedState>();
    state.lock().await.model_downloader.clone()
}

pub struct LocalLlmExt<'a, R: Runtime, M: Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: Runtime, M: Manager<R>> LocalLlmExt<'a, R, M> {
    pub fn models_dir(&self) -> PathBuf {
        hypr_local_llm_core::llm_models_dir(&models_base(self.manager))
    }

    #[tracing::instrument(skip_all)]
    pub async fn is_model_downloading(&self, model: &crate::SupportedModel) -> bool {
        downloader(self.manager).await.is_downloading(model).await
    }

    #[tracing::instrument(skip_all)]
    pub async fn is_model_downloaded(
        &self,
        model: &crate::SupportedModel,
    ) -> Result<bool, crate::Error> {
        Ok(downloader(self.manager).await.is_downloaded(model).await?)
    }

    #[tracing::instrument(skip_all)]
    pub async fn server_url(&self) -> Result<Option<String>, crate::Error> {
        let state = self.manager.state::<crate::SharedState>();
        let guard = state.lock().await;

        Ok(guard.server.as_ref().map(|server| server.url().to_string()))
    }

    #[tracing::instrument(skip_all)]
    pub async fn download_model(
        &self,
        model: crate::SupportedModel,
        channel: Channel<i8>,
    ) -> Result<(), crate::Error> {
        let key = model.download_key();

        let (dl, channels) = {
            let state = self.manager.state::<crate::SharedState>();
            let guard = state.lock().await;
            (
                guard.model_downloader.clone(),
                guard.download_channels.clone(),
            )
        };

        dl.cancel_download(&model).await?;

        {
            let mut guard = channels.lock().unwrap();
            if let Some(existing) = guard.insert(key.clone(), channel) {
                let _ = existing.send(-1);
            }
        }

        if let Err(e) = dl.download(&model).await {
            let mut guard = channels.lock().unwrap();
            if let Some(channel) = guard.remove(&key) {
                let _ = channel.send(-1);
            }
            return Err(e.into());
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub async fn cancel_download(
        &self,
        model: crate::SupportedModel,
    ) -> Result<bool, crate::Error> {
        Ok(downloader(self.manager)
            .await
            .cancel_download(&model)
            .await?)
    }

    #[tracing::instrument(skip_all)]
    pub async fn delete_model(&self, model: &crate::SupportedModel) -> Result<(), crate::Error> {
        downloader(self.manager).await.delete(model).await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub async fn list_downloaded_model(&self) -> Result<Vec<crate::SupportedModel>, crate::Error> {
        Ok(hypr_local_llm_core::list_downloaded_models(
            &self.models_dir(),
        )?)
    }

    #[tracing::instrument(skip_all)]
    pub async fn list_custom_models(&self) -> Result<Vec<crate::CustomModelInfo>, crate::Error> {
        Ok(hypr_local_llm_core::list_custom_models()?)
    }

    pub fn start_server(&self) {}
}

pub trait LocalLlmPluginExt<R: Runtime> {
    fn local_llm(&self) -> LocalLlmExt<'_, R, Self>
    where
        Self: Manager<R> + Sized;
}

impl<R: Runtime, T: Manager<R>> LocalLlmPluginExt<R> for T {
    fn local_llm(&self) -> LocalLlmExt<'_, R, Self>
    where
        Self: Sized,
    {
        LocalLlmExt {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
