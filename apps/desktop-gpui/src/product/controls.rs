//! Native counterparts of the shipping settings building blocks
//! (`settings/page-title.tsx`, `settings/setting-row.tsx`, and the `@anlg/ui` switch, button,
//! and select). Sizes follow the Tailwind classes those components use.

use gpui::{
    App, ClickEvent, Div, ElementId, FontWeight, SharedString, Stateful, Window, deferred, div,
    prelude::*, px, svg,
};

use crate::ui::theme::Theme;

/// Shipping's `--font-hand` stack.
pub const HAND_FONT: &str = if cfg!(target_os = "macos") {
    "Bradley Hand"
} else if cfg!(target_os = "windows") {
    "Segoe Print"
} else {
    "Comic Sans MS"
};

/// Width of the fixed control column in a setting row (`w-48`).
pub const CONTROL_WIDTH: f32 = 192.;

type Handler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

pub fn page_title(title: impl Into<SharedString>) -> Div {
    div()
        .font_family(HAND_FONT)
        .text_size(px(30.))
        .line_height(px(37.5))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title.into())
}

pub fn section_title(title: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(18.))
        .line_height(px(28.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title.into())
}

pub fn description(text: impl Into<SharedString>, colors: &Theme) -> Div {
    div()
        .text_xs()
        .line_height(px(16.))
        .text_color(colors.muted_foreground)
        .child(text.into())
}

/// `SettingRow`: a title and description on the left, the control on the right.
/// `fixed` keeps the control column at `w-48`; otherwise it shrinks to the control.
pub fn setting_row(
    title: impl Into<SharedString>,
    detail: Option<SharedString>,
    fixed: bool,
    control: impl IntoElement,
    colors: &Theme,
) -> Div {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .mb_1()
                        .text_sm()
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title.into()),
                )
                .when_some(detail, |view, detail| {
                    view.child(description(detail, colors))
                }),
        )
        .child(
            div()
                .flex()
                .justify_end()
                .when(fixed, |view| view.w(px(CONTROL_WIDTH)).min_w_0())
                .when(!fixed, |view| view.flex_shrink_0())
                .child(control),
        )
}

/// The indented group shipping uses for dependent settings (`ml-3 border-l-2 pl-4`).
pub fn nested(colors: &Theme) -> Div {
    div()
        .ml_3()
        .pl_4()
        .border_l_2()
        .border_color(colors.muted)
        .flex()
        .flex_col()
        .gap_4()
}

pub fn switch(
    id: impl Into<ElementId>,
    checked: bool,
    enabled: bool,
    colors: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(44.))
        .h(px(24.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .rounded_full()
        .border_2()
        .border_color(if checked {
            colors.primary
        } else {
            colors.border
        })
        .bg(if checked {
            colors.primary
        } else {
            colors.muted
        })
        .when(checked, |view| view.pl(px(20.)))
        .child(
            div()
                .size(px(20.))
                .rounded_full()
                .shadow_lg()
                .bg(if checked {
                    colors.primary_foreground
                } else {
                    colors.background
                }),
        )
        .when(!enabled, |view| view.opacity(0.5))
        .when(enabled, |view| view.cursor_pointer().on_click(on_click))
}

pub fn outline_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
    colors: &Theme,
    on_click: Option<Handler>,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(36.))
        .px_4()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .gap_2()
        .rounded(px(8.))
        .border_1()
        .border_color(colors.input)
        .bg(colors.background)
        .shadow_xs()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .child(label.into())
        .when(!enabled, |view| view.opacity(0.5))
        .when_some(on_click.filter(|_| enabled), |view, on_click| {
            view.cursor_pointer()
                .hover(|style| style.bg(colors.accent))
                .on_click(on_click)
        })
}

pub fn icon(path: &'static str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
}

/// The select trigger (`SelectTrigger` with `bg-card h-9`): the current label, or a muted
/// placeholder, with the caret.
pub fn select_trigger(
    id: impl Into<ElementId>,
    label: Option<SharedString>,
    placeholder: &'static str,
    enabled: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let empty = label.is_none();
    div()
        .id(id)
        .w_full()
        .h(px(36.))
        .pl_3()
        .pr_2()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .rounded(px(8.))
        .border_1()
        .border_color(colors.input)
        .bg(colors.card)
        .text_sm()
        .child(
            div()
                .min_w_0()
                .truncate()
                .when(empty, |view| view.text_color(colors.muted_foreground))
                .child(label.unwrap_or_else(|| placeholder.into())),
        )
        .child(icon(
            "ArrowDown01Icon.svg",
            16.,
            colors.foreground.opacity(0.5),
        ))
        .when(!enabled, |view| view.opacity(0.5))
        .when(enabled, |view| view.cursor_pointer())
}

/// Places `menu` directly under the element it is attached to, above all other content.
pub fn dropdown(menu: impl IntoElement, align_end: bool) -> Div {
    div()
        .absolute()
        .top_full()
        .when(!align_end, |view| view.left_0())
        .when(align_end, |view| view.right_0())
        .child(
            deferred(
                gpui::anchored()
                    .anchor(if align_end {
                        gpui::Anchor::TopRight
                    } else {
                        gpui::Anchor::TopLeft
                    })
                    .snap_to_window_with_margin(px(8.))
                    .child(menu),
            )
            .with_priority(1),
        )
}

pub fn menu_panel(width: f32, colors: &Theme) -> Div {
    div()
        .mt_1()
        .w(px(width))
        .p_1()
        .flex()
        .flex_col()
        .rounded(px(12.))
        .border_1()
        .border_color(colors.border)
        .bg(colors.popover)
        .text_color(colors.popover_foreground)
        .shadow_md()
        .occlude()
}

pub fn menu_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    detail: Option<SharedString>,
    selected: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let hover = colors.accent;
    div()
        .id(id)
        .w_full()
        .py(px(6.))
        .pl_2()
        .pr_2()
        .flex()
        .items_center()
        .gap_2()
        .rounded(px(8.))
        .text_sm()
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(div().flex_1().min_w_0().truncate().child(label.into()))
        .when_some(detail, |view, detail| {
            view.child(
                div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(detail),
            )
        })
        .child(div().size(px(16.)).flex_shrink_0().when(selected, |view| {
            view.child(icon("product/CheckIcon.svg", 16., colors.foreground))
        }))
}
