use crate::WindowImpl;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq, Hash)]
#[serde(tag = "type", content = "value")]
pub enum AppWindow {
    #[serde(rename = "main")]
    Main,
    #[serde(rename = "control")]
    Control,
}

impl std::fmt::Display for AppWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Main => write!(f, "main"),
            Self::Control => write!(f, "control"),
        }
    }
}

impl std::str::FromStr for AppWindow {
    type Err = strum::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "main" => return Ok(Self::Main),
            "control" => return Ok(Self::Control),
            _ => {}
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

        let title = match self {
            Self::Main => app
                .config()
                .product_name
                .clone()
                .unwrap_or_else(|| self.title()),
            _ => self.title(),
        };

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

                if major >= 26 && cfg!(debug_assertions) {
                    24.0
                } else {
                    18.0
                }
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
            Self::Main => "Char".into(),
            Self::Control => "Control".into(),
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
                    .window_builder(app, "/app/main")
                    .maximizable(true)
                    .minimizable(true)
                    .min_inner_size(620.0, 500.0);
                let window = builder.build()?;
                window.set_size(LogicalSize::new(910.0, 600.0))?;
                window
            }
            Self::Control => {
                let window = self
                    .window_builder(app, "/app/control")
                    .transparent(true)
                    .resizable(false)
                    .always_on_top(true)
                    .skip_taskbar(true)
                    .accept_first_mouse(true)
                    .visible_on_all_workspaces(true)
                    .decorations(false)
                    .build()?;

                let collapsed_size = LogicalSize::new(80.0, 80.0);
                window.set_size(LogicalSize::new(1.0, 1.0))?;
                std::thread::sleep(std::time::Duration::from_millis(10));
                window.set_size(collapsed_size)?;
                window
            }
        };

        Ok(window)
    }
}
