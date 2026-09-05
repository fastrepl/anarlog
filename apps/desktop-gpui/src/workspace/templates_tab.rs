//! The Templates tab: `templates/{template-sidebar,details,template-form,
//! sections-editor,auto-form}.tsx` in the user-templates mode (community
//! templates need the web API).

use gpui::{
    AnyElement, ClickEvent, Context, Div, Entity, Focusable as _, MouseButton, SharedString,
    Window, div, prelude::*, px,
};

use super::Workspace;
use super::menu::{Align, Entry, MenuSpec, Select, Trailing};
use super::toast::FlashVariant;
use crate::db::TemplateIcon;
use crate::templates::{AUTO_TEMPLATE_ID, Draft, Section, UserTemplate};
use crate::text_area::{TextArea, TextAreaEvent, TextAreaStyle};
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

pub(crate) struct TemplatesState {
    templates: Vec<UserTemplate>,
    /// `tab.state.selectedMineId`
    selected: Option<String>,
    reverse_alphabetical: bool,
    sort_menu_open: bool,
    search: Entity<TextInput>,
    form: Option<TemplateForm>,
    auto: Option<AutoForm>,
}

/// `TemplateForm` inputs, keyed by the template id.
struct TemplateForm {
    id: String,
    title: Entity<TextInput>,
    description: Entity<TextArea>,
    /// `isAddingTag`
    tag_input: Option<Entity<TextInput>>,
    sections: Vec<SectionForm>,
    next_key: u64,
    actions_open: bool,
}

struct SectionForm {
    key: u64,
    title: Entity<TextInput>,
    description: Entity<TextArea>,
    menu_open: bool,
}

/// `AutoFormatForm`
struct AutoForm {
    editor: Entity<TextArea>,
    /// The value the editor was reset to (`form.reset`).
    baseline: String,
    actions_open: bool,
}

impl Workspace {
    fn plain_input_style(&self) -> TextInputStyle {
        TextInputStyle {
            text: self.theme.foreground,
            placeholder: self.theme.muted_foreground,
            selection: self.theme.selection,
            underline_when_focused: false,
            masked: false,
        }
    }

    /// `openNew({ type: "templates" })`, optionally selecting a template.
    pub(crate) fn open_templates(
        &mut self,
        select: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_settings(cx);
        self.close_folders(cx);
        self.close_calendar(cx);
        self.close_contacts(cx);
        if self.templates_tab.is_none() {
            let style = self.plain_input_style();
            let search = cx.new(|cx| TextInput::new("Search templates...", style, window, cx));
            cx.subscribe(&search, |this, input, event: &TextInputEvent, cx| {
                match event {
                    TextInputEvent::Escape => input.update(cx, |input, cx| input.set_text("", cx)),
                    TextInputEvent::Changed => {}
                    _ => return,
                }
                if this.templates_tab.is_some() {
                    cx.notify();
                }
            })
            .detach();
            self.templates_tab = Some(TemplatesState {
                templates: Vec::new(),
                selected: None,
                reverse_alphabetical: false,
                sort_menu_open: false,
                search,
                form: None,
                auto: None,
            });
        }
        if let (Some(state), Some(select)) = (self.templates_tab.as_mut(), select) {
            state.selected = Some(select);
        }
        self.reload_templates_tab(window, cx);
        cx.notify();
    }

    pub(crate) fn close_templates(&mut self, cx: &mut Context<Self>) {
        if self.templates_tab.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn templates_open(&self) -> bool {
        self.templates_tab.is_some()
    }

    pub(crate) fn reload_templates_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.templates_tab.is_none() {
            return;
        }
        let task = self.store.list_user_templates();
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(templates)) = task.await else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                if let Some(state) = this.templates_tab.as_mut() {
                    state.templates = templates;
                }
                this.sync_template_form(window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The change watcher has no window: refresh the list and the form values
    /// (inputs the user is not typing in) without rebuilding.
    pub(crate) fn reload_templates_from_watcher(&mut self, cx: &mut Context<Self>) {
        if self.templates_tab.is_none() {
            return;
        }
        let task = self.store.list_user_templates();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(templates)) = task.await else {
                return;
            };
            this.update(cx, |this, cx| {
                if let Some(state) = this.templates_tab.as_mut() {
                    state.templates = templates;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// `resolveTemplateTabSelection` in user mode.
    fn resolved_template_id(&self) -> Option<String> {
        let state = self.templates_tab.as_ref()?;
        match state.selected.as_deref() {
            Some(AUTO_TEMPLATE_ID) => Some(AUTO_TEMPLATE_ID.to_string()),
            selected => Some(
                state
                    .templates
                    .iter()
                    .find(|template| Some(template.id.as_str()) == selected)
                    .or_else(|| state.templates.first())
                    .map(|template| template.id.clone())
                    .unwrap_or_else(|| AUTO_TEMPLATE_ID.to_string()),
            ),
        }
    }

    fn select_template(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.templates_tab.as_mut() {
            state.selected = Some(id);
        }
        self.sync_template_form(window, cx);
        cx.notify();
    }

    /// `<TemplateForm key={template.id}>` / `<AutoFormatForm key=...>`.
    fn sync_template_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.resolved_template_id() else {
            return;
        };
        if id == AUTO_TEMPLATE_ID {
            if let Some(state) = self.templates_tab.as_mut() {
                state.form = None;
            }
            self.ensure_auto_form(window, cx);
            return;
        }
        if let Some(state) = self.templates_tab.as_mut() {
            state.auto = None;
        }
        if self
            .templates_tab
            .as_ref()
            .and_then(|state| state.form.as_ref())
            .is_some_and(|form| form.id == id)
        {
            return;
        }
        let Some(template) = self
            .templates_tab
            .as_ref()
            .and_then(|state| state.templates.iter().find(|t| t.id == id).cloned())
        else {
            return;
        };
        let style = self.plain_input_style();
        let title = cx.new(|cx| {
            let mut input = TextInput::new("Enter template title", style, window, cx);
            input.set_text(template.title.clone(), cx);
            input
        });
        cx.subscribe(&title, |this, _, event: &TextInputEvent, cx| {
            if *event == TextInputEvent::Changed {
                this.save_template_form(cx);
            }
        })
        .detach();
        let description = cx.new(|cx| {
            let mut area = TextArea::new(
                "Describe the template purpose...",
                TextAreaStyle {
                    text: self.theme.muted_foreground,
                    placeholder: self.theme.muted_foreground,
                    selection: self.theme.selection,
                    font_size: px(14.0),
                    line_height: px(20.0),
                    rows: 1,
                },
                window,
                cx,
            );
            area.set_text(template.description.clone(), cx);
            area
        });
        cx.subscribe(&description, |this, _, event: &TextAreaEvent, cx| {
            if *event == TextAreaEvent::Changed {
                this.save_template_form(cx);
            }
        })
        .detach();
        let mut form = TemplateForm {
            id: id.clone(),
            title,
            description,
            tag_input: None,
            sections: Vec::new(),
            next_key: 0,
            actions_open: false,
        };
        for section in &template.sections {
            let section_form = self.new_section_form(section, &mut form.next_key, window, cx);
            form.sections.push(section_form);
        }
        if let Some(state) = self.templates_tab.as_mut() {
            state.form = Some(form);
        }
    }

    fn new_section_form(
        &self,
        section: &Section,
        next_key: &mut u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> SectionForm {
        let key = *next_key;
        *next_key += 1;
        let style = self.plain_input_style();
        let title = cx.new(|cx| {
            let mut input = TextInput::new("Untitled", style, window, cx);
            input.set_text(section.title.clone(), cx);
            input
        });
        cx.subscribe(&title, |this, _, event: &TextInputEvent, cx| {
            if *event == TextInputEvent::Changed {
                this.save_template_form(cx);
            }
        })
        .detach();
        let description = cx.new(|cx| {
            let mut area = TextArea::new(
                "Template content with Jinja2: {{ variable }}, {% if condition %}",
                TextAreaStyle {
                    text: self.theme.foreground,
                    placeholder: self.theme.muted_foreground,
                    selection: self.theme.selection,
                    font_size: px(14.0),
                    line_height: px(20.0),
                    rows: 1,
                },
                window,
                cx,
            );
            area.set_text(section.description.clone(), cx);
            area
        });
        cx.subscribe(&description, |this, _, event: &TextAreaEvent, cx| {
            if matches!(event, TextAreaEvent::Changed | TextAreaEvent::Blurred) {
                if *event == TextAreaEvent::Changed {
                    this.save_template_form(cx);
                }
                cx.notify();
            }
        })
        .detach();
        SectionForm {
            key,
            title,
            description,
            menu_open: false,
        }
    }

    /// `useForm.onChange -> handleSubmit -> saveTemplate({ ...template, ...value })`.
    fn save_template_form(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.templates_tab.as_ref() else {
            return;
        };
        let Some(form) = state.form.as_ref() else {
            return;
        };
        let Some(template) = state.templates.iter().find(|t| t.id == form.id) else {
            return;
        };
        let mut next = template.clone();
        next.title = form.title.read(cx).text().to_string();
        next.description = form.description.read(cx).text().to_string();
        next.sections = form
            .sections
            .iter()
            .map(|section| Section {
                title: section.title.read(cx).text().to_string(),
                description: section.description.read(cx).text().to_string(),
            })
            .collect();
        if next == *template {
            return;
        }
        if let Some(state) = self.templates_tab.as_mut()
            && let Some(slot) = state.templates.iter_mut().find(|t| t.id == next.id)
        {
            *slot = next.clone();
        }
        let task = self.store.save_user_template(next);
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

    /// Writes a field outside the text inputs (icon, targets, sections order).
    fn update_template<F: FnOnce(&mut UserTemplate)>(
        &mut self,
        id: &str,
        edit: F,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.templates_tab.as_mut() else {
            return;
        };
        let Some(template) = state.templates.iter_mut().find(|t| t.id == id) else {
            return;
        };
        edit(template);
        let next = template.clone();
        cx.notify();
        let task = self.store.save_user_template(next);
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

    /// `createDefaultTemplate` / `handleDuplicateTemplate`: create, then select.
    fn create_template_from(&mut self, draft: Draft, window: &mut Window, cx: &mut Context<Self>) {
        let task = self.store.create_user_template(draft);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| match result {
                Ok(id) => {
                    if let Some(state) = this.templates_tab.as_mut() {
                        state.selected = Some(id);
                    }
                    this.reload_templates_tab(window, cx);
                }
                Err(error) => this.flash(FlashVariant::Error, error.to_string(), cx),
            })
            .ok();
        })
        .detach();
    }

    fn duplicate_template(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(template) = self
            .templates_tab
            .as_ref()
            .and_then(|state| state.templates.iter().find(|t| t.id == id).cloned())
        else {
            return;
        };
        self.create_template_from(
            Draft {
                title: crate::templates::copy_title(&template.title),
                description: template.description.clone(),
                category: template.category.clone(),
                icon: Some(template.icon.clone()),
                targets: template.targets.clone(),
                sections: template.sections.clone(),
            },
            window,
            cx,
        );
    }

    fn delete_template(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let task = self.store.delete_user_template(id.clone());
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.flash(FlashVariant::Error, error.to_string(), cx);
                } else if let Some(state) = this.templates_tab.as_mut() {
                    if state.selected.as_deref() == Some(id.as_str()) {
                        state.selected = None;
                    }
                    if state.form.as_ref().is_some_and(|form| form.id == id) {
                        state.form = None;
                    }
                }
                this.reload_templates_tab(window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn toggle_template_favorite(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = self.store.toggle_template_favorite(id);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.flash(FlashVariant::Error, error.to_string(), cx);
                }
                this.reload_templates_tab(window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// `selected_template_id`: `""` means Auto.
    fn selected_template_setting(&self) -> String {
        self.provider_settings
            .value("selected_template_id", &["general", "selected_template_id"])
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default()
    }

    fn auto_summary_prompt(&self) -> String {
        self.provider_settings
            .value("auto_summary_prompt", &["ai", "auto_summary_prompt"])
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default()
    }

    /// `notifyPlanRequired("pro")`
    pub(crate) fn notify_pro_required(&mut self, cx: &mut Context<Self>) {
        self.flash(FlashVariant::Warning, "This requires Anarlog Pro", cx);
    }

    fn ensure_auto_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let default_format = crate::templates::default_auto_format();
        let override_ = crate::templates::normalize_format(&self.auto_summary_prompt());
        let customized = !override_.is_empty()
            && override_ != crate::templates::normalize_format(default_format);
        let initial = if customized {
            override_
        } else {
            default_format.to_string()
        };
        if self
            .templates_tab
            .as_ref()
            .and_then(|state| state.auto.as_ref())
            .is_some_and(|auto| auto.baseline == initial)
        {
            return;
        }
        let theme = self.theme;
        let editor = cx.new(|cx| {
            let mut area = TextArea::new(
                "",
                TextAreaStyle {
                    text: theme.foreground,
                    placeholder: theme.muted_foreground,
                    selection: theme.selection,
                    font_size: px(14.0),
                    line_height: px(20.0),
                    rows: 1,
                },
                window,
                cx,
            );
            area.set_text(initial.clone(), cx);
            area
        });
        cx.subscribe(&editor, |this, _, event: &TextAreaEvent, cx| {
            if *event == TextAreaEvent::Changed && this.templates_tab.is_some() {
                cx.notify();
            }
        })
        .detach();
        if let Some(state) = self.templates_tab.as_mut() {
            state.auto = Some(AutoForm {
                editor,
                baseline: initial,
                actions_open: false,
            });
        }
    }

    /// `TemplatesSidebarContent`
    pub(super) fn render_templates_sidebar(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let Some(state) = self.templates_tab.as_ref() else {
            return div();
        };
        let query = state.search.read(cx).text().trim().to_lowercase();
        let searching = !query.is_empty();
        let selected = self.resolved_template_id();
        let auto_customized = !self.auto_summary_prompt().trim().is_empty();

        // `sortedUserTemplates` + `filteredMine`
        let mut favorites: Vec<&UserTemplate> =
            state.templates.iter().filter(|t| t.pinned).collect();
        favorites.sort_by(|a, b| {
            let order = a
                .pin_order
                .unwrap_or(i64::MAX)
                .cmp(&b.pin_order.unwrap_or(i64::MAX));
            order.then_with(|| a.title.cmp(&b.title))
        });
        let mut others: Vec<&UserTemplate> = state.templates.iter().filter(|t| !t.pinned).collect();
        others.sort_by(|a, b| {
            if state.reverse_alphabetical {
                b.title.cmp(&a.title)
            } else {
                a.title.cmp(&b.title)
            }
        });
        let mine: Vec<&UserTemplate> = favorites
            .into_iter()
            .chain(others)
            .filter(|template| {
                query.is_empty()
                    || template.title.to_lowercase().contains(&query)
                    || template.description.to_lowercase().contains(&query)
                    || template
                        .category
                        .as_deref()
                        .is_some_and(|category| category.to_lowercase().contains(&query))
                    || template.targets.as_ref().is_some_and(|targets| {
                        targets.iter().any(|t| t.to_lowercase().contains(&query))
                    })
            })
            .collect();
        let show_auto = query.is_empty() || "auto".contains(query.as_str());
        let is_empty = !show_auto && mine.is_empty();
        let search_input = state.search.clone();
        let focus_input = search_input.clone();

        // GPUI caches a nowrap text's first (max-content) measurement, so the
        // truncating block gets its exact width: the sidebar minus the row's
        // `px-3`, the 16px glyph, and the `gap-2`.
        let title_width = px(self.sidebar_width - 52.0);
        let row = |id: SharedString, is_selected: bool| {
            div()
                .id(id)
                .w_full()
                .rounded_lg()
                .px_3()
                .py_2()
                .tw_text_sm()
                .cursor_pointer()
                .when(is_selected, |row| row.bg(theme.accent))
                .when(!is_selected, |row| {
                    row.hover(move |style| style.bg(alpha(theme.accent, 0.5)))
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        };

        let mut list = div().pt_1().w_full();
        if show_auto {
            let is_selected = selected.as_deref() == Some(AUTO_TEMPLATE_ID);
            list = list.child(
                row("template-row-auto".into(), is_selected)
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.select_template(AUTO_TEMPLATE_ID.to_string(), window, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .min_w_0()
                            .items_center()
                            .gap_2()
                            .child(icon("sparkle", px(16.0), gpui::rgb(0x8e51ff)))
                            .child(
                                // `flex-grow` with an auto basis keeps GPUI's truncation working.
                                div()
                                    .min_w_0()
                                    .flex_grow()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .w(title_width)
                                            .truncate()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.foreground)
                                            .child("Auto"),
                                    )
                                    .when(auto_customized, |column| {
                                        column.child(
                                            div()
                                                .w(title_width)
                                                .truncate()
                                                .tw_text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child("Customized"),
                                        )
                                    }),
                            ),
                    ),
            );
        }
        for template in &mine {
            let is_selected = selected.as_deref() == Some(template.id.as_str());
            let id = template.id.clone();
            let title = if template.title.trim().is_empty() {
                "Untitled".to_string()
            } else {
                template.title.trim().to_string()
            };
            list = list.child(
                row(format!("template-row-{}", template.id).into(), is_selected)
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.select_template(id.clone(), window, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .min_w_0()
                            .items_center()
                            .gap_2()
                            .child(self.template_icon_glyph(&template.icon, px(16.0)))
                            .child(
                                // A flex column stretches the text block to a definite
                                // width, which GPUI's truncation needs at measure time.
                                div().min_w_0().flex_grow().flex().flex_col().child(
                                    div()
                                        .w(title_width)
                                        .truncate()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(theme.foreground)
                                        .child(SharedString::from(title)),
                                ),
                            ),
                    ),
            );
        }

        let mut header_actions = div().flex().items_center().gap_1();
        if state.templates.len() > 1 {
            header_actions = header_actions.child(
                div()
                    .relative()
                    .child(
                        self.tracked_chrome_button("templates-sort", cx)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(state) = this.templates_tab.as_mut() {
                                    state.sort_menu_open = !state.sort_menu_open;
                                    cx.notify();
                                }
                            }))
                            .child(icon(
                                "arrows-down-up",
                                px(16.0),
                                self.chrome_icon_color("templates-sort"),
                            )),
                    )
                    .when(state.sort_menu_open, |anchor| {
                        let spec = MenuSpec {
                            id: "templates-sort-menu",
                            width: 160.0,
                            entries: vec![
                                Entry::Item {
                                    icon: None,
                                    dim_icon: false,
                                    label: "A to Z".into(),
                                    trailing: Trailing::None,
                                    destructive: false,
                                    on_select: Some(Box::new(|this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                                        if let Some(state) = this.templates_tab.as_mut() {
                                            state.reverse_alphabetical = false;
                                            cx.notify();
                                        }
                                    }) as Select),
                                    submenu: None,
                                },
                                Entry::Item {
                                    icon: None,
                                    dim_icon: false,
                                    label: "Z to A".into(),
                                    trailing: Trailing::None,
                                    destructive: false,
                                    on_select: Some(Box::new(|this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                                        if let Some(state) = this.templates_tab.as_mut() {
                                            state.reverse_alphabetical = true;
                                            cx.notify();
                                        }
                                    }) as Select),
                                    submenu: None,
                                },
                            ],
                            open_sub: None,
                            on_hover_sub: |_, _, _| {},
                            on_close: |this, cx| {
                                if let Some(state) = this.templates_tab.as_mut() {
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
            );
        }
        header_actions = header_actions.child(
            self.tracked_chrome_button("templates-new", cx)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.create_template_from(
                        Draft {
                            title: "New Template".to_string(),
                            ..Draft::default()
                        },
                        window,
                        cx,
                    );
                }))
                .child(icon(
                    "plus",
                    px(16.0),
                    self.chrome_icon_color("templates-new"),
                )),
        );

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
                                self.tracked_chrome_button("templates-back", cx)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.close_templates(cx);
                                    }))
                                    .child(icon(
                                        "arrow-left",
                                        px(16.0),
                                        self.chrome_icon_color("templates-back"),
                                    )),
                            )
                            .child(div().flex_1())
                            .child(header_actions),
                    ),
            )
            .child(
                div().pb_2().child(
                    div()
                        .id("templates-search")
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
                            Some(alpha(theme.accent, 0.5)),
                            Some((1.0, theme.border)),
                        ))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                            focus_input.read(cx).focus_handle(cx).focus(window);
                        }))
                        .child(icon("search", px(16.0), theme.muted_foreground))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .tw_text_sm()
                                .child(state.search.clone()),
                        )
                        .when(searching, |row| {
                            row.child(
                                div()
                                    .id("templates-search-clear")
                                    .size(px(16.0))
                                    .flex_shrink_0()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
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
                    .id("templates-list")
                    .min_h_0()
                    .w_full()
                    .flex_1()
                    .overflow_y_scroll()
                    .child(if is_empty {
                        div()
                            .px_3()
                            .py_8()
                            .flex()
                            .flex_col()
                            .items_center()
                            .text_color(theme.muted_foreground)
                            .child(div().mb_2().child(icon(
                                "book-open-text",
                                px(32.0),
                                alpha(theme.muted_foreground, 0.7),
                            )))
                            .child(div().tw_text_sm().child(if searching {
                                "No templates found"
                            } else {
                                "No templates yet"
                            }))
                            .when(!searching, |column| {
                                column.child(
                                    div()
                                        .id("templates-create-first")
                                        .mt_3()
                                        .tw_text_sm()
                                        .text_decoration_1()
                                        .cursor_pointer()
                                        .hover(move |style| style.text_color(theme.foreground))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, window, cx| {
                                                this.create_template_from(
                                                    Draft {
                                                        title: "New Template".to_string(),
                                                        ..Draft::default()
                                                    },
                                                    window,
                                                    cx,
                                                );
                                            },
                                        ))
                                        .child("Create my first template"),
                                )
                            })
                            .into_any_element()
                    } else {
                        list.into_any_element()
                    }),
            )
    }

    pub(super) fn template_icon_glyph(
        &self,
        glyph: &TemplateIcon,
        size: gpui::Pixels,
    ) -> AnyElement {
        match glyph {
            TemplateIcon::Emoji(value) => div()
                .flex()
                .size(size)
                .items_center()
                .justify_center()
                .tw_text_sm()
                .child(SharedString::from(value.clone()))
                .into_any_element(),
            TemplateIcon::Icon { name, color } => icon(
                super::note::template_icon_asset(name),
                size,
                super::note::parse_hex_color(color).unwrap_or(gpui::rgb(0x9ca3af)),
            )
            .into_any_element(),
        }
    }

    /// `TemplateDetailsColumn` in user mode.
    pub(super) fn render_templates_main(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let Some(state) = self.templates_tab.as_ref() else {
            return div().into_any_element();
        };
        match self.resolved_template_id().as_deref() {
            Some(AUTO_TEMPLATE_ID) => self.render_auto_details(cx),
            Some(id) => match (
                state.templates.iter().find(|t| t.id == id),
                state.form.as_ref().filter(|form| form.id == id),
            ) {
                (Some(template), Some(form)) => {
                    self.render_template_form(template, form, window, cx)
                }
                _ => div().into_any_element(),
            },
            None => div()
                .flex()
                .h_full()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(
                    div()
                        .tw_text_sm()
                        .text_color(theme.muted_foreground)
                        .child("No templates yet"),
                )
                .into_any_element(),
        }
    }

    /// `Button size="sm" variant="ghost"` text buttons in the detail headers.
    fn ghost_text_button(
        &self,
        id: &'static str,
        content: AnyElement,
        color: gpui::Rgba,
        hover_color: gpui::Rgba,
        on_click: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        let theme = self.theme;
        div()
            .id(id)
            .flex()
            .h(px(32.0))
            .flex_shrink_0()
            .items_center()
            .gap(px(6.0))
            .px_3()
            .rounded(px(8.0))
            .tw_text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(color)
            .cursor_pointer()
            .hover(move |style| style.bg(theme.accent).text_color(hover_color))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(
                cx.listener(move |this, _: &ClickEvent, window, cx| on_click(this, window, cx)),
            )
            .child(content)
    }

    fn ghost_icon_button(
        &self,
        id: SharedString,
        glyph: &'static str,
        color: gpui::Rgba,
        active: bool,
        on_click: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        let theme = self.theme;
        div()
            .id(id)
            .flex()
            .size(px(32.0))
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .rounded(px(8.0))
            .when(active, |button| button.bg(theme.muted))
            .cursor_pointer()
            .hover(move |style| style.bg(theme.accent))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(
                cx.listener(move |this, _: &ClickEvent, window, cx| on_click(this, window, cx)),
            )
            .child(icon(glyph, px(16.0), color))
    }

    /// The `Set as default` / `Current default` toggle shared by both details.
    fn default_toggle(&self, id: &str, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let is_default = if id == AUTO_TEMPLATE_ID {
            self.selected_template_setting().is_empty()
        } else {
            self.selected_template_setting() == id
        };
        let next: serde_json::Value = if id == AUTO_TEMPLATE_ID || is_default {
            serde_json::Value::String(String::new())
        } else {
            serde_json::Value::String(id.to_string())
        };
        let emerald = gpui::rgb(0x009966);
        let content = if is_default {
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(icon("check", px(14.0), emerald))
                .child("Current default")
                .into_any_element()
        } else {
            div().child("Set as default").into_any_element()
        };
        let (color, hover) = if is_default {
            (emerald, gpui::rgb(0x007a55))
        } else {
            (theme.muted_foreground, gpui::rgb(0x000000))
        };
        self.ghost_text_button(
            "template-default",
            content,
            color,
            hover,
            move |this, _, cx| {
                if id_is_auto_and_default(is_default, &next) {
                    return;
                }
                this.set_setting("selected_template_id", next.clone(), cx);
            },
            cx,
        )
        .into_any_element()
    }

    /// `TemplateForm`
    fn render_template_form(
        &self,
        template: &UserTemplate,
        form: &TemplateForm,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let id = template.id.clone();
        let title_text = form.title.read(cx).text().to_string();
        let focus_title = form.title.clone();

        let header = div()
            .flex()
            .h(px(48.0))
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
                            .id("template-icon")
                            .flex()
                            .size(px(28.0))
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .hover(move |style| style.bg(theme.accent))
                            .child(self.template_icon_glyph(&template.icon, px(16.0))),
                    )
                    .child(
                        div()
                            .id("template-title")
                            .relative()
                            .min_w_0()
                            .max_w_full()
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                                focus_title.read(cx).focus_handle(cx).focus(window);
                            }))
                            .child(
                                div()
                                    .invisible()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .whitespace_nowrap()
                                    .child(SharedString::from(format!(
                                        "{} ",
                                        if title_text.is_empty() {
                                            "Enter template title"
                                        } else {
                                            title_text.as_str()
                                        }
                                    ))),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(form.title.clone()),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .id("template-share")
                            .flex()
                            .h(px(32.0))
                            .items_center()
                            .gap(px(6.0))
                            .px_3()
                            .rounded(px(8.0))
                            .tw_text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent).text_color(theme.foreground))
                            .child(icon("share-network", px(16.0), theme.muted_foreground))
                            .child("Share"),
                    )
                    .child(self.default_toggle(&template.id, cx))
                    .child({
                        let id = id.clone();
                        let pinned = template.pinned;
                        self.ghost_icon_button(
                            "template-favorite".into(),
                            "heart",
                            if pinned {
                                gpui::rgb(0xff2056)
                            } else {
                                theme.muted_foreground
                            },
                            false,
                            move |this, window, cx| {
                                this.toggle_template_favorite(id.clone(), window, cx)
                            },
                            cx,
                        )
                    })
                    .child(self.render_template_actions(&id, form.actions_open, cx)),
            );

        let body = div()
            .id("template-body")
            .min_h_0()
            .flex_1()
            .overflow_y_scroll()
            .px_6()
            .pt_3()
            .pb_6()
            .child(
                div()
                    .min_w_0()
                    .child(
                        // `Textarea rows={1}`: `min-h-[24px] text-sm text-muted-foreground`, borderless.
                        div()
                            .id("template-description")
                            .min_h(px(24.0))
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click({
                                let area = form.description.clone();
                                move |_: &ClickEvent, window, cx| {
                                    area.update(cx, |area, cx| area.focus_end(window, cx));
                                }
                            })
                            .child(form.description.clone()),
                    )
                    .child(self.render_template_targets(template, form, cx)),
            )
            .child(
                div()
                    .mt_6()
                    .child(self.render_sections(template, form, window, cx)),
            );

        div()
            .flex()
            .h_full()
            .flex_1()
            .flex_col()
            .child(header)
            .child(body)
            .into_any_element()
    }

    /// The `DotsThree` menu: Duplicate / Delete.
    fn render_template_actions(&self, id: &str, open: bool, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let id = id.to_string();
        let dup_id = id.clone();
        let del_id = id.clone();
        div()
            .relative()
            .child(self.ghost_icon_button(
                "template-actions".into(),
                "more-horizontal",
                if open {
                    theme.foreground
                } else {
                    theme.muted_foreground
                },
                open,
                |this, _, cx| {
                    if let Some(form) = this.templates_tab.as_mut().and_then(|s| s.form.as_mut()) {
                        form.actions_open = !form.actions_open;
                        cx.notify();
                    }
                },
                cx,
            ))
            .when(open, |anchor| {
                let spec = MenuSpec {
                    id: "template-actions-menu",
                    width: 160.0,
                    entries: vec![
                        Entry::Item {
                            icon: None,
                            dim_icon: false,
                            label: "Duplicate".into(),
                            trailing: Trailing::None,
                            destructive: false,
                            on_select: Some(Box::new(
                                move |this: &mut Workspace,
                                      window: &mut Window,
                                      cx: &mut Context<Workspace>| {
                                    this.duplicate_template(&dup_id, window, cx);
                                },
                            ) as Select),
                            submenu: None,
                        },
                        Entry::Item {
                            icon: None,
                            dim_icon: false,
                            label: "Delete".into(),
                            trailing: Trailing::None,
                            destructive: true,
                            on_select: Some(Box::new(
                                move |this: &mut Workspace,
                                      window: &mut Window,
                                      cx: &mut Context<Workspace>| {
                                    this.delete_template(del_id.clone(), window, cx);
                                },
                            ) as Select),
                            submenu: None,
                        },
                    ],
                    open_sub: None,
                    on_hover_sub: |_, _, _| {},
                    on_close: |this, cx| {
                        if let Some(form) =
                            this.templates_tab.as_mut().and_then(|s| s.form.as_mut())
                        {
                            form.actions_open = false;
                            cx.notify();
                        }
                    },
                };
                anchor.child(
                    div()
                        .absolute()
                        .top(px(36.0))
                        .right_0()
                        .child(self.render_menu_inline(spec, Align::End, cx)),
                )
            })
    }

    /// `TemplateTargetsInput`: badges, the Add tag button, or the inline input.
    fn render_template_targets(
        &self,
        template: &UserTemplate,
        form: &TemplateForm,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        let theme = self.theme;
        let id = template.id.clone();
        let targets = template.targets.clone().unwrap_or_default();
        let mut row = div()
            .id("template-targets")
            .mt_2()
            .flex()
            .min_h(px(24.0))
            .w_full()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .cursor_text()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.start_adding_tag(window, cx);
            }));
        for (index, target) in targets.iter().enumerate() {
            let remove_id = id.clone();
            row = row.child(
                div()
                    .flex()
                    .h(px(24.0))
                    .items_center()
                    .gap_1()
                    .rounded_md()
                    .bg(theme.muted)
                    .px_2()
                    .py(px(2.0))
                    .tw_text_xs()
                    .text_color(theme.foreground)
                    .child(SharedString::from(target.clone()))
                    .child(
                        div()
                            .id(SharedString::from(format!("target-remove-{index}")))
                            .ml(px(2.0))
                            .flex()
                            .size(px(12.0))
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.update_template(
                                    &remove_id,
                                    |template| {
                                        if let Some(targets) = template.targets.as_mut()
                                            && index < targets.len()
                                        {
                                            targets.remove(index);
                                        }
                                    },
                                    cx,
                                );
                            }))
                            .child(icon("x", px(10.0), theme.foreground)),
                    ),
            );
        }
        match &form.tag_input {
            None => {
                row = row.child(
                    div()
                        .id("template-add-tag")
                        .flex()
                        .h(px(24.0))
                        .items_center()
                        .gap_1()
                        .rounded_md()
                        .bg(theme.muted)
                        .px_2()
                        .py(px(2.0))
                        .tw_text_xs()
                        .text_color(theme.muted_foreground)
                        .cursor_pointer()
                        .hover(move |style| style.bg(alpha(theme.muted, 0.8)))
                        .child(icon("plus", px(12.0), theme.muted_foreground))
                        .child("Add tag"),
                );
            }
            Some(input) => {
                row = row.child(
                    div()
                        .min_w(px(84.0))
                        .flex_1()
                        .tw_text_xs()
                        .text_color(theme.muted_foreground)
                        .child(input.clone()),
                );
            }
        }
        row
    }

    fn start_adding_tag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let style = TextInputStyle {
            text: self.theme.muted_foreground,
            placeholder: self.theme.muted_foreground,
            selection: self.theme.selection,
            underline_when_focused: false,
            masked: false,
        };
        let Some(form) = self.templates_tab.as_mut().and_then(|s| s.form.as_mut()) else {
            return;
        };
        if let Some(input) = &form.tag_input {
            input.read(cx).focus_handle(cx).focus(window);
            return;
        }
        let input = cx.new(|cx| TextInput::new("", style, window, cx));
        cx.subscribe(
            &input,
            |this, input, event: &TextInputEvent, cx| match event {
                TextInputEvent::Enter | TextInputEvent::Committed => this.submit_tags(cx),
                TextInputEvent::Escape => {
                    if let Some(form) = this.templates_tab.as_mut().and_then(|s| s.form.as_mut()) {
                        form.tag_input = None;
                        cx.notify();
                    }
                }
                TextInputEvent::BackspaceEmpty => {
                    let id = this
                        .templates_tab
                        .as_ref()
                        .and_then(|s| s.form.as_ref())
                        .map(|f| f.id.clone());
                    if let Some(id) = id {
                        this.update_template(
                            &id,
                            |template| {
                                if let Some(targets) = template.targets.as_mut() {
                                    targets.pop();
                                }
                            },
                            cx,
                        );
                    }
                }
                TextInputEvent::Changed => {
                    // `,` submits like Enter.
                    if input.read(cx).text().contains(',') {
                        this.submit_tags(cx);
                    }
                }
                _ => {}
            },
        )
        .detach();
        input.read(cx).focus_handle(cx).focus(window);
        form.tag_input = Some(input);
        cx.notify();
    }

    /// `submitTargets`: comma-split, trimmed, appended; the input closes.
    fn submit_tags(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.templates_tab.as_mut().and_then(|s| s.form.as_mut()) else {
            return;
        };
        let Some(input) = form.tag_input.take() else {
            return;
        };
        let id = form.id.clone();
        let next: Vec<String> = input
            .read(cx)
            .text()
            .split(',')
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .map(str::to_string)
            .collect();
        cx.notify();
        if next.is_empty() {
            return;
        }
        self.update_template(
            &id,
            |template| {
                template.targets.get_or_insert_with(Vec::new).extend(next);
            },
            cx,
        );
    }

    /// `SectionsList`
    fn render_sections(
        &self,
        template: &UserTemplate,
        form: &TemplateForm,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let id = template.id.clone();
        let total = form.sections.len();
        let mut list = div().flex().flex_col().gap_2();
        for (index, section) in form.sections.iter().enumerate() {
            let focused = section
                .description
                .read(cx)
                .focus_handle(cx)
                .is_focused(window);
            let key = section.key;
            let hovered =
                self.hovered == Some("template-section") && self.hovered_section == Some(key);
            let menu_open = section.menu_open;
            list = list.child(
                div()
                    .id(SharedString::from(format!("template-section-{key}")))
                    .relative()
                    .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                        this.hovered_section = if *hovering { Some(key) } else { None };
                        this.set_hovered("template-section", *hovering, cx);
                    }))
                    // The `-left-5` drag handle, visible on hover.
                    .when(hovered, |item| {
                        item.child(
                            div()
                                .absolute()
                                .top(px(10.0))
                                .left(px(-20.0))
                                .opacity(0.3)
                                .child(icon("grip-vertical", px(16.0), theme.muted_foreground)),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .pr(px(36.0))
                            .child(
                                div()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .h(px(36.0))
                                    .flex()
                                    .items_center()
                                    .child(section.title.clone()),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("section-description-{key}")))
                                    .min_h(px(100.0))
                                    .w_full()
                                    .rounded_xl()
                                    .border_1()
                                    .cursor_text()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click({
                                        let area = section.description.clone();
                                        move |_: &ClickEvent, window, cx| {
                                            area.update(cx, |area, cx| area.focus_end(window, cx));
                                        }
                                    })
                                    .border_color(if focused {
                                        gpui::rgb(0x2b7fff)
                                    } else {
                                        theme.border
                                    })
                                    .when(focused, |area| {
                                        area.shadow(vec![gpui::BoxShadow {
                                            color: alpha(theme.primary, 0.2).into(),
                                            offset: gpui::point(px(0.0), px(0.0)),
                                            blur_radius: px(0.0),
                                            spread_radius: px(2.0),
                                        }])
                                    })
                                    .p_3()
                                    .tw_text_sm()
                                    .child(section.description.clone()),
                            ),
                    )
                    .when(hovered || menu_open, |item| {
                        let (up_id, down_id, above_id, below_id, del_id) =
                            (id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
                        let entry =
                            |label: &'static str,
                             destructive: bool,
                             enabled: bool,
                             action: Select| Entry::Item {
                                icon: None,
                                dim_icon: false,
                                label: label.into(),
                                trailing: Trailing::None,
                                destructive,
                                on_select: enabled.then_some(action),
                                submenu: None,
                            };
                        item.child(
                            div()
                                .absolute()
                                .top(px(8.0))
                                .right(px(8.0))
                                .child(
                                    div()
                                        .id(SharedString::from(format!("section-actions-{key}")))
                                        .flex()
                                        .size(px(28.0))
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(8.0))
                                        .cursor_pointer()
                                        .hover(move |style| style.bg(theme.accent))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _, cx| {
                                                if let Some(section) = this.section_form_mut(key) {
                                                    section.menu_open = !section.menu_open;
                                                    cx.notify();
                                                }
                                            },
                                        ))
                                        .child(icon(
                                            "more-horizontal",
                                            px(16.0),
                                            theme.muted_foreground,
                                        )),
                                )
                                .when(menu_open, |anchor| {
                                    let spec = MenuSpec {
                                        id: "section-actions-menu",
                                        width: 160.0,
                                        entries: vec![
                                            entry(
                                                "Insert above",
                                                false,
                                                true,
                                                Box::new(move |this, window, cx| {
                                                    this.insert_section_at(
                                                        &above_id, index, window, cx,
                                                    );
                                                }),
                                            ),
                                            entry(
                                                "Insert below",
                                                false,
                                                true,
                                                Box::new(move |this, window, cx| {
                                                    this.insert_section_at(
                                                        &below_id,
                                                        index + 1,
                                                        window,
                                                        cx,
                                                    );
                                                }),
                                            ),
                                            entry(
                                                "Move up",
                                                false,
                                                index > 0,
                                                Box::new(move |this, _, cx| {
                                                    this.move_section(&up_id, index, -1, cx);
                                                }),
                                            ),
                                            entry(
                                                "Move down",
                                                false,
                                                index + 1 < total,
                                                Box::new(move |this, _, cx| {
                                                    this.move_section(&down_id, index, 1, cx);
                                                }),
                                            ),
                                            entry(
                                                "Delete",
                                                true,
                                                true,
                                                Box::new(move |this, _, cx| {
                                                    this.delete_section(&del_id, index, cx);
                                                }),
                                            ),
                                        ],
                                        open_sub: None,
                                        on_hover_sub: |_, _, _| {},
                                        on_close: |this, cx| {
                                            if let Some(form) = this
                                                .templates_tab
                                                .as_mut()
                                                .and_then(|s| s.form.as_mut())
                                            {
                                                for section in &mut form.sections {
                                                    section.menu_open = false;
                                                }
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
                    }),
            );
        }
        div().flex().flex_col().gap_3().child(list).child(
            // `Add Section`: `rounded-full px-4 py-2.5 text-sm border bg-card` with the soft shadow.
            div()
                .id("template-add-section")
                .flex()
                .w_auto()
                .items_center()
                .gap_2()
                .rounded(px(8.0))
                .border_1()
                .border_color(theme.border)
                .bg(theme.card)
                .px_4()
                .py(px(10.0))
                .tw_text_sm()
                .text_color(theme.foreground)
                .shadow(vec![
                    gpui::BoxShadow {
                        color: alpha(gpui::rgb(0x57534e), 0.08).into(),
                        offset: gpui::point(px(0.0), px(2.0)),
                        blur_radius: px(6.0),
                        spread_radius: px(0.0),
                    },
                    gpui::BoxShadow {
                        color: alpha(gpui::rgb(0x57534e), 0.22).into(),
                        offset: gpui::point(px(0.0), px(10.0)),
                        blur_radius: px(18.0),
                        spread_radius: px(-10.0),
                    },
                ])
                .cursor_pointer()
                .hover(move |style| style.bg(theme.background))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    let count = this
                        .templates_tab
                        .as_ref()
                        .and_then(|s| s.form.as_ref())
                        .map_or(0, |form| form.sections.len());
                    this.insert_section_at(&id, count, window, cx);
                }))
                .child(icon("plus", px(16.0), theme.foreground))
                .child("Add Section"),
        )
    }

    fn section_form_mut(&mut self, key: u64) -> Option<&mut SectionForm> {
        self.templates_tab
            .as_mut()
            .and_then(|s| s.form.as_mut())
            .and_then(|form| form.sections.iter_mut().find(|section| section.key == key))
    }

    /// `insertSectionAt` / `addSection`
    fn insert_section_at(
        &mut self,
        id: &str,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let blank = Section {
            title: String::new(),
            description: String::new(),
        };
        let mut next_key = self
            .templates_tab
            .as_ref()
            .and_then(|s| s.form.as_ref())
            .map_or(0, |form| form.next_key);
        let section_form = self.new_section_form(&blank, &mut next_key, window, cx);
        if let Some(form) = self.templates_tab.as_mut().and_then(|s| s.form.as_mut()) {
            let index = index.min(form.sections.len());
            form.sections.insert(index, section_form);
            form.next_key = next_key;
        }
        self.update_template(
            id,
            |template| {
                let index = index.min(template.sections.len());
                template.sections.insert(index, blank);
            },
            cx,
        );
    }

    /// `moveSection`
    fn move_section(&mut self, id: &str, index: usize, direction: isize, cx: &mut Context<Self>) {
        let target = index as isize + direction;
        if target < 0 {
            return;
        }
        let target = target as usize;
        if let Some(form) = self.templates_tab.as_mut().and_then(|s| s.form.as_mut()) {
            if target >= form.sections.len() {
                return;
            }
            let section = form.sections.remove(index);
            form.sections.insert(target, section);
        }
        self.update_template(
            id,
            |template| {
                if target < template.sections.len() && index < template.sections.len() {
                    let section = template.sections.remove(index);
                    template.sections.insert(target, section);
                }
            },
            cx,
        );
    }

    /// `deleteSection`
    fn delete_section(&mut self, id: &str, index: usize, cx: &mut Context<Self>) {
        if let Some(form) = self.templates_tab.as_mut().and_then(|s| s.form.as_mut())
            && index < form.sections.len()
        {
            form.sections.remove(index);
        }
        self.update_template(
            id,
            |template| {
                if index < template.sections.len() {
                    template.sections.remove(index);
                }
            },
            cx,
        );
    }

    /// `AutoTemplateDetails` → `AutoFormatForm`
    fn render_auto_details(&self, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let Some(auto) = self.templates_tab.as_ref().and_then(|s| s.auto.as_ref()) else {
            return div().into_any_element();
        };
        let current = auto.editor.read(cx).text().to_string();
        let dirty = crate::templates::normalize_format(&current)
            != crate::templates::normalize_format(&auto.baseline);
        let actions_open = auto.actions_open;

        let header = div()
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
                    .items_center()
                    .gap_2()
                    .child(icon("sparkle", px(16.0), gpui::rgb(0x8e51ff)))
                    .child(
                        div()
                            .truncate()
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child("Auto"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1()
                    .child(self.default_toggle(AUTO_TEMPLATE_ID, cx))
                    .child(
                        div()
                            .relative()
                            .child(self.ghost_icon_button(
                                "auto-actions".into(),
                                "more-horizontal",
                                if actions_open { theme.foreground } else { theme.muted_foreground },
                                actions_open,
                                |this, _, cx| {
                                    if let Some(auto) = this.templates_tab.as_mut().and_then(|s| s.auto.as_mut()) {
                                        auto.actions_open = !auto.actions_open;
                                        cx.notify();
                                    }
                                },
                                cx,
                            ))
                            .when(actions_open, |anchor| {
                                let spec = MenuSpec {
                                    id: "auto-actions-menu",
                                    width: 224.0,
                                    entries: vec![Entry::Item {
                                        icon: None,
                                        dim_icon: false,
                                        label: "Reset to default format".into(),
                                        trailing: Trailing::None,
                                        destructive: false,
                                        on_select: Some(Box::new(|this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                                            // `resetToDefault` is Pro-gated.
                                            this.notify_pro_required(cx);
                                        }) as Select),
                                        submenu: None,
                                    }],
                                    open_sub: None,
                                    on_hover_sub: |_, _, _| {},
                                    on_close: |this, cx| {
                                        if let Some(auto) = this.templates_tab.as_mut().and_then(|s| s.auto.as_mut()) {
                                            auto.actions_open = false;
                                            cx.notify();
                                        }
                                    },
                                };
                                anchor.child(
                                    div()
                                        .absolute()
                                        .top(px(36.0))
                                        .right_0()
                                        .child(self.render_menu_inline(spec, Align::End, cx)),
                                )
                            }),
                    ),
            );

        let mono = crate::theme::mono_font_family(cx.text_system());
        let body = div()
            .id("auto-body")
            .min_h_0()
            .flex_1()
            .overflow_y_scroll()
            .px_6()
            .pt_3()
            .pb_6()
            .child(
                div()
                    .flex()
                    .max_w(px(896.0))
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
                                            .tw_text_lg()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground)
                                            .child("Summary format"),
                                    )
                                    .child(
                                        div()
                                            .mt_1()
                                            .tw_text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child("Choose how Auto structures and styles your summaries."),
                                    ),
                            )
                            .child(self.outline_button(
                                "auto-improve",
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(icon("magic-wand", px(16.0), theme.foreground))
                                    .child("Improve with examples")
                                    .into_any_element(),
                                false,
                                Some(Box::new(|this, _, cx| this.notify_pro_required(cx))),
                                cx,
                            )),
                    )
                    .child(
                        // `PlanGate plan="pro" allowed={false}`: `opacity-60`, presses notify.
                        div()
                            .id("auto-plan-gate")
                            .relative()
                            .flex()
                            .flex_col()
                            .gap_5()
                            .opacity(0.6)
                            .cursor_not_allowed()
                            .child(
                                div()
                                    .rounded_2xl()
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.card)
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .min_h(px(448.0))
                                            .px_4()
                                            .py_3()
                                            .tw_text_sm()
                                            .line_height(px(20.0))
                                            .when_some(mono, |editor, family| editor.font_family(family))
                                            .child(auto.editor.clone()),
                                    ),
                            )
                            .child(
                                div().flex().items_center().justify_end().gap_2().child(
                                    div()
                                        .flex()
                                        .h(px(36.0))
                                        .items_center()
                                        .px_4()
                                        .rounded(px(8.0))
                                        .bg(theme.primary)
                                        .tw_text_sm()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(theme.primary_foreground)
                                        .when(!dirty, |button| button.opacity(0.5))
                                        .child("Save"),
                                ),
                            )
                            .child(
                                // The capture-phase blocker over the gated content.
                                div()
                                    .id("auto-plan-gate-blocker")
                                    .absolute()
                                    .inset_0()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _: &gpui::MouseDownEvent, _, cx| {
                                            cx.stop_propagation();
                                            this.notify_pro_required(cx);
                                        }),
                                    ),
                            ),
                    ),
            );

        div()
            .flex()
            .h_full()
            .min_h_0()
            .flex_col()
            .child(header)
            .child(body)
            .into_any_element()
    }
}

fn id_is_auto_and_default(is_default: bool, next: &serde_json::Value) -> bool {
    is_default && next.as_str() == Some("")
}
