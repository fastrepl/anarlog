//! Small element helpers mirroring the Tailwind utility combinations the Tauri
//! app uses repeatedly.

use gpui::{Div, ElementId, Pixels, Rgba, SharedString, Stateful, Svg, div, prelude::*, px, svg};

use crate::theme::{Theme, alpha};

/// Tailwind font-size utilities set a line height too (12/16, 14/20, 16/24);
/// GPUI's `text_xs`/`text_sm`/`text_base` only set the size and keep a φ line
/// height, which makes every row taller than the web app's.
pub trait TailwindText: Styled + Sized {
    fn tw_text_xs(self) -> Self {
        self.text_size(px(12.0)).line_height(px(16.0))
    }
    fn tw_text_sm(self) -> Self {
        self.text_size(px(14.0)).line_height(px(20.0))
    }
    fn tw_text_base(self) -> Self {
        self.text_size(px(16.0)).line_height(px(24.0))
    }
    /// `text-[11px] leading-4`.
    fn tw_text_11(self) -> Self {
        self.text_size(px(11.0)).line_height(px(16.0))
    }
}

impl<T: Styled> TailwindText for T {}

/// Monochrome Hugeicons glyph. `svg` paints with its own text colour only, so
/// the colour is passed explicitly rather than inherited.
pub fn icon(name: &str, size: Pixels, color: Rgba) -> Svg {
    svg()
        .path(SharedString::from(format!("icons/{name}.svg")))
        .size(size)
        .flex_shrink_0()
        .text_color(color)
}

/// `LeftSurfaceChromeButton`: `size-7 rounded-full text-muted-foreground
/// hover:bg-accent hover:text-foreground`. `hovered` drives the icon colour
/// because svg children do not pick up the parent's hover text colour.
pub fn chrome_button(id: impl Into<ElementId>, theme: Theme) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .size(px(28.0))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .rounded_full()
        .cursor_pointer()
        .hover(move |style| style.bg(theme.accent))
}

/// `Button size="icon" variant="ghost"` as used by the header overflow menu.
pub fn ghost_icon_button(id: impl Into<ElementId>, theme: Theme) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .size(px(36.0))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .rounded_full()
        .cursor_pointer()
        .hover(move |style| style.bg(theme.accent))
}

/// Windows-style title bar control: `h-10 w-[46px]`, `hover:bg-foreground/10`,
/// or the red close treatment.
pub fn window_control(id: impl Into<ElementId>, theme: Theme, close: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .h(px(40.0))
        .w(px(46.0))
        .items_center()
        .justify_center()
        .text_color(theme.foreground)
        .when(close, move |button| {
            button.hover(move |style| style.bg(theme.close_hover))
        })
        .when(!close, move |button| {
            button.hover(move |style| style.bg(alpha(theme.foreground, 0.1)))
        })
}
