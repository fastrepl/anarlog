//! The templates route: `TemplatesSidebarContent` in the left panel and `TemplateView` (the
//! template form, or Auto's summary format) in the main surface.

use std::{sync::Arc, time::Duration};

use desktop_runtime::{CancellationToken, Reply, RuntimeHandle};
use gpui::{
    Anchor, AnyElement, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent,
    MouseButton, Pixels, Point, Render, SharedString, Subscription, Window, anchored, deferred,
    div, prelude::*, px, rgb,
};

use super::{
    icon_picker::{IconPicker, IconPickerEvent},
    resource_list::{
        self as list, Glyph, MenuOwner, context_item, context_menu, dropdown, floating_menu,
        floating_menu_body, ghost_icon_button, icon, menu_item,
    },
    templates::{self, Changes, Library, Section, Selection, Sort},
};
use crate::ui::a11y::Accessible;
use crate::{
    contracts::ProductRoute,
    product::{
        account,
        cloud::resources::ResourceKind,
        sharing::{
            resource_dialog::{ResourceDialogEvent, ResourceShareDialog},
            shared_library::SharedLibrary,
        },
    },
    ui::{
        input::{InputEvent, TextInput},
        theme::{Theme, is_dark, monospace_font, theme},
    },
};

const SAVE_DELAY: Duration = Duration::from_millis(250);
const PRO_REQUIRED: &str = "This requires Anarlog Pro";

pub enum TemplatesEvent {
    Back,
    /// Sign in, or upgrade to Pro.
    Product(ProductRoute),
    Navigate(super::navigation::Route),
}

impl EventEmitter<TemplatesEvent> for TemplatesView {}

#[derive(Clone, Copy, PartialEq)]
enum Menu {
    Sort,
    Actions,
    Section(usize),
    AutoActions,
}

struct SectionInputs {
    title: Entity<TextInput>,
    description: Entity<TextInput>,
}

/// The open template's editable fields; edits autosave like shipping's form listener. Saves
/// go through the runtime's FIFO queue, so they land in edit order.
struct Form {
    id: Arc<str>,
    title: Entity<TextInput>,
    description: Entity<TextInput>,
    tag: Option<(Entity<TextInput>, Subscription)>,
    targets: Vec<Arc<str>>,
    sections: Vec<SectionInputs>,
    icon: Glyph,
    pending: Changes,
    revision: u64,
    _subscriptions: [Subscription; 2],
    section_subscriptions: Vec<Subscription>,
}

pub struct TemplatesView {
    runtime: RuntimeHandle,
    library: Arc<Library>,
    selected: Option<Selection>,
    search: Entity<TextInput>,
    sort: Sort,
    menu: Option<Menu>,
    context: Option<(Point<Pixels>, Arc<str>)>,
    form: Option<Form>,
    picker: Option<(Entity<IconPicker>, Subscription)>,
    signed_in: bool,
    sharing: Option<(Entity<ResourceShareDialog>, Subscription)>,
    shared_library: Entity<SharedLibrary>,
    /// The visible "requires Pro" toast, by sequence number.
    notice: Option<(u64, list::WarningToast)>,
    notices: u64,
    message: String,
    cancel: CancellationToken,
    focus: FocusHandle,
    auto: Option<AutoEditor>,
    /// The section being dragged by its grip, at its current index.
    dragged_section: Option<usize>,
    _subscriptions: Vec<Subscription>,
}

/// A section dragged by its grip; the list reorders under the pointer like shipping's
/// `Reorder.Group`, so the drag itself draws nothing.
struct SectionDrag;

impl Render for SectionDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Auto's editable summary format, rebuilt when the stored format changes (shipping keys its
/// form by the stored value).
struct AutoEditor {
    stored: Arc<str>,
    initial: String,
    input: Entity<TextInput>,
    _subscription: Subscription,
}

impl TemplatesView {
    pub fn new(runtime: RuntimeHandle, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new("Search templates...", cx).inline());
        let subscription = cx.subscribe(&search, |this, _, event, cx| {
            if matches!(event, InputEvent::Changed) {
                this.context = None;
                cx.notify();
            }
        });
        let shared_library = cx.new(|cx| {
            SharedLibrary::new(runtime.clone(), ResourceKind::Template, search.clone(), cx)
        });
        Self {
            runtime,
            library: Arc::default(),
            selected: None,
            search,
            sort: Sort::default(),
            menu: None,
            context: None,
            form: None,
            picker: None,
            signed_in: false,
            sharing: None,
            shared_library,
            notice: None,
            notices: 0,
            message: String::new(),
            auto: None,
            dragged_section: None,
            cancel: CancellationToken::new(),
            focus: cx.focus_handle(),
            _subscriptions: vec![subscription],
        }
    }

    pub fn set_signed_in(&mut self, signed_in: bool, cx: &mut Context<Self>) {
        self.signed_in = signed_in;
        cx.notify();
    }

    pub fn activate(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        list::watch(
            &self.runtime,
            templates::LIBRARY_SQL,
            self.cancel.clone(),
            cx,
            |this, rows, cx| {
                match rows {
                    Ok(rows) => this.apply(templates::decode_library(&rows), cx),
                    Err(error) => this.message = format!("Could not load templates: {error}"),
                }
                cx.notify();
            },
        );
    }

    pub fn suspend(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.menu = None;
        self.context = None;
        self.flush(cx);
    }

    fn apply(&mut self, library: Library, cx: &mut Context<Self>) {
        self.message.clear();
        self.library = Arc::new(library);
        self.sync_form(cx);
    }

    /// Shipping's `ResourceShareButton` dialog for the selected template.
    fn share(&mut self, cx: &mut Context<Self>) {
        let (source, title): (Arc<str>, Arc<str>) = match self.current() {
            Selection::Mine(id) => {
                let title = self
                    .template(&id)
                    .map(|template| template.title.trim().to_owned())
                    .filter(|title| !title.is_empty())
                    .unwrap_or_else(|| "Untitled template".into());
                (id.clone(), title.into())
            }
            Selection::Auto => ("auto".into(), "Auto".into()),
        };
        let runtime = self.runtime.clone();
        let dialog = cx
            .new(|cx| ResourceShareDialog::new(runtime, ResourceKind::Template, source, title, cx));
        let subscription = cx.subscribe(&dialog, |this, _, event, cx| {
            this.sharing = None;
            if let ResourceDialogEvent::Navigate(route) = event {
                cx.emit(TemplatesEvent::Navigate(route.clone()));
            }
            cx.notify();
        });
        self.sharing = Some((dialog, subscription));
        cx.notify();
    }

    fn current(&self) -> Selection {
        templates::resolve(&self.library, self.selected.as_ref())
    }

    fn template(&self, id: &str) -> Option<&templates::Template> {
        self.library.templates.iter().find(|t| t.id.as_ref() == id)
    }

    /// Opens the form for the resolved selection, saving the previous form's edits first.
    fn sync_form(&mut self, cx: &mut Context<Self>) {
        let id = match self.current() {
            Selection::Mine(id) => Some(id),
            Selection::Auto => None,
        };
        if self.form.as_ref().map(|form| &form.id) == id.as_ref() {
            return;
        }
        self.flush(cx);
        self.form = id.and_then(|id| {
            let template = self.template(&id)?.clone();
            Some(self.build_form(template, cx))
        });
    }

    fn build_form(&self, template: templates::Template, cx: &mut Context<Self>) -> Form {
        let input = |placeholder: &str, text: &str, muted: bool, cx: &mut Context<Self>| {
            cx.new(|cx| {
                let mut input = TextInput::new(placeholder, cx).inline();
                if muted {
                    input = input.muted();
                }
                input.set_text(text.into(), cx);
                input
            })
        };
        let title = input("Enter template title", &template.title, false, cx);
        let description = input(
            "Describe the template purpose...",
            &template.description,
            true,
            cx,
        );
        let watch = |input: &Entity<TextInput>, title: bool, cx: &mut Context<Self>| {
            cx.subscribe(input, move |this, input, event, cx| {
                if matches!(event, InputEvent::Changed) {
                    let text = input.read(cx).buffer.text.clone();
                    let changes = if title {
                        Changes {
                            title: Some(text),
                            ..Changes::default()
                        }
                    } else {
                        Changes {
                            description: Some(text),
                            ..Changes::default()
                        }
                    };
                    this.edit(changes, cx);
                }
            })
        };
        let subscriptions = [watch(&title, true, cx), watch(&description, false, cx)];
        let mut form = Form {
            id: template.id.clone(),
            title,
            description,
            tag: None,
            targets: template.targets.clone(),
            sections: Vec::new(),
            icon: template.icon.clone(),
            pending: Changes::default(),
            revision: 0,
            _subscriptions: subscriptions,
            section_subscriptions: Vec::new(),
        };
        for section in &template.sections {
            let inputs = self.section_inputs(section, cx);
            form.section_subscriptions
                .extend(Self::watch_section(&inputs, cx));
            form.sections.push(inputs);
        }
        form
    }

    fn section_inputs(&self, section: &Section, cx: &mut Context<Self>) -> SectionInputs {
        SectionInputs {
            title: cx.new(|cx| {
                let mut input = TextInput::new("Untitled", cx).inline();
                input.set_text(section.title.to_string(), cx);
                input
            }),
            description: cx.new(|cx| {
                let mut input = TextInput::new(
                    "Template content with Jinja2: {{ variable }}, {% if condition %}",
                    cx,
                )
                .inline()
                .multiline()
                .fill();
                input.set_text(section.description.to_string(), cx);
                input
            }),
        }
    }

    fn watch_section(inputs: &SectionInputs, cx: &mut Context<Self>) -> [Subscription; 2] {
        [&inputs.title, &inputs.description].map(|input| {
            cx.subscribe(input, |this, _, event, cx| {
                if matches!(event, InputEvent::Changed) {
                    this.sections_changed(cx);
                }
            })
        })
    }

    fn sections_changed(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        let sections = form
            .sections
            .iter()
            .map(|inputs| Section {
                title: inputs.title.read(cx).buffer.text.trim().into(),
                description: inputs.description.read(cx).buffer.text.as_str().into(),
            })
            .collect();
        self.edit(
            Changes {
                sections: Some(sections),
                ..Changes::default()
            },
            cx,
        );
    }

    fn edit(&mut self, changes: Changes, cx: &mut Context<Self>) {
        let Some(form) = &mut self.form else { return };
        form.pending.merge(changes);
        form.revision += 1;
        let revision = form.revision;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            let _ = this.update(cx, |this, cx| {
                if this
                    .form
                    .as_ref()
                    .is_some_and(|form| form.revision == revision)
                {
                    this.flush(cx);
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn flush(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &mut self.form else { return };
        if form.pending.is_empty() {
            return;
        }
        let reply = templates::save(
            &self.runtime,
            form.id.clone(),
            std::mem::take(&mut form.pending),
        );
        cx.spawn(async move |this, cx| {
            if let Err(error) = receive(reply).await {
                let _ = this.update(cx, |this, cx| {
                    this.message = format!("Could not save the template: {error}");
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn select(&mut self, selection: Selection, cx: &mut Context<Self>) {
        self.selected = Some(selection);
        self.menu = None;
        self.sync_form(cx);
        cx.notify();
    }

    /// `useNotifyPlanRequired`: a six-second warning toast with Upgrade.
    fn notify_pro(&mut self, cx: &mut Context<Self>) {
        const TOTAL: u64 = 6_000;
        const TICK: u64 = 100;
        self.notices += 1;
        let id = self.notices;
        self.notice = Some((
            id,
            list::WarningToast {
                message: PRO_REQUIRED,
                action: "Upgrade",
                remaining_ms: TOTAL,
                total_ms: TOTAL,
                paused: false,
            },
        ));
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(TICK))
                    .await;
                let open = this.update(cx, |this, cx| {
                    let Some((_, toast)) =
                        this.notice.as_mut().filter(|(current, _)| *current == id)
                    else {
                        return false;
                    };
                    if !toast.paused {
                        toast.remaining_ms = toast.remaining_ms.saturating_sub(TICK);
                        if toast.remaining_ms == 0 {
                            this.notice = None;
                        }
                        cx.notify();
                    }
                    this.notice.is_some()
                });
                if !matches!(open, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    /// Runs a write; ids it returns become the selection, errors become the message.
    fn run<T: 'static>(
        &mut self,
        reply: desktop_runtime::Result<Reply<T>>,
        then: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        self.context = None;
        cx.spawn(async move |this, cx| {
            let result = receive(reply).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(value) => then(this, value, cx),
                    Err(error) => this.message = format!("Could not update templates: {error}"),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        if !account::is_pro(cx) {
            self.notify_pro(cx);
            return;
        }
        let reply = templates::create(&self.runtime);
        self.run(
            reply,
            |this, id, cx| this.select(Selection::Mine(id), cx),
            cx,
        );
    }

    fn duplicate(&mut self, id: Arc<str>, cx: &mut Context<Self>) {
        self.menu = None;
        self.context = None;
        if !account::is_pro(cx) {
            self.notify_pro(cx);
            return;
        }
        self.flush(cx);
        let reply = templates::duplicate(&self.runtime, id);
        self.run(
            reply,
            |this, id, cx| this.select(Selection::Mine(id), cx),
            cx,
        );
    }

    fn delete(&mut self, id: Arc<str>, cx: &mut Context<Self>) {
        self.flush(cx);
        if self.form.as_ref().is_some_and(|form| form.id == id) {
            self.form = None;
        }
        let reply = templates::delete(&self.runtime, id);
        self.run(
            reply,
            |this, (), cx| {
                this.selected = None;
                this.sync_form(cx);
            },
            cx,
        );
    }

    fn toggle_favorite(&mut self, id: Arc<str>, cx: &mut Context<Self>) {
        let reply = templates::toggle_favorite(&self.runtime, id);
        self.run(reply, |_, (), _| {}, cx);
    }

    fn auto_editor(&mut self, cx: &mut Context<Self>) -> Entity<TextInput> {
        let stored = self.library.auto_format.clone();
        if let Some(auto) = self.auto.as_ref().filter(|auto| auto.stored == stored) {
            return auto.input.clone();
        }
        let initial = templates::auto_format(&stored, Self::default_format());
        let input = cx.new(|cx| {
            let mut input = TextInput::new("", cx)
                .inline()
                .multiline()
                .paragraph_gap(px(4.));
            input.set_text(initial.clone(), cx);
            input
        });
        let subscription = cx.observe(&input, |_, _, cx| cx.notify());
        self.auto = Some(AutoEditor {
            stored,
            initial,
            input: input.clone(),
            _subscription: subscription,
        });
        input
    }

    fn default_format() -> &'static str {
        anlg_template_app::template_source(anlg_template_app::EditableTemplate::EnhanceFormat)
    }

    fn save_auto_format(&mut self, source: String, cx: &mut Context<Self>) {
        if !account::is_pro(cx) {
            self.notify_pro(cx);
            return;
        }
        match templates::stored_format(&source, Self::default_format()) {
            Ok(stored) => {
                let reply = templates::save_auto_format(&self.runtime, stored.clone());
                self.run(
                    reply,
                    move |this, (), cx| {
                        // Saving the value already stored rebuilds nothing, so reset the form.
                        if let Some(auto) = &mut this.auto
                            && auto.stored.as_ref() == stored
                        {
                            auto.initial = templates::auto_format(&stored, Self::default_format());
                            let text = auto.initial.clone();
                            auto.input.update(cx, |input, cx| input.set_text(text, cx));
                        }
                    },
                    cx,
                );
            }
            Err(error) => {
                self.message = error;
                cx.notify();
            }
        }
    }

    fn set_default(&mut self, id: Arc<str>, cx: &mut Context<Self>) {
        let reply = templates::set_default(&self.runtime, id);
        self.run(reply, |_, (), _| {}, cx);
    }

    fn change_sections(
        &mut self,
        change: impl FnOnce(&mut Vec<SectionInputs>, &mut Context<Self>),
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        let Some(mut form) = self.form.take() else {
            return;
        };
        change(&mut form.sections, cx);
        form.section_subscriptions = form
            .sections
            .iter()
            .flat_map(|inputs| Self::watch_section(inputs, cx))
            .collect();
        self.form = Some(form);
        self.sections_changed(cx);
    }

    fn insert_section(&mut self, index: usize, cx: &mut Context<Self>) {
        let inputs = self.section_inputs(
            &Section {
                title: "".into(),
                description: "".into(),
            },
            cx,
        );
        self.change_sections(
            move |sections, _| sections.insert(index.min(sections.len()), inputs),
            cx,
        );
    }

    fn set_targets(&mut self, targets: Vec<Arc<str>>, cx: &mut Context<Self>) {
        let Some(form) = &mut self.form else { return };
        form.targets = targets.clone();
        self.edit(
            Changes {
                targets: Some(targets),
                ..Changes::default()
            },
            cx,
        );
    }

    fn start_tag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &mut self.form else { return };
        if let Some((tag, _)) = &form.tag {
            tag.focus_handle(cx).focus(window, cx);
            return;
        }
        let tag = cx.new(|cx| TextInput::new("", cx).inline().muted());
        let subscription = cx.subscribe(&tag, |this, input, event, cx| match event {
            InputEvent::Submitted => this.submit_tag(cx),
            InputEvent::Changed if input.read(cx).buffer.text.contains(',') => this.submit_tag(cx),
            _ => {}
        });
        tag.focus_handle(cx).focus(window, cx);
        form.tag = Some((tag, subscription));
        cx.notify();
    }

    /// `TemplateTargetsInput`: comma-separated entries become tags.
    fn submit_tag(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &mut self.form else { return };
        let Some((tag, _)) = form.tag.take() else {
            return;
        };
        let text = tag.read(cx).buffer.text.clone();
        let added = text
            .split(',')
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .map(Arc::from)
            .collect::<Vec<_>>();
        if !added.is_empty() {
            let mut targets = form.targets.clone();
            targets.extend(added);
            self.set_targets(targets, cx);
        }
        cx.notify();
    }

    fn open_picker(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        if self.picker.take().is_some() {
            cx.notify();
            return;
        }
        let picker = cx.new(|cx| IconPicker::new(form.icon.clone(), cx));
        let subscription = cx.subscribe(&picker, |this, _, event, cx| match event {
            IconPickerEvent::Changed { icon, close } => {
                this.set_icon(icon.clone(), cx);
                if *close {
                    this.picker = None;
                }
            }
        });
        self.picker = Some((picker, subscription));
        cx.notify();
    }

    fn set_icon(&mut self, icon: Glyph, cx: &mut Context<Self>) {
        let Some(form) = &mut self.form else { return };
        form.icon = icon.clone();
        self.edit(
            Changes {
                icon: Some(icon),
                ..Changes::default()
            },
            cx,
        );
    }

    /// Option+Up/Down walks the list, as shipping's window-level shortcut does.
    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let search = self.search.read(cx).buffer.text.clone();
        let items = templates::visible(&self.library, &search, self.sort);
        if items.is_empty() {
            return;
        }
        let current = self.current();
        let next = match items.iter().position(|item| *item == current) {
            None if forward => 0,
            None => items.len() - 1,
            Some(index) if forward => (index + 1).min(items.len() - 1),
            Some(index) => index.saturating_sub(1),
        };
        if items[next] != current {
            self.select(items[next].clone(), cx);
        }
    }

    pub fn render_nav(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let search = self.search.read(cx).buffer.text.clone();
        let items = templates::visible(&self.library, &search, self.sort);
        let current = self.current();
        let searching = !search.is_empty();
        let mut actions = Vec::new();
        if self.library.templates.len() > 1 {
            actions.push(
                ghost_icon_button("templates-sort", "ArrowUpDownIcon", colors)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.menu = (this.menu != Some(Menu::Sort)).then_some(Menu::Sort);
                        cx.notify();
                    }))
                    .when(self.menu == Some(Menu::Sort), |button| {
                        button.child(self.sort_menu(window, colors, cx))
                    })
                    .into_any_element(),
            );
        }
        actions.push(
            ghost_icon_button("templates-new", "PlusIcon", colors)
                .on_click(cx.listener(|this, _, _, cx| this.create(cx)))
                .into_any_element(),
        );
        let focused = self.search.focus_handle(cx).is_focused(window);
        div()
            .id("templates-nav")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let modifiers = event.keystroke.modifiers;
                let key = event.keystroke.key.as_str();
                let searching = this.search.focus_handle(cx).is_focused(window);
                if key == "escape" && searching {
                    this.search
                        .update(cx, |input, cx| input.set_text(String::new(), cx));
                    cx.stop_propagation();
                } else if modifiers.alt
                    && !modifiers.platform
                    && !modifiers.control
                    && !modifiers.shift
                    && matches!(key, "up" | "down")
                    && !searching
                {
                    this.step(key == "down", cx);
                    cx.stop_propagation();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .pb_2()
            .child(list::sidebar_header(
                "templates-back",
                colors,
                cx.listener(|_, _, _, cx| cx.emit(TemplatesEvent::Back)),
                actions,
                cx,
            ))
            .child(list::search_field(
                &self.search,
                focused,
                searching,
                colors,
                cx.listener(|this, _, _, cx| {
                    this.search
                        .update(cx, |input, cx| input.set_text(String::new(), cx));
                }),
            ))
            .child(
                div()
                    .id("templates-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .map(|view| {
                        if items.is_empty() {
                            view.child(
                                list::empty_list(
                                    "BookOpenTextIcon",
                                    if searching {
                                        "No templates found"
                                    } else {
                                        "No templates yet"
                                    },
                                    colors,
                                )
                                .when(!searching, |empty| {
                                    empty.child(
                                        div()
                                            .id("create-first-template")
                                            .a11y_button("Create my first template")
                                            .mt_3()
                                            .text_sm()
                                            .underline()
                                            .cursor_pointer()
                                            .hover(|style| style.text_color(colors.foreground))
                                            .child("Create my first template")
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.create(cx)),
                                            ),
                                    )
                                }),
                            )
                        } else {
                            view.child(div().pt_1().children(items.into_iter().map(|item| {
                                self.list_item(item.clone(), item == current, colors, cx)
                            })))
                        }
                    })
                    .child(self.shared_library.clone()),
            )
            .when_some(self.context.clone(), |view, (position, id)| {
                view.child(self.context_menu(position, id, colors, cx))
            })
            .into_any_element()
    }

    fn list_item(
        &self,
        item: Selection,
        selected: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (key, glyph, title, subtitle): (SharedString, AnyElement, SharedString, Option<&str>) =
            match &item {
                Selection::Auto => (
                    "template-auto".into(),
                    icon("SparkleIcon", 16., rgb(0x8b5cf6).into()).into_any_element(),
                    "Auto".into(),
                    self.library.auto_customized.then_some("Customized"),
                ),
                Selection::Mine(id) => {
                    let Some(template) = self.template(id) else {
                        return div().into_any_element();
                    };
                    (
                        format!("template-{id}").into(),
                        template.icon.render(16.),
                        template.display_title().to_owned().into(),
                        None,
                    )
                }
            };
        let context_id = match &item {
            Selection::Mine(id) => Some(id.clone()),
            Selection::Auto => None,
        };
        list::list_row(key, selected, false, colors)
            .aria_label(match subtitle {
                Some(subtitle) => SharedString::from(format!("{title}, {subtitle}")),
                None => title.clone(),
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.focus.focus(window, cx);
                this.select(item.clone(), cx);
            }))
            .when_some(context_id, |row, id| {
                row.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.select(Selection::Mine(id.clone()), cx);
                        this.context = Some((event.position, id.clone()));
                        cx.notify();
                    }),
                )
            })
            .child(
                div().flex().items_center().gap_2().child(glyph).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .font_weight(crate::ui::theme::MEDIUM)
                                .child(title),
                        )
                        .when_some(subtitle, |view, subtitle| {
                            view.child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .line_height(px(16.))
                                    .text_color(colors.muted_foreground)
                                    .child(subtitle),
                            )
                        }),
                ),
            )
            .into_any_element()
    }

    fn sort_menu(&self, window: &Window, colors: Theme, cx: &mut Context<Self>) -> AnyElement {
        let option = |label: &'static str, sort: Sort, cx: &mut Context<Self>| {
            menu_item(label, label, false, false, window, colors).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.sort = sort;
                    this.menu = None;
                    cx.notify();
                },
            ))
        };
        dropdown(
            Anchor::TopRight,
            floating_menu(128., colors).child(
                floating_menu_body(colors)
                    .child(option("A to Z", Sort::AToZ, cx))
                    .child(option("Z to A", Sort::ZToA, cx)),
            ),
            cx,
        )
    }

    fn context_menu(
        &self,
        position: Point<Pixels>,
        id: Arc<str>,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pinned = self.template(&id).is_some_and(|template| template.pinned);
        let favorite = id.clone();
        let duplicate = id.clone();
        deferred(
            anchored().position(position).snap_to_window().child(
                context_menu(colors)
                    .id("template-context-menu")
                    .occlude()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.context = None;
                        cx.notify();
                    }))
                    .child(
                        context_item(
                            "favorite",
                            if pinned { "Unfavorite" } else { "Favorite" },
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_favorite(favorite.clone(), cx)
                        })),
                    )
                    .child(div().mx(px(-4.)).my_1().h(px(1.)).bg(colors.border))
                    .child(context_item("duplicate", "Duplicate", colors).on_click(
                        cx.listener(move |this, _, _, cx| this.duplicate(duplicate.clone(), cx)),
                    ))
                    .child(
                        context_item("delete", "Delete", colors).on_click(
                            cx.listener(move |this, _, _, cx| this.delete(id.clone(), cx)),
                        ),
                    ),
            ),
        )
        .with_priority(1)
        .into_any_element()
    }

    fn header_button(
        id: &'static str,
        label: &'static str,
        default: bool,
        colors: Theme,
        window: &Window,
    ) -> gpui::Stateful<gpui::Div> {
        let emerald: gpui::Hsla = if is_dark(window) {
            rgb(0x34d399).into()
        } else {
            rgb(0x059669).into()
        };
        div()
            .id(id)
            .a11y_button(if default { "Current default" } else { label })
            .group(id)
            .h(px(28.))
            .px_2()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .line_height(px(16.))
            .font_weight(crate::ui::theme::MEDIUM)
            .map(|button| {
                if default {
                    button
                        .text_color(emerald)
                        .child(icon("CheckIconBold", 14., emerald))
                        .child("Current default")
                } else {
                    let hover = if is_dark(window) {
                        colors.foreground
                    } else {
                        rgb(0x000000).into()
                    };
                    button
                        .cursor_pointer()
                        .text_color(colors.muted_foreground)
                        .hover(move |style| style.bg(colors.accent).text_color(hover))
                        .child(label)
                }
            })
    }

    fn render_form(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let Some(form) = &self.form else {
            return self.render_empty(colors, cx);
        };
        let Some(template) = self.template(&form.id) else {
            return div().into_any_element();
        };
        let id = form.id.clone();
        let is_default = self.library.default_id == id;
        let pinned = template.pinned;
        let rose: gpui::Hsla = rgb(0xf43f5e).into();
        let sections_editable = account::is_pro(cx) && template.warnings.is_empty();
        let actions_open = self.menu == Some(Menu::Actions);
        let (default_id, favorite_id) = (id.clone(), id.clone());
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
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
                        self.gate(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .id("template-icon")
                                        .size(px(28.))
                                        .flex_shrink_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .hover(|style| style.bg(colors.accent))
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.open_picker(cx)),
                                        )
                                        .child(form.icon.render(16.))
                                        .when_some(self.picker.as_ref(), |trigger, (picker, _)| {
                                            trigger.child(list::popover(
                                                picker.clone().into_any_element(),
                                                cx,
                                            ))
                                        }),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child(form.title.clone()),
                                ),
                            cx,
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(list::share_button("template-share", colors).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.share(cx);
                                }),
                            ))
                            .child(
                                Self::header_button(
                                    "template-default",
                                    "Set as default",
                                    is_default,
                                    colors,
                                    window,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        let next = if is_default {
                                            "".into()
                                        } else {
                                            default_id.clone()
                                        };
                                        this.set_default(next, cx)
                                    },
                                )),
                            )
                            .child(
                                div()
                                    .id("template-favorite")
                                    .group("template-favorite")
                                    .size(px(28.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|style| style.bg(colors.accent))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.toggle_favorite(favorite_id.clone(), cx)
                                    }))
                                    .child(
                                        icon(
                                            if pinned { "HeartIconBold" } else { "HeartIcon" },
                                            16.,
                                            if pinned {
                                                rose
                                            } else {
                                                colors.muted_foreground
                                            },
                                        )
                                        .group_hover(
                                            "template-favorite",
                                            move |style| {
                                                style.text_color(if pinned {
                                                    rgb(0xe11d48).into()
                                                } else {
                                                    colors.foreground
                                                })
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                ghost_icon_button("template-actions", "MoreHorizontalIcon", colors)
                                    .when(actions_open, |button| button.bg(colors.muted))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.menu = (this.menu != Some(Menu::Actions))
                                            .then_some(Menu::Actions);
                                        cx.notify();
                                    }))
                                    .when(actions_open, |button| {
                                        button.child(self.actions_menu(
                                            id.clone(),
                                            window,
                                            colors,
                                            cx,
                                        ))
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .id("template-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_6()
                    .pt_3()
                    .pb_6()
                    .child(
                        self.gate(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    // The input centers a 24px line; shipping's
                                    // textarea sets a 20px line from the top.
                                    div()
                                        .h(px(24.))
                                        .overflow_hidden()
                                        .text_sm()
                                        .child(div().mt(px(-2.)).child(form.description.clone())),
                                )
                                .child(self.render_targets(form, colors, cx))
                                .when(!template.warnings.is_empty(), |view| {
                                    view.children(template.warnings.iter().map(|warning| {
                                        div()
                                            .mt_2()
                                            .text_xs()
                                            .text_color(colors.destructive)
                                            .child(SharedString::from(warning.clone()))
                                    }))
                                })
                                .child(self.render_sections(
                                    form,
                                    sections_editable,
                                    window,
                                    colors,
                                    cx,
                                )),
                            cx,
                        ),
                    ),
            )
            .into_any_element()
    }

    /// `PlanGate`: without Pro the content is dimmed and every pointer press explains why.
    fn gate(&self, content: gpui::Div, cx: &mut Context<Self>) -> AnyElement {
        if account::is_pro(cx) {
            return content.into_any_element();
        }
        content
            .relative()
            .opacity(0.6)
            .cursor_not_allowed()
            .child(
                div()
                    .id("plan-gate")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .cursor_not_allowed()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.notify_pro(cx);
                        }),
                    ),
            )
            .into_any_element()
    }

    fn render_targets(&self, form: &Form, colors: Theme, cx: &mut Context<Self>) -> AnyElement {
        let chip = || {
            div()
                .h(px(24.))
                .px_2()
                .flex()
                .items_center()
                .gap_1()
                .border_1()
                .border_color(gpui::transparent_black())
                .bg(colors.muted)
                .text_xs()
                .line_height(px(16.))
        };
        div()
            .id("template-targets")
            .mt_2()
            .min_h(px(24.))
            .w_full()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .cursor_text()
            .on_click(cx.listener(|this, _, window, cx| this.start_tag(window, cx)))
            .children(form.targets.iter().enumerate().map(|(index, target)| {
                chip().child(SharedString::from(target.clone())).child(
                    div()
                        .id(("remove-target", index))
                        .ml(px(2.))
                        .size(px(12.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            let Some(form) = &this.form else { return };
                            let mut targets = form.targets.clone();
                            targets.remove(index);
                            this.set_targets(targets, cx);
                        }))
                        .child(icon("XIcon", 10., colors.foreground)),
                )
            }))
            .map(|row| match &form.tag {
                Some((tag, _)) => row.child(
                    div()
                        .flex_1()
                        .min_w(px(84.))
                        .text_xs()
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                            let Some(form) = &mut this.form else { return };
                            let empty = form
                                .tag
                                .as_ref()
                                .is_some_and(|(tag, _)| tag.read(cx).buffer.text.is_empty());
                            match event.keystroke.key.as_str() {
                                "escape" => {
                                    form.tag = None;
                                    cx.stop_propagation();
                                    cx.notify();
                                }
                                "tab" if !empty => {
                                    this.submit_tag(cx);
                                    cx.stop_propagation();
                                }
                                "backspace" if empty && !form.targets.is_empty() => {
                                    let mut targets = form.targets.clone();
                                    targets.pop();
                                    this.set_targets(targets, cx);
                                    cx.stop_propagation();
                                }
                                _ => {}
                            }
                        }))
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| this.submit_tag(cx)))
                        .child(tag.clone()),
                ),
                None => row.child(
                    chip()
                        .id("add-tag")
                        .a11y_button("Add tag")
                        .text_color(colors.muted_foreground)
                        .cursor_pointer()
                        .hover(|style| style.bg(colors.muted.opacity(0.8)))
                        .on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.start_tag(window, cx);
                        }))
                        .child(icon("PlusIcon", 12., colors.muted_foreground))
                        .child("Add tag"),
                ),
            })
            .into_any_element()
    }

    fn render_sections(
        &self,
        form: &Form,
        editable: bool,
        window: &Window,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let total = form.sections.len();
        div()
            .mt_6()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(form.sections.iter().enumerate().map(|(index, inputs)| {
                        let group: SharedString = format!("template-section-{index}").into();
                        let menu_open = self.menu == Some(Menu::Section(index));
                        let view_entity = cx.entity().downgrade();
                        div()
                            .group(group.clone())
                            .relative()
                            .bg(colors.card)
                            .on_drag_move(cx.listener(
                                move |this, event: &gpui::DragMoveEvent<SectionDrag>, _, cx| {
                                    if let Some(from) = this.dragged_section
                                        && from != index
                                        && event.bounds.contains(&event.event.position)
                                    {
                                        this.change_sections(
                                            move |sections, _| {
                                                let moved = sections.remove(from);
                                                sections.insert(index, moved);
                                            },
                                            cx,
                                        );
                                        this.dragged_section = Some(index);
                                    }
                                },
                            ))
                            .when(editable, |view| {
                                view.child(
                                    div()
                                        .id(("template-section-grip", index))
                                        .absolute()
                                        .top(px(10.))
                                        .left(px(-20.))
                                        .cursor_move()
                                        .opacity(0.)
                                        .group_hover(group.clone(), |style| style.opacity(0.3))
                                        .hover(|style| style.opacity(0.6))
                                        .on_drag(SectionDrag, move |_, _, _, cx| {
                                            let _ = view_entity.update(cx, |this, _| {
                                                this.dragged_section = Some(index)
                                            });
                                            cx.new(|_| SectionDrag)
                                        })
                                        .child(icon(
                                            "GripVerticalIcon",
                                            16.,
                                            colors.muted_foreground,
                                        )),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top_2()
                                        .right_2()
                                        .when(!menu_open, |view| {
                                            view.opacity(0.).group_hover(group.clone(), |style| {
                                                style.opacity(1.)
                                            })
                                        })
                                        .child(
                                            ghost_icon_button(
                                                format!("section-actions-{index}"),
                                                "MoreHorizontalIcon",
                                                colors,
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.menu = (this.menu
                                                    != Some(Menu::Section(index)))
                                                .then_some(Menu::Section(index));
                                                cx.notify();
                                            }))
                                            .when(
                                                menu_open,
                                                |button| {
                                                    button.child(self.section_menu(
                                                        index, total, window, colors, cx,
                                                    ))
                                                },
                                            ),
                                        ),
                                )
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .pr_9()
                                    .child(
                                        div()
                                            .h(px(36.))
                                            .flex()
                                            .items_center()
                                            .text_sm()
                                            .font_weight(crate::ui::theme::MEDIUM)
                                            .when(!editable, |view| view.opacity(0.5))
                                            .child(inputs.title.clone()),
                                    )
                                    .child(
                                        // `min-h-[100px] p-3 text-sm`; the text scrolls inside.
                                        div()
                                            .h(px(100.))
                                            .w_full()
                                            .p_3()
                                            .border_1()
                                            .text_sm()
                                            .line_height(px(20.))
                                            .map(|view| {
                                                if !editable {
                                                    view.bg(colors.muted).border_color(colors.input)
                                                } else if inputs
                                                    .description
                                                    .focus_handle(cx)
                                                    .is_focused(window)
                                                {
                                                    view.border_color(rgb(0x3b82f6))
                                                } else {
                                                    view.border_color(colors.input)
                                                }
                                            })
                                            .child(inputs.description.clone()),
                                    ),
                            )
                    })),
            )
            .when(editable, |view| {
                view.child(
                    div().flex().child(
                        div()
                            .id("add-section")
                            .a11y_button("Add Section")
                            .px_4()
                            .py(px(10.))
                            .flex()
                            .items_center()
                            .gap_4()
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.card)
                            .text_sm()
                            .line_height(px(20.))
                            .font_weight(crate::ui::theme::MEDIUM)
                            .text_color(colors.foreground)
                            .shadow_sm()
                            .cursor_pointer()
                            .hover(|style| style.bg(colors.background))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.insert_section(total, cx)),
                            )
                            .child(icon("PlusIcon", 16., colors.foreground))
                            .child("Add Section"),
                    ),
                )
            })
            .into_any_element()
    }

    fn section_menu(
        &self,
        index: usize,
        total: usize,
        window: &Window,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let item = |id: &'static str, label, destructive, disabled| {
            menu_item(id, label, destructive, disabled, window, colors)
        };
        dropdown(
            Anchor::TopRight,
            floating_menu(160., colors).child(
                floating_menu_body(colors)
                    .child(item("insert-above", "Insert above", false, false).on_click(
                        cx.listener(move |this, _, _, cx| this.insert_section(index, cx)),
                    ))
                    .child(item("insert-below", "Insert below", false, false).on_click(
                        cx.listener(move |this, _, _, cx| this.insert_section(index + 1, cx)),
                    ))
                    .child(
                        item("move-up", "Move up", false, index == 0).when(index > 0, |item| {
                            item.on_click(cx.listener(move |this, _, _, cx| {
                                this.change_sections(
                                    move |sections, _| sections.swap(index, index - 1),
                                    cx,
                                )
                            }))
                        }),
                    )
                    .child(
                        item("move-down", "Move down", false, index + 1 == total).when(
                            index + 1 < total,
                            |item| {
                                item.on_click(cx.listener(move |this, _, _, cx| {
                                    this.change_sections(
                                        move |sections, _| sections.swap(index, index + 1),
                                        cx,
                                    )
                                }))
                            },
                        ),
                    )
                    .child(
                        item("delete-section", "Delete", true, false).on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.change_sections(
                                    move |sections, _| {
                                        sections.remove(index);
                                    },
                                    cx,
                                )
                            },
                        )),
                    ),
            ),
            cx,
        )
    }

    fn actions_menu(
        &self,
        id: Arc<str>,
        window: &Window,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let duplicate = id.clone();
        dropdown(
            Anchor::TopRight,
            floating_menu(128., colors).child(
                floating_menu_body(colors)
                    .child(
                        menu_item("duplicate", "Duplicate", false, false, window, colors).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.duplicate(duplicate.clone(), cx)
                            }),
                        ),
                    )
                    .child(
                        menu_item("delete", "Delete", true, false, window, colors).on_click(
                            cx.listener(move |this, _, _, cx| this.delete(id.clone(), cx)),
                        ),
                    ),
            ),
            cx,
        )
    }

    fn render_empty(&self, colors: Theme, cx: &mut Context<Self>) -> AnyElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("No templates yet"),
            )
            .child(
                div()
                    .id("create-template")
                    .a11y_button("Create template")
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_1()
                    .border_color(colors.input)
                    .bg(colors.background)
                    .shadow_xs()
                    .text_xs()
                    .font_weight(crate::ui::theme::MEDIUM)
                    .cursor_pointer()
                    .hover(|style| style.bg(colors.accent))
                    .on_click(cx.listener(|this, _, _, cx| this.create(cx)))
                    .child(icon("PlusIcon", 16., colors.foreground))
                    .child("Create template"),
            )
            .into_any_element()
    }

    /// `AutoTemplateDetails`: the built-in summary format.
    fn render_auto(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let is_default = self.library.default_id.is_empty();
        let editor = self.auto_editor(cx);
        let dirty = self
            .auto
            .as_ref()
            .is_some_and(|auto| auto.input.read(cx).buffer.text != auto.initial);
        let menu_open = self.menu == Some(Menu::AutoActions);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
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
                            .flex()
                            .min_w_0()
                            .items_center()
                            .gap_2()
                            .child(icon("SparkleIcon", 16., rgb(0x8b5cf6).into()))
                            .child(
                                div()
                                    .truncate()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Auto"),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Self::header_button("auto-default", "Set as default", is_default, colors, window)
                                    .when(!is_default, |button| {
                                        button.on_click(cx.listener(|this, _, _, cx| this.set_default("".into(), cx)))
                                    }),
                            )
                            .child(
                                ghost_icon_button("auto-actions", "MoreHorizontalIcon", colors)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.menu = (this.menu != Some(Menu::AutoActions))
                                            .then_some(Menu::AutoActions);
                                        cx.notify();
                                    }))
                                    .when(menu_open, |button| {
                                        button.child(dropdown(
                                            Anchor::TopRight,
                                            floating_menu(224., colors).child(
                                                floating_menu_body(colors).child(
                                                    menu_item(
                                                        "reset-format",
                                                        "Reset to default format",
                                                        false,
                                                        !self.library.auto_customized,
                                                        window,
                                                        colors,
                                                    )
                                                    .when(self.library.auto_customized, |item| {
                                                        item.on_click(cx.listener(|this, _, _, cx| {
                                                            this.save_auto_format(
                                                                Self::default_format().into(),
                                                                cx,
                                                            )
                                                        }))
                                                    }),
                                                ),
                                            ),
                                            cx,
                                        ))
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .id("auto-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_6()
                    .pt_3()
                    .pb_6()
                    .child(
                        div()
                            .max_w(px(896.))
                            .flex()
                            .flex_col()
                            .gap_5()
                            .child(
                                div()
                                    .flex()
                                    .items_start()
                                    .justify_between()
                                    .gap_4()
                                    .child(
                                        div()
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .line_height(px(28.))
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .child("Summary format"),
                                            )
                                            .child(
                                                div()
                                                    .mt_1()
                                                    .text_sm()
                                                    .line_height(px(20.))
                                                    .text_color(colors.muted_foreground)
                                                    .child("Choose how Auto structures and styles your summaries."),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("improve-format")
                                            .flex_shrink_0()
                                            .h(px(28.))
                                            .px_2()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .border_1()
                                            .border_color(colors.input)
                                            .bg(colors.background)
                                            .shadow_xs()
                                            .text_xs()
                                            .font_weight(crate::ui::theme::MEDIUM)
                                            .cursor_pointer()
                                            .hover(|style| style.bg(colors.accent))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if !account::is_pro(cx) {
                                                    this.notify_pro(cx);
                                                }
                                            }))
                                            .child(icon("MagicWand01Icon", 16., colors.foreground))
                                            .child("Improve with examples"),
                                    ),
                            )
                            .child(
                                self.gate(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_5()
                                        .child(
                                            div()
                                                .id("auto-format")
                                                .min_h(px(448.))
                                                .px_4()
                                                .py_3()
                                                .border_1()
                                                .border_color(colors.border)
                                                .bg(colors.card)
                                                .font_family(monospace_font(cx))
                                                .text_sm()
                                                .line_height(px(20.))
                                                .cursor_text()
                                                .on_click({
                                                    let focus = editor.focus_handle(cx);
                                                    move |_, window, cx| focus.focus(window, cx)
                                                })
                                                .child(editor.clone()),
                                        )
                                        .child(
                                            div().flex().justify_end().child(
                                                div()
                                                    .id("save-format")
                                                    .h(px(36.))
                                                    .px_4()
                                                    .flex()
                                                    .items_center()
                                                    .bg(colors.primary)
                                                    .text_color(colors.primary_foreground)
                                                    .text_sm()
                                                    .font_weight(crate::ui::theme::MEDIUM)
                                                    .shadow_xs()
                                                    .when_else(
                                                        dirty,
                                                        |button| {
                                                            button
                                                                .cursor_pointer()
                                                                .hover(|style| style.bg(colors.primary.opacity(0.9)))
                                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                                    let source = this
                                                                        .auto
                                                                        .as_ref()
                                                                        .map(|auto| auto.input.read(cx).buffer.text.clone())
                                                                        .unwrap_or_default();
                                                                    this.save_auto_format(source, cx)
                                                                }))
                                                        },
                                                        |button| button.opacity(0.5),
                                                    )
                                                    .child("Save"),
                                            ),
                                        ),
                                    cx,
                                ),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl MenuOwner for TemplatesView {
    fn close_menu(&mut self) {
        self.menu = None;
        self.picker = None;
    }
}

async fn receive<T>(reply: desktop_runtime::Result<Reply<T>>) -> desktop_runtime::Result<T> {
    reply?.receive().await
}

impl Drop for TemplatesView {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Render for TemplatesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !cx.has_active_drag() {
            self.dragged_section = None;
        }
        let colors = theme(window);
        let content = match self.current() {
            Selection::Auto => self.render_auto(window, cx),
            Selection::Mine(_) => self.render_form(window, cx),
        };
        div()
            .relative()
            .size_full()
            .child(content)
            .when(!self.message.is_empty(), |view| {
                view.child(
                    div()
                        .absolute()
                        .bottom_2()
                        .left_6()
                        .text_xs()
                        .text_color(colors.destructive)
                        .child(self.message.clone()),
                )
            })
            .when_some(self.notice.as_ref(), |view, (_, toast)| {
                view.child(toast.render(
                    [
                        Box::new(cx.listener(|this, _, _, cx| {
                            this.notice = None;
                            cx.emit(TemplatesEvent::Product(ProductRoute::Billing));
                            cx.notify();
                        })),
                        Box::new(cx.listener(|this, _, _, cx| {
                            this.notice = None;
                            cx.notify();
                        })),
                        Box::new(cx.listener(|this, _, _, cx| {
                            if let Some((_, toast)) = &mut this.notice {
                                toast.paused = !toast.paused;
                            }
                            cx.notify();
                        })),
                    ],
                    window,
                    colors,
                ))
            })
            .when_some(self.sharing.as_ref(), |view, (dialog, _)| {
                view.child(list::modal(window, dialog.clone().into_any_element()))
            })
    }
}
