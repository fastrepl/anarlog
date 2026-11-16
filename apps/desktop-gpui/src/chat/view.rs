use std::time::Duration;

use chrono::Utc;
use gpui::{
    Animation, AnimationExt, AnyElement, App, BoxShadow, Context, Div, Entity, FontWeight, Hsla,
    KeyDownEvent, MouseButton, Render, ScrollHandle, SharedString, Stateful, Svg, Transformation,
    Window, WindowAppearance, anchored, deferred, div, percentage, point, prelude::*, px, rgb,
    rgba, svg,
};

use super::{ChatView, Entry, Phase, markdown, state, store::Display};
use crate::{
    chat::state::{ChatEvent, ChatMode},
    ui::theme::{Theme, monospace_font, theme},
    workspace::navigation::Route,
};

const FLOATING_MAX_WIDTH: f32 = 650.;
const CTA_MAX_WIDTH: f32 = 640.;
const RIGHT_PANEL_MIN_WIDTH: f32 = 320.;

fn dark(window: &Window) -> bool {
    matches!(
        window.appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}

fn shadow(y: f32, blur: f32, alpha: f32) -> BoxShadow {
    BoxShadow {
        color: gpui::black().opacity(alpha),
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }
}

fn icon(path: &'static str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_none()
        .text_color(color)
}

fn spinner(id: &'static str, size: f32, color: Hsla) -> impl IntoElement {
    icon("chat/LoaderCircleIcon.svg", size, color).with_animation(
        id,
        Animation::new(Duration::from_secs(1)).repeat(),
        |icon, delta| icon.with_transformation(Transformation::rotate(percentage(delta))),
    )
}

/// The right panel's share of the window: shipping's 30% default, 20–50%, at least 320px.
pub fn reserved_width(chat: &Entity<ChatView>, window: &Window, cx: &App) -> f32 {
    let chat = chat.read(cx);
    if chat.mode != ChatMode::RightPanelOpen {
        return 0.;
    }
    let total = f32::from(window.viewport_size().width);
    (total * chat.right_fraction).max(RIGHT_PANEL_MIN_WIDTH)
}

struct ResizeChat;

pub fn right_panel(chat: &Entity<ChatView>, window: &Window, cx: &App) -> Option<AnyElement> {
    let width = reserved_width(chat, window, cx);
    if width == 0. {
        return None;
    }
    let colors = theme(window);
    let resize = chat.clone();
    Some(
        div()
            .id("chat-right-panel")
            .relative()
            .w(px(width))
            .h_full()
            .flex_shrink_0()
            .bg(colors.card)
            .border_l_1()
            .border_r_1()
            .border_color(colors.border)
            .rounded_tr(px(12.))
            .overflow_hidden()
            .on_drag_move(move |event: &gpui::DragMoveEvent<ResizeChat>, window, cx| {
                let total = f32::from(window.viewport_size().width);
                if total > 0. {
                    let fraction = (total - f32::from(event.event.position.x)) / total;
                    resize.update(cx, |chat, cx| {
                        chat.right_fraction = fraction.clamp(0.2, 0.5);
                        cx.notify();
                    });
                }
            })
            .child(chat.clone())
            .child(
                div()
                    .id("chat-right-panel-resize")
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(px(4.))
                    .h_full()
                    .cursor_col_resize()
                    .on_drag(ResizeChat, |_, _, _, cx| cx.new(|_| gpui::Empty)),
            )
            .into_any_element(),
    )
}

/// The floating button or floating chat drawn over the main surface.
pub fn overlay(
    chat: &Entity<ChatView>,
    route: &Route,
    window: &Window,
    cx: &App,
) -> Option<AnyElement> {
    let colors = theme(window);
    let mode = chat.read(cx).mode;
    match mode {
        ChatMode::FloatingClosed if state::shows_cta(route) => {
            let open = chat.clone();
            Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .items_center()
                    .px_4()
                    .pb_3()
                    .child(
                        div()
                            .id("chat-cta")
                            .w_full()
                            .max_w(px(CTA_MAX_WIDTH))
                            .h(px(40.))
                            .flex()
                            .items_center()
                            .px_4()
                            .rounded_full()
                            .border_1()
                            .border_color(colors.border.opacity(0.7))
                            .bg(if dark(window) {
                                colors.card
                            } else {
                                gpui::white()
                            })
                            .shadow(vec![shadow(6., 20., if dark(window) { 0.32 } else { 0.1 })])
                            .cursor_text()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(colors.muted_foreground)
                            .on_click(move |_, window, cx| {
                                open.update(cx, |chat, cx| chat.event(ChatEvent::Open, window, cx))
                            })
                            .child(div().flex_1().min_w_0().truncate().child("Ask anything")),
                    )
                    .into_any_element(),
            )
        }
        ChatMode::FloatingOpen => {
            let close = chat.clone();
            Some(
                div()
                    .id("chat-floating-frame")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .items_center()
                    .px(px(11.))
                    .pt(px(46.))
                    .pb(px(7.))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        close.update(cx, |chat, cx| {
                            if chat.draft_empty(cx) {
                                chat.event(ChatEvent::Close, window, cx);
                            }
                        })
                    })
                    .child(
                        div()
                            .id("chat-floating-panel")
                            .occlude()
                            .w_full()
                            .max_w(px(FLOATING_MAX_WIDTH))
                            .max_h_full()
                            .flex()
                            .flex_col()
                            .min_h_0()
                            .overflow_hidden()
                            .rounded(px(24.))
                            .border_1()
                            .border_color(colors.border.opacity(0.7))
                            .border_t_1()
                            .bg(if dark(window) {
                                rgb(0x202020).into()
                            } else {
                                Hsla::from(rgb(0xf4f4f5))
                            })
                            .shadow(vec![shadow(
                                if dark(window) { 16. } else { 12. },
                                if dark(window) { 40. } else { 32. },
                                if dark(window) { 0.4 } else { 0.14 },
                            )])
                            .child(chat.clone()),
                    )
                    .into_any_element(),
            )
        }
        _ => None,
    }
}

pub(super) fn at_bottom(scroll: &ScrollHandle) -> bool {
    -f32::from(scroll.offset().y) >= f32::from(scroll.max_offset().y) - 4.
}

fn humanize(name: &str) -> String {
    let mut text = name.replace('_', " ");
    if let Some(first) = text.get(..1) {
        text.replace_range(..1, &first.to_uppercase());
    }
    text
}

impl ChatView {
    fn toolbar_button(
        &self,
        id: &'static str,
        path: &'static str,
        size: f32,
        colors: &Theme,
        listener: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .group(id)
            .size(px(size))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .hover(|style| style.bg(colors.muted.opacity(0.8)))
            .active(|style| style.bg(colors.muted))
            .on_click(cx.listener(move |this, _, window, cx| listener(this, window, cx)))
            .child(
                icon(path, 16., colors.muted_foreground)
                    .group_hover(id, |style| style.text_color(colors.foreground)),
            )
    }

    fn toolbar(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let colors = theme(window);
        let right = self.mode == ChatMode::RightPanelOpen;
        let button = if right { 28. } else { 32. };
        let actions = if right {
            vec![
                self.toolbar_button(
                    "chat-new",
                    "chat/PlusIcon.svg",
                    button,
                    &colors,
                    |this, _, cx| this.new_chat(cx),
                    cx,
                ),
                self.toolbar_button(
                    "chat-float",
                    "chat/PictureInPictureIcon.svg",
                    button,
                    &colors,
                    |this, window, cx| this.event(ChatEvent::Open, window, cx),
                    cx,
                ),
                self.toolbar_button(
                    "chat-close",
                    "chat/XIcon.svg",
                    button,
                    &colors,
                    |this, window, cx| this.event(ChatEvent::Close, window, cx),
                    cx,
                ),
            ]
        } else {
            vec![
                self.toolbar_button(
                    "chat-new",
                    "chat/PlusIcon.svg",
                    button,
                    &colors,
                    |this, _, cx| this.new_chat(cx),
                    cx,
                ),
                self.toolbar_button(
                    "chat-right",
                    "workspace/SidebarLeftIcon.svg",
                    button,
                    &colors,
                    |this, window, cx| this.event(ChatEvent::OpenRightPanel, window, cx),
                    cx,
                ),
            ]
        };
        let open = self.history_open;
        let trigger = div()
            .id("chat-history")
            .group("chat-history")
            .relative()
            .h(px(button))
            .ml(px(-8.))
            .px(px(10.))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .rounded_full()
            .cursor_pointer()
            .when(open, |view| view.bg(colors.muted.opacity(0.8)))
            .hover(|style| style.bg(colors.muted.opacity(0.8)))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_history(cx)))
            .child(icon("chat/HistoryIcon.svg", 16., colors.muted_foreground))
            .child(icon(
                if open {
                    "ArrowUp01Icon.svg"
                } else {
                    "ArrowDown01Icon.svg"
                },
                14.,
                colors.muted_foreground,
            ))
            .when(open, |view| {
                view.child(self.history_menu(right, window, cx))
            });
        div()
            .flex()
            .flex_shrink_0()
            .when(right, |view| view.h(px(36.)).items_start().pt(px(9.)))
            .when(!right, |view| view.h(px(44.)).items_center())
            .child(
                div()
                    .flex()
                    .size_full()
                    .min_w_0()
                    .items_center()
                    .gap_2()
                    .when(right, |view| view.pl_3().pr_1())
                    .when(!right, |view| view.px_3())
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .child(trigger),
                    )
                    .child(div().flex().flex_none().items_center().children(actions)),
            )
    }

    fn history_menu(&self, right: bool, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let now = Utc::now();
        let items = self.groups.iter().map(|group| {
            let active = self.group.as_ref() == Some(&group.id);
            let id = group.id.clone();
            div()
                .id(SharedString::from(format!("chat-group-{}", group.id)))
                .w_full()
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(10.))
                .py(px(6.))
                .rounded(px(14.))
                .cursor_pointer()
                .when(active, |view| {
                    view.bg(colors.muted).hover(|s| s.bg(colors.accent))
                })
                .when(!active, |view| {
                    view.hover(|s| s.bg(colors.accent))
                        .active(|s| s.bg(colors.muted))
                })
                .on_click(cx.listener(move |this, _, _, cx| this.select_group(id.clone(), cx)))
                .child(icon(
                    "chat/MessageCircleIcon.svg",
                    14.,
                    colors.muted_foreground,
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .text_sm()
                                .line_height(px(20.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if active {
                                    colors.foreground
                                } else {
                                    colors.muted_foreground
                                })
                                .child(SharedString::from(group.title.clone())),
                        )
                        .child(
                            div()
                                .mt(px(2.))
                                .text_size(px(11.))
                                .line_height(px(16.))
                                .text_color(colors.muted_foreground)
                                .child(
                                    group
                                        .created_at
                                        .map(|at| state::distance_to_now(at, now))
                                        .unwrap_or_default(),
                                ),
                        ),
                )
        });
        let panel = div()
            .id("chat-history-menu")
            .occlude()
            .w(px(288.))
            .p(px(2.))
            .rounded(px(22.))
            .border_1()
            .border_color(colors.app_floating_border)
            .bg(colors.app_floating_chrome)
            .shadow(vec![shadow(10., 15., 0.1), shadow(4., 6., 0.1)])
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.history_open = false;
                cx.notify();
            }))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .p(px(6.))
                    .rounded(px(20.))
                    .border_1()
                    .border_color(colors.app_floating_border)
                    .bg(colors.app_floating_panel)
                    .text_color(colors.popover_foreground)
                    .child(
                        div()
                            .px_2()
                            .py(px(6.))
                            .text_size(px(10.))
                            .line_height(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors.muted_foreground)
                            .child("RECENT CHATS"),
                    )
                    .when(self.groups.is_empty(), |view| {
                        view.child(
                            div()
                                .px_3()
                                .py_6()
                                .flex()
                                .flex_col()
                                .items_center()
                                .child(
                                    icon(
                                        "chat/MessageCircleIcon.svg",
                                        24.,
                                        colors.muted_foreground.opacity(0.7),
                                    )
                                    .mb(px(6.)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .line_height(px(16.))
                                        .text_color(colors.muted_foreground)
                                        .child("No recent chats"),
                                ),
                        )
                    })
                    .when(!self.groups.is_empty(), |view| {
                        view.child(div().flex().flex_col().gap(px(2.)).children(items))
                    }),
            );
        let placement = if right {
            div().absolute().left_0().top_full().mt_1()
        } else {
            div().absolute().left_full().top_0().ml_1()
        };
        placement
            .child(deferred(
                anchored().snap_to_window_with_margin(px(8.)).child(panel),
            ))
            .into_any_element()
    }

    fn timestamp(
        &self,
        at: Option<chrono::DateTime<Utc>>,
        with_day: bool,
        colors: &Theme,
    ) -> Option<Div> {
        at.map(|at| {
            div()
                .text_xs()
                .line_height(px(16.))
                .text_color(colors.muted_foreground)
                .child(state::local_timestamp(at, with_day))
        })
    }

    fn activity(
        &self,
        id: &str,
        parts: &[Display],
        running: bool,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let parts = parts
            .iter()
            .filter(|part| match part {
                Display::Reasoning(text) => !text.trim().is_empty(),
                Display::Tool { .. } => true,
                Display::Text(_) => false,
            })
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return None;
        }
        let open = self.expanded.contains(id);
        let running = running
            || parts
                .iter()
                .any(|part| matches!(part, Display::Tool { running: true, .. }));
        let toggle = id.to_owned();
        Some(
            div()
                .my_2()
                .px_2()
                .py_1()
                .rounded(px(6.))
                .border_1()
                .border_color(colors.border)
                .child(
                    div()
                        .id(SharedString::from(format!("chat-activity-{id}")))
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.muted_foreground)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.expanded.remove(&toggle) {
                                this.expanded.insert(toggle.clone());
                            }
                            cx.notify();
                        }))
                        .child(if running {
                            spinner("chat-activity-spinner", 12., colors.muted_foreground)
                                .into_any_element()
                        } else {
                            icon("chat/BrainIcon.svg", 12., colors.muted_foreground)
                                .into_any_element()
                        })
                        .child(
                            div()
                                .flex_1()
                                .truncate()
                                .when(open, |view| view.font_weight(FontWeight::MEDIUM))
                                .child("Activity"),
                        )
                        .child(
                            icon("chat/ArrowRight01Icon.svg", 12., colors.muted_foreground).when(
                                open,
                                |icon| {
                                    icon.with_transformation(Transformation::rotate(percentage(
                                        0.25,
                                    )))
                                },
                            ),
                        ),
                )
                .when(open, |view| {
                    view.child(
                        div()
                            .mt_1()
                            .px_1()
                            .pt_2()
                            .border_t_1()
                            .border_color(colors.border)
                            .flex()
                            .flex_col()
                            .children(parts.into_iter().map(|part| {
                                let (path, text) = match part {
                                    Display::Reasoning(text) => {
                                        ("chat/BrainIcon.svg", text.clone())
                                    }
                                    Display::Tool { name, .. } => {
                                        ("workspace/ZapIcon.svg", humanize(name))
                                    }
                                    Display::Text(_) => unreachable!(),
                                };
                                div()
                                    .my(px(6.))
                                    .flex()
                                    .items_start()
                                    .gap_2()
                                    .text_xs()
                                    .line_height(px(16.))
                                    .text_color(colors.muted_foreground)
                                    .child(icon(path, 12., colors.muted_foreground).mt(px(2.)))
                                    .child(div().flex_1().min_w_0().child(SharedString::from(text)))
                            })),
                    )
                })
                .into_any_element(),
        )
    }

    fn message(
        &self,
        entry: &Entry,
        last_assistant: bool,
        style: &markdown::Style,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let message = &entry.message;
        let user = message.user;
        let group = SharedString::from(format!("chat-message-{}", message.id));
        let copied = self.copied.as_deref() == Some(message.id.as_str());
        let (id, text) = (message.id.clone(), message.text());
        let body = markdown::render(&entry.blocks, style, &format!("chat-md-{}", message.id));
        div()
            .flex()
            .py_2()
            .when(user, |view| view.justify_end())
            .child(
                div()
                    .group(group.clone())
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .when(user, |view| view.max_w(gpui::relative(0.85)).items_end())
                    .when(!user, |view| view.w_full())
                    .child(
                        div()
                            .min_w_0()
                            .max_w_full()
                            .text_sm()
                            .line_height(px(20.))
                            .when(user, |view| {
                                view.rounded(px(16.))
                                    .bg(rgb(0xdbeafe))
                                    .px_3()
                                    .py_1()
                                    .text_color(rgb(0x262626))
                            })
                            .when(!user, |view| view.w_full().text_color(colors.foreground))
                            .when(!user, |view| {
                                view.children(self.activity(
                                    &message.id,
                                    &message.parts,
                                    false,
                                    colors,
                                    cx,
                                ))
                            })
                            .child(body),
                    )
                    .child(
                        div()
                            .mt_1()
                            .min_h(px(24.))
                            .flex()
                            .items_center()
                            .gap_1()
                            .opacity(0.)
                            .group_hover(group, |style| style.opacity(1.))
                            .when(!user, |view| {
                                view.child(
                                    div()
                                        .id(SharedString::from(format!("chat-copy-{}", message.id)))
                                        .p_1()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.copy(id.clone(), text.clone(), cx)
                                        }))
                                        .child(if copied {
                                            icon("chat/CheckIcon.svg", 14., rgb(0x22c55e).into())
                                        } else {
                                            icon("chat/CopyIcon.svg", 14., colors.muted_foreground)
                                        }),
                                )
                                .when(last_assistant, |view| {
                                    view.child(
                                        div()
                                            .id("chat-regenerate")
                                            .p_1()
                                            .cursor_pointer()
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.regenerate(cx)),
                                            )
                                            .child(icon(
                                                "chat/RotateCcwIcon.svg",
                                                14.,
                                                colors.muted_foreground,
                                            )),
                                    )
                                })
                            })
                            .children(self.timestamp(message.created_at, false, colors)),
                    ),
            )
            .into_any_element()
    }

    fn empty_state(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let colors = theme(window);
        match self.model_configured? {
            false => Some(
                div()
                    .flex()
                    .flex_col()
                    .py_2()
                    .pb_1()
                    .child(
                        div()
                            .mb_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(colors.foreground)
                                    .child("Anarlog AI"),
                            )
                            .child(
                                div()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(rgb(0xbae6fd))
                                    .bg(rgb(0xe0f2fe))
                                    .text_color(rgb(0x0c4a6e))
                                    .text_size(px(10.))
                                    .line_height(px(15.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Beta"),
                            ),
                    )
                    .child(
                        div()
                            .mb_2()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(colors.muted_foreground)
                            .child("Hi, I'm Anarlog AI. Set up a language model and I'll be ready to help."),
                    )
                    .child(
                        div()
                            .id("chat-open-ai-settings")
                            .flex()
                            .flex_none()
                            .self_start()
                            .items_center()
                            .gap(px(6.))
                            .px_3()
                            .py(px(6.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(colors.primary)
                            .bg(colors.primary)
                            .text_color(colors.primary_foreground)
                            .text_xs()
                            .line_height(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .shadow(vec![BoxShadow {
                                color: rgba(0x57534e2e).into(),
                                offset: point(px(0.), px(4.)),
                                blur_radius: px(14.),
                                spread_radius: px(0.),
                                inset: false,
                            }])
                            .cursor_pointer()
                            .hover(|style| style.bg(colors.primary.opacity(0.9)))
                            .on_click(cx.listener(|this, _, _, cx| this.open_settings(cx)))
                            .child(icon("chat/SparkleIcon.svg", 12., colors.primary_foreground))
                            .child("Open AI Settings"),
                    )
                    .into_any_element(),
            ),
            true => {
                let context = self.context.is_some();
                let suggestions = [
                    (
                        "List action items.",
                        "chat/ListChecksIcon.svg",
                        if context {
                            "What are my action items from this meeting?"
                        } else {
                            "What are my outstanding action items across my meetings?"
                        },
                    ),
                    (
                        "Draft follow-up email.",
                        "chat/Mail01Icon.svg",
                        if context {
                            "Draft a follow-up email to the participants"
                        } else {
                            "Draft a follow-up email for my most recent meeting."
                        },
                    ),
                    (
                        "Find key decisions.",
                        "Search01Icon.svg",
                        if context {
                            "What were the key decisions that have been made?"
                        } else {
                            "What key decisions were made in my recent meetings?"
                        },
                    ),
                ];
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .pb_1()
                        .children(suggestions.into_iter().map(|(label, path, prompt)| {
                            div()
                                .id(label)
                                .group(label)
                                .w_full()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .py_2()
                                .pr_3()
                                .rounded(px(8.))
                                .cursor_pointer()
                                .text_sm()
                                .line_height(px(20.))
                                .text_color(colors.muted_foreground)
                                .hover(|style| style.bg(colors.muted.opacity(0.55)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.send(prompt.to_owned(), cx)
                                }))
                                .child(
                                    div().size(px(24.)).flex().items_center().justify_center().child(
                                        icon(path, 16., colors.muted_foreground.opacity(0.75))
                                            .group_hover(label, |style| {
                                                style.text_color(colors.foreground)
                                            }),
                                    ),
                                )
                                .child(div().flex_1().min_w_0().truncate().child(label))
                        }))
                        .into_any_element(),
                )
            }
        }
    }

    fn body(&self, window: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = theme(window);
        let right = self.mode == ChatMode::RightPanelOpen;
        let style = markdown::Style {
            colors,
            mono: monospace_font(cx),
        };
        let last_assistant = self.entries.iter().rposition(|entry| !entry.message.user);
        let busy = self.busy();
        let streaming = busy
            && !(self.stream_text.trim().is_empty() && self.stream_reasoning.trim().is_empty());
        let mut content = div()
            .flex()
            .flex_col()
            // A shrinkable child would be squeezed to the viewport and never scroll.
            .when(right, |view| {
                view.min_h_full().flex_shrink_0().px_3().py_5()
            })
            .when(!right, |view| view.px_5().py_3())
            .when(right, |view| view.child(div().flex_1()));
        if self.entries.is_empty() && !busy {
            content = content.children(self.empty_state(window, cx));
        } else {
            content = content
                .children(
                    self.entries
                        .first()
                        .and_then(|entry| self.timestamp(entry.message.created_at, true, &colors))
                        .map(|stamp| div().pt_2().pb_4().flex().justify_center().child(stamp)),
                )
                .children(self.entries.iter().enumerate().map(|(index, entry)| {
                    self.message(
                        entry,
                        Some(index) == last_assistant && !busy,
                        &style,
                        &colors,
                        cx,
                    )
                }));
            if streaming {
                content = content.child(
                    div().flex().py_2().child(
                        div()
                            .w_full()
                            .min_w_0()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(colors.foreground)
                            .children(self.activity(
                                "streaming",
                                &[Display::Reasoning(self.stream_reasoning.clone())],
                                true,
                                &colors,
                                cx,
                            ))
                            .child(markdown::render(
                                &self.stream_blocks,
                                &style,
                                "chat-md-stream",
                            )),
                    ),
                );
            } else if busy {
                content = content.child(
                    div().flex().py_2().child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(colors.foreground)
                            .child(spinner("chat-thinking", 16., colors.foreground))
                            .child("Thinking..."),
                    ),
                );
            }
            if let Phase::Failed(error) = &self.phase {
                content = content.child(
                    div().flex().py_2().child(
                        div()
                            .group("chat-error")
                            .relative()
                            .max_w_full()
                            .rounded(px(16.))
                            .border_1()
                            .border_color(rgb(0xfecaca))
                            .bg(rgb(0xfef2f2))
                            .px_3()
                            .py_1()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(rgb(0xdc2626))
                            .child(SharedString::from(error.clone()))
                            .child(
                                div()
                                    .id("chat-retry")
                                    .absolute()
                                    .top(px(-4.))
                                    .right(px(-4.))
                                    .p_1()
                                    .rounded_full()
                                    .bg(rgb(0xfee2e2))
                                    .hover(|style| style.bg(rgb(0xfecaca)))
                                    .cursor_pointer()
                                    .opacity(0.)
                                    .group_hover("chat-error", |style| style.opacity(1.))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.phase = Phase::Ready;
                                        this.regenerate(cx);
                                    }))
                                    .child(icon(
                                        "chat/RotateCcwIcon.svg",
                                        12.,
                                        rgb(0xdc2626).into(),
                                    )),
                            ),
                    ),
                );
            }
        }
        let max_height = (f32::from(window.viewport_size().height) * 0.7).min(576.);
        let scroll = self.scroll.clone();
        let show_recent = !self.entries.is_empty() && !at_bottom(&self.scroll);
        div()
            .id("chat-body")
            .relative()
            .flex()
            .flex_col()
            .min_h_0()
            .when(right, |view| view.flex_1())
            .child(
                div()
                    .id("chat-scroll")
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .when(right, |view| view.flex_1())
                    .when(!right, |view| view.max_h(px(max_height)))
                    .child(content),
            )
            .when(show_recent, |view| {
                view.child(
                    div()
                        .absolute()
                        .bottom_3()
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .id("chat-go-to-recent")
                                .h(px(32.))
                                .px_3()
                                .flex()
                                .items_center()
                                .gap_1()
                                .rounded_full()
                                .border_1()
                                .border_color(colors.border)
                                .bg(colors.accent)
                                .text_color(colors.accent_foreground)
                                .shadow(vec![shadow(1., 2., 0.05)])
                                .cursor_pointer()
                                .text_xs()
                                .line_height(px(16.))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    scroll.scroll_to_bottom();
                                    cx.notify();
                                }))
                                .child(icon("ArrowDown01Icon.svg", 12., colors.accent_foreground))
                                .child("Go to recent"),
                        ),
                )
            })
    }

    fn queue_list(&self, colors: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        if self.queue.is_empty() {
            return None;
        }
        Some(
            div().flex_shrink_0().px_3().pb(px(6.)).child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .children(self.queue.iter().map(|(id, text)| {
                        let id = *id;
                        div()
                            .id(SharedString::from(format!("chat-queued-{id}")))
                            .group("chat-queued")
                            .min_h(px(28.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .rounded(px(6.))
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .hover(|style| style.bg(colors.muted.opacity(0.55)))
                            .child(icon(
                                "chat/ArrowTurnForwardIcon.svg",
                                14.,
                                colors.muted_foreground,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(SharedString::from(text.clone())),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("chat-queued-remove-{id}")))
                                    .size(px(24.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(6.))
                                    .opacity(0.65)
                                    .cursor_pointer()
                                    .on_click(
                                        cx.listener(move |this, _, _, cx| {
                                            this.remove_queued(id, cx)
                                        }),
                                    )
                                    .child(icon(
                                        "chat/TrashIcon.svg",
                                        14.,
                                        colors.muted_foreground,
                                    )),
                            )
                    })),
            ),
        )
    }

    fn composer(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let colors = theme(window);
        let right = self.mode == ChatMode::RightPanelOpen;
        let has_content = !self.draft_empty(cx);
        let busy = self.busy();
        let send_enabled = has_content;
        let mic = div()
            .size(px(28.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .rounded_full()
            // Voice input is not ported yet; shipping disables it the same way when unavailable.
            .opacity(0.45)
            .child(icon("chat/Mic01Icon.svg", 17., colors.muted_foreground));
        let send = div()
            .id("chat-send")
            .size(px(28.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .when(!send_enabled, |view| view.border_color(colors.border))
            .when(send_enabled, |view| {
                view.bg(colors.primary)
                    .border_color(rgb(0x57534e))
                    .cursor_pointer()
                    .hover(|style| style.bg(colors.primary.opacity(0.9)))
                    .active(|style| style.bg(colors.primary.opacity(0.8)))
            })
            .on_click(cx.listener(|this, _, _, cx| {
                let text = this.input.read(cx).buffer.text.clone();
                this.submit(text, cx);
            }))
            .child(icon(
                "chat/ArrowUp02Icon.svg",
                15.,
                if send_enabled {
                    colors.primary_foreground
                } else {
                    colors.muted_foreground.opacity(0.6)
                },
            ));
        let stop = div()
            .id("chat-stop")
            .size(px(28.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .hover(|style| style.bg(colors.accent))
            .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
            .child(icon("chat/SquareIcon.svg", 14., colors.foreground));
        let actions = div()
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .when(!busy, |view| view.child(mic))
            .when(busy, |view| view.child(stop))
            .when(!busy && (right || has_content), |view| view.child(send));
        let indicator = self.sent.position().map(|(position, total)| {
            div()
                .pb_1()
                .when(right, |view| view.px_2())
                .when(!right, |view| view.px_4())
                .text_size(px(11.))
                .line_height(px(11.))
                .text_color(colors.muted_foreground.opacity(0.8))
                .child(format!("History {position}/{total}"))
        });
        let surface = if right {
            div()
                .flex()
                .flex_col()
                .rounded(px(12.))
                .border_1()
                .border_color(colors.border)
                .bg(colors.card)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .px_2()
                        .pt_3()
                        .pb_2()
                        .child(div().mb_1().text_sm().child(self.input.clone()))
                        .child(div().flex().justify_end().child(actions)),
                )
        } else {
            div()
                .flex()
                .items_center()
                .min_h(px(40.))
                .py(px(3.))
                .pl_4()
                .pr(px(6.))
                .rounded(px(20.))
                .border_1()
                .border_color(colors.border.opacity(0.7))
                .bg(if dark(window) {
                    colors.card
                } else {
                    gpui::white()
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .mr(px(14.))
                        .text_sm()
                        .child(self.input.clone()),
                )
                .child(actions)
        };
        div()
            .relative()
            .min_w_0()
            .flex_shrink_0()
            .when(right, |view| view.px_2().pb_3())
            .when(!right, |view| view.px_1().pb_1())
            .children(indicator)
            .child(surface)
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let right = self.mode == ChatMode::RightPanelOpen;
        div()
            .id("chat")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .min_h_0()
            .overflow_hidden()
            .when(right, |view| view.size_full())
            .when(!right, |view| view.max_h_full())
            .text_color(colors.card_foreground)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let modifiers = event.keystroke.modifiers;
                if modifiers.secondary(cx) || modifiers.alt || modifiers.shift {
                    return;
                }
                let handled = match event.keystroke.key.as_str() {
                    "escape" => {
                        this.event(ChatEvent::Close, window, cx);
                        true
                    }
                    "up" => this.recall(true, cx),
                    "down" => this.recall(false, cx),
                    _ => false,
                };
                if handled {
                    cx.stop_propagation();
                }
            }))
            .child(self.toolbar(window, cx))
            .child(self.body(window, cx))
            .when(self.model_configured == Some(true), |view| {
                view.children(self.queue_list(&colors, cx))
                    .child(self.composer(window, cx))
            })
    }
}
