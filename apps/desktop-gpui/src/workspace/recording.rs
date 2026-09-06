//! The listener store (`store/zustand/listener`) and `useStartListening` /
//! `stopListening`: the capture lifecycle as the header, sidebar, and toasts
//! see it. The engine itself is `listener-core`'s root actor
//! (`crate::recording::Recorder`).

use std::rc::Rc;

use anlg_listener_core::actors::SessionParams;
use anlg_listener_core::{
    DegradedError, SessionDataEvent, SessionLifecycleEvent, SessionProgressEvent, TranscriptionMode,
};
use gpui::{AnyElement, Context, Div, SharedString, div, prelude::*, px};

use super::Workspace;
use crate::recording::{Event, Recorder};
use crate::ui::TailwindText as _;

/// `getSessionMode(sessionId)`
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SessionMode {
    Inactive,
    Active,
    Finalizing,
}

/// `createCaptureLifecycle`'s transcript identity and persistence queue: the
/// transcript row is created on the first delta and later deltas are
/// journaled, one write in flight at a time (the worker's batching), then
/// flushed into the columns when the capture ends.
pub(crate) struct LivePersistence {
    pub transcript_id: String,
    pub created_at: String,
    pub started_at_ms: i64,
    pub memo: String,
    pub provider: String,
    pub model: String,
    pub created: bool,
    pub writing: bool,
    pub pending: Vec<anlg_listener_core::LiveTranscriptDelta>,
    /// The capture ended: once the queue drains, flush the journal.
    pub finishing: bool,
}

pub(crate) struct LiveCapture {
    pub session_id: String,
    pub persistence: LivePersistence,
    /// `live.requestedLiveTranscription`: the session asked for live mode.
    pub requested_live: bool,
    /// `live.liveTranscriptionActive`: the engine is streaming live.
    pub live_active: bool,
    /// `live.degraded`: the engine's degradation error, if any.
    pub error: Option<DegradedError>,
    pub mic: f32,
    pub speaker: f32,
    pub muted: bool,
}

impl LiveCapture {
    /// `Boolean(degraded)` for the amber tint: any degradation or a capture
    /// that is not transcribing live.
    pub fn degraded(&self) -> bool {
        self.error.is_some() || !self.live_active
    }
}

#[derive(Default)]
pub(crate) struct RecordingState {
    pub recorder: Option<Rc<Recorder>>,
    pub live: Option<LiveCapture>,
    pub finalizing: Vec<String>,
    /// Persistence queues of captures that ended and still have writes or
    /// the final flush outstanding, by session id.
    pub flushing: Vec<(String, LivePersistence)>,
    /// The persistent `recording-without-transcription` warning toast.
    pub toast: Option<RecordingToast>,
    /// `Record` was clicked and the engine has not answered yet.
    pub starting: bool,
}

pub(crate) struct RecordingToast {
    pub title: &'static str,
    pub description: &'static str,
    pub action: &'static str,
}

impl Workspace {
    pub(crate) fn session_mode(&self, session_id: &str) -> SessionMode {
        if self
            .recording
            .live
            .as_ref()
            .is_some_and(|live| live.session_id == session_id)
        {
            SessionMode::Active
        } else if self.recording.finalizing.iter().any(|id| id == session_id) {
            SessionMode::Finalizing
        } else {
            SessionMode::Inactive
        }
    }

    /// Spawn the root actor once the window is up and pump its events.
    pub(crate) fn spawn_recorder(&mut self, cx: &mut Context<Self>) {
        let runtime = self.store.runtime().clone();
        let base = self
            .store
            .path()
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        let audio = cx.global::<crate::audio::Audio>().0.clone();
        cx.spawn(async move |this, cx| {
            let spawned = Recorder::spawn(runtime, base, audio).await;
            let (recorder, mut events) = match spawned {
                Ok(spawned) => spawned,
                Err(error) => {
                    tracing::error!(%error, "failed_to_spawn_root_actor");
                    return;
                }
            };
            this.update(cx, |this, _| {
                this.recording.recorder = Some(Rc::new(recorder));
            })
            .ok();
            while let Some(event) = events.recv().await {
                if this
                    .update(cx, |this, cx| this.handle_recording_event(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    /// `startListening`: the capture params from the note and settings, then
    /// `start_capture`; on success the sidebar collapses and, without a
    /// transcription provider, the warning toast appears.
    pub(crate) fn start_listening(&mut self, session_id: String, cx: &mut Context<Self>) {
        // `canStartLiveSession`: one capture at a time.
        if self.recording.live.is_some() || self.recording.starting {
            return;
        }
        let Some(recorder) = self.recording.recorder.clone() else {
            self.flash(
                super::toast::FlashVariant::Error,
                "Anarlog could not safely start recording. Please try again.",
                cx,
            );
            return;
        };
        // `useSTTConnection`: the provider's base URL and credential-store
        // key; `None` (no `conn`) records without a transcription endpoint.
        let connection = self.store.stt_connection(&self.provider_settings);
        let languages = self.transcription_languages();
        // `memoMd = session?.raw_md ?? ""`
        let memo = match &self.note {
            super::Note::Ready { preview, .. } if preview.session.id == session_id => {
                preview.memo_body.clone()
            }
            _ => String::new(),
        };
        let mic_device = self
            .provider_settings
            .string_setting("microphone_device", &["general", "microphone_device"])
            .filter(|device| !device.is_empty());
        self.recording.starting = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let connection = connection.await.ok().flatten();
            let has_provider = connection.is_some();
            let params = SessionParams {
                session_id: session_id.clone(),
                languages,
                onboarding: false,
                transcription_mode: TranscriptionMode::Live,
                model: connection.as_ref().map(|c| c.model.clone()).unwrap_or_default(),
                base_url: connection.as_ref().map(|c| c.base_url.clone()).unwrap_or_default(),
                api_key: connection.as_ref().map(|c| c.api_key.clone()).unwrap_or_default(),
                keywords: Vec::new(),
                mic_device,
                participant_human_ids: Vec::new(),
                self_human_id: None,
                speaker_assignments: Vec::new(),
            };
            let result = recorder.start(params).await;
            this.update(cx, |this, cx| {
                this.recording.starting = false;
                match result {
                    Ok(Ok(())) => {
                        this.recording.live = Some(LiveCapture {
                            session_id,
                            persistence: LivePersistence {
                                transcript_id: uuid::Uuid::new_v4().to_string(),
                                created_at: chrono::Utc::now()
                                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                                    .to_string(),
                                started_at_ms: chrono::Utc::now().timestamp_millis(),
                                memo,
                                provider: connection
                                    .as_ref()
                                    .map(|c| c.provider.clone())
                                    .unwrap_or_default(),
                                model: connection
                                    .as_ref()
                                    .map(|c| c.model.clone())
                                    .unwrap_or_default(),
                                created: false,
                                writing: false,
                                pending: Vec::new(),
                                finishing: false,
                            },
                            requested_live: true,
                            live_active: has_provider,
                            error: None,
                            mic: 0.0,
                            speaker: 0.0,
                            muted: false,
                        });
                        // `setLeftSidebarExpanded(false)`
                        this.sidebar_expanded = false;
                        if !has_provider {
                            this.recording.toast = Some(RecordingToast {
                                title: "Live transcription is not configured",
                                description: "Audio is being saved. Choose a transcription provider to ensure this recording can be transcribed.",
                                action: "Configure",
                            });
                        }
                    }
                    Ok(Err(error)) => {
                        tracing::error!(?error, "[listener] failed to start recording");
                        this.flash(
                            super::toast::FlashVariant::Error,
                            "Anarlog could not safely start recording. Please try again.",
                            cx,
                        );
                    }
                    Err(error) => {
                        tracing::error!(%error, "[listener] failed to start recording");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `stopListening` → `stop_capture`; the session finalizes until the
    /// engine reports `inactive` with the encoded audio path.
    pub(crate) fn stop_listening(&mut self, cx: &mut Context<Self>) {
        let Some(recorder) = self.recording.recorder.clone() else {
            return;
        };
        if self.recording.live.is_none() {
            return;
        }
        // The tokio task runs to completion on its own; the lifecycle events
        // carry the outcome.
        drop(recorder.stop());
        cx.notify();
    }

    fn handle_recording_event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Lifecycle(SessionLifecycleEvent::Active {
                session_id,
                requested_transcription_mode,
                current_transcription_mode,
                error,
            }) => {
                let requested_live = requested_transcription_mode == TranscriptionMode::Live;
                let live_active = current_transcription_mode == TranscriptionMode::Live;
                match self.recording.live.as_mut() {
                    Some(live) if live.session_id == session_id => {
                        live.requested_live = requested_live;
                        live.live_active = live_active;
                        live.error = error;
                    }
                    _ => {
                        self.recording.live = Some(LiveCapture {
                            session_id,
                            persistence: LivePersistence {
                                transcript_id: uuid::Uuid::new_v4().to_string(),
                                created_at: chrono::Utc::now()
                                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                                    .to_string(),
                                started_at_ms: chrono::Utc::now().timestamp_millis(),
                                memo: String::new(),
                                provider: String::new(),
                                model: String::new(),
                                created: false,
                                writing: false,
                                pending: Vec::new(),
                                finishing: false,
                            },
                            requested_live,
                            live_active,
                            error,
                            mic: 0.0,
                            speaker: 0.0,
                            muted: false,
                        });
                    }
                }
            }
            Event::Lifecycle(SessionLifecycleEvent::Finalizing { session_id }) => {
                if let Some(live) = self
                    .recording
                    .live
                    .take_if(|live| live.session_id == session_id)
                {
                    self.finish_live_persistence(live.persistence, session_id.clone(), cx);
                }
                self.recording.toast = None;
                if !self.recording.finalizing.contains(&session_id) {
                    self.recording.finalizing.push(session_id);
                }
            }
            Event::Lifecycle(SessionLifecycleEvent::Inactive {
                session_id,
                audio_path,
                error,
            }) => {
                if let Some(live) = self
                    .recording
                    .live
                    .take_if(|live| live.session_id == session_id)
                {
                    self.finish_live_persistence(live.persistence, session_id.clone(), cx);
                }
                self.recording.toast = None;
                self.recording.finalizing.retain(|id| *id != session_id);
                if let Some(error) = error {
                    tracing::error!(%error, "[listener] capture ended with an error");
                }
                if audio_path.is_some() {
                    // `onStopped` → `catalogLocalSessionAudio`: the primary
                    // audio attachment row, `transcript_status: processing`.
                    let task = self.store.catalog_session_audio(session_id.clone());
                    cx.spawn(async move |this, cx| {
                        if let Ok(Err(error)) = task.await {
                            tracing::error!(%error, "[listener] failed to catalog session audio");
                        }
                        this.update(cx, |this, cx| {
                            if this.selected.as_deref() == Some(session_id.as_str()) {
                                this.reload_note(session_id, cx);
                            }
                        })
                        .ok();
                    })
                    .detach();
                }
            }
            Event::Progress(SessionProgressEvent::AudioReady { .. })
            | Event::Progress(SessionProgressEvent::AudioInitializing { .. })
            | Event::Progress(SessionProgressEvent::Connecting { .. })
            | Event::Progress(SessionProgressEvent::Connected { .. }) => {}
            Event::Error(error) => match error {
                anlg_listener_core::SessionErrorEvent::AudioError {
                    error, is_fatal, ..
                } => {
                    tracing::warn!(%error, is_fatal, "[listener] audio error");
                }
                anlg_listener_core::SessionErrorEvent::ConnectionError { error, .. } => {
                    tracing::warn!(%error, "[listener] connection error");
                    if let Some(live) = self.recording.live.as_mut()
                        && live.error.is_none()
                    {
                        live.error = Some(DegradedError::StreamError { message: error });
                    }
                }
            },
            Event::Data(SessionDataEvent::AudioAmplitude { mic, speaker, .. }) => {
                if let Some(live) = self.recording.live.as_mut() {
                    live.mic = f32::from(mic) / f32::from(u16::MAX);
                    live.speaker = f32::from(speaker) / f32::from(u16::MAX);
                }
            }
            Event::Data(SessionDataEvent::MicMuted { value, .. }) => {
                if let Some(live) = self.recording.live.as_mut() {
                    live.muted = value;
                }
            }
            Event::Data(SessionDataEvent::TranscriptDelta { session_id, delta }) => {
                // `handlePersist`: empty deltas are ignored.
                if delta.new_words.is_empty() && delta.replaced_ids.is_empty() {
                    return;
                }
                if let Some(live) = self
                    .recording
                    .live
                    .as_mut()
                    .filter(|live| live.session_id == session_id)
                {
                    live.persistence.pending.push(*delta);
                    self.drain_live_persistence(session_id, cx);
                }
            }
            Event::Data(_) => {}
        }
        cx.notify();
    }

    fn persistence_mut(&mut self, session_id: &str) -> Option<&mut LivePersistence> {
        if let Some(live) = self
            .recording
            .live
            .as_mut()
            .filter(|live| live.session_id == session_id)
        {
            return Some(&mut live.persistence);
        }
        self.recording
            .flushing
            .iter_mut()
            .find(|(id, _)| id == session_id)
            .map(|(_, persistence)| persistence)
    }

    /// The persistence worker's drain: coalesce the pending deltas into one
    /// write — `createLiveTranscript` first, `applyLiveTranscriptDeltaToDatabase`
    /// after — keep draining while more arrive, and once a finished capture's
    /// queue is empty run `flushLiveTranscriptDeltasToDatabase`.
    fn drain_live_persistence(&mut self, session_id: String, cx: &mut Context<Self>) {
        let Some(persistence) = self.persistence_mut(&session_id) else {
            return;
        };
        if persistence.writing {
            return;
        }
        if persistence.pending.is_empty() {
            if !persistence.finishing {
                return;
            }
            let transcript_id = persistence.transcript_id.clone();
            self.recording.flushing.retain(|(id, _)| *id != session_id);
            let flush = self.store.flush_live_deltas(transcript_id);
            cx.spawn(async move |this, cx| {
                if let Ok(Err(error)) = flush.await {
                    tracing::error!(%error, "[listener] failed to flush live transcript");
                }
                this.update(cx, |this, cx| {
                    if this.selected.as_deref() == Some(session_id.as_str()) {
                        this.reload_note(session_id.clone(), cx);
                    }
                })
                .ok();
            })
            .detach();
            return;
        }
        let deltas = std::mem::take(&mut persistence.pending);
        let delta = crate::live_transcript::coalesce_deltas(&deltas);
        persistence.writing = true;
        let transcript_id = persistence.transcript_id.clone();
        let created = persistence.created;
        let (created_at, started_at_ms, memo, provider, model) = (
            persistence.created_at.clone(),
            persistence.started_at_ms,
            persistence.memo.clone(),
            persistence.provider.clone(),
            persistence.model.clone(),
        );
        let task = if created {
            self.store.journal_live_delta(transcript_id, delta)
        } else {
            self.store.create_live_transcript(
                transcript_id,
                session_id.clone(),
                created_at,
                started_at_ms,
                memo,
                provider,
                model,
                delta,
            )
        };
        cx.spawn(async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| {
                if let Err(error) = &result {
                    tracing::error!(%error, "[listener] failed to persist transcript");
                }
                if let Some(persistence) = this.persistence_mut(&session_id) {
                    persistence.writing = false;
                    if result.is_ok() {
                        persistence.created = true;
                    }
                }
                this.drain_live_persistence(session_id.clone(), cx);
                if this.selected.as_deref() == Some(session_id.as_str()) {
                    this.reload_note(session_id.clone(), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// The capture ended: move its queue aside, mark it finishing, and let
    /// the drain write the tail and flush the journal in order.
    fn finish_live_persistence(
        &mut self,
        mut persistence: LivePersistence,
        session_id: String,
        cx: &mut Context<Self>,
    ) {
        persistence.finishing = true;
        self.recording.flushing.retain(|(id, _)| *id != session_id);
        self.recording
            .flushing
            .push((session_id.clone(), persistence));
        self.drain_live_persistence(session_id, cx);
    }

    /// `getTranscriptionLanguages(aiLanguage, spokenLanguages)`: the AI
    /// language first, then the distinct spoken languages, all as base codes.
    fn transcription_languages(&self) -> Vec<anlg_language::Language> {
        let ai = self
            .provider_settings
            .string_setting("ai_language", &["language", "ai_language"])
            .unwrap_or_else(|| "en".to_string());
        let mut codes = vec![super::settings::base_language_code(&ai)];
        codes.extend(self.spoken_languages());
        codes
            .iter()
            .filter_map(|code| code.parse::<anlg_language::Language>().ok())
            .collect()
    }

    /// `HeaderViewTranscriptLiveIcon` → `DancingSticks` at 16×16, amber
    /// while degraded, the waveform while muted.
    pub(super) fn render_dancing_sticks(&self, live: &LiveCapture) -> AnyElement {
        let color = if live.degraded() {
            gpui::rgb(0xf59e0b)
        } else {
            gpui::rgb(0xef4444)
        };
        if live.muted {
            return crate::ui::icon("waveform", px(16.0), self.theme.foreground).into_any_element();
        }
        dancing_sticks(
            live.mic.hypot(live.speaker).min(1.0),
            color,
            16.0,
            16.0,
            2.0,
            1.0,
        )
    }

    /// The transcript tab body while `getSessionMode` is not inactive, after
    /// `useTranscriptScreen`: `batch_fallback` (`BatchState`) when the
    /// capture is not transcribing live, else `listening` / `finalizing`
    /// (`TranscriptListeningState`).
    pub(super) fn render_live_transcript_screen(
        &self,
        session_id: &str,
        window: &gpui::Window,
    ) -> Option<AnyElement> {
        let theme = self.theme;
        let mode = self.session_mode(session_id);
        if mode == SessionMode::Inactive {
            return None;
        }
        let live = self
            .recording
            .live
            .as_ref()
            .filter(|live| live.session_id == session_id);
        let copy = |title: String, description: String| {
            self.transcript_screen_copy(&title, &description, window)
        };
        if let Some(live) = live.filter(|live| !live.live_active) {
            // `BatchState`
            let fallback = live.requested_live;
            let reconnecting = fallback
                && live.error.as_ref().is_some_and(|error| {
                    !matches!(
                        error,
                        DegradedError::AuthenticationFailed { .. }
                            | DegradedError::ProviderConfiguration { .. }
                    )
                });
            let title = if fallback {
                if reconnecting {
                    "Reconnecting live transcription"
                } else if live.error.is_some() {
                    "Live transcription stopped"
                } else {
                    "Live transcription unavailable"
                }
            } else {
                "Batch transcription mode"
            };
            let description = if fallback {
                format!(
                    "{}Recording continues{}. A complete transcript will be generated after you stop.",
                    live.error
                        .as_ref()
                        .map(|error| format!("{}. ", degraded_message(error)))
                        .unwrap_or_default(),
                    if reconnecting {
                        " while we reconnect"
                    } else {
                        ""
                    }
                )
            } else {
                "Recording continues. Your transcript will be generated after you stop.".to_string()
            };
            return Some(
                transcript_screen()
                    .child(div().mb_5().child(dancing_sticks(
                        live.mic.hypot(live.speaker).min(1.0),
                        gpui::rgb(0xa3a3a3),
                        36.0,
                        80.0,
                        3.0,
                        3.0,
                    )))
                    .child(copy(title.to_string(), description))
                    .into_any_element(),
            );
        }
        // `TranscriptListeningState`
        let finalizing = mode == SessionMode::Finalizing;
        Some(
            transcript_screen()
                .child(div().mb_5().child(if finalizing {
                    crate::ui::icon("circle-notch", px(36.0), theme.muted_foreground)
                } else {
                    crate::ui::icon("waveform", px(36.0), theme.muted_foreground)
                }))
                .child(copy(
                    if finalizing {
                        "Finalizing transcript..."
                    } else {
                        "Listening..."
                    }
                    .to_string(),
                    if finalizing {
                        "Transcript is still being written."
                    } else {
                        "Transcript will appear here when the first segment arrives."
                    }
                    .to_string(),
                ))
                .into_any_element(),
        )
    }

    /// `TranscriptEmptyState` without a batch: `Audio available` with
    /// Re-transcribe and Upload transcript when the session has audio.
    pub(super) fn render_transcript_empty_state(
        &self,
        has_audio: bool,
        window: &gpui::Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        // `Button size="sm"`: `h-8 px-3 gap-2 text-sm`, default or outline.
        let button = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .relative()
                .flex()
                .h(px(32.0))
                .items_center()
                .gap_2()
                .px_3()
                .tw_text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                .child(crate::squircle::squircle(
                    crate::squircle::CONTROL_RADIUS,
                    Some(if primary {
                        theme.primary
                    } else {
                        theme.background
                    }),
                    (!primary).then_some((1.0, theme.border)),
                ))
                .text_color(if primary {
                    theme.primary_foreground
                } else {
                    theme.foreground
                })
                .child(div().relative().flex().items_center().gap_2().child(label))
        };
        transcript_screen()
            .child(div().mb_5().child(crate::ui::icon(
                "waveform",
                px(36.0),
                theme.muted_foreground,
            )))
            .child(div().mb_6().child(self.transcript_screen_copy(
                if has_audio {
                    "Audio available"
                } else {
                    "No transcript available"
                },
                if has_audio {
                    "Re-transcribe this audio, or upload a transcript file."
                } else {
                    "Upload audio or a transcript file to populate this note."
                },
                window,
            )))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(has_audio, |row| {
                        row.child(
                            button("transcript-retranscribe", "Re-transcribe", true).on_click(
                                cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                                    this.retranscribe(cx)
                                }),
                            ),
                        )
                    })
                    .child(
                        button("transcript-upload", "Upload transcript", false).on_click(
                            cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                                this.upload_transcript(window, cx)
                            }),
                        ),
                    ),
            )
            .into_any_element()
    }

    /// `flex max-w-md flex-col gap-2` with the `text-base font-medium` title
    /// and the centred `text-sm leading-relaxed` description, wrapped the
    /// way WebKit wraps it.
    fn transcript_screen_copy(&self, title: &str, description: &str, window: &gpui::Window) -> Div {
        let theme = self.theme;
        let mut style = window.text_style();
        style.font_size = px(14.0).into();
        style.color = theme.muted_foreground.into();
        if let Some(font) = &self.font_family {
            style.font_family = font.clone();
        }
        let run = style.to_run(description.len());
        div()
            .flex()
            .w_full()
            .max_w(px(448.0))
            .flex_col()
            .gap_2()
            .items_center()
            .child(
                div()
                    .tw_text_base()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child(SharedString::from(title.to_string())),
            )
            .child(
                div().w_full().child(
                    crate::prose_text::ProseText::new(
                        description.to_string(),
                        vec![run],
                        px(14.0),
                        px(22.0),
                    )
                    .centered(),
                ),
            )
    }

    /// `useRegenerateTranscript` runs the batch pipeline over the stored
    /// audio, which needs a configured provider; the batch pipeline is not
    /// ported yet, so the missing-provider outcome is reported directly.
    pub(crate) fn retranscribe(&mut self, cx: &mut Context<Self>) {
        self.flash(
            super::toast::FlashVariant::Error,
            "Transcription provider needed to re-transcribe this audio.",
            cx,
        );
    }

    /// The persistent sonner warning (`richColors`, `duration: Infinity`)
    /// with the title, description, `Configure` action, and close button.
    pub(super) fn render_recording_toast(&self, cx: &Context<Self>) -> Option<gpui::Stateful<Div>> {
        let toast = self.recording.toast.as_ref()?;
        let (background, border, text) = if self.theme.dark {
            (
                gpui::rgb(0x1d1f00),
                gpui::rgb(0x3d3d00),
                gpui::rgb(0xf3cf58),
            )
        } else {
            (
                gpui::rgb(0xfffcf0),
                gpui::rgb(0xfdf5d3),
                gpui::rgb(0xdc7609),
            )
        };
        Some(
            div()
                .id("recording-toast")
                .absolute()
                .right(px(32.0))
                .bottom(px(32.0))
                .w(px(300.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .px_4()
                .py_4()
                .rounded(px(8.0))
                .border_1()
                .border_color(border)
                .bg(background)
                .text_color(text)
                .shadow(vec![gpui::BoxShadow {
                    color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                    offset: gpui::point(px(0.0), px(4.0)),
                    blur_radius: px(12.0),
                    spread_radius: px(0.0),
                }])
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .size(px(16.0))
                        .flex_shrink_0()
                        .items_center()
                        .ml(px(-3.0))
                        .mr(px(4.0))
                        .child(crate::ui::icon("alert-triangle", px(20.0), text)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .line_height(px(19.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(SharedString::from(toast.title)),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .line_height(px(19.0))
                                .child(SharedString::from(toast.description)),
                        ),
                )
                .child(
                    div()
                        .id("recording-toast-action")
                        .flex_shrink_0()
                        .h(px(24.0))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .rounded(px(4.0))
                        .bg(self.theme.foreground)
                        .text_color(self.theme.background)
                        .tw_text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                            this.open_settings(
                                super::settings::SettingsTab::Transcription,
                                window,
                                cx,
                            );
                        }))
                        .child(SharedString::from(toast.action)),
                )
                .child(
                    // sonner's close button: a 20px circle over the top-left corner.
                    div()
                        .id("recording-toast-close")
                        .absolute()
                        .left(px(-10.0))
                        .top(px(-10.0))
                        .size(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(border)
                        .bg(background)
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.recording.toast = None;
                            cx.notify();
                        }))
                        .child(crate::ui::icon("x", px(12.0), text)),
                ),
        )
    }
}

/// `flex h-full min-h-[400px] flex-col items-center justify-center px-6 text-center`
fn transcript_screen() -> Div {
    div()
        .flex()
        .h_full()
        .min_h(px(400.0))
        .flex_col()
        .items_center()
        .justify_center()
        .px_6()
}

/// `degradedMessage`
fn degraded_message(error: &DegradedError) -> String {
    match error {
        DegradedError::AuthenticationFailed { provider } => {
            format!("Authentication failed ({provider})")
        }
        DegradedError::UpstreamUnavailable { message } => message.clone(),
        DegradedError::ConnectionTimeout => "Transcription connection timed out".to_string(),
        DegradedError::ProviderConfiguration { provider, .. } => {
            format!("Transcription provider is misconfigured ({provider})")
        }
        DegradedError::StreamError { .. } => "Transcription stream error".to_string(),
    }
}

/// `DancingSticks`: a 1px line while silent, otherwise sticks of
/// `stick_width` with `gap`, their heights following `generatePattern`
/// scaled by `0.2 + 0.8 * amplitude`.
fn dancing_sticks(
    amplitude: f32,
    color: gpui::Rgba,
    height: f32,
    width: f32,
    stick_width: f32,
    gap: f32,
) -> AnyElement {
    let container = div()
        .flex()
        .w(px(width))
        .h(px(height))
        .items_center()
        .justify_center();
    if amplitude == 0.0 {
        return container
            .child(div().w(px(width)).h(px(1.0)).rounded_full().bg(color))
            .into_any_element();
    }
    let count = (((width + gap) / (stick_width + gap)).floor() as usize).max(1);
    let scale = 0.2 + 0.8 * amplitude.clamp(0.0, 1.0);
    let mid = (count as f32 - 1.0) / 2.0;
    container
        .gap(px(gap))
        .children((0..count).map(|index| {
            let base = if count <= 1 {
                100.0
            } else {
                let distance = (index as f32 - mid).abs() / mid;
                50.0 + 50.0 * (1.0 - distance)
            };
            let stick = height * scale * (base / 100.0).clamp(0.25, 1.0);
            div()
                .w(px(stick_width))
                .h(px(stick))
                .rounded_full()
                .bg(color)
        }))
        .into_any_element()
}
