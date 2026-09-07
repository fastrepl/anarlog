//! `TemplatePickerPopover` (`note-input/template-picker.tsx`): the popover
//! under the active enhanced tab that swaps the summary's template.

use gpui::{
    AnyElement, ClickEvent, Context, Entity, Focusable as _, MouseButton, MouseDownEvent,
    SharedString, Window, div, prelude::*, px,
};

use super::Workspace;
use super::toast::FlashVariant;
use crate::db::TemplateIcon;
use crate::templates::UserTemplate;
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::ui::{TailwindText as _, icon};

pub(crate) struct TemplatePicker {
    /// The enhanced document whose template the picker replaces.
    pub(crate) note_id: String,
    search: Entity<TextInput>,
    templates: Vec<UserTemplate>,
    scroll: gpui::ScrollHandle,
}

impl Workspace {
    pub(crate) fn template_picker_open(&self) -> bool {
        self.template_picker.is_some()
    }

    /// Closing hands focus back to the shell so the global shortcuts keep
    /// dispatching (Radix returns focus to the trigger).
    pub(crate) fn close_template_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.template_picker.take().is_some() {
            self.focus_handle.focus(window);
            cx.notify();
        }
    }

    /// Opens the picker for the active enhanced note, with the search focused
    /// (`autoFocus`).
    pub(crate) fn toggle_template_picker(
        &mut self,
        note_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .template_picker
            .as_ref()
            .is_some_and(|picker| picker.note_id == note_id)
        {
            self.close_template_picker(window, cx);
            return;
        }
        let theme = self.theme;
        let search = cx.new(|cx| {
            TextInput::new(
                "Search templates...",
                TextInputStyle {
                    text: theme.foreground,
                    placeholder: theme.muted_foreground,
                    selection: theme.selection,
                    underline_when_focused: false,
                    masked: false,
                },
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &search,
            window,
            |this, _, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Changed => cx.notify(),
                TextInputEvent::Escape => this.close_template_picker(window, cx),
                _ => {}
            },
        )
        .detach();
        search.read(cx).focus_handle(cx).focus(window);
        self.template_picker = Some(TemplatePicker {
            note_id,
            search,
            templates: Vec::new(),
            scroll: gpui::ScrollHandle::new(),
        });
        cx.notify();
        let task = self.store.list_user_templates();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(templates)) = task.await else {
                return;
            };
            this.update(cx, |this, cx| {
                if let Some(picker) = this.template_picker.as_mut() {
                    picker.templates = templates;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// `handleSelectTemplate` → `service.enhance(sessionId, { templateId,
    /// targetNoteId })`: `enhance` returns `no_model` and does nothing while
    /// no language model is configured; with one, Tauri would regenerate the
    /// summary under the new template, which the shell cannot run yet.
    fn choose_template(
        &mut self,
        template_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_template_picker(window, cx);
        if !self.provider_settings.has_llm() {
            return;
        }
        let _ = template_id;
        self.flash(
            FlashVariant::Warning,
            "Summary generation is not available in this build yet",
            cx,
        );
    }

    /// `handleCreateTemplate`: create, then open the Templates tab on it.
    fn create_template_from_picker(
        &mut self,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_template_picker(window, cx);
        let title = if title.trim().is_empty() {
            "New Template".to_string()
        } else {
            title.trim().to_string()
        };
        let task = self.store.create_user_template(crate::templates::Draft {
            title,
            ..crate::templates::Draft::default()
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| match result {
                Ok(id) => {
                    this.reload_settings(cx);
                    this.open_templates(Some(id), window, cx);
                }
                Err(error) => this.flash(FlashVariant::Error, error.to_string(), cx),
            })
            .ok();
        })
        .detach();
    }

    /// The `w-80 pb-0` popover: the app panel with the search row and the
    /// result sections, then the See all templates footer.
    pub(super) fn render_template_picker(
        &self,
        used_template_id: &str,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let picker = self.template_picker.as_ref()?;
        let theme = self.theme;
        let query = picker.search.read(cx).text().trim().to_string();
        let lower = query.to_lowercase();
        let matches = |template: &UserTemplate| {
            lower.is_empty()
                || template.title.to_lowercase().contains(&lower)
                || template.description.to_lowercase().contains(&lower)
                || template
                    .category
                    .as_deref()
                    .is_some_and(|category| category.to_lowercase().contains(&lower))
                || template.targets.as_ref().is_some_and(|targets| {
                    targets.iter().any(|t| t.to_lowercase().contains(&lower))
                })
        };
        // `sortFavoriteTemplates` / `sortOtherTemplates`
        let mut favorites: Vec<&UserTemplate> = picker
            .templates
            .iter()
            .filter(|t| t.pinned && matches(t))
            .collect();
        favorites.sort_by(|a, b| {
            a.pin_order
                .unwrap_or(i64::MAX)
                .cmp(&b.pin_order.unwrap_or(i64::MAX))
                .then_with(|| a.title.cmp(&b.title))
        });
        let mut others: Vec<&UserTemplate> = picker
            .templates
            .iter()
            .filter(|t| !t.pinned && matches(t))
            .collect();
        others.sort_by(|a, b| a.title.cmp(&b.title));

        let search_input = picker.search.clone();
        let focus_input = search_input.clone();
        let searching = !query.is_empty();

        let result_row = |id: SharedString,
                          glyph: AnyElement,
                          title: String,
                          favorite: bool,
                          used: bool,
                          on_click: super::menu::Select| {
            let on_click = std::rc::Rc::new(on_click);
            div()
                .id(id)
                .flex()
                .h(px(32.0))
                .w_full()
                .items_center()
                .gap(px(6.0))
                .rounded_md()
                .px(px(10.0))
                .cursor_pointer()
                .hover(move |style| style.bg(theme.accent))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, window, cx| on_click(this, window, cx)),
                )
                .child(glyph)
                .child(
                    // The `min-w-0 flex-1` button: the title truncates and the
                    // heart follows it directly.
                    div()
                        .min_w_0()
                        .flex_1()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .min_w_0()
                                .flex_shrink()
                                .truncate()
                                .tw_text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.foreground)
                                .child(SharedString::from(title)),
                        )
                        .when(favorite, |row| {
                            row.child(icon("heart", px(14.0), gpui::rgb(0xff2056)))
                        }),
                )
                .when(used, |row| {
                    // `Regenerate`: `text-[11px] font-medium px-1.5 py-0.5 gap-1`.
                    row.child(
                        div()
                            .id("template-regenerate")
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_1()
                            .rounded_md()
                            .px(px(6.0))
                            .py(px(2.0))
                            .text_size(px(11.0))
                            .line_height(px(15.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .cursor_pointer()
                            .hover(move |style| style.text_color(theme.foreground))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                this.choose_template(None, window, cx);
                            }))
                            .child(icon("arrow-clockwise", px(12.0), theme.muted_foreground))
                            .child("Regenerate"),
                    )
                })
        };

        let glyph_for = |icon_value: &TemplateIcon| self.template_icon_glyph(icon_value, px(16.0));

        let mut results = div().flex().flex_col();
        // Auto
        let auto_used = used_template_id.is_empty();
        results = results.child(result_row(
            "template-pick-auto".into(),
            icon("tpl-sparkles", px(16.0), gpui::rgb(0x9ca3af)).into_any_element(),
            "Auto".to_string(),
            false,
            auto_used,
            Box::new(|this, window, cx| this.choose_template(None, window, cx)),
        ));
        if searching {
            // `Create new template` section with its header.
            let title = query.clone();
            results = results.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .child(icon("plus", px(14.0), gpui::rgb(0x2b7fff)))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .line_height(px(15.0))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.muted_foreground)
                                    .when_some(self.mono_font_family.clone(), |t, family| {
                                        t.font_family(family)
                                    })
                                    .child("Create new template"),
                            ),
                    )
                    .child(result_row(
                        "template-pick-create".into(),
                        glyph_for(&TemplateIcon::default_template()),
                        query.clone(),
                        false,
                        false,
                        Box::new(move |this, window, cx| {
                            this.create_template_from_picker(title.clone(), window, cx)
                        }),
                    )),
            );
        }
        let has_templates = !favorites.is_empty() || !others.is_empty();
        if has_templates {
            let mut list = div().flex().flex_col();
            for template in favorites.iter().chain(others.iter()) {
                let id = template.id.clone();
                let title = if template.title.trim().is_empty() {
                    "Untitled".to_string()
                } else {
                    template.title.clone()
                };
                let used = used_template_id == template.id;
                list = list.child(result_row(
                    SharedString::from(format!("template-pick-{}", template.id)),
                    glyph_for(&template.icon),
                    title,
                    template.pinned,
                    used,
                    Box::new(move |this, window, cx| {
                        this.choose_template(Some(id.clone()), window, cx)
                    }),
                ));
            }
            results = results.child(list);
        } else if !searching {
            results = results.child(
                div()
                    .px_2()
                    .py_3()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child("No templates yet"),
            );
        }

        // `PopoverContent variant="app"` (`w-80 pb-0`): the chrome wraps the
        // `AppFloatingPanel` and the footer.
        let panel = super::menu::menu_chrome(theme, "template-picker", 320.0)
            .pb_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, window, cx| {
                this.close_template_picker(window, cx);
            }))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(20.0))
                    .child(crate::squircle::squircle(
                        crate::squircle::PANEL_RADIUS,
                        Some(theme.floating_panel),
                        Some((1.0, theme.floating_border)),
                    ))
                    .child(
                        // `border-b py-1` search row: `h-8 rounded-md px-2.5 gap-2`.
                        div().border_b_1().border_color(theme.border).py_1().child(
                            div()
                                .id("template-picker-search")
                                .flex()
                                .h(px(32.0))
                                .items_center()
                                .gap_2()
                                .rounded_md()
                                .px(px(10.0))
                                .cursor_text()
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
                                        .child(picker.search.clone()),
                                )
                                .when(searching, |row| {
                                    row.child(
                                        div()
                                            .id("template-picker-clear")
                                            .rounded(px(2.0))
                                            .p(px(2.0))
                                            .cursor_pointer()
                                            .hover(move |style| style.bg(theme.accent))
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .on_click(cx.listener(
                                                move |_, _: &ClickEvent, _, cx| {
                                                    search_input.update(cx, |input, cx| {
                                                        input.set_text("", cx)
                                                    });
                                                },
                                            ))
                                            .child(icon("x", px(12.0), theme.muted_foreground)),
                                    )
                                }),
                        ),
                    )
                    .child(
                        // `scroll-fade-y max-h-80 overflow-y-auto p-1` with the
                        // 6px WebKit scrollbar inside the scroller.
                        div()
                            .relative()
                            .child(
                                div()
                                    .id("template-picker-results")
                                    .max_h(px(320.0))
                                    .overflow_y_scroll()
                                    .track_scroll(&picker.scroll)
                                    .pl_1()
                                    .pr(px(4.0) + crate::ui::scrollbar_gutter(&picker.scroll))
                                    .py_1()
                                    .child(results),
                            )
                            .child(crate::ui::webkit_scrollbar(
                                picker.scroll.clone(),
                                theme.scrollbar_thumb,
                            )),
                    ),
            )
            .child(
                // `See all templates ›` (`py-1.5 text-xs font-medium`).
                div()
                    .id("template-picker-see-all")
                    .flex()
                    .w_full()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .px_3()
                    .py(px(6.0))
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.muted_foreground)
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.accent).text_color(theme.foreground))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.close_template_picker(window, cx);
                        this.open_templates(None, window, cx);
                    }))
                    .child("See all templates")
                    .child(icon("caret-right", px(14.0), theme.muted_foreground)),
            );

        // `align="start" sideOffset={4}` under the `h-6` trigger.
        Some(
            div()
                .absolute()
                .top(px(28.0))
                .left_0()
                .child(
                    gpui::deferred(
                        gpui::anchored()
                            .anchor(gpui::Corner::TopLeft)
                            .snap_to_window_with_margin(px(8.0))
                            .child(panel),
                    )
                    .with_priority(2),
                )
                .into_any_element(),
        )
    }
}
