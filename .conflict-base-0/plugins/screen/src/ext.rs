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
    Display,
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

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DisplayContextMetadata {
    pub id: u32,
    pub name: String,
    pub rect: CaptureRect,
    pub is_primary: bool,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CaptureSubject {
    Window { window: WindowContextMetadata },
    Display { display: DisplayContextMetadata },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowCaptureTarget {
    pub window_id: Option<u32>,
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
    pub subject: CaptureSubject,
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
                window_id: target.window_id,
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
            hypr_screen_core::CaptureStrategy::Display => CaptureStrategy::Display,
        },
        crop: CaptureRect {
            x: capture.crop.x,
            y: capture.crop.y,
            width: capture.crop.width,
            height: capture.crop.height,
        },
        subject: match capture.subject {
            hypr_screen_core::CaptureSubject::Window(window) => CaptureSubject::Window {
                window: WindowContextMetadata {
                    id: window.id,
                    pid: window.pid,
                    app_name: window.app_name,
                    title: window.title,
                    rect: CaptureRect {
                        x: window.rect.x,
                        y: window.rect.y,
                        width: window.rect.width,
                        height: window.rect.height,
                    },
                },
            },
            hypr_screen_core::CaptureSubject::Display(display) => CaptureSubject::Display {
                display: DisplayContextMetadata {
                    id: display.id,
                    name: display.name,
                    rect: CaptureRect {
                        x: display.rect.x,
                        y: display.rect.y,
                        width: display.rect.width,
                        height: display.rect.height,
                    },
                    is_primary: display.is_primary,
                },
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> hypr_screen_core::CaptureRect {
        hypr_screen_core::CaptureRect {
            x: 1,
            y: 2,
            width: 300,
            height: 200,
        }
    }

    #[test]
    fn maps_window_subject_capture() {
        let capture = map_capture(hypr_screen_core::WindowContextImage {
            image_bytes: vec![1, 2, 3],
            mime_type: "image/png".to_string(),
            captured_at_ms: 10,
            width: 300,
            height: 200,
            strategy: hypr_screen_core::CaptureStrategy::WindowWithContext,
            crop: rect(),
            subject: hypr_screen_core::CaptureSubject::Window(hypr_screen_core::WindowMetadata {
                id: 7,
                pid: 42,
                app_name: "Ghostty".to_string(),
                title: "cargo run".to_string(),
                rect: rect(),
            }),
        });

        assert!(matches!(
            capture.strategy,
            CaptureStrategy::WindowWithContext
        ));
        assert!(matches!(
            capture.subject,
            CaptureSubject::Window { window }
                if window.id == 7 && window.pid == 42 && window.app_name == "Ghostty"
        ));
    }

    #[test]
    fn maps_display_subject_capture() {
        let capture = map_capture(hypr_screen_core::WindowContextImage {
            image_bytes: vec![1, 2, 3],
            mime_type: "image/png".to_string(),
            captured_at_ms: 10,
            width: 400,
            height: 300,
            strategy: hypr_screen_core::CaptureStrategy::Display,
            crop: rect(),
            subject: hypr_screen_core::CaptureSubject::Display(hypr_screen_core::DisplayMetadata {
                id: 3,
                name: "Built-in Retina Display".to_string(),
                rect: rect(),
                is_primary: true,
            }),
        });

        assert!(matches!(capture.strategy, CaptureStrategy::Display));
        assert!(matches!(
            capture.subject,
            CaptureSubject::Display { display }
                if display.id == 3
                    && display.name == "Built-in Retina Display"
                    && display.is_primary
        ));
    }
}
