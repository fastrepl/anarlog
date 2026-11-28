//! The summary states of shipping's `Enhanced` view and its summary tab: Generate summary,
//! AI setup, streaming, generation errors, and the template picker with Regenerate.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use desktop_runtime::{CancellationToken, DocumentId, Result, RuntimeHandle, SessionId};
use gpui::{
    Animation, AnimationExt, AnyElement, Context, Focusable, FontWeight, HighlightStyle, Hsla,
    MouseButton, SharedString, StyledText, Window, div, prelude::*, px, rgb,
};
use serde_json::{Value, json};

use super::{NoteEvent, NoteView, icon, neutral};
use crate::ui::a11y::Accessible;
use crate::{
    contracts::{ProductRoute, WorkspaceEvent},
    meeting::{
        ai::StreamObserver, ai_view::AiViewServices, context, model::failure, widgets::spinner,
    },
    ui::{
        input::{InputEvent, TextInput},
        theme::Theme,
    },
    workspace::{
        resource_list::Glyph,
        templates::{Library, Template},
    },
};

/// Shipping's `MIN_WORDS_FOR_SUMMARY` and `MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY`.
const MIN_WORDS: usize = 5;
const MIN_CHARACTERS: usize = 160;

/// A summary generating (or failed) for one session.
pub(super) struct SummaryJob {
    /// The summary being replaced; `None` while generation creates one.
    pub(super) document: Option<DocumentId>,
    stream: Arc<Mutex<(String, String)>>,
    pub(super) text: String,
    pub(super) reasoning: bool,
    pub(super) error: Option<String>,
    cancellation: CancellationToken,
}

impl SummaryJob {
    pub(super) fn running(&self) -> bool {
        self.error.is_none()
    }

    pub(super) fn shows(&self, document: &str) -> bool {
        self.document
            .as_ref()
            .is_none_or(|target| target.0.as_ref() == document)
    }

    fn pull(&mut self) -> bool {
        let (text, reasoning) = self
            .stream
            .lock()
            .map(|stream| stream.clone())
            .unwrap_or_default();
        let reasoning = !reasoning.is_empty() && text.is_empty();
        let changed = text.len() != self.text.len() || reasoning != self.reasoning;
        self.text = text;
        self.reasoning = reasoning;
        changed
    }
}

impl Drop for SummaryJob {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

/// The language model chat and summaries use: whether one is set up, and whether it runs on
/// this device (shipping's warm-up tip).
#[derive(Clone, Copy, Default)]
pub(super) struct ModelState {
    pub(super) configured: bool,
    /// Anarlog's hosted model, which needs Pro (shipping's `not_pro`).
    pub(super) hosted: bool,
    pub(super) local: bool,
}

pub(super) fn load_model(runtime: &RuntimeHandle) -> Result<desktop_runtime::Reply<ModelState>> {
    let runtime_for_read = runtime.clone();
    runtime.service(move |_| async move {
        let settings = crate::meeting::config::preferences(&runtime_for_read).await?;
        let provider = settings.text("current_llm_provider");
        let hosted = matches!(provider.as_str(), "anarlog" | "hyprnote");
        Ok(ModelState {
            configured: settings.value("intelligence_disabled") != &Value::Bool(true)
                && !provider.is_empty()
                && !settings.text("current_llm_model").is_empty(),
            hosted,
            local: matches!(
                provider.as_str(),
                "apple_foundation" | "lmstudio" | "ollama" | "unsloth"
            ),
        })
    })
}

/// Shipping's `getEligibility`: enough transcript to summarize.
fn eligible(words: usize, characters: usize) -> std::result::Result<(), &'static str> {
    if words == 0 {
        Err("No transcript recorded")
    } else if words < MIN_WORDS || characters < MIN_CHARACTERS {
        Err("Not enough transcript recorded to summarize.")
    } else {
        Ok(())
    }
}

/// The session's transcripts, their non-empty words and the words' characters.
async fn transcript_counts(
    runtime: &RuntimeHandle,
    session: SessionId,
) -> Result<(usize, usize, usize)> {
    let rows = runtime
        .submit(move |services| async move {
            services
                .executor
                .execute(
                    "SELECT words_json FROM transcripts WHERE session_id = ? AND deleted_at IS NULL"
                        .into(),
                    vec![json!(session)],
                )
                .await
                .map_err(failure)
        })?
        .receive()
        .await?;
    let words: Vec<String> = rows
        .iter()
        .filter_map(|row| serde_json::from_str::<Vec<Value>>(row["words_json"].as_str()?).ok())
        .flatten()
        .filter_map(|word| word["text"].as_str().map(str::to_owned))
        .filter(|text| !text.is_empty())
        .collect();
    let joined = words.join(" ");
    let characters = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok((rows.len(), words.len(), characters.chars().count()))
}

async fn check_eligible(runtime: &RuntimeHandle, session: SessionId) -> Result<()> {
    let (_, words, characters) = transcript_counts(runtime, session).await?;
    eligible(words, characters).map_err(failure)
}

/// What an uploaded or imported transcript leads to.
#[derive(Debug, PartialEq)]
pub(super) enum AutoSummary {
    Generate,
    Skip,
    /// Shipping's "Summary wasn't generated" warning, with its reason.
    TooShort(String),
}

/// Shipping's enhancer after a transcript lands: `queueAutoEnhanceIfSummaryEmpty` after audio
/// (`if_empty`) and `enhance` after a subtitle import. Without a usable model, or (`if_empty`)
/// with a summary that has content, nothing is generated.
pub(super) async fn auto_summary(
    runtime: &RuntimeHandle,
    session: SessionId,
    if_empty: bool,
    pro: bool,
) -> Result<AutoSummary> {
    let model = load_model(runtime)?.receive().await?;
    if !model.configured || (model.hosted && !pro) {
        return Ok(AutoSummary::Skip);
    }
    if if_empty {
        let id = session.clone();
        let rows = runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute(
                        "SELECT s.title, d.body FROM session_documents d JOIN sessions s ON s.id = d.session_id
                         WHERE d.session_id = ? AND d.kind IN ('summary', 'template_output') AND d.deleted_at IS NULL"
                            .into(),
                        vec![json!(id)],
                    )
                    .await
                    .map_err(failure)
            })?
            .receive()
            .await?;
        let text = |row: &Value, key: &str| row[key].as_str().unwrap_or_default().to_owned();
        if rows
            .iter()
            .any(|row| super::summary_has_content(&text(row, "body"), &text(row, "title")))
        {
            return Ok(AutoSummary::Skip);
        }
    }
    let (transcripts, words, characters) = transcript_counts(runtime, session).await?;
    // Shipping's `getSummaryEligibility` reasons.
    Ok(if transcripts == 0 || words == 0 {
        AutoSummary::Skip
    } else if words < MIN_WORDS {
        AutoSummary::TooShort(format!(
            "Not enough words recorded ({words}/{MIN_WORDS} minimum)"
        ))
    } else if characters < MIN_CHARACTERS {
        AutoSummary::TooShort(format!(
            "Transcript too short to summarize ({characters}/{MIN_CHARACTERS} characters minimum)"
        ))
    } else {
        AutoSummary::Generate
    })
}

/// Shipping's `getEnhancedNoteTitle`: the template's title, else a meaningful stored title.
pub(super) fn tab_title(raw: &str, template: Option<&Template>, template_id: &str) -> SharedString {
    if let Some(title) = template
        .map(|template| template.title.trim())
        .filter(|title| !title.is_empty())
    {
        return title.to_owned().into();
    }
    let title = raw.trim();
    let generated = title.is_empty()
        || title == "Summary"
        || title == template_id
        || uuid::Uuid::parse_str(title).is_ok()
        || chrono::DateTime::parse_from_rfc3339(title).is_ok();
    if generated {
        "Summary".into()
    } else {
        title.to_owned().into()
    }
}

/// One template picker row: Auto (`None`) or a template.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PickerItem {
    pub(super) id: Option<Arc<str>>,
    pub(super) title: Arc<str>,
    pub(super) glyph: Glyph,
    pub(super) favorite: bool,
}

/// Shipping's picker order: Auto, favorites by pin order, then the rest by title; a search
/// filters templates by title, description, category and tags.
pub(super) fn picker_items(library: &Library, query: &str) -> Vec<PickerItem> {
    let query = query.trim().to_lowercase();
    let matches = |template: &Template| {
        query.is_empty()
            || [&template.title, &template.description, &template.category]
                .iter()
                .any(|text| text.to_lowercase().contains(&query))
            || template
                .targets
                .iter()
                .any(|target| target.to_lowercase().contains(&query))
    };
    let item = |template: &Template| PickerItem {
        id: Some(template.id.clone()),
        title: if template.title.is_empty() {
            "Untitled".into()
        } else {
            template.title.clone()
        },
        glyph: template.icon.clone(),
        favorite: template.pinned,
    };
    let mut favorites: Vec<&Template> = library
        .templates
        .iter()
        .filter(|template| template.pinned && matches(template))
        .collect();
    favorites.sort_by(|a, b| {
        a.pin_order
            .unwrap_or(i64::MAX)
            .cmp(&b.pin_order.unwrap_or(i64::MAX))
            .then_with(|| a.title.cmp(&b.title))
    });
    let mut others: Vec<PickerItem> = library
        .templates
        .iter()
        .filter(|template| !template.pinned && matches(template))
        .map(item)
        .collect();
    others.sort_by_key(|item| item.title.to_lowercase());
    std::iter::once(PickerItem {
        id: None,
        title: "Auto".into(),
        glyph: Glyph::Icon {
            key: "sparkles".into(),
            color: 0x9ca3af,
        },
        favorite: false,
    })
    .chain(favorites.into_iter().map(item))
    .chain(others)
    .collect()
}

/// The template picker open under the active summary tab.
pub(super) struct TemplatePicker {
    pub(super) search: gpui::Entity<TextInput>,
    _subscription: gpui::Subscription,
}

/// Markdown blocks as the summary streams, set like the note's typography.
fn markdown_blocks(text: &str) -> Vec<(u8, usize, String)> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let indent = line.len() - line.trim_start().len();
            let line = line.trim();
            for (prefix, level) in [("### ", 3), ("## ", 2), ("# ", 1)] {
                if let Some(rest) = line.strip_prefix(prefix) {
                    return (level, 0, rest.to_owned());
                }
            }
            for prefix in ["- ", "* ", "+ "] {
                if let Some(rest) = line.strip_prefix(prefix) {
                    return (0, indent / 2 + 1, rest.to_owned());
                }
            }
            (0, 0, line.to_owned())
        })
        .collect()
}

/// `**bold**` runs, set semibold like the editor's bold mark.
fn inline_bold(text: &str) -> StyledText {
    let mut plain = String::new();
    let mut highlights = Vec::new();
    for (index, part) in text.split("**").enumerate() {
        let start = plain.len();
        plain.push_str(part);
        if index % 2 == 1 && !part.is_empty() {
            highlights.push((
                start..plain.len(),
                HighlightStyle {
                    font_weight: Some(FontWeight::SEMIBOLD),
                    ..Default::default()
                },
            ));
        }
    }
    StyledText::new(plain).with_highlights(highlights)
}

fn pulse(element: impl IntoElement + 'static, id: &'static str) -> impl IntoElement {
    div().child(element).with_animation(
        id,
        Animation::new(Duration::from_secs(2)).repeat(),
        |view, delta| view.opacity(0.5 + 0.5 * (delta * std::f32::consts::TAU).cos().abs()),
    )
}

/// A shadcn `Button`.
fn button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    small: bool,
    colors: Theme,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .gap_2()
        .font_weight(crate::ui::theme::MEDIUM)
        .cursor_pointer()
        .a11y_button(label)
        .map(|button| {
            if small {
                button.h_7().px_2().text_xs().line_height(px(16.))
            } else {
                button.h_9().px_4().text_sm().line_height(px(20.))
            }
        })
        .map(|button| {
            if primary {
                button
                    .bg(colors.primary)
                    .text_color(colors.primary_foreground)
                    .shadow_xs()
                    .hover(move |style| style.bg(colors.primary.opacity(0.9)))
            } else {
                button
                    .border_1()
                    .border_color(colors.input)
                    .bg(colors.background)
                    .text_color(colors.foreground)
                    .shadow_xs()
                    .hover(move |style| style.bg(colors.accent))
            }
        })
        .child(label)
}

impl NoteView {
    /// Shipping's enhancer `enhance`: generates into `target` (or a new summary) with
    /// `template` (`Some(None)` is Auto; `None` keeps shipping's template order).
    pub(super) fn generate_summary(
        &mut self,
        target: Option<DocumentId>,
        template: Option<Option<String>>,
        cx: &mut Context<Self>,
    ) {
        if let Some(session) = self
            .current
            .as_ref()
            .map(|session| session.summary.id.clone())
        {
            self.generate_summary_for(session, target, template, cx);
        }
    }

    fn generate_summary_for(
        &mut self,
        session: SessionId,
        target: Option<DocumentId>,
        template: Option<Option<String>>,
        cx: &mut Context<Self>,
    ) {
        self.start_summary_generation(session, target, template, false, cx);
    }

    fn start_summary_generation(
        &mut self,
        session: SessionId,
        target: Option<DocumentId>,
        template: Option<Option<String>>,
        is_auto: bool,
        cx: &mut Context<Self>,
    ) {
        if self.jobs.get(&session).is_some_and(SummaryJob::running) {
            return;
        }
        // A clean editor of the summary being replaced reopens on the generated revision.
        if let Some(target) = &target
            && self
                .summary_editors
                .get(target)
                .is_some_and(|summary| !summary.editor.read(cx).is_dirty())
        {
            self.summary_editors.remove(target);
        }
        let stream = Arc::new(Mutex::new((String::new(), String::new())));
        let cancellation = CancellationToken::new();
        let mut job = SummaryJob {
            document: target.clone(),
            stream: stream.clone(),
            text: String::new(),
            reasoning: false,
            error: None,
            cancellation: cancellation.clone(),
        };
        let Some(services) = cx.try_global::<AiViewServices>().cloned() else {
            job.error = Some("Summary generation is not ready yet.".into());
            self.jobs.insert(session, job);
            cx.notify();
            return;
        };
        let observer: StreamObserver = Arc::new(move |update| {
            if let Ok(mut stream) = stream.lock() {
                stream.0.push_str(&update.text);
                stream.1.push_str(&update.reasoning);
            }
        });
        let runtime = self.runtime.clone();
        let ai = services.ai.clone();
        let id = session.clone();
        let queue = services.ai.enqueue(Box::pin(async move {
            check_eligible(&runtime, id.clone()).await?;
            let (base, context) = context::load_summary(&runtime, id, target, template).await?;
            let used_template = context.user.template.is_some();
            let prefs = crate::meeting::config::preferences(&runtime).await.ok();
            let provider = prefs
                .as_ref()
                .map(|p| p.text("current_llm_provider"))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "unknown".to_owned());
            let model = prefs
                .as_ref()
                .map(|p| p.text("current_llm_model"))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "unknown".to_owned());
            let result = ai.summarize(base, context, cancellation, observer).await;
            crate::meeting::ai::record_enhancement_result(
                is_auto,
                &provider,
                &model,
                used_template,
                &result,
            );
            result
        }));
        self.jobs.insert(session.clone(), job);
        self.poll_jobs(cx);
        cx.spawn(async move |this, cx| {
            let result = match queue {
                Ok(reply) => reply.await.map_err(failure).and_then(|result| result),
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        this.jobs.remove(&session);
                        let title = this
                            .current
                            .as_ref()
                            .filter(|current| current.summary.id == session)
                            .map(|current| current.summary.title.to_string())
                            .unwrap_or_default();
                        crate::meeting::automation::summary_ready(&session, &title, cx);
                    }
                    Err(desktop_runtime::ServiceError::Cancelled) => {
                        this.jobs.remove(&session);
                    }
                    Err(error) => {
                        if let Some(job) = this.jobs.get_mut(&session) {
                            job.error = Some(error.to_string());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Shipping's summary after an upload, re-transcription or transcript import: the view
    /// showing the session takes it, else the main window's; it switches to the summary
    /// unless the transcript is shown.
    pub(super) fn transcribed(&mut self, session: SessionId, cx: &mut Context<Self>) {
        if self.session_id() == Some(&session) {
            self.claim_summary(session, cx);
        } else if !self.standalone {
            let this = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = this.update(cx, |this, cx| this.claim_summary(session, cx));
            });
        }
    }

    fn claim_summary(&mut self, session: SessionId, cx: &mut Context<Self>) {
        let uploads = crate::meeting::upload::Uploads::global(cx);
        let Some(if_empty) = uploads.update(cx, |uploads, _| uploads.claim_summary(&session))
        else {
            return;
        };
        let runtime = self.runtime.clone();
        let pro = crate::product::account::is_pro(cx);
        cx.spawn(async move |this, cx| {
            let decision = auto_summary(&runtime, session.clone(), if_empty, pro).await;
            let _ = this.update(cx, |this, cx| match decision {
                Ok(AutoSummary::Generate) => {
                    this.start_summary_generation(session.clone(), None, None, true, cx);
                    if this.session_id() == Some(&session)
                        && this.view != Some(super::NoteTab::Transcript)
                    {
                        this.view = None;
                    }
                    cx.notify();
                }
                Ok(AutoSummary::TooShort(reason)) => crate::application::toasts::show(
                    crate::application::toasts::Toast::warning("Summary wasn't generated")
                        .id(format!("auto-summary-too-short-{}", session.0))
                        .description(reason),
                    cx,
                ),
                Ok(AutoSummary::Skip) | Err(_) => {}
            });
        })
        .detach();
    }

    /// Copies streamed text into running jobs at about 30 fps while any runs.
    fn poll_jobs(&mut self, cx: &mut Context<Self>) {
        if self.job_poll.is_some() {
            return;
        }
        self.job_poll = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let running = this.update(cx, |this, cx| {
                    let mut changed = false;
                    for job in this.jobs.values_mut().filter(|job| job.running()) {
                        changed |= job.pull();
                    }
                    if changed {
                        cx.notify();
                    }
                    let running = this.jobs.values().any(SummaryJob::running);
                    if !running {
                        this.job_poll = None;
                    }
                    running
                });
                if !matches!(running, Ok(true)) {
                    break;
                }
            }
        }));
    }

    pub(super) fn open_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = cx.new(|cx| TextInput::new("Search templates...", cx).inline());
        let subscription = cx.subscribe(&search, |this, _, event, cx| match event {
            InputEvent::Submitted => {
                let query = this.picker_query(cx);
                if let Some(item) = picker_items(&this.templates, &query).into_iter().next() {
                    this.choose_template(item.id, cx);
                }
            }
            _ => cx.notify(),
        });
        search.focus_handle(cx).focus(window, cx);
        self.picker = Some(TemplatePicker {
            search,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn picker_query(&self, cx: &gpui::App) -> String {
        self.picker
            .as_ref()
            .map(|picker| picker.search.read(cx).buffer.text.clone())
            .unwrap_or_default()
    }

    fn choose_template(&mut self, template: Option<Arc<str>>, cx: &mut Context<Self>) {
        self.picker = None;
        if let Some(NoteTabTarget(document)) = self.active_summary_document() {
            self.generate_summary(document, Some(template.map(|id| id.to_string())), cx);
        }
        cx.notify();
    }

    /// The summary document behind the active tab; `None` inside for the not-yet-created one.
    fn active_summary_document(&self) -> Option<NoteTabTarget> {
        let tabs = self.tabs();
        match super::current_tab(
            self.view.as_ref(),
            self.mode == crate::meeting::live::SessionMode::Active,
            &tabs,
        ) {
            super::NoteTab::Summary(id) => Some(NoteTabTarget(
                self.details
                    .summaries
                    .iter()
                    .any(|summary| summary.id.0 == id)
                    .then_some(DocumentId(id)),
            )),
            _ => None,
        }
    }

    /// Regenerates the active summary with the template it used (shipping's `onRegenerate`).
    pub(super) fn regenerate(&mut self, cx: &mut Context<Self>) {
        self.picker = None;
        if let Some(NoteTabTarget(document)) = self.active_summary_document() {
            self.generate_summary(document, None, cx);
        }
    }

    /// The summary body for a tab without an editor: generating, failed, set-up or empty.
    pub(super) fn summary_state(
        &mut self,
        document: Option<&DocumentId>,
        has_content: bool,
        title: &str,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let session = self.current.as_ref()?.summary.id.clone();
        let job = self
            .jobs
            .get(&session)
            .filter(|job| document.is_none_or(|document| job.shows(&document.0)));
        if let Some(job) = job {
            return Some(match &job.error {
                Some(error) => self.enhance_error(error.clone(), colors, cx),
                None => self.streaming(job, title, colors),
            });
        }
        // Shipping's `shouldShowEmptySummaryConfigError`.
        let pro = crate::product::account::is_pro(cx);
        let config_error = self
            .model
            .is_some_and(|model| !model.configured || (model.hosted && !pro));
        match document {
            None if config_error => Some(self.config_error(colors, cx)),
            None => Some(self.generate_screen(colors, cx)),
            Some(_) if config_error && !has_content => Some(self.config_error(colors, cx)),
            Some(_) => None,
        }
    }

    /// Shipping's `EmptySummary`.
    fn generate_screen(&self, colors: Theme, cx: &mut Context<Self>) -> AnyElement {
        centered()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .px_6()
            .child(
                button("generate-summary", "Generate summary", true, false, colors)
                    .on_click(cx.listener(|this, _, _, cx| this.generate_summary(None, None, cx))),
            )
            .into_any_element()
    }

    /// Shipping's `ConfigError`.
    fn config_error(&self, colors: Theme, cx: &mut Context<Self>) -> AnyElement {
        centered()
            // `min-h-[400px]` inside the padded scroll box.
            .min_h(px(504.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .px_6()
            .child(
                div()
                    .mb_6()
                    .max_w(px(448.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .text_center()
                    .child(
                        div()
                            .id("summary-setup-title")
                            .a11y_heading("Set up AI summaries", 2)
                            .text_base()
                            .line_height(px(24.))
                            .font_weight(crate::ui::theme::MEDIUM)
                            .text_color(colors.foreground)
                            .child("Set up AI summaries"),
                    )
                    .child(
                        div()
                            .id("summary-setup-description")
                            .a11y_text("Start a Pro trial or add your own LLM API key to generate a summary from this transcript.")
                            .text_sm()
                            .line_height(px(22.75))
                            .text_color(colors.muted_foreground)
                            .child("Start a Pro trial or add your own LLM API key to generate a summary from this transcript."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        button("summary-get-pro", "Get Pro", true, false, colors)
                            .shadow_none()
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(WorkspaceEvent::Product(ProductRoute::Billing))
                            })),
                    )
                    .child(
                        button("summary-api-key", "Add API key", false, false, colors)
                            .shadow_none()
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(WorkspaceEvent::Product(ProductRoute::Intelligence))
                            })),
                    ),
            )
            .into_any_element()
    }

    /// Shipping's `EnhanceError`.
    fn enhance_error(&self, error: String, colors: Theme, cx: &mut Context<Self>) -> AnyElement {
        centered()
            // `min-h-[400px]` inside the padded scroll box.
            .min_h(px(504.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .px_6()
            .text_center()
            .child(div().mb_5().child(icon(
                "editor/AlertCircleIcon.svg",
                36.,
                colors.muted_foreground,
            )))
            .child(
                div()
                    .mb_6()
                    .max_w(px(448.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .id("summary-failed")
                            .a11y_status("Summary generation failed", true)
                            .aria_description(error.clone())
                            .text_base()
                            .line_height(px(24.))
                            .font_weight(crate::ui::theme::MEDIUM)
                            .text_color(colors.foreground)
                            .child("Summary generation failed"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(22.75))
                            .text_color(colors.muted_foreground)
                            .child(error),
                    ),
            )
            .child(
                button("summary-retry", "Retry", true, true, colors)
                    .child(icon(
                        "editor/RefreshIcon.svg",
                        16.,
                        colors.primary_foreground,
                    ))
                    .flex_row_reverse()
                    .on_click(cx.listener(|this, _, _, cx| {
                        let session = this
                            .current
                            .as_ref()
                            .map(|session| session.summary.id.clone());
                        let target = session
                            .as_ref()
                            .and_then(|session| this.jobs.remove(session))
                            .and_then(|job| job.document.clone());
                        this.generate_summary(target, None, cx);
                    })),
            )
            .into_any_element()
    }

    /// Shipping's `StreamingView`.
    fn streaming(&self, job: &SummaryJob, title: &str, colors: Theme) -> AnyElement {
        let local = self.model.is_some_and(|model| model.local);
        let frame = div().px_3().pt_2().pb_2();
        if job.text.trim().is_empty() {
            let (status, tip) = if job.reasoning {
                (
                    "Model is thinking...",
                    "Reasoning models think through the transcript before writing.",
                )
            } else if local {
                (
                    "Analyzing structure...",
                    "On-device models can take a few minutes to warm up before text appears.",
                )
            } else {
                (
                    "Analyzing structure...",
                    "Tip: The Anarlog team loves our users!",
                )
            };
            return frame
                .flex()
                .flex_col()
                .gap(px(2.))
                .text_sm()
                .text_color(colors.muted_foreground)
                .child(pulse(
                    div().line_height(px(20.)).child(status),
                    "summary-status",
                ))
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(6.))
                        .pl_4()
                        .text_xs()
                        .line_height(px(20.))
                        .child(
                            div()
                                .mt(px(5.))
                                .size_2()
                                .flex_shrink_0()
                                .border_b_1()
                                .border_l_1()
                                .border_color(colors.muted_foreground.opacity(0.6)),
                        )
                        .child(tip),
                )
                .into_any_element();
        }
        frame
            .child(
                div()
                    .mb_4()
                    .min_h(px(30.))
                    .text_size(px(24.))
                    .line_height(px(30.))
                    .font_weight(FontWeight::BOLD)
                    .map(|view| {
                        if title.trim().is_empty() {
                            view.child(pulse(
                                div()
                                    .text_color(colors.muted_foreground.opacity(0.6))
                                    .child("Generating title..."),
                                "summary-title",
                            ))
                        } else {
                            view.text_color(colors.foreground)
                                .child(SharedString::from(title.trim().to_owned()))
                        }
                    }),
            )
            .children(
                markdown_blocks(&job.text)
                    .into_iter()
                    .map(|(level, depth, text)| {
                        let (size, line, weight) = match level {
                            1 => (20., 28., FontWeight::BOLD),
                            2 => (18., 26., FontWeight::SEMIBOLD),
                            3 => (16., 24., FontWeight::SEMIBOLD),
                            _ => (16., 24., FontWeight::NORMAL),
                        };
                        div()
                            .relative()
                            .py(px(size * 0.125))
                            .pl(px(depth as f32 * 24.))
                            .text_size(px(size))
                            .line_height(px(line))
                            .font_weight(weight)
                            .text_color(colors.foreground)
                            .when(depth > 0, |view| {
                                view.child(
                                    div()
                                        .absolute()
                                        .left(px(depth as f32 * 24. - 16.))
                                        .top(px(size * 0.125 + 10.))
                                        .size(px(6.))
                                        .bg(colors.foreground.opacity(0.65)),
                                )
                            })
                            .child(inline_bold(&text))
                    }),
            )
            .into_any_element()
    }

    /// Shipping's `TemplatePickerPopover`, under the active summary tab.
    pub(super) fn template_picker(
        &self,
        used: Option<Arc<str>>,
        generating: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let picker = self.picker.as_ref()?;
        let query = picker.search.read(cx).buffer.text.clone();
        let items = picker_items(&self.templates, &query);
        let searching = !query.trim().is_empty();
        let (auto, templates) = items.split_at(1);
        let row = |item: &PickerItem, index: usize, cx: &mut Context<Self>| {
            let id = item.id.clone();
            let is_used = item.id == used;
            div()
                .id(("summary-template", index))
                .a11y_menu_check(item.title.to_string(), is_used)
                .h_8()
                .flex_shrink_0()
                .w_full()
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .hover(move |style| style.bg(colors.accent))
                .on_click(cx.listener(move |this, _, _, cx| this.choose_template(id.clone(), cx)))
                .child(item.glyph.render(16.))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .line_height(px(20.))
                        .font_weight(crate::ui::theme::MEDIUM)
                        .text_color(colors.foreground)
                        .child(SharedString::from(item.title.to_string())),
                )
                .when(item.favorite, |row| {
                    row.child(icon("editor/HeartIcon.svg", 14., rgb(0xf43f5e).into()))
                })
                .child(div().flex_1())
                .when(is_used, |row| {
                    row.child(
                        div()
                            .id("summary-regenerate")
                            .a11y_button("Regenerate")
                            .a11y_disabled(generating)
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_1()
                            .px(px(6.))
                            .py(px(2.))
                            .text_size(px(11.))
                            .line_height(px(16.5))
                            .font_weight(crate::ui::theme::MEDIUM)
                            .text_color(colors.muted_foreground)
                            .when_else(
                                generating,
                                |button| button.opacity(0.7).cursor_not_allowed(),
                                |button| {
                                    button
                                        .cursor_pointer()
                                        .hover(move |style| style.text_color(colors.foreground))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.regenerate(cx)
                                        }))
                                },
                            )
                            .child(icon(
                                "editor/RotateClockwiseIcon.svg",
                                12.,
                                colors.muted_foreground,
                            ))
                            .child("Regenerate"),
                    )
                })
                .into_any_element()
        };
        let mono = crate::ui::theme::monospace_font(cx);
        let section_header = |label: &'static str| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .child(icon("editor/PlusIcon.svg", 14., rgb(0x3b82f6).into()))
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_size(px(11.))
                        .line_height(px(16.5))
                        .font_weight(crate::ui::theme::MEDIUM)
                        .text_color(colors.muted_foreground)
                        .child(label),
                )
        };
        let mut rows: Vec<AnyElement> = vec![row(&auto[0], 0, cx)];
        if searching {
            rows.push(section_header("Create new template").into_any_element());
            let title: SharedString = query.trim().to_owned().into();
            rows.push(
                div()
                    .id("summary-template-create")
                    .a11y_menu_item(format!("Create new template {title}"))
                    .h_8()
                    .flex_shrink_0()
                    .w_full()
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .hover(move |style| style.bg(colors.accent))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.picker = None;
                        cx.emit(NoteEvent::OpenTemplates);
                    }))
                    .child(Glyph::template_default().render(16.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .font_weight(crate::ui::theme::MEDIUM)
                            .text_color(colors.foreground)
                            .child(title),
                    )
                    .into_any_element(),
            );
        }
        if templates.is_empty() && !searching {
            rows.push(
                div()
                    .id("summary-no-templates")
                    .a11y_text("No templates yet")
                    .px_2()
                    .py_3()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("No templates yet")
                    .into_any_element(),
            );
        }
        for (index, item) in templates.iter().enumerate() {
            rows.push(row(item, index + 1, cx));
        }
        let has_search = !query.is_empty();
        Some(
            div()
                .id("summary-template-picker")
                .a11y_group(gpui::Role::Menu, "Templates")
                .w(px(320.))
                .occlude()
                .p(px(2.))
                .pb_0()
                .border_1()
                .border_color(colors.app_floating_border)
                .bg(colors.app_floating_chrome)
                .shadow_lg()
                .text_color(colors.popover_foreground)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.picker = None;
                    cx.notify();
                }))
                .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        this.picker = None;
                        cx.stop_propagation();
                        cx.notify();
                    }
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .border_1()
                        .border_color(colors.app_floating_border)
                        .bg(colors.app_floating_panel)
                        .child(
                            div().border_b_1().border_color(colors.border).py_1().child(
                                div()
                                    .h_8()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px(px(10.))
                                    .child(icon(
                                        "editor/Search01Icon.svg",
                                        16.,
                                        colors.muted_foreground,
                                    ))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_sm()
                                            .child(picker.search.clone()),
                                    )
                                    .when(has_search, |view| {
                                        view.child(
                                            div()
                                                .id("summary-template-clear")
                                                .a11y_button("Clear search")
                                                .p(px(2.))
                                                .cursor_pointer()
                                                .hover(move |style| style.bg(colors.accent))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    if let Some(picker) = &this.picker {
                                                        picker.search.update(cx, |search, cx| {
                                                            search.set_text(String::new(), cx)
                                                        });
                                                    }
                                                }))
                                                .child(icon(
                                                    "editor/XIcon.svg",
                                                    12.,
                                                    colors.muted_foreground,
                                                )),
                                        )
                                    }),
                            ),
                        )
                        .child(
                            div()
                                .id("summary-template-results")
                                .max_h(px(320.))
                                .overflow_y_scroll()
                                .p_1()
                                .flex()
                                .flex_col()
                                .children(rows),
                        ),
                )
                .child(
                    div()
                        .id("summary-see-all")
                        .a11y_button("See all templates")
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .px_3()
                        .py(px(6.))
                        .text_xs()
                        .line_height(px(16.))
                        .font_weight(crate::ui::theme::MEDIUM)
                        .text_color(colors.muted_foreground)
                        .cursor_pointer()
                        .hover(move |style| style.bg(colors.accent).text_color(colors.foreground))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.picker = None;
                            cx.emit(NoteEvent::OpenTemplates);
                        }))
                        .child("See all templates")
                        .child(icon(
                            "editor/ArrowRight01Icon.svg",
                            14.,
                            colors.muted_foreground,
                        )),
                )
                .into_any_element(),
        )
    }

    /// The summary tab's leading glyph: a spinner while generating, else the template's icon.
    pub(super) fn summary_glyph(
        &self,
        template: Option<&Template>,
        generating: bool,
        color: Hsla,
    ) -> AnyElement {
        if generating {
            spinner("summary-tab-generating", 16., color).into_any_element()
        } else if let Some(template) = template {
            template.icon.render(16.)
        } else {
            icon("editor/SparkleIcon.svg", 16., color).into_any_element()
        }
    }

    pub(super) fn error_color(&self, colors: Theme) -> Hsla {
        neutral(colors, 0xdc2626, 0xf87171)
    }
}

struct NoteTabTarget(Option<DocumentId>);

/// A screen centered in the note's scroll box, which pads `pt-2 pb-24`.
fn centered() -> gpui::Div {
    div().size_full().pt_2().pb(px(96.))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(id: &str, title: &str, pinned: bool, pin_order: Option<i64>) -> Template {
        Template {
            id: id.into(),
            title: title.into(),
            description: "".into(),
            pinned,
            pin_order,
            category: "".into(),
            icon: Glyph::template_default(),
            sections: Vec::new(),
            targets: vec!["Sales".into()],
            warnings: Vec::new(),
        }
    }

    #[test]
    fn picker_lists_auto_then_favorites_then_templates_by_title() {
        let library = Library {
            templates: vec![
                template("b", "beta", false, None),
                template("f2", "Zed", true, Some(2)),
                template("a", "Alpha", false, None),
                template("f1", "Yak", true, Some(1)),
            ],
            ..Library::default()
        };
        let ids: Vec<Option<String>> = picker_items(&library, "")
            .into_iter()
            .map(|item| item.id.map(|id| id.to_string()))
            .collect();
        let expected = [None, Some("f1"), Some("f2"), Some("a"), Some("b")];
        assert_eq!(ids, expected.map(|id| id.map(str::to_owned)));
        assert_eq!(picker_items(&library, "sales").len(), 5, "tags match too");
        assert_eq!(picker_items(&library, "alp").len(), 2);
    }

    #[test]
    fn summary_tab_titles_hide_generated_names() {
        assert_eq!(tab_title("Summary", None, "").as_ref(), "Summary");
        assert_eq!(tab_title("", None, "").as_ref(), "Summary");
        assert_eq!(tab_title("t-1", None, "t-1").as_ref(), "Summary");
        assert_eq!(tab_title("Board recap", None, "").as_ref(), "Board recap");
        let sync = template("t-1", "Weekly team sync", true, Some(1));
        assert_eq!(
            tab_title("Summary", Some(&sync), "t-1").as_ref(),
            "Weekly team sync"
        );
    }

    #[test]
    fn summaries_need_shipping_minimum_transcript() {
        assert!(eligible(0, 0).is_err());
        assert!(eligible(4, 400).is_err());
        assert!(eligible(40, 159).is_err());
        assert!(eligible(40, 160).is_ok());
    }

    #[tokio::test]
    async fn transcripts_from_files_summarize_like_shipping_enhancer() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = desktop_runtime::RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        let sql = |sql: &'static str, params: Vec<Value>| {
            let runtime = runtime.clone();
            async move {
                runtime
                    .submit(move |services| async move {
                        services
                            .executor
                            .execute(sql.into(), params)
                            .await
                            .map_err(failure)
                    })
                    .unwrap()
                    .receive()
                    .await
                    .unwrap()
            }
        };
        let words = |text: &str| {
            json!(
                text.split(' ')
                    .map(|word| json!({"text": word}))
                    .collect::<Vec<_>>()
            )
            .to_string()
        };
        sql(
            "INSERT INTO sessions(id,title) VALUES ('s','Sync'),('short','Short')",
            vec![],
        )
        .await;
        sql(
            "INSERT INTO transcripts(id,session_id,words_json) VALUES ('t','s',?),('u','short',?)",
            vec![
                json!(words(
                    &"We agreed to ship the pricing page next week after legal review. ".repeat(4)
                )),
                json!(words("Hello there everyone")),
            ],
        )
        .await;
        let session = SessionId("s".into());
        let decide =
            |session: SessionId, if_empty: bool| auto_summary(&runtime, session, if_empty, false);
        // Without a language model nothing is generated.
        assert_eq!(
            decide(session.clone(), true).await.unwrap(),
            AutoSummary::Skip
        );
        sql("INSERT INTO app_settings(id,value_json) VALUES ('current_llm_provider','\"openai\"'),('current_llm_model','\"gpt-5\"')", vec![]).await;
        assert_eq!(
            decide(session.clone(), true).await.unwrap(),
            AutoSummary::Generate
        );
        assert_eq!(
            decide(SessionId("short".into()), true).await.unwrap(),
            AutoSummary::TooShort("Not enough words recorded (3/5 minimum)".into())
        );
        // A summary with content stays after audio, while an imported transcript regenerates.
        sql("INSERT INTO session_documents(id,session_id,kind,body) VALUES ('d','s','summary','Shipped')", vec![]).await;
        assert_eq!(
            decide(session.clone(), true).await.unwrap(),
            AutoSummary::Skip
        );
        assert_eq!(decide(session, false).await.unwrap(), AutoSummary::Generate);
        runtime.shutdown().await.unwrap();
    }

    #[test]
    fn streamed_markdown_reads_as_headings_bullets_and_paragraphs() {
        assert_eq!(
            markdown_blocks("# Decisions\n- Ship it\n  - Soon\n\nDone."),
            [
                (1, 0, "Decisions".into()),
                (0, 1, "Ship it".into()),
                (0, 2, "Soon".into()),
                (0, 0, "Done.".into())
            ]
        );
    }
}
