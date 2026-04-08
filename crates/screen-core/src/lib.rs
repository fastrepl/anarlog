use std::time::{SystemTime, UNIX_EPOCH};

use std::cmp::{max, min};

use image::{
    ExtendedColorType, ImageEncoder, RgbaImage, codecs::webp::WebPEncoder, imageops::FilterType,
};
use xcap::{Monitor, Window, XCapError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureStrategy {
    WindowOnly,
    WindowWithContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowMetadata {
    pub id: u32,
    pub pid: u32,
    pub app_name: String,
    pub title: String,
    pub rect: CaptureRect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowContextCaptureOptions {
    pub image_policy: WindowContextImagePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowContextImagePolicy {
    pub max_long_side: u32,
}

impl Default for WindowContextCaptureOptions {
    fn default() -> Self {
        Self {
            image_policy: WindowContextImagePolicy::default(),
        }
    }
}

impl Default for WindowContextImagePolicy {
    fn default() -> Self {
        Self::siglip_text_heavy()
    }
}

impl WindowContextImagePolicy {
    pub fn siglip_text_heavy() -> Self {
        Self {
            max_long_side: 1920,
        }
    }

    fn normalized(&self) -> Self {
        Self {
            max_long_side: self.max_long_side.clamp(512, 2048),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowContextImage {
    pub image_bytes: Vec<u8>,
    pub mime_type: String,
    pub captured_at_ms: i64,
    pub width: u32,
    pub height: u32,
    pub strategy: CaptureStrategy,
    pub crop: CaptureRect,
    pub window: WindowMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowCaptureTarget {
    pub pid: u32,
    pub app_name: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no focused window is available for capture")]
    NoFocusedWindow,
    #[error("focused window has invalid bounds")]
    InvalidWindowBounds,
    #[error("window is outside the bounds of its current monitor")]
    WindowOutsideMonitor,
    #[error(transparent)]
    Xcap(#[from] XCapError),
    #[error(transparent)]
    Image(#[from] image::ImageError),
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn capture_frontmost_window_context(
    options: WindowContextCaptureOptions,
) -> Result<WindowContextImage> {
    capture_window_context(None, options)
}

pub fn capture_target_window_context(
    target: &WindowCaptureTarget,
    options: WindowContextCaptureOptions,
) -> Result<WindowContextImage> {
    capture_window_context(Some(target), options)
}

fn capture_window_context(
    target: Option<&WindowCaptureTarget>,
    options: WindowContextCaptureOptions,
) -> Result<WindowContextImage> {
    let image_policy = options.image_policy.normalized();
    let window = resolve_window(target)?;
    let metadata = window_metadata(&window)?;
    let monitor = window.current_monitor()?;
    let monitor_rect = monitor_rect(&monitor)?;

    if metadata.rect.width == 0 || metadata.rect.height == 0 {
        return Err(Error::InvalidWindowBounds);
    }

    let (crop, strategy) = compute_capture_rect(metadata.rect, monitor_rect)?;
    let local_x = (crop.x - monitor_rect.x) as u32;
    let local_y = (crop.y - monitor_rect.y) as u32;

    let image = monitor.capture_region(local_x, local_y, crop.width, crop.height)?;
    let image = resize_for_model(image, image_policy.max_long_side);
    let (width, height) = image.dimensions();
    let encoded = encode_webp_lossless(&image)?;

    Ok(WindowContextImage {
        image_bytes: encoded.bytes,
        mime_type: encoded.mime_type.to_string(),
        captured_at_ms: unix_ms(SystemTime::now()),
        width,
        height,
        strategy,
        crop,
        window: metadata,
    })
}

fn resolve_window(target: Option<&WindowCaptureTarget>) -> Result<Window> {
    let windows = Window::all()?;
    if let Some(target) = target {
        return select_matching_window(&windows, target).ok_or(Error::NoFocusedWindow);
    }

    resolve_focused_window(&windows)
}

fn resolve_focused_window(windows: &[Window]) -> Result<Window> {
    let Some(frontmost_pid) = windows.iter().find_map(|window| {
        let is_minimized = window.is_minimized().ok()?;
        let is_focused = window.is_focused().ok()?;
        (!is_minimized && is_focused)
            .then(|| window.pid().ok())
            .flatten()
    }) else {
        return Err(Error::NoFocusedWindow);
    };

    select_matching_window(
        windows,
        &WindowCaptureTarget {
            pid: frontmost_pid,
            app_name: None,
            title: None,
        },
    )
    .ok_or(Error::NoFocusedWindow)
}

fn select_matching_window(windows: &[Window], target: &WindowCaptureTarget) -> Option<Window> {
    windows
        .iter()
        .filter_map(|window| {
            let pid = window.pid().ok()?;
            let is_minimized = window.is_minimized().ok()?;
            let width = window.width().ok()?;
            let height = window.height().ok()?;
            if is_minimized || width == 0 || height == 0 {
                return None;
            }

            Some((
                candidate_match_score(
                    target,
                    pid,
                    window.app_name().ok().as_deref(),
                    window.title().ok().as_deref(),
                )?,
                window,
            ))
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, window)| window.clone())
}

fn candidate_match_score(
    target: &WindowCaptureTarget,
    pid: u32,
    app_name: Option<&str>,
    title: Option<&str>,
) -> Option<u8> {
    if pid != target.pid {
        return None;
    }

    let normalized_target_title = target.title.as_deref().filter(|value| !value.is_empty());
    let normalized_target_app_name = target.app_name.as_deref().filter(|value| !value.is_empty());

    if let Some(target_title) = normalized_target_title {
        if title == Some(target_title) {
            return Some(0);
        }
        if normalized_target_app_name.is_none() {
            return Some(2);
        }
    }

    if let Some(target_app_name) = normalized_target_app_name
        && app_name == Some(target_app_name)
    {
        return Some(1);
    }

    Some(2)
}

fn window_metadata(window: &Window) -> Result<WindowMetadata> {
    Ok(WindowMetadata {
        id: window.id()?,
        pid: window.pid()?,
        app_name: window.app_name()?,
        title: window.title().unwrap_or_default(),
        rect: CaptureRect {
            x: window.x()?,
            y: window.y()?,
            width: window.width()?,
            height: window.height()?,
        },
    })
}

fn monitor_rect(monitor: &Monitor) -> Result<CaptureRect> {
    Ok(CaptureRect {
        x: monitor.x()?,
        y: monitor.y()?,
        width: monitor.width()?,
        height: monitor.height()?,
    })
}

fn compute_capture_rect(
    window: CaptureRect,
    monitor: CaptureRect,
) -> Result<(CaptureRect, CaptureStrategy)> {
    let window_area = window.width as f32 * window.height as f32;
    let monitor_area = monitor.width as f32 * monitor.height as f32;
    if window_area <= 0.0 || monitor_area <= 0.0 {
        return Err(Error::InvalidWindowBounds);
    }

    let ratio = window_area / monitor_area;
    let (scale, min_padding) = match ratio {
        ratio if ratio >= 0.65 => (1.0_f32, 24_i64),
        ratio if ratio >= 0.35 => (1.18_f32, 40_i64),
        ratio if ratio >= 0.18 => (1.42_f32, 64_i64),
        _ => (1.8_f32, 96_i64),
    };

    let desired_width = max(
        (window.width as f32 * scale).round() as i64,
        window.width as i64 + min_padding * 2,
    );
    let desired_height = max(
        (window.height as f32 * scale).round() as i64,
        window.height as i64 + min_padding * 2,
    );

    let crop = clamp_rect_around_window(window, monitor, desired_width, desired_height)?;
    let strategy = if crop.width <= window.width + 64 && crop.height <= window.height + 64 {
        CaptureStrategy::WindowOnly
    } else {
        CaptureStrategy::WindowWithContext
    };

    Ok((crop, strategy))
}

fn clamp_rect_around_window(
    window: CaptureRect,
    monitor: CaptureRect,
    desired_width: i64,
    desired_height: i64,
) -> Result<CaptureRect> {
    let monitor_left = monitor.x as i64;
    let monitor_top = monitor.y as i64;
    let monitor_width = monitor.width as i64;
    let monitor_height = monitor.height as i64;
    let monitor_right = monitor_left + monitor_width;
    let monitor_bottom = monitor_top + monitor_height;

    let window_left = window.x as i64;
    let window_top = window.y as i64;
    let window_right = window_left + window.width as i64;
    let window_bottom = window_top + window.height as i64;

    if window_left >= monitor_right
        || window_right <= monitor_left
        || window_top >= monitor_bottom
        || window_bottom <= monitor_top
    {
        return Err(Error::WindowOutsideMonitor);
    }

    let clamped_width = min(desired_width, monitor_width);
    let clamped_height = min(desired_height, monitor_height);

    let center_x = window_left + window.width as i64 / 2;
    let center_y = window_top + window.height as i64 / 2;

    let min_x = monitor_left;
    let max_x = monitor_right - clamped_width;
    let min_y = monitor_top;
    let max_y = monitor_bottom - clamped_height;

    let x = (center_x - clamped_width / 2).clamp(min_x, max_x);
    let y = (center_y - clamped_height / 2).clamp(min_y, max_y);

    Ok(CaptureRect {
        x: x as i32,
        y: y as i32,
        width: clamped_width as u32,
        height: clamped_height as u32,
    })
}

fn resize_for_model(image: RgbaImage, max_long_side: u32) -> RgbaImage {
    let (width, height) = image.dimensions();
    let long_side = width.max(height);
    if long_side <= max_long_side {
        return image;
    }

    let scale = max_long_side as f32 / long_side as f32;
    let resized_width = max(1, (width as f32 * scale).round() as u32);
    let resized_height = max(1, (height as f32 * scale).round() as u32);

    image::imageops::resize(&image, resized_width, resized_height, FilterType::Lanczos3)
}

struct EncodedImage {
    bytes: Vec<u8>,
    mime_type: &'static str,
}

fn encode_webp_lossless(image: &RgbaImage) -> Result<EncodedImage> {
    let mut bytes = Vec::new();
    WebPEncoder::new_lossless(&mut bytes).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ExtendedColorType::Rgba8,
    )?;
    Ok(EncodedImage {
        bytes,
        mime_type: "image/webp",
    })
}

fn unix_ms(value: SystemTime) -> i64 {
    match value.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis().min(i64::MAX as u128) as i64,
        Err(error) => -(error.duration().as_millis().min(i64::MAX as u128) as i64),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CaptureRect, CaptureStrategy, WindowCaptureTarget, WindowContextImagePolicy,
        candidate_match_score, clamp_rect_around_window, compute_capture_rect,
        encode_webp_lossless,
    };
    use image::RgbaImage;

    #[test]
    fn small_window_gets_context() {
        let window = CaptureRect {
            x: 600,
            y: 260,
            width: 480,
            height: 320,
        };
        let monitor = CaptureRect {
            x: 0,
            y: 0,
            width: 1728,
            height: 1117,
        };

        let (crop, strategy) = compute_capture_rect(window, monitor).unwrap();

        assert_eq!(strategy, CaptureStrategy::WindowWithContext);
        assert!(crop.width > window.width);
        assert!(crop.height > window.height);
    }

    #[test]
    fn large_window_stays_tight() {
        let window = CaptureRect {
            x: 20,
            y: 20,
            width: 1500,
            height: 980,
        };
        let monitor = CaptureRect {
            x: 0,
            y: 0,
            width: 1728,
            height: 1117,
        };

        let (crop, strategy) = compute_capture_rect(window, monitor).unwrap();

        assert_eq!(strategy, CaptureStrategy::WindowOnly);
        assert!(crop.width <= window.width + 64);
        assert!(crop.height <= window.height + 64);
    }

    #[test]
    fn crop_is_clamped_to_monitor() {
        let window = CaptureRect {
            x: 1450,
            y: 900,
            width: 320,
            height: 240,
        };
        let monitor = CaptureRect {
            x: 0,
            y: 0,
            width: 1728,
            height: 1117,
        };

        let crop = clamp_rect_around_window(window, monitor, 900, 700).unwrap();

        assert!(crop.x >= monitor.x);
        assert!(crop.y >= monitor.y);
        assert!(crop.x as i64 + crop.width as i64 <= monitor.width as i64);
        assert!(crop.y as i64 + crop.height as i64 <= monitor.height as i64);
    }

    #[test]
    fn target_matching_prefers_exact_title() {
        let target = WindowCaptureTarget {
            pid: 42,
            app_name: Some("Arc".to_string()),
            title: Some("PR Review".to_string()),
        };

        assert_eq!(
            candidate_match_score(&target, 42, Some("Arc"), Some("PR Review")),
            Some(0)
        );
        assert_eq!(
            candidate_match_score(&target, 42, Some("Arc"), Some("Inbox")),
            Some(1)
        );
        assert_eq!(
            candidate_match_score(&target, 42, Some("Other"), Some("Inbox")),
            Some(2)
        );
        assert_eq!(
            candidate_match_score(&target, 99, Some("Arc"), Some("PR Review")),
            None
        );
    }

    #[test]
    fn default_policy_prefers_lossless_webp() {
        let policy = WindowContextImagePolicy::default();

        assert_eq!(policy.max_long_side, 1920);
    }

    #[test]
    fn encode_webp_uses_webp_container() {
        let image = RgbaImage::from_raw(1, 1, vec![0, 0, 0, 255]).unwrap();
        let encoded = encode_webp_lossless(&image).unwrap();

        assert_eq!(encoded.mime_type, "image/webp");
        assert_eq!(&encoded.bytes[..4], b"RIFF");
        assert_eq!(&encoded.bytes[8..12], b"WEBP");
    }
}
