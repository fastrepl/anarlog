//! Small element helpers mirroring the Tailwind utility combinations the Tauri
//! app uses repeatedly.

use gpui::{
    Div, ElementId, FontWeight, Pixels, Rgba, SharedString, Stateful, Svg, div, prelude::*, px, svg,
};

use crate::theme::{Theme, alpha};

/// The resolved `system-ui` family, recorded once so free functions that
/// shape text (`setting_row`'s description) use the workspace's font.
static UI_FONT: std::sync::OnceLock<Option<SharedString>> = std::sync::OnceLock::new();

pub fn set_ui_font(family: Option<SharedString>) {
    let _ = UI_FONT.set(family);
}

pub fn ui_font() -> Option<SharedString> {
    UI_FONT.get().cloned().flatten()
}

/// A `p` in the UI font: the app's global `p { text-wrap: pretty }` applies,
/// so the paragraph wraps like WebKit's pretty algorithm rather than greedily.
pub fn pretty_paragraph(
    text: &str,
    size: Pixels,
    line_height: Pixels,
    color: impl Into<gpui::Hsla>,
) -> crate::prose_text::ProseText {
    let font = gpui::font(ui_font().unwrap_or_else(|| "sans-serif".into()));
    let run = gpui::TextRun {
        len: text.len(),
        font,
        color: color.into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    crate::prose_text::ProseText::new(text.to_string(), vec![run], size, line_height).pretty()
}

/// Tailwind font-size utilities set a line height too (12/16, 14/20, 16/24);
/// GPUI's `text_xs`/`text_sm`/`text_base` only set the size and keep a φ line
/// height, which makes every row taller than the web app's.
///
/// WebCore truncates the used line height to whole pixels after evaluating
/// Tailwind's `calc()` ratios in single precision, so `text-xs`
/// (`calc(1 / 0.75) * 12px` = 15.99999) lays out as 15px while `text-sm`
/// (`calc(1.25 / 0.875) * 14px` = 20.0000006) stays 20px. The values here are
/// the measured WebKit boxes, not the nominal Tailwind ones.
pub trait TailwindText: Styled + Sized {
    fn tw_text_xs(self) -> Self {
        self.text_size(px(12.0)).line_height(px(15.0))
    }
    fn tw_text_sm(self) -> Self {
        self.text_size(px(14.0)).line_height(px(20.0))
    }
    fn tw_text_base(self) -> Self {
        self.text_size(px(16.0)).line_height(px(24.0))
    }
    /// `text-lg`
    fn tw_text_lg(self) -> Self {
        self.text_size(px(18.0)).line_height(px(28.0))
    }
    /// `text-[11px] leading-4`.
    fn tw_text_11(self) -> Self {
        self.text_size(px(11.0)).line_height(px(16.0))
    }
}

impl<T: Styled> TailwindText for T {}

/// Monochrome Hugeicons glyph. `svg` paints with its own text colour only, so
/// the colour is passed explicitly rather than inherited.
/// `<CircleNotch className="animate-spin" />`: Tailwind's 1s linear rotation.
pub fn spinner(id: impl Into<gpui::ElementId>, size: Pixels, color: Rgba) -> impl IntoElement {
    use gpui::AnimationExt;
    icon("circle-notch", size, color).with_animation(
        id,
        gpui::Animation::new(std::time::Duration::from_secs(1)).repeat(),
        |svg, delta| svg.with_transformation(gpui::Transformation::rotate(gpui::percentage(delta))),
    )
}

/// Tailwind's `ring-{width}` (after `ring-offset-{offset}`): a spread-only
/// `box-shadow`, which GPUI's shadow shader paints as nothing at zero blur,
/// so the ring is a border box floated `offset` outside the element's edge.
/// The element must be `relative()`; GPUI positions the box against its
/// padding box (and paints the element's own border over children), so its
/// `border` width is stepped over; `radius` is the element's corner radius.
pub fn ring(color: Rgba, width: f32, offset: f32, border: f32, radius: f32) -> Div {
    let outside = width + offset;
    let inset = outside + border;
    div()
        .absolute()
        .top(px(-inset))
        .left(px(-inset))
        .right(px(-inset))
        .bottom(px(-inset))
        .rounded(px(radius + outside))
        .border(px(width))
        .border_color(color)
}

/// The shadcn `Input`'s `shadow-xs`.
pub fn input_shadow() -> Vec<gpui::BoxShadow> {
    vec![gpui::BoxShadow {
        color: gpui::hsla(0.0, 0.0, 0.0, 0.05),
        offset: gpui::point(px(0.0), px(1.0)),
        blur_radius: px(2.0),
        spread_radius: px(0.0),
    }]
}

/// The shadcn `Input`'s `focus-visible:ring-1 ring-ring` over its
/// `rounded-md border` box.
pub fn input_focus_ring(theme: Theme) -> Div {
    ring(theme.ring, 1.0, 0.0, 1.0, 6.0)
}

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
pub fn chrome_button(id: impl Into<ElementId>, theme: Theme, hovered: bool) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .flex()
        .size(px(28.0))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .cursor_pointer()
        // `hover:bg-accent` under the Button's control squircle.
        .when(hovered, |button| {
            button.child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                Some(theme.accent),
                None,
            ))
        })
}

/// `Button variant="ghost" size="icon"` as used by the header overflow menu:
/// the `size-7 rounded-full` button whose hover fill is the control squircle.
pub fn ghost_icon_button(id: impl Into<ElementId>, theme: Theme, hovered: bool) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .flex()
        .size(px(28.0))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .when(hovered, |button| {
            button.child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                Some(theme.accent),
                None,
            ))
        })
}

/// `Kbd`: `inline-flex h-5 min-w-5 items-center justify-center rounded px-1
/// font-mono text-xs leading-none font-medium border border-border bg-muted
/// text-muted-foreground` under a 1px `--kbd-shadow-outer` drop and a 1px
/// `--kbd-shadow-inset` highlight (GPUI paints no zero-blur shadow, so both
/// are drawn as bands). `lifted` is the empty state's `group-hover`:
/// `-translate-y-0.5` with the shadow grown to 2px.
pub fn kbd(
    theme: Theme,
    mono: Option<SharedString>,
    text: impl Into<SharedString>,
    lifted: bool,
) -> Div {
    let (outer_shadow, inset) = if theme.dark {
        (
            crate::theme::alpha(gpui::rgb(0x000000), 0.35),
            crate::theme::alpha(gpui::rgb(0xffffff), 0.1),
        )
    } else {
        (
            crate::theme::alpha(gpui::rgb(0x000000), 0.1),
            crate::theme::alpha(gpui::rgb(0xffffff), 0.8),
        )
    };
    let drop = if lifted { 2.0 } else { 1.0 };
    div()
        .relative()
        .flex_shrink_0()
        .when(lifted, |chip| chip.mt(px(-2.0)).mb(px(2.0)))
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(drop))
                .bottom(px(-drop))
                .rounded(px(4.0))
                .bg(outer_shadow),
        )
        .child(
            div()
                .relative()
                .flex()
                .h(px(20.0))
                .min_w(px(20.0))
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .border_1()
                .border_color(theme.border)
                .bg(theme.muted)
                .px_1()
                .text_size(px(12.0))
                .line_height(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.muted_foreground)
                .when_some(mono, |chip, family| chip.font_family(family))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top_0()
                        .h(px(1.0))
                        .rounded_t(px(3.0))
                        .bg(inset),
                )
                .child(text.into()),
        )
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

/// `::-webkit-scrollbar` from `styles/scrollbar.css`: a 6px gutter with a
/// `#e5e5e5` (`.dark`: `#44403c`) thumb under `border-radius: 4px`. WebKitGTK
/// reserves the gutter inside the scroller, so callers pad their content by
/// [`scrollbar_gutter`] and paint this over the right edge of a `relative`
/// wrapper around the `overflow_y_scroll` container.
pub const WEBKIT_SCROLLBAR_WIDTH: f32 = 6.0;

pub fn scrollbar_gutter(handle: &gpui::ScrollHandle) -> Pixels {
    if handle.max_offset().height > px(0.0) {
        px(WEBKIT_SCROLLBAR_WIDTH)
    } else {
        px(0.0)
    }
}

pub fn webkit_scrollbar(handle: gpui::ScrollHandle, thumb: Rgba) -> impl IntoElement {
    // The gutter the caller reserved this frame; the scroller only learns its
    // overflow while laying out, so a change here needs one more frame.
    let laid_out_gutter = scrollbar_gutter(&handle);
    gpui::canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            if scrollbar_gutter(&handle) != laid_out_gutter {
                window.request_animation_frame();
            }
            let max = handle.max_offset().height;
            if max <= px(0.0) {
                return;
            }
            let viewport = bounds.size.height;
            let content = viewport + max;
            // Measured against WebKitGTK: the thumb is 5px wide at the gutter's
            // left, starts 1px down, and its length is taken from a track
            // inset 2px at both ends.
            let track = viewport - px(4.0);
            let thumb_height = (track * (f32::from(viewport) / f32::from(content)))
                .floor()
                .max(px(20.0));
            let progress = (f32::from(-handle.offset().y) / f32::from(max)).clamp(0.0, 1.0);
            let top = bounds.top() + px(1.0) + (viewport - px(2.0) - thumb_height) * progress;
            window.paint_quad(
                gpui::fill(
                    gpui::Bounds::new(
                        gpui::point(bounds.left(), top),
                        gpui::size(px(WEBKIT_SCROLLBAR_WIDTH - 1.0), thumb_height),
                    ),
                    thumb,
                )
                .corner_radii(px(2.5)),
            );
        },
    )
    .absolute()
    .top_0()
    .bottom_0()
    .right_0()
    .w(px(WEBKIT_SCROLLBAR_WIDTH))
}
