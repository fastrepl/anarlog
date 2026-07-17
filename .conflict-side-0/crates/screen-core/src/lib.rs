use std::{
    cmp::{max, min},
    time::{SystemTime, UNIX_EPOCH},
};

use image::{
    ExtendedColorType, ImageEncoder, RgbaImage, codecs::png::PngEncoder, imageops::FilterType,
};
use xcap::{Monitor, Window, XCapError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureStrategy {
    WindowOnly,
    WindowWithContext,
    Display,
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
pub struct DisplayMetadata {
    pub id: u32,
    pub name: String,
    pub rect: CaptureRect,
    pub is_primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureSubject {
    Window(WindowMetadata),
    Display(DisplayMetadata),
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
    pub subject: CaptureSubject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowCaptureTarget {
    pub window_id: Option<u32>,
    pub pid: u32,
    pub app_name: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no capture source is available")]
    NoCaptureSource,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureStage {
    ExactTargetWindow,
    SamePidWindow,
    FrontmostWindow,
    PrimaryDisplay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WindowCandidate {
    id: u32,
    pid: u32,
    app_name: Option<String>,
    title: Option<String>,
    is_minimized: bool,
    width: u32,
    height: u32,
    is_focused: Option<bool>,
}

pub fn capture_frontmost_window_context(
    options: WindowContextCaptureOptions,
) -> Result<WindowContextImage> {
    capture_with_plan(
        None,
        options,
        [CaptureStage::FrontmostWindow, CaptureStage::PrimaryDisplay],
    )
}

pub fn capture_target_window_context(
    target: &WindowCaptureTarget,
    options: WindowContextCaptureOptions,
) -> Result<WindowContextImage> {
    capture_with_plan(
        Some(target),
        options,
        [CaptureStage::ExactTargetWindow, CaptureStage::SamePidWindow],
    )
}

fn capture_with_plan<I>(
    target: Option<&WindowCaptureTarget>,
    options: WindowContextCaptureOptions,
    stages: I,
) -> Result<WindowContextImage>
where
    I: IntoIterator<Item = CaptureStage>,
{
    let image_policy = options.image_policy.normalized();
    execute_capture_plan(stages, |stage| match stage {
        CaptureStage::ExactTargetWindow => {
            let Some(target) = target else {
                return Ok(None);
            };
            let Some(window) = resolve_exact_target_window(target)? else {
                return Ok(None);
            };
            capture_window_source(&window, image_policy.clone()).map(Some)
        }
        CaptureStage::SamePidWindow => {
            let Some(target) = target else {
                return Ok(None);
            };
            let Some(window) = resolve_same_pid_window(target)? else {
                return Ok(None);
            };
            capture_window_source(&window, image_policy.clone()).map(Some)
        }
        CaptureStage::FrontmostWindow => {
            let Some(window) = resolve_frontmost_window()? else {
                return Ok(None);
            };
            capture_window_source(&window, image_policy.clone()).map(Some)
        }
        CaptureStage::PrimaryDisplay => {
            let Some(monitor) = resolve_primary_monitor()? else {
                return Ok(None);
            };
            capture_display_source(&monitor, image_policy.clone()).map(Some)
        }
    })
}

fn execute_capture_plan<I, F, T>(stages: I, mut attempt: F) -> Result<T>
where
    I: IntoIterator<Item = CaptureStage>,
    F: FnMut(CaptureStage) -> Result<Option<T>>,
{
    let mut last_error = None;

    for stage in stages {
        match attempt(stage) {
            Ok(Some(value)) => return Ok(value),
            Ok(None) => {}
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or(Error::NoCaptureSource))
}

fn resolve_exact_target_window(target: &WindowCaptureTarget) -> Result<Option<Window>> {
    if target.window_id.is_none()
        && target
            .title
            .as_deref()
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return Ok(None);
    }
    let windows = Window::all()?;
    let candidates = collect_window_candidates(&windows);
    Ok(select_exact_target_candidate(&candidates, target))
}

fn resolve_same_pid_window(target: &WindowCaptureTarget) -> Result<Option<Window>> {
    if target.window_id.is_some() {
        return Ok(None);
    }
    let windows = Window::all()?;
    let candidates = collect_window_candidates(&windows);
    Ok(select_same_pid_best_match_candidate(&candidates, target))
}

fn resolve_frontmost_window() -> Result<Option<Window>> {
    let windows = Window::all()?;
    let candidates = collect_window_candidates(&windows);
    Ok(select_frontmost_candidate(&candidates))
}

fn resolve_primary_monitor() -> Result<Option<Monitor>> {
    let monitors = Monitor::all()?;
    Ok(select_primary_monitor(&monitors))
}

fn collect_window_candidates(windows: &[Window]) -> Vec<(WindowCandidate, Window)> {
    windows
        .iter()
        .filter_map(|window| {
            Some((
                WindowCandidate {
                    id: window.id().ok()?,
                    pid: window.pid().ok()?,
                    app_name: window.app_name().ok(),
                    title: window.title().ok(),
                    is_minimized: window.is_minimized().ok()?,
                    width: window.width().ok()?,
                    height: window.height().ok()?,
                    is_focused: window.is_focused().ok(),
                },
                window.clone(),
            ))
        })
        .collect()
}

fn select_exact_target_candidate<T: Clone>(
    candidates: &[(WindowCandidate, T)],
    target: &WindowCaptureTarget,
) -> Option<T> {
    candidates
        .iter()
        .find(|(candidate, _)| {
            if !is_usable_candidate(candidate) {
                return false;
            }

            if let Some(window_id) = target.window_id {
                return candidate.id == window_id;
            }

            let Some(target_title) = target.title.as_deref().filter(|value| !value.is_empty())
            else {
                return false;
            };

            candidate.pid == target.pid && candidate.title.as_deref() == Some(target_title)
        })
        .map(|(_, value)| value.clone())
}

fn select_same_pid_best_match_candidate<T: Clone>(
    candidates: &[(WindowCandidate, T)],
    target: &WindowCaptureTarget,
) -> Option<T> {
    candidates
        .iter()
        .filter_map(|(candidate, value)| {
            same_pid_match_score(target, candidate).map(|score| (score, value))
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, value)| value.clone())
}

fn select_frontmost_candidate<T: Clone>(candidates: &[(WindowCandidate, T)]) -> Option<T> {
    candidates
        .iter()
        .find(|(candidate, _)| is_usable_candidate(candidate) && candidate.is_focused == Some(true))
        .map(|(_, value)| value.clone())
}

fn select_primary_monitor(monitors: &[Monitor]) -> Option<Monitor> {
    monitors
        .iter()
        .find(|monitor| monitor.is_primary().ok() == Some(true))
        .cloned()
        .or_else(|| monitors.first().cloned())
}

fn is_usable_candidate(candidate: &WindowCandidate) -> bool {
    !candidate.is_minimized && candidate.width > 0 && candidate.height > 0
}

fn same_pid_match_score(target: &WindowCaptureTarget, candidate: &WindowCandidate) -> Option<u8> {
    if target.window_id.is_some() {
        return None;
    }

    if !is_usable_candidate(candidate) || candidate.pid != target.pid {
        return None;
    }

    let normalized_target_app_name = target.app_name.as_deref().filter(|value| !value.is_empty());
    let normalized_target_title = target.title.as_deref().filter(|value| !value.is_empty());

    if let Some(target_app_name) = normalized_target_app_name
        && candidate.app_name.as_deref() == Some(target_app_name)
    {
        return Some(0);
    }

    if let Some(target_title) = normalized_target_title
        && candidate.title.as_deref() == Some(target_title)
    {
        return Some(1);
    }

    Some(2)
}

fn capture_window_source(
    window: &Window,
    image_policy: WindowContextImagePolicy,
) -> Result<WindowContextImage> {
    let metadata = window_metadata(window)?;
    let monitor = window.current_monitor()?;
    let monitor_rect = monitor_rect(&monitor)?;

    if metadata.rect.width == 0 || metadata.rect.height == 0 {
        return Err(Error::InvalidWindowBounds);
    }

    let (crop, strategy) = compute_capture_rect(metadata.rect, monitor_rect)?;
    let local_x = (crop.x - monitor_rect.x) as u32;
    let local_y = (crop.y - monitor_rect.y) as u32;

    let image = monitor.capture_region(local_x, local_y, crop.width, crop.height)?;
    build_capture_image(
        image,
        image_policy,
        strategy,
        crop,
        CaptureSubject::Window(metadata),
    )
}

fn capture_display_source(
    monitor: &Monitor,
    image_policy: WindowContextImagePolicy,
) -> Result<WindowContextImage> {
    let metadata = display_metadata(monitor)?;
    let crop = metadata.rect;
    let image = monitor.capture_image()?;

    build_capture_image(
        image,
        image_policy,
        CaptureStrategy::Display,
        crop,
        CaptureSubject::Display(metadata),
    )
}

fn build_capture_image(
    image: RgbaImage,
    image_policy: WindowContextImagePolicy,
    strategy: CaptureStrategy,
    crop: CaptureRect,
    subject: CaptureSubject,
) -> Result<WindowContextImage> {
    let image = resize_for_model(image, image_policy.max_long_side);
    let (width, height) = image.dimensions();
    let encoded = encode_png(&image)?;

    Ok(WindowContextImage {
        image_bytes: encoded.bytes,
        mime_type: encoded.mime_type.to_string(),
        captured_at_ms: unix_ms(SystemTime::now()),
        width,
        height,
        strategy,
        crop,
        subject,
    })
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

fn display_metadata(monitor: &Monitor) -> Result<DisplayMetadata> {
    let id = monitor.id()?;
    let name = monitor
        .friendly_name()
        .or_else(|_| monitor.name())
        .unwrap_or_else(|_| format!("display-{id}"));

    Ok(DisplayMetadata {
        id,
        name,
        rect: monitor_rect(monitor)?,
        is_primary: monitor.is_primary()?,
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

fn encode_png(image: &RgbaImage) -> Result<EncodedImage> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ExtendedColorType::Rgba8,
    )?;
    Ok(EncodedImage {
        bytes,
        mime_type: "image/png",
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
        CaptureRect, CaptureStage, CaptureStrategy, Error, WindowCandidate, WindowCaptureTarget,
        WindowContextImagePolicy, clamp_rect_around_window, compute_capture_rect, encode_png,
        execute_capture_plan, same_pid_match_score, select_exact_target_candidate,
        select_frontmost_candidate, select_same_pid_best_match_candidate,
    };
    use image::RgbaImage;

    fn candidate(
        id: u32,
        pid: u32,
        app_name: Option<&str>,
        title: Option<&str>,
        is_focused: Option<bool>,
    ) -> WindowCandidate {
        WindowCandidate {
            id,
            pid,
            app_name: app_name.map(str::to_string),
            title: title.map(str::to_string),
            is_minimized: false,
            width: 800,
            height: 600,
            is_focused,
        }
    }

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
            window_id: None,
            pid: 42,
            app_name: Some("Arc".to_string()),
            title: Some("PR Review".to_string()),
        };
        let candidates = vec![
            (
                candidate(1, 42, Some("Arc"), Some("Inbox"), Some(false)),
                1usize,
            ),
            (
                candidate(2, 42, Some("Arc"), Some("PR Review"), Some(false)),
                2usize,
            ),
            (
                candidate(3, 99, Some("Arc"), Some("PR Review"), Some(false)),
                3usize,
            ),
        ];

        assert_eq!(select_exact_target_candidate(&candidates, &target), Some(2));
        assert_eq!(
            select_same_pid_best_match_candidate(&candidates, &target),
            Some(1)
        );
    }

    #[test]
    fn same_pid_matching_prefers_app_name_then_title() {
        let target = WindowCaptureTarget {
            window_id: None,
            pid: 42,
            app_name: Some("Arc".to_string()),
            title: Some("PR Review".to_string()),
        };
        let app_match = candidate(1, 42, Some("Arc"), Some("Inbox"), Some(false));
        let title_match = candidate(2, 42, Some("Other"), Some("PR Review"), Some(false));
        let weak_match = candidate(3, 42, Some("Other"), Some("Else"), Some(false));

        assert_eq!(same_pid_match_score(&target, &app_match), Some(0));
        assert_eq!(same_pid_match_score(&target, &title_match), Some(1));
        assert_eq!(same_pid_match_score(&target, &weak_match), Some(2));
    }

    #[test]
    fn exact_target_matching_prefers_window_id_when_available() {
        let target = WindowCaptureTarget {
            window_id: Some(7),
            pid: 42,
            app_name: Some("Arc".to_string()),
            title: Some("PR Review".to_string()),
        };
        let candidates = vec![
            (
                candidate(6, 42, Some("Arc"), Some("PR Review"), Some(false)),
                1usize,
            ),
            (
                candidate(7, 99, Some("Other"), Some("Else"), Some(false)),
                2usize,
            ),
        ];

        assert_eq!(select_exact_target_candidate(&candidates, &target), Some(2));
        assert_eq!(
            select_same_pid_best_match_candidate(&candidates, &target),
            None
        );
    }

    #[test]
    fn frontmost_candidate_requires_focus_flag() {
        let candidates = vec![
            (candidate(1, 1, Some("Arc"), Some("A"), None), 1usize),
            (
                candidate(2, 2, Some("Ghostty"), Some("B"), Some(false)),
                2usize,
            ),
        ];

        assert_eq!(select_frontmost_candidate(&candidates), None);
    }

    #[test]
    fn capture_plan_returns_target_window_without_fallback() {
        let result = execute_capture_plan(
            [
                CaptureStage::ExactTargetWindow,
                CaptureStage::SamePidWindow,
                CaptureStage::FrontmostWindow,
                CaptureStage::PrimaryDisplay,
            ],
            |stage| match stage {
                CaptureStage::ExactTargetWindow => Ok(Some("exact")),
                _ => panic!("later stages should not run"),
            },
        )
        .unwrap();

        assert_eq!(result, "exact");
    }

    #[test]
    fn capture_plan_falls_back_when_target_resolution_returns_none() {
        let result = execute_capture_plan(
            [CaptureStage::ExactTargetWindow, CaptureStage::SamePidWindow],
            |stage| match stage {
                CaptureStage::ExactTargetWindow => Ok(None),
                CaptureStage::SamePidWindow => Ok(Some("same-pid")),
                _ => panic!("unexpected fallback stage"),
            },
        )
        .unwrap();

        assert_eq!(result, "same-pid");
    }

    #[test]
    fn target_capture_plan_stops_when_target_cannot_be_resolved() {
        let result = execute_capture_plan(
            [CaptureStage::ExactTargetWindow, CaptureStage::SamePidWindow],
            |stage| match stage {
                CaptureStage::ExactTargetWindow => Ok(None::<&str>),
                CaptureStage::SamePidWindow => Ok(None::<&str>),
                _ => panic!("unexpected fallback stage"),
            },
        );

        assert!(matches!(result, Err(Error::NoCaptureSource)));
    }

    #[test]
    fn capture_plan_falls_back_to_display_when_frontmost_fails() {
        let result = execute_capture_plan(
            [CaptureStage::FrontmostWindow, CaptureStage::PrimaryDisplay],
            |stage| match stage {
                CaptureStage::FrontmostWindow => Ok(None),
                CaptureStage::PrimaryDisplay => Ok(Some("display")),
                _ => unreachable!(),
            },
        )
        .unwrap();

        assert_eq!(result, "display");
    }

    #[test]
    fn capture_plan_retries_after_capture_error() {
        let result = execute_capture_plan(
            [
                CaptureStage::ExactTargetWindow,
                CaptureStage::SamePidWindow,
                CaptureStage::FrontmostWindow,
            ],
            |stage| match stage {
                CaptureStage::ExactTargetWindow => Err(Error::InvalidWindowBounds),
                CaptureStage::SamePidWindow => Ok(None),
                CaptureStage::FrontmostWindow => Ok(Some("frontmost")),
                CaptureStage::PrimaryDisplay => unreachable!(),
            },
        )
        .unwrap();

        assert_eq!(result, "frontmost");
    }

    #[test]
    fn default_policy_uses_siglip_text_heavy_defaults() {
        let policy = WindowContextImagePolicy::default();

        assert_eq!(policy.max_long_side, 1920);
    }

    #[test]
    fn encode_png_uses_png_container() {
        let image = RgbaImage::from_raw(1, 1, vec![0, 0, 0, 255]).unwrap();
        let encoded = encode_png(&image).unwrap();

        assert_eq!(encoded.mime_type, "image/png");
        assert_eq!(&encoded.bytes[..8], b"\x89PNG\r\n\x1a\n");
    }
}
