//! `DetailsColumn` from shipping's `contacts/details.tsx`, with `RelatedNotesSection`,
//! `ContactOrganizationSelector` and the CRM `EnrichContactButton`.

use gpui::{
    AnyElement, Context, Entity, EntityId, Focusable, KeyDownEvent, SharedString, Subscription,
    Window, div, prelude::*, px, svg,
};

use super::{
    Contacts, ContactsEvent, Dismissal, Page, avatar,
    data::{Human, RelatedNote, note_date, visible_notes},
    dropdown, focus_input, icon_button, menu_trigger,
    ops::{Field, Kind, Write},
    radio_item, red, related_note_label, text_sm, text_xs,
};
use crate::{
    ui::{
        input::{InputEvent, TextInput},
        theme::{Theme, theme},
    },
    workspace::sidebar::{floating_panel, floating_panel_body},
};

const FIELDS: [(Field, &str, &str); 6] = [
    (Field::Name, "Name", "Name"),
    (Field::JobTitle, "Job Title", "Software Engineer"),
    (Field::Email, "Email", "john@example.com"),
    (Field::Phone, "Phone", "+1 (555) 123-4567"),
    (
        Field::LinkedIn,
        "LinkedIn",
        "https://www.linkedin.com/in/johntopia/",
    ),
    (Field::Memo, "Notes", "Add notes about this contact..."),
];

const SUMMARY_PLACEHOLDER: &str = "AI-generated summary of all interactions and notes with this contact will appear here. This will synthesize key discussion points, action items, and relationship context across all meetings and notes.";

struct EmailRow {
    committed: SharedString,
    input: Entity<TextInput>,
    _subscriptions: [Subscription; 2],
}

struct OrganizationPicker {
    search: Entity<TextInput>,
    highlighted: Option<usize>,
    _subscription: Subscription,
}

pub(super) struct PersonPage {
    pub id: SharedString,
    fields: Vec<(Field, Entity<TextInput>)>,
    emails: Vec<EmailRow>,
    new_email: Option<EmailRow>,
    pub notes: Option<Vec<RelatedNote>>,
    notes_search: Entity<TextInput>,
    notes_newest: bool,
    notes_menu: bool,
    notes_dismissal: Dismissal,
    picker: Option<OrganizationPicker>,
    picker_dismissal: Dismissal,
    _subscriptions: Vec<Subscription>,
}

impl PersonPage {
    pub fn new(human: &Human, window: &mut Window, cx: &mut Context<Contacts>) -> Self {
        let mut subscriptions = Vec::new();
        let fields = FIELDS
            .iter()
            .map(|&(field, _, placeholder)| {
                let value = match field {
                    Field::Name => &human.name,
                    Field::JobTitle => &human.job_title,
                    Field::Email => &human.email,
                    Field::Phone => &human.phone,
                    Field::LinkedIn => &human.linkedin,
                    Field::Memo => &human.memo,
                    Field::Organization => &human.organization_id,
                };
                let input = cx.new(|cx| {
                    let mut input = TextInput::new(placeholder, cx).inline();
                    if field == Field::Memo {
                        input = input.multiline();
                    }
                    input.set_text(value.to_string(), cx);
                    input
                });
                let id = human.id.clone();
                subscriptions.push(cx.subscribe(&input, move |this, input, event, cx| {
                    if matches!(event, InputEvent::Changed) {
                        let value = input.read(cx).buffer.text.clone();
                        this.write(
                            Write::UpdateHuman {
                                id: id.to_string(),
                                field,
                                value,
                            },
                            cx,
                        );
                    }
                }));
                (field, input)
            })
            .collect();
        let notes_search = cx.new(|cx| TextInput::new("Search...", cx).inline());
        subscriptions.push(cx.observe(&notes_search, |_, _, cx| cx.notify()));
        let mut page = Self {
            id: human.id.clone(),
            fields,
            emails: Vec::new(),
            new_email: None,
            notes: None,
            notes_search,
            notes_newest: true,
            notes_menu: false,
            notes_dismissal: Dismissal::default(),
            picker: None,
            picker_dismissal: Dismissal::default(),
            _subscriptions: subscriptions,
        };
        page.sync_emails(human, window, cx);
        page
    }

    /// Keeps one input per stored additional email, reusing inputs whose value is unchanged.
    pub fn sync_emails(&mut self, human: &Human, window: &mut Window, cx: &mut Context<Contacts>) {
        if self
            .emails
            .iter()
            .map(|row| &row.committed)
            .eq(human.additional_emails.iter())
        {
            return;
        }
        let mut previous = std::mem::take(&mut self.emails);
        self.emails = human
            .additional_emails
            .iter()
            .map(
                |email| match previous.iter().position(|row| &row.committed == email) {
                    Some(index) => previous.remove(index),
                    None => email_row(email.clone(), window, cx),
                },
            )
            .collect();
    }

    fn committed_emails(&self) -> Vec<String> {
        self.emails
            .iter()
            .map(|row| row.committed.to_string())
            .collect()
    }
}

fn email_row(value: SharedString, window: &mut Window, cx: &mut Context<Contacts>) -> EmailRow {
    let input = cx.new(|cx| {
        let mut input = TextInput::new("jane@example.com", cx).inline();
        input.set_text(value.to_string(), cx);
        input
    });
    let entity = input.entity_id();
    let handle = input.focus_handle(cx);
    let blur = cx.on_focus_out(&handle, window, move |this, _, _, cx| {
        this.commit_email(entity, cx)
    });
    // Enter leaves the field, which commits it.
    let submit = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
        if matches!(event, InputEvent::Submitted) {
            this.focus.focus(window, cx);
        }
    });
    EmailRow {
        committed: value,
        input,
        _subscriptions: [blur, submit],
    }
}

impl Contacts {
    fn person_page(&mut self) -> Option<&mut PersonPage> {
        match &mut self.page {
            Page::Person(page) => Some(page),
            _ => None,
        }
    }

    /// `EditableAdditionalEmailRow`'s blur: rename, remove when emptied, or add the new row.
    fn commit_email(&mut self, input: EntityId, cx: &mut Context<Self>) {
        let Some(page) = self.person_page() else {
            return;
        };
        let id = page.id.to_string();
        if page
            .new_email
            .as_ref()
            .is_some_and(|row| row.input.entity_id() == input)
        {
            let row = page.new_email.take().unwrap();
            let value = row.input.read(cx).buffer.text.trim().to_owned();
            if !value.is_empty() {
                let mut emails = page.committed_emails();
                emails.push(value.clone());
                page.emails.push(EmailRow {
                    committed: value.into(),
                    ..row
                });
                self.write(Write::AdditionalEmails { id, emails }, cx);
            }
            cx.notify();
            return;
        }
        let Some(index) = page
            .emails
            .iter()
            .position(|row| row.input.entity_id() == input)
        else {
            return;
        };
        let value = page.emails[index]
            .input
            .read(cx)
            .buffer
            .text
            .trim()
            .to_owned();
        if page.emails[index].committed == value {
            return;
        }
        if value.is_empty() {
            page.emails.remove(index);
        } else {
            page.emails[index].committed = value.into();
        }
        let emails = page.committed_emails();
        self.write(Write::AdditionalEmails { id, emails }, cx);
        cx.notify();
    }

    fn remove_email(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(page) = self.person_page() else {
            return;
        };
        if index >= page.emails.len() {
            return;
        }
        page.emails.remove(index);
        let (id, emails) = (page.id.to_string(), page.committed_emails());
        self.write(Write::AdditionalEmails { id, emails }, cx);
        cx.notify();
    }

    fn add_email(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let row = email_row(SharedString::default(), window, cx);
        focus_input(&row.input, window, cx);
        if let Some(page) = self.person_page() {
            page.new_email = Some(row);
        }
        cx.notify();
    }

    fn set_organization(&mut self, organization: String, cx: &mut Context<Self>) {
        let Some(page) = self.person_page() else {
            return;
        };
        page.picker = None;
        let id = page.id.to_string();
        self.write(
            Write::UpdateHuman {
                id,
                field: Field::Organization,
                value: organization,
            },
            cx,
        );
        cx.notify();
    }

    fn create_organization(&mut self, name: String, cx: &mut Context<Self>) {
        let organization = uuid::Uuid::new_v4().to_string();
        self.write(
            Write::CreateOrganization {
                id: organization.clone(),
                owner: super::data::LOCAL_OWNER.into(),
                name,
            },
            cx,
        );
        self.set_organization(organization, cx);
    }

    fn open_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = cx.new(|cx| TextInput::new("Search or add company", cx).inline());
        let subscription = cx.subscribe(&search, |this, _, event, cx| match event {
            InputEvent::Changed => {
                if let Some(picker) = this.person_page().and_then(|page| page.picker.as_mut()) {
                    picker.highlighted = None;
                }
                cx.notify();
            }
            InputEvent::Submitted => this.choose_highlighted(cx),
            InputEvent::Rejected => {}
        });
        focus_input(&search, window, cx);
        if let Some(page) = self.person_page() {
            page.picker = Some(OrganizationPicker {
                search,
                highlighted: None,
                _subscription: subscription,
            });
        }
        cx.notify();
    }

    /// `OrganizationControl`'s matches and whether "Create" is offered.
    fn picker_options(
        &self,
        cx: &gpui::App,
    ) -> (Vec<(SharedString, SharedString)>, Option<String>) {
        let Page::Person(page) = &self.page else {
            return (Vec::new(), None);
        };
        let Some(picker) = &page.picker else {
            return (Vec::new(), None);
        };
        let term = picker.search.read(cx).buffer.text.clone();
        let query = term.to_lowercase();
        let matches = self
            .data
            .organizations
            .iter()
            .filter(|organization| {
                term.trim().is_empty() || organization.name.to_lowercase().contains(&query)
            })
            .map(|organization| (organization.id.clone(), organization.name.clone()))
            .collect::<Vec<_>>();
        let create =
            (!term.trim().is_empty() && matches.is_empty()).then(|| term.trim().to_owned());
        (matches, create)
    }

    fn choose_highlighted(&mut self, cx: &mut Context<Self>) {
        let (matches, create) = self.picker_options(cx);
        let highlighted = self
            .person_page()
            .and_then(|page| page.picker.as_ref())
            .and_then(|picker| picker.highlighted);
        match highlighted {
            Some(index) if index < matches.len() => {
                self.set_organization(matches[index].0.to_string(), cx)
            }
            _ => {
                if let Some(name) = create {
                    self.create_organization(name, cx);
                }
            }
        }
    }

    fn move_highlight(&mut self, down: bool, cx: &mut Context<Self>) {
        let (matches, create) = self.picker_options(cx);
        let count = matches.len() + usize::from(create.is_some());
        if let Some(picker) = self.person_page().and_then(|page| page.picker.as_mut())
            && count > 0
        {
            picker.highlighted = Some(match (picker.highlighted, down) {
                (Some(index), true) if index + 1 < count => index + 1,
                (_, true) => 0,
                (Some(index), false) if index > 0 && index < count => index - 1,
                (_, false) => count - 1,
            });
            cx.notify();
        }
    }

    pub(super) fn person(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let Page::Person(page) = &self.page else {
            return div().into_any_element();
        };
        let human = self.data.human(&page.id).cloned().unwrap_or_default();
        let owner = self.owner();
        let read_only = human.id == owner;
        let name = human.avatar_name();
        let compact = avatar::person(human.avatar.as_ref(), &name, 24.);
        let large = avatar::person(human.avatar.as_ref(), &name, 64.);
        let organization = self.data.organization(&human.organization_id).cloned();
        let duplicates = if read_only {
            Vec::new()
        } else {
            self.data
                .duplicates(&human, &owner)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
        };
        let notes = page.notes.clone().unwrap_or_default();
        let enrich = (!read_only).then(|| self.enrich(colors, cx).into_any_element());
        let header = self.header(
            human.display_name(),
            compact,
            read_only,
            human.pinned,
            human.avatar.is_some(),
            enrich,
            colors,
            cx,
        );
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .id("contact-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll(cx.listener(|_, _, _, cx| cx.notify()))
                    .child(
                        div()
                            .py_6()
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_b_1()
                            .border_color(colors.border)
                            .map(|view| {
                                if read_only {
                                    view.child(large)
                                } else {
                                    view.child(self.avatar_upload(
                                        Kind::Human,
                                        human.id.clone(),
                                        large,
                                        cx,
                                    ))
                                }
                            }),
                    )
                    .when(!duplicates.is_empty(), |view| {
                        view.child(self.duplicates(&duplicates, colors, cx))
                    })
                    .child(if read_only {
                        self.read_only_fields(
                            &human,
                            organization.as_ref().map(|o| o.name.clone()),
                            colors,
                        )
                        .into_any_element()
                    } else {
                        self.fields(colors, cx).into_any_element()
                    })
                    .when(!notes.is_empty(), |view| {
                        view.child(summary(&human.summary, colors))
                    })
                    .child(self.related_notes(&notes, window, colors, cx))
                    .child(div().h(px(384.))),
            )
            .into_any_element()
    }

    fn read_only_fields(
        &self,
        human: &Human,
        company: Option<SharedString>,
        colors: Theme,
    ) -> impl IntoElement {
        let emails = std::iter::once(&human.email)
            .chain(&human.additional_emails)
            .filter(|email| !email.trim().is_empty())
            .map(|email| email.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let rows: [(&str, SharedString); 7] = [
            ("Name", human.name.clone()),
            ("Job Title", human.job_title.clone()),
            ("Company", company.unwrap_or_default()),
            ("Email", emails.into()),
            ("Phone", human.phone.clone()),
            ("LinkedIn", human.linkedin.clone()),
            ("Notes", human.memo.clone()),
        ];
        div().children(rows.into_iter().map(|(label, value)| {
            field_row(Some(label), colors).child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(16.))
                    .line_height(px(24.))
                    .when(!value.is_empty(), |view| {
                        view.children(
                            value
                                .split('\n')
                                .map(|line| div().child(SharedString::from(line.to_owned()))),
                        )
                    }),
            )
        }))
    }

    fn fields(&self, colors: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let Page::Person(page) = &self.page else {
            return div();
        };
        let input = |field: Field| {
            page.fields
                .iter()
                .find(|(candidate, _)| *candidate == field)
                .map(|(_, input)| input.clone())
                .unwrap()
        };
        let single = |field: Field, label: &'static str| {
            field_row(Some(label), colors).child(input_box(input(field)))
        };
        let human = self.data.human(&page.id).cloned().unwrap_or_default();
        div()
            .child(single(Field::Name, "Name"))
            .child(single(Field::JobTitle, "Job Title"))
            .child(field_row(Some("Company"), colors).child(self.company(&human, colors, cx)))
            .child(single(Field::Email, "Email"))
            .children(page.emails.iter().enumerate().map(|(index, row)| {
                field_row(None, colors).child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(input_box(row.input.clone()))
                        .child(
                            icon_button(
                                SharedString::from(format!("remove-email-{index}")),
                                "chat/XIcon.svg",
                                14.,
                                colors,
                            )
                            .size(px(24.))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.remove_email(index, cx)),
                            ),
                        ),
                )
            }))
            .child(match &page.new_email {
                Some(row) => field_row(None, colors).child(input_box(row.input.clone())),
                None => field_row(None, colors).child(
                    div().flex_1().child(
                        text_xs(div())
                            .id("add-email")
                            .ml(px(-8.))
                            .h(px(28.))
                            .px_2()
                            .flex()
                            .items_center()
                            .w_auto()
                            .flex_none()
                            .rounded(px(8.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(colors.muted_foreground)
                            .cursor_pointer()
                            .hover(move |style| {
                                style.bg(colors.accent).text_color(colors.accent_foreground)
                            })
                            .child("Add email")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.add_email(window, cx)),
                            ),
                    ),
                ),
            })
            .child(single(Field::Phone, "Phone"))
            .child(single(Field::LinkedIn, "LinkedIn"))
            .child(
                div()
                    .flex()
                    .border_b_1()
                    .border_color(colors.border)
                    .px_4()
                    .py_3()
                    .child(
                        text_sm(div())
                            .w(px(112.))
                            .flex_shrink_0()
                            .pt_2()
                            .text_color(colors.muted_foreground)
                            .child("Notes"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h(px(80.))
                            // The 24px input line sits where the textarea's 20px line does.
                            .pt(px(6.))
                            .overflow_hidden()
                            .child(input(Field::Memo)),
                    ),
            )
    }

    /// `ContactOrganizationSelector`.
    fn company(&self, human: &Human, colors: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let Page::Person(page) = &self.page else {
            return div();
        };
        let organization = self.data.organization(&human.organization_id);
        let trigger = div()
            .id("company-trigger")
            .ml(px(-8.))
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .rounded(px(8.))
            .cursor_pointer()
            .hover(move |style| style.bg(colors.accent))
            .on_click(cx.listener(|this, event, window, cx| {
                if this
                    .person_page()
                    .is_some_and(|page| !page.picker_dismissal.closed_by(event))
                {
                    this.open_picker(window, cx)
                }
            }))
            .map(
                |trigger| match organization.filter(|o| !o.name.is_empty()) {
                    Some(organization) => trigger
                        .child(text_sm(div()).child(organization.name.clone()))
                        .child(
                            div()
                                .id("remove-company")
                                .ml_2()
                                .group("remove-company")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.set_organization(String::new(), cx);
                                }))
                                .child(
                                    svg()
                                        .path("contacts/CircleMinusIcon.svg")
                                        .size(px(16.))
                                        .text_color(colors.muted_foreground)
                                        .group_hover("remove-company", |style| {
                                            style.text_color(red(600))
                                        }),
                                ),
                        ),
                    None => trigger.child(
                        text_sm(div())
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_color(colors.muted_foreground)
                            .child(
                                svg()
                                    .path("workspace/PlusIcon.svg")
                                    .size(px(16.))
                                    .text_color(colors.muted_foreground),
                            )
                            .child("Add organization"),
                    ),
                },
            );
        div().flex_1().flex().child(
            div()
                .relative()
                .child(trigger)
                .when_some(page.picker.as_ref(), |view, picker| {
                    view.child(dropdown(false, 32., self.picker(picker, colors, cx)))
                }),
        )
    }

    fn picker(
        &self,
        picker: &OrganizationPicker,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (matches, create) = self.picker_options(cx);
        let searching = !picker.search.read(cx).buffer.text.trim().is_empty();
        let option = |index: usize, cx: &mut Context<Self>| {
            div()
                .id(("organization-option", index))
                .w_full()
                .px_3()
                .py_2()
                .flex()
                .items_center()
                .text_size(px(14.))
                .line_height(px(20.))
                .cursor_pointer()
                .when(picker.highlighted == Some(index), |view| {
                    view.bg(colors.muted)
                })
                .when(picker.highlighted != Some(index), |view| {
                    view.hover(move |style| style.bg(colors.accent))
                })
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    if *hovered
                        && let Some(picker) =
                            this.person_page().and_then(|page| page.picker.as_mut())
                    {
                        picker.highlighted = Some(index);
                        cx.notify();
                    }
                }))
        };
        let list = div()
            .id("organization-options")
            .w_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(2.))
            .border_1()
            .border_color(colors.border)
            .children(matches.iter().enumerate().map(|(index, (id, name))| {
                let id = id.to_string();
                option(index, cx)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.set_organization(id.clone(), cx)),
                    )
                    .child(
                        div()
                            .mr_2()
                            .size(px(20.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(8.))
                            .bg(colors.muted)
                            .child(
                                svg()
                                    .path("contacts/Building03Icon.svg")
                                    .size(px(12.))
                                    .text_color(colors.foreground),
                            ),
                    )
                    .child(
                        div()
                            .truncate()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(name.clone()),
                    )
            }))
            .when_some(create, |list, name| {
                let index = matches.len();
                let label = name.clone();
                list.child(
                    option(index, cx)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.create_organization(name.clone(), cx)
                        }))
                        .child(
                            div()
                                .mr_2()
                                .size(px(20.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(8.))
                                .bg(colors.accent)
                                .child(text_xs(div()).child("+")),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(colors.muted_foreground)
                                .child("Create")
                                .child(
                                    div()
                                        .max_w(px(140.))
                                        .truncate()
                                        .text_color(colors.foreground)
                                        .child(format!("\"{label}\"")),
                                ),
                        ),
                )
            });
        floating_panel(288., colors)
            .id("organization-picker")
            .occlude()
            .on_mouse_down_out(cx.listener(|this, event, _, cx| {
                if let Some(page) = this.person_page() {
                    page.picker = None;
                    page.picker_dismissal.record(event);
                }
                cx.notify();
            }))
            .child(
                floating_panel_body(colors).p_3().child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            text_sm(div())
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(colors.muted_foreground)
                                .child("Organization"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .w_full()
                                        .px_2()
                                        .py(px(4.))
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .rounded(px(2.))
                                        .border_1()
                                        .border_color(colors.border)
                                        .bg(colors.muted)
                                        .on_key_down(cx.listener(
                                            |this, event: &KeyDownEvent, _, cx| {
                                                match event.keystroke.key.as_str() {
                                                    "down" => this.move_highlight(true, cx),
                                                    "up" => this.move_highlight(false, cx),
                                                    "escape" => {
                                                        if let Some(page) = this.person_page() {
                                                            page.picker = None;
                                                        }
                                                        cx.notify();
                                                    }
                                                    _ => return,
                                                }
                                                cx.stop_propagation();
                                            },
                                        ))
                                        .child(
                                            svg()
                                                .path("Search01Icon.svg")
                                                .size(px(16.))
                                                .flex_shrink_0()
                                                .text_color(colors.muted_foreground),
                                        )
                                        .child(
                                            div().flex_1().min_w_0().child(picker.search.clone()),
                                        ),
                                )
                                .when(searching || !matches.is_empty(), |view| {
                                    view.child(if searching {
                                        list
                                    } else {
                                        list.max_h(px(240.)).overflow_y_scroll()
                                    })
                                }),
                        ),
                ),
            )
    }

    /// The red "Duplicate Contact Found" banner.
    fn duplicates(
        &self,
        duplicates: &[Human],
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let many = duplicates.len() > 1;
        let selected = self
            .person_page_ref()
            .map(|page| page.id.to_string())
            .unwrap_or_default();
        div()
            .px_6()
            .py_4()
            .border_b_1()
            .border_color(colors.border)
            .bg(red(50))
            .child(
                text_sm(div())
                    .mb_1()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(red(900))
                    .child(if many {
                        "Duplicate Contacts Found"
                    } else {
                        "Duplicate Contact Found"
                    }),
            )
            .child(text_sm(div()).mb_3().text_color(red(800)).child(format!(
                "{} with the same name or email {}. Merge to consolidate all related notes and information.",
                if many {
                    format!("{} contacts", duplicates.len())
                } else {
                    "Another contact".into()
                },
                if many { "exist" } else { "exists" },
            )))
            .child(div().flex().flex_col().gap_2().children(duplicates.iter().map(|duplicate| {
                let duplicate_id = duplicate.id.to_string();
                let selected = selected.clone();
                div()
                    .p_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.muted)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(avatar::person(duplicate.avatar.as_ref(), &duplicate.avatar_name(), 32.))
                            .child(
                                div()
                                    .child(
                                        text_sm(div())
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(colors.foreground)
                                            .child(if duplicate.name.is_empty() {
                                                SharedString::from("Unnamed Contact")
                                            } else {
                                                duplicate.name.clone()
                                            }),
                                    )
                                    .child(
                                        text_xs(div())
                                            .text_color(colors.muted_foreground)
                                            .child(duplicate.email.clone()),
                                    ),
                            ),
                    )
                    .child(
                        text_xs(div())
                            .id(SharedString::from(format!("merge-{duplicate_id}")))
                            .h(px(28.))
                            .px_2()
                            .flex()
                            .items_center()
                            .rounded(px(8.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .bg(colors.primary)
                            .text_color(colors.primary_foreground)
                            .shadow_xs()
                            .cursor_pointer()
                            .hover(move |style| style.bg(colors.primary.opacity(0.9)))
                            .child("Merge")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.write(
                                    Write::Merge {
                                        selected: selected.clone(),
                                        duplicate: duplicate_id.clone(),
                                    },
                                    cx,
                                )
                            })),
                    )
            })))
    }

    fn person_page_ref(&self) -> Option<&PersonPage> {
        match &self.page {
            Page::Person(page) => Some(page),
            _ => None,
        }
    }

    /// `EnrichContactButton` without a connected CRM: disabled, with the connect prompt on hover.
    fn enrich(&self, colors: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("enrich-contact")
            .relative()
            .flex()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.prompt = *hovered;
                cx.notify();
            }))
            .child(
                text_xs(div())
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(8.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.muted_foreground)
                    .opacity(0.5)
                    .child(
                        svg()
                            .path("chat/SparkleIcon.svg")
                            .size(px(16.))
                            .text_color(colors.muted_foreground),
                    )
                    .child("Enrich"),
            )
            .when(self.prompt, |view| {
                view.child(dropdown(
                    true,
                    38.,
                    text_sm(div())
                        .relative()
                        .w(px(288.))
                        .px_3()
                        .py(px(10.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.popover)
                        .text_color(colors.popover_foreground)
                        .shadow_sm()
                        .child(
                            svg()
                                .path("contacts/PopoverArrow.svg")
                                .absolute()
                                .top(px(-9.))
                                .right(px(14.))
                                .w(px(17.))
                                .h(px(9.))
                                .text_color(colors.border),
                        )
                        .child(
                            svg()
                                .path("contacts/PopoverArrow.svg")
                                .absolute()
                                .top(px(-7.5))
                                .right(px(15.5))
                                .w(px(14.))
                                .h(px(8.5))
                                .text_color(colors.popover),
                        )
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child("Connect a CRM"),
                        )
                        .child(
                            div()
                                .mt(px(2.))
                                .line_height(px(19.25))
                                .text_color(colors.muted_foreground)
                                .child("Connect a CRM in Settings › CRM to enrich contacts."),
                        ),
                ))
            })
    }

    /// `RelatedNotesSection`.
    fn related_notes(
        &self,
        notes: &[RelatedNote],
        window: &Window,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Page::Person(page) = &self.page else {
            return div();
        };
        let search = page.notes_search.read(cx).buffer.text.clone();
        let focused = page.notes_search.focus_handle(cx).is_focused(window);
        let visible = visible_notes(notes, &search, page.notes_newest);
        div()
            .p_6()
            .child(
                div()
                    .mb_3()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        text_sm(div())
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(colors.muted_foreground)
                            .child("Related Notes"),
                    )
                    .when(notes.len() > 1, |view| {
                        let newest = page.notes_newest;
                        view.child(
                            div()
                                .relative()
                                .child(
                                    menu_trigger(
                                        "related-sort",
                                        "contacts/ArrowUpDownIcon.svg",
                                        15.,
                                        page.notes_menu,
                                        colors,
                                    )
                                    .on_click(cx.listener(
                                        |this, event, _, cx| {
                                            if let Some(page) = this.person_page()
                                                && !page.notes_dismissal.closed_by(event)
                                            {
                                                page.notes_menu = !page.notes_menu;
                                            }
                                            cx.notify();
                                        },
                                    )),
                                )
                                .when(page.notes_menu, |view| {
                                    view.child(dropdown(
                                        false,
                                        32.,
                                        floating_panel(128., colors)
                                            .id("related-sort-menu")
                                            .occlude()
                                            .on_mouse_down_out(cx.listener(|this, event, _, cx| {
                                                if let Some(page) = this.person_page() {
                                                    page.notes_menu = false;
                                                    page.notes_dismissal.record(event);
                                                }
                                                cx.notify();
                                            }))
                                            .child(
                                                floating_panel_body(colors).children(
                                                    [(true, "Newest"), (false, "Oldest")]
                                                        .into_iter()
                                                        .map(|(value, label)| {
                                                            radio_item(
                                                                SharedString::from(format!(
                                                                    "related-{label}"
                                                                )),
                                                                label,
                                                                newest == value,
                                                                false,
                                                                colors,
                                                            )
                                                            .on_click(cx.listener(
                                                                move |this, _, _, cx| {
                                                                    if let Some(page) =
                                                                        this.person_page()
                                                                    {
                                                                        page.notes_newest = value;
                                                                        page.notes_menu = false;
                                                                    }
                                                                    cx.notify();
                                                                },
                                                            ))
                                                        }),
                                                ),
                                            ),
                                    ))
                                }),
                        )
                    })
                    .child(self.notes_search(&search, focused, colors, cx)),
            )
            .map(|view| {
                if visible.is_empty() {
                    view.child(
                        text_sm(div())
                            .px_2()
                            .py_2()
                            .text_color(colors.muted_foreground)
                            .child(if notes.is_empty() {
                                "No related notes found"
                            } else {
                                "No results found."
                            }),
                    )
                } else {
                    view.children(visible.into_iter().map(|note| {
                        let id = note.id.clone();
                        div()
                            .id(SharedString::from(format!("related-{}", note.id)))
                            .w_full()
                            .px_2()
                            .py_2()
                            .flex()
                            .items_center()
                            .gap_3()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(move |style| style.bg(colors.accent))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(ContactsEvent::OpenNote(desktop_runtime::SessionId(
                                    id.to_string().into(),
                                )))
                            }))
                            .child(
                                div()
                                    .size(px(6.))
                                    .flex_shrink_0()
                                    .rounded(px(3.))
                                    .bg(colors.muted_foreground),
                            )
                            .child(
                                text_sm(div())
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(related_note_label(note)),
                            )
                            .when_some(note_date(&note.created_at), |view, date| {
                                view.child(
                                    text_xs(div())
                                        .flex_shrink_0()
                                        .text_color(colors.muted_foreground)
                                        .child(date),
                                )
                            })
                    }))
                }
            })
    }

    fn notes_search(
        &self,
        search: &str,
        focused: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Page::Person(page) = &self.page else {
            return div();
        };
        div()
            .ml_auto()
            .h(px(32.))
            .w(px(208.))
            .max_w(gpui::relative(0.48))
            .px(px(10.))
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(8.))
            .border_1()
            .border_color(colors.border)
            .bg(if focused {
                colors.accent
            } else {
                colors.muted.opacity(0.5)
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape"
                    && let Some(page) = this.person_page()
                {
                    page.notes_search
                        .update(cx, |input, cx| input.set_text(String::new(), cx));
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                svg()
                    .path("Search01Icon.svg")
                    .size(px(14.))
                    .flex_shrink_0()
                    .text_color(colors.muted_foreground),
            )
            .child(div().flex_1().min_w_0().child(page.notes_search.clone()))
            .when(!search.is_empty(), |view| {
                view.child(
                    div()
                        .id("clear-related-search")
                        .flex_shrink_0()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(page) = this.person_page() {
                                page.notes_search
                                    .update(cx, |input, cx| input.set_text(String::new(), cx));
                            }
                            cx.notify();
                        }))
                        .child(
                            svg()
                                .path("chat/XIcon.svg")
                                .size(px(14.))
                                .text_color(colors.muted_foreground),
                        ),
                )
            })
    }
}

fn field_row(label: Option<&'static str>, colors: Theme) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .border_b_1()
        .border_color(colors.border)
        .px_4()
        .py_3()
        .child(
            text_sm(div())
                .w(px(112.))
                .flex_shrink_0()
                .text_color(colors.muted_foreground)
                .children(label),
        )
}

/// A borderless shipping `Input` (`h-7 border-none p-0`).
fn input_box(input: Entity<TextInput>) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .h(px(28.))
        .flex()
        .items_center()
        .child(input)
}

/// `ContactSummarySection` with a saved summary or shipping's placeholder.
fn summary(facts: &[SharedString], colors: Theme) -> impl IntoElement {
    div()
        .p_6()
        .border_b_1()
        .border_color(colors.border)
        .child(
            div().mb_3().flex().items_center().gap_2().child(
                text_sm(div())
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.muted_foreground)
                    .child("Summary"),
            ),
        )
        .child(
            div()
                .p_4()
                .rounded(px(8.))
                .border_1()
                .border_color(colors.border)
                .bg(colors.muted)
                .map(|view| {
                    if facts.is_empty() {
                        view.child(
                            div()
                                .text_size(px(14.))
                                .line_height(px(22.75))
                                .text_color(colors.muted_foreground)
                                .child(SUMMARY_PLACEHOLDER),
                        )
                    } else {
                        // Shipping's `space-y-2` does not separate these items; they sit 22.75px apart.
                        view.child(div().flex().flex_col().children(facts.iter().map(|fact| {
                            div()
                                .flex()
                                .text_size(px(14.))
                                .line_height(px(22.75))
                                .text_color(colors.foreground)
                                .child(
                                    div()
                                        .w(px(20.))
                                        .h(px(22.75))
                                        .flex_shrink_0()
                                        .pl(px(3.5))
                                        .flex()
                                        .items_center()
                                        .child(
                                            div()
                                                .size(px(5.))
                                                .rounded(px(2.5))
                                                .bg(colors.foreground),
                                        ),
                                )
                                .child(div().flex_1().min_w_0().child(fact.clone()))
                        })))
                    }
                }),
        )
}
