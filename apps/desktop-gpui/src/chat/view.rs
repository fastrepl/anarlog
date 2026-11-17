use std::time::Duration;

use chrono::Utc;
use gpui::{
    Animation, AnimationExt, AnyElement, App, BoxShadow, Context, Div, Entity, FontWeight, Hsla,
    KeyDownEvent, MouseButton, Render, ScrollHandle, SharedString, Stateful, Svg, Transformation,
    Window, WindowAppearance, anchored, canvas, deferred, div, percentage, point, prelude::*, px,
    rgb, rgba, svg,
};

use super::{
    ChatView, Entry, Phase, markdown, state,
    store::Display,
    voice::{self, VoicePhase},
};
use crate::ui::a11y::Accessible;
use crate::{
    chat::state::{ChatEvent, ChatMode},
    ui::theme::{Theme, monospace_font, theme},
    workspace::navigation::Route,
};

const FLOATING_MAX_WIDTH: f32 = 650.;
const CTA_MAX_WIDTH: f32 = 640.;
const RIGHT_PANEL_MIN_WIDTH: f32 = 320.;
const REVEAL: Duration = Duration::from_millis(70);

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

pub(super) fn icon(path: &'static str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_none()
        .text_color(color)
}

pub(super) fn spinner(id: impl Into<gpui::ElementId>, size: f32, color: Hsla) -> impl IntoElement {
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
    // On the empty tab shipping's inline CTA wrapper adds a ~6px line-box gap below its
    // `bottom-3` offset; note pages use a block wrapper without it.
    let lift = if matches!(route, Route::Empty) {
        px(6.)
    } else {
        px(0.)
    };
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
                    .pb(px(12.) + lift)
                    .child(
                        div()
                            .id("chat-cta")
                            .a11y_button(if matches!(route, Route::Task(_)) {
                                "Work on this task"
                            } else {
                                "Ask anything"
                            })
                            .w_full()
                            .max_w(px(CTA_MAX_WIDTH))
                            .h(px(40.))
                            .flex()
                            .items_center()
                            .px_4()
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
                            .child(div().flex_1().min_w_0().truncate().child(
                                if matches!(route, Route::Task(_)) {
                                    "Work on this task"
                                } else {
                                    "Ask anything"
                                },
                            )),
                    )
                    .into_any_element(),
            )
        }
        ChatMode::FloatingOpen => {
            let close = chat.clone();
            let state = chat.read(cx);
            let measured = state.panel_height.clone();
            let height = measured.get();
            // The panel grows upward from its composer; closing is instant.
            let reveal = state
                .revealed_at
                .filter(|at| height > 0. && at.elapsed() < REVEAL)
                .map(|_| state.reveals);
            let panel = div()
                .id("chat-floating-panel")
                .occlude()
                .w_full()
                .max_w(px(FLOATING_MAX_WIDTH))
                .when(reveal.is_none(), |view| view.max_h_full())
                .when(reveal.is_some(), |view| view.h(px(height)).flex_shrink_0())
                .flex()
                .flex_col()
                .min_h_0()
                .overflow_hidden()
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
                .child(chat.clone())
                .child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, (), _, _| measured.set(f32::from(bounds.size.height)),
                    )
                    .absolute()
                    .inset_0(),
                );
            let panel = match reveal {
                Some(generation) => {
                    const START: f32 = 45.;
                    div()
                        .w_full()
                        .max_w(px(FLOATING_MAX_WIDTH))
                        .flex()
                        .flex_col()
                        .justify_end()
                        .overflow_hidden()
                        .child(panel)
                        .with_animation(
                            ("chat-reveal", generation),
                            Animation::new(REVEAL),
                            move |view, delta| view.h(px(START + (height - START) * delta)),
                        )
                        .into_any_element()
                }
                None => panel.into_any_element(),
            };
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
                    .pb(px(7.) + lift)
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        close.update(cx, |chat, cx| {
                            if chat.draft_empty(cx) {
                                chat.event(ChatEvent::Close, window, cx);
                            }
                        })
                    })
                    .child(panel)
                    .into_any_element(),
            )
        }
        _ => None,
    }
}

pub(super) fn at_bottom(scroll: &ScrollHandle) -> bool {
    -f32::from(scroll.offset().y) >= f32::from(scroll.max_offset().y) - 4.
}

/// Shipping's `isContextLengthError`.
fn context_length_error(message: &str) -> bool {
    let message = message.to_lowercase();
    (message.contains("n_keep") && message.contains("n_ctx"))
        || (message.contains("context") && message.contains("exceeds"))
        || message.contains("context length")
        || message.contains("context size")
}

impl ChatView {
    /// The background behind messages: the floating panel's, or the card in the right panel.
    pub(super) fn surface(&self, window: &Window) -> Hsla {
        let colors = theme(window);
        if self.mode == ChatMode::RightPanelOpen {
            colors.card
        } else if dark(window) {
            rgb(0x202020).into()
        } else {
            rgb(0xf4f4f5).into()
        }
    }

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
            // Shipping's toolbar labels.
            .a11y_button(match id {
                "chat-new" => "New chat",
                "chat-float" => "Float chat",
                "chat-close" => "Close chat",
                _ => "Open in right panel",
            })
            .group(id)
            .size(px(size))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
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
            .a11y_button("Chat history")
            .aria_expanded(open)
            .group("chat-history")
            .relative()
            .h(px(button))
            .ml(px(-8.))
            .px(px(10.))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
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
                .a11y_menu_check(group.title.to_string(), active)
                .w_full()
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(10.))
                .py(px(6.))
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
                                .font_weight(crate::ui::theme::MEDIUM)
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
            .a11y_group(gpui::Role::Menu, "Recent chats")
            .occlude()
            .w(px(288.))
            .p(px(2.))
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
                                        .id("chat-history-empty")
                                        .a11y_text("No recent chats")
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

    /// A reply's visible parts in order: its text segments and the edit cards between them.
    fn visible_parts(
        &self,
        entry: &Entry,
        style: &markdown::Style,
        colors: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut segment = 0;
        let mut parts = Vec::new();
        for part in &entry.message.parts {
            match part {
                Display::Text(_) => {
                    if let Some(blocks) = entry.blocks.get(segment) {
                        let key = if segment == 0 {
                            format!("chat-md-{}", entry.message.id)
                        } else {
                            format!("chat-md-{}-{segment}", entry.message.id)
                        };
                        parts.push(markdown::render(blocks, style, &key));
                    }
                    segment += 1;
                }
                Display::Tool(tool) if tool.edit() && !entry.message.user => {
                    parts.push(self.edit_card(tool, style, colors, window, cx));
                }
                _ => {}
            }
        }
        parts
    }

    fn message(
        &self,
        entry: &Entry,
        last_assistant: bool,
        style: &markdown::Style,
        colors: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let message = &entry.message;
        let user = message.user;
        let group = SharedString::from(format!("chat-message-{}", message.id));
        let copied = self.copied.as_deref() == Some(message.id.as_str());
        let (id, text) = (message.id.clone(), message.text());
        let body = self.visible_parts(entry, style, colors, window, cx);
        div()
            .id(SharedString::from(format!("chat-message-{}", message.id)))
            .role(gpui::Role::ListItem)
            .aria_label(if user { "You" } else { "Anarlog AI" })
            .aria_description(text.clone())
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
                                view.bg(rgb(0xdbeafe))
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
                                    window,
                                    cx,
                                ))
                            })
                            .children(body),
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
                                        .a11y_button("Copy message")
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
                                            .a11y_button("Regenerate message")
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
        match self.model_ready(cx)? {
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
                                    .id("chat-setup-title")
                                    .a11y_heading("Anarlog AI", 2)
                                    .text_sm()
                                    .line_height(px(20.))
                                    .font_weight(crate::ui::theme::MEDIUM)
                                    .text_color(colors.foreground)
                                    .child("Anarlog AI"),
                            )
                            .child(
                                div()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .border_1()
                                    .border_color(rgb(0xbae6fd))
                                    .bg(rgb(0xe0f2fe))
                                    .text_color(rgb(0x0c4a6e))
                                    .text_size(px(10.))
                                    .line_height(px(15.))
                                    .font_weight(crate::ui::theme::MEDIUM)
                                    .child("Beta"),
                            ),
                    )
                    .child(
                        div()
                            .id("chat-setup-description")
                            .a11y_text("Hi, I'm Anarlog AI. Set up a language model and I'll be ready to help.")
                            .mb_2()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(colors.muted_foreground)
                            .child("Hi, I'm Anarlog AI. Set up a language model and I'll be ready to help."),
                    )
                    .child(
                        div()
                            .id("chat-open-ai-settings")
                            .a11y_button("Open AI Settings")
                            .flex()
                            .flex_none()
                            .self_start()
                            .items_center()
                            .gap(px(6.))
                            .px_3()
                            .py(px(6.))
                            .border_1()
                            .border_color(colors.primary)
                            .bg(colors.primary)
                            .text_color(colors.primary_foreground)
                            .text_xs()
                            .line_height(px(16.))
                            .font_weight(crate::ui::theme::MEDIUM)
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
                // Shipping's `hasContext`: the current note or a mention in the draft.
                let context = self.context.is_some()
                    || !self.dropped.is_empty()
                    || !self.composer.read(cx).context_refs().is_empty();
                let suggestions: Vec<(&'static str, &'static str, &'static str)> = if self
                    .live_meeting(cx)
                {
                    vec![
                        (
                            "Recap the last 5 minutes.",
                            "chat/HistoryIcon.svg",
                            "Recap what was discussed in the last 5 minutes of this meeting. Use the transcript timestamps, where the latest one is now.",
                        ),
                        (
                            "Suggest what to ask next.",
                            "chat/LightbulbIcon.svg",
                            "Based on the conversation so far, suggest a few questions I could ask next.",
                        ),
                        (
                            "List action items so far.",
                            "chat/ListChecksIcon.svg",
                            "What action items have come up so far in this meeting?",
                        ),
                    ]
                } else {
                    let mut suggestions = Vec::new();
                    if self.summary_shown(cx) {
                        suggestions.extend([
                            (
                                "Make the summary shorter.",
                                "chat/ArrowShrinkIcon.svg",
                                "Make the current summary noticeably shorter than it is now. Keep every decision, owner, deadline, and open question; cut repetition and secondary detail.",
                            ),
                            (
                                "Make the summary longer.",
                                "chat/ArrowExpandIcon.svg",
                                "Make the current summary more detailed than it is now. Add supporting context, rationale, and examples from the transcript without inventing anything.",
                            ),
                        ]);
                    }
                    suggestions.extend([
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
                    ]);
                    suggestions
                };
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .pb_1()
                        .children(suggestions.into_iter().map(|(label, path, prompt)| {
                            div()
                                .id(label)
                                .a11y_button(label)
                                .group(label)
                                .w_full()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .py_2()
                                .pr_3()
                                .cursor_pointer()
                                .text_sm()
                                .line_height(px(20.))
                                .text_color(colors.muted_foreground)
                                .hover(|style| style.bg(colors.muted.opacity(0.55)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let refs = this.pending_refs(cx);
                                    this.submit(prompt.to_owned(), refs, cx)
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
            chat: cx.weak_entity(),
            menu: self.markdown.menu.clone(),
            copied: self.markdown.copied.clone(),
            selection: self.markdown.selection.clone(),
            texts: self.markdown.texts.clone(),
            streaming: false,
        };
        let last_assistant = self.entries.iter().rposition(|entry| !entry.message.user);
        let busy = self.busy();
        let streaming = busy
            && !(self.stream_text.trim().is_empty()
                && self.stream_reasoning.trim().is_empty()
                && self.stream_tools.is_empty());
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
                        window,
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
                            .children(
                                self.activity(
                                    "streaming",
                                    &std::iter::once(Display::Reasoning(
                                        self.stream_reasoning.clone(),
                                    ))
                                    .chain(self.stream_tools.iter().cloned().map(Display::Tool))
                                    .collect::<Vec<_>>(),
                                    // A waiting edit means the model is idle until the review.
                                    !self
                                        .stream_tools
                                        .iter()
                                        .any(|tool| tool.edit() && tool.running()),
                                    &colors,
                                    window,
                                    cx,
                                ),
                            )
                            .children(
                                self.stream_tools
                                    .iter()
                                    .filter(|tool| tool.edit())
                                    .map(|tool| self.edit_card(tool, &style, &colors, window, cx))
                                    .collect::<Vec<_>>(),
                            )
                            .child(markdown::render(
                                &self.stream_blocks,
                                &markdown::Style {
                                    streaming: true,
                                    ..style.clone()
                                },
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
                            .child("Thinking...")
                            .id("chat-thinking-status")
                            .a11y_status("Thinking...", false),
                    ),
                );
            }
            if let Phase::Failed(error) = &self.phase {
                content = content.child(
                    div().flex().py_2().child(
                        div()
                            .id("chat-error")
                            .a11y_status(error.clone(), true)
                            .group("chat-error")
                            .relative()
                            .max_w_full()
                            .border_1()
                            .border_color(rgb(0xfecaca))
                            .bg(rgb(0xfef2f2))
                            .px_3()
                            .py_1()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(rgb(0xdc2626))
                            .child(SharedString::from(error.clone()))
                            .when(context_length_error(error), |view| {
                                view.child(
                                    div()
                                        .id("chat-context-length-help")
                                        .a11y_link("Learn how to fix this")
                                        .mt_2()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .text_xs()
                                        .line_height(px(16.))
                                        .text_color(rgb(0xb91c1c))
                                        .hover(|style| style.text_color(rgb(0x7f1d1d)))
                                        .underline()
                                        .cursor_pointer()
                                        .on_click(|_, _, cx| {
                                            let base = std::env::var("VITE_WEB_APP_URL")
                                                .unwrap_or_else(|_| "https://anarlog.so".into());
                                            cx.open_url(&format!(
                                                "{}/docs/faq/local-llm-setup#context-length-error",
                                                base.trim_end_matches('/')
                                            ));
                                        })
                                        .child(icon(
                                            "chat/ExternalLinkIcon.svg",
                                            12.,
                                            rgb(0xb91c1c).into(),
                                        ))
                                        .child("Learn how to fix this"),
                                )
                            })
                            .child(
                                div()
                                    .id("chat-retry")
                                    .a11y_button("Retry")
                                    .absolute()
                                    .top(px(-4.))
                                    .right(px(-4.))
                                    .p_1()
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
                    .a11y_group(gpui::Role::List, "Messages")
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
                                .a11y_button("Go to recent")
                                .h(px(32.))
                                .px_3()
                                .flex()
                                .items_center()
                                .gap_1()
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
                    .children(self.queue.iter().map(|queued| {
                        let (id, text) = (queued.id, &queued.text);
                        div()
                            .id(SharedString::from(format!("chat-queued-{id}")))
                            .a11y_text(text.clone())
                            .group("chat-queued")
                            .min_h(px(28.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py_1()
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
                                    .a11y_button(format!("Remove queued message: {text}"))
                                    .size(px(24.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
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

    fn composer_surface(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let colors = theme(window);
        let right = self.mode == ChatMode::RightPanelOpen;
        let has_content = !self.draft_empty(cx);
        let busy = self.busy();
        let show_send = right || busy || has_content;
        let mic = div()
            .id("chat-voice")
            .size(px(28.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|style| style.bg(colors.muted))
            .a11y_button("Start voice input")
            .on_click(cx.listener(|this, _, _, cx| this.start_voice(cx)))
            .child(icon("chat/Mic01Icon.svg", 17., colors.muted_foreground));
        let send = div()
            .id("chat-send")
            .a11y_button("Send message")
            .a11y_disabled(!has_content)
            .size(px(28.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .border_1()
            .when(!has_content, |view| view.border_color(colors.border))
            .when(has_content, |view| {
                view.bg(colors.primary)
                    .border_color(rgb(0x57534e))
                    .cursor_pointer()
                    .hover(|style| style.bg(colors.primary.opacity(0.9)))
                    .active(|style| style.bg(colors.primary.opacity(0.8)))
            })
            .on_click(cx.listener(|this, _, _, cx| this.submit_draft(cx)))
            .child(icon(
                "chat/ArrowUp02Icon.svg",
                15.,
                if has_content {
                    colors.primary_foreground
                } else {
                    colors.muted_foreground.opacity(0.6)
                },
            ));
        let stop = div()
            .id("chat-stop")
            .a11y_button("Stop response")
            .size(px(28.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|style| style.bg(colors.accent))
            .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
            .child(icon("chat/SquareIcon.svg", 14., colors.foreground));
        let voice = self.voice != VoicePhase::Idle;
        let actions = if voice {
            self.voice_status(stop, send, show_send && !busy, &colors, cx)
        } else {
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap_1()
                .when(!busy, |view| view.child(mic))
                .when(busy, |view| view.child(stop))
                .when(!busy && show_send, |view| view.child(send))
        };
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
        // The floating draft keeps clear of the controls drawn over its right edge.
        let max_height = if right {
            f32::from(window.viewport_size().height) * 0.4
        } else {
            144.
        };
        let padding = if right || voice {
            0.
        } else if has_content && !busy {
            74.
        } else {
            42.
        };
        self.composer.update(cx, |composer, _| {
            composer.max_height = max_height;
            composer.padding_right = padding;
            composer.paragraph_gap = if right { 4. } else { 0. };
        });
        let surface = if right {
            div()
                .flex()
                .flex_col()
                .border_1()
                .border_color(colors.border)
                .bg(colors.card)
                .text_color(colors.card_foreground)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .px_2()
                        .pt_3()
                        .pb_2()
                        .child(
                            div()
                                .mb_1()
                                .min_h_0()
                                .text_sm()
                                .line_height(px(20.))
                                .child(self.composer.clone()),
                        )
                        .child(div().flex().justify_end().child(actions)),
                )
        } else {
            div()
                .flex()
                .when(voice, |view| view.items_stretch().py_2())
                .when(!voice, |view| view.items_center().py(px(3.)))
                .min_h(px(40.))
                .max_h(px(160.))
                .overflow_hidden()
                .pl_4()
                .pr(px(6.))
                .border_1()
                .border_color(colors.border.opacity(0.7))
                .bg(if dark(window) {
                    colors.card
                } else {
                    gpui::white()
                })
                .text_color(colors.card_foreground)
                .child(
                    div()
                        .relative()
                        .flex()
                        .when(voice, |view| view.flex_col().items_stretch())
                        .when(!voice, |view| view.items_center())
                        .w_full()
                        .min_w_0()
                        .min_h(px(30.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .min_h(px(20.))
                                .text_sm()
                                .line_height(px(20.))
                                .child(self.composer.clone()),
                        )
                        .child(if voice {
                            actions.mt_2()
                        } else {
                            div().absolute().right_0().bottom(px(2.)).child(actions)
                        }),
                )
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

    /// Shipping's `VoiceStatus`: the live waveform and elapsed time (or the starting and
    /// transcribing spinner), the stop-recording button, then stop-response or send.
    fn voice_status(
        &self,
        stop: Stateful<Div>,
        send: Stateful<Div>,
        show_send: bool,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let recording = match self.voice {
            VoicePhase::Recording(started) => Some(started),
            _ => None,
        };
        let busy = self.busy();
        let bars = div()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(px(3.))
            .overflow_hidden()
            .map(|view| match recording {
                Some(_) => view.children((0..voice::WAVEFORM.len()).map(|index| {
                    div()
                        .w(px(1.))
                        .flex_shrink_0()
                        .bg(colors.muted_foreground.opacity(0.55))
                        .h(px(voice::bar_height(index, 0.)))
                        .with_animation(
                            ("chat-voice-bar", index),
                            Animation::new(Duration::from_millis(700)).repeat(),
                            move |bar, delta| bar.h(px(voice::bar_height(index, delta))),
                        )
                })),
                None => view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.muted_foreground)
                        .child(spinner(
                            "chat-voice-processing",
                            14.,
                            colors.muted_foreground,
                        ))
                        .child(if self.voice == VoicePhase::Starting {
                            "Starting…"
                        } else {
                            "Transcribing…"
                        }),
                ),
            });
        let stop_voice = div()
            .id("chat-voice-stop")
            .a11y_button(if recording.is_some() {
                "Stop listening"
            } else if self.voice == VoicePhase::Starting {
                "Starting voice input"
            } else {
                "Transcribing voice input"
            })
            .a11y_disabled(recording.is_none())
            .size(px(28.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .bg(colors.muted)
            .text_color(colors.foreground)
            .when(recording.is_none(), |view| view.opacity(0.6))
            .when(recording.is_some(), |view| {
                view.cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.stop_voice(cx)))
            })
            .child(match recording {
                Some(_) => icon("chat/SquareIcon.svg", 12., colors.foreground).into_any_element(),
                None => {
                    spinner("chat-voice-stop-spinner", 14., colors.foreground).into_any_element()
                }
            });
        div()
            .flex()
            .w_full()
            .min_h(px(28.))
            .items_center()
            .gap_2()
            .child(bars)
            .children(recording.map(|started| {
                div()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(colors.muted_foreground)
                    .child(VoicePhase::elapsed(started))
            }))
            .child(stop_voice)
            .when(busy, |view| view.child(stop))
            .when(!busy && show_send, |view| view.child(send))
    }

    /// Shipping's compact app toast, bottom right of the window.
    fn toast_card(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (tone, message, _) = self.toast?;
        let colors = theme(window);
        let viewport = window.viewport_size();
        let tint: Hsla = match tone {
            voice::Tone::Warning => rgb(0xd97706).into(),
            voice::Tone::Error => colors.destructive,
        };
        Some(
            deferred(
                anchored()
                    .position(point(viewport.width - px(16.), viewport.height - px(16.)))
                    .anchor(gpui::Anchor::BottomRight)
                    .child(
                        div()
                            .id("chat-toast")
                            .a11y_status(message, true)
                            .occlude()
                            .w(px(320.).min(viewport.width - px(32.)))
                            .min_h(px(44.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .px(px(10.))
                            .py(px(6.))
                            .border_1()
                            .border_color(colors.border.opacity(0.7))
                            .bg(colors.popover)
                            .text_color(colors.popover_foreground)
                            .shadow_md()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toast = None;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .size(px(32.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon("chat/AlertCircleIcon.svg", 16., tint)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(13.))
                                    .line_height(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(message),
                            ),
                    ),
            )
            .with_priority(3)
            .into_any_element(),
        )
    }

    /// Streamdown's fullscreen table: the whole window, its controls and a close button.
    fn fullscreen_table(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (id, header, rows) = self.markdown.fullscreen.as_ref()?;
        let colors = theme(window);
        let style = markdown::Style {
            colors,
            mono: monospace_font(cx),
            chat: cx.weak_entity(),
            menu: self.markdown.menu.clone(),
            copied: self.markdown.copied.clone(),
            selection: self.markdown.selection.clone(),
            texts: self.markdown.texts.clone(),
            streaming: false,
        };
        let viewport = window.viewport_size();
        Some(
            deferred(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("chat-table-fullscreen")
                        .a11y_dialog("Table")
                        .occlude()
                        .w(viewport.width)
                        .h(viewport.height)
                        .flex()
                        .flex_col()
                        .bg(colors.background)
                        .text_color(colors.foreground)
                        .text_sm()
                        .line_height(px(20.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap_1()
                                .p_4()
                                .child(markdown::table_controls(id, header, rows, &style))
                                .child(
                                    div()
                                        .id("chat-table-fullscreen-close")
                                        .a11y_button("Close")
                                        .group("chat-table-fullscreen-close")
                                        .p_1()
                                        .cursor_pointer()
                                        .hover(|style| style.bg(colors.muted))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.markdown.fullscreen = None;
                                            this.markdown.menu = None;
                                            cx.notify();
                                        }))
                                        .child(
                                            icon("chat/XIcon.svg", 20., colors.muted_foreground)
                                                .group_hover(
                                                    "chat-table-fullscreen-close",
                                                    |style| style.text_color(colors.foreground),
                                                ),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .id("chat-table-fullscreen-body")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .px_4()
                                .pb_4()
                                .child(
                                    markdown::table(header, rows, &style, id)
                                        .w_full()
                                        .border_1()
                                        .border_color(colors.border),
                                ),
                        ),
                ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let right = self.mode == ChatMode::RightPanelOpen;
        // Text elements register again as they paint this frame.
        self.markdown.texts.borrow_mut().clear();
        if self.focus_composer && self.model_ready(cx) == Some(true) {
            self.focus_composer = false;
            gpui::Focusable::focus_handle(self.composer.read(cx), cx).focus(window, cx);
        }
        div()
            .id("chat")
            .a11y_group(gpui::Role::Complementary, "Chat")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .min_h_0()
            .overflow_hidden()
            .when(right, |view| view.size_full())
            .when(!right, |view| view.max_h_full())
            .text_color(colors.card_foreground)
            .on_drop(
                cx.listener(|this, drag: &crate::editor::drop::SessionDrag, _, cx| {
                    this.add_dropped_session(drag.id.clone(), cx)
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    if this.markdown.press(event.position) {
                        this.focus.focus(window, cx);
                    }
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    this.markdown.selecting = false;
                } else if this.markdown.drag(event.position) {
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.markdown.selecting = false),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let modifiers = event.keystroke.modifiers;
                let key = event.keystroke.key.as_str();
                if modifiers.secondary(cx) && key == "c" {
                    if let Some(text) = this.markdown.selected_text() {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                        cx.stop_propagation();
                    }
                    return;
                }
                if key != "escape" || modifiers.secondary(cx) || modifiers.alt || modifiers.shift {
                    return;
                }
                if this.markdown.fullscreen.take().is_some() {
                    this.markdown.menu = None;
                    cx.notify();
                } else {
                    this.event(ChatEvent::Close, window, cx);
                }
                cx.stop_propagation();
            }))
            .child(self.toolbar(window, cx))
            .child(self.body(window, cx))
            .when(self.model_ready(cx) == Some(true), |view| {
                view.children(self.queue_list(&colors, cx))
                    .child(self.composer_surface(window, cx))
            })
            .children(self.fullscreen_table(window, cx))
            .children(self.toast_card(window, cx))
    }
}
