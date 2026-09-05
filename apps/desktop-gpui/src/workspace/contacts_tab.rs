//! The Contacts tab: `sidebar/contacts.tsx`, `contacts/{shared,person-item,
//! organization-item,new-person-form,details,contact-page-header,
//! related-notes}.tsx`, in the people / organizations list and the person
//! details column.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, Entity, Focusable as _, ImageSource, MouseButton,
    RenderImage, SharedString, Window, div, prelude::*, px,
};

use super::Workspace;
use super::menu::{Align, Entry, MenuSpec, Select, Trailing};
use super::toast::FlashVariant;
use crate::contacts::{Human, HumanSession, Organization};
use crate::text_area::{TextArea, TextAreaEvent, TextAreaStyle};
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

const AVATAR_RASTER_SIZE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sort {
    Alphabetical,
    ReverseAlphabetical,
    Oldest,
    Newest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Selection {
    Person(String),
    Organization(String),
}

pub(crate) struct ContactsState {
    humans: Vec<Human>,
    organizations: Vec<Organization>,
    selected: Option<Selection>,
    sort: Sort,
    sort_menu_open: bool,
    search: Entity<TextInput>,
    /// `showNewPerson`
    new_person: Option<Entity<TextInput>>,
    details: Option<PersonDetails>,
    /// `useContactSummary` is LLM-backed; the shell shows the stored facts.
    avatars: HashMap<String, Arc<RenderImage>>,
}

struct PersonDetails {
    id: String,
    name: Entity<TextInput>,
    job_title: Entity<TextInput>,
    email: Entity<TextInput>,
    phone: Entity<TextInput>,
    linkedin: Entity<TextInput>,
    memo: Entity<TextArea>,
    sessions: Vec<HumanSession>,
    actions_open: bool,
    organization_open: bool,
    organization_search: Option<Entity<TextInput>>,
    related_newest: bool,
    related_sort_open: bool,
}

impl Workspace {
    pub(crate) fn open_contacts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_settings(cx);
        self.close_folders(cx);
        self.close_templates(cx);
        self.close_calendar(cx);
        if self.contacts.is_none() {
            let style = self.contact_input_style(self.theme.foreground);
            let search = cx.new(|cx| TextInput::new("Search contacts...", style, window, cx));
            cx.subscribe(&search, |this, input, event: &TextInputEvent, cx| {
                match event {
                    TextInputEvent::Escape => input.update(cx, |input, cx| input.set_text("", cx)),
                    TextInputEvent::Changed => {}
                    _ => return,
                }
                if this.contacts.is_some() {
                    cx.notify();
                }
            })
            .detach();
            self.contacts = Some(ContactsState {
                humans: Vec::new(),
                organizations: Vec::new(),
                selected: None,
                sort: Sort::Alphabetical,
                sort_menu_open: false,
                search,
                new_person: None,
                details: None,
                avatars: HashMap::new(),
            });
        }
        self.reload_contacts(window, cx);
        cx.notify();
    }

    pub(crate) fn close_contacts(&mut self, cx: &mut Context<Self>) {
        if self.contacts.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn contacts_open(&self) -> bool {
        self.contacts.is_some()
    }

    fn contact_input_style(&self, text: gpui::Rgba) -> TextInputStyle {
        TextInputStyle {
            text,
            placeholder: self.theme.muted_foreground,
            selection: self.theme.selection,
            underline_when_focused: false,
            masked: false,
        }
    }

    pub(crate) fn reload_contacts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.contacts.is_none() {
            return;
        }
        let task = self.store.list_contacts();
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok((humans, organizations))) = task.await else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                if let Some(state) = this.contacts.as_mut() {
                    state.humans = humans;
                    state.organizations = organizations;
                }
                this.sync_contact_details(window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn reload_contacts_from_watcher(&mut self, cx: &mut Context<Self>) {
        if self.contacts.is_none() {
            return;
        }
        let task = self.store.list_contacts();
        cx.spawn(async move |this, cx| {
            let Ok(Ok((humans, organizations))) = task.await else {
                return;
            };
            this.update(cx, |this, cx| {
                if let Some(state) = this.contacts.as_mut() {
                    state.humans = humans;
                    state.organizations = organizations;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// `effectiveSelection`
    fn effective_contact(&self) -> Option<Selection> {
        let state = self.contacts.as_ref()?;
        match &state.selected {
            Some(Selection::Person(id)) if state.humans.iter().any(|h| &h.id == id) => {
                Some(Selection::Person(id.clone()))
            }
            Some(Selection::Organization(id))
                if state.organizations.iter().any(|o| &o.id == id) =>
            {
                Some(Selection::Organization(id.clone()))
            }
            _ => state
                .humans
                .first()
                .map(|h| Selection::Person(h.id.clone()))
                .or_else(|| {
                    state
                        .organizations
                        .first()
                        .map(|o| Selection::Organization(o.id.clone()))
                }),
        }
    }

    fn select_contact(
        &mut self,
        selection: Option<Selection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(state) = self.contacts.as_mut() {
            state.selected = selection;
        }
        self.sync_contact_details(window, cx);
        cx.notify();
    }

    /// `<DetailsColumn key={selection.id}>`: rebuild the field inputs when the
    /// person changes; otherwise refresh the related notes.
    fn sync_contact_details(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selection = self.effective_contact();
        let Some(Selection::Person(id)) = selection else {
            if let Some(state) = self.contacts.as_mut() {
                state.details = None;
            }
            return;
        };
        if self
            .contacts
            .as_ref()
            .and_then(|state| state.details.as_ref())
            .is_some_and(|details| details.id == id)
        {
            self.refresh_related_notes(id, cx);
            return;
        }
        let Some(human) = self
            .contacts
            .as_ref()
            .and_then(|state| state.humans.iter().find(|h| h.id == id).cloned())
        else {
            return;
        };
        let theme = self.theme;
        let mut field = |placeholder: &'static str,
                         value: &str,
                         column: &'static str,
                         this: &Self,
                         cx: &mut Context<Self>| {
            let style = this.contact_input_style(theme.foreground);
            let input = cx.new(|cx| {
                let mut input = TextInput::new(placeholder, style, &mut *window, cx);
                input.set_text(value.to_string(), cx);
                input
            });
            cx.subscribe(&input, move |this, input, event: &TextInputEvent, cx| {
                if *event == TextInputEvent::Changed {
                    let value = input.read(cx).text().to_string();
                    this.persist_human_field(column, value, cx);
                }
            })
            .detach();
            input
        };
        let name = field("Name", &human.name, "name", self, cx);
        let job_title = field("Software Engineer", &human.job_title, "job_title", self, cx);
        let email = field("john@example.com", &human.email, "email", self, cx);
        let phone = field("+1 (555) 123-4567", &human.phone, "phone", self, cx);
        let linkedin = field(
            "https://www.linkedin.com/in/johntopia/",
            &human.linkedin_username,
            "linkedin_username",
            self,
            cx,
        );
        let memo = cx.new(|cx| {
            let mut area = TextArea::new(
                "Add notes about this contact...",
                TextAreaStyle {
                    text: theme.foreground,
                    placeholder: theme.muted_foreground,
                    selection: theme.selection,
                    font_size: px(14.0),
                    line_height: px(20.0),
                    rows: 3,
                },
                window,
                cx,
            );
            area.set_text(human.memo.clone(), cx);
            area
        });
        cx.subscribe(&memo, |this, area, event: &TextAreaEvent, cx| {
            if *event == TextAreaEvent::Changed {
                let value = area.read(cx).text().to_string();
                this.persist_human_field("memo", value, cx);
            }
        })
        .detach();
        if let Some(state) = self.contacts.as_mut() {
            state.details = Some(PersonDetails {
                id: id.clone(),
                name,
                job_title,
                email,
                phone,
                linkedin,
                memo,
                sessions: Vec::new(),
                actions_open: false,
                organization_open: false,
                organization_search: None,
                related_newest: true,
                related_sort_open: false,
            });
        }
        self.refresh_related_notes(id, cx);
    }

    fn refresh_related_notes(&mut self, id: String, cx: &mut Context<Self>) {
        let task = self.store.human_sessions(id.clone());
        cx.spawn(async move |this, cx| {
            let Ok(Ok(sessions)) = task.await else {
                return;
            };
            this.update(cx, |this, cx| {
                if let Some(details) = this
                    .contacts
                    .as_mut()
                    .and_then(|state| state.details.as_mut())
                    .filter(|details| details.id == id)
                {
                    details.sessions = sessions;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// `persistHumanUpdate`: immediate write; the local row follows.
    fn persist_human_field(&mut self, column: &'static str, value: String, cx: &mut Context<Self>) {
        let Some(state) = self.contacts.as_mut() else {
            return;
        };
        let Some(details) = state.details.as_ref() else {
            return;
        };
        let id = details.id.clone();
        if let Some(human) = state.humans.iter_mut().find(|h| h.id == id) {
            match column {
                "name" => human.name = value.clone(),
                "email" => human.email = value.clone(),
                "phone" => human.phone = value.clone(),
                "job_title" => human.job_title = value.clone(),
                "linkedin_username" => human.linkedin_username = value.clone(),
                "memo" => human.memo = value.clone(),
                "organization_id" => human.organization_id = value.clone(),
                _ => {}
            }
        }
        cx.notify();
        let task = self.store.update_human_field(id, column, value);
        cx.spawn(async move |this, cx| {
            if let Ok(Err(error)) = task.await {
                this.update(cx, |this, cx| {
                    this.flash(FlashVariant::Error, error.to_string(), cx)
                })
                .ok();
            }
        })
        .detach();
    }

    fn contact_pin(
        &mut self,
        table: &'static str,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = self.store.toggle_contact_pin(table, id);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.flash(FlashVariant::Error, error.to_string(), cx);
                }
                this.reload_contacts(window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn delete_contact(
        &mut self,
        table: &'static str,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(state) = self.contacts.as_mut() {
            state.selected = None;
            state.details = None;
        }
        let task = self.store.delete_contact(table, id);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.flash(FlashVariant::Error, error.to_string(), cx);
                }
                this.reload_contacts(window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// `NewPersonForm` Enter: `createHuman({ name })`, then select it.
    fn submit_new_person(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.contacts.as_mut() else {
            return;
        };
        let Some(input) = state.new_person.take() else {
            return;
        };
        let name = input.read(cx).text().trim().to_string();
        cx.notify();
        if name.is_empty() {
            return;
        }
        let task = self.store.create_contact_human(name);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| match result {
                Ok(id) => {
                    if let Some(state) = this.contacts.as_mut() {
                        state.selected = Some(Selection::Person(id));
                    }
                    this.reload_contacts(window, cx);
                }
                Err(error) => this.flash(FlashVariant::Error, error.to_string(), cx),
            })
            .ok();
        })
        .detach();
    }

    /// Rasterises every avatar the tab can show, once per seed, before the
    /// frame's element tree borrows the state.
    pub(crate) fn prepare_contact_avatars(&mut self) {
        let Some(state) = self.contacts.as_ref() else {
            return;
        };
        let seeds: Vec<String> = state.humans.iter().map(Human::avatar_seed).collect();
        for seed in seeds {
            self.avatar_image(&seed);
        }
    }

    fn avatar_image(&mut self, seed: &str) {
        let Some(state) = self.contacts.as_mut() else {
            return;
        };
        state.avatars.entry(seed.to_string()).or_insert_with(|| {
            let mut pixels = crate::contacts::avatar_pixels(seed, AVATAR_RASTER_SIZE);
            // GPUI textures are BGRA.
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            let buffer = image::RgbaImage::from_raw(
                AVATAR_RASTER_SIZE as u32,
                AVATAR_RASTER_SIZE as u32,
                pixels,
            )
            .expect("raster dimensions match the buffer");
            Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
                buffer
            )]))
        });
    }

    /// `ContactFacehash`: the dithered raster under a squircle-ish clip with
    /// the `1px rgb(0 0 0 / 0.1)` border and the white initials.
    fn render_avatar(&self, seed: &str, size: f32) -> AnyElement {
        let image = self
            .contacts
            .as_ref()
            .and_then(|state| state.avatars.get(seed).cloned());
        let initials = crate::contacts::avatar_initials(seed);
        let radius = if size >= 48.0 {
            8.0
        } else {
            8.0_f32.min(size * 0.25)
        };
        div()
            .relative()
            .size(px(size))
            .flex_shrink_0()
            .rounded(px(radius))
            .overflow_hidden()
            .border_1()
            .border_color(gpui::hsla(0.0, 0.0, 0.0, 0.1))
            .shadow(vec![gpui::BoxShadow {
                color: gpui::hsla(0.0, 0.0, 0.0, 0.08),
                offset: gpui::point(px(0.0), px(1.0)),
                blur_radius: px(2.0),
                spread_radius: px(0.0),
            }])
            .when_some(image, |avatar, image| {
                avatar.child(
                    gpui::img(ImageSource::Render(image))
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(gpui::ObjectFit::Cover),
                )
            })
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px((size * 0.38).max(7.0)))
                    .line_height(px((size * 0.38).max(7.0)))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(gpui::rgb(0xffffff))
                    .child(SharedString::from(initials)),
            )
            .into_any_element()
    }

    /// `ContactsNav` / `ContactsList`
    pub(super) fn render_contacts_sidebar(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let Some(state) = self.contacts.as_ref() else {
            return div();
        };
        let query = state.search.read(cx).text().trim().to_lowercase();
        let searching = !query.is_empty();
        // `isActive` reads the explicit selection, not the effective fallback.
        let selected = state.selected.clone();

        let compare =
            |a_name: &str, a_created: &str, b_name: &str, b_created: &str| match state.sort {
                Sort::Alphabetical => a_name.cmp(b_name),
                Sort::ReverseAlphabetical => b_name.cmp(a_name),
                Sort::Oldest => a_created.cmp(b_created),
                Sort::Newest => b_created.cmp(a_created),
            };
        let mut humans: Vec<&Human> = state
            .humans
            .iter()
            .filter(|h| {
                query.is_empty()
                    || [&h.name, &h.email, &h.phone]
                        .iter()
                        .any(|value| value.to_lowercase().contains(&query))
            })
            .collect();
        humans.sort_by(|a, b| compare(&a.name, &a.created_at, &b.name, &b.created_at));
        let mut organizations: Vec<&Organization> = state
            .organizations
            .iter()
            .filter(|o| query.is_empty() || o.name.to_lowercase().contains(&query))
            .collect();
        organizations.sort_by(|a, b| compare(&a.name, &a.created_at, &b.name, &b.created_at));

        enum Item<'a> {
            Person(&'a Human),
            Org(&'a Organization),
        }
        let mut pinned: Vec<(Option<i64>, Item)> = humans
            .iter()
            .filter(|h| h.pinned)
            .map(|h| (h.pin_order, Item::Person(h)))
            .chain(
                organizations
                    .iter()
                    .filter(|o| o.pinned)
                    .map(|o| (o.pin_order, Item::Org(o))),
            )
            .collect();
        pinned.sort_by_key(|(order, _)| order.unwrap_or(i64::MAX));
        let unpinned: Vec<Item> = organizations
            .iter()
            .filter(|o| !o.pinned)
            .map(|o| Item::Org(o))
            .chain(humans.iter().filter(|h| !h.pinned).map(|h| Item::Person(h)))
            .collect();

        let row_width = self.sidebar_width - 4.0;
        // `px-3`, the 32px avatar, `gap-2`, and the 22px pin button.
        let title_width = (row_width - 24.0 - 32.0 - 8.0 - 8.0 - 22.0).max(0.0);
        let render_item = |this: &Self, item: &Item| -> AnyElement {
            match item {
                Item::Person(human) => {
                    let active = selected == Some(Selection::Person(human.id.clone()));
                    let id = human.id.clone();
                    let pin_id = human.id.clone();
                    let name = human.display_name();
                    let show_email = !human.email.is_empty() && !human.name.is_empty();
                    div()
                        .id(SharedString::from(format!("contact-{}", human.id)))
                        .flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .overflow_hidden()
                        .rounded_lg()
                        .px_3()
                        .py_2()
                        .tw_text_sm()
                        .cursor_pointer()
                        .when(active, |row| row.bg(theme.accent))
                        .when(!active, |row| {
                            row.hover(move |style| style.bg(alpha(theme.accent, 0.5)))
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.select_contact(Some(Selection::Person(id.clone())), window, cx);
                        }))
                        .child(this.render_avatar(&human.avatar_seed(), 32.0))
                        .child(
                            div()
                                .min_w_0()
                                .flex_grow()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .w(px(title_width))
                                        .truncate()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(theme.foreground)
                                        .child(SharedString::from(name)),
                                )
                                .when(show_email, |column| {
                                    column.child(
                                        div()
                                            .w(px(title_width))
                                            .truncate()
                                            .tw_text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(SharedString::from(human.email.clone())),
                                    )
                                }),
                        )
                        .child(
                            // The pin button: blue when pinned, otherwise hover-only.
                            div()
                                .id(SharedString::from(format!("contact-pin-{}", human.id)))
                                .flex_shrink_0()
                                .rounded(px(2.0))
                                .p_1()
                                .cursor_pointer()
                                .when(!human.pinned, |button| {
                                    button.opacity(0.0).hover(|style| style.opacity(1.0))
                                })
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    this.contact_pin("humans", pin_id.clone(), window, cx);
                                }))
                                .child(icon(
                                    "push-pin",
                                    px(14.0),
                                    if human.pinned {
                                        gpui::rgb(0x155dfc)
                                    } else {
                                        alpha(theme.muted_foreground, 0.7)
                                    },
                                )),
                        )
                        .into_any_element()
                }
                Item::Org(organization) => {
                    let active = selected == Some(Selection::Organization(organization.id.clone()));
                    let id = organization.id.clone();
                    let pin_id = organization.id.clone();
                    div()
                        .id(SharedString::from(format!(
                            "organization-{}",
                            organization.id
                        )))
                        .flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .overflow_hidden()
                        .rounded_lg()
                        .px_3()
                        .py_2()
                        .tw_text_sm()
                        .cursor_pointer()
                        .when(active, |row| row.bg(theme.accent))
                        .when(!active, |row| {
                            row.hover(move |style| style.bg(alpha(theme.accent, 0.5)))
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.select_contact(
                                Some(Selection::Organization(id.clone())),
                                window,
                                cx,
                            );
                        }))
                        .child(
                            div()
                                .flex()
                                .size(px(32.0))
                                .flex_shrink_0()
                                .items_center()
                                .justify_center()
                                .rounded_lg()
                                .bg(theme.muted)
                                .child(icon("buildings", px(16.0), theme.muted_foreground)),
                        )
                        .child(
                            div().min_w_0().flex_grow().flex().flex_col().child(
                                div()
                                    .w(px(title_width))
                                    .truncate()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.foreground)
                                    .child(SharedString::from(if organization.name.is_empty() {
                                        "Unnamed".to_string()
                                    } else {
                                        organization.name.clone()
                                    })),
                            ),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!(
                                    "organization-pin-{}",
                                    organization.id
                                )))
                                .flex_shrink_0()
                                .rounded(px(2.0))
                                .p_1()
                                .cursor_pointer()
                                .when(!organization.pinned, |button| {
                                    button.opacity(0.0).hover(|style| style.opacity(1.0))
                                })
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    this.contact_pin("organizations", pin_id.clone(), window, cx);
                                }))
                                .child(icon(
                                    "push-pin",
                                    px(14.0),
                                    if organization.pinned {
                                        gpui::rgb(0x155dfc)
                                    } else {
                                        alpha(theme.muted_foreground, 0.7)
                                    },
                                )),
                        )
                        .into_any_element()
                }
            }
        };

        let mut list = div().flex().flex_col();
        if let Some(input) = &state.new_person {
            // `NewPersonForm`: the `h-8 rounded-lg bg-accent/50` field with the return glyph.
            let focus = input.clone();
            list = list.child(
                div().px_2().py_2().child(
                    div()
                        .id("new-person")
                        .relative()
                        .flex()
                        .h(px(32.0))
                        .w_full()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .cursor_text()
                        .child(crate::squircle::squircle(
                            crate::squircle::CONTROL_RADIUS,
                            Some(alpha(theme.accent, 0.5)),
                            Some((1.0, theme.border)),
                        ))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                            focus.read(cx).focus_handle(cx).focus(window);
                        }))
                        .child(div().min_w_0().flex_1().tw_text_sm().child(input.clone()))
                        .child(icon(
                            "arrow-elbow-down-left",
                            px(16.0),
                            theme.muted_foreground,
                        )),
                ),
            );
        }
        for (_, item) in &pinned {
            list = list.child(render_item(self, item));
        }
        if !pinned.is_empty() && !unpinned.is_empty() {
            list = list.child(div().mx_3().my_1().h(px(1.0)).bg(theme.accent));
        }
        for item in &unpinned {
            list = list.child(render_item(self, item));
        }

        let search_input = state.search.clone();
        let focus_input = search_input.clone();
        let sort_open = state.sort_menu_open;
        let sort = state.sort;

        div()
            .flex()
            .flex_col()
            .h_full()
            .w(px(self.sidebar_width))
            .flex_shrink_0()
            .pr_1()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .h(px(48.0))
                    .flex_shrink_0()
                    .items_start()
                    .pt(px(9.0))
                    .pr_1()
                    .pl_2()
                    .child(
                        div()
                            .flex()
                            .min_w_0()
                            .flex_1()
                            .items_center()
                            .gap_1()
                            .child(
                                self.tracked_chrome_button("contacts-back", cx)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_contacts(cx)))
                                    .child(icon(
                                        "arrow-left",
                                        px(16.0),
                                        self.chrome_icon_color("contacts-back"),
                                    )),
                            )
                            .child(div().flex_1())
                            // `hidden @[220px]:block`: the sort menu needs a 220px column.
                            .when(self.sidebar_width - 4.0 >= 220.0, |header| {
                                header.child(
                                div()
                                    .relative()
                                    .child(
                                        self.tracked_chrome_button("contacts-sort", cx)
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                if let Some(state) = this.contacts.as_mut() {
                                                    state.sort_menu_open = !state.sort_menu_open;
                                                    cx.notify();
                                                }
                                            }))
                                            .child(icon(
                                                "arrows-down-up",
                                                px(16.0),
                                                self.chrome_icon_color("contacts-sort"),
                                            )),
                                    )
                                    .when(sort_open, |anchor| {
                                        let option = |label: &'static str, value: Sort| Entry::Item {
                                            icon: None,
                                            dim_icon: false,
                                            label: label.into(),
                                            trailing: Trailing::Check(sort == value),
                                            destructive: false,
                                            on_select: Some(Box::new(move |this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                                                if let Some(state) = this.contacts.as_mut() {
                                                    state.sort = value;
                                                    cx.notify();
                                                }
                                            }) as Select),
                                            submenu: None,
                                        };
                                        let spec = MenuSpec {
                                            id: "contacts-sort-menu",
                                            width: 160.0,
                                            entries: vec![
                                                option("A-Z", Sort::Alphabetical),
                                                option("Z-A", Sort::ReverseAlphabetical),
                                                option("Oldest", Sort::Oldest),
                                                option("Newest", Sort::Newest),
                                            ],
                                            open_sub: None,
                                            on_hover_sub: |_, _, _| {},
                                            on_close: |this, cx| {
                                                if let Some(state) = this.contacts.as_mut() {
                                                    state.sort_menu_open = false;
                                                    cx.notify();
                                                }
                                            },
                                        };
                                        anchor.child(
                                            div()
                                                .absolute()
                                                .top(px(32.0))
                                                .right_0()
                                                .child(self.render_menu_inline(spec, Align::End, cx)),
                                        )
                                    }),
                                )
                            })
                            .child(
                                self.tracked_chrome_button("contacts-add", cx)
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.show_new_person(window, cx);
                                    }))
                                    .child(icon("plus", px(16.0), self.chrome_icon_color("contacts-add"))),
                            ),
                    ),
            )
            .child(
                div().pb_2().child(
                    // `h-8 rounded-lg border bg-muted px-3 gap-2`
                    div()
                        .id("contacts-search")
                        .relative()
                        .flex()
                        .h(px(32.0))
                        .w_full()
                        .flex_shrink_0()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .cursor_text()
                        .child(crate::squircle::squircle(
                            crate::squircle::CONTROL_RADIUS,
                            Some(theme.muted),
                            Some((1.0, theme.border)),
                        ))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                            focus_input.read(cx).focus_handle(cx).focus(window);
                        }))
                        .child(icon("search", px(16.0), theme.muted_foreground))
                        .child(div().min_w_0().flex_1().tw_text_sm().child(state.search.clone()))
                        .when(searching, |row| {
                            row.child(
                                div()
                                    .id("contacts-search-clear")
                                    .size(px(16.0))
                                    .flex_shrink_0()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                        search_input.update(cx, |input, cx| input.set_text("", cx));
                                    }))
                                    .child(icon("x", px(16.0), theme.muted_foreground)),
                            )
                        }),
                ),
            )
            .child(
                div()
                    .id("contacts-list")
                    .min_h_0()
                    .w_full()
                    .flex_1()
                    .overflow_y_scroll()
                    .child(list),
            )
    }

    fn show_new_person(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let style = self.contact_input_style(self.theme.foreground);
        let Some(state) = self.contacts.as_mut() else {
            return;
        };
        if let Some(input) = &state.new_person {
            input.read(cx).focus_handle(cx).focus(window);
            return;
        }
        let input = cx.new(|cx| TextInput::new("Add person", style, window, cx));
        cx.subscribe_in(
            &input,
            window,
            |this, _, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Enter => this.submit_new_person(window, cx),
                TextInputEvent::Escape => {
                    if let Some(state) = this.contacts.as_mut() {
                        state.new_person = None;
                        cx.notify();
                    }
                }
                _ => {}
            },
        )
        .detach();
        input.read(cx).focus_handle(cx).focus(window);
        state.new_person = Some(input);
        cx.notify();
    }

    /// `ContactView`: `DetailsColumn` for a person, the organization details, or
    /// the empty prompt.
    pub(super) fn render_contacts_main(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let Some(selection) = self.effective_contact() else {
            return div()
                .flex()
                .flex_1()
                .h_full()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .tw_text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Select a person to view details"),
                )
                .into_any_element();
        };
        match selection {
            Selection::Person(id) => {
                let Some(human) = self
                    .contacts
                    .as_ref()
                    .and_then(|state| state.humans.iter().find(|h| h.id == id).cloned())
                else {
                    return div().into_any_element();
                };
                let Some(state) = self.contacts.as_ref() else {
                    return div().into_any_element();
                };
                let Some(details) = state.details.as_ref().filter(|d| d.id == id) else {
                    return div().into_any_element();
                };
                self.render_person_details(&human, details, state, cx)
            }
            Selection::Organization(id) => {
                let Some(state) = self.contacts.as_ref() else {
                    return div().into_any_element();
                };
                let Some(organization) = state.organizations.iter().find(|o| o.id == id).cloned()
                else {
                    return div().into_any_element();
                };
                let members: Vec<Human> = state
                    .humans
                    .iter()
                    .filter(|h| h.organization_id == organization.id)
                    .cloned()
                    .collect();
                self.render_organization_details(&organization, &members, cx)
            }
        }
    }

    /// `ContactPageHeader`
    fn render_contact_header(
        &self,
        title: String,
        pinned: bool,
        menu_open: bool,
        menu: MenuSpec,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        div()
            .flex()
            .h(px(48.0))
            .flex_shrink_0()
            .items_center()
            .justify_between()
            .gap_3()
            .pr_1()
            .pl_3()
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .flex_1()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child(SharedString::from(title)),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .child(
                        div()
                            .id("contact-actions")
                            .flex()
                            .size(px(32.0))
                            .items_center()
                            .justify_center()
                            .rounded(px(8.0))
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(details) =
                                    this.contacts.as_mut().and_then(|s| s.details.as_mut())
                                {
                                    details.actions_open = !details.actions_open;
                                    cx.notify();
                                }
                            }))
                            .child(icon(
                                "more-horizontal",
                                px(16.0),
                                if menu_open {
                                    theme.foreground
                                } else {
                                    theme.muted_foreground
                                },
                            )),
                    )
                    .when(menu_open, |anchor| {
                        let _ = pinned;
                        anchor.child(
                            div()
                                .absolute()
                                .top(px(36.0))
                                .right_0()
                                .child(self.render_menu_inline(menu, Align::End, cx)),
                        )
                    }),
            )
    }

    /// `DetailsColumn`
    fn render_person_details(
        &self,
        human: &Human,
        details: &PersonDetails,
        state: &ContactsState,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let id = human.id.clone();
        let organization = state
            .organizations
            .iter()
            .find(|o| o.id == human.organization_id)
            .cloned();

        let (pin_id, del_id) = (id.clone(), id.clone());
        let menu = MenuSpec {
            id: "contact-actions-menu",
            width: 192.0,
            entries: vec![
                Entry::Item {
                    icon: Some("push-pin"),
                    dim_icon: false,
                    label: if human.pinned {
                        "Unpin".into()
                    } else {
                        "Pin".into()
                    },
                    trailing: Trailing::None,
                    destructive: false,
                    on_select: Some(Box::new(
                        move |this: &mut Workspace,
                              window: &mut Window,
                              cx: &mut Context<Workspace>| {
                            this.contact_pin("humans", pin_id.clone(), window, cx);
                        },
                    ) as Select),
                    submenu: None,
                },
                Entry::Item {
                    icon: Some("trash"),
                    dim_icon: false,
                    label: "Delete".into(),
                    trailing: Trailing::None,
                    destructive: true,
                    on_select: Some(Box::new(
                        move |this: &mut Workspace,
                              window: &mut Window,
                              cx: &mut Context<Workspace>| {
                            this.delete_contact("humans", del_id.clone(), window, cx);
                        },
                    ) as Select),
                    submenu: None,
                },
            ],
            open_sub: None,
            on_hover_sub: |_, _, _| {},
            on_close: |this, cx| {
                if let Some(details) = this.contacts.as_mut().and_then(|s| s.details.as_mut()) {
                    details.actions_open = false;
                    cx.notify();
                }
            },
        };

        // `border-b px-4 py-3` rows with the `w-28 text-sm text-muted-foreground` label.
        let row = |label: &'static str, field: AnyElement| {
            div()
                .flex()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .px_4()
                .py_3()
                .child(
                    div()
                        .w(px(112.0))
                        .tw_text_sm()
                        .text_color(theme.muted_foreground)
                        .child(label),
                )
                .child(div().flex_1().child(field))
        };
        // `Input h-7 border-none p-0 text-base`
        let field = |id: &'static str, input: &Entity<TextInput>| {
            let focus = input.clone();
            div()
                .id(id)
                .flex()
                .h(px(28.0))
                .w_full()
                .items_center()
                // `text-base md:text-sm`: the window is past the `md` breakpoint.
                .tw_text_sm()
                .cursor_text()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                    focus.read(cx).focus_handle(cx).focus(window);
                }))
                .child(div().min_w_0().flex_1().child(input.clone()))
                .into_any_element()
        };

        let organization_field: AnyElement = match &organization {
            Some(organization) => div()
                .id("contact-organization")
                .flex()
                .items_center()
                .mx(px(-8.0))
                .rounded_lg()
                .px_2()
                .py_1()
                .cursor_pointer()
                .hover(move |style| style.bg(theme.accent))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .tw_text_base()
                        .text_color(theme.foreground)
                        .child(SharedString::from(organization.name.clone())),
                )
                .child(
                    div()
                        .id("contact-organization-remove")
                        .ml_2()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.persist_human_field("organization_id", String::new(), cx);
                        }))
                        .child(icon("x", px(16.0), theme.muted_foreground)),
                )
                .into_any_element(),
            None => div()
                .id("contact-organization")
                .flex()
                .items_center()
                .gap_1()
                .mx(px(-8.0))
                .rounded_lg()
                .px_2()
                .py_1()
                .tw_text_base()
                .text_color(theme.muted_foreground)
                .cursor_pointer()
                .hover(move |style| style.bg(theme.accent))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.toggle_organization_picker(window, cx);
                }))
                .child(icon("plus", px(16.0), theme.muted_foreground))
                .child("Add organization")
                .into_any_element(),
        };

        let memo_area = details.memo.clone();
        let body = div()
            .id("contact-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                // The 64px avatar block: `border-b py-6`.
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .py_6()
                    .child(self.render_avatar(&human.avatar_seed(), 64.0)),
            )
            .child(
                div()
                    .child(row("Name", field("contact-name", &details.name)))
                    .child(row(
                        "Job Title",
                        field("contact-job-title", &details.job_title),
                    ))
                    .child(
                        div()
                            .relative()
                            .child(row("Company", organization_field))
                            .when(details.organization_open, |anchor| {
                                anchor.child(self.render_organization_picker(details, state, cx))
                            }),
                    )
                    .child(row("Email", field("contact-email", &details.email)))
                    .child(row("Phone", field("contact-phone", &details.phone)))
                    .child(row(
                        "LinkedIn",
                        field("contact-linkedin", &details.linkedin),
                    ))
                    .child(
                        // `EditablePersonMemoField`: `pt-2` label, `min-h-[80px] py-2 text-base` textarea.
                        div()
                            .flex()
                            .border_b_1()
                            .border_color(theme.border)
                            .px_4()
                            .py_3()
                            .child(
                                div()
                                    .w(px(112.0))
                                    .pt_2()
                                    .tw_text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Notes"),
                            )
                            .child(
                                div()
                                    .id("contact-memo")
                                    .flex_1()
                                    .min_h(px(80.0))
                                    .py_2()
                                    .cursor_text()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click(move |_: &ClickEvent, window, cx| {
                                        memo_area.update(cx, |area, cx| area.focus_end(window, cx));
                                    })
                                    .child(details.memo.clone()),
                            ),
                    ),
            )
            .when(!details.sessions.is_empty(), |body| {
                body.child(self.render_contact_summary(human))
            })
            .child(self.render_related_notes(details, cx))
            .child(div().pb(px(384.0)));

        div()
            .flex()
            .h_full()
            .flex_1()
            .flex_col()
            .child(self.render_contact_header(
                human.display_name(),
                human.pinned,
                details.actions_open,
                menu,
                cx,
            ))
            .child(body)
            .into_any_element()
    }

    /// `ContactSummarySection` with the stored facts or the explainer copy.
    fn render_contact_summary(&self, human: &Human) -> Div {
        let theme = self.theme;
        div()
            .border_b_1()
            .border_color(theme.border)
            .p_6()
            .child(
                div()
                    .mb_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .child("Summary"),
                    ),
            )
            .child(
                div()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.muted)
                    .p_4()
                    .child(if human.summary_facts.is_empty() {
                        div()
                            .tw_text_sm()
                            .line_height(px(22.0))
                            .text_color(theme.muted_foreground)
                            .child("AI-generated summary of all interactions and notes with this contact will appear here. This will synthesize key discussion points, action items, and relationship context across all meetings and notes.")
                            .into_any_element()
                    } else {
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .pl_5()
                            .tw_text_sm()
                            .line_height(px(22.0))
                            .text_color(theme.foreground)
                            .children(human.summary_facts.iter().map(|fact| {
                                div()
                                    .relative()
                                    .child(
                                        div()
                                            .absolute()
                                            .left(px(-14.0))
                                            .top(px(8.0))
                                            .size(px(5.0))
                                            .rounded_full()
                                            .bg(theme.foreground),
                                    )
                                    .child(SharedString::from(fact.clone()))
                            }))
                            .into_any_element()
                    }),
            )
    }

    /// `RelatedNotesSection`
    fn render_related_notes(&self, details: &PersonDetails, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let mut sessions = details.sessions.clone();
        if !details.related_newest {
            sessions.reverse();
        }
        let sort_open = details.related_sort_open;
        div()
            .p_6()
            .child(
                div()
                    .mb_3()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .child("Related Notes"),
                    )
                    .when(details.sessions.len() > 1, |header| {
                        header.child(
                            div()
                                .relative()
                                .child(
                                    div()
                                        .id("related-sort")
                                        .flex()
                                        .size(px(28.0))
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(8.0))
                                        .cursor_pointer()
                                        .hover(move |style| style.bg(theme.accent))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            if let Some(details) = this.contacts.as_mut().and_then(|s| s.details.as_mut()) {
                                                details.related_sort_open = !details.related_sort_open;
                                                cx.notify();
                                            }
                                        }))
                                        .child(icon("arrows-down-up", px(16.0), theme.muted_foreground)),
                                )
                                .when(sort_open, |anchor| {
                                    let option = |label: &'static str, newest: bool| Entry::Item {
                                        icon: None,
                                        dim_icon: false,
                                        label: label.into(),
                                        trailing: Trailing::Check(details.related_newest == newest),
                                        destructive: false,
                                        on_select: Some(Box::new(move |this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                                            if let Some(details) = this.contacts.as_mut().and_then(|s| s.details.as_mut()) {
                                                details.related_newest = newest;
                                                cx.notify();
                                            }
                                        }) as Select),
                                        submenu: None,
                                    };
                                    let spec = MenuSpec {
                                        id: "related-sort-menu",
                                        width: 160.0,
                                        entries: vec![option("Newest", true), option("Oldest", false)],
                                        open_sub: None,
                                        on_hover_sub: |_, _, _| {},
                                        on_close: |this, cx| {
                                            if let Some(details) = this.contacts.as_mut().and_then(|s| s.details.as_mut()) {
                                                details.related_sort_open = false;
                                                cx.notify();
                                            }
                                        },
                                    };
                                    anchor.child(
                                        div()
                                            .absolute()
                                            .top(px(32.0))
                                            .left_0()
                                            .child(self.render_menu_inline(spec, Align::Start, cx)),
                                    )
                                }),
                        )
                    }),
            )
            .child(if sessions.is_empty() {
                div()
                    .px_2()
                    .py_2()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child("No related notes found")
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .children(sessions.iter().map(|session| {
                        let id = session.id.clone();
                        let date = crate::timeline::parse_date(&session.created_at, &chrono::Local)
                            .map(|utc| utc.with_timezone(&chrono::Local).format("%-m/%-d/%Y").to_string());
                        div()
                            .id(SharedString::from(format!("related-{}", session.id)))
                            .flex()
                            .w_full()
                            .items_center()
                            .gap_3()
                            .rounded_md()
                            .px_2()
                            .py_2()
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.close_contacts(cx);
                                this.select_session_from_calendar(id.clone(), cx);
                            }))
                            .child(div().size(px(6.0)).flex_shrink_0().rounded_full().bg(theme.muted_foreground))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_grow()
                                    .flex()
                                    .flex_col()
                                    .child(div().truncate().tw_text_sm().text_color(theme.foreground).child(
                                        SharedString::from(if session.title.is_empty() {
                                            "Untitled Note".to_string()
                                        } else {
                                            session.title.clone()
                                        }),
                                    )),
                            )
                            .when_some(date, |row, date| {
                                row.child(
                                    div()
                                        .flex_shrink_0()
                                        .tw_text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(SharedString::from(date)),
                                )
                            })
                    }))
                    .into_any_element()
            })
    }

    fn toggle_organization_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let style = self.contact_input_style(self.theme.foreground);
        let Some(details) = self.contacts.as_mut().and_then(|s| s.details.as_mut()) else {
            return;
        };
        details.organization_open = !details.organization_open;
        if details.organization_open && details.organization_search.is_none() {
            let input =
                cx.new(|cx| TextInput::new("Search or create organization", style, window, cx));
            cx.subscribe_in(
                &input,
                window,
                |this, input, event: &TextInputEvent, window, cx| match event {
                    TextInputEvent::Changed => cx.notify(),
                    TextInputEvent::Enter => {
                        let name = input.read(cx).text().trim().to_string();
                        this.create_organization_for_contact(name, window, cx);
                    }
                    TextInputEvent::Escape => {
                        if let Some(details) =
                            this.contacts.as_mut().and_then(|s| s.details.as_mut())
                        {
                            details.organization_open = false;
                            cx.notify();
                        }
                    }
                    _ => {}
                },
            )
            .detach();
            details.organization_search = Some(input);
        }
        if let Some(input) = &details.organization_search {
            input.update(cx, |input, cx| input.set_text("", cx));
            if details.organization_open {
                input.read(cx).focus_handle(cx).focus(window);
            }
        }
        cx.notify();
    }

    /// `EditPersonOrganizationSelector`: the `p-3` app panel with the search
    /// field, matching organizations, and the create row.
    fn render_organization_picker(
        &self,
        details: &PersonDetails,
        state: &ContactsState,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let Some(input) = details.organization_search.clone() else {
            return div().into_any_element();
        };
        let query = input.read(cx).text().trim().to_string();
        let lower = query.to_lowercase();
        let matches: Vec<&Organization> = state
            .organizations
            .iter()
            .filter(|o| lower.is_empty() || o.name.to_lowercase().contains(&lower))
            .collect();
        let exact = state
            .organizations
            .iter()
            .any(|o| o.name.to_lowercase() == lower);
        let focus = input.clone();
        let mut list = div().flex().flex_col().gap_1();
        for organization in matches {
            let id = organization.id.clone();
            list = list.child(
                div()
                    .id(SharedString::from(format!(
                        "org-option-{}",
                        organization.id
                    )))
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .px_2()
                    .py(px(6.0))
                    .tw_text_sm()
                    .text_color(theme.foreground)
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.accent))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.persist_human_field("organization_id", id.clone(), cx);
                        if let Some(details) =
                            this.contacts.as_mut().and_then(|s| s.details.as_mut())
                        {
                            details.organization_open = false;
                        }
                        cx.notify();
                    }))
                    .child(icon("buildings", px(16.0), theme.muted_foreground))
                    .child(SharedString::from(organization.name.clone())),
            );
        }
        if !query.is_empty() && !exact {
            let name = query.clone();
            list = list.child(
                div()
                    .id("org-option-create")
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .px_2()
                    .py(px(6.0))
                    .tw_text_sm()
                    .text_color(theme.foreground)
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.accent))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.create_organization_for_contact(name.clone(), window, cx);
                    }))
                    .child(icon("plus", px(16.0), theme.muted_foreground))
                    .child(SharedString::from(format!("Create \"{query}\""))),
            );
        }
        let panel = div()
            .id("organization-picker")
            .occlude()
            .relative()
            .w(px(320.0))
            .child(crate::squircle::squircle(
                crate::squircle::PANEL_RADIUS,
                Some(theme.floating_panel),
                Some((1.0, theme.floating_border)),
            ))
            .shadow(vec![gpui::BoxShadow {
                color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                offset: gpui::point(px(0.0), px(8.0)),
                blur_radius: px(24.0),
                spread_radius: px(0.0),
            }])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|this, _: &gpui::MouseDownEvent, _, cx| {
                if let Some(details) = this.contacts.as_mut().and_then(|s| s.details.as_mut()) {
                    details.organization_open = false;
                    cx.notify();
                }
            }))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_3()
                    .child(
                        div()
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .child("Organization"),
                    )
                    .child(
                        div()
                            .id("organization-search")
                            .flex()
                            .w_full()
                            .items_center()
                            .gap_2()
                            .rounded(px(2.0))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.muted)
                            .px_2()
                            .py(px(6.0))
                            .tw_text_sm()
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(move |_: &ClickEvent, window, cx| {
                                focus.read(cx).focus_handle(cx).focus(window);
                            })
                            .child(icon("search", px(16.0), theme.muted_foreground))
                            .child(div().min_w_0().flex_1().child(input)),
                    )
                    .child(list),
            );
        div()
            .absolute()
            .top(px(44.0))
            .left(px(120.0))
            .child(
                gpui::deferred(
                    gpui::anchored()
                        .anchor(gpui::Corner::TopLeft)
                        .snap_to_window_with_margin(px(16.0))
                        .child(panel),
                )
                .with_priority(2),
            )
            .into_any_element()
    }

    fn create_organization_for_contact(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if name.is_empty() {
            return;
        }
        let task = self.store.create_organization(name);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(id) => {
                        this.persist_human_field("organization_id", id, cx);
                        if let Some(details) =
                            this.contacts.as_mut().and_then(|s| s.details.as_mut())
                        {
                            details.organization_open = false;
                        }
                    }
                    Err(error) => this.flash(FlashVariant::Error, error.to_string(), cx),
                }
                this.reload_contacts(window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// `OrganizationDetailsColumn`: the header, name, and member list.
    fn render_organization_details(
        &self,
        organization: &Organization,
        members: &[Human],
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let (pin_id, del_id) = (organization.id.clone(), organization.id.clone());
        let menu = MenuSpec {
            id: "contact-actions-menu",
            width: 192.0,
            entries: vec![
                Entry::Item {
                    icon: Some("push-pin"),
                    dim_icon: false,
                    label: if organization.pinned {
                        "Unpin".into()
                    } else {
                        "Pin".into()
                    },
                    trailing: Trailing::None,
                    destructive: false,
                    on_select: Some(Box::new(
                        move |this: &mut Workspace,
                              window: &mut Window,
                              cx: &mut Context<Workspace>| {
                            this.contact_pin("organizations", pin_id.clone(), window, cx);
                        },
                    ) as Select),
                    submenu: None,
                },
                Entry::Item {
                    icon: Some("trash"),
                    dim_icon: false,
                    label: "Delete".into(),
                    trailing: Trailing::None,
                    destructive: true,
                    on_select: Some(Box::new(
                        move |this: &mut Workspace,
                              window: &mut Window,
                              cx: &mut Context<Workspace>| {
                            this.delete_contact("organizations", del_id.clone(), window, cx);
                        },
                    ) as Select),
                    submenu: None,
                },
            ],
            open_sub: None,
            on_hover_sub: |_, _, _| {},
            on_close: |this, cx| {
                if let Some(details) = this.contacts.as_mut().and_then(|s| s.details.as_mut()) {
                    details.actions_open = false;
                    cx.notify();
                }
            },
        };
        let title = if organization.name.is_empty() {
            "Unnamed".to_string()
        } else {
            organization.name.clone()
        };
        div()
            .flex()
            .h_full()
            .flex_1()
            .flex_col()
            .child(self.render_contact_header(title, organization.pinned, false, menu, cx))
            .child(
                div()
                    .id("organization-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_b_1()
                            .border_color(theme.border)
                            .py_6()
                            .child(
                                div()
                                    .flex()
                                    .size(px(64.0))
                                    .items_center()
                                    .justify_center()
                                    .rounded_lg()
                                    .bg(theme.muted)
                                    .child(icon("buildings", px(28.0), theme.muted_foreground)),
                            ),
                    )
                    .child(
                        div()
                            .p_6()
                            .child(
                                div()
                                    .mb_3()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.muted_foreground)
                                    .child("People"),
                            )
                            .child(if members.is_empty() {
                                div()
                                    .px_2()
                                    .py_2()
                                    .tw_text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("No people in this organization yet")
                                    .into_any_element()
                            } else {
                                div()
                                    .flex()
                                    .flex_col()
                                    .children(members.iter().map(|human| {
                                        let id = human.id.clone();
                                        div()
                                            .id(SharedString::from(format!("member-{}", human.id)))
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .rounded_md()
                                            .px_2()
                                            .py_2()
                                            .cursor_pointer()
                                            .hover(move |style| style.bg(theme.accent))
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .on_click(cx.listener(
                                                move |this, _: &ClickEvent, window, cx| {
                                                    this.select_contact(
                                                        Some(Selection::Person(id.clone())),
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            ))
                                            .child(self.render_avatar(&human.avatar_seed(), 24.0))
                                            .child(
                                                div()
                                                    .tw_text_sm()
                                                    .text_color(theme.foreground)
                                                    .child(SharedString::from(
                                                        human.display_name(),
                                                    )),
                                            )
                                    }))
                                    .into_any_element()
                            }),
                    ),
            )
            .into_any_element()
    }
}
