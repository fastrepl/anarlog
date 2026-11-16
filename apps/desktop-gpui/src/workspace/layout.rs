use gpui::{Context, MouseButton, Render, Window, canvas, div, prelude::*, px};

use super::{
    navigation::Route,
    shell::{Navigate, WorkspaceAction, WorkspaceView},
};
use crate::ui::theme::{RADIUS, monospace_font, system_font, theme};

impl WorkspaceView {
    fn empty_action(
        label: &'static str,
        keys: &'static str,
        action: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let colors = theme(window);
        let modifier = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Ctrl"
        };
        div()
            .id(label)
            .flex()
            .items_center()
            .justify_between()
            .gap_8()
            .px_4()
            .py_2()
            .rounded(px(RADIUS))
            .text_sm()
            .line_height(px(20.))
            .text_color(colors.foreground)
            .cursor_pointer()
            .hover(|style| style.bg(colors.accent))
            .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
            .child(label)
            .child(
                div()
                    .h(px(20.))
                    .min_w(px(20.))
                    .px_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.muted)
                    .text_color(colors.muted_foreground)
                    .font_family(monospace_font(cx))
                    .text_xs()
                    .line_height(px(12.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(format!("{modifier} {keys}")),
            )
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let route = self
            .navigation
            .current()
            .map(|tab| tab.route.clone())
            .unwrap_or(Route::Empty);
        let layout_route = route.clone();
        let layout_sidebar = self.sidebar.clone();
        let chat_width = crate::chat::reserved_width(&self.chat, window, cx);
        let entity = cx.entity().downgrade();
        let catalog = self
            .catalogs
            .iter()
            .find(|(catalog, _)| Some(*catalog) == self.active_catalog)
            .map(|(_, view)| view.clone());
        let resource_nav = match route {
            Route::Templates => Some(
                self.templates
                    .update(cx, |templates, cx| templates.render_nav(window, cx)),
            ),
            Route::Folders | Route::Folder(_) => Some(
                self.folders
                    .update(cx, |folders, cx| folders.render_nav(window, cx)),
            ),
            _ => None,
        };
        let contacts = self.contacts.clone().filter(|_| route.contacts());
        let sidebar = if route == Route::Calendar {
            div()
                .w(px(self.sidebar.width()))
                .h_full()
                .flex_shrink_0()
                .child(self.calendar_sidebar.clone())
        } else if let Some(nav) = resource_nav {
            // Shipping's custom sidebars end in a 4px gutter (`pr-1`); the row gap supplies it.
            div()
                .w(px(self.sidebar.width() - 4.))
                .h_full()
                .flex_shrink_0()
                .child(nav)
        } else if let Some(contacts) = &contacts {
            let list = contacts.read(cx).list();
            let width = self.sidebar.width();
            list.update(cx, |list, cx| list.set_width(width, cx));
            div().w(px(width)).h_full().flex_shrink_0().child(list)
        } else {
            div()
                .w(px(self.sidebar.width()))
                .h_full()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap_1()
                .map(|sidebar| {
                    if catalog.is_some() {
                        // Shipping custom sidebars leave the chrome row free instead of the timeline header.
                        sidebar.pt(px(44.))
                    } else {
                        sidebar.child(self.sidebar_header(window, cx))
                    }
                })
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .when_some(catalog.clone(), |view, catalog| view.child(catalog))
                        .when(catalog.is_none(), |view| view.child(self.library.clone())),
                )
        };
        let surface = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.library
                        .update(cx, |library, cx| library.clear_selection(cx));
                }),
            )
            .overflow_hidden()
            .bg(colors.card)
            .map(|surface| {
                if self.sidebar.expanded {
                    // Shipping's "left" surface chrome: left border only, both left corners round.
                    surface
                        .border_l_1()
                        .border_color(colors.border)
                        .rounded_l(px(12.))
                } else {
                    surface.rounded_t(px(12.))
                }
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .when_some(
                        self.route_content
                            .as_ref()
                            .filter(|(target, _)| target == &route)
                            .map(|(_, view)| view.clone()),
                        |view, content| view.child(content),
                    )
                    .when(self.route_content.is_none(), |view| match &route {
                        _ if contacts.is_some() => view.child(contacts.clone().unwrap()),
                        Route::Session(_) => view.child(self.note.clone()),
                        Route::Calendar => view.child(self.calendar.clone()),
                        Route::Templates => view.child(self.templates.clone()),
                        Route::Folders | Route::Folder(_) => view.child(self.folders.clone()),
                        Route::Empty => view.child(
                            div()
                                .size_full()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .min_w(px(280.))
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(Self::empty_action(
                                            "New Note",
                                            "N",
                                            |this, cx| this.create(false, cx),
                                            window,
                                            cx,
                                        ))
                                        .child(Self::empty_action(
                                            "Start Recording",
                                            "⇧ N",
                                            |this, cx| this.create(true, cx),
                                            window,
                                            cx,
                                        ))
                                        .child(div().my_1().h(px(1.)).bg(colors.accent))
                                        .child(Self::empty_action(
                                            "Settings",
                                            ",",
                                            |this, cx| {
                                                this.navigate(
                                                    Navigate::Open(Route::settings("app"), false),
                                                    cx,
                                                )
                                            },
                                            window,
                                            cx,
                                        )),
                                ),
                        ),
                        _ if catalog.is_some() => {
                            view.child(catalog.as_ref().unwrap().read(cx).detail())
                        }
                        _ => view.child(div().p_6().text_sm().child(format!(
                            "{} content has not been connected to the workspace.",
                            route.label()
                        ))),
                    }),
            )
            .children(crate::chat::overlay(&self.chat, &route, window, cx))
            .when(!self.message.is_empty(), |view| {
                view.child(
                    div()
                        .px_3()
                        .py_1()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(self.message.clone()),
                )
            });
        div()
            .id("workspace")
            .size_full()
            .relative()
            .flex()
            .when(self.sidebar.expanded, |view| view.pl(px(4.)))
            .gap(px(4.))
            .overflow_hidden()
            .font_family(system_font(cx))
            .text_color(colors.foreground)
            .bg(colors.background)
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::shortcuts))
            .on_key_down(cx.listener(Self::dismiss_overlay))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    this.resizing = false;
                } else if this.resizing {
                    this.sidebar.resize(f32::from(event.position.x) - 4.);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.resizing = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.resizing = false),
            )
            .when(self.sidebar.expanded, |view| {
                view.child(sidebar).when(self.sidebar.can_resize(), |view| view.child(
                    div()
                        .id("sidebar-resizer")
                        .absolute()
                        .left(px(self.sidebar.width() + 4.))
                        .w(px(4.))
                        .h_full()
                        .cursor_col_resize()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.resizing = true;
                                cx.stop_propagation();
                            }),
                        ),
                ))
            })
            .child(surface)
            .children(crate::chat::right_panel(&self.chat, window, cx))
            .when(!self.sidebar.expanded, |view| {
                view.child(self.collapsed_sidebar_chrome(window, cx))
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        // The chat right panel takes its width from the note surface.
                        let width = f32::from(bounds.size.width) - chat_width;
                        let mut sidebar = layout_sidebar.clone();
                        sidebar.layout(width, &layout_route);
                        if sidebar != layout_sidebar {
                            window.defer(cx, move |window, cx| {
                                let _ = entity.update(cx, |this, cx| {
                                    let route = this.navigation.current()
                                        .map(|tab| tab.route.clone()).unwrap_or(Route::Empty);
                                    let old_width = this.sidebar.width();
                                    let was_expanded = this.sidebar.expanded;
                                    if let Some(expansion) = this.sidebar.layout(width, &route) {
                                        let mut size = window.viewport_size();
                                        size.width += px(expansion);
                                        window.resize(size);
                                    }
                                    if was_expanded != this.sidebar.expanded {
                                        this.resizing = false;
                                        cx.emit(WorkspaceAction::SidebarChanged);
                                    }
                                    if old_width != this.sidebar.width() || was_expanded != this.sidebar.expanded {
                                        cx.notify();
                                    }
                                });
                            });
                        }
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .when_some(self.note_operation.clone(),|view,(ids,moving)| view.child(
                div().absolute().inset_0().occlude().bg(gpui::rgba(0x00000055)).flex().items_center().justify_center().child(
                    div().w(px(420.)).p_6().rounded(px(12.)).bg(colors.card).border_1().border_color(colors.border).flex().flex_col().gap_4()
                        .child(div().text_lg().child(format!("{} {} notes?",if moving {"Move"} else {"Delete"},ids.len())))
                        .when(moving,|view| view.child(self.move_target.clone()))
                        .when(!moving,|view| view.child(div().text_sm().child("The selected notes and their related records will be removed from the library.")))
                        .child(div().flex().justify_end().gap_3()
                            .child(div().id("cancel-note-operation").cursor_pointer().px_3().py_2().child("Cancel").on_click(cx.listener(|this,_,_,cx| this.close_note_operation(cx))))
                            .child(div().id("confirm-note-operation").cursor_pointer().px_3().py_2().rounded(px(6.)).bg(colors.accent).child(if self.mutation_busy {"Saving…"} else if moving {"Move notes"} else {"Delete notes"}).on_click(cx.listener(|this,_,_,cx| this.submit_note_operation(cx))))
                        )
                )
            ))
            // Deferred so the dialog's backdrop also covers application toasts, like shipping's.
            .when(self.picker_open, |view| {
                view.child(gpui::deferred(self.picker.clone()))
            })
    }
}
