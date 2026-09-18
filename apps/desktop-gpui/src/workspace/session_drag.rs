//! `chat/context/session-drag.ts`: a timeline note dragged as
//! `application/x-anarlog-session-context` — the chat panel takes it as a
//! manual context ref, the note editors as an inline mention.

use gpui::{Context, IntoElement, Render, SharedString, Window, div, prelude::*, px};

use crate::theme::Theme;

/// `writeSessionContextDragData`'s payload: the note and its title (or
/// `Untitled`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionDrag {
    pub session_id: String,
    pub title: String,
}

impl SessionDrag {
    pub fn new(session_id: &str, title: &str) -> Self {
        let title = title.trim();
        Self {
            session_id: session_id.to_string(),
            title: if title.is_empty() {
                "Untitled".to_string()
            } else {
                title.to_string()
            },
        }
    }

    /// `readSessionMentionDragData`: the `mention-@` node the editors insert.
    pub fn mention_item(&self) -> crate::editor::mention_picker::MentionItem {
        crate::editor::mention_picker::MentionItem {
            id: self.session_id.clone(),
            kind: "session".into(),
            label: self.title.clone(),
        }
    }
}

/// The drag image: the browser ghosts the row itself (title over its time,
/// no background), so the shell draws the same two lines translucently.
pub(crate) struct SessionDragPreview {
    pub title: SharedString,
    pub time: SharedString,
    pub mono_font_family: Option<SharedString>,
    pub theme: Theme,
}

impl Render for SessionDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .px_3()
            .py_2()
            .max_w(px(240.0))
            .opacity(0.7)
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .text_color(self.theme.foreground)
                    .child(self.title.clone()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(self.theme.muted_foreground)
                    .when_some(self.mono_font_family.clone(), |time, family| {
                        time.font_family(family)
                    })
                    .child(self.time.clone()),
            )
    }
}
