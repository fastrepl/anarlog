use base64::{Engine as _, engine::general_purpose::STANDARD};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowContextImagePolicy {
    pub max_long_side: Option<u32>,
}

impl Default for WindowContextImagePolicy {
    fn default() -> Self {
        Self {
            max_long_side: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowContextCaptureOptions {
    pub image_policy: Option<WindowContextImagePolicy>,
}

impl Default for WindowContextCaptureOptions {
    fn default() -> Self {
        Self { image_policy: None }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CaptureStrategy {
    WindowOnly,
    WindowWithContext,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowContextMetadata {
    pub id: u32,
    pub pid: u32,
    pub app_name: String,
    pub title: String,
    pub rect: CaptureRect,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowCaptureTarget {
    pub pid: u32,
    pub app_name: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowContextCapture {
    pub mime_type: String,
    pub data_base64: String,
    pub captured_at_ms: i64,
    pub width: u32,
    pub height: u32,
    pub strategy: CaptureStrategy,
    pub crop: CaptureRect,
    pub window: WindowContextMetadata,
}

pub struct Screen<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Screen<'a, R, M> {
    pub fn capture_frontmost_window_context(
        &self,
        options: WindowContextCaptureOptions,
    ) -> Result<WindowContextCapture, crate::Error> {
        let _ = self.manager;
        let capture = hypr_screen_core::capture_frontmost_window_context(map_options(options))?;

        Ok(map_capture(capture))
    }

    pub fn capture_target_window_context(
        &self,
        target: WindowCaptureTarget,
        options: WindowContextCaptureOptions,
    ) -> Result<WindowContextCapture, crate::Error> {
        let _ = self.manager;
        let capture = hypr_screen_core::capture_target_window_context(
            &hypr_screen_core::WindowCaptureTarget {
                pid: target.pid,
                app_name: target.app_name,
                title: target.title,
            },
            map_options(options),
        )?;

        Ok(map_capture(capture))
    }
}

fn map_options(
    options: WindowContextCaptureOptions,
) -> hypr_screen_core::WindowContextCaptureOptions {
    let default_policy = hypr_screen_core::WindowContextImagePolicy::default();
    let image_policy = options.image_policy.unwrap_or_default();
    hypr_screen_core::WindowContextCaptureOptions {
        image_policy: hypr_screen_core::WindowContextImagePolicy {
            max_long_side: image_policy
                .max_long_side
                .unwrap_or(default_policy.max_long_side),
        },
    }
}

fn map_capture(capture: hypr_screen_core::WindowContextImage) -> WindowContextCapture {
    WindowContextCapture {
        mime_type: capture.mime_type,
        data_base64: STANDARD.encode(capture.image_bytes),
        captured_at_ms: capture.captured_at_ms,
        width: capture.width,
        height: capture.height,
        strategy: match capture.strategy {
            hypr_screen_core::CaptureStrategy::WindowOnly => CaptureStrategy::WindowOnly,
            hypr_screen_core::CaptureStrategy::WindowWithContext => {
                CaptureStrategy::WindowWithContext
            }
        },
        crop: CaptureRect {
            x: capture.crop.x,
            y: capture.crop.y,
            width: capture.crop.width,
            height: capture.crop.height,
        },
        window: WindowContextMetadata {
            id: capture.window.id,
            pid: capture.window.pid,
            app_name: capture.window.app_name,
            title: capture.window.title,
            rect: CaptureRect {
                x: capture.window.rect.x,
                y: capture.window.rect.y,
                width: capture.window.rect.width,
                height: capture.window.rect.height,
            },
        },
    }
}

pub trait ScreenPluginExt<R: tauri::Runtime> {
    fn screen(&self) -> Screen<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> ScreenPluginExt<R> for T {
    fn screen(&self) -> Screen<'_, R, Self>
    where
        Self: Sized,
    {
        Screen {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
