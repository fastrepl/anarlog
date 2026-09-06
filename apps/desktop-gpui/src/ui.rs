//! Small element helpers mirroring the Tailwind utility combinations the Tauri
//! app uses repeatedly.

use gpui::{Div, ElementId, Pixels, Rgba, SharedString, Stateful, Svg, div, prelude::*, px, svg};

use crate::theme::{Theme, alpha};

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

/// `Button size="icon" variant="ghost"` as used by the header overflow menu.
pub fn ghost_icon_button(id: impl Into<ElementId>, theme: Theme, hovered: bool) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .flex()
        .size(px(36.0))
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
