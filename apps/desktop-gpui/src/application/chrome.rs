use gpui::{
    AnyElement, Context, FocusHandle, KeyDownEvent, Keystroke, MouseButton, Window, anchored,
    deferred, div, point, prelude::*, px, svg,
};

use super::ApplicationView;
use crate::{
    ui::theme::{system_font, theme},
    workspace::navigation::Route,
};

#[derive(Default)]
pub(super) struct Chrome {
    menu: Option<usize>,
    item: usize,
    return_focus: Option<FocusHandle>,
}

#[derive(Clone, Copy)]
enum Command {
    NewNote,
    Settings,
    Close,
    Edit(&'static str),
    Sidebar,
    Fullscreen,
    Link(&'static str),
}

struct Entry {
    label: &'static str,
    shortcut: &'static str,
    command: Command,
}

const MENUS: [(&str, &[Option<Entry>]); 4] = [
    (
        "File",
        &[
            Some(Entry {
                label: "New Note",
                shortcut: "Ctrl+N",
                command: Command::NewNote,
            }),
            Some(Entry {
                label: "Settings",
                shortcut: "Ctrl+,",
                command: Command::Settings,
            }),
            None,
            Some(Entry {
                label: "Close",
                shortcut: "Alt+F4",
                command: Command::Close,
            }),
        ],
    ),
    (
        "Edit",
        &[
            Some(Entry {
                label: "Undo",
                shortcut: "Ctrl+Z",
                command: Command::Edit("ctrl-z"),
            }),
            Some(Entry {
                label: "Redo",
                shortcut: "Ctrl+Y",
                command: Command::Edit("ctrl-y"),
            }),
            None,
            Some(Entry {
                label: "Cut",
                shortcut: "Ctrl+X",
                command: Command::Edit("ctrl-x"),
            }),
            Some(Entry {
                label: "Copy",
                shortcut: "Ctrl+C",
                command: Command::Edit("ctrl-c"),
            }),
            Some(Entry {
                label: "Paste",
                shortcut: "Ctrl+V",
                command: Command::Edit("ctrl-v"),
            }),
            Some(Entry {
                label: "Select All",
                shortcut: "Ctrl+A",
                command: Command::Edit("ctrl-a"),
            }),
        ],
    ),
    (
        "View",
        &[
            Some(Entry {
                label: "Hide Sidebar",
                shortcut: "Ctrl+\\",
                command: Command::Sidebar,
            }),
            Some(Entry {
                label: "Full Screen",
                shortcut: "F11",
                command: Command::Fullscreen,
            }),
        ],
    ),
    (
        "Help",
        &[
            Some(Entry {
                label: "Documentation",
                shortcut: "",
                command: Command::Link("https://docs.anarlog.so"),
            }),
            None,
            Some(Entry {
                label: "Report a Bug",
                shortcut: "",
                command: Command::Link("https://anarlog.so/discord"),
            }),
            Some(Entry {
                label: "Suggest a Feature",
                shortcut: "",
                command: Command::Link("https://anarlog.so/discord"),
            }),
        ],
    ),
];

impl ApplicationView {
    fn open_menu(&mut self, index: usize, window: &Window, cx: &mut Context<Self>) {
        if self.closing || self.writers_paused {
            return;
        }
        if self.chrome.menu.is_none() {
            self.chrome.return_focus = window.focused(cx);
        }
        self.chrome.menu = Some(index);
        self.chrome.item = 0;
        cx.notify();
    }

    fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.chrome.menu = None;
        if let Some(focus) = self.chrome.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    fn menu_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        self.close_menu(window, cx);
        if self.closing || self.writers_paused {
            return;
        }
        match command {
            Command::NewNote => {
                if self.product.is_some() {
                    self.leave_product(Route::Empty, cx);
                }
                if self.product.is_none() {
                    self.workspace.update(cx, |view, cx| view.create(false, cx));
                }
            }
            Command::Settings => self.leave_product(Route::settings("app"), cx),
            Command::Close => self.request_quit(cx),
            Command::Edit(key) => window.defer(cx, move |window, cx| {
                window.dispatch_keystroke(Keystroke::parse(key).expect("menu shortcut"), cx);
            }),
            Command::Sidebar => self
                .workspace
                .update(cx, |view, cx| view.toggle_sidebar(cx)),
            Command::Fullscreen => window.toggle_fullscreen(),
            Command::Link(url) => cx.open_url(url),
        }
    }

    pub(super) fn chrome_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if cfg!(target_os = "macos") {
            return false;
        }
        let key = &event.keystroke;
        if key.key == "f11" {
            window.toggle_fullscreen();
            return true;
        }
        if key.modifiers.alt && key.key == "f4" {
            self.request_quit(cx);
            return true;
        }
        if key.modifiers.alt
            && let Some(index) = ["f", "e", "v", "h"]
                .iter()
                .position(|letter| *letter == key.key)
        {
            self.open_menu(index, window, cx);
            return true;
        }
        let Some(menu) = self.chrome.menu else {
            return false;
        };
        if key.modifiers.control || key.modifiers.platform {
            self.close_menu(window, cx);
            return false;
        }
        let entries = MENUS[menu].1;
        match key.key.as_str() {
            "escape" | "tab" => self.close_menu(window, cx),
            "left" => self.open_menu((menu + MENUS.len() - 1) % MENUS.len(), window, cx),
            "right" => self.open_menu((menu + 1) % MENUS.len(), window, cx),
            "up" | "down" => {
                let step = if key.key == "down" {
                    1
                } else {
                    entries.len() - 1
                };
                loop {
                    self.chrome.item = (self.chrome.item + step) % entries.len();
                    if entries[self.chrome.item].is_some() {
                        break;
                    }
                }
                cx.notify();
            }
            "enter" | "space" => {
                if let Some(entry) = &entries[self.chrome.item] {
                    self.menu_command(entry.command, window, cx);
                }
            }
            _ => {}
        }
        true
    }

    pub(super) fn title_bar(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        div()
            .h(px(40.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .font_family(system_font(cx))
            .text_sm()
            .line_height(px(20.))
            .text_color(colors.muted_foreground)
            .child(
                div()
                    .ml_2()
                    .size(px(28.))
                    .id("title-sidebar")
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .cursor_pointer()
                    .hover(|style| style.bg(colors.accent).text_color(colors.foreground))
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.workspace
                            .update(cx, |view, cx| view.toggle_sidebar(cx));
                    }))
                    .child(
                        svg()
                            .path("workspace/SidebarLeftIcon.svg")
                            .size(px(16.))
                            .text_color(colors.muted_foreground),
                    ),
            )
            .child(div().ml_2().flex().children(MENUS.iter().enumerate().map(
                |(index, (label, _))| {
                    div()
                        .id(("application-menu", index))
                        .w(px(46.))
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .when(self.chrome.menu == Some(index), |view| {
                            view.bg(colors.accent).text_color(colors.foreground)
                        })
                        .hover(|style| style.bg(colors.accent).text_color(colors.foreground))
                        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.chrome.menu == Some(index) {
                                this.close_menu(window, cx);
                            } else {
                                this.open_menu(index, window, cx);
                            }
                        }))
                        .on_hover(cx.listener(move |this, hovered, window, cx| {
                            if *hovered
                                && this.chrome.menu.is_some()
                                && this.chrome.menu != Some(index)
                            {
                                this.open_menu(index, window, cx);
                            }
                        }))
                        .child(*label)
                },
            )))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_w(px(16.))
                    .id("window-drag")
                    .on_mouse_down(MouseButton::Left, |event, window, _| {
                        if event.click_count == 2 {
                            window.zoom_window();
                        } else {
                            window.start_window_move();
                        }
                    }),
            )
            .children(
                ["Minimize", "Maximize", "Close"]
                    .into_iter()
                    .enumerate()
                    .map(|(index, label)| {
                        div()
                            .id(label)
                            .w(px(46.))
                            .h_full()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(colors.foreground)
                            .hover(move |style| {
                                if index == 2 {
                                    style
                                        .bg(gpui::rgb(0xc42b1c))
                                        .text_color(gpui::rgb(0xffffff))
                                } else {
                                    style.bg(colors.accent)
                                }
                            })
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.prevent_default()
                            })
                            .on_click(cx.listener(move |this, _, window, cx| match index {
                                0 => window.minimize_window(),
                                1 => window.zoom_window(),
                                _ => this.request_quit(cx),
                            }))
                            .child(match index {
                                0 => div()
                                    .w(px(10.))
                                    .h(px(1.))
                                    .bg(colors.foreground)
                                    .into_any_element(),
                                1 => div()
                                    .size(px(10.))
                                    .border_1()
                                    .border_color(colors.foreground)
                                    .when(window.is_maximized(), |view| view.shadow_sm())
                                    .into_any_element(),
                                _ => div().text_lg().child("×").into_any_element(),
                            })
                    }),
            )
    }

    pub(super) fn title_menu(&self, window: &Window, cx: &Context<Self>) -> Option<AnyElement> {
        let menu = self.chrome.menu?;
        let colors = theme(window);
        Some(
            deferred(
                anchored()
                    .position(point(px(44. + menu as f32 * 46.), px(40.)))
                    .snap_to_window()
                    .child(
                        div()
                            .id("application-menu-popup")
                            .w(px(224.))
                            .p_1()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.card)
                            .shadow_lg()
                            .font_family(system_font(cx))
                            .text_sm()
                            .line_height(px(20.))
                            .occlude()
                            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                                window.prevent_default();
                                cx.stop_propagation();
                            })
                            .on_mouse_down_out(cx.listener(
                                |this, event: &gpui::MouseDownEvent, window, cx| {
                                    let in_menu_bar = event.position.y < px(40.)
                                        && (px(44.)..px(44. + MENUS.len() as f32 * 46.))
                                            .contains(&event.position.x);
                                    if !in_menu_bar {
                                        this.close_menu(window, cx);
                                    }
                                },
                            ))
                            .children(MENUS[menu].1.iter().enumerate().map(|(index, entry)| {
                                let Some(entry) = entry else {
                                    return div()
                                        .h(px(1.))
                                        .my_1()
                                        .bg(colors.border)
                                        .into_any_element();
                                };
                                let command = entry.command;
                                let label = if matches!(command, Command::Sidebar)
                                    && !self.workspace.read(cx).sidebar_expanded()
                                {
                                    "Show Sidebar"
                                } else {
                                    entry.label
                                };
                                div()
                                    .id(("application-menu-entry", index))
                                    .px_2()
                                    .py(px(6.))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .when(index == self.chrome.item, |view| view.bg(colors.accent))
                                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                                        if *hovered {
                                            this.chrome.item = index;
                                            cx.notify();
                                        }
                                    }))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.menu_command(command, window, cx)
                                    }))
                                    .child(label)
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(colors.muted_foreground)
                                            .child(entry.shortcut),
                                    )
                                    .into_any_element()
                            })),
                    ),
            )
            .into_any_element(),
        )
    }
}
