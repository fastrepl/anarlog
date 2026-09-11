//! `useContactSummary` and `ContactSummarySection`: the AI relationship
//! brief on the person detail view. The query is keyed by the person and
//! the hash of their newest meetings; a stale key drops the run, a stored
//! brief for the current key needs no model call.

use std::time::Duration;

use gpui::{AnimationExt as _, ClickEvent, Div, MouseButton, SharedString, div, prelude::*, px};

use super::Workspace;
use crate::contact_summary::{self, MeetingSnapshot, Summary, Target};
use crate::contacts::{Human, HumanSession};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, spinner};

/// `useQuery({ queryKey: ["contact-summary", humanId, sourceHash], retry: 1 })`.
#[derive(Default)]
pub(super) struct SummaryQuery {
    /// The `sourceHash` this state belongs to.
    key: String,
    /// `query.isFetching`.
    generating: bool,
    /// `query.data`: `Some(None)` once a run found nothing to summarize.
    data: Option<Option<Summary>>,
    /// `query.error`.
    error: Option<String>,
    /// Bumped per run; a finished run whose number moved on is dropped.
    run: u64,
    /// The in-flight run, aborted like the query's `AbortSignal` when the
    /// key changes.
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for SummaryQuery {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl SummaryQuery {
    fn reset(&mut self, key: String) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.key = key;
        self.generating = false;
        self.data = None;
        self.error = None;
        self.run += 1;
    }
}

impl Workspace {
    /// Re-key the query for the selected person's current meetings and
    /// start a run when the stored brief is not theirs (`enabled`).
    pub(super) fn sync_contact_summary(&mut self, cx: &mut Context<Self>) {
        let Some((human, sessions)) = self.contact_summary_inputs() else {
            return;
        };
        let hash = contact_summary::source_hash(&sessions);
        let has_llm = self.provider_settings.has_llm();
        let Some(query) = self.contact_summary_query_mut() else {
            return;
        };
        if query.key != hash {
            query.reset(hash.clone());
        }
        let never_ran = query.data.is_none() && query.error.is_none() && query.task.is_none();
        let enabled =
            has_llm && contact_summary::needs_generation(human.summary.as_ref(), &sessions, &hash);
        if never_ran && enabled {
            self.start_contact_summary(cx);
        }
    }

    /// `summary.retry()`: `refetch` runs regardless of `enabled`.
    pub(super) fn retry_contact_summary(&mut self, cx: &mut Context<Self>) {
        self.start_contact_summary(cx);
    }

    fn contact_summary_inputs(&self) -> Option<(Human, Vec<HumanSession>)> {
        let state = self.contacts.as_ref()?;
        let details = state.details.as_ref()?;
        let human = state.humans.iter().find(|human| human.id == details.id)?;
        Some((human.clone(), details.sessions.clone()))
    }

    fn contact_summary_query_mut(&mut self) -> Option<&mut SummaryQuery> {
        self.contacts
            .as_mut()?
            .details
            .as_mut()
            .map(|details| &mut details.summary)
    }

    fn start_contact_summary(&mut self, cx: &mut Context<Self>) {
        let Some((human, sessions)) = self.contact_summary_inputs() else {
            return;
        };
        let organization = self.contacts.as_ref().and_then(|state| {
            state
                .organizations
                .iter()
                .find(|organization| organization.id == human.organization_id)
                .map(|organization| organization.name.clone())
        });
        let store = self.store.clone();
        let connection = store.llm_connection(&self.provider_settings);
        let Some(query) = self.contact_summary_query_mut() else {
            return;
        };
        if let Some(task) = query.task.take() {
            task.abort();
        }
        query.generating = true;
        query.error = None;
        query.run += 1;
        let run = query.run;
        let hash = query.key.clone();
        let human_id = human.id.clone();
        let target = Target {
            name: human.name.clone(),
            email: human.email.clone(),
            job_title: human.job_title.clone(),
            organization,
            notes: human.memo.clone(),
        };
        let saved = human.summary.clone();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let pipeline_store = store.clone();
        let pipeline_id = human_id.clone();
        let task = store.runtime().spawn(async move {
            let Ok(Some(conn)) = connection.await else {
                // `enabled: false` without a model: nothing runs, nothing fails.
                let _ = sender.send(None);
                return;
            };
            let mut result = generate(
                &pipeline_store,
                &conn,
                &pipeline_id,
                &target,
                saved.as_ref(),
                &sessions,
                &hash,
            )
            .await;
            if result.is_err() {
                // `retry: 1` with the default one-second first delay.
                tokio::time::sleep(Duration::from_secs(1)).await;
                result = generate(
                    &pipeline_store,
                    &conn,
                    &pipeline_id,
                    &target,
                    saved.as_ref(),
                    &sessions,
                    &hash,
                )
                .await;
            }
            let _ = sender.send(Some(result));
        });
        query.task = Some(task);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Ok(outcome) = receiver.await else {
                return;
            };
            this.update(cx, |this, cx| {
                let Some(state) = this.contacts.as_mut() else {
                    return;
                };
                let Some(details) = state
                    .details
                    .as_mut()
                    .filter(|details| details.id == human_id && details.summary.run == run)
                else {
                    return;
                };
                let query = &mut details.summary;
                query.generating = false;
                query.task = None;
                match outcome {
                    None => {}
                    Some(Ok(data)) => {
                        if let Some(summary) = &data
                            && let Some(human) = state.humans.iter_mut().find(|h| h.id == human_id)
                        {
                            human.summary = Some(summary.clone());
                        }
                        query.data = Some(data);
                    }
                    Some(Err(error)) => {
                        tracing::error!(%error, "[contacts] failed to generate contact summary");
                        query.error = Some(error);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `ContactSummarySection`.
    pub(super) fn render_contact_summary(
        &self,
        human: &Human,
        query: &SummaryQuery,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        // `query.data ?? savedSummary`.
        let facts: Vec<String> = match &query.data {
            Some(Some(summary)) => summary.facts.clone(),
            _ => human
                .summary
                .as_ref()
                .map(|summary| summary.facts.clone())
                .unwrap_or_default(),
        };
        let has_facts = !facts.is_empty();
        let generating = query.generating;
        let failed = query.error.is_some();
        let try_again = |id: &'static str| {
            div()
                .id(id)
                .flex()
                .flex_shrink_0()
                .h(px(28.0))
                .items_center()
                .justify_center()
                .px_2()
                .rounded_full()
                .tw_text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .cursor_pointer()
                .hover(move |style| style.bg(theme.accent))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.retry_contact_summary(cx);
                }))
                .child("Try again")
        };
        let body = if has_facts {
            // `list-disc space-y-2 pl-5 text-sm leading-relaxed`, as WebKit
            // lays it out: the list reset wins over `space-y-2`'s zero-
            // specificity margins, so the items sit at the 22px line pitch
            // with the disc marker 17px left of the text.
            div()
                .flex()
                .flex_col()
                .pl_5()
                .tw_text_sm()
                .line_height(px(22.0))
                .text_color(theme.foreground)
                .children(facts.iter().map(|fact| {
                    div()
                        .relative()
                        .child(
                            div()
                                .absolute()
                                .left(px(-17.0))
                                .top(px(9.0))
                                .size(px(5.0))
                                .rounded_full()
                                .bg(theme.foreground),
                        )
                        .child(SharedString::from(fact.clone()))
                }))
                .into_any_element()
        } else if generating {
            // The `animate-pulse` skeleton: three dotted bars fading
            // 1 → 0.5 → 1 over two seconds, each 150ms behind the last.
            // `space-y-2.5` loses to the app's `* { margin: 0 }` reset, so
            // the rows touch.
            div()
                .flex()
                .flex_col()
                .py_1()
                .children([0.8_f32, 2.0 / 3.0, 0.6].into_iter().enumerate().map(
                    |(index, width)| {
                        let delay = index as f32 * 0.075;
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .size(px(4.0))
                                    .flex_shrink_0()
                                    .rounded_full()
                                    .bg(alpha(theme.muted_foreground, 0.2)),
                            )
                            .child(
                                div()
                                    .h_3()
                                    .rounded_full()
                                    .w(gpui::relative(width))
                                    .bg(alpha(theme.muted_foreground, 0.1)),
                            )
                            .with_animation(
                                SharedString::from(format!("contact-summary-skeleton-{index}")),
                                gpui::Animation::new(Duration::from_secs(2)).repeat(),
                                move |row, delta| row.opacity(pulse(delta - delay)),
                            )
                            .into_any_element()
                    },
                ))
                .into_any_element()
        } else if failed {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .tw_text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Summary generation failed"),
                )
                .child(try_again("contact-summary-retry"))
                .into_any_element()
        } else {
            div()
                .tw_text_sm()
                .line_height(px(22.0))
                .text_color(theme.muted_foreground)
                .child("AI-generated summary of all interactions and notes with this contact will appear here. This will synthesize key discussion points, action items, and relationship context across all meetings and notes.")
                .into_any_element()
        };
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
                    )
                    .when(generating, |header| {
                        header.child(spinner(
                            "contact-summary-spinner",
                            px(14.0),
                            theme.muted_foreground,
                        ))
                    }),
            )
            .child(
                div()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.muted)
                    .p_4()
                    .child(body)
                    .when(has_facts && failed, |card| {
                        card.child(
                            div()
                                .mt_3()
                                .pt_3()
                                .border_t_1()
                                .border_color(theme.border)
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child(
                                    div()
                                        .tw_text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("Summary generation failed"),
                                )
                                .child(try_again("contact-summary-retry-footer")),
                        )
                    }),
            )
    }
}

/// Tailwind's `pulse` keyframes: opacity 1 at the ends, 0.5 halfway, each
/// half eased like `cubic-bezier(0.4, 0, 0.6, 1)`.
fn pulse(t: f32) -> f32 {
    let t = t.rem_euclid(1.0);
    let half = if t < 0.5 { t * 2.0 } else { 2.0 - t * 2.0 };
    let eased = half * half * (3.0 - 2.0 * half);
    1.0 - 0.5 * eased
}

/// `generateAndSaveContactSummary`: read the meetings the brief has not
/// seen, ask the model for the facts, and store the record.
async fn generate(
    store: &crate::db::Store,
    conn: &crate::llm_stream::Connection,
    human_id: &str,
    target: &Target,
    saved: Option<&Summary>,
    sessions: &[HumanSession],
    hash: &str,
) -> Result<Option<Summary>, String> {
    let existing_facts =
        contact_summary::incremental_update(saved, sessions).map(|(facts, _)| facts);
    let mut snapshots = Vec::new();
    for session in contact_summary::sessions_to_read(saved, sessions) {
        let snapshot = store
            .enhancer_snapshot(session.id.clone())
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        if let Some(snapshot) = snapshot {
            snapshots.push(meeting_snapshot(&snapshot));
        }
    }
    let meetings = contact_summary::build_source(&snapshots);
    if meetings.is_empty() {
        return Ok(None);
    }
    let mut request = crate::llm_stream::Request::new(
        contact_summary::SYSTEM_PROMPT,
        contact_summary::prompt(target, existing_facts, &meetings),
        contact_summary::MAX_OUTPUT_TOKENS,
    );
    request.json_schema = Some(contact_summary::schema());
    let output = tokio::time::timeout(
        contact_summary::GENERATION_TIMEOUT,
        crate::llm_stream::generate_object(conn, &request, contact_summary::MAX_RETRIES),
    )
    .await
    .map_err(|_| "The contact summary generation timed out.".to_string())??;
    let facts = contact_summary::normalize_facts(&contact_summary::facts_from_output(&output)?);
    if facts.len() < 3 {
        return Err("Contact summary requires at least three facts".to_string());
    }
    let summary = contact_summary::record(facts, hash, sessions, now_iso());
    store
        .update_human_contact_summary(human_id.to_string(), summary.clone())
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(Some(summary))
}

/// The `SessionContentSnapshot` fields the brief reads, with each note's
/// `markdown || content`.
fn meeting_snapshot(snapshot: &crate::enhancer::Snapshot) -> MeetingSnapshot {
    let or_content = |markdown: String, content: &str| {
        if markdown.is_empty() {
            content.to_string()
        } else {
            markdown
        }
    };
    MeetingSnapshot {
        session_id: snapshot.session_id.clone(),
        title: snapshot.title.clone(),
        created_at: snapshot.created_at.clone(),
        enhanced_notes: snapshot
            .enhanced_notes
            .iter()
            .map(|note| {
                or_content(
                    crate::db::enhancer::body_to_markdown(&note.content, &note.content_format),
                    &note.content,
                )
            })
            .collect(),
        raw_note: or_content(snapshot.raw_markdown.clone(), &snapshot.raw_content),
        transcript_words: snapshot
            .transcripts
            .iter()
            .flat_map(|transcript| transcript.words.iter().cloned())
            .collect(),
    }
}

/// `new Date().toISOString()`.
fn now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_dims_halfway_and_wraps() {
        assert!((pulse(0.0) - 1.0).abs() < 1e-6);
        assert!((pulse(0.5) - 0.5).abs() < 1e-6);
        assert!((pulse(1.0) - 1.0).abs() < 1e-6);
        assert!((pulse(-0.25) - pulse(0.75)).abs() < 1e-6);
        assert!(pulse(0.25) < 1.0 && pulse(0.25) > 0.5);
    }

    #[test]
    fn meeting_snapshot_falls_back_to_the_stored_body() {
        let snapshot = crate::enhancer::Snapshot {
            session_id: "s".into(),
            owner_user_id: String::new(),
            title: "T".into(),
            created_at: "2026-03-01T00:00:00.000Z".into(),
            event_id: String::new(),
            event_json: String::new(),
            meeting_chat: String::new(),
            raw_note_id: None,
            raw_template_id: String::new(),
            raw_content: "# memo".into(),
            raw_content_format: "markdown".into(),
            raw_markdown: "# memo".into(),
            enhanced_notes: vec![crate::enhancer::EnhancedNote {
                id: "n".into(),
                title: String::new(),
                content: r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Hi"}]}]}"#.into(),
                content_format: "prosemirror_json".into(),
                template_id: String::new(),
                position: 0,
            }],
            transcripts: vec![crate::enhancer::SnapshotTranscript {
                id: "t".into(),
                started_at: 0,
                ended_at: None,
                memo: String::new(),
                words: vec!["a".into(), "b".into()],
            }],
            participants: Vec::new(),
            segments: Vec::new(),
            supplemental_context: String::new(),
        };
        let meeting = meeting_snapshot(&snapshot);
        assert_eq!(meeting.enhanced_notes, vec!["Hi".to_string()]);
        assert_eq!(meeting.raw_note, "# memo");
        assert_eq!(meeting.transcript_words, vec!["a", "b"]);
    }
}
