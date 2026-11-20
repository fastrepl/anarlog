//! Shipping's empty memo (`RawEditor`'s `TemplateEmptyState`): favorite and suggested
//! templates over a pristine memo, and "New template".

use std::sync::LazyLock;

use desktop_runtime::SessionId;
use gpui::{AnyElement, Context, Entity, SharedString, div, prelude::*, px};
use regex::Regex;
use serde_json::json;

use super::{NoteEvent, NoteView, icon};
use crate::ui::a11y::Accessible;
use crate::{
    editor::EditorPane,
    meeting::model::failure,
    ui::theme::Theme,
    workspace::templates::{Library, Template},
};

const DEFAULT_SUGGESTIONS: [&str; 3] = [
    "default-project-kickoff",
    "default-daily-standup",
    "default-one-on-one-meeting",
];

/// Shipping's `contextualTemplateRules`, matched against the event's title and description.
static CONTEXTUAL: LazyLock<[(&str, Regex); 4]> = LazyLock::new(|| {
    let rule = |pattern: &str| Regex::new(&format!("(?i){pattern}")).expect("template rule");
    [
        (
            "default-sales-discovery-call",
            rule(
                r"\b(sales|discovery|demo|prospect|lead|client|customer|pipeline|qualification|qualifying|pitch)\b",
            ),
        ),
        (
            "default-project-kickoff",
            rule(r"\b(kickoff|kick-off|project launch)\b"),
        ),
        (
            "default-daily-standup",
            rule(r"\b(standup|stand-up|daily sync|scrum)\b"),
        ),
        (
            "default-one-on-one-meeting",
            rule(r"\b(1:1|one[- ]on[- ]one)\b"),
        ),
    ]
});

fn has_sections(template: &Template) -> bool {
    template
        .sections
        .iter()
        .any(|section| !section.title.trim().is_empty())
}

/// Shipping's `getFavoriteTemplates` and `getSuggestedTemplates`.
pub(super) fn memo_templates(
    library: &Library,
    event_text: &str,
    participants: usize,
) -> (Vec<Template>, Vec<Template>) {
    let mut favorites: Vec<Template> = library
        .templates
        .iter()
        .filter(|template| template.pinned && has_sections(template))
        .cloned()
        .collect();
    favorites.sort_by(|a, b| {
        a.pin_order
            .unwrap_or(i64::MAX)
            .cmp(&b.pin_order.unwrap_or(i64::MAX))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    let available: Vec<&Template> = library
        .templates
        .iter()
        .filter(|template| {
            has_sections(template) && !favorites.iter().any(|favorite| favorite.id == template.id)
        })
        .collect();
    let mut preferred: Vec<&str> = CONTEXTUAL
        .iter()
        .filter(|(_, rule)| rule.is_match(event_text))
        .map(|(id, _)| *id)
        .collect();
    if participants == 2 {
        preferred.push("default-one-on-one-meeting");
    }
    preferred.extend(DEFAULT_SUGGESTIONS);
    let mut suggested: Vec<Template> = Vec::new();
    for id in preferred {
        if let Some(template) = available.iter().find(|template| template.id.as_ref() == id)
            && !suggested.iter().any(|chosen| chosen.id == template.id)
        {
            suggested.push((*template).clone());
        }
    }
    let mut fallback: Vec<&Template> = available
        .into_iter()
        .filter(|template| !suggested.iter().any(|chosen| chosen.id == template.id))
        .collect();
    fallback.sort_by_key(|template| template.title.to_lowercase());
    suggested.extend(fallback.into_iter().cloned());
    suggested.truncate(DEFAULT_SUGGESTIONS.len());
    (favorites, suggested)
}

impl NoteView {
    /// The memo editor when it is the one this note shows.
    pub(super) fn memo_editor(&self) -> Option<Entity<EditorPane>> {
        self.content.clone()?.downcast::<EditorPane>().ok()
    }

    /// Shipping's `TemplateEmptyState`, over a pristine memo of a note without a transcript.
    pub(super) fn memo_templates_overlay(
        &self,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let editor = self.memo_editor()?;
        if !editor.read(cx).is_pristine()
            || self.brief_generating()
            || self.tabs().contains(&super::NoteTab::Transcript)
        {
            return None;
        }
        let (favorites, suggested) = memo_templates(
            &self.templates,
            &self.details.event_text,
            self.details.participants,
        );
        let label = |text: &'static str| {
            div()
                .h_8()
                .flex()
                .items_center()
                .text_xs()
                .line_height(px(16.))
                .text_color(colors.muted_foreground)
                .child(text)
        };
        let item = |id: SharedString, glyph: AnyElement, title: SharedString| {
            let selector = format!("{id}-title");
            div()
                .id(id)
                .a11y_button(title.clone())
                .ml(px(-8.))
                .h_8()
                .max_w_full()
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .text_color(colors.muted_foreground)
                .cursor_pointer()
                .hover(move |style| style.bg(colors.accent).text_color(colors.foreground))
                .child(glyph)
                .child(
                    div()
                        .debug_selector(|| selector)
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .line_height(px(20.))
                        .font_weight(crate::ui::theme::MEDIUM)
                        .child(title),
                )
        };
        let section = |label_text: &'static str,
                       templates: Vec<Template>,
                       cx: &mut Context<Self>| {
            (!templates.is_empty()).then(|| {
                // Full width, like shipping's fragment: a fit-content section would size itself to
                // its rows' margin boxes, and each row's `max_w_full` would then cut 8px (its
                // negative margin) off the widest title.
                div()
                    .id(label_text)
                    .a11y_group(gpui::Role::Group, label_text)
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_start()
                    .child(label(label_text))
                    .children(templates.into_iter().map(|template| {
                        let title: SharedString = if template.title.is_empty() {
                            "Untitled".into()
                        } else {
                            template.title.to_string().into()
                        };
                        let id = template.id.clone();
                        item(
                            format!("memo-template-{id}").into(),
                            template.icon.render(16.),
                            title,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(template) = this
                                .templates
                                .templates
                                .iter()
                                .find(|candidate| candidate.id == id)
                            {
                                let template = template.clone();
                                this.apply_memo_template(&template, cx);
                            }
                        }))
                    }))
            })
        };
        Some(
            div()
                .absolute()
                // `inset-x-0 top-8` inside the memo's `px-3 pt-2` scroll box.
                .top(px(40.))
                .left(px(12.))
                .right(px(12.))
                .flex()
                .flex_col()
                .items_start()
                .children(self.brief_suggestion(colors, cx))
                .children(section("Start with a favorite template", favorites, cx))
                .children(section("Suggested templates", suggested, cx))
                .child(
                    item(
                        "memo-new-template".into(),
                        icon("editor/PlusIcon.svg", 16., colors.muted_foreground)
                            .into_any_element(),
                        "New template".into(),
                    )
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(NoteEvent::OpenTemplates))),
                )
                .into_any_element(),
        )
    }

    fn apply_memo_template(&mut self, template: &Template, cx: &mut Context<Self>) {
        let Some(editor) = self.memo_editor() else {
            return;
        };
        let Some(session) = self
            .current
            .as_ref()
            .map(|session| session.summary.id.clone())
        else {
            return;
        };
        let titles: Vec<&str> = template
            .sections
            .iter()
            .map(|section| section.title.as_ref())
            .collect();
        editor.update(cx, |editor, cx| editor.apply_template(&titles, cx));
        self.memo_template = Some((session, template.id.clone()));
    }

    /// Shipping saves the applied template's id with the memo (`raw_template_id`), which later
    /// summaries use; written once the memo exists.
    pub(super) fn memo_saved(&mut self, session: &SessionId, cx: &mut Context<Self>) {
        let Some((pending, template)) = self.memo_template.take() else {
            return;
        };
        if &pending != session {
            return;
        }
        let session = session.clone();
        let reply = self.runtime.submit(move |services| async move {
            services
                .executor
                .execute(
                    // updated_at stays: the open editor's next save compares against it.
                    "UPDATE session_documents SET template_id = ? WHERE session_id = ? AND kind = 'note' AND deleted_at IS NULL"
                        .into(),
                    vec![json!(template), json!(session)],
                )
                .await
                .map(|_| ())
                .map_err(failure)
        });
        cx.spawn(async move |this, cx| {
            if let Err(error) = async { reply?.receive().await }.await {
                let _ = this.update(cx, |this, cx| {
                    this.message = format!("Template not recorded on the memo: {error}");
                    cx.notify();
                });
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{resource_list::Glyph, templates::Section};

    fn template(id: &str, title: &str, pinned: bool, sections: &[&str]) -> Template {
        Template {
            id: id.into(),
            title: title.into(),
            description: "".into(),
            pinned,
            pin_order: None,
            category: "".into(),
            icon: Glyph::template_default(),
            sections: sections
                .iter()
                .map(|title| Section {
                    title: (*title).into(),
                    description: "".into(),
                })
                .collect(),
            targets: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn ids(templates: &[Template]) -> Vec<&str> {
        templates
            .iter()
            .map(|template| template.id.as_ref())
            .collect()
    }

    /// Template rows once came out 8px short of their titles, cutting "Project Kickoff" to
    /// "Project Kickof" without an ellipsis at every window width. Titles show in full when
    /// they fit and stay inside the pane when they don't.
    #[gpui::test]
    fn template_rows_show_titles_that_fit_in_full(cx: &mut gpui::TestAppContext) {
        use crate::contracts::{EditorInit, LaneContext};

        cx.executor().allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = desktop_runtime::RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        let open =
            futures::executor::block_on(runtime.create_note("Intro".into()).unwrap().receive())
                .unwrap();
        cx.update(crate::ui::theme::UiFonts::init);
        let (note, cx) =
            cx.add_window_view(|window, cx| NoteView::new(runtime.clone(), window, cx));
        let session = open.summary.id.clone();
        note.update(cx, |note, cx| note.open(session.clone(), cx));
        let settle = |cx: &mut gpui::VisualTestContext, selector: &'static str| {
            for _ in 0..600 {
                cx.update(|window, cx| window.draw(cx).clear(cx));
                cx.run_until_parked();
                if let Some(bounds) = cx.debug_bounds(selector) {
                    return bounds;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            panic!("{selector} never rendered");
        };
        while note.read_with(cx, |note, _| note.current.is_none()) {
            cx.run_until_parked();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let init = EditorInit {
            session_id: session.clone(),
            document: open.note.clone().unwrap(),
            return_focus: cx.update(|_, cx| cx.focus_handle()),
        };
        let memo = cx.update(|window, cx| {
            cx.new(|cx| {
                EditorPane::new(
                    LaneContext {
                        runtime: runtime.clone(),
                    },
                    init,
                    window,
                    cx,
                )
            })
        });
        note.update(cx, |note, cx| note.set_content(&session, memo.into(), cx));
        let shaped = |title: &str, cx: &mut gpui::VisualTestContext| {
            cx.update(|window, cx| {
                let run = gpui::TextRun {
                    len: title.len(),
                    font: gpui::Font {
                        weight: crate::ui::theme::MEDIUM,
                        ..gpui::font(crate::ui::theme::system_font(cx))
                    },
                    color: gpui::black(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                window
                    .text_system()
                    .shape_line(title.to_owned().into(), px(14.), &[run], None)
                    .width
            })
        };
        // Wide, too narrow for the title, then wide again after the resize.
        for width in [900., 140., 700.] {
            cx.simulate_resize(gpui::size(px(width), px(600.)));
            for (selector, title) in [
                (
                    "memo-template-default-project-kickoff-title",
                    "Project Kickoff",
                ),
                ("memo-new-template-title", "New template"),
            ] {
                let bounds = settle(cx, selector);
                let text = shaped(title, cx);
                if width < 200. {
                    assert!(bounds.right() < px(width), "{title:?} overflows {bounds:?}");
                } else {
                    assert!(
                        bounds.size.width >= text,
                        "at {width}px {title:?} got {:?} of {text:?}",
                        bounds.size.width
                    );
                }
            }
        }
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }

    #[test]
    fn suggestions_follow_the_event_then_shipping_defaults() {
        let library = Library {
            templates: vec![
                template(
                    "default-project-kickoff",
                    "Project Kickoff",
                    false,
                    &["Goals"],
                ),
                template("default-daily-standup", "Daily Standup", false, &["Today"]),
                template(
                    "default-one-on-one-meeting",
                    "1:1 Meeting",
                    false,
                    &["Updates"],
                ),
                template(
                    "default-sales-discovery-call",
                    "Sales Discovery Call",
                    false,
                    &["Pain"],
                ),
                template("favorite", "Weekly sync", true, &["Highlights"]),
                template("empty", "Empty", true, &[" "]),
                template("a-other", "Another", false, &["Notes"]),
            ],
            ..Library::default()
        };
        let (favorites, suggested) = memo_templates(&library, "", 0);
        assert_eq!(ids(&favorites), ["favorite"]);
        assert_eq!(
            ids(&suggested),
            [
                "default-project-kickoff",
                "default-daily-standup",
                "default-one-on-one-meeting"
            ]
        );
        let (_, suggested) = memo_templates(&library, "Customer demo", 2);
        assert_eq!(
            ids(&suggested),
            [
                "default-sales-discovery-call",
                "default-one-on-one-meeting",
                "default-project-kickoff"
            ]
        );
        let (_, suggested) = memo_templates(&library, "our 1:1", 0);
        assert_eq!(suggested[0].id.as_ref(), "default-one-on-one-meeting");
    }
}
