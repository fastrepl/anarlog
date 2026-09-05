use std::path::PathBuf;

use gpui::{Rgba, TextSystem, rgb};

/// Neutral light palette matching the Tauri app's default appearance. Dark
/// mode and user appearance settings land once settings move off the webview.
#[derive(Clone, Copy)]
pub struct Theme {
    pub background: Rgba,
    pub sidebar: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub selected: Rgba,
    pub hover: Rgba,
    pub danger: Rgba,
    pub link: Rgba,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            background: rgb(0xffffff),
            sidebar: rgb(0xf5f5f5),
            border: rgb(0xe5e5e5),
            text: rgb(0x171717),
            text_muted: rgb(0x737373),
            selected: rgb(0xe5e5e5),
            hover: rgb(0xebebeb),
            danger: rgb(0xb91c1c),
            link: rgb(0x2563eb),
        }
    }
}

/// The family the Tauri app's `system-ui` CSS resolves to on this machine.
/// GPUI's built-in fallbacks are requested at normal weight, so an explicit
/// installed family is also what makes bold and semibold runs render.
pub fn ui_font_family(text_system: &TextSystem) -> Option<String> {
    if cfg!(target_os = "macos") {
        return Some(".SystemUIFont".to_string());
    }
    let installed = text_system.all_font_names();
    let is_installed = |family: &str| installed.iter().any(|name| name == family);

    if cfg!(target_os = "windows") {
        return is_installed("Segoe UI").then(|| "Segoe UI".to_string());
    }

    gtk_font_family()
        .filter(|family| is_installed(family))
        .or_else(|| {
            [
                "Cantarell",
                "Inter",
                "Ubuntu",
                "Noto Sans",
                "DejaVu Sans",
                "Liberation Sans",
            ]
            .into_iter()
            .find(|family| is_installed(family))
            .map(str::to_string)
        })
}

/// WebKitGTK resolves `system-ui` through GTK's `gtk-font-name` setting.
fn gtk_font_family() -> Option<String> {
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(dirs::config_dir)?;
    ["gtk-4.0", "gtk-3.0"].iter().find_map(|version| {
        let ini = std::fs::read_to_string(config_dir.join(version).join("settings.ini")).ok()?;
        parse_gtk_font_name(&ini)
    })
}

/// `gtk-font-name=Inter 11` -> `Inter`; the trailing token is the point size.
fn parse_gtk_font_name(ini: &str) -> Option<String> {
    let value = ini.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "gtk-font-name").then(|| value.trim())
    })?;
    let family = match value.rsplit_once(' ') {
        Some((family, size)) if size.parse::<f32>().is_ok() => family,
        _ => value,
    };
    let family = family.trim();
    (!family.is_empty()).then(|| family.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtk_font_name_drops_the_size_and_keeps_multi_word_families() {
        assert_eq!(
            parse_gtk_font_name("[Settings]\ngtk-theme-name=X\ngtk-font-name=Inter 11\n"),
            Some("Inter".to_string())
        );
        assert_eq!(
            parse_gtk_font_name("gtk-font-name = Noto Sans 10.5"),
            Some("Noto Sans".to_string())
        );
        assert_eq!(
            parse_gtk_font_name("gtk-font-name=Cantarell"),
            Some("Cantarell".to_string())
        );
        assert_eq!(parse_gtk_font_name("gtk-theme-name=X"), None);
    }
}
