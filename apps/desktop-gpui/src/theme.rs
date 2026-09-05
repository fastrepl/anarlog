use std::path::PathBuf;

use gpui::{Rgba, TextSystem, rgb};

/// Light tokens from `packages/design-system/src/tokens.css`, converted from
/// their HSL channels. Dark mode lands with the appearance settings.
#[derive(Clone, Copy)]
pub struct Theme {
    pub background: Rgba,
    pub foreground: Rgba,
    pub card: Rgba,
    pub muted_foreground: Rgba,
    pub accent: Rgba,
    pub border: Rgba,
    pub destructive: Rgba,
    /// Tailwind `red-500`, used by the current-time line and recording dot.
    pub red: Rgba,
    /// Tailwind `neutral-700`, the breadcrumb title colour.
    pub title: Rgba,
    /// Windows-style close button hover.
    pub close_hover: Rgba,
    /// `--color-blue-600` from `note-typography.css`.
    pub link: Rgba,
    pub white: Rgba,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            background: rgb(0xfafaf9),
            foreground: rgb(0x1c1917),
            card: rgb(0xffffff),
            muted_foreground: rgb(0x78726d),
            accent: rgb(0xf0f0ef),
            border: rgb(0xe7e5e4),
            destructive: rgb(0xef4444),
            red: rgb(0xef4444),
            title: rgb(0x404040),
            close_hover: rgb(0xc42b1c),
            link: rgb(0x2563eb),
            white: rgb(0xffffff),
        }
    }
}

/// Tailwind `color/NN` opacity modifier.
pub fn alpha(color: Rgba, alpha: f32) -> Rgba {
    Rgba { a: alpha, ..color }
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

/// Tailwind's `font-mono` stack, then what fontconfig's `monospace` maps to
/// on common Linux installs.
pub fn mono_font_family(text_system: &TextSystem) -> Option<String> {
    let installed = text_system.all_font_names();
    [
        "SF Mono",
        "SFMono-Regular",
        "Menlo",
        "Monaco",
        "Consolas",
        "Liberation Mono",
        "Courier New",
        "DejaVu Sans Mono",
        "Noto Sans Mono",
        "Cascadia Code",
        "Cousine",
    ]
    .into_iter()
    .find(|family| installed.iter().any(|name| name == family))
    .map(str::to_string)
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

    #[test]
    fn alpha_only_touches_the_alpha_channel() {
        let faded = alpha(rgb(0xef4444), 0.85);
        assert_eq!(faded.a, 0.85);
        assert_eq!(
            (faded.r, faded.g, faded.b),
            (rgb(0xef4444).r, rgb(0xef4444).g, rgb(0xef4444).b)
        );
    }
}
