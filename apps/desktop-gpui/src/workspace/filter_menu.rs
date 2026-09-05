//! `apps/desktop/src/sidebar/note-filter-menu.tsx`: the funnel button's
//! Grouping (Date / Folder) and Ordering (Newest / Oldest) submenus, backed
//! by `useSidebarNotes`.

use gpui::{AnyElement, Context, Window, point, px};

use super::Workspace;
use super::menu::{Align, Entry, MenuSpec, Trailing};
use crate::timeline::{GroupBy, SortOrder};

impl Workspace {
    pub(crate) fn toggle_filter_menu(&mut self, cx: &mut Context<Self>) {
        self.filter_menu_open = !self.filter_menu_open;
        self.filter_submenu = None;
        cx.notify();
    }

    fn close_filter_menu(&mut self, cx: &mut Context<Self>) {
        if self.filter_menu_open {
            self.filter_menu_open = false;
            self.filter_submenu = None;
            cx.notify();
        }
    }

    fn hover_filter_submenu(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        // Hovering a plain item closes the submenu; hovering a trigger opens it.
        if self.filter_submenu != index {
            self.filter_submenu = index;
            cx.notify();
        }
    }

    /// `isDefaultView`: the trigger only highlights once grouping or
    /// ordering leaves the defaults.
    pub(crate) fn is_default_notes_view(&self) -> bool {
        self.group_by == GroupBy::Date && self.sort_order == SortOrder::Newest
    }

    pub(crate) fn set_group_by(&mut self, group_by: GroupBy, cx: &mut Context<Self>) {
        if self.group_by != group_by {
            self.group_by = group_by;
            self.rebuild_timeline(cx);
        }
    }

    pub(crate) fn set_sort_order(&mut self, order: SortOrder, cx: &mut Context<Self>) {
        if self.sort_order != order {
            self.sort_order = order;
            self.rebuild_timeline(cx);
        }
    }

    pub(super) fn render_filter_menu(
        &self,
        _window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if !self.filter_menu_open {
            return None;
        }
        let group_by = self.group_by;
        let sort_order = self.sort_order;
        let grouping_label = if group_by == GroupBy::Folder {
            "Folder"
        } else {
            "Date"
        };
        let ordering_label = if sort_order == SortOrder::Oldest {
            "Oldest"
        } else {
            "Newest"
        };

        let spec = MenuSpec {
            id: "filter-menu",
            width: 224.0,
            open_sub: self.filter_submenu,
            on_hover_sub: Self::hover_filter_submenu,
            on_close: Self::close_filter_menu,
            entries: vec![
                Entry::Item {
                    icon: None,
                    dim_icon: false,
                    label: "Grouping".into(),
                    trailing: Trailing::Text(grouping_label.into()),
                    destructive: false,
                    on_select: None,
                    submenu: Some(vec![
                        Entry::Item {
                            icon: Some("calendar-blank"),
                            dim_icon: true,
                            label: "Date".into(),
                            trailing: Trailing::Check(group_by == GroupBy::Date),
                            destructive: false,
                            on_select: Some(Box::new(|this, _, cx| {
                                this.set_group_by(GroupBy::Date, cx)
                            })),
                            submenu: None,
                        },
                        Entry::Item {
                            icon: Some("folder"),
                            dim_icon: true,
                            label: "Folder".into(),
                            trailing: Trailing::Check(group_by == GroupBy::Folder),
                            destructive: false,
                            on_select: Some(Box::new(|this, _, cx| {
                                this.set_group_by(GroupBy::Folder, cx)
                            })),
                            submenu: None,
                        },
                    ]),
                },
                Entry::Item {
                    icon: None,
                    dim_icon: false,
                    label: "Ordering".into(),
                    trailing: Trailing::Text(ordering_label.into()),
                    destructive: false,
                    on_select: None,
                    submenu: Some(vec![
                        Entry::Item {
                            icon: Some("sort-descending"),
                            dim_icon: true,
                            label: "Newest".into(),
                            trailing: Trailing::Check(sort_order == SortOrder::Newest),
                            destructive: false,
                            on_select: Some(Box::new(|this, _, cx| {
                                this.set_sort_order(SortOrder::Newest, cx)
                            })),
                            submenu: None,
                        },
                        Entry::Item {
                            icon: Some("sort-ascending"),
                            dim_icon: true,
                            label: "Oldest".into(),
                            trailing: Trailing::Check(sort_order == SortOrder::Oldest),
                            destructive: false,
                            on_select: Some(Box::new(|this, _, cx| {
                                this.set_sort_order(SortOrder::Oldest, cx)
                            })),
                            submenu: None,
                        },
                    ]),
                },
            ],
        };

        // The trigger is the third `size-7` header button: shell `pl-1` +
        // header `pl-2` + spacer + search + new note = 96px; `align="start"`,
        // `sideOffset` 4 under its bottom edge (title bar 40 + `pt-[9px]` + 28).
        let position = point(px(96.0), px(40.0 + 9.0 + 28.0 + 4.0));
        Some(self.render_app_menu(spec, position, Align::Start, cx))
    }
}
