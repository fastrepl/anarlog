//! `services/enhancer/index.ts` (`EnhancerService`), the `enhance` / `title`
//! tasks of `store/zustand/ai-task`, and the enhanced tab's streaming, error,
//! and header states (`note-input/enhanced/*`, `header-enhanced.tsx`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AsyncApp, ClickEvent, Context, Div, InteractiveElement as _, MouseButton, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px,
};

use super::Workspace;
use super::toast::FlashVariant;
use crate::db::{DocumentUpdate, Store};
use crate::enhancer::prompts::{self, EnhanceArgs};
use crate::enhancer::runner::{self, Generation, Progress, Step};
use crate::enhancer::text::{
    append_tag_line, ensure_markdown_first_line_title, extract_tag_names, has_summary_content,
    persistable_generated_title, should_hydrate_template_title,
};
use crate::enhancer::validator::EnhanceValidator;
use crate::enhancer::{PendingJob, Snapshot, resolve_template_id};
use crate::ui::{TailwindText as _, icon};

const PENDING_AUTO_ENHANCE_RECOVERY_INTERVAL: Duration = Duration::from_secs(5);
const MAX_AUTO_ENHANCE_FAILURES: u32 = 8;
const AUTO_ENHANCE_BACKOFF_BASE: Duration = Duration::from_secs(30);
const AUTO_ENHANCE_BACKOFF_MAX: Duration = Duration::from_secs(15 * 60);
const SUMMARY_MAX_OUTPUT_TOKENS: u32 = 8192;
const TITLE_MAX_OUTPUT_TOKENS: u32 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskStatus {
    Generating,
    Success,
    Error,
}

/// `AutoEnhanceMode`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutoEnhanceMode {
    Regenerate,
    IfEmpty,
}

/// `TaskState` for one `${entityId}-enhance` / `${sessionId}-title` task.
#[derive(Debug, Clone)]
pub(crate) struct TaskState {
    pub status: TaskStatus,
    pub streamed_text: String,
    /// The streamed markdown rendered for the streaming view.
    pub blocks: Vec<crate::document::Block>,
    pub error: Option<String>,
    pub step: Option<Step>,
    /// Bumped by reset / regenerate so a stale run cannot clobber the state.
    run: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct EnhanceOpts {
    pub is_auto: bool,
    pub pending: Option<PendingJob>,
    /// `Some(None)` is an explicit "Auto" (no template).
    pub template_id: Option<Option<String>>,
    pub target_note_id: Option<String>,
    pub template_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EnhanceResult {
    Started { note_id: String },
    AlreadyActive { note_id: String },
    NoModel,
    TooShort,
}

#[derive(Default)]
pub(crate) struct EnhancerState {
    tasks: HashMap<String, TaskState>,
    next_run: u64,
    /// `activeAutoEnhance`: sessions with an auto-summary in flight.
    active_auto: HashMap<String, Option<PendingJob>>,
    /// `autoEnhanceFailures` by `session:generation`.
    failures: HashMap<String, (u32, Instant)>,
    resume_scheduled: bool,
    /// `pendingRetries`: sessions waiting on the 500ms eligibility recheck.
    eligibility_retries: HashMap<String, u64>,
}

pub(crate) fn enhance_task_id(note_id: &str) -> String {
    format!("{note_id}-enhance")
}

pub(crate) fn title_task_id(session_id: &str) -> String {
    format!("{session_id}-title")
}

fn failure_key(job: &PendingJob) -> String {
    format!("{}:{}", job.session_id, job.generation)
}

impl Workspace {
    pub(crate) fn enhance_task(&self, task_id: &str) -> Option<&TaskState> {
        self.enhancer.tasks.get(task_id)
    }

    pub(crate) fn enhance_generating(&self, note_id: &str) -> bool {
        self.enhance_task(&enhance_task_id(note_id))
            .is_some_and(|task| task.status == TaskStatus::Generating)
    }

    /// `EnhancerService.start`: resume durable auto-summaries.
    pub(crate) fn start_enhancer(&mut self, cx: &mut Context<Self>) {
        self.schedule_pending_resume(Duration::ZERO, cx);
    }

    /// The listener subscription: a session that starts capturing again drops
    /// its queued auto-summary.
    pub(crate) fn enhancer_on_live_started(&mut self, session_id: &str) {
        self.enhancer.active_auto.remove(session_id);
        self.enhancer.eligibility_retries.remove(session_id);
    }

    /// `requestAutoEnhance(sessionId, mode)`: `regenerate` (a capture that
    /// extended an existing transcript) re-runs the summary of the note the
    /// auto-enhance would pick, dropping its tasks and retries first;
    /// `if_empty` is `queueAutoEnhanceIfSummaryEmpty`.
    pub(crate) fn request_auto_enhance_with(
        &mut self,
        session_id: String,
        mode: AutoEnhanceMode,
        cx: &mut Context<Self>,
    ) {
        match mode {
            AutoEnhanceMode::IfEmpty => self.queue_auto_enhance_if_summary_empty(session_id, cx),
            AutoEnhanceMode::Regenerate => {
                let store = self.store.clone();
                cx.spawn(async move |this, cx| {
                    let snapshot = match load_snapshot(&store, &session_id).await {
                        Ok(snapshot) => snapshot,
                        Err(error) => {
                            tracing::error!(%error, "[enhancer] failed to load session");
                            return;
                        }
                    };
                    let (_, selected) = match store.enhancer_settings().await {
                        Ok(Ok(settings)) => settings,
                        _ => (Default::default(), None),
                    };
                    let template_id =
                        resolve_template_id(None, &snapshot.raw_template_id, selected.as_deref());
                    let template = match snapshot.auto_enhanced_note(template_id.as_deref()) {
                        Some(note) => Some(note.template_id.clone()).filter(|id| !id.is_empty()),
                        None => template_id,
                    };
                    // `ensurePendingAutoEnhanceDocument`
                    let pending = match store
                        .enhancer_ensure_summary(session_id.clone(), template, true)
                        .await
                        .map_err(anyhow::Error::from)
                        .and_then(|result| result)
                    {
                        Ok((_, Some(pending))) => pending,
                        Ok((_, None)) => return,
                        Err(error) => {
                            tracing::error!(%error, "[enhancer] failed to prepare summary document");
                            return;
                        }
                    };
                    let note_ids: Vec<String> =
                        snapshot.enhanced_notes.iter().map(|note| note.id.clone()).collect();
                    this.update(cx, |this, cx| {
                        // `resetEnhanceTasks` / `activeAutoEnhance.delete` / `clearRetry`
                        for note_id in note_ids {
                            this.enhancer.tasks.remove(&enhance_task_id(&note_id));
                        }
                        this.enhancer.active_auto.remove(&session_id);
                        this.enhancer.eligibility_retries.remove(&session_id);
                        this.schedule_pending_resume(PENDING_AUTO_ENHANCE_RECOVERY_INTERVAL, cx);
                        this.queue_auto_enhance(session_id.clone(), Some(pending), cx);
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        }
    }

    /// `queueAutoEnhanceIfSummaryEmpty`.
    pub(crate) fn queue_auto_enhance_if_summary_empty(
        &mut self,
        session_id: String,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let snapshot = match load_snapshot(&store, &session_id).await {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    tracing::error!(%error, "[enhancer] failed to load session");
                    return;
                }
            };
            let (_, selected) = match store.enhancer_settings().await {
                Ok(Ok(settings)) => settings,
                _ => (Default::default(), None),
            };
            let template_id =
                resolve_template_id(None, &snapshot.raw_template_id, selected.as_deref());
            let existing = snapshot.auto_enhanced_note(template_id.as_deref()).cloned();
            if let Some(note) = &existing
                && has_summary_content(&note.content, Some(&snapshot.title))
            {
                // `summary_exists`
                return;
            }
            let eligibility = snapshot.eligibility();
            if eligibility.is_eligible() {
                let template = existing
                    .as_ref()
                    .map(|note| Some(note.template_id.clone()).filter(|id| !id.is_empty()))
                    .unwrap_or(template_id);
                let pending = match store
                    .enhancer_ensure_summary(session_id.clone(), template, true)
                    .await
                    .map_err(anyhow::Error::from)
                    .and_then(|result| result)
                {
                    Ok((_, Some(pending))) => pending,
                    Ok((_, None)) => return,
                    Err(error) => {
                        tracing::error!(%error, "[enhancer] failed to prepare summary document");
                        return;
                    }
                };
                this.update(cx, |this, cx| {
                    this.schedule_pending_resume(PENDING_AUTO_ENHANCE_RECOVERY_INTERVAL, cx);
                    this.queue_auto_enhance(session_id.clone(), Some(pending), cx);
                })
                .ok();
            } else if existing.is_none() && eligibility.word_count() > 0 {
                if let Ok(Ok((note, _))) = store
                    .enhancer_ensure_summary(session_id.clone(), template_id.clone(), false)
                    .await
                    && let Some(template_id) = template_id
                {
                    hydrate_template_title(&store, &session_id, &note.id, &template_id).await;
                }
                this.update(cx, |this, cx| {
                    if this.selected.as_deref() == Some(session_id.as_str()) {
                        this.reload_note(session_id.clone(), cx);
                    }
                    this.queue_auto_enhance(session_id.clone(), None, cx);
                })
                .ok();
            } else {
                this.update(cx, |this, cx| {
                    this.queue_auto_enhance(session_id.clone(), None, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// `queueAutoEnhance`.
    fn queue_auto_enhance(
        &mut self,
        session_id: String,
        pending: Option<PendingJob>,
        cx: &mut Context<Self>,
    ) {
        if self.enhancer.active_auto.contains_key(&session_id) {
            return;
        }
        self.enhancer
            .active_auto
            .insert(session_id.clone(), pending);
        self.run_auto_enhance(session_id, 0, cx);
    }

    /// `runAutoEnhance` → `tryAutoEnhance`: the eligibility recheck loop and
    /// the auto `enhance`.
    fn run_auto_enhance(&mut self, session_id: String, attempt: u32, cx: &mut Context<Self>) {
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let active = this
                .read_with(cx, |this, _| {
                    this.enhancer.active_auto.contains_key(&session_id)
                })
                .unwrap_or(false);
            if !active {
                return;
            }
            let eligibility = match load_snapshot(&store, &session_id).await {
                Ok(snapshot) => snapshot.eligibility(),
                Err(error) => {
                    this.update(cx, |this, cx| {
                        this.handle_auto_enhance_error(&session_id, &error.to_string(), cx)
                    })
                    .ok();
                    return;
                }
            };
            let still_active = this
                .read_with(cx, |this, _| {
                    this.enhancer.active_auto.contains_key(&session_id)
                })
                .unwrap_or(false);
            if !still_active {
                return;
            }
            if !eligibility.is_eligible() {
                if attempt < 20 {
                    let retry = this
                        .update(cx, |this, _| {
                            this.enhancer.next_run += 1;
                            let token = this.enhancer.next_run;
                            this.enhancer
                                .eligibility_retries
                                .insert(session_id.clone(), token);
                            token
                        })
                        .unwrap_or(0);
                    cx.background_executor()
                        .timer(Duration::from_millis(500))
                        .await;
                    this.update(cx, |this, cx| {
                        if this.enhancer.eligibility_retries.get(&session_id) == Some(&retry) {
                            this.enhancer.eligibility_retries.remove(&session_id);
                            this.run_auto_enhance(session_id.clone(), attempt + 1, cx);
                        }
                    })
                    .ok();
                    return;
                }
                let pending = this
                    .update(cx, |this, _| {
                        this.enhancer.active_auto.remove(&session_id).flatten()
                    })
                    .ok()
                    .flatten();
                if let Some(pending) = pending {
                    let _ = store.enhancer_discard_pending(pending).await;
                }
                if let crate::enhancer::Eligibility::Ineligible { reason, code, .. } = eligibility {
                    this.update(cx, |this, cx| {
                        this.on_auto_enhance_skipped(&session_id, &reason, Some(code), cx)
                    })
                    .ok();
                }
                return;
            }

            let pending = this
                .read_with(cx, |this, _| {
                    this.enhancer
                        .active_auto
                        .get(&session_id)
                        .cloned()
                        .flatten()
                })
                .ok()
                .flatten();
            let result = enhance_flow(
                &store,
                &this,
                cx,
                session_id.clone(),
                EnhanceOpts {
                    is_auto: true,
                    pending: pending.clone(),
                    ..EnhanceOpts::default()
                },
            )
            .await;
            let still_active = this
                .read_with(cx, |this, _| {
                    this.enhancer.active_auto.contains_key(&session_id)
                })
                .unwrap_or(false);
            if !still_active {
                return;
            }
            match result {
                Ok(EnhanceResult::TooShort) => {
                    this.update(cx, |this, _| {
                        this.enhancer.active_auto.remove(&session_id);
                    })
                    .ok();
                    if let Some(pending) = pending {
                        let _ = store.enhancer_discard_pending(pending).await;
                    }
                }
                Ok(EnhanceResult::NoModel) => {
                    this.update(cx, |this, cx| {
                        this.enhancer.active_auto.remove(&session_id);
                        if pending.is_some() {
                            this.schedule_pending_resume(
                                PENDING_AUTO_ENHANCE_RECOVERY_INTERVAL,
                                cx,
                            );
                        }
                        tracing::info!(session_id, "auto-enhance-no-model");
                    })
                    .ok();
                }
                Ok(EnhanceResult::Started { note_id })
                | Ok(EnhanceResult::AlreadyActive { note_id }) => {
                    this.update(cx, |this, cx| {
                        this.enhancer.active_auto.remove(&session_id);
                        this.on_auto_enhance_started(&session_id, &note_id, cx);
                    })
                    .ok();
                }
                Err(error) => {
                    this.update(cx, |this, cx| {
                        this.handle_auto_enhance_error(&session_id, &error.to_string(), cx)
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// `useAutoEnhance`'s `auto-enhance-skipped` listener: the toast for a
    /// short transcript.
    fn on_auto_enhance_skipped(
        &mut self,
        session_id: &str,
        reason: &str,
        code: Option<crate::enhancer::SkipCode>,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(session_id, reason, "auto-enhance-skipped");
        if code == Some(crate::enhancer::SkipCode::TranscriptTooShort) {
            self.flash(
                FlashVariant::Warning,
                format!("Summary wasn't generated: {reason}"),
                cx,
            );
        }
    }

    /// `auto-enhance-started`: the open note switches to the new summary.
    fn on_auto_enhance_started(&mut self, session_id: &str, note_id: &str, cx: &mut Context<Self>) {
        if self.selected.as_deref() == Some(session_id) {
            self.pending_enhanced_tab = Some(note_id.to_string());
            self.reload_note(session_id.to_string(), cx);
        }
    }

    fn handle_auto_enhance_error(
        &mut self,
        session_id: &str,
        reason: &str,
        cx: &mut Context<Self>,
    ) {
        self.enhancer.active_auto.remove(session_id);
        self.enhancer.eligibility_retries.remove(session_id);
        tracing::error!(session_id, reason, "[enhancer] auto-enhance failed");
        self.schedule_pending_resume(PENDING_AUTO_ENHANCE_RECOVERY_INTERVAL, cx);
        self.on_auto_enhance_skipped(session_id, reason, None, cx);
    }

    /// `schedulePendingAutoEnhanceResume` / `resumePendingAutoEnhance`.
    fn schedule_pending_resume(&mut self, delay: Duration, cx: &mut Context<Self>) {
        if self.enhancer.resume_scheduled {
            return;
        }
        self.enhancer.resume_scheduled = true;
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            this.update(cx, |this, _| this.enhancer.resume_scheduled = false)
                .ok();
            let jobs = match store.enhancer_pending_jobs().await {
                Ok(Ok(jobs)) => jobs,
                Ok(Err(error)) => {
                    tracing::error!(%error, "[enhancer] failed to resume pending auto-enhance");
                    this.update(cx, |this, cx| {
                        this.schedule_pending_resume(PENDING_AUTO_ENHANCE_RECOVERY_INTERVAL, cx)
                    })
                    .ok();
                    return;
                }
                Err(_) => return,
            };
            this.update(cx, |this, cx| {
                for job in &jobs {
                    let live = this
                        .recording
                        .live
                        .as_ref()
                        .is_some_and(|live| live.session_id == job.session_id);
                    if live {
                        continue;
                    }
                    if this
                        .enhancer
                        .failures
                        .get(&failure_key(job))
                        .is_some_and(|(_, next)| Instant::now() < *next)
                    {
                        continue;
                    }
                    tracing::info!(session_id = %job.session_id, "[enhancer] resuming pending auto-enhance");
                    this.queue_auto_enhance(job.session_id.clone(), Some(job.clone()), cx);
                }
                if !jobs.is_empty() {
                    this.schedule_pending_resume(PENDING_AUTO_ENHANCE_RECOVERY_INTERVAL, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// `useEnhancedNoteActions.onRegenerate` (`None`: regenerate the note under
    /// its own template, keeping the stored summary until the new one lands)
    /// and `header-enhanced`'s `handleSelectTemplate` (`Some`: `service.enhance`
    /// with `templateId`, `targetNoteId`, and `templateTitle`).
    pub(crate) fn regenerate_summary(
        &mut self,
        session_id: String,
        note_id: String,
        template: Option<(Option<String>, Option<String>)>,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        let Some((template_id, title)) = template else {
            let note_template = match &self.note {
                super::Note::Ready { preview, .. } => preview
                    .enhanced
                    .iter()
                    .find(|doc| doc.id == note_id)
                    .map(|doc| doc.template_id.clone())
                    .filter(|id| !id.is_empty()),
                _ => None,
            };
            if self.enhance_generating(&note_id) {
                return;
            }
            let conn_task = self.store.llm_connection(&self.provider_settings);
            cx.spawn(async move |this, cx| {
                let Ok(Some(conn)) = conn_task.await else {
                    this.update(cx, |this, cx| {
                        this.flash(
                            FlashVariant::Error,
                            "Set up Intelligence in Settings before regenerating this summary.",
                            cx,
                        )
                    })
                    .ok();
                    return;
                };
                if let Ok(snapshot) = load_snapshot(&store, &session_id).await {
                    let eligibility = snapshot.eligibility();
                    if eligibility.too_short()
                        && let crate::enhancer::Eligibility::Ineligible { reason, code, .. } =
                            eligibility
                    {
                        this.update(cx, |this, cx| {
                            this.on_auto_enhance_skipped(&session_id, &reason, Some(code), cx)
                        })
                        .ok();
                        return;
                    }
                }
                generate_enhance(
                    store,
                    this,
                    cx,
                    GenerateArgs {
                        session_id,
                        note_id,
                        template_id: note_template,
                        pending: None,
                        conn,
                    },
                )
                .await;
            })
            .detach();
            return;
        };
        let opts = EnhanceOpts {
            template_id: Some(template_id),
            target_note_id: Some(note_id.clone()),
            template_title: title,
            ..EnhanceOpts::default()
        };
        cx.spawn(async move |this, cx| {
            let result = enhance_flow(&store, &this, cx, session_id.clone(), opts).await;
            this.update(cx, |this, cx| match result {
                Ok(EnhanceResult::NoModel) => this.flash(
                    FlashVariant::Error,
                    "Set up Intelligence in Settings before regenerating this summary.",
                    cx,
                ),
                Ok(EnhanceResult::TooShort) => {}
                Ok(EnhanceResult::Started { note_id: started })
                | Ok(EnhanceResult::AlreadyActive { note_id: started }) => {
                    if started != note_id && this.selected.as_deref() == Some(session_id.as_str()) {
                        this.pending_enhanced_tab = Some(started);
                        this.reload_note(session_id.clone(), cx);
                    }
                }
                Err(error) => {
                    tracing::error!(%error, "[enhancer] failed to replace summary template");
                    this.flash(FlashVariant::Error, error.to_string(), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// `EnhanceError`'s Retry: `generate` on the note's own template.
    fn retry_enhance(&mut self, session_id: String, note_id: String, cx: &mut Context<Self>) {
        self.enhancer.tasks.remove(&enhance_task_id(&note_id));
        self.regenerate_summary(session_id, note_id, None, cx);
    }

    fn set_task(
        &mut self,
        task_id: &str,
        run: u64,
        update: impl FnOnce(&mut TaskState),
        cx: &mut Context<Self>,
    ) -> bool {
        match self.enhancer.tasks.get_mut(task_id) {
            Some(task) if task.run == run => {
                update(task);
                cx.notify();
                true
            }
            _ => false,
        }
    }

    /// The `Enhanced` view: error, streaming, config error, or the document.
    pub(crate) fn render_enhanced_state(
        &self,
        preview: &crate::db::NotePreview,
        note_id: &str,
        has_content: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let task = self.enhance_task(&enhance_task_id(note_id))?;
        match task.status {
            TaskStatus::Error => Some(self.render_enhance_error(preview, note_id, task, cx)),
            TaskStatus::Generating => Some(self.render_streaming_view(preview, task, window)),
            // `isAwaitingPersistedContent`: the stream ended but the note has
            // not reloaded yet.
            TaskStatus::Success if !task.streamed_text.trim().is_empty() && !has_content => {
                Some(self.render_streaming_view(preview, task, window))
            }
            TaskStatus::Success => None,
        }
    }

    /// `StreamingView`.
    fn render_streaming_view(
        &self,
        preview: &crate::db::NotePreview,
        task: &TaskState,
        window: &Window,
    ) -> Div {
        let theme = self.theme;
        if task.streamed_text.trim().is_empty() {
            let reasoning = matches!(task.step, Some(Step::Reasoning));
            let local = self
                .provider_settings
                .llm_provider
                .as_deref()
                .is_some_and(crate::llm_stream::is_local_model_provider);
            // `text-muted-foreground flex flex-col gap-0.5 pb-2 text-sm`
            return div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .pb_2()
                .tw_text_sm()
                .text_color(theme.muted_foreground)
                .child(
                    div()
                        .line_height(px(20.0))
                        .opacity(0.7)
                        .child(if reasoning {
                            "Model is thinking..."
                        } else {
                            "Analyzing structure..."
                        }),
                )
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(6.0))
                        .pl_4()
                        .text_size(px(12.0))
                        .line_height(px(20.0))
                        .child(
                            // `border-muted-foreground/60 mt-[5px] h-2 w-2 rounded-bl-[2px] border-b border-l`
                            div()
                                .mt(px(5.0))
                                .size(px(8.0))
                                .flex_shrink_0()
                                .border_b_1()
                                .border_l_1()
                                .rounded_bl(px(2.0))
                                .border_color(gpui::Rgba {
                                    a: 0.6,
                                    ..theme.muted_foreground
                                }),
                        )
                        .child(if reasoning {
                            "Reasoning models think through the transcript before writing."
                        } else if local {
                            "On-device models can take a few minutes to warm up before text appears."
                        } else {
                            "Tip: The Anarlog team loves our users!"
                        }),
                );
        }

        let title = preview.session.title.trim().to_string();
        let generated_title = self
            .enhance_task(&title_task_id(&preview.session.id))
            .filter(|task| task.status != TaskStatus::Generating)
            .map(|task| persistable_generated_title(&task.streamed_text))
            .unwrap_or_default();
        let visible_title = if title.is_empty() {
            generated_title
        } else {
            title
        };
        // `SummaryTitleSpace`: `mb-4 min-h-[1.875rem]` with the `text-[1.5rem]
        // leading-[1.875rem] font-bold` title or the pulsing placeholder.
        let title_space =
            div()
                .mb_4()
                .flex()
                .min_h(px(30.0))
                .items_start()
                .child(if visible_title.is_empty() {
                    div()
                        .text_size(px(24.0))
                        .line_height(px(30.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme.muted_foreground)
                        .opacity(0.6)
                        .child("Generating title...")
                } else {
                    div()
                        .text_size(px(24.0))
                        .line_height(px(30.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme.foreground)
                        .child(SharedString::from(visible_title))
                });
        div()
            .pb_2()
            .flex()
            .flex_col()
            .gap_1()
            .child(title_space)
            .child(
                div()
                    .id("streaming-summary")
                    .flex()
                    .flex_col()
                    .children(self.document_renderer(window).blocks(&task.blocks, 0))
                    // Streamdown's block caret while the text is animating.
                    .when(task.status == TaskStatus::Generating, |body| {
                        body.child(div().mt_1().w(px(8.0)).h(px(18.0)).bg(theme.foreground))
                    }),
            )
    }

    /// `EnhanceError`: the warning glyph, the message, and Retry.
    fn render_enhance_error(
        &self,
        preview: &crate::db::NotePreview,
        note_id: &str,
        task: &TaskState,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let has_model = self.provider_settings.has_llm();
        let hovered = self.hovered == Some("enhance-retry");
        let session_id = preview.session.id.clone();
        let note_id = note_id.to_string();
        div()
            .flex()
            .h_full()
            .min_h(px(400.0))
            .flex_col()
            .items_center()
            .justify_center()
            .px_6()
            .text_center()
            .child(
                div()
                    .mb_5()
                    .child(icon("warning-circle", px(36.0), theme.muted_foreground)),
            )
            .child(
                div()
                    .mb_6()
                    .flex()
                    .max_w(px(448.0))
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .justify_center()
                            .tw_text_base()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child("Summary generation failed"),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_center()
                            .text_size(px(14.0))
                            .line_height(px(22.0))
                            .text_color(theme.muted_foreground)
                            .child(SharedString::from(
                                task.error
                                    .clone()
                                    .filter(|message| !message.is_empty())
                                    .unwrap_or_else(|| {
                                        "Something went wrong while generating the summary."
                                            .to_string()
                                    }),
                            )),
                    ),
            )
            .child(
                // `Button size="sm" className="gap-2"`: `h-8 px-3 rounded-md`.
                div()
                    .id("enhance-retry")
                    .relative()
                    .flex()
                    .h(px(32.0))
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .px_3()
                    .child(crate::squircle::squircle(
                        crate::squircle::CONTROL_RADIUS,
                        Some(if hovered && has_model {
                            gpui::Rgba {
                                a: 0.9,
                                ..theme.primary
                            }
                        } else {
                            theme.primary
                        }),
                        None,
                    ))
                    .tw_text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.primary_foreground)
                    .when(!has_model, |button| button.opacity(0.5))
                    .when(has_model, |button| button.cursor_pointer())
                    .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                        this.set_hovered("enhance-retry", *hovering, cx);
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .when(has_model, |button| {
                        button.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.retry_enhance(session_id.clone(), note_id.clone(), cx);
                        }))
                    })
                    .child(icon("arrow-clockwise", px(16.0), theme.primary_foreground))
                    .child("Retry"),
            )
    }
}

async fn load_snapshot(store: &Arc<Store>, session_id: &str) -> anyhow::Result<Snapshot> {
    match store.enhancer_snapshot(session_id.to_string()).await {
        Ok(Ok(Some(snapshot))) => Ok(snapshot),
        Ok(Ok(None)) => anyhow::bail!("Session {session_id} no longer exists"),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(error.into()),
    }
}

/// `hydrateTemplateTitle`: a placeholder title becomes the template's.
async fn hydrate_template_title(
    store: &Arc<Store>,
    session_id: &str,
    note_id: &str,
    template_id: &str,
) {
    let Ok(Ok(Some(template))) = store.enhancer_template(template_id.to_string()).await else {
        return;
    };
    let title = template.title.trim().to_string();
    if title.is_empty() {
        return;
    }
    let Ok(snapshot) = load_snapshot(store, session_id).await else {
        return;
    };
    let Some(note) = snapshot.enhanced_note(note_id) else {
        return;
    };
    if note.template_id != template_id
        || !should_hydrate_template_title(Some(&note.title), template_id)
    {
        return;
    }
    if let Ok(Err(error)) = store
        .enhancer_update_title_if_current(
            session_id.to_string(),
            note_id.to_string(),
            template_id.to_string(),
            note.title.clone(),
            title,
        )
        .await
    {
        tracing::error!(%error, "[enhancer] failed to hydrate template title");
    }
}

/// `EnhancerService.enhance`.
async fn enhance_flow(
    store: &Arc<Store>,
    this: &WeakEntity<Workspace>,
    cx: &mut AsyncApp,
    session_id: String,
    opts: EnhanceOpts,
) -> anyhow::Result<EnhanceResult> {
    let conn_task = this.update(cx, |this, _| {
        this.store.llm_connection(&this.provider_settings)
    })?;
    let Some(conn) = conn_task.await? else {
        return Ok(EnhanceResult::NoModel);
    };

    let snapshot = load_snapshot(store, &session_id).await?;
    let eligibility = snapshot.eligibility();
    if eligibility.too_short() {
        if let crate::enhancer::Eligibility::Ineligible { reason, code, .. } = &eligibility {
            let (reason, code) = (reason.clone(), *code);
            this.update(cx, |this, cx| {
                this.on_auto_enhance_skipped(&session_id, &reason, Some(code), cx)
            })
            .ok();
        }
        return Ok(EnhanceResult::TooShort);
    }

    let (_, selected) = store.enhancer_settings().await??;
    let mut template_id = resolve_template_id(
        opts.template_id
            .as_ref()
            .map(|template| template.as_deref()),
        &snapshot.raw_template_id,
        selected.as_deref(),
    );
    let pending_note = opts
        .pending
        .as_ref()
        .and_then(|pending| snapshot.enhanced_note(&pending.note_id))
        .cloned();
    let target_note = opts
        .target_note_id
        .as_ref()
        .and_then(|id| snapshot.enhanced_note(id))
        .cloned();
    let auto_note = if target_note.is_none() && pending_note.is_none() && opts.is_auto {
        snapshot.auto_enhanced_note(template_id.as_deref()).cloned()
    } else {
        None
    };
    if let Some(note) = pending_note.as_ref().or(auto_note.as_ref()) {
        template_id = Some(note.template_id.clone()).filter(|id| !id.is_empty());
    }

    let mut note = match target_note.clone().or(pending_note).or(auto_note) {
        Some(note) => note,
        None => {
            let (note, _) = store
                .enhancer_ensure_summary(session_id.clone(), template_id.clone(), false)
                .await??;
            if let Some(template_id) = &template_id {
                hydrate_template_title(store, &session_id, &note.id, template_id).await;
            }
            this.update(cx, |this, cx| {
                if this.selected.as_deref() == Some(session_id.as_str()) {
                    this.reload_note(session_id.clone(), cx);
                }
            })
            .ok();
            note
        }
    };
    let task_id = enhance_task_id(&note.id);
    let existing = this.read_with(cx, |this, _| this.enhance_task(&task_id).cloned())?;
    if existing
        .as_ref()
        .is_some_and(|task| task.status == TaskStatus::Generating)
    {
        return Ok(EnhanceResult::AlreadyActive { note_id: note.id });
    }

    if target_note.is_some() {
        let title = opts
            .template_title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .unwrap_or("Summary")
            .to_string();
        store
            .enhancer_replace_template(
                session_id.clone(),
                note.id.clone(),
                template_id.clone(),
                title.clone(),
            )
            .await??;
        if let Some(template_id) = &template_id
            && opts
                .template_title
                .as_deref()
                .map(str::trim)
                .is_none_or(str::is_empty)
        {
            hydrate_template_title(store, &session_id, &note.id, template_id).await;
        }
        note.title = title;
        note.content = String::new();
        note.content_format = "prosemirror_json".to_string();
        note.template_id = template_id.clone().unwrap_or_default();
        this.update(cx, |this, cx| {
            if this.selected.as_deref() == Some(session_id.as_str()) {
                this.reload_note(session_id.clone(), cx);
            }
        })
        .ok();
    }

    if existing
        .as_ref()
        .is_some_and(|task| task.status == TaskStatus::Success)
        && has_summary_content(&note.content, Some(&snapshot.title))
    {
        return Ok(EnhanceResult::AlreadyActive { note_id: note.id });
    }

    let note_id = note.id.clone();
    let args = GenerateArgs {
        session_id: session_id.clone(),
        note_id: note_id.clone(),
        template_id,
        pending: opts.pending.clone(),
        conn,
    };
    let store = store.clone();
    let this_for_task = this.clone();
    cx.spawn(async move |cx| {
        generate_enhance(store, this_for_task, cx, args).await;
    })
    .detach();
    Ok(EnhanceResult::Started { note_id })
}

struct GenerateArgs {
    session_id: String,
    note_id: String,
    template_id: Option<String>,
    pending: Option<PendingJob>,
    conn: crate::llm_stream::Connection,
}

/// `generate(enhanceTaskId, { taskType: "enhance" })`: the streamed run,
/// then `runEnhanceSuccess`.
async fn generate_enhance(
    store: Arc<Store>,
    this: WeakEntity<Workspace>,
    cx: &mut AsyncApp,
    args: GenerateArgs,
) {
    let task_id = enhance_task_id(&args.note_id);
    let Ok(run) = this.update(cx, |this, cx| {
        this.enhancer.next_run += 1;
        let run = this.enhancer.next_run;
        this.enhancer.tasks.insert(
            task_id.clone(),
            TaskState {
                status: TaskStatus::Generating,
                streamed_text: String::new(),
                blocks: Vec::new(),
                error: None,
                step: None,
                run,
            },
        );
        cx.notify();
        run
    }) else {
        return;
    };

    let outcome = generate_enhance_inner(&store, &this, cx, &args, &task_id, run).await;
    let set_error = |this: &WeakEntity<Workspace>, cx: &mut AsyncApp, message: String| {
        this.update(cx, |this, cx| {
            this.set_task(
                &task_id,
                run,
                |task| {
                    task.status = TaskStatus::Error;
                    task.streamed_text.clear();
                    task.blocks.clear();
                    task.error = Some(message);
                    task.step = None;
                },
                cx,
            );
        })
        .ok();
    };
    match outcome {
        Ok(()) => {
            this.update(cx, |this, cx| {
                let done = this.set_task(
                    &task_id,
                    run,
                    |task| {
                        task.status = TaskStatus::Success;
                        task.error = None;
                        task.step = None;
                    },
                    cx,
                );
                if done && this.selected.as_deref() == Some(args.session_id.as_str()) {
                    this.reload_note(args.session_id.clone(), cx);
                }
            })
            .ok();
        }
        Err(error) => {
            let message = error.to_string();
            tracing::error!(%message, "[enhancer] summary generation failed");
            set_error(&this, cx, message.clone());
            // `enhancement_failed`: durable auto jobs back off or are discarded.
            if let Some(pending) = args.pending.clone() {
                if is_retryable_error(&message) {
                    record_auto_failure(&store, &this, cx, &args.session_id, pending).await;
                } else {
                    this.update(cx, |this, _| {
                        this.enhancer.failures.remove(&failure_key(&pending));
                    })
                    .ok();
                    let _ = store.enhancer_discard_pending(pending).await;
                }
            }
        }
    }
}

/// `isRetryableAIError`: network and rate-limit / server failures.
fn is_retryable_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("error sending request")
        || lower.contains("connection")
        || lower.contains("did not return any text")
        || lower.contains("429")
        || lower.contains("rate limit")
        || lower.contains("overloaded")
        || lower.contains("(5")
        || lower.contains("500")
        || lower.contains("502")
        || lower.contains("503")
        || lower.contains("504")
}

async fn record_auto_failure(
    store: &Arc<Store>,
    this: &WeakEntity<Workspace>,
    cx: &mut AsyncApp,
    session_id: &str,
    job: PendingJob,
) {
    let key = failure_key(&job);
    let attempts = this
        .update(cx, |this, _| {
            let attempts = this
                .enhancer
                .failures
                .get(&key)
                .map_or(0, |(count, _)| *count)
                + 1;
            let backoff = if attempts >= MAX_AUTO_ENHANCE_FAILURES {
                AUTO_ENHANCE_BACKOFF_MAX
            } else {
                AUTO_ENHANCE_BACKOFF_BASE
                    .saturating_mul(1 << (attempts - 1).min(16))
                    .min(AUTO_ENHANCE_BACKOFF_MAX)
            };
            this.enhancer
                .failures
                .insert(key.clone(), (attempts, Instant::now() + backoff));
            attempts
        })
        .unwrap_or(0);
    if attempts >= MAX_AUTO_ENHANCE_FAILURES {
        let _ = store.enhancer_discard_pending(job).await;
        this.update(cx, |this, cx| {
            this.enhancer.failures.remove(&key);
            this.on_auto_enhance_skipped(
                session_id,
                "Could not generate the summary after repeated attempts.",
                None,
                cx,
            );
        })
        .ok();
    }
}

async fn generate_enhance_inner(
    store: &Arc<Store>,
    this: &WeakEntity<Workspace>,
    cx: &mut AsyncApp,
    args: &GenerateArgs,
    task_id: &str,
    run: u64,
) -> anyhow::Result<()> {
    // `transformArgs`
    let snapshot = load_snapshot(store, &args.session_id).await?;
    let (settings, _) = store.enhancer_settings().await??;
    let template_record = match &args.template_id {
        Some(id) => match store.enhancer_template(id.clone()).await? {
            Ok(record) => record,
            Err(error) => {
                tracing::error!(%error, "[enhance] failed to load template");
                None
            }
        },
        None => None,
    };
    let enhance_args = prompts::enhance_args(
        &snapshot,
        args.template_id.as_deref(),
        template_record.as_ref(),
        &settings,
    );
    // `collectEnhanceImageContext` over the memos for a model that takes images.
    let images = if crate::enhancer::images::model_supports_image_input(
        Some(&args.conn.provider_id),
        Some(&args.conn.model_id),
    ) {
        let session_dir = store.session_dir(&args.session_id);
        let contents: Vec<String> = std::iter::once(snapshot.raw_content.clone())
            .chain(snapshot.transcripts.iter().map(|t| t.memo.clone()))
            .collect();
        store
            .runtime()
            .spawn_blocking(move || {
                let refs: Vec<&str> = contents.iter().map(String::as_str).collect();
                crate::enhancer::images::collect_enhance_image_context(&session_dir, &refs)
            })
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let system = prompts::enhance_system_prompt(&enhance_args).map_err(anyhow::Error::msg)?;
    let prompt =
        prompts::enhance_user_prompt(&enhance_args, images.len()).map_err(anyhow::Error::msg)?;
    let validator = EnhanceValidator::new(
        enhance_args.template.as_ref(),
        !enhance_args.format_override.trim().is_empty(),
    );

    let mut progress = runner::run(
        store.runtime(),
        Generation {
            conn: args.conn.clone(),
            system,
            prompt,
            images: images
                .into_iter()
                .map(|image| crate::llm_stream::ImagePart {
                    base64: image.base64,
                    mime_type: image.mime_type,
                })
                .collect(),
            max_output_tokens: SUMMARY_MAX_OUTPUT_TOKENS,
            validator: Some(validator),
            normalize_bullets: true,
        },
    );
    let text = loop {
        let Some(event) = progress.recv().await else {
            anyhow::bail!("AI generation ended unexpectedly.");
        };
        let alive = this.update(cx, |this, cx| match &event {
            Progress::Step(step) => {
                let step = step.clone();
                this.set_task(task_id, run, |task| task.step = Some(step), cx)
            }
            Progress::Text(text) => {
                let blocks = crate::document::from_body("markdown", text);
                let text = text.clone();
                this.set_task(
                    task_id,
                    run,
                    |task| {
                        task.streamed_text = text;
                        task.blocks = blocks;
                    },
                    cx,
                )
            }
            Progress::Done(_) | Progress::Error(_) => this
                .enhance_task(task_id)
                .is_some_and(|task| task.run == run),
        })?;
        if !alive {
            // Reset or regenerated: this run no longer owns the task.
            anyhow::bail!("Aborted");
        }
        match event {
            Progress::Done(text) => break text,
            Progress::Error(message) => anyhow::bail!(message),
            _ => {}
        }
    };

    enhance_success(store, this, cx, args, &enhance_args, text, run, task_id).await
}

/// `runEnhanceSuccess`.
#[allow(clippy::too_many_arguments)]
async fn enhance_success(
    store: &Arc<Store>,
    this: &WeakEntity<Workspace>,
    cx: &mut AsyncApp,
    args: &GenerateArgs,
    enhance_args: &EnhanceArgs,
    text: String,
    run: u64,
    task_id: &str,
) -> anyhow::Result<()> {
    use crate::enhancer::summary_length::{
        SummaryLengthPolicy, constrain_summary_length, count_normalized_characters,
        summary_length_policy,
    };
    let policy: Option<SummaryLengthPolicy> = if enhance_args.has_template_sections() {
        None
    } else {
        summary_length_policy(&enhance_args.transcripts, enhance_args.summary_length)
    };
    let constrained = constrain_summary_length(&text, policy.as_ref());
    if constrained.is_empty() {
        return Ok(());
    }
    let template = enhance_args.template.as_ref();
    let mut sources: Vec<&str> = vec![
        constrained.as_str(),
        enhance_args.pre_meeting_memo.as_str(),
        enhance_args.post_meeting_memo.as_str(),
    ];
    if let Some(template) = template {
        sources.push(template.title.as_str());
        if let Some(description) = &template.description {
            sources.push(description);
        }
        for section in &template.sections {
            sources.push(section.title.as_str());
            if let Some(description) = &section.description {
                sources.push(description);
            }
        }
    }
    let tag_names = extract_tag_names(sources);
    let text_with_tags = append_tag_line(&constrained, &tag_names);

    let initial = load_snapshot(store, &args.session_id).await?;
    let mut trimmed_title = initial.title.trim().to_string();
    let mut generated_title = String::new();
    let mut persist_generated_title = false;
    let title_draft = this
        .read_with(cx, |this, cx| {
            this.has_live_title_draft(&args.session_id, cx)
        })
        .unwrap_or(false);
    if trimmed_title.is_empty() && !title_draft {
        let title_task = title_task_id(&args.session_id);
        let existing = this.read_with(cx, |this, _| this.enhance_task(&title_task).cloned())?;
        match existing {
            Some(task) if matches!(task.status, TaskStatus::Success | TaskStatus::Generating) => {
                generated_title = persistable_generated_title(&task.streamed_text);
            }
            _ => {
                generated_title = generate_title(
                    store,
                    this,
                    cx,
                    args,
                    &settings_of(store).await?,
                    &text_with_tags,
                )
                .await
                .unwrap_or_default();
            }
        }
        let alive = this
            .read_with(cx, |this, _| {
                this.enhance_task(task_id)
                    .is_some_and(|task| task.run == run)
            })
            .unwrap_or(false);
        if !alive {
            return Ok(());
        }
    }

    let snapshot = load_snapshot(store, &args.session_id).await?;
    let Some(note) = snapshot.enhanced_note(&args.note_id).cloned() else {
        anyhow::bail!("Summary {} no longer exists", args.note_id);
    };
    trimmed_title = snapshot.title.trim().to_string();
    if trimmed_title.is_empty() && !title_draft && !generated_title.is_empty() {
        trimmed_title = generated_title.clone();
        persist_generated_title = true;
    }
    let titled = ensure_markdown_first_line_title(&constrained, Some(&trimmed_title));
    let tag_line = append_tag_line("", &tag_names);
    let reserved = if tag_line.is_empty() {
        0
    } else {
        count_normalized_characters(&tag_line) + 1
    };
    let persistable_body = constrain_summary_length(
        &titled,
        policy
            .as_ref()
            .map(|policy| SummaryLengthPolicy {
                max_characters: policy.max_characters.saturating_sub(reserved),
                max_sections: None,
                ..policy.clone()
            })
            .as_ref(),
    );
    let alive = this
        .read_with(cx, |this, _| {
            this.enhance_task(task_id)
                .is_some_and(|task| task.run == run)
        })
        .unwrap_or(false);
    if !alive {
        return Ok(());
    }
    let persistable_text = append_tag_line(&persistable_body, &tag_names);
    let next_content = anlg_tiptap::md_to_tiptap_json(&persistable_text)
        .map_err(anyhow::Error::msg)?
        .to_string();
    let (current_content, current_format) = match &args.pending {
        Some(pending) => (
            pending.expected_body.clone(),
            pending.expected_content_format.clone(),
        ),
        None => (note.content.clone(), note.content_format.clone()),
    };
    store
        .enhancer_persist_note(
            args.session_id.clone(),
            snapshot.owner_user_id.clone(),
            DocumentUpdate {
                id: note.id.clone(),
                current_content,
                current_content_format: current_format,
                next_content,
            },
            tag_names,
            args.pending.clone(),
        )
        .await??;

    if persist_generated_title {
        persist_title(store, &args.session_id, &generated_title).await?;
    }
    tracing::info!(session_id = %args.session_id, "note.enhanced");
    // `dispatchEvent("note.enhanced")` + `runNoteEnhancedAutomations`.
    store.run_note_enhanced_automations(args.session_id.clone());
    // `showSummaryReadyNotification(sessionId, trimmedTitle)`,
    // `playCompletionSound` and `requestAppAttention`.
    let session_id = args.session_id.clone();
    this.update(cx, |this, cx| {
        this.notify_summary_ready(&session_id, Some(&trimmed_title));
        this.play_completion_sound(cx);
        this.request_app_attention();
    })
    .ok();
    Ok(())
}

async fn settings_of(store: &Arc<Store>) -> anyhow::Result<prompts::PromptSettings> {
    Ok(store.enhancer_settings().await??.0)
}

/// The `title` task: `startTask(titleTaskId, { taskType: "title",
/// skipPersist: true })`, streamed into its own task state.
async fn generate_title(
    store: &Arc<Store>,
    this: &WeakEntity<Workspace>,
    cx: &mut AsyncApp,
    args: &GenerateArgs,
    settings: &prompts::PromptSettings,
    enhanced_note: &str,
) -> anyhow::Result<String> {
    let task_id = title_task_id(&args.session_id);
    let run = this.update(cx, |this, cx| {
        this.enhancer.next_run += 1;
        let run = this.enhancer.next_run;
        this.enhancer.tasks.insert(
            task_id.clone(),
            TaskState {
                status: TaskStatus::Generating,
                streamed_text: String::new(),
                blocks: Vec::new(),
                error: None,
                step: None,
                run,
            },
        );
        cx.notify();
        run
    })?;
    let dictionary_terms = prompts::parse_dictionary_terms_json(&settings.dictionary_terms_json);
    let (system, prompt) = prompts::title_prompts(
        settings.ai_language.as_deref(),
        enhanced_note,
        &dictionary_terms,
    )
    .map_err(anyhow::Error::msg)?;
    let mut progress = runner::run(
        store.runtime(),
        Generation {
            conn: args.conn.clone(),
            system,
            prompt,
            images: Vec::new(),
            max_output_tokens: TITLE_MAX_OUTPUT_TOKENS,
            validator: None,
            normalize_bullets: false,
        },
    );
    let result: anyhow::Result<String> = loop {
        let Some(event) = progress.recv().await else {
            break Err(anyhow::anyhow!("AI generation ended unexpectedly."));
        };
        match event {
            Progress::Step(step) => {
                this.update(cx, |this, cx| {
                    this.set_task(&task_id, run, |task| task.step = Some(step), cx);
                })
                .ok();
            }
            Progress::Text(text) => {
                this.update(cx, |this, cx| {
                    this.set_task(&task_id, run, |task| task.streamed_text = text, cx);
                })
                .ok();
            }
            Progress::Done(text) => break Ok(text),
            Progress::Error(message) => break Err(anyhow::anyhow!(message)),
        }
    };
    this.update(cx, |this, cx| {
        this.set_task(
            &task_id,
            run,
            |task| match &result {
                Ok(text) => {
                    task.status = TaskStatus::Success;
                    task.streamed_text = text.clone();
                }
                Err(error) => {
                    task.status = TaskStatus::Error;
                    task.error = Some(error.to_string());
                }
            },
            cx,
        );
    })
    .ok();
    result.map(|text| persistable_generated_title(&text))
}

/// `persistGeneratedTitle`: the session title and each summary's heading.
async fn persist_title(store: &Arc<Store>, session_id: &str, text: &str) -> anyhow::Result<()> {
    let trimmed = persistable_generated_title(text);
    if trimmed.is_empty() {
        return Ok(());
    }
    let snapshot = load_snapshot(store, session_id).await?;
    if !snapshot.title.trim().is_empty() {
        return Ok(());
    }
    let documents: Vec<DocumentUpdate> = snapshot
        .enhanced_notes
        .iter()
        .filter(|note| !note.content.trim().is_empty())
        .filter_map(|note| {
            let parsed = if note.content_format == "markdown" {
                anlg_tiptap::md_to_tiptap_json(&note.content).ok()?
            } else {
                serde_json::from_str::<serde_json::Value>(&note.content).ok()?
            };
            Some(DocumentUpdate {
                id: note.id.clone(),
                current_content: note.content.clone(),
                current_content_format: note.content_format.clone(),
                next_content: crate::document::ensure_first_line_title(parsed, &trimmed)
                    .to_string(),
            })
        })
        .collect();
    store
        .enhancer_apply_generated_title(
            session_id.to_string(),
            snapshot.title.clone(),
            trimmed,
            documents,
        )
        .await??;
    Ok(())
}

impl Workspace {
    /// `playCompletionSound`: the chosen cuelume cue at 0.7 unless
    /// notifications or the completion sound are off.
    pub(crate) fn play_completion_sound(&mut self, cx: &mut Context<Self>) {
        let settings = &self.provider_settings;
        if settings.bool_setting(
            "notification_disabled",
            &["notification", "disabled"],
            false,
        ) || !settings.bool_setting(
            "notification_completion_sound",
            &["notification", "completion_sound"],
            true,
        ) {
            return;
        }
        let name = settings.string_setting(
            "notification_completion_sound_name",
            &["notification", "completion_sound_name"],
        );
        self.preview_completion_sound(
            crate::cuelume::normalize_completion_sound_name(name.as_deref()),
            cx,
        );
    }

    /// `previewCompletionSound(sound)`.
    pub(crate) fn preview_completion_sound(&mut self, name: &str, _cx: &mut Context<Self>) {
        self.completion_cue =
            crate::sfx::Sound::play_cue(name, crate::cuelume::COMPLETION_SOUND_VOLUME);
    }

    /// `hasLiveSessionTitleDraft`: the title field is being edited.
    fn has_live_title_draft(&self, session_id: &str, cx: &gpui::App) -> bool {
        self.selected.as_deref() == Some(session_id) && self.title_input.read(cx).is_dirty()
    }
}
