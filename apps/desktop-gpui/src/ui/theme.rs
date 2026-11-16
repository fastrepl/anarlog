use gpui::{
    App, FontStyle, FontWeight, Global, SharedString, TextSystem, Window, WindowAppearance, font,
};

include!(concat!(env!("OUT_DIR"), "/tokens.rs"));

pub const RADIUS: f32 = 8.0;

const SYSTEM_FONT: &str = if cfg!(target_os = "macos") {
    ".SystemUIFont"
} else if cfg!(target_os = "windows") {
    "Segoe UI"
} else {
    "sans-serif"
};

pub struct UiFonts {
    sans: SharedString,
    mono: SharedString,
}

impl Global for UiFonts {}

impl UiFonts {
    pub fn init(cx: &mut App) {
        let text_system = cx.text_system();
        let fonts = Self {
            sans: resolve_family(
                text_system,
                &[
                    SYSTEM_FONT,
                    "Noto Sans",
                    "DejaVu Sans",
                    "Liberation Sans",
                    "Arial",
                ],
            ),
            mono: resolve_family(
                text_system,
                &[
                    "monospace",
                    "Menlo",
                    "Consolas",
                    "DejaVu Sans Mono",
                    "Liberation Mono",
                ],
            ),
        };
        cx.set_global(fonts);
    }
}

fn resolve_family(text_system: &TextSystem, families: &[&'static str]) -> SharedString {
    let available = text_system.all_font_names();
    for family in families {
        if !available.iter().any(|name| name == family) {
            continue;
        }
        let normal = text_system.resolve_font(&font(*family));
        let mut variant = text_system
            .get_font_for_id(normal)
            .expect("resolved system font");
        variant.style = FontStyle::Normal;
        variant.weight = FontWeight::NORMAL;
        let family = variant.family.clone();
        variant.style = FontStyle::Italic;
        let italic = text_system.resolve_font(&variant);
        variant.weight = FontWeight::BOLD;
        let bold_italic = text_system.resolve_font(&variant);
        variant.style = FontStyle::Normal;
        let bold = text_system.resolve_font(&variant);
        if normal != italic && bold != bold_italic && normal != bold && italic != bold_italic {
            return family;
        }
    }
    let id = text_system.resolve_font(&font(families[0]));
    text_system
        .get_font_for_id(id)
        .expect("resolved system font")
        .family
}

pub fn system_font(cx: &App) -> SharedString {
    cx.global::<UiFonts>().sans.clone()
}

pub fn monospace_font(cx: &App) -> SharedString {
    cx.global::<UiFonts>().mono.clone()
}

pub fn theme(window: &Window) -> Theme {
    match window.appearance() {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Theme::dark(),
        _ => Theme::light(),
    }
}
