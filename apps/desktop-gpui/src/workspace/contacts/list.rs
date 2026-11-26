//! `ContactsNav` from shipping's `sidebar/contacts.tsx`, with `ColumnHeader`, `PersonItem`,
//! `OrganizationItem` and `NewPersonForm`.

use gpui::{
    Context, Entity, KeyDownEvent, MouseButton, MouseDownEvent, Pixels, Point, Render,
    ScrollHandle, SharedString, Subscription, WeakEntity, Window, anchored, deferred, div,
    prelude::*, px, rgb, svg,
};

use super::{
    BLUE_600, BLUE_700, Contacts, ContactsEvent, Dismissal, avatar,
    data::{Human, Item, Organization, Selection, SortOption},
    dropdown, focus_input, icon_button, menu_trigger,
    ops::Kind,
    radio_item, text_sm, text_xs,
};
use crate::ui::a11y::Accessible;
use crate::{
    ui::{
        input::{InputEvent, TextInput},
        theme::{Theme, theme},
    },
    workspace::sidebar::{floating_panel, floating_panel_body},
};

#[derive(Clone)]
struct Menu {
    position: Point<Pixels>,
    kind: Kind,
    id: SharedString,
    pinned: bool,
}

#[derive(Clone)]
struct Dragged {
    index: usize,
    label: SharedString,
}

impl Render for Dragged {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        text_sm(div())
            .px_3()
            .py_2()
            .bg(colors.accent)
            .font_weight(crate::ui::theme::MEDIUM)
            .child(self.label.clone())
    }
}

pub struct ContactsList {
    contacts: WeakEntity<Contacts>,
    search: Entity<TextInput>,
    sort: SortOption,
    sort_open: bool,
    sort_dismissal: Dismissal,
    /// Shipping hides the sort menu while the sidebar is narrower than 220px.
    show_sort: bool,
    new_person: Option<(Entity<TextInput>, Subscription)>,
    menu: Option<Menu>,
    hovered: Option<(Kind, SharedString)>,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl ContactsList {
    pub fn new(contacts: WeakEntity<Contacts>, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            TextInput::new("Search contacts...", cx)
                .inline()
                .label("Search contacts")
        });
        let subscription = cx.observe(&search, |_, _, cx| cx.notify());
        Self {
            contacts,
            search,
            sort: SortOption::default(),
            sort_open: false,
            sort_dismissal: Dismissal::default(),
            show_sort: false,
            new_person: None,
            menu: None,
            hovered: None,
            scroll: ScrollHandle::new(),
            _subscriptions: vec![subscription],
        }
    }

    pub fn set_width(&mut self, width: f32, cx: &mut Context<Self>) {
        // The header is the list's width less the sidebar's 4px right padding.
        let show = width - 4. >= 220.;
        if show != self.show_sort {
            self.show_sort = show;
            cx.notify();
        }
    }

    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        focus_input(&self.search, window, cx);
        self.search.update(cx, |input, cx| {
            input.buffer.anchor = 0;
            input.buffer.cursor = input.buffer.text.len();
            cx.notify();
        });
    }

    fn contacts(
        &self,
        cx: &mut Context<Self>,
        action: impl FnOnce(&mut Contacts, &mut Context<Contacts>),
    ) {
        if let Some(contacts) = self.contacts.upgrade() {
            contacts.update(cx, action);
        }
    }

    fn select(&mut self, selection: Selection, cx: &mut Context<Self>) {
        self.contacts(cx, |contacts, cx| contacts.select(Some(selection), cx));
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.new_person.is_some() {
            return;
        }
        let input = cx.new(|cx| TextInput::new("Add person", cx).inline());
        let subscription = cx.subscribe(&input, |this, input, event, cx| match event {
            InputEvent::Submitted => {
                let name = input.read(cx).buffer.text.trim().to_owned();
                if !name.is_empty() {
                    this.new_person = None;
                    this.contacts(cx, |contacts, cx| contacts.create_person(name, cx));
                }
                cx.notify();
            }
            _ => cx.notify(),
        });
        focus_input(&input, window, cx);
        self.new_person = Some((input, subscription));
        cx.notify();
    }

    fn submit_new_person(&mut self, cx: &mut Context<Self>) {
        let Some((input, _)) = &self.new_person else {
            return;
        };
        let name = input.read(cx).buffer.text.trim().to_owned();
        if !name.is_empty() {
            self.new_person = None;
            self.contacts(cx, |contacts, cx| contacts.create_person(name, cx));
            cx.notify();
        }
    }

    fn reorder(
        &mut self,
        pinned: Vec<(Kind, SharedString)>,
        from: usize,
        to: usize,
        cx: &mut Context<Self>,
    ) {
        if from == to || from >= pinned.len() || to >= pinned.len() {
            return;
        }
        let mut order = pinned;
        let moved = order.remove(from);
        order.insert(to, moved);
        let entries = order
            .into_iter()
            .map(|(kind, id)| (kind, id.to_string()))
            .collect();
        self.contacts(cx, |contacts, cx| {
            contacts.write(super::ops::Write::Reorder(entries), cx)
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        id: SharedString,
        kind: Kind,
        avatar: gpui::Div,
        title: SharedString,
        subtitle: Option<SharedString>,
        pinned: bool,
        read_only: bool,
        active: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let group = SharedString::from(format!("contact-row-{id}"));
        let selection = match kind {
            Kind::Human => Selection::Person(id.clone()),
            Kind::Organization => Selection::Organization(id.clone()),
        };
        let pin_id = id.clone();
        let hover_id = id.clone();
        let text_selector_id = id.clone();
        let pin_selector_id = id.clone();
        let menu_id = id.clone();
        let is_hovered = self.hovered.as_ref() == Some(&(kind, id.clone()));
        let pin_color = if pinned {
            rgb(BLUE_600).into()
        } else {
            colors.muted_foreground.opacity(0.7)
        };
        text_sm(div())
            .id(group.clone())
            .a11y_row(
                match &subtitle {
                    Some(subtitle) => SharedString::from(format!("{title}, {subtitle}")),
                    None => title.clone(),
                },
                active,
            )
            .group(group.clone())
            .w_full()
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .gap_2()
            .overflow_hidden()
            .cursor_pointer()
            .map(|row| {
                if active {
                    row.bg(colors.accent)
                } else {
                    row.hover(move |style| style.bg(colors.accent.opacity(0.5)))
                }
            })
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    let next = Some((kind, hover_id.clone()));
                    if this.hovered != next {
                        this.hovered = next;
                        cx.notify();
                    }
                } else if this.hovered.as_ref() == Some(&(kind, hover_id.clone())) {
                    this.hovered = None;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| this.select(selection.clone(), cx)))
            .when(!read_only, |row| {
                row.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.menu = Some(Menu {
                            position: event.position,
                            kind,
                            id: menu_id.clone(),
                            pinned,
                        });
                        cx.notify();
                    }),
                )
            })
            .child(avatar)
            .child(
                div()
                    .debug_selector(move || format!("contact-text-{text_selector_id}"))
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .font_weight(crate::ui::theme::MEDIUM)
                            .truncate()
                            .child(title),
                    )
                    .when_some(subtitle, |view, subtitle| {
                        view.child(
                            text_xs(div())
                                .truncate()
                                .text_color(colors.muted_foreground)
                                .child(subtitle),
                        )
                    }),
            )
            .when(pinned || is_hovered, |row| {
                row.child(
                    div()
                        .id(SharedString::from(format!("pin-{id}")))
                        .debug_selector(move || format!("contact-pin-{pin_selector_id}"))
                        .a11y_button(match (kind, pinned) {
                            (Kind::Human, true) => "Unpin contact",
                            (Kind::Human, false) => "Pin contact",
                            (Kind::Organization, true) => "Unpin organization",
                            (Kind::Organization, false) => "Pin organization",
                        })
                        .a11y_disabled(read_only)
                        .group(SharedString::from(format!("pin-{id}")))
                        .flex_shrink_0()
                        .p_1()
                        .when(!read_only, |pin| {
                            pin.cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    let id = pin_id.clone();
                                    this.contacts(cx, |contacts, cx| {
                                        contacts.toggle_pin(kind, id, cx)
                                    });
                                }))
                        })
                        .child(
                            svg()
                                .path(if pinned {
                                    "contacts/PinIconBold.svg"
                                } else {
                                    "workspace/PinIcon.svg"
                                })
                                .size(px(14.))
                                .text_color(pin_color)
                                .when(!read_only, |icon| {
                                    icon.group_hover(format!("pin-{id}"), move |style| {
                                        style.text_color(if pinned {
                                            gpui::Hsla::from(rgb(BLUE_700))
                                        } else {
                                            colors.muted_foreground
                                        })
                                    })
                                }),
                        ),
                )
            })
    }

    fn person(
        &self,
        human: &Human,
        read_only: bool,
        active: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let title = if human.name.is_empty() {
            if human.email.is_empty() {
                "Unnamed".into()
            } else {
                human.email.clone()
            }
        } else {
            human.name.clone()
        };
        let subtitle =
            (!human.name.is_empty() && !human.email.is_empty()).then(|| human.email.clone());
        self.row(
            human.id.clone(),
            Kind::Human,
            avatar::person(human.avatar.as_ref(), &human.avatar_name(), 32.),
            title,
            subtitle,
            read_only || human.pinned,
            read_only,
            active,
            colors,
            cx,
        )
    }

    fn organization(
        &self,
        organization: &Organization,
        read_only: bool,
        active: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        self.row(
            organization.id.clone(),
            Kind::Organization,
            avatar::organization(
                organization.avatar.as_ref(),
                &organization.name,
                32.,
                colors.muted,
                colors,
            ),
            organization.name.clone(),
            None,
            read_only || organization.pinned,
            read_only,
            active,
            colors,
            cx,
        )
    }

    fn item(
        &self,
        item: Item,
        active: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        match item {
            Item::Person(human) => self.person(human, false, active, colors, cx),
            Item::Organization(organization) => {
                self.organization(organization, false, active, colors, cx)
            }
        }
    }

    fn header(&self, colors: Theme, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .h(px(48.))
            .flex_shrink_0()
            .flex()
            .items_start()
            .pr_1()
            .pl(crate::zoom::window_controls_gutter(cx))
            .pt(crate::zoom::window_controls_top(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        icon_button(
                            "contacts-back",
                            "workspace/ArrowLeft02Icon.svg",
                            16.,
                            colors,
                        )
                        .a11y_button("Go home")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.contacts(cx, |_, cx| cx.emit(ContactsEvent::Back))
                        })),
                    ),
            )
            .child(
                div()
                    .ml_1()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .when(self.show_sort, |view| {
                        let sort = self.sort;
                        view.child(
                            div()
                                .relative()
                                .child(
                                    menu_trigger(
                                        "contacts-sort",
                                        "contacts/ArrowUpDownIcon.svg",
                                        16.,
                                        self.sort_open,
                                        colors,
                                    )
                                    .a11y_button("Sort options")
                                    .aria_expanded(self.sort_open)
                                    .on_click(cx.listener(
                                        |this, event, _, cx| {
                                            if !this.sort_dismissal.closed_by(event) {
                                                this.sort_open = !this.sort_open;
                                            }
                                            cx.notify();
                                        },
                                    )),
                                )
                                .when(self.sort_open, |view| {
                                    view.child(dropdown(
                                        true,
                                        32.,
                                        floating_panel(128., colors)
                                            .id("contacts-sort-menu")
                                            .a11y_group(gpui::Role::Menu, "Sort options")
                                            .occlude()
                                            .on_mouse_down_out(cx.listener(|this, event, _, cx| {
                                                this.sort_open = false;
                                                this.sort_dismissal.record(event);
                                                cx.notify();
                                            }))
                                            .child(floating_panel_body(colors).children(
                                                SortOption::ALL.into_iter().map(
                                                    |(option, label)| {
                                                        radio_item(
                                                            SharedString::from(format!(
                                                                "sort-{label}"
                                                            )),
                                                            label,
                                                            sort == option,
                                                            true,
                                                            colors,
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.sort = option;
                                                                this.sort_open = false;
                                                                cx.notify();
                                                            },
                                                        ))
                                                    },
                                                ),
                                            )),
                                    ))
                                }),
                        )
                    })
                    .child(
                        icon_button("contacts-add", "workspace/PlusIcon.svg", 16., colors)
                            .a11y_button("Add contact")
                            .on_click(cx.listener(|this, _, window, cx| this.add(window, cx))),
                    ),
            )
    }

    fn search_box(&self, window: &Window, colors: Theme, cx: &mut Context<Self>) -> gpui::Div {
        let focused = gpui::Focusable::focus_handle(self.search.read(cx), cx).is_focused(window);
        let empty = self.search.read(cx).buffer.text.is_empty();
        div().pb_2().child(
            div()
                .h(px(32.))
                .w_full()
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .border_1()
                .border_color(colors.border)
                .bg(if focused { colors.accent } else { colors.muted })
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        this.search
                            .update(cx, |input, cx| input.set_text(String::new(), cx));
                        cx.stop_propagation();
                    }
                }))
                .child(
                    svg()
                        .path("Search01Icon.svg")
                        .size(px(16.))
                        .flex_shrink_0()
                        .text_color(colors.muted_foreground),
                )
                .child(div().flex_1().min_w_0().child(self.search.clone()))
                .when(!empty, |view| {
                    view.child(
                        div()
                            .id("clear-contacts-search")
                            .a11y_button("Clear search")
                            .size(px(16.))
                            .flex_shrink_0()
                            .cursor_pointer()
                            .group("clear-contacts-search")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.search
                                    .update(cx, |input, cx| input.set_text(String::new(), cx));
                            }))
                            .child(
                                svg()
                                    .path("chat/XIcon.svg")
                                    .size(px(16.))
                                    .text_color(colors.muted_foreground)
                                    .group_hover("clear-contacts-search", move |style| {
                                        style.text_color(colors.foreground)
                                    }),
                            ),
                    )
                }),
        )
    }

    fn new_person_form(
        &self,
        window: &Window,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let (input, _) = self.new_person.as_ref()?;
        let focused = gpui::Focusable::focus_handle(input.read(cx), cx).is_focused(window);
        let ready = !input.read(cx).buffer.text.trim().is_empty();
        Some(
            div().px_2().py_2().child(
                div()
                    .h(px(32.))
                    .w_full()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_1()
                    .border_color(colors.border)
                    .bg(if focused {
                        colors.accent
                    } else {
                        colors.accent.opacity(0.5)
                    })
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key == "escape" {
                            this.new_person = None;
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }))
                    .child(div().flex_1().min_w_0().child(input.clone()))
                    .when(ready, |view| {
                        view.child(
                            div()
                                .id("submit-new-person")
                                .a11y_button("Add person")
                                .flex_shrink_0()
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| this.submit_new_person(cx)))
                                .child(
                                    svg()
                                        .path("contacts/ArrowTurnBackwardIcon.svg")
                                        .size(px(16.))
                                        .text_color(colors.muted_foreground),
                                ),
                        )
                    }),
            ),
        )
    }

    fn context_menu(&self, menu: Menu, colors: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let noun = match menu.kind {
            Kind::Human => "Contact",
            Kind::Organization => "Organization",
        };
        let labels = [
            format!("{} {noun}", if menu.pinned { "Unpin" } else { "Pin" }),
            format!("Delete {noun}"),
        ];
        deferred(
            anchored().position(menu.position).snap_to_window().child(
                div()
                    .id("contacts-context-menu")
                    .role(gpui::Role::Menu)
                    .w(px(224.))
                    .p_1()
                    .bg(colors.card)
                    .border_1()
                    .border_color(colors.border)
                    .shadow_lg()
                    .occlude()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.menu = None;
                        cx.notify();
                    }))
                    .children(labels.into_iter().enumerate().map(|(index, label)| {
                        let menu = menu.clone();
                        text_sm(div())
                            .id(("contacts-menu-item", index))
                            .a11y_menu_item(label.clone())
                            .h(px(30.))
                            .px_3()
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .hover(move |style| style.bg(colors.accent))
                            .child(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.menu = None;
                                let (kind, id) = (menu.kind, menu.id.clone());
                                this.contacts(cx, |contacts, cx| {
                                    if index == 0 {
                                        contacts.toggle_pin(kind, id, cx)
                                    } else {
                                        contacts.delete(kind, id, cx)
                                    }
                                });
                                cx.notify();
                            }))
                    })),
            ),
        )
    }
}

impl Render for ContactsList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let Some(contacts) = self.contacts.upgrade() else {
            return div();
        };
        let (data, selected, owner) = {
            let contacts = contacts.read(cx);
            (
                contacts.data.clone(),
                contacts.selected.clone(),
                contacts.owner(),
            )
        };
        let search = self.search.read(cx).buffer.text.clone();
        let searching = !search.trim().is_empty();
        let sections = data.sections(&owner, &search, self.sort);
        let me = data.human(&owner);
        let active = |selection: Selection| selected.as_ref() == Some(&selection);
        let pinned_keys = sections
            .pinned
            .iter()
            .map(|item| match item {
                Item::Person(human) => (Kind::Human, human.id.clone()),
                Item::Organization(organization) => (Kind::Organization, organization.id.clone()),
            })
            .collect::<Vec<_>>();
        let divider = !sections.pinned.is_empty() && !sections.others.is_empty();
        div()
            .size_full()
            .pr_1()
            .flex()
            .flex_col()
            .child(self.header(colors, cx))
            .child(self.search_box(window, colors, cx))
            .child(
                div()
                    .id("contacts-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .when_some(me, |view, me| {
                        let is_active = active(Selection::Person(me.id.clone()));
                        view.child(self.person(me, true, is_active, colors, cx))
                    })
                    .children(self.new_person_form(window, colors, cx))
                    .children(sections.team.iter().map(|organization| {
                        let is_active = active(Selection::Organization(organization.id.clone()));
                        self.organization(organization, true, is_active, colors, cx)
                    }))
                    .children(sections.pinned.iter().enumerate().map(|(index, item)| {
                        let row = self.item(*item, active(item.selection()), colors, cx);
                        if searching {
                            return row;
                        }
                        let label = match item {
                            Item::Person(human) => human.display_name(),
                            Item::Organization(organization) => organization.name.clone(),
                        };
                        let keys = pinned_keys.clone();
                        row.on_drag(Dragged { index, label }, |dragged, _, _, cx| {
                            cx.new(|_| dragged.clone())
                        })
                        .drag_over::<Dragged>(move |style, _, _, _| style.bg(colors.accent))
                        .on_drop(cx.listener(
                            move |this, dragged: &Dragged, _, cx| {
                                this.reorder(keys.clone(), dragged.index, index, cx)
                            },
                        ))
                    }))
                    .when(divider, |view| {
                        view.child(div().mx_3().my_1().h(px(1.)).bg(colors.accent))
                    })
                    .children(
                        sections
                            .others
                            .iter()
                            .map(|item| self.item(*item, active(item.selection()), colors, cx)),
                    ),
            )
            .when_some(self.menu.clone(), |view, menu| {
                view.child(self.context_menu(menu, colors, cx))
            })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use desktop_runtime::{Profile, RuntimeHandle};
    use gpui::{Modifiers, point};

    use super::*;

    #[gpui::test]
    fn unpinned_rows_only_reserve_pin_space_on_hover(cx: &mut gpui::TestAppContext) {
        cx.executor().allow_parking();
        cx.update(crate::ui::theme::UiFonts::init);
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        let contacts = cx.new(|cx| {
            let mut contacts = Contacts::new(runtime.clone(), cx);
            contacts.data = Arc::new(super::super::data::ContactsData {
                humans: vec![
                    Human {
                        id: "p1".into(),
                        name: "Pinned Long Contact Name".into(),
                        pinned: true,
                        ..Default::default()
                    },
                    Human {
                        id: "p2".into(),
                        name: "Unpinned Long Contact Name".into(),
                        pinned: false,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            });
            contacts
        });
        let list = contacts.read_with(cx, |contacts, _| contacts.list());
        struct ListRoot(Entity<ContactsList>);
        impl Render for ListRoot {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                self.0.clone()
            }
        }
        let (_, cx) = cx.add_window_view(|_, _| ListRoot(list.clone()));
        cx.simulate_resize(gpui::size(px(220.), px(320.)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();

        let p1_text = cx.debug_bounds("contact-text-p1").unwrap();
        let p2_text = cx.debug_bounds("contact-text-p2").unwrap();
        assert!(cx.debug_bounds("contact-pin-p1").is_some());
        assert!(cx.debug_bounds("contact-pin-p2").is_none());
        assert!(
            p2_text.size.width > p1_text.size.width,
            "unhovered unpinned row gets full width ({:?} vs {:?})",
            p2_text.size.width,
            p1_text.size.width
        );

        cx.simulate_mouse_move(p2_text.center(), None, Modifiers::none());
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("contact-pin-p2").is_some());
        assert_eq!(
            cx.debug_bounds("contact-text-p2").unwrap().size.width,
            p1_text.size.width
        );

        cx.simulate_mouse_move(point(px(219.), px(319.)), None, Modifiers::none());
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("contact-pin-p2").is_none());

        drop(contacts);
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }
}
