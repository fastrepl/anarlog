use crate::WindowImpl;

const NOTE_WINDOW_WIDTH: f64 = 720.0;
const NOTE_WINDOW_HEIGHT: f64 = 820.0;
const NOTE_WINDOW_POSITION_TOLERANCE: f64 = 1.0;
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
            builder = builder.decorations(false);
        }

        #[cfg(target_os = "linux")]
        {
            builder = builder.decorations(false);
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
}
