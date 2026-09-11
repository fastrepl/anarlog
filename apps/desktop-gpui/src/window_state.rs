//! `tauri-plugin-window-state`'s `.window-state.json` in the app config
//! directory: the Tauri app restores its `main` window's size, position and
//! maximized flag from it (`persisted_window_state_flags`) and saves them on
//! exit, so the shell reads and writes the same entry and both shells open
//! on the frame the other left.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

/// The window label the Tauri app saves the main window under.
pub const MAIN_LABEL: &str = "main";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Logical inner size.
    pub width: f64,
    pub height: f64,
    /// Logical outer position.
    pub x: f64,
    pub y: f64,
    pub maximized: bool,
}

/// `<app_config_dir>/.window-state.json`: Tauri's `app_config_dir` is the
/// platform config directory joined with the bundle identifier.
pub fn path(identifier: &str) -> Option<PathBuf> {
    Some(
        dirs::config_dir()?
            .join(identifier)
            .join(".window-state.json"),
    )
}

fn read(path: &Path) -> Map<String, Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| match value {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default()
}

/// The saved frame of `label`, when the file has one with a usable size.
pub fn load(path: &Path, label: &str) -> Option<Frame> {
    let state = read(path);
    let entry = state.get(label)?.as_object()?;
    let number = |key: &str| entry.get(key).and_then(Value::as_f64);
    let width = number("width")?;
    let height = number("height")?;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some(Frame {
        width,
        height,
        x: number("x").unwrap_or(0.0),
        y: number("y").unwrap_or(0.0),
        maximized: entry
            .get("maximized")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Writes `label`'s frame the way the plugin does (size, position with the
/// `prev_*` copies, `maximized`), keeping the other windows' entries and the
/// flags the shell does not track.
pub fn save(path: &Path, label: &str, frame: Frame) -> std::io::Result<()> {
    let mut state = read(path);
    let mut entry = state
        .get(label)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let round = |value: f64| Value::from(value.round() as i64);
    entry.insert("width".into(), round(frame.width));
    entry.insert("height".into(), round(frame.height));
    entry.insert("x".into(), round(frame.x));
    entry.insert("y".into(), round(frame.y));
    entry.insert("prev_x".into(), round(frame.x));
    entry.insert("prev_y".into(), round(frame.y));
    entry.insert("maximized".into(), json!(frame.maximized));
    entry.entry("visible").or_insert(json!(true));
    entry.entry("decorated").or_insert(json!(true));
    entry.entry("fullscreen").or_insert(json!(false));
    state.insert(label.to_string(), Value::Object(entry));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&Value::Object(state))?;
    std::fs::write(path, text)
}

/// `save_window_state(persisted_window_state_flags())` for the main window:
/// its size, position and maximized flag under `main`.
pub fn save_main(identifier: &str, bounds: gpui::WindowBounds) {
    let Some(path) = path(identifier) else {
        return;
    };
    let (rect, maximized) = match bounds {
        gpui::WindowBounds::Windowed(rect) => (rect, false),
        gpui::WindowBounds::Maximized(rect) => (rect, true),
        // Fullscreen is not persisted (`StateFlags::FULLSCREEN` is off).
        gpui::WindowBounds::Fullscreen(rect) => (rect, false),
    };
    let frame = Frame {
        width: f64::from(f32::from(rect.size.width)),
        height: f64::from(f32::from(rect.size.height)),
        x: f64::from(f32::from(rect.origin.x)),
        y: f64::from(f32::from(rect.origin.y)),
        maximized,
    };
    if let Err(error) = save(&path, MAIN_LABEL, frame) {
        tracing::warn!(%error, "failed to save the window state");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_plugin_file_and_writes_it_back_in_shape() {
        let dir = std::env::temp_dir().join(format!("anlg-window-state-{}", uuid::Uuid::new_v4()));
        let file = dir.join(".window-state.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &file,
            r#"{
  "floating-bar": { "width": 111, "height": 67, "x": 820, "y": 40, "prev_x": 820, "prev_y": 40, "maximized": false, "visible": true, "decorated": true, "fullscreen": false },
  "main": { "width": 1100, "height": 696, "x": 0, "y": 40, "prev_x": 0, "prev_y": 40, "maximized": false, "visible": true, "decorated": false, "fullscreen": false }
}"#,
        )
        .unwrap();

        assert_eq!(
            load(&file, MAIN_LABEL),
            Some(Frame {
                width: 1100.0,
                height: 696.0,
                x: 0.0,
                y: 40.0,
                maximized: false,
            })
        );
        assert_eq!(load(&file, "note"), None);

        save(
            &file,
            MAIN_LABEL,
            Frame {
                width: 910.4,
                height: 600.0,
                x: 12.0,
                y: 34.0,
                maximized: true,
            },
        )
        .unwrap();
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(written["main"]["width"], json!(910));
        assert_eq!(written["main"]["prev_x"], json!(12));
        assert_eq!(written["main"]["maximized"], json!(true));
        // The plugin's own flags and the other window survive.
        assert_eq!(written["main"]["decorated"], json!(false));
        assert_eq!(written["floating-bar"]["width"], json!(111));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_file_yields_nothing_and_saving_creates_it() {
        let dir = std::env::temp_dir().join(format!("anlg-window-state-{}", uuid::Uuid::new_v4()));
        let file = dir.join(".window-state.json");
        assert_eq!(load(&file, MAIN_LABEL), None);
        save(
            &file,
            MAIN_LABEL,
            Frame {
                width: 500.0,
                height: 500.0,
                x: 0.0,
                y: 0.0,
                maximized: false,
            },
        )
        .unwrap();
        assert_eq!(
            load(&file, MAIN_LABEL).map(|frame| frame.width),
            Some(500.0)
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
