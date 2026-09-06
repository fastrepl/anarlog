use std::path::PathBuf;

use gpui::{Rgba, TextSystem, rgb};

/// Light and dark tokens from `packages/design-system/src/tokens.css`,
/// converted from their HSL channels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub background: Rgba,
    pub foreground: Rgba,
    pub card: Rgba,
    pub popover: Rgba,
    pub muted: Rgba,
    pub muted_foreground: Rgba,
    pub accent: Rgba,
    pub border: Rgba,
    pub primary: Rgba,
    pub primary_foreground: Rgba,
    pub destructive: Rgba,
    /// Tailwind `red-500`, used by the current-time line and recording dot.
    pub red: Rgba,
    /// Tailwind `neutral-700` (`dark:text-white`), the breadcrumb title colour.
    pub title: Rgba,
    /// Windows-style close button hover.
    pub close_hover: Rgba,
    /// `--color-blue-600` from `note-typography.css`.
    pub link: Rgba,
    pub white: Rgba,
    /// Text selection highlight (WebKitGTK's default).
    pub selection: Rgba,
    /// `::-webkit-scrollbar-thumb` (`scrollbar.css` / `dark-theme.css`).
    pub scrollbar_thumb: Rgba,
    /// `--app-floating-*` tokens for `variant="app"` menus.
    pub floating_chrome: Rgba,
    pub floating_panel: Rgba,
    pub floating_border: Rgba,
    /// sonner's `--normal-bg/--normal-border/--normal-text` for the theme.
    pub toast_background: Rgba,
    pub toast_border: Rgba,
    pub toast_text: Rgba,
    /// Delete menu item: `text-red-600 hover:bg-red-50 hover:text-red-700`
    /// (`dark:text-red-400 dark:hover:bg-red-950/50 dark:hover:text-red-300`).
    pub delete_text: Rgba,
    pub delete_hover_background: Rgba,
    pub delete_hover_text: Rgba,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            dark: false,
            background: rgb(0xfafaf9),
            foreground: rgb(0x1c1917),
            card: rgb(0xffffff),
            popover: rgb(0xffffff),
            muted: rgb(0xf5f5f4),
            muted_foreground: rgb(0x78726d),
            accent: rgb(0xf0f0ef),
            border: rgb(0xe7e5e4),
            primary: rgb(0x2d2825),
            primary_foreground: rgb(0xfafaf9),
            destructive: rgb(0xef4444),
            red: rgb(0xef4444),
            title: rgb(0x404040),
            close_hover: rgb(0xc42b1c),
            link: rgb(0x2563eb),
            white: rgb(0xffffff),
            selection: alpha(rgb(0x3584e4), 0.35),
            scrollbar_thumb: rgb(0xe5e5e5),
            floating_chrome: rgb(0xf0f0ef),
            floating_panel: rgb(0xfafaf9),
            floating_border: rgb(0xdddbd9),
            toast_background: rgb(0xffffff),
            toast_border: rgb(0xededed),
            toast_text: rgb(0x171717),
            delete_text: rgb(0xdc2626),
            delete_hover_background: rgb(0xfef2f2),
            delete_hover_text: rgb(0xb91c1c),
        }
    }

    /// The `.dark` block of `tokens.css`, plus sonner's dark theme.
    pub fn dark() -> Self {
        Self {
            dark: true,
            background: rgb(0x1f1b19),
            foreground: rgb(0xfafaf9),
            card: rgb(0x2d2825),
            popover: rgb(0x2d2825),
            muted: rgb(0x36322f),
            muted_foreground: rgb(0x928b87),
            accent: rgb(0x443f3c),
            border: rgb(0x4c4743),
            primary: rgb(0x2d2825),
            primary_foreground: rgb(0xfafaf9),
            destructive: rgb(0x7f1d1d),
            red: rgb(0xef4444),
            title: rgb(0xffffff),
            close_hover: rgb(0xc42b1c),
            link: rgb(0x2563eb),
            white: rgb(0xffffff),
            selection: alpha(rgb(0x3584e4), 0.45),
            scrollbar_thumb: rgb(0x44403c),
            floating_chrome: rgb(0x393532),
            floating_panel: rgb(0x272321),
            floating_border: rgb(0x59534f),
            toast_background: rgb(0x000000),
            toast_border: rgb(0x333333),
            toast_text: rgb(0xfcfcfc),
            delete_text: rgb(0xf87171),
            delete_hover_background: alpha(rgb(0x450a0a), 0.5),
            delete_hover_text: rgb(0xfca5a5),
        }
    }

    /// `resolveIsDarkMode`: the `theme` setting (`light` / `dark` / `system`)
    /// against the window's appearance.
    pub fn resolve(preference: &str, appearance: gpui::WindowAppearance) -> Self {
        let system_dark = matches!(
            appearance,
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
        );
        let dark = match preference {
            "dark" => true,
            "light" => false,
            _ => system_dark,
        };
        if dark { Self::dark() } else { Self::light() }
    }
}

/// Tailwind `color/NN` opacity modifier.
pub fn alpha(color: Rgba, alpha: f32) -> Rgba {
    Rgba { a: alpha, ..color }
}

/// Source-over compositing of `top` onto an opaque `bottom`.
pub fn over(top: Rgba, bottom: Rgba) -> Rgba {
    let a = top.a;
    Rgba {
        r: top.r * a + bottom.r * (1.0 - a),
        g: top.g * a + bottom.g * (1.0 - a),
        b: top.b * a + bottom.b * (1.0 - a),
        a: 1.0,
    }
}

/// `GlassDialogContent`: `bg-card/60 backdrop-blur-2xl` over the `bg-black/40`
/// overlay. GPUI cannot blur what is behind an element, but the blur of a
/// dimmed, mostly `bg-background` page converges on this flat colour (light
/// mode: `#d6d6d6`, as measured on the running Tauri app).
pub fn glass_card_fill(theme: Theme) -> Rgba {
    let dimmed_page = over(alpha(gpui::rgb(0x000000), 0.4), theme.background);
    over(alpha(theme.card, 0.6), dimmed_page)
}

/// `--font-sans` in `styles/globals.css`.
const SANS_STACK: [&str; 5] = [
    "system-ui",
    "-apple-system",
    "BlinkMacSystemFont",
    "Segoe UI",
    "sans-serif",
];

/// Tailwind's `font-mono` stack.
const MONO_STACK: [&str; 8] = [
    "ui-monospace",
    "SFMono-Regular",
    "Menlo",
    "Monaco",
    "Consolas",
    "Liberation Mono",
    "Courier New",
    "monospace",
];

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

    fontconfig::resolve_stack(&SANS_STACK, &is_installed)
        .or_else(|| gtk_font_family().filter(|family| is_installed(family)))
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

/// Tailwind's `font-mono` stack, resolved the way WebKit does on each
/// platform: fontconfig on Linux, the first installed family elsewhere.
pub fn mono_font_family(text_system: &TextSystem) -> Option<String> {
    let installed = text_system.all_font_names();
    let is_installed = |family: &str| installed.iter().any(|name| name == family);
    if let Some(family) = fontconfig::resolve_stack(&MONO_STACK, &is_installed) {
        return Some(family);
    }
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
    .find(|family| is_installed(family))
    .map(str::to_string)
}

/// WebKitGTK (`FontCacheFreeType`) asks fontconfig for each family in a CSS
/// stack and keeps the match only when it is the requested family, a generic
/// family, or a *strong* alias of it (metric aliases, `local.conf` rules);
/// otherwise it moves on to the next family. `fc-match` gives the match and
/// `fc-pattern -c` the substituted family list, where strong bindings come
/// first and the weak default chain follows.
mod fontconfig {
    use std::process::Command;
    use std::sync::OnceLock;

    const GENERIC: [&str; 6] = [
        "sans",
        "sans-serif",
        "serif",
        "monospace",
        "cursive",
        "fantasy",
    ];

    fn run(program: &str, pattern: &str) -> Option<String> {
        let mut command = Command::new(program);
        if program == "fc-pattern" {
            command.arg("-c");
        }
        let output = command
            .args(["-f", "%{family}", &escape(pattern)])
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// fontconfig pattern syntax reads `-` as the size separator and `:`/`,`
    /// as element separators.
    fn escape(pattern: &str) -> String {
        pattern
            .chars()
            .flat_map(|c| match c {
                '-' | ':' | ',' | '\\' => vec!['\\', c],
                _ => vec![c],
            })
            .collect()
    }

    fn first_family(families: &str) -> &str {
        families.split(',').next().unwrap_or("").trim()
    }

    /// The weak default chain fontconfig appends to every pattern, taken from
    /// a family nothing matches.
    fn default_chain() -> &'static [String] {
        static CHAIN: OnceLock<Vec<String>> = OnceLock::new();
        CHAIN.get_or_init(|| {
            run("fc-pattern", "zzz-no-such-family")
                .map(|list| {
                    list.split(',')
                        .skip(1)
                        .map(|s| s.trim().to_string())
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    pub(super) fn strong_aliases(family: &str, substituted: &str) -> Vec<String> {
        let chain = default_chain();
        substituted
            .split(',')
            .map(str::trim)
            .take_while(|entry| !chain.iter().any(|weak| weak.eq_ignore_ascii_case(entry)))
            .filter(|entry| !entry.eq_ignore_ascii_case(family))
            .map(str::to_string)
            .collect()
    }

    fn accepts(family: &str, matched: &str) -> bool {
        if GENERIC.contains(&family) || matched.eq_ignore_ascii_case(family) {
            return true;
        }
        run("fc-pattern", family)
            .map(|substituted| {
                strong_aliases(family, &substituted)
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(matched))
            })
            .unwrap_or(false)
    }

    pub(super) fn resolve_stack(
        stack: &[&str],
        is_installed: &dyn Fn(&str) -> bool,
    ) -> Option<String> {
        if !cfg!(target_os = "linux") {
            return None;
        }
        stack.iter().find_map(|family| {
            let matched = run("fc-match", family)?;
            let matched = first_family(&matched);
            (!matched.is_empty() && accepts(family, matched) && is_installed(matched))
                .then(|| matched.to_string())
        })
    }
}

/// GTK's `gtk-font-name`, the fallback when fontconfig tools are unavailable.
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
    fn strong_aliases_stop_at_the_weak_default_chain() {
        // Only meaningful where fontconfig is installed; the default chain is
        // read from `fc-pattern`, so an empty chain keeps everything.
        let aliases = super::fontconfig::strong_aliases(
            "Menlo",
            "JetBrains Mono,DejaVu LGC Sans,Noto Sans,sans-serif",
        );
        assert_eq!(aliases.first().map(String::as_str), Some("JetBrains Mono"));
        assert!(!aliases.iter().any(|a| a == "Menlo"));
    }

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

    #[test]
    fn glass_card_fill_matches_the_measured_tauri_dialog() {
        // `bg-card/60` over `bg-black/40` over the white page: 0.6 + 0.4 * 0.6.
        // The probe read #d6d6d6; the blur also samples text and borders, so
        // the flat fill lands within a step of it.
        let fill = glass_card_fill(Theme::light());
        for channel in [fill.r, fill.g, fill.b] {
            let byte = (channel * 255.0).round() as i32;
            assert!((byte - 0xd6).abs() <= 1, "channel {byte}");
        }
        assert_eq!(fill.a, 1.0);
    }
}
