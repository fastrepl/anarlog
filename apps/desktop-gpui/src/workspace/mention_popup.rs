//! The `@` suggestion popup (`mention.css`: `.mention-container` /
//! `.mention-item`) and the candidate list `useMentionConfig` builds from the
//! timeline sessions, humans, and organizations.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use super::Workspace;
use crate::editor::mention_picker::{MentionItem, Search, search_candidates};
use crate::ui::{TailwindText, icon};

/// A popup row callback: the row index and the app.
type RowAction = Rc<dyn Fn(usize, &mut gpui::App)>;

/// `useMentionConfig`'s empty-query listing: titled sessions in the timeline
/// table's order, then humans with names, then organizations with names.
pub(crate) fn candidates(
    sessions: &[crate::timeline::SessionRow],
    humans: &[crate::contacts::Human],
    organizations: &[crate::contacts::Organization],
) -> Vec<MentionItem> {
    let mut items = Vec::new();
    for session in sessions {
        if !session.title.is_empty() {
            items.push(MentionItem {
                id: session.id.clone(),
                kind: "session".into(),
                label: session.title.clone(),
            });
        }
    }
    for human in humans {
        if !human.name.is_empty() {
            items.push(MentionItem {
                id: human.id.clone(),
                kind: "human".into(),
                label: human.name.clone(),
            });
        }
    }
    for organization in organizations {
        if !organization.name.is_empty() {
            items.push(MentionItem {
                id: organization.id.clone(),
                kind: "organization".into(),
                label: organization.name.clone(),
            });
        }
    }
    items
}

/// `handleSearch`: an empty query lists the candidates; a query asks the
/// shared tantivy index like `useSearchEngine().search(query)` and maps the
/// hits (`id`, `doc_type`, `title`) — falling back to the label filter while
/// the index is still opening.
pub(crate) fn search_over(
    shared: Rc<RefCell<Vec<MentionItem>>>,
    index: std::sync::Arc<crate::search::SearchIndex>,
    runtime: tokio::runtime::Handle,
) -> Search {
    Rc::new(move |query: &str| {
        if query.trim().is_empty() || !index.is_ready() {
            return search_candidates(&shared.borrow(), query);
        }
        match index.search(&runtime, query) {
            Ok(hits) => hits
                .into_iter()
                .map(|hit| MentionItem {
                    id: hit.document.id,
                    kind: hit.document.doc_type,
                    label: hit.document.title,
                })
                .take(crate::editor::mention_picker::MAX_ITEMS)
                .collect(),
            Err(error) => {
                tracing::error!(%error, "mention search failed");
                Vec::new()
            }
        }
    })
}

impl Workspace {
    /// Refresh the shared candidate list from the rows the workspace holds
    /// and let an open popup re-query it.
    pub(crate) fn refresh_mention_candidates(&mut self, cx: &mut Context<Self>) {
        let items = candidates(
            &self.session_rows,
            &self.mention_humans,
            &self.mention_organizations,
        );
        *self.mention_candidates.borrow_mut() = items;
        if let Some(editor) = &self.editor {
            editor.update(cx, |editor, cx| editor.rerun_mention_search(cx));
        }
        if let Some(composer) = &self.chat.composer {
            composer.update(cx, |composer, cx| composer.rerun_mention_search(cx));
        }
    }

    /// Loads humans and organizations for the picker (the contacts tab keeps
    /// its own copy while open).
    pub(crate) fn load_mention_contacts(&mut self, cx: &mut Context<Self>) {
        let task = self.store.list_contacts();
        cx.spawn(async move |this, cx| {
            if let Ok(Ok((humans, organizations))) = task.await {
                this.update(cx, |this, cx| {
                    this.mention_humans = humans;
                    this.mention_organizations = organizations;
                    this.refresh_mention_candidates(cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// The note editor's `@` popup.
    pub(crate) fn render_mention_popup(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let editor = self.editor.as_ref()?;
        let (items, selected, anchor, line_height) = {
            let editor = editor.read(cx);
            let state = editor.mention()?;
            let (anchor, line_height) = editor.mention_anchor()?;
            (state.items.clone(), state.selected, anchor, line_height)
        };
        let hover_editor = editor.clone();
        let click_editor = editor.clone();
        Some(self.render_mention_popup_at(
            "mention-popup",
            items,
            selected,
            anchor,
            line_height,
            Rc::new(move |index, cx| {
                hover_editor.update(cx, |editor, cx| editor.select_mention(index, cx));
            }),
            Rc::new(move |index, cx| {
                click_editor.update(cx, |editor, cx| editor.insert_mention(index, cx));
            }),
            window,
        ))
    }

    /// The chat composer's `@` popup (`ChatEditor` wires the same
    /// `MentionSuggestion`).
    pub(crate) fn render_composer_mention_popup(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let composer = self.chat.composer.as_ref()?;
        let (items, selected, anchor, line_height) = {
            let composer = composer.read(cx);
            let state = composer.mention()?;
            let (anchor, line_height) = composer.mention_anchor()?;
            (state.items.clone(), state.selected, anchor, line_height)
        };
        let hover_composer = composer.clone();
        let click_composer = composer.clone();
        Some(self.render_mention_popup_at(
            "composer-mention-popup",
            items,
            selected,
            anchor,
            line_height,
            Rc::new(move |index, cx| {
                hover_composer.update(cx, |composer, cx| composer.select_mention(index, cx));
            }),
            Rc::new(move |index, cx| {
                click_composer.update(cx, |composer, cx| composer.insert_mention(index, cx));
            }),
            window,
        ))
    }

    /// `MentionSuggestion`: `bottom-start` of the trigger with a 4px offset,
    /// flipped above when the window bottom is near.
    #[allow(clippy::too_many_arguments)]
    fn render_mention_popup_at(
        &self,
        id: &'static str,
        items: Vec<MentionItem>,
        selected: usize,
        anchor: gpui::Point<gpui::Pixels>,
        line_height: gpui::Pixels,
        on_hover: RowAction,
        on_click: RowAction,
        window: &Window,
    ) -> AnyElement {
        let theme = self.theme;
        let viewport = window.viewport_size();
        let row = 32.0;
        let height = items.len() as f32 * row + 8.0;
        let below = anchor.y + line_height + px(4.0);
        let top = if below + px(height) > viewport.height {
            anchor.y - px(4.0) - px(height)
        } else {
            below
        };
        let left = anchor.x.min(viewport.width - px(280.0)).max(px(0.0));
        gpui::deferred(
            div()
                .id(id)
                .absolute()
                .left(left)
                .top(top)
                .max_w(px(280.0))
                .p_1()
                .rounded(px(16.0))
                .bg(theme.popover)
                .text_color(theme.foreground)
                .border_1()
                .border_color(theme.border)
                .shadow(vec![
                    gpui::BoxShadow {
                        color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                        offset: gpui::point(px(0.0), px(10.0)),
                        blur_radius: px(15.0),
                        spread_radius: px(-3.0),
                    },
                    gpui::BoxShadow {
                        color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                        offset: gpui::point(px(0.0), px(4.0)),
                        blur_radius: px(6.0),
                        spread_radius: px(-4.0),
                    },
                ])
                .overflow_hidden()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .children(items.into_iter().enumerate().map(|(index, item)| {
                    let on_hover = on_hover.clone();
                    let on_click = on_click.clone();
                    let glyph = match item.kind.as_str() {
                        "session" => Some("note"),
                        "human" => Some("user"),
                        "organization" => Some("buildings"),
                        _ => None,
                    };
                    div()
                        .id(("mention-item", index))
                        .flex()
                        .items_center()
                        .h(px(row))
                        .px_2()
                        .rounded(px(12.0))
                        .w_full()
                        .gap(px(6.0))
                        .tw_text_sm()
                        .cursor_pointer()
                        .when(index == selected, |item| item.bg(theme.muted))
                        .hover(move |style| style.bg(theme.muted))
                        .on_hover(move |hovered, _, cx| {
                            if *hovered {
                                on_hover(index, cx);
                            }
                        })
                        .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| {
                            cx.stop_propagation()
                        })
                        .on_click(move |_: &ClickEvent, _, cx| on_click(index, cx))
                        .when_some(glyph, |row, glyph| {
                            row.child(icon(glyph, px(16.0), theme.muted_foreground))
                        })
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .truncate()
                                .tw_text_sm()
                                .line_height(px(16.0))
                                .child(gpui::SharedString::from(item.label)),
                        )
                })),
        )
        .with_priority(6)
        .into_any_element()
    }
}
