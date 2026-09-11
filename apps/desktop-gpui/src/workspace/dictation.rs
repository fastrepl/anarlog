//! `chat/components/input/use-dictation.ts` + `VoiceStatus`: the composer's
//! microphone button records a temporary WAV, the batch transcriber turns it
//! into text, and the text lands in the composer.

use std::time::Instant;

use gpui::{AnimationExt as _, AnyElement, ClickEvent, Context, SharedString, div, prelude::*, px};

use super::Workspace;
use super::toast::FlashVariant;
use crate::dictation::{self, Phase, Recording};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

/// One composer's voice input.
#[derive(Default)]
pub(crate) struct DictationState {
    pub phase: Phase,
    pub started_at: Option<Instant>,
    pub elapsed_seconds: u64,
    pub recording: Option<Recording>,
    /// Bumped per start so a stale tick or stop cannot touch a later run.
    pub run: u64,
}

impl Workspace {
    /// `start()`: refused while a meeting records, otherwise open the
    /// microphone and count the seconds until `stop()` or the cap.
    pub(crate) fn start_dictation(&mut self, cx: &mut Context<Self>) {
        if self.chat.dictation.phase != Phase::Idle || self.chat.busy() {
            return;
        }
        // `getCaptureState() !== "inactive"`
        if self.recording.live.is_some() || self.recording.starting {
            self.flash(
                FlashVariant::Warning,
                "Voice input is unavailable while Anarlog is recording a meeting.",
                cx,
            );
            return;
        }
        self.chat.dictation.run += 1;
        let run = self.chat.dictation.run;
        self.chat.dictation.phase = Phase::Starting;
        cx.notify();
        let audio = cx.global::<crate::audio::Audio>().0.clone();
        let device = self
            .provider_settings
            .string_setting("microphone_device", &["general", "microphone_device"])
            .filter(|device| !device.is_empty());
        match Recording::start(self.store.runtime(), audio, device) {
            Ok(recording) => {
                self.chat.dictation.recording = Some(recording);
                self.chat.dictation.started_at = Some(Instant::now());
                self.chat.dictation.elapsed_seconds = 0;
                self.chat.dictation.phase = Phase::Recording;
                cx.notify();
                self.tick_dictation(run, cx);
            }
            Err(error) => {
                tracing::error!(%error, "[chat-dictation] failed to start recording");
                self.chat.dictation.phase = Phase::Idle;
                self.flash_with_description(
                    FlashVariant::Error,
                    "Could not start voice input",
                    "Check microphone permission and the selected input device, then try again.",
                    cx,
                );
            }
        }
    }

    /// The 250ms elapsed timer; the five-minute cap stops the recording.
    fn tick_dictation(&mut self, run: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .await;
                let keep_going = this
                    .update(cx, |this, cx| {
                        let state = &mut this.chat.dictation;
                        if state.run != run || state.phase != Phase::Recording {
                            return false;
                        }
                        let elapsed = state
                            .started_at
                            .map_or(0, |started| started.elapsed().as_secs());
                        state.elapsed_seconds = elapsed.min(dictation::MAX_SECONDS);
                        cx.notify();
                        if elapsed >= dictation::MAX_SECONDS {
                            this.stop_dictation(cx);
                            return false;
                        }
                        true
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
    }

    /// `stop()`: finish the file, run the batch transcriber over it
    /// (`numSpeakers: 1`, no keywords), and insert the text.
    pub(crate) fn stop_dictation(&mut self, cx: &mut Context<Self>) {
        if self.chat.dictation.phase != Phase::Recording {
            return;
        }
        let Some(recording) = self.chat.dictation.recording.take() else {
            return;
        };
        self.chat.dictation.phase = Phase::Transcribing;
        cx.notify();
        let run = self.chat.dictation.run;
        let connection = self.store.stt_connection(&self.provider_settings);
        let languages = self.transcription_languages();
        let runtime = self.store.runtime().clone();
        let composer = self.chat.composer.clone();
        cx.spawn(async move |this, cx| {
            let recorded = recording.stop().await;
            let outcome = match &recorded {
                Ok(recorded) => {
                    let connection = connection.await.ok().flatten();
                    transcribe(&runtime, connection.as_ref(), languages, recorded).await
                }
                Err(error) => Err(error.to_string()),
            };
            if let Ok(recorded) = &recorded {
                dictation::discard(recorded);
            }
            this.update(cx, |this, cx| {
                match outcome {
                    Ok(text) => {
                        if let Some(composer) = composer {
                            composer.update(cx, |composer, cx| composer.insert_text(&text, cx));
                        }
                    }
                    Err(message) => {
                        tracing::error!(
                            error = %message,
                            "[chat-dictation] failed to transcribe recording"
                        );
                        if dictation::is_no_speech(&message) {
                            this.flash_with_description(
                                FlashVariant::Warning,
                                "No speech detected",
                                "Try speaking a little closer to the microphone.",
                                cx,
                            );
                        } else {
                            this.flash_with_description(
                                FlashVariant::Error,
                                "Could not transcribe voice input",
                                message,
                                cx,
                            );
                        }
                    }
                }
                if this.chat.dictation.run == run {
                    this.chat.dictation.phase = Phase::Idle;
                    this.chat.dictation.started_at = None;
                    this.chat.focus_pending = true;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The composer unmounting mid-recording (`useMountEffect` cleanup):
    /// the recording is cancelled and its file dropped.
    pub(crate) fn cancel_dictation(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.chat.dictation;
        if matches!(state.phase, Phase::Starting | Phase::Recording) {
            state.phase = Phase::Idle;
            state.started_at = None;
            if let Some(recording) = state.recording.take() {
                self.store.runtime().spawn(recording.cancel());
            }
            cx.notify();
        }
    }

    pub(crate) fn dictation_active(&self) -> bool {
        self.chat.dictation.phase != Phase::Idle
    }

    /// `VoiceStatus`: the waveform (or the `Starting…` / `Transcribing…`
    /// spinner), the elapsed `m:ss`, the stop button, and the send / stop
    /// response control.
    pub(super) fn render_voice_status(
        &self,
        show_send: bool,
        send_enabled: bool,
        streaming: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let phase = self.chat.dictation.phase;
        let processing = phase != Phase::Recording;
        let indicator: AnyElement =
            if processing {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .tw_text_xs()
                    .text_color(theme.muted_foreground)
                    .child(crate::ui::spinner(
                        "chat-dictation-spinner",
                        px(14.0),
                        theme.muted_foreground,
                    ))
                    .child(if phase == Phase::Starting {
                        "Starting…"
                    } else {
                        "Transcribing…"
                    })
                    .into_any_element()
            } else {
                // `chat-input-waveform-bar`: `w-px rounded-full bg-muted-foreground/55`,
                // scaling between 0.45 and 1 over 700ms, each bar 70ms behind.
                div()
                    .flex()
                    .min_w_0()
                    .flex_1()
                    .items_center()
                    .gap(px(3.0))
                    .overflow_hidden()
                    .children(dictation::WAVEFORM_HEIGHTS.iter().enumerate().map(
                        |(index, height)| {
                            let height = *height;
                            div()
                                .flex_shrink_0()
                                .w(px(1.0))
                                .h(px(height))
                                .rounded_full()
                                .bg(alpha(theme.muted_foreground, 0.55))
                                .with_animation(
                                    ("chat-dictation-bar", index),
                                    gpui::Animation::new(std::time::Duration::from_millis(700))
                                        .repeat()
                                        .with_easing(gpui::ease_in_out),
                                    move |bar, delta| {
                                        // The `-70ms * index` delay shifts the cycle.
                                        let phase = (delta - index as f32 * 0.1).rem_euclid(1.0);
                                        let scale = if phase < 0.5 {
                                            0.45 + (phase / 0.5) * 0.55
                                        } else {
                                            1.0 - ((phase - 0.5) / 0.5) * 0.55
                                        };
                                        bar.h(px(height * scale))
                                    },
                                )
                        },
                    ))
                    .into_any_element()
            };
        let stop = div()
            .id("chat-dictation-stop")
            .flex()
            .size(px(28.0))
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(theme.muted)
            .when(processing, |button| button.opacity(0.6))
            .when(!processing, |button| {
                button
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.stop_dictation(cx)))
            })
            .child(if processing {
                crate::ui::spinner("chat-dictation-stop-spinner", px(14.0), theme.foreground)
                    .into_any_element()
            } else {
                icon("square", px(12.0), theme.foreground).into_any_element()
            });
        let mut row = div()
            .mt_2()
            .flex()
            .min_h(px(28.0))
            .w_full()
            .items_center()
            .gap_2()
            .child(if processing {
                div().min_w_0().flex_1().child(indicator).into_any_element()
            } else {
                indicator
            });
        if !processing {
            row = row.child(div().tw_text_xs().text_color(theme.muted_foreground).child(
                SharedString::from(dictation::format_elapsed(
                    self.chat.dictation.elapsed_seconds,
                )),
            ));
        }
        row = row.child(stop);
        if streaming {
            row = row.child(
                div()
                    .id("chat-dictation-stop-response")
                    .flex()
                    .size(px(28.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(14.0))
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.muted))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.stop_chat(cx)))
                    .child(icon("square", px(14.0), theme.foreground)),
            );
        } else if show_send {
            row = row.child(self.render_chat_send_button(send_enabled, cx));
        }
        row.into_any_element()
    }
}

/// `runBatch(recordedPath, { numSpeakers: 1, keywords: [] })` with the words
/// joined into text; a transcript without words is `No speech was detected`.
async fn transcribe(
    runtime: &tokio::runtime::Handle,
    connection: Option<&crate::db::SttConnection>,
    languages: Vec<anlg_language::Language>,
    recorded: &dictation::RecordedAudio,
) -> Result<String, String> {
    let target = super::recording::batch_target(connection).ok_or_else(|| {
        let label = connection
            .map(|conn| conn.model.clone())
            .unwrap_or_else(|| "the selected speech-to-text provider".to_string());
        format!("{label} is not available for batch transcription on this platform.")
    })?;
    let provider = serde_json::from_value::<anlg_listener2_core::BatchProvider>(
        serde_json::Value::String(target.provider.clone()),
    )
    .map_err(|_| "Transcription failed".to_string())?;
    let params = anlg_listener2_core::BatchParams {
        session_id: format!("chat-dictation-{}", uuid::Uuid::new_v4()),
        provider,
        file_path: recorded.file_path.to_string_lossy().into_owned(),
        model: Some(target.model.clone()),
        base_url: target.base_url.clone(),
        api_key: target.api_key.clone(),
        languages,
        keywords: Vec::new(),
        num_speakers: Some(1),
        min_speakers: None,
        max_speakers: None,
        known_speakers: Vec::new(),
    };
    let (events_tx, mut events_rx) =
        tokio::sync::mpsc::unbounded_channel::<anlg_listener2_core::BatchEvent>();
    struct Runtime(tokio::sync::mpsc::UnboundedSender<anlg_listener2_core::BatchEvent>);
    impl anlg_listener2_core::BatchRuntime for Runtime {
        fn emit(&self, event: anlg_listener2_core::BatchEvent) {
            let _ = self.0.send(event);
        }
    }
    let run = runtime.spawn(anlg_listener2_core::run_batch(
        std::sync::Arc::new(Runtime(events_tx)),
        params,
    ));
    let mut outcome: Option<Result<String, String>> = None;
    while let Some(event) = events_rx.recv().await {
        match event {
            anlg_listener2_core::BatchEvent::BatchResponse { response, .. } => {
                let text = dictation::transcript_text(&crate::batch::transform_batch(&response));
                outcome = Some(if text.is_empty() {
                    Err("No speech was detected in the audio.".to_string())
                } else {
                    Ok(text)
                });
                break;
            }
            anlg_listener2_core::BatchEvent::BatchFailed { error, .. } => {
                outcome = Some(Err(error));
                break;
            }
            _ => {}
        }
    }
    let _ = run.await;
    outcome.unwrap_or_else(|| Err("Transcription failed".to_string()))
}
