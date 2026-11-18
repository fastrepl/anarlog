//! The contacts tab, ported from shipping's `contacts/*`, `crm/enrich-contact.tsx` and
//! `sidebar/contacts.tsx`: people and organizations in the custom sidebar, and the person or
//! organization page on the main surface. Edits save as they are typed, like shipping.

pub mod assets;
pub(crate) mod avatar;
mod data;
pub(crate) mod identity;
mod list;
pub(crate) mod ops;
mod organization;
mod person;

use std::sync::Arc;

use desktop_runtime::{CancellationToken, RuntimeHandle, SessionId};
use futures::{
    FutureExt,
    future::{Either, select},
};
use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Hsla, KeyDownEvent, Render,
    ScrollHandle, SharedString, Window, anchored, deferred, div, prelude::*, px, rgb, svg,
};
use serde_json::Value;

pub use avatar::facehash;
pub use data::Selection;
use data::{ContactsData, RelatedNote};
use list::ContactsList;
use ops::{Kind, Write};
use organization::OrganizationPage;
use person::PersonPage;

use super::sidebar::{floating_panel, floating_panel_body};
use crate::ui::a11y::Accessible;
use crate::ui::theme::{Theme, theme};

pub enum ContactsEvent {
    Back,
    OpenNote(SessionId),
    OpenSettings(&'static str),
}

enum Page {
    Empty,
    Person(Box<PersonPage>),
    Organization(OrganizationPage),
}

impl Page {
    fn selection(&self) -> Option<Selection> {
        match self {
            Self::Empty => None,
            Self::Person(page) => Some(Selection::Person(page.id.clone())),
            Self::Organization(page) => Some(Selection::Organization(page.id.clone())),
        }
    }
}

pub struct Contacts {
    runtime: RuntimeHandle,
    viewer: Option<Arc<str>>,
    data: Arc<ContactsData>,
    selected: Option<Selection>,
    list: Entity<ContactsList>,
    page: Page,
    /// The page header's options menu.
    menu: bool,
    menu_dismissal: Dismissal,
    /// The CRM prompt under the Enrich button.
    prompt: bool,
    crm: crate::product::crm::Enrichment,
    scroll: ScrollHandle,
    focus: FocusHandle,
    message: SharedString,
    watching: bool,
    watch_cancel: CancellationToken,
    notes_cancel: CancellationToken,
}

impl EventEmitter<ContactsEvent> for Contacts {}

// Tailwind's blue-600/700 and red-50/600/700/800/900 in shipping's palette.
pub(super) const BLUE_600: u32 = 0x155dfc;
pub(super) const BLUE_700: u32 = 0x1447e6;
const RED_50: u32 = 0xfef2f2;
const RED_600: u32 = 0xe7000b;
const RED_700: u32 = 0xc10007;
const RED_800: u32 = 0x9f0712;
const RED_900: u32 = 0x82181a;

pub(super) fn text_xs<E: Styled>(view: E) -> E {
    view.text_size(px(12.)).line_height(px(16.))
}

pub(super) fn text_sm<E: Styled>(view: E) -> E {
    view.text_size(px(14.)).line_height(px(20.))
}

/// Shipping's ghost `Button size="icon"`: 28px, muted until hovered.
pub(super) fn icon_button(
    id: impl Into<SharedString>,
    icon: &'static str,
    icon_size: f32,
    colors: Theme,
) -> gpui::Stateful<gpui::Div> {
    menu_trigger(id, icon, icon_size, false, colors)
}

/// An `icon_button` that opens a menu. Radix's modal menus block hover while they are open.
pub(super) fn menu_trigger(
    id: impl Into<SharedString>,
    icon: &'static str,
    icon_size: f32,
    open: bool,
    colors: Theme,
) -> gpui::Stateful<gpui::Div> {
    let id = id.into();
    let icon = svg()
        .path(icon)
        .size(px(icon_size))
        .text_color(colors.muted_foreground);
    div()
        .id(id.clone())
        .group(id.clone())
        .size(px(28.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .map(|button| {
            if open {
                button.child(icon)
            } else {
                button
                    .hover(move |style| style.bg(colors.accent))
                    .child(icon.group_hover(id, move |style| style.text_color(colors.foreground)))
            }
        })
}

/// Radix closes an open menu when its trigger is clicked again. GPUI closes it on the outside
/// mouse-down first, so the trigger's click must not reopen it.
#[derive(Default)]
pub(super) struct Dismissal(Option<gpui::Point<gpui::Pixels>>);

impl Dismissal {
    pub fn record(&mut self, event: &gpui::MouseDownEvent) {
        self.0 = Some(event.position);
    }

    /// Whether `event` is the click whose press just closed the menu.
    pub fn closed_by(&mut self, event: &gpui::ClickEvent) -> bool {
        let pressed = match event {
            gpui::ClickEvent::Mouse(click) => Some(click.down.position),
            _ => None,
        };
        let dismissed = self.0.take();
        pressed.is_some() && pressed == dismissed
    }
}

/// A dropdown anchored under its trigger: `align="start"` or `align="end"`.
pub(super) fn dropdown(end: bool, top: f32, menu: impl IntoElement) -> gpui::Div {
    let anchor = div().absolute().top(px(top));
    let anchor = if end {
        anchor.right_0()
    } else {
        anchor.left_0()
    };
    anchor.child(deferred(
        anchored()
            .anchor(if end {
                gpui::Anchor::TopRight
            } else {
                gpui::Anchor::TopLeft
            })
            .snap_to_window()
            .child(menu),
    ))
}

/// `DropdownMenuRadioItem`: a dot marks the chosen value.
pub(super) fn radio_item(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    checked: bool,
    small: bool,
    colors: Theme,
) -> gpui::Stateful<gpui::Div> {
    let item = div()
        .id(id)
        .a11y_menu_radio(label, checked)
        .relative()
        .py(px(6.))
        .pl(px(32.))
        .pr_2()
        .flex()
        .items_center()
        .text_color(colors.popover_foreground)
        .cursor_pointer()
        .hover(move |style| style.bg(colors.accent))
        .when(checked, |item| {
            item.child(
                div()
                    .absolute()
                    .left(px(8.))
                    .size(px(14.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().size(px(8.)).bg(colors.popover_foreground)),
            )
        })
        .child(label);
    if small { text_xs(item) } else { text_sm(item) }
}

impl Contacts {
    pub fn new(runtime: RuntimeHandle, cx: &mut Context<Self>) -> Self {
        let contacts = cx.entity().downgrade();
        let list = cx.new(|cx| ContactsList::new(contacts, cx));
        Self {
            runtime,
            viewer: None,
            data: Arc::default(),
            selected: None,
            list,
            page: Page::Empty,
            menu: false,
            menu_dismissal: Dismissal::default(),
            prompt: false,
            crm: Default::default(),
            scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            message: SharedString::default(),
            watching: false,
            watch_cancel: CancellationToken::new(),
            notes_cancel: CancellationToken::new(),
        }
    }

    pub fn list(&self) -> Entity<ContactsList> {
        self.list.clone()
    }

    pub fn set_viewer(&mut self, viewer: Option<Arc<str>>, cx: &mut Context<Self>) {
        if self.viewer != viewer {
            self.viewer = viewer;
            self.list.update(cx, |_, cx| cx.notify());
            cx.notify();
        }
    }

    pub fn activate(&mut self, cx: &mut Context<Self>) {
        if self.watching {
            return;
        }
        self.watching = true;
        self.load_crm(cx);
        self.watch_cancel.cancel();
        self.watch_cancel = CancellationToken::new();
        let cancel = self.watch_cancel.clone();
        Self::watch(
            &self.runtime,
            data::CONTACTS_CHANGES_SQL,
            Some(data::CONTACTS_SQL),
            vec![],
            cancel,
            cx,
            |this, rows, cx| {
                let data = Arc::new(data::decode(&rows));
                if data != this.data {
                    this.data = data;
                    this.list.update(cx, |_, cx| cx.notify());
                    cx.notify();
                }
            },
        );
        if let Page::Person(page) = &self.page {
            let id = page.id.clone();
            self.watch_notes(id, cx);
        }
    }

    pub fn suspend(&mut self) {
        self.watching = false;
        self.watch_cancel.cancel();
        self.notes_cancel.cancel();
    }

    /// Opens `selection`, or yourself (then the first contact) when it is `None`.
    pub fn select(&mut self, selection: Option<Selection>, cx: &mut Context<Self>) {
        self.selected = selection;
        // The list's own row click lands here while it is being updated, so it can't be leased.
        gpui::App::notify(cx, self.list.entity_id());
        cx.notify();
    }

    fn owner(&self) -> SharedString {
        self.data.owner(self.viewer.as_deref())
    }

    fn effective_selection(&self) -> Option<Selection> {
        self.data
            .effective_selection(self.selected.as_ref(), &self.owner())
    }

    fn write(&mut self, write: Write, cx: &mut Context<Self>) {
        let reply = ops::submit(&self.runtime, write);
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            if let Err(error) = result {
                tracing::error!(%error, "contacts write failed");
                let _ = this.update(cx, |this, cx| {
                    this.message = format!("Could not save the contact: {error}").into();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn toggle_pin(&mut self, kind: Kind, id: SharedString, cx: &mut Context<Self>) {
        self.write(Write::TogglePin(kind, id.to_string()), cx);
    }

    fn delete(&mut self, kind: Kind, id: SharedString, cx: &mut Context<Self>) {
        self.write(Write::Delete(kind, id.to_string()), cx);
        self.select(None, cx);
    }

    fn create_person(&mut self, name: String, cx: &mut Context<Self>) {
        let id = uuid::Uuid::new_v4().to_string();
        self.write(
            Write::CreateHuman {
                id: id.clone(),
                owner: data::LOCAL_OWNER.into(),
                name,
            },
            cx,
        );
        self.select(Some(Selection::Person(id.into())), cx);
    }

    fn set_avatar(
        &mut self,
        kind: Kind,
        id: SharedString,
        avatar: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.write(Write::Avatar(kind, id.to_string(), avatar), cx);
    }

    /// `AvatarUploadButton`: picks an image and stores a centred, at most 256px JPEG.
    fn upload_avatar(&mut self, kind: Kind, id: SharedString, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let encoded = cx
                .background_spawn(async move { avatar_data_url(&std::fs::read(path)?) })
                .await;
            let _ = this.update(cx, |this, cx| match encoded {
                Ok(url) => this.set_avatar(kind, id, Some(url), cx),
                Err(error) => {
                    tracing::error!(%error, "could not process the avatar image");
                }
            });
        })
        .detach();
    }

    fn watch_notes(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.notes_cancel.cancel();
        self.notes_cancel = CancellationToken::new();
        let cancel = self.notes_cancel.clone();
        Self::watch(
            &self.runtime,
            data::NOTES_SQL,
            None,
            vec![Value::from(id.to_string())],
            cancel,
            cx,
            move |this, rows, cx| {
                let notes = data::decode_notes(&rows);
                if let Page::Person(page) = &mut this.page
                    && page.id == id
                    && (page.notes.as_deref() != Some(notes.as_slice()))
                {
                    page.notes = Some(notes);
                    cx.notify();
                }
            },
        );
    }

    /// With `read`, `sql` only signals that the tables `read` reads changed, and each change
    /// reads `read` in full: contacts outgrow a watch's 1,000 rows.
    fn watch(
        runtime: &RuntimeHandle,
        sql: &'static str,
        read: Option<&'static str>,
        params: Vec<Value>,
        cancel: CancellationToken,
        cx: &mut Context<Self>,
        apply: impl Fn(&mut Self, Arc<[Value]>, &mut Context<Self>) + 'static,
    ) {
        let reply = match read {
            Some(_) => runtime.watch_changes(sql.into(), params.clone()),
            None => runtime.watch_query(sql.into(), params.clone()),
        };
        let runtime = runtime.clone();
        cx.spawn(async move |this, cx| {
            let fail = |this: &mut Self, error: String, cx: &mut Context<Self>| {
                this.message = format!("Could not load contacts: {error}").into();
                cx.notify();
            };
            let mut watch = match reply {
                Ok(reply) => match reply.receive().await {
                    Ok(watch) => watch,
                    Err(error) => {
                        let _ = this.update(cx, |this, cx| fail(this, error.to_string(), cx));
                        return;
                    }
                },
                Err(error) => {
                    let _ = this.update(cx, |this, cx| fail(this, error.to_string(), cx));
                    return;
                }
            };
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                let mut rows = watch.snapshots.borrow_and_update().rows.clone();
                let mut error = watch
                    .terminal_error()
                    .or_else(|| watch.errors.try_recv().ok());
                if let (Some(read), None) = (read, &error) {
                    match async {
                        runtime
                            .read_rows(read.into(), params.clone())?
                            .receive()
                            .await
                    }
                    .await
                    {
                        Ok(read) => rows = read.into(),
                        Err(failed) => error = Some(failed),
                    }
                }
                let updated = this.update(cx, |this, cx| {
                    if cancel.is_cancelled() {
                        return;
                    }
                    match error {
                        Some(error) => fail(this, error.to_string(), cx),
                        None => apply(this, rows, cx),
                    }
                });
                if updated.is_err() || watch.terminal_error().is_some() {
                    break;
                }
                match select(
                    watch.snapshots.changed().boxed(),
                    cancel.cancelled().boxed(),
                )
                .await
                {
                    Either::Left((Ok(()), _)) => {}
                    _ => break,
                }
            }
            let _ = watch.unsubscribe().await;
        })
        .detach();
    }

    /// Rebuilds the page's inputs when the shown contact changes, like shipping's `key`.
    fn sync_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selection = self.effective_selection();
        let exists = match &selection {
            Some(Selection::Person(id)) => self.data.human(id).is_some(),
            Some(Selection::Organization(id)) => self.data.organization(id).is_some(),
            None => false,
        };
        let target = selection.filter(|_| exists);
        if self.page.selection() == target {
            if let Page::Person(page) = &mut self.page
                && let Some(human) = self.data.human(&page.id)
            {
                page.sync_fields(human, window, cx);
                page.sync_emails(human, window, cx);
            }
            return;
        }
        self.menu = false;
        self.prompt = false;
        self.scroll = ScrollHandle::new();
        self.notes_cancel.cancel();
        self.page = match target {
            Some(Selection::Person(id)) => {
                let human = self.data.human(&id).cloned().unwrap_or_default();
                let page = PersonPage::new(&human, window, cx);
                self.watch_notes(id, cx);
                Page::Person(Box::new(page))
            }
            Some(Selection::Organization(id)) => {
                let organization = self.data.organization(&id).cloned().unwrap_or_default();
                Page::Organization(OrganizationPage::new(&organization, cx))
            }
            None => Page::Empty,
        };
    }

    /// `ContactPageHeader`.
    #[allow(clippy::too_many_arguments)]
    fn header(
        &self,
        title: SharedString,
        compact: impl IntoElement,
        read_only: bool,
        pinned: bool,
        avatar: bool,
        actions: Option<AnyElement>,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let compact_identity = self.scroll.offset().y < px(0.);
        div()
            .h(px(48.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .pr_1()
            .pl_3()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(compact_identity, |view| view.child(compact))
                    .child(
                        text_sm(div())
                            .min_w_0()
                            .truncate()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title),
                    ),
            )
            .when(!read_only, |view| {
                view.child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap_1()
                        .children(actions)
                        .child(
                            div()
                                .relative()
                                .child(
                                    menu_trigger(
                                        "contact-options",
                                        "workspace/MoreHorizontalIcon.svg",
                                        16.,
                                        self.menu,
                                        colors,
                                    )
                                    .a11y_button("Contact options")
                                    .aria_expanded(self.menu)
                                    .on_click(cx.listener(
                                        |this, event, _, cx| {
                                            if !this.menu_dismissal.closed_by(event) {
                                                this.menu = !this.menu;
                                            }
                                            cx.notify();
                                        },
                                    )),
                                )
                                .when(self.menu, |view| {
                                    view.child(dropdown(
                                        true,
                                        32.,
                                        self.options_menu(pinned, avatar, colors, cx),
                                    ))
                                }),
                        ),
                )
            })
    }

    fn options_menu(
        &self,
        pinned: bool,
        avatar: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selection = self.page.selection();
        let (kind, id) = match selection {
            Some(Selection::Person(id)) => (Kind::Human, id),
            Some(Selection::Organization(id)) => (Kind::Organization, id),
            None => (Kind::Human, SharedString::default()),
        };
        // `DropdownMenuItem`; Delete swaps the accent hover for red.
        let item = |id: &'static str, icon: &'static str, label: &'static str, danger: bool| {
            let color = if danger {
                red(600)
            } else {
                colors.popover_foreground
            };
            div()
                .id(id)
                .a11y_menu_item(label)
                .group(id)
                .h(px(32.))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .line_height(px(20.))
                .text_color(color)
                .cursor_pointer()
                .hover(move |style| {
                    if danger {
                        style.bg(red(50)).text_color(red(700))
                    } else {
                        style.bg(colors.accent)
                    }
                })
                .child(
                    svg()
                        .path(icon)
                        .size(px(16.))
                        .flex_shrink_0()
                        .text_color(color)
                        .when(danger, |icon| {
                            icon.group_hover(id, |style| style.text_color(red(700)))
                        }),
                )
                .child(label)
        };
        let (pin_id, remove_id, delete_id) = (id.clone(), id.clone(), id);
        floating_panel(192., colors)
            .id("contact-options-menu")
            .occlude()
            .on_mouse_down_out(cx.listener(|this, event, _, cx| {
                this.menu = false;
                this.menu_dismissal.record(event);
                cx.notify();
            }))
            .child(
                floating_panel_body(colors)
                    .child(
                        item(
                            "contact-pin",
                            if pinned {
                                "contacts/PinIconBold.svg"
                            } else {
                                "workspace/PinIcon.svg"
                            },
                            if pinned { "Unpin" } else { "Pin" },
                            false,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.menu = false;
                            this.toggle_pin(kind, pin_id.clone(), cx);
                        })),
                    )
                    .when(avatar, |menu| {
                        menu.child(
                            item(
                                "contact-remove-photo",
                                "contacts/CircleMinusIcon.svg",
                                "Remove photo",
                                false,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.menu = false;
                                    this.set_avatar(kind, remove_id.clone(), None, cx);
                                },
                            )),
                        )
                    })
                    .child(div().my_1().mx(px(-4.)).h(px(1.)).bg(colors.muted))
                    .child(
                        item("contact-delete", "chat/TrashIcon.svg", "Delete", true).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.menu = false;
                                this.delete(kind, delete_id.clone(), cx);
                            }),
                        ),
                    ),
            )
    }

    /// `AvatarUploadButton` around a 64px avatar: a camera veil on hover opens the picker.
    fn avatar_upload(
        &self,
        kind: Kind,
        id: SharedString,
        avatar: gpui::Div,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("change-photo")
            .a11y_button("Change photo")
            .group("change-photo")
            .relative()
            .cursor_pointer()
            .child(avatar)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::rgba(0x00000066))
                    .opacity(0.)
                    .group_hover("change-photo", |style| style.opacity(1.))
                    .child(
                        svg()
                            .path("contacts/Camera01Icon.svg")
                            .size(px(20.))
                            .text_color(gpui::white()),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.upload_avatar(kind, id.clone(), cx)))
    }
}

/// `compressAvatarImage`: centre-crops, scales to at most 256px and flattens onto white.
fn avatar_data_url(bytes: &[u8]) -> anyhow::Result<String> {
    use base64::Engine;
    let image = image::load_from_memory(bytes)?;
    let side = image.width().min(image.height());
    anyhow::ensure!(side > 0, "image has no pixels");
    let square = image.crop_imm(
        (image.width() - side) / 2,
        (image.height() - side) / 2,
        side,
        side,
    );
    let output = side.min(256);
    let scaled = square
        .resize_exact(output, output, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let mut flat = image::RgbImage::new(output, output);
    for (x, y, pixel) in scaled.enumerate_pixels() {
        let alpha = pixel[3] as u32;
        let blend = |channel: u8| ((channel as u32 * alpha + 255 * (255 - alpha)) / 255) as u8;
        flat.put_pixel(
            x,
            y,
            image::Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])]),
        );
    }
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 95).encode_image(&flat)?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(jpeg)
    ))
}

impl Drop for Contacts {
    fn drop(&mut self) {
        self.watch_cancel.cancel();
        self.notes_cancel.cancel();
    }
}

impl Render for Contacts {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_page(window, cx);
        let colors = theme(window);
        let content = match &self.page {
            Page::Person(_) => self.person(window, cx).into_any_element(),
            Page::Organization(_) => self.organization(window, cx).into_any_element(),
            Page::Empty => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(text_sm(div()).text_color(colors.muted_foreground).child(
                    match self.effective_selection() {
                        Some(Selection::Organization(_)) => {
                            "Select an organization to view details"
                        }
                        _ => "Select a person to view details",
                    },
                ))
                .into_any_element(),
        };
        div()
            .id("contacts-page")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.secondary(cx) && event.keystroke.key == "f" {
                    let list = this.list.clone();
                    list.update(cx, |list, cx| list.focus_search(window, cx));
                    cx.stop_propagation();
                }
            }))
            .child(content)
            .when(!self.message.is_empty(), |view| {
                view.child(
                    text_xs(div())
                        .px_3()
                        .py_1()
                        .text_color(colors.destructive)
                        .child(self.message.clone()),
                )
            })
    }
}

/// `RelatedNotesSection`'s note dates and the summary placeholder share these helpers.
pub(super) fn related_note_label(note: &RelatedNote) -> SharedString {
    if note.title.is_empty() {
        "Untitled Note".into()
    } else {
        note.title.clone()
    }
}

pub(super) fn red(shade: u32) -> Hsla {
    rgb(match shade {
        50 => RED_50,
        700 => RED_700,
        800 => RED_800,
        900 => RED_900,
        _ => RED_600,
    })
    .into()
}

pub(super) fn focus_input(
    input: &Entity<crate::ui::input::TextInput>,
    window: &mut Window,
    cx: &mut App,
) {
    use gpui::Focusable;
    input.focus_handle(cx).focus(window, cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn contacts_load_and_follow_changes_past_a_watchs_row_limit(cx: &mut gpui::TestAppContext) {
        use futures::executor::block_on;

        cx.executor().allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        block_on(ready.receive()).unwrap();
        let insert = |sql: &'static str| {
            let reply = runtime
                .submit(move |services| async move {
                    sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
                        .execute(services.db.pool())
                        .await
                        .map_err(|error| {
                            desktop_runtime::ServiceError::Failed(error.to_string().into())
                        })?;
                    Ok(())
                })
                .unwrap();
            block_on(reply.receive()).unwrap();
        };
        insert(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 1001)
             INSERT INTO humans (id, name) SELECT 'h' || i, 'Person ' || i FROM n",
        );
        let view = cx.new(|cx| {
            let mut view = Contacts::new(runtime.clone(), cx);
            view.activate(cx);
            view
        });
        let humans =
            |cx: &mut gpui::TestAppContext| view.read_with(cx, |view, _| view.data.humans.len());
        let settle = |cx: &mut gpui::TestAppContext, count: usize| {
            for _ in 0..400 {
                cx.run_until_parked();
                if humans(cx) == count {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        settle(cx, 1001);
        assert_eq!(humans(cx), 1001);
        insert("INSERT INTO humans (id, name) VALUES ('h-new', 'Newcomer')");
        settle(cx, 1002);
        assert_eq!(humans(cx), 1002);
        view.read_with(cx, |view, _| {
            assert!(view.message.is_empty(), "{}", view.message)
        });
        drop(view);
        block_on(runtime.shutdown()).unwrap();
    }

    #[test]
    fn clicking_an_open_menus_trigger_closes_it_instead_of_reopening() {
        let press = |x: f32| gpui::MouseDownEvent {
            button: gpui::MouseButton::Left,
            position: gpui::point(px(x), px(10.)),
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        };
        let click = |x: f32| {
            gpui::ClickEvent::Mouse(gpui::MouseClickEvent {
                down: press(x),
                up: gpui::MouseUpEvent {
                    button: gpui::MouseButton::Left,
                    position: gpui::point(px(x), px(10.)),
                    modifiers: Default::default(),
                    click_count: 1,
                },
            })
        };
        let mut dismissal = Dismissal::default();
        dismissal.record(&press(5.));
        assert!(dismissal.closed_by(&click(5.)));
        assert!(!dismissal.closed_by(&click(5.)));
        // A dismissal elsewhere does not swallow a later trigger click.
        dismissal.record(&press(300.));
        assert!(!dismissal.closed_by(&click(5.)));
    }

    #[test]
    fn uploaded_photos_are_centred_square_jpegs_of_at_most_256_pixels() {
        let mut source = image::RgbaImage::new(600, 300);
        for (x, _, pixel) in source.enumerate_pixels_mut() {
            *pixel = image::Rgba([if x < 300 { 255 } else { 0 }, 0, 0, 0]);
        }
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(source)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let url = avatar_data_url(&png).unwrap();
        let encoded = url.strip_prefix("data:image/jpeg;base64,").unwrap();
        use base64::Engine;
        let jpeg = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let decoded = image::load_from_memory(&jpeg).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (256, 256));
        // Fully transparent pixels flatten onto white.
        assert!(
            decoded
                .get_pixel(128, 128)
                .0
                .iter()
                .all(|channel| *channel > 240)
        );
    }
}
