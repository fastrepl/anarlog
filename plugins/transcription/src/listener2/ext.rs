use std::sync::Arc;

use hypr_transcription_core::listener2 as core;
use tauri_specta::Event;

pub struct Listener2<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Listener2<'a, R, M> {
    pub async fn run_batch(
        &self,
        params: core::BatchParams,
    ) -> Result<core::BatchRunOutput, core::Error> {
        let state = self.manager.state::<crate::SharedState>();
        let guard = state.lock().await;
        let app = guard.app.clone();
        drop(guard);

        let runtime = Arc::new(TauriBatchRuntime { app });
        core::run_batch(runtime, params).await
    }

    pub async fn run_denoise(&self, params: core::DenoiseParams) -> Result<(), core::Error> {
        let state = self.manager.state::<crate::SharedState>();
        let guard = state.lock().await;
        let app = guard.app.clone();
        drop(guard);

        let runtime = Arc::new(TauriDenoiseRuntime { app });
        core::run_denoise(runtime, params).await
    }

    pub fn parse_subtitle(&self, path: String) -> Result<core::Subtitle, String> {
        core::parse_subtitle_from_path(path)
    }

    pub fn export_to_vtt(
        &self,
        session_id: String,
        words: Vec<core::VttWord>,
    ) -> Result<String, String> {
        use tauri_plugin_settings::SettingsPluginExt;

        let base = self
            .manager
            .settings()
            .vault_base()
            .map_err(|e| e.to_string())?;
        let session_dir = base.join("sessions").join(&session_id);

        std::fs::create_dir_all(&session_dir).map_err(|e| e.to_string())?;

        let vtt_path = session_dir.join("transcript.vtt");

        core::export_words_to_vtt_file(words, &vtt_path)?;
        Ok(vtt_path.to_string())
    }
}

pub trait Listener2PluginExt<R: tauri::Runtime> {
    fn listener2(&self) -> Listener2<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> Listener2PluginExt<R> for T {
    fn listener2(&self) -> Listener2<'_, R, Self>
    where
        Self: Sized,
    {
        Listener2 {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}

struct TauriBatchRuntime {
    app: tauri::AppHandle,
}

impl core::BatchRuntime for TauriBatchRuntime {
    fn emit(&self, event: core::BatchEvent) {
        let _ = event.emit(&self.app);
    }
}

struct TauriDenoiseRuntime {
    app: tauri::AppHandle,
}

impl core::DenoiseRuntime for TauriDenoiseRuntime {
    fn emit(&self, event: core::DenoiseEvent) {
        let _ = event.emit(&self.app);
    }
}
