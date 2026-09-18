//! `useCreatePreMeetingBrief` + `pre-meeting-brief-job.ts`: the empty memo's
//! `CreateBriefSuggestion` and the streamed brief it writes into the memo.

use std::time::Duration;

use gpui::{ClickEvent, Context, Div, MouseButton, Window, div, prelude::*, px};

use super::{Note, Workspace};
use crate::db::NotePreview;
use crate::llm_stream::{self, Chunk, Request};
use crate::pre_meeting::{self, BriefEvent, BriefParticipant};
use crate::ui::{TailwindText as _, icon};
use crate::workspace::toast::FlashVariant;

impl Workspace {
    /// `visible: available || isGenerating`, where `available` needs an
    /// inactive session, a configured model, an empty memo, and
    /// `canCreatePreMeetingBrief`.
    pub(super) fn brief_visible(&self, preview: &NotePreview, memo_empty: bool) -> bool {
        if self.brief_generating(&preview.session.id) {
            return true;
        }
        self.brief_available(preview, memo_empty)
    }

    pub(super) fn brief_generating(&self, session_id: &str) -> bool {
        self.brief_jobs.contains(session_id)
    }

    fn brief_available(&self, preview: &NotePreview, memo_empty: bool) -> bool {
        let inactive =
            self.session_mode(&preview.session.id) == super::recording::SessionMode::Inactive;
        let has_model = self.provider_settings.llm_provider.is_some()
            && self.provider_settings.llm_model.is_some();
        inactive
            && has_model
            && memo_empty
            && pre_meeting::can_create_pre_meeting_brief(
                preview.brief.event.as_ref(),
                chrono::Utc::now().timestamp_millis(),
                &preview.brief.notes,
                preview.brief.has_participants,
            )
    }

    /// `CreateBriefSuggestion`: an `h-8 -ml-2 px-2 gap-2 rounded-md mb-6`
    /// row in muted foreground with the Sparkle icon, foreground on hover.
    pub(super) fn render_brief_suggestion(
        &self,
        session_id: &str,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let theme = self.theme;
        let session_id = session_id.to_string();
        let hovered = self.hovered == Some("brief-suggestion");
        let color = if hovered {
            theme.foreground
        } else {
            theme.muted_foreground
        };
        div()
            .id("brief-suggestion")
            .flex()
            .h(px(32.0))
            .w_auto()
            .max_w_full()
            .items_center()
            .gap_2()
            .mb_6()
            .ml(px(-8.0))
            .px_2()
            .rounded_md()
            .text_color(color)
            .cursor_pointer()
            .when(hovered, |row| row.bg(theme.accent))
            .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                this.set_hovered("brief-suggestion", *hovering, cx);
            }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.start_pre_meeting_brief(session_id.clone(), window, cx);
            }))
            .child(icon("sparkle", px(16.0), color))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .tw_text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child("Create a brief to prepare this meeting"),
            )
    }

    /// `createBrief` → `runPreMeetingBriefJob`: stream the structured brief,
    /// rendering each partial into the memo editor (or the stored memo when
    /// the editor is not mounted), then flush the final one.
    pub(super) fn start_pre_meeting_brief(
        &mut self,
        session_id: String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Note::Ready { preview, .. } = &self.note else {
            return;
        };
        if preview.session.id != session_id {
            return;
        }
        let preview: &NotePreview = preview;
        let memo_empty = self
            .editor
            .as_ref()
            .filter(|editor| editor.read(cx).session_id == session_id)
            .is_some_and(|editor| editor.read(cx).doc().is_pristine());
        if self.brief_generating(&session_id) || !self.brief_available(preview, memo_empty) {
            return;
        }
        let notes: Vec<pre_meeting::PastSessionNote> =
            pre_meeting::select_brief_source_notes(&preview.brief.notes)
                .into_iter()
                .cloned()
                .collect();
        if notes.is_empty() {
            return;
        }
        let event = preview.brief.event.clone();
        let session_title = preview.session.title.clone();
        let language = self
            .provider_settings
            .string_setting("ai_language", &["language", "ai_language"])
            .unwrap_or_else(|| "en".to_string());
        let store = self.store.clone();
        let connection = store.llm_connection(&self.provider_settings);
        let participants = store.brief_participants(session_id.clone());
        self.brief_jobs.insert(session_id.clone());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let outcome: Result<(), String> = async {
                let conn = connection
                    .await
                    .ok()
                    .flatten()
                    .ok_or_else(|| "no language model".to_string())?;
                // `eventRef.current ?? { title, participants }`
                let event = match event {
                    Some(event) => event,
                    None => BriefEvent {
                        title: Some(session_title),
                        participants: participants
                            .await
                            .ok()
                            .and_then(Result::ok)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|(name, email)| BriefParticipant {
                                name: Some(name),
                                email: Some(email),
                                ..BriefParticipant::default()
                            })
                            .collect(),
                        ..BriefEvent::default()
                    },
                };
                let (system, prompt) = pre_meeting::brief_prompts(&language, &event, &notes)?;
                let mut request =
                    Request::new(system, prompt, pre_meeting::BRIEF_MAX_OUTPUT_TOKENS);
                request.json_schema = Some(pre_meeting::brief_schema());
                let mut receiver = llm_stream::stream(store.runtime(), conn, request);
                let mut text = String::new();
                let mut first = true;
                let deadline = cx.background_executor().timer(Duration::from_secs(
                    pre_meeting::BRIEF_GENERATION_TIMEOUT_SECS,
                ));
                let mut deadline = std::pin::pin!(deadline);
                loop {
                    let next = receiver.recv();
                    let next = std::pin::pin!(next);
                    let chunk = match futures_util::future::select(next, deadline.as_mut()).await {
                        futures_util::future::Either::Left((chunk, _)) => chunk,
                        futures_util::future::Either::Right(_) => {
                            return Err("The pre-meeting brief timed out.".to_string());
                        }
                    };
                    match chunk {
                        Some(Chunk::TextDelta(delta)) => {
                            text.push_str(&delta);
                            if let Some((opener, bullets)) =
                                pre_meeting::brief_from_partial_json(&text)
                            {
                                let markdown = pre_meeting::format_pre_meeting_brief(
                                    opener.as_deref(),
                                    &bullets,
                                );
                                if !markdown.is_empty() {
                                    let applied = this
                                        .update(cx, |this, cx| {
                                            this.apply_brief(
                                                &session_id,
                                                &markdown,
                                                first,
                                                false,
                                                cx,
                                            )
                                        })
                                        .unwrap_or(false);
                                    if applied {
                                        first = false;
                                    }
                                }
                            }
                        }
                        Some(Chunk::ReasoningDelta(_) | Chunk::ToolCall(_) | Chunk::Item(_)) => {}
                        Some(Chunk::Error(error)) => return Err(error),
                        Some(Chunk::Done) | None => break,
                    }
                }
                let brief = match pre_meeting::brief_from_partial_json(&text)
                    .filter(|_| serde_json::from_str::<serde_json::Value>(text.trim()).is_ok())
                {
                    Some((opener, bullets)) => {
                        pre_meeting::format_pre_meeting_brief(opener.as_deref(), &bullets)
                    }
                    None => {
                        // `NoObjectGeneratedError`: salvage a reply with exactly
                        // three bullets, like the web app.
                        let brief = pre_meeting::trim_pre_meeting_brief(&text);
                        if brief.lines().filter(|line| line.starts_with("- ")).count() != 3 {
                            return Err(
                                "No object generated: could not parse the response.".to_string()
                            );
                        }
                        brief
                    }
                };
                if brief.is_empty() {
                    return Err("empty-brief".to_string());
                }
                this.update(cx, |this, cx| {
                    this.apply_brief(&session_id, &brief, first, true, cx);
                })
                .ok();
                Ok(())
            }
            .await;
            this.update(cx, |this, cx| {
                this.brief_jobs.remove(&session_id);
                if let Err(error) = outcome {
                    tracing::error!(%error, "Failed to create pre-meeting brief");
                    // `sonnerToast.error(…, { id: "pre-meeting-brief-error" })`
                    this.flash(
                        FlashVariant::Error,
                        "Could not create the pre-meeting brief. Try again.",
                        cx,
                    );
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `applyBriefToEditor` when the memo editor is mounted for the session
    /// (flushing on the final call), else `persistBrief` writes the memo.
    fn apply_brief(
        &mut self,
        session_id: &str,
        markdown: &str,
        first: bool,
        flush: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        // The suggestion only shows for an empty memo, so `existingMarkdown`
        // is empty and the merge is the brief itself.
        let body = crate::document::md2json(&pre_meeting::merge_brief_markdown(markdown, ""));
        if let Some(editor) = self
            .editor
            .clone()
            .filter(|editor| editor.read(cx).session_id == session_id)
        {
            editor.update(cx, |editor, cx| {
                editor.replace_body_generated(&body, first, cx);
                if flush {
                    editor.flush(cx);
                }
            });
            return true;
        }
        let task = self.store.update_memo(session_id.to_string(), body);
        let session_id = session_id.to_string();
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(())) = task.await {
                this.update(cx, |this, cx| {
                    if this.selected.as_deref() == Some(session_id.as_str()) {
                        this.reload_note(session_id, cx);
                    }
                })
                .ok();
            }
        })
        .detach();
        true
    }
}
