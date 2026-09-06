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

pub(crate) struct LiveCapture {
    pub session_id: String,
    /// `live.degraded`: no live transcription (no provider, or the engine
    /// fell back), which tints the transcript tab amber.
    pub degraded: bool,
    pub mic: f32,
    pub speaker: f32,
    pub muted: bool,
}

#[derive(Default)]
pub(crate) struct RecordingState {
    pub recorder: Option<Rc<Recorder>>,
    pub live: Option<LiveCapture>,
    pub finalizing: Vec<String>,
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
        // `useSTTConnection`: no current provider means no `conn`, so the
        // engine records without a transcription endpoint.
        let has_provider = self
            .provider_settings
            .stt_provider
            .as_deref()
            .is_some_and(|provider| !provider.is_empty());
        let languages = self.transcription_languages();
        let params = SessionParams {
            session_id: session_id.clone(),
            languages,
            onboarding: false,
            transcription_mode: TranscriptionMode::Live,
            model: self.provider_settings.stt_model.clone().unwrap_or_default(),
            base_url: String::new(),
            api_key: String::new(),
            keywords: Vec::new(),
            mic_device: self
                .provider_settings
                .string_setting("microphone_device", &["general", "microphone_device"])
                .filter(|device| !device.is_empty()),
            participant_human_ids: Vec::new(),
            self_human_id: None,
            speaker_assignments: Vec::new(),
        };
        self.recording.starting = true;
        cx.notify();
        let task = recorder.start(params);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.recording.starting = false;
                match result {
                    Ok(Ok(())) => {
                        this.recording.live = Some(LiveCapture {
                            session_id,
                            degraded: !has_provider,
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
                current_transcription_mode,
                error,
                ..
            }) => {
                let degraded =
                    error.is_some() || current_transcription_mode != TranscriptionMode::Live;
                match self.recording.live.as_mut() {
                    Some(live) if live.session_id == session_id => {
                        live.degraded = live.degraded || degraded;
                    }
                    _ => {
                        self.recording.live = Some(LiveCapture {
                            session_id,
                            degraded,
                            mic: 0.0,
                            speaker: 0.0,
                            muted: false,
                        });
                    }
                }
                if let Some(DegradedError::ProviderConfiguration { .. }) = error {
                    // The provider is misconfigured rather than absent; the
                    // toast for the absent case is already showing.
                }
            }
            Event::Lifecycle(SessionLifecycleEvent::Finalizing { session_id }) => {
                if self
                    .recording
                    .live
                    .as_ref()
                    .is_some_and(|live| live.session_id == session_id)
                {
                    self.recording.live = None;
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
                if self
                    .recording
                    .live
                    .as_ref()
                    .is_some_and(|live| live.session_id == session_id)
                {
                    self.recording.live = None;
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
                    if let Some(live) = self.recording.live.as_mut() {
                        live.degraded = true;
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
            Event::Data(_) => {}
        }
        cx.notify();
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

    /// `HeaderViewTranscriptLiveIcon` → `DancingSticks` at 16×16: a 1px
    /// line while silent, otherwise five 2px sticks scaled by amplitude.
    pub(super) fn render_dancing_sticks(&self, live: &LiveCapture) -> AnyElement {
        let color = if live.degraded {
            gpui::rgb(0xf59e0b)
        } else {
            gpui::rgb(0xef4444)
        };
        if live.muted {
            return crate::ui::icon("waveform", px(16.0), self.theme.foreground).into_any_element();
        }
        let amplitude = live.mic.hypot(live.speaker).min(1.0);
        let container = div().flex().size(px(16.0)).items_center().justify_center();
        if amplitude == 0.0 {
            return container
                .child(div().w(px(16.0)).h(px(1.0)).rounded_full().bg(color))
                .into_any_element();
        }
        let scale = 0.2 + 0.8 * amplitude;
        // `generatePattern(5)`: 50..100..50 from the edges to the middle.
        let pattern = [50.0f32, 75.0, 100.0, 75.0, 50.0];
        container
            .gap(px(1.0))
            .children(pattern.into_iter().map(|base| {
                let height = 16.0 * scale * (base / 100.0).clamp(0.25, 1.0);
                div().w(px(2.0)).h(px(height)).rounded_full().bg(color)
            }))
            .into_any_element()
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
