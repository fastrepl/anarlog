use std::sync::Arc;

use tauri::Manager;

use crate::camera::CameraWatermark;

pub trait CameraWatermarkPluginExt<R: tauri::Runtime> {
    fn camera_watermark(&self) -> Arc<CameraWatermark>;
}

impl<R: tauri::Runtime, T: Manager<R>> CameraWatermarkPluginExt<R> for T {
    fn camera_watermark(&self) -> Arc<CameraWatermark> {
        self.state::<Arc<CameraWatermark>>().inner().clone()
    }
}
