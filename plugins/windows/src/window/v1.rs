use crate::WindowImpl;

const NOTE_WINDOW_WIDTH: f64 = 720.0;
const NOTE_WINDOW_HEIGHT: f64 = 820.0;
const NOTE_WINDOW_POSITION_TOLERANCE: f64 = 1.0;
const WINDOW_TITLEBAR_HEIGHT: f64 = 64.0;
const MIN_VISIBLE_TITLEBAR_WIDTH: f64 = 128.0;
const MIN_VISIBLE_TITLEBAR_HEIGHT: f64 = 32.0;
const NOTE_WINDOW_OFFSETS: [(f64, f64); 6] = [
    (0.0, 0.0),
    (144.0, 72.0),
    (-144.0, 72.0),
    (288.0, 144.0),
    (-288.0, 144.0),
    (0.0, 216.0),
];
const NOTE_WINDOW_OVERFLOW_OFFSET: f64 = 48.0;
static NOTE_WINDOW_POSITIONING_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq, Hash)]
#[serde(tag = "type", content = "value")]
pub enum AppWindow {
    #[serde(rename = "main")]
    Main,
    #[serde(rename = "composer")]
    Composer,
    #[serde(rename = "note")]
    Note(String),
}

impl std::fmt::Display for AppWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Main => write!(f, "main"),
            Self::Composer => write!(f, "composer"),
            Self::Note(id) => write!(f, "note-{id}"),
        }
    }
}

impl std::str::FromStr for AppWindow {
    type Err = strum::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "main" => return Ok(Self::Main),
            "composer" => return Ok(Self::Composer),
            _ => {}
        }

        if let Some(id) = s.strip_prefix("note-").filter(|id| !id.is_empty()) {
            return Ok(Self::Note(id.to_string()));
        }

        Err(strum::ParseError::VariantNotFound)
    }
}

impl AppWindow {
    pub fn center_on_primary(
        &self,
        app: &tauri::AppHandle<tauri::Wry>,
        window: &tauri::WebviewWindow<tauri::Wry>,
    ) -> Result<(), crate::Error> {
        if !matches!(self, Self::Main) {
            return Ok(());
        }

        use tauri::PhysicalPosition;

        let Some(monitor) = app.primary_monitor()? else {
            return Ok(());
        };
        let window_scale_factor = window.scale_factor()?;
        let window_size = window
            .outer_size()?
            .to_logical::<f64>(window_scale_factor)
            .to_physical::<u32>(monitor.scale_factor());
        let work_area = monitor.work_area();
        let window_frame = WindowFrame::new(
            0.0,
            0.0,
            f64::from(window_size.width),
            f64::from(window_size.height),
        );
        let work_area = WindowFrame::from_physical(work_area.position, work_area.size);
        let (x, y) = centered_window_position(window_frame, work_area);

        window.set_position(PhysicalPosition::new(x, y))?;
        tracing::info!("main_window_centered_on_primary");

        Ok(())
    }

    pub(crate) fn ensure_visible(
        &self,
        app: &tauri::AppHandle<tauri::Wry>,
        window: &tauri::WebviewWindow<tauri::Wry>,
    ) {
        if !matches!(self, Self::Main) {
            return;
        }

        use tauri::PhysicalPosition;

        let (Ok(position), Ok(size), Ok(scale_factor), Ok(monitors)) = (
            window.outer_position(),
            window.outer_size(),
            window.scale_factor(),
            app.available_monitors(),
        ) else {
            return;
        };
        let window_frame = WindowFrame::new(
            f64::from(position.x),
            f64::from(position.y),
            f64::from(size.width),
            f64::from(size.height),
        );
        let work_areas = monitors
            .iter()
            .map(|monitor| {
                let work_area = monitor.work_area();
                WindowFrame::from_physical(work_area.position, work_area.size)
            })
            .collect::<Vec<_>>();

        if has_accessible_titlebar(window_frame, &work_areas, scale_factor) {
            return;
        }

        let primary_work_area = app.primary_monitor().ok().flatten().map(|monitor| {
            let work_area = monitor.work_area();
            WindowFrame::from_physical(work_area.position, work_area.size)
        });
        let Some(work_area) = recenter_work_area(window_frame, &work_areas, primary_work_area)
        else {
            return;
        };
        let target_scale_factor = monitors
            .iter()
            .find(|monitor| {
                let monitor_work_area = monitor.work_area();
                WindowFrame::from_physical(monitor_work_area.position, monitor_work_area.size)
                    == work_area
            })
            .map(tauri::Monitor::scale_factor)
            .unwrap_or(scale_factor);
        let target_window_frame =
            window_frame_at_scale(window_frame, scale_factor, target_scale_factor);
        let (x, y) = centered_window_position(target_window_frame, work_area);

        match window.set_position(PhysicalPosition::new(x, y)) {
            Ok(()) => tracing::info!("window_recentered"),
            Err(error) => tracing::warn!(%error, "window_recenter_failed"),
        }
    }

    fn window_builder<'a>(
        &'a self,
        app: &'a tauri::AppHandle<tauri::Wry>,
        url: impl Into<std::path::PathBuf>,
    ) -> tauri::WebviewWindowBuilder<'a, tauri::Wry, tauri::AppHandle<tauri::Wry>> {
        use tauri::{WebviewUrl, WebviewWindow};

        let title = app
            .config()
            .product_name
            .clone()
            .unwrap_or_else(|| self.title());

        #[allow(unused_mut)]
        let mut builder = WebviewWindow::builder(app, self.label(), WebviewUrl::App(url.into()))
            .title(title)
            .disable_drag_drop_handler();

        #[cfg(target_os = "macos")]
        {
            if matches!(self, Self::Main) {
                builder = builder.background_throttling(
                    tauri::utils::config::BackgroundThrottlingPolicy::Disabled,
                );
            }

            let traffic_light_y = {
                use tauri_plugin_os::{Version, version};
                let major = match version() {
                    Version::Semantic(major, _, _) => major,
                    Version::Custom(s) => s
                        .split('.')
                        .next()
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(0),
                    _ => 0,
                };

                if major >= 26 { 25.0 } else { 19.0 }
            };

            builder = builder
                .visible(false)
                .decorations(true)
                .hidden_title(true)
                .theme(Some(tauri::Theme::Light))
                .traffic_light_position(tauri::LogicalPosition::new(12.0, traffic_light_y))
                .title_bar_style(tauri::TitleBarStyle::Overlay);
        }

        #[cfg(target_os = "windows")]
        {
            builder = builder.decorations(!matches!(self, Self::Main));
        }

        #[cfg(target_os = "linux")]
        {
            builder = builder.decorations(true);
        }

        builder
    }
}

impl WindowImpl for AppWindow {
    fn title(&self) -> String {
        match self {
            Self::Main => "Anarlog".into(),
            Self::Composer => "Composer".into(),
            Self::Note(_) => "Note".into(),
        }
    }

    fn build_window(
        &self,
        app: &tauri::AppHandle<tauri::Wry>,
    ) -> Result<tauri::WebviewWindow, crate::Error> {
        use tauri::LogicalSize;

        let window = match self {
            Self::Main => {
                let builder = self
                    .window_builder(app, "/app")
                    .maximizable(true)
                    .minimizable(true)
                    .min_inner_size(500.0, 500.0);
                let window = builder.build()?;
                window.set_size(LogicalSize::new(910.0, 600.0))?;
                window
            }
            Self::Composer => {
                let builder = self
                    .window_builder(app, "/app/composer")
                    .maximizable(false)
                    .minimizable(false)
                    .resizable(false);
                let window = builder.build()?;
                window.set_size(LogicalSize::new(
                    crate::window::composer::WIDTH,
                    crate::window::composer::HEIGHT,
                ))?;
                window
            }
            Self::Note(id) => {
                let encoded_id: String =
                    url::form_urlencoded::byte_serialize(id.as_bytes()).collect();
                let builder = self
                    .window_builder(app, format!("/app/note/{encoded_id}"))
                    .maximizable(true)
                    .minimizable(true)
                    .min_inner_size(420.0, 500.0);
                let window = builder.build()?;
                window.set_size(LogicalSize::new(NOTE_WINDOW_WIDTH, NOTE_WINDOW_HEIGHT))?;
                window
            }
        };

        #[cfg(target_os = "windows")]
        window.set_decorations(!matches!(self, Self::Main))?;

        #[cfg(target_os = "linux")]
        window.set_decorations(true)?;

        Ok(window)
    }

    fn position_new_window(
        &self,
        app: &tauri::AppHandle<tauri::Wry>,
        window: &tauri::WebviewWindow<tauri::Wry>,
    ) -> Result<(), crate::Error> {
        let Self::Note(_) = self else {
            return Ok(());
        };

        use tauri::{Manager, Position};

        let _positioning_guard = NOTE_WINDOW_POSITIONING_LOCK
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let monitor = window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| app.primary_monitor().ok().flatten());
        let Some(monitor) = monitor else {
            return Ok(());
        };

        let monitor_scale_factor = monitor.scale_factor();
        let window_scale_factor = window.scale_factor()?;
        let monitor_position = monitor.position().to_logical::<f64>(monitor_scale_factor);
        let monitor_size = monitor.size().to_logical::<f64>(monitor_scale_factor);
        let window_size = window.outer_size()?.to_logical::<f64>(window_scale_factor);
        let target_label = window.label().to_string();
        let note_windows = app
            .webview_windows()
            .into_iter()
            .filter(|(label, _)| matches!(label.parse::<Self>(), Ok(Self::Note(_))))
            .collect::<Vec<_>>();
        let mut note_labels = note_windows
            .iter()
            .map(|(label, _)| label.clone())
            .collect::<Vec<_>>();
        if !note_labels.iter().any(|label| label == &target_label) {
            note_labels.push(target_label.clone());
        }
        note_labels.sort_unstable();
        let slot_index = note_labels
            .iter()
            .position(|label| label == &target_label)
            .unwrap_or(note_labels.len());
        let open_note_positions = note_windows
            .into_iter()
            .filter(|(label, _)| label != &target_label)
            .filter_map(|(_, note_window)| {
                let position = note_window.outer_position().ok()?;
                let note_scale_factor = note_window.scale_factor().unwrap_or(window_scale_factor);
                let position = position.to_logical::<f64>(note_scale_factor);

                Some((position.x, position.y))
            })
            .collect::<Vec<_>>();
        let position = staggered_note_window_position(
            monitor_position.x,
            monitor_position.y,
            monitor_size.width,
            monitor_size.height,
            window_size.width,
            window_size.height,
            &open_note_positions,
            slot_index,
        );

        window.set_position(Position::Logical(tauri::LogicalPosition::new(
            position.0, position.1,
        )))?;

        Ok(())
    }
}

fn staggered_note_window_position(
    monitor_x: f64,
    monitor_y: f64,
    monitor_width: f64,
    monitor_height: f64,
    window_width: f64,
    window_height: f64,
    occupied_positions: &[(f64, f64)],
    slot_index: usize,
) -> (f64, f64) {
    let base_x = monitor_x + ((monitor_width - window_width) / 2.0);
    let base_y = monitor_y + ((monitor_height - window_height) / 2.0);
    let candidate_for_offset = |offset: (f64, f64)| {
        (
            clamp_to_monitor(base_x + offset.0, monitor_x, monitor_width, window_width),
            clamp_to_monitor(base_y + offset.1, monitor_y, monitor_height, window_height),
        )
    };
    let candidate_for_slot = |index: usize| {
        let offset = NOTE_WINDOW_OFFSETS[index % NOTE_WINDOW_OFFSETS.len()];
        let overflow_offset =
            (index / NOTE_WINDOW_OFFSETS.len()) as f64 * NOTE_WINDOW_OVERFLOW_OFFSET;
        candidate_for_offset((offset.0 + overflow_offset, offset.1 + overflow_offset))
    };
    let fallback = candidate_for_slot(slot_index);

    if occupied_positions.is_empty() {
        return fallback;
    }

    (0..(NOTE_WINDOW_OFFSETS.len() + occupied_positions.len() + 1))
        .map(|index| candidate_for_slot(slot_index + index))
        .find(|position| {
            !occupied_positions
                .iter()
                .any(|occupied| same_window_position(*position, *occupied))
        })
        .unwrap_or(fallback)
}

fn clamp_to_monitor(
    value: f64,
    monitor_start: f64,
    monitor_length: f64,
    window_length: f64,
) -> f64 {
    let max_start = monitor_start + (monitor_length - window_length).max(0.0);
    value.clamp(monitor_start, max_start)
}

fn same_window_position(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() <= NOTE_WINDOW_POSITION_TOLERANCE
        && (a.1 - b.1).abs() <= NOTE_WINDOW_POSITION_TOLERANCE
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct WindowFrame {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl WindowFrame {
    fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn from_physical(
        position: tauri::PhysicalPosition<i32>,
        size: tauri::PhysicalSize<u32>,
    ) -> Self {
        Self::new(
            f64::from(position.x),
            f64::from(position.y),
            f64::from(size.width),
            f64::from(size.height),
        )
    }

    fn intersection(self, other: Self) -> Option<Self> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);

        (right > left && bottom > top).then(|| Self::new(left, top, right - left, bottom - top))
    }

    fn area(self) -> f64 {
        self.width.max(0.0) * self.height.max(0.0)
    }
}

fn has_accessible_titlebar(
    window: WindowFrame,
    work_areas: &[WindowFrame],
    scale_factor: f64,
) -> bool {
    let titlebar = WindowFrame::new(
        window.x,
        window.y,
        window.width,
        window.height.min(WINDOW_TITLEBAR_HEIGHT * scale_factor),
    );
    let required_width = titlebar
        .width
        .min(MIN_VISIBLE_TITLEBAR_WIDTH * scale_factor);
    let required_height = titlebar
        .height
        .min(MIN_VISIBLE_TITLEBAR_HEIGHT * scale_factor);

    work_areas.iter().any(|work_area| {
        titlebar.intersection(*work_area).is_some_and(|visible| {
            visible.width >= required_width && visible.height >= required_height
        })
    })
}

fn recenter_work_area(
    window: WindowFrame,
    work_areas: &[WindowFrame],
    primary_work_area: Option<WindowFrame>,
) -> Option<WindowFrame> {
    work_areas
        .iter()
        .copied()
        .filter_map(|work_area| {
            window
                .intersection(work_area)
                .map(|overlap| (work_area, overlap.area()))
        })
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(work_area, _)| work_area)
        .or(primary_work_area)
        .or_else(|| work_areas.first().copied())
}

fn centered_window_position(window: WindowFrame, work_area: WindowFrame) -> (i32, i32) {
    let x = work_area.x + ((work_area.width - window.width).max(0.0) / 2.0);
    let y = work_area.y + ((work_area.height - window.height).max(0.0) / 2.0);

    (x.round() as i32, y.round() as i32)
}

fn window_frame_at_scale(
    window: WindowFrame,
    current_scale_factor: f64,
    target_scale_factor: f64,
) -> WindowFrame {
    WindowFrame::new(
        window.x,
        window.y,
        window.width / current_scale_factor * target_scale_factor,
        window.height / current_scale_factor * target_scale_factor,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staggers_note_windows_around_the_center() {
        assert_eq!(
            staggered_note_window_position(0.0, 0.0, 1600.0, 1100.0, 720.0, 820.0, &[], 0),
            (440.0, 140.0)
        );
        assert_eq!(
            staggered_note_window_position(0.0, 0.0, 1600.0, 1100.0, 720.0, 820.0, &[], 1),
            (584.0, 212.0)
        );
        assert_eq!(
            staggered_note_window_position(0.0, 0.0, 1600.0, 1100.0, 720.0, 820.0, &[], 2),
            (296.0, 212.0)
        );
    }

    #[test]
    fn keeps_staggered_note_windows_inside_the_monitor() {
        assert_eq!(
            staggered_note_window_position(0.0, 0.0, 900.0, 840.0, 720.0, 820.0, &[], 3),
            (180.0, 20.0)
        );
        assert_eq!(
            staggered_note_window_position(0.0, 0.0, 640.0, 480.0, 720.0, 820.0, &[], 1),
            (0.0, 0.0)
        );
    }

    #[test]
    fn skips_occupied_stagger_slots() {
        assert_eq!(
            staggered_note_window_position(
                0.0,
                0.0,
                1600.0,
                1100.0,
                720.0,
                820.0,
                &[(584.0, 212.0)],
                1,
            ),
            (296.0, 212.0)
        );
        assert_eq!(
            staggered_note_window_position(
                0.0,
                0.0,
                1600.0,
                1100.0,
                720.0,
                820.0,
                &[(440.0, 140.0)],
                1,
            ),
            (584.0, 212.0)
        );
    }

    #[test]
    fn cascades_when_stagger_slots_are_full() {
        assert_eq!(
            staggered_note_window_position(
                0.0,
                0.0,
                1600.0,
                1100.0,
                720.0,
                820.0,
                &[
                    (440.0, 140.0),
                    (584.0, 212.0),
                    (296.0, 212.0),
                    (728.0, 280.0),
                    (152.0, 280.0),
                    (440.0, 356.0),
                ],
                6,
            ),
            (488.0, 188.0)
        );
    }

    #[test]
    fn keeps_windows_with_an_accessible_titlebar_in_place() {
        let monitor = WindowFrame::new(0.0, 0.0, 1920.0, 1050.0);

        assert!(has_accessible_titlebar(
            WindowFrame::new(200.0, 100.0, 900.0, 600.0),
            &[monitor],
            1.0,
        ));
        assert!(has_accessible_titlebar(
            WindowFrame::new(1792.0, 100.0, 900.0, 600.0),
            &[monitor],
            1.0,
        ));
    }

    #[test]
    fn recenters_windows_without_an_accessible_titlebar() {
        let monitor = WindowFrame::new(0.0, 0.0, 1920.0, 1050.0);

        assert!(!has_accessible_titlebar(
            WindowFrame::new(1800.0, 100.0, 900.0, 600.0),
            &[monitor],
            1.0,
        ));
        assert!(!has_accessible_titlebar(
            WindowFrame::new(200.0, -50.0, 900.0, 600.0),
            &[monitor],
            1.0,
        ));
        assert!(!has_accessible_titlebar(
            WindowFrame::new(2200.0, 100.0, 900.0, 600.0),
            &[monitor],
            1.0,
        ));
    }

    #[test]
    fn applies_the_window_scale_factor_to_titlebar_visibility() {
        let monitor = WindowFrame::new(0.0, 0.0, 3840.0, 2100.0);
        let window = WindowFrame::new(3600.0, 200.0, 1800.0, 1200.0);

        assert!(has_accessible_titlebar(window, &[monitor], 1.0));
        assert!(!has_accessible_titlebar(window, &[monitor], 2.0));
    }

    #[test]
    fn recenters_on_the_display_with_the_largest_overlap() {
        let primary = WindowFrame::new(0.0, 0.0, 1920.0, 1050.0);
        let secondary = WindowFrame::new(1920.0, 50.0, 1496.0, 939.0);
        let window = WindowFrame::new(3300.0, 300.0, 900.0, 600.0);

        assert_eq!(
            recenter_work_area(window, &[primary, secondary], Some(primary)),
            Some(secondary)
        );
        assert_eq!(centered_window_position(window, secondary), (2218, 220));
    }

    #[test]
    fn recenters_fully_offscreen_windows_on_the_primary_display() {
        let primary = WindowFrame::new(-1920.0, 0.0, 1920.0, 1050.0);
        let secondary = WindowFrame::new(0.0, 0.0, 1496.0, 939.0);
        let window = WindowFrame::new(4000.0, 300.0, 900.0, 600.0);

        assert_eq!(
            recenter_work_area(window, &[primary, secondary], Some(primary)),
            Some(primary)
        );
        assert_eq!(centered_window_position(window, primary), (-1410, 225));
    }

    #[test]
    fn keeps_the_titlebar_visible_when_the_window_is_larger_than_the_display() {
        let monitor = WindowFrame::new(1920.0, 50.0, 1496.0, 939.0);
        let window = WindowFrame::new(4000.0, 300.0, 1800.0, 1200.0);

        assert_eq!(centered_window_position(window, monitor), (1920, 50));
    }

    #[test]
    fn recenters_using_the_target_display_scale() {
        let window = WindowFrame::new(4000.0, 300.0, 1800.0, 1200.0);
        let target = WindowFrame::new(0.0, 0.0, 1920.0, 1050.0);
        let scaled_window = window_frame_at_scale(window, 2.0, 1.0);

        assert_eq!(scaled_window.width, 900.0);
        assert_eq!(scaled_window.height, 600.0);
        assert_eq!(centered_window_position(scaled_window, target), (510, 225));
    }
}
