use tauri::WebviewWindow;

use crate::OverlayOptions;

pub struct Overlay<'a, M: tauri::Manager<tauri::Wry>> {
    manager: &'a M,
}

impl<'a, M: tauri::Manager<tauri::Wry>> Overlay<'a, M> {
    pub async fn spawn_listener(&self, window: WebviewWindow) {
        self.spawn_listener_with_options(window, OverlayOptions::default())
            .await;
    }

    pub async fn spawn_listener_with_options(
        &self,
        window: WebviewWindow,
        options: OverlayOptions,
    ) {
        let app = self.manager.app_handle().clone();
        crate::spawn_overlay_listener(app, window, options).await;
    }

    pub async fn abort_listener(&self, window_label: &str) {
        let app = self.manager.app_handle();
        crate::abort_overlay_listener(app, window_label).await;
    }
}

pub trait OverlayPluginExt {
    fn overlay(&self) -> Overlay<'_, Self>
    where
        Self: tauri::Manager<tauri::Wry> + Sized;
}

impl<T: tauri::Manager<tauri::Wry>> OverlayPluginExt for T {
    fn overlay(&self) -> Overlay<'_, Self>
    where
        Self: Sized,
    {
        Overlay { manager: self }
    }
}
