//! Sidebar chrome ported from `main/sidebar-timeline-chrome.tsx` and
//! `sidebar/note-filter-menu.tsx`: the timeline header row (toggle, search, new note and the
//! sort menu) and the toggle that stays visible while the sidebar is collapsed.

use gpui::{
    AnyElement, Context, KeyDownEvent, Window, anchored, deferred, div, prelude::*, px, svg,
};

use super::{
    shell::WorkspaceView,
    timeline::{Grouping, Ordering, View},
};
use crate::ui::theme::{RADIUS, Theme, theme};

/// The macOS traffic lights overlay the sidebar; keep the shipping app's gutter.
pub(super) const CHROME_GUTTER: f32 = if cfg!(target_os = "macos") { 76. } else { 8. };
const MENU_WIDTH: f32 = 224.;
const SUBMENU_WIDTH: f32 = 176.;
const MENU_ITEM_HEIGHT: f32 = 32.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SortMenu {
    /// Highlighted row of the open menu (Grouping or Ordering); none until pointed at.
    row: Option<usize>,
    /// Whether that row's submenu is open.
    submenu: bool,
    /// The keyboard-highlighted submenu option; pointer-opened submenus start without one.
    option: Option<usize>,
}

impl SortMenu {
    fn options(row: usize) -> [(&'static str, &'static str); 2] {
        if row == 0 {
            [
                ("Date", "workspace/Calendar04Icon.svg"),
                ("Folder", "workspace/Folder01Icon.svg"),
            ]
        } else {
            [
                ("Newest", "workspace/SortDescendingIcon.svg"),
                ("Oldest", "workspace/SortByUp01Icon.svg"),
            ]
        }
    }

    fn selected(row: usize, view: View) -> usize {
        if row == 0 {
            usize::from(view.grouping == Grouping::Folder)
        } else {
            usize::from(view.ordering == Ordering::Oldest)
        }
    }

    fn apply(row: usize, option: usize, view: View) -> View {
        let mut view = view;
        if row == 0 {
            view.grouping = if option == 0 {
                Grouping::Date
            } else {
                Grouping::Folder
            };
        } else {
            view.ordering = if option == 0 {
                Ordering::Newest
            } else {
                Ordering::Oldest
            };
        }
        view
    }

    /// Keyboard navigation; returns the view to apply when an option is chosen, or `None` with
    /// `self` set to `None` when the menu should close.
    fn key(menu: &mut Option<Self>, key: &str, view: View) -> Option<View> {
        let state = menu.as_mut()?;
        let submenu = state.submenu.then_some(state.option);
        match (key, submenu, state.row) {
            ("escape", None, _) | ("tab", _, _) => *menu = None,
            ("escape" | "left", Some(_), _) => {
                state.submenu = false;
                state.option = None;
            }
            ("up" | "down", None, row) => state.row = Some(row.map_or(0, |row| 1 - row)),
            ("up" | "down", Some(option), _) => {
                state.option = Some(option.map_or(0, |option| 1 - option))
            }
            ("right" | "enter" | "space", None, Some(row)) => {
                state.submenu = true;
                state.option = Some(Self::selected(row, view));
            }
            ("enter" | "space", Some(Some(option)), Some(row)) => {
                let view = Self::apply(row, option, view);
                *menu = None;
                return Some(view);
            }
            _ => {}
        }
        None
    }
}

fn chrome_button(
    id: &'static str,
    icon: &'static str,
    icon_size: f32,
    active: bool,
    colors: Theme,
) -> gpui::Stateful<gpui::Div> {
    let color = if active {
        colors.foreground
    } else {
        colors.muted_foreground
    };
    div()
        .id(id)
        .group(id)
        .relative()
        .size(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(RADIUS))
        .cursor_pointer()
        .hover(|style| style.bg(colors.accent))
        .child(
            svg()
                .path(icon)
                .size(px(icon_size))
                .text_color(color)
                .group_hover(id, |style| style.text_color(colors.foreground)),
        )
}

fn menu_item(
    id: impl Into<gpui::ElementId>,
    highlighted: bool,
    colors: Theme,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(MENU_ITEM_HEIGHT))
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .rounded(px(14.))
        .text_sm()
        .line_height(px(20.))
        .text_color(colors.popover_foreground)
        .cursor_pointer()
        .when(highlighted, |view| view.bg(colors.accent))
        .hover(|style| style.bg(colors.accent))
}

/// The shipping `AppFloatingPanel` inside its chrome ring.
pub(super) fn floating_panel(width: f32, colors: Theme) -> gpui::Div {
    div()
        .w(px(width))
        .p(px(2.))
        .rounded(px(22.))
        .border_1()
        .border_color(colors.app_floating_border)
        .bg(colors.app_floating_chrome)
        .shadow_lg()
}

pub(super) fn floating_panel_body(colors: Theme) -> gpui::Div {
    div()
        .p(px(6.))
        .rounded(px(20.))
        .border_1()
        .border_color(colors.app_floating_border)
        .bg(colors.app_floating_panel)
        .flex()
        .flex_col()
}

impl WorkspaceView {
    pub(super) fn sidebar_header(&self, window: &Window, cx: &mut Context<Self>) -> gpui::Div {
        let colors = theme(window);
        let view = self.library.read(cx).view();
        let filtered = view != View::default();
        div()
            .min_h(px(36.))
            .flex_shrink_0()
            .pt(px(9.))
            .pl(px(CHROME_GUTTER))
            .pr_1()
            .flex()
            .items_start()
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        chrome_button(
                            "hide-sidebar",
                            "workspace/SidebarLeftIcon.svg",
                            16.,
                            false,
                            colors,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                    )
                    .child(
                        chrome_button("open-picker", "Search01Icon.svg", 15., false, colors)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.show_picker(window, cx)),
                            ),
                    )
                    .child(
                        chrome_button("new-note", "NoteEditIcon.svg", 15., false, colors)
                            .on_click(cx.listener(|this, _, _, cx| this.create(false, cx))),
                    )
                    .child(
                        chrome_button(
                            "sort-notes",
                            if filtered {
                                "workspace/FilterIconFilled.svg"
                            } else {
                                "workspace/FilterIcon.svg"
                            },
                            15.,
                            filtered,
                            colors,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sort_menu = match this.sort_menu {
                                Some(_) => None,
                                None => Some(SortMenu::default()),
                            };
                            cx.notify();
                        }))
                        .when_some(self.sort_menu, |button, menu| {
                            button.child(
                                div().absolute().top(px(32.)).left_0().child(deferred(
                                    anchored()
                                        .snap_to_window()
                                        .child(self.sort_menu(menu, view, colors, cx)),
                                )),
                            )
                        }),
                    ),
            )
    }

    fn sort_menu(
        &self,
        menu: SortMenu,
        view: View,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = |index: usize, label: &'static str| {
            let value = Self::sort_value(index, view);
            let open = move |this: &mut Self, cx: &mut Context<Self>| {
                if let Some(menu) = this.sort_menu.as_mut()
                    && (menu.row, menu.submenu) != (Some(index), true)
                {
                    *menu = SortMenu {
                        row: Some(index),
                        submenu: true,
                        option: None,
                    };
                    cx.notify();
                }
            };
            menu_item(("sort-row", index), menu.row == Some(index), colors)
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    if *hovered {
                        open(this, cx);
                    }
                }))
                .on_click(cx.listener(move |this, _, _, cx| open(this, cx)))
                .child(div().flex_1().child(label))
                .child(div().text_color(colors.muted_foreground).child(value))
                .child(
                    svg()
                        .path("workspace/ArrowRight01Icon.svg")
                        .size(px(16.))
                        .text_color(colors.popover_foreground),
                )
        };
        // Both panels sit in one row so a click in the submenu is not a click outside the menu.
        div()
            .id("sort-menu")
            .flex()
            .items_start()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.sort_menu = None;
                cx.notify();
            }))
            .child(
                floating_panel(MENU_WIDTH, colors).child(
                    floating_panel_body(colors)
                        .child(row(0, "Grouping"))
                        .child(row(1, "Ordering")),
                ),
            )
            .when_some(menu.row.filter(|_| menu.submenu), |root, row| {
                let selected = SortMenu::selected(row, view);
                root.child(
                    div()
                        .ml(px(-9.))
                        .mt(px(9. + MENU_ITEM_HEIGHT * row as f32))
                        .child(floating_panel(SUBMENU_WIDTH, colors).child(
                            floating_panel_body(colors).children(
                                SortMenu::options(row).into_iter().enumerate().map(
                                    |(option, (label, icon))| {
                                        menu_item(
                                            ("sort-option", option),
                                            menu.option == Some(option),
                                            colors,
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_sort(row, option, cx);
                                        }))
                                        .child(
                                            svg()
                                                .path(icon)
                                                .size(px(16.))
                                                .flex_shrink_0()
                                                .text_color(colors.popover_foreground.opacity(0.7)),
                                        )
                                        .child(div().flex_1().child(label))
                                        .when(
                                            option == selected,
                                            |item| {
                                                item.child(
                                                    svg()
                                                        .path("workspace/CheckIcon.svg")
                                                        .size(px(16.))
                                                        .text_color(colors.popover_foreground),
                                                )
                                            },
                                        )
                                    },
                                ),
                            ),
                        )),
                )
            })
            .into_any_element()
    }

    fn sort_value(row: usize, view: View) -> &'static str {
        SortMenu::options(row)[SortMenu::selected(row, view)].0
    }

    fn choose_sort(&mut self, row: usize, option: usize, cx: &mut Context<Self>) {
        let view = SortMenu::apply(row, option, self.library.read(cx).view());
        self.sort_menu = None;
        self.set_timeline_view(view, cx);
        cx.notify();
    }

    /// Oldest-first orders the whole library, so the order is part of the library query.
    fn set_timeline_view(&mut self, view: View, cx: &mut Context<Self>) {
        self.library
            .update(cx, |library, cx| library.set_view(view, cx));
        let oldest = view.ordering == Ordering::Oldest;
        if self.query.oldest != oldest {
            self.query.oldest = oldest;
            self.reload(cx);
        }
    }

    /// Routes keys to the open sort menu; returns whether the menu consumed them.
    pub(super) fn sort_menu_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if self.sort_menu.is_none() {
            return false;
        }
        let view = self.library.read(cx).view();
        if let Some(view) = SortMenu::key(&mut self.sort_menu, event.keystroke.key.as_str(), view) {
            self.set_timeline_view(view, cx);
        }
        cx.notify();
        true
    }

    /// The sidebar toggle shown over the main surface while the sidebar is collapsed, with the
    /// shipping red dot when a meeting is about to start.
    pub(super) fn collapsed_sidebar_chrome(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let colors = theme(window);
        let upcoming = self.library.read(cx).has_upcoming_meeting();
        div()
            .absolute()
            .top_0()
            .left_1()
            .h(px(48.))
            .pt(px(9.))
            .pl(px(CHROME_GUTTER))
            .child(
                chrome_button(
                    "show-sidebar",
                    "workspace/ViewSidebarLeftIcon.svg",
                    16.,
                    false,
                    colors,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx)))
                .when(upcoming, |button| {
                    button.child(
                        div()
                            .absolute()
                            .top(px(2.))
                            .right(px(2.))
                            .size(px(10.))
                            .rounded_full()
                            .bg(colors.background)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(div().size(px(6.)).rounded_full().bg(gpui::rgb(0xfb2c36))),
                    )
                }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_menu_keys_open_submenus_choose_options_and_close() {
        let view = View::default();
        let mut menu = Some(SortMenu::default());
        assert_eq!(SortMenu::key(&mut menu, "down", view), None);
        assert_eq!(menu.unwrap().row, Some(0));
        SortMenu::key(&mut menu, "down", view);
        assert_eq!(menu.unwrap().row, Some(1));
        SortMenu::key(&mut menu, "right", view);
        assert_eq!(
            menu.unwrap().option,
            Some(0),
            "opens on the current ordering"
        );
        SortMenu::key(&mut menu, "down", view);
        assert_eq!(
            SortMenu::key(&mut menu, "enter", view),
            Some(View {
                grouping: Grouping::Date,
                ordering: Ordering::Oldest,
            })
        );
        assert_eq!(menu, None);

        let mut menu = Some(SortMenu::default());
        SortMenu::key(&mut menu, "enter", view);
        assert!(!menu.unwrap().submenu, "nothing is highlighted yet");
        SortMenu::key(&mut menu, "down", view);
        SortMenu::key(&mut menu, "enter", view);
        assert!(menu.unwrap().submenu);
        SortMenu::key(&mut menu, "left", view);
        assert!(!menu.unwrap().submenu);
        SortMenu::key(&mut menu, "escape", view);
        assert_eq!(menu, None);
    }
}
