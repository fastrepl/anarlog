//! `apps/desktop/src/main/windows-title-bar.tsx`: the app draws its own title
//! bar on Windows and Linux (sidebar toggle, menu bar, window controls) and
//! lets the whole strip drag the window.

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, MouseButton, MouseDownEvent, Pixels, Point,
    ResizeEdge, SharedString, Size, Stateful, Window, anchored, deferred, div, point, prelude::*,
    px,
};

use super::{Menu, Workspace};
use crate::actions::shortcut_label;
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon, window_control};

pub fn uses_windows_style_title_bar() -> bool {
    cfg!(any(target_os = "windows", target_os = "linux"))
}

/// `usesTitleBarSidebarActions`: on Windows the sidebar's Search / New note /
/// Filter buttons sit in the title bar beside the toggle while the sidebar is
/// expanded (#7395); Linux keeps them in the sidebar's chrome row.
pub fn uses_title_bar_sidebar_actions() -> bool {
    cfg!(target_os = "windows")
}

const TITLE_BAR_HEIGHT: f32 = 40.0;
/// `pl-2` + the `size-7` toggle + `ml-2` on the menubar.
const MENUBAR_LEFT: f32 = 8.0 + 28.0 + 8.0;
/// `px-2.5` on each menu trigger.
const MENU_TRIGGER_PADDING: f32 = 10.0;

/// Which window edge a point within `threshold` of the border belongs to.
pub fn resize_edge(
    position: Point<Pixels>,
    threshold: Pixels,
    size: Size<Pixels>,
) -> Option<ResizeEdge> {
    let top = position.y < threshold;
    let bottom = position.y > size.height - threshold;
    let left = position.x < threshold;
    let right = position.x > size.width - threshold;
    Some(match (top, bottom, left, right) {
        (true, _, true, _) => ResizeEdge::TopLeft,
        (true, _, _, true) => ResizeEdge::TopRight,
        (_, true, true, _) => ResizeEdge::BottomLeft,
        (_, true, _, true) => ResizeEdge::BottomRight,
        (true, ..) => ResizeEdge::Top,
        (_, true, ..) => ResizeEdge::Bottom,
        (_, _, true, _) => ResizeEdge::Left,
        (_, _, _, true) => ResizeEdge::Right,
        _ => return None,
    })
}

const MENUS: [(Menu, &str); 4] = [
    (Menu::File, "File"),
    (Menu::Edit, "Edit"),
    (Menu::View, "View"),
    (Menu::Help, "Help"),
];

/// One `DropdownMenuItem` (or a separator) with the action it dispatches.
enum MenuEntry {
    Item {
        label: SharedString,
        shortcut: Option<&'static str>,
        action: Option<Box<dyn gpui::Action>>,
        url: Option<&'static str>,
    },
    Separator,
}

fn item(
    label: impl Into<SharedString>,
    shortcut: Option<&'static str>,
    action: impl gpui::Action,
) -> MenuEntry {
    MenuEntry::Item {
        label: label.into(),
        shortcut,
        action: Some(Box::new(action)),
        url: None,
    }
}

fn link(label: impl Into<SharedString>, url: &'static str) -> MenuEntry {
    MenuEntry::Item {
        label: label.into(),
        shortcut: None,
        action: None,
        url: Some(url),
    }
}

impl Workspace {
    fn menu_entries(&self, menu: Menu) -> Vec<MenuEntry> {
        use crate::actions::*;
        match menu {
            Menu::File => vec![
                item("New Note", Some("Ctrl+N"), NewNote),
                item("Settings", Some("Ctrl+,"), OpenSettings),
                MenuEntry::Separator,
                item("Close", Some("Alt+F4"), CloseWindow),
            ],
            Menu::Edit => vec![
                item("Undo", Some("Ctrl+Z"), Undo),
                item("Redo", Some("Ctrl+Y"), Redo),
                MenuEntry::Separator,
                item("Cut", Some("Ctrl+X"), Cut),
                item("Copy", Some("Ctrl+C"), Copy),
                item("Paste", Some("Ctrl+V"), Paste),
                item("Select All", Some("Ctrl+A"), SelectAll),
            ],
            Menu::View => vec![
                item(
                    if self.sidebar_expanded {
                        "Hide Sidebar"
                    } else {
                        "Show Sidebar"
                    },
                    Some("Ctrl+\\"),
                    ToggleSidebar,
                ),
                item("Full Screen", Some("F11"), ToggleFullscreen),
            ],
            Menu::Help => vec![
                link("Documentation", "https://docs.anarlog.so"),
                MenuEntry::Separator,
                link("Report a Bug", "https://anarlog.so/discord"),
                link("Suggest a Feature", "https://anarlog.so/discord"),
            ],
        }
    }

    /// Left edge of each menu trigger, from the measured label widths.
    fn menu_trigger_x(&self, menu: Menu, window: &Window) -> Pixels {
        let mut x = px(MENUBAR_LEFT);
        for (candidate, label) in MENUS {
            if candidate == menu {
                break;
            }
            x += self.measure_text(label, px(14.0), window) + px(MENU_TRIGGER_PADDING * 2.0);
        }
        x
    }

    pub(super) fn render_title_bar(&self, window: &Window, cx: &Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let maximized = window.is_maximized();

        div()
            .id("title-bar")
            .flex()
            .h(px(TITLE_BAR_HEIGHT))
            .flex_shrink_0()
            .bg(theme.background)
            // `data-tauri-drag-region`: press-and-drag anywhere that is not a
            // control moves the window; a double click toggles maximize.
            .on_mouse_down(
                MouseButton::Left,
                |_: &MouseDownEvent, window: &mut Window, _: &mut App| window.start_window_move(),
            )
            .on_click(|event: &ClickEvent, window: &mut Window, _: &mut App| {
                if event.click_count() == 2 {
                    window.zoom_window();
                }
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .pl_2()
                    .child(
                        self.tracked_chrome_button("toggle-sidebar", cx)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.toggle_sidebar(cx);
                            }))
                            .child(icon(
                                if self.sidebar_expanded {
                                    "sidebar-left"
                                } else {
                                    "view-sidebar-left"
                                },
                                px(16.0),
                                self.chrome_icon_color("toggle-sidebar"),
                            )),
                    )
                    .when(
                        uses_title_bar_sidebar_actions()
                            && self.shows_sidebar_timeline_chrome()
                            && self.sidebar_expanded,
                        |row| row.child(self.render_sidebar_note_actions(cx)),
                    )
                    .child(
                        div().ml_2().flex().h_full().items_center().children(
                            MENUS
                                .iter()
                                .map(|(menu, label)| self.render_menu_trigger(*menu, label, cx)),
                        ),
                    )
                    .child(div().min_w_4().flex_1()),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        window_control("minimize", theme, false)
                            .on_click(|_, window, _| window.minimize_window())
                            .child(div().h(px(1.0)).w(px(10.0)).bg(theme.foreground)),
                    )
                    .child(
                        window_control("maximize", theme, false)
                            .on_click(|_, window, _| window.zoom_window())
                            .child(if maximized {
                                div()
                                    .relative()
                                    .size(px(12.0))
                                    .child(
                                        div()
                                            .absolute()
                                            .top(px(2.0))
                                            .right_0()
                                            .size(px(8.0))
                                            .border_1()
                                            .border_color(theme.foreground),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom(px(2.0))
                                            .left_0()
                                            .size(px(8.0))
                                            .bg(theme.background)
                                            .border_1()
                                            .border_color(theme.foreground),
                                    )
                            } else {
                                div()
                                    .size(px(10.0))
                                    .border_1()
                                    .border_color(theme.foreground)
                            }),
                    )
                    .child(
                        window_control("close", theme, true)
                            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                this.set_hovered("close", *hovered, cx);
                            }))
                            .on_click(cx.listener(|this, _, window, _| this.close_window(window)))
                            .child(icon(
                                "close-x",
                                px(12.0),
                                if self.hovered == Some("close") {
                                    theme.white
                                } else {
                                    theme.foreground
                                },
                            )),
                    ),
            )
    }

    /// `TitleBarMenu` trigger: `h-7 rounded-md px-2.5 text-sm
    /// text-muted-foreground`, accent while hovered or open. With a menu open,
    /// hovering another trigger switches to it (menubar behaviour).
    fn render_menu_trigger(
        &self,
        menu: Menu,
        label: &'static str,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let open = self.open_menu == Some(menu);
        div()
            .id(SharedString::from(format!("menu-{label}")))
            .h(px(28.0))
            .px(px(MENU_TRIGGER_PADDING))
            .flex()
            .items_center()
            .rounded_md()
            .tw_text_sm()
            .text_color(if open {
                theme.foreground
            } else {
                theme.muted_foreground
            })
            .when(open, |trigger| trigger.bg(theme.accent))
            .cursor_pointer()
            .hover(move |style| style.bg(theme.accent).text_color(theme.foreground))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                if *hovered && this.open_menu.is_some() {
                    this.set_menu(Some(menu), window, cx);
                }
            }))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                let next = if this.open_menu == Some(menu) {
                    None
                } else {
                    Some(menu)
                };
                this.set_menu(next, window, cx);
            }))
            .child(label)
    }

    /// `DropdownMenuContent className="w-56 rounded-lg"` anchored under the
    /// open trigger with `sideOffset={1}`.
    pub(super) fn render_open_menu(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let menu = self.open_menu?;
        let x = self.menu_trigger_x(menu, window);
        let entries = self.menu_entries(menu);
        // Radix anchors `sideOffset={1}` below the 28px trigger centred in the bar.
        let trigger_bottom = (TITLE_BAR_HEIGHT - 28.0) / 2.0 + 28.0;
        let target = (menu == Menu::Edit)
            .then(|| self.menu_edit_target.clone())
            .flatten();
        Some(self.render_title_bar_menu_panel(
            "menu-panel",
            entries,
            point(x, px(trigger_bottom + 1.0)),
            target,
            1,
            cx,
        ))
    }

    /// The webview's editing context menu (Cut / Copy / Paste / Select All)
    /// at the pointer, acting on the editable that was right-clicked.
    pub(super) fn render_edit_context_menu(&self, cx: &Context<Self>) -> Option<AnyElement> {
        use crate::actions::*;
        let (position, target) = self.edit_context_menu.clone()?;
        let entries = vec![
            item("Cut", Some("Ctrl+X"), Cut),
            item("Copy", Some("Ctrl+C"), Copy),
            item("Paste", Some("Ctrl+V"), Paste),
            MenuEntry::Separator,
            item("Select All", Some("Ctrl+A"), SelectAll),
        ];
        // The OS menu draws over everything, the format toolbar included.
        Some(self.render_title_bar_menu_panel(
            "edit-context-menu",
            entries,
            position,
            Some(target),
            5,
            cx,
        ))
    }

    /// One `w-56 rounded-lg` menu panel at `position`; an item refocuses
    /// `edit_target` before its action, like `runEditCommand`.
    fn render_title_bar_menu_panel(
        &self,
        id: &'static str,
        entries: Vec<MenuEntry>,
        position: Point<Pixels>,
        edit_target: Option<gpui::FocusHandle>,
        priority: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let edit_target = std::rc::Rc::new(edit_target);
        // The title bar's Radix menus take the keyboard; the webview's native
        // editing context menu leaves the field focused.
        let keyboard = id != "edit-context-menu";
        let mut runtime_items: Vec<Option<super::menu::RuntimeItem>> =
            Vec::with_capacity(entries.len());

        let items = entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                MenuEntry::Separator => {
                    runtime_items.push(None);
                    div()
                        .mx(px(-4.0))
                        .my_1()
                        .h(px(1.0))
                        .bg(theme.accent)
                        .into_any_element()
                }
                MenuEntry::Item {
                    label,
                    shortcut,
                    action,
                    url,
                } => {
                    let action = action.map(std::rc::Rc::new);
                    let run: super::menu::Run = {
                        let edit_target = edit_target.clone();
                        let action = action.clone();
                        std::rc::Rc::new(
                            move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                                this.set_menu(None, window, cx);
                                this.menu_edit_target = None;
                                if this.edit_context_menu.take().is_some() {
                                    cx.notify();
                                }
                                if let Some(url) = url {
                                    crate::opener::open_url(url);
                                }
                                if let Some(action) = &action {
                                    // `runEditCommand`: the item runs on the
                                    // element that had focus when the menu opened.
                                    if let Some(target) = edit_target.as_ref() {
                                        window.focus(target);
                                    }
                                    window.dispatch_action(action.boxed_clone(), cx);
                                }
                            },
                        )
                    };
                    runtime_items.push(Some(super::menu::RuntimeItem {
                        label: label.clone(),
                        enabled: true,
                        submenu: false,
                        activate: run.clone(),
                    }));
                    let highlighted = keyboard && self.menu_keyboard.is_highlighted(id, index);
                    div()
                        .id(("menu-item", index))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py(px(6.0))
                        .rounded(px(14.0))
                        .cursor_default()
                        .when(!keyboard, |item| {
                            item.hover(move |style| style.bg(theme.accent))
                        })
                        .when(highlighted, |item| item.bg(theme.accent))
                        .when(keyboard, |item| {
                            item.on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                this.menu_hover(id, false, index, *hovered, cx);
                            }))
                        })
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            run(this, window, cx);
                        }))
                        .child(label)
                        .when_some(shortcut, |item, shortcut| {
                            item.child(
                                div()
                                    .ml_auto()
                                    .tw_text_xs()
                                    .text_color(alpha(theme.foreground, 0.6))
                                    .child(SharedString::from(shortcut_label(shortcut))),
                            )
                        })
                        .into_any_element()
                }
            })
            .collect::<Vec<_>>();
        if keyboard {
            *self.menu_runtime.borrow_mut() = Some(super::menu::MenuRuntime {
                id,
                items: runtime_items,
                sub_items: None,
                set_sub: None,
            });
        }

        let panel = div()
            .id(id)
            .occlude()
            .w(px(224.0))
            .flex()
            .flex_col()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.card)
            .shadow_md()
            .p_1()
            .tw_text_sm()
            .text_color(theme.foreground)
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, window, cx| {
                this.set_menu(None, window, cx);
                if this.edit_context_menu.take().is_some() {
                    cx.notify();
                }
            }));
        let panel = if keyboard {
            self.menu_keyboard_host(panel, cx)
        } else {
            panel
        };
        let panel = panel.children(items);

        deferred(
            anchored()
                .position(position)
                .snap_to_window_with_margin(px(8.0))
                .child(panel),
        )
        .with_priority(priority)
        .into_any_element()
    }
}
