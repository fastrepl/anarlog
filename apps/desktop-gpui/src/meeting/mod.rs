pub mod ai;
pub mod ai_view;
pub mod automation;
pub mod capture;
pub mod config;
pub mod context;
pub mod floating;
pub mod foundation;
pub mod host;
pub mod icons;
pub mod live;
pub mod model;
pub mod persistence;
pub mod playback;
pub mod player_view;
pub mod provider;
pub mod recovery;
pub mod retention;
pub mod schedule;
pub mod store;
pub mod subscription;
#[cfg(test)]
mod tests;
pub mod tools;
pub mod upload;
mod views;
pub mod widgets;

use std::sync::Arc;
use std::time::Duration;

use crate::contracts::{LaneContext, MeetingEvent, MeetingIntent};
use crate::ui::{
    input::{InputEvent, TextInput},
    theme::theme,
};
use capture::{CaptureService, Phase};
use desktop_runtime::{CancellationToken, Generation, ServiceError, SessionId};
use gpui::{
    Context, Entity, EventEmitter, Global, Render, SharedString, Subscription, Task, Window, div,
    prelude::*,
};
use model::{Transcript, Word, failure};
use store::{SpeakerAssignment, SpeakerScope, TranscriptStore};
use views::{TranscriptAction, TranscriptView};

#[derive(Clone)]
pub struct MeetingServices {
    pub capture: CaptureService,
    pub playback: playback::Playback,
    /// The profile's audio vault (`<vault>/sessions/<session>/audio.*`).
    pub vault: std::path::PathBuf,
}
impl Global for MeetingServices {}

enum Editor {
    Word {
        transcript: Arc<str>,
        base: Word,
        input: Entity<TextInput>,
    },
    Speaker {
        assignment: SpeakerAssignment,
        humans: Vec<(String, String)>,
        segment_ids: Vec<String>,
        all: Option<(i32, i32)>,
        previous_human: Option<String>,
        input: Entity<TextInput>,
        _search: Subscription,
    },
}

pub struct MeetingPane {
    pub context: LaneContext,
    pub intent: MeetingIntent,
    session: Option<SessionId>,
    transcript: Entity<TranscriptView>,
    transcripts: Vec<Transcript>,
    status: Arc<str>,
    error: Option<ServiceError>,
    phase: Phase,
    capture_revision: u64,
    generation: Generation,
    cancellation: CancellationToken,
    editor: Option<Editor>,
    services: Option<MeetingServices>,
    player: Option<Entity<player_view::PlayerView>>,
    ai: Option<Entity<ai_view::AiPane>>,
    ai_open: bool,
    devices: Option<capture::Devices>,
    microphone: String,
    _subscription: Subscription,
    _ai_subscription: Option<Subscription>,
    _poll: Task<()>,
}

impl MeetingPane {
    pub fn new(
        context: LaneContext,
        intent: MeetingIntent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let transcript = cx.new(TranscriptView::new);
        let subscription = cx.subscribe(&transcript, |this, _, event, cx| this.action(event, cx));
        let session = match &intent {
            MeetingIntent::Open(session)
            | MeetingIntent::Start {
                session_id: session,
            } => Some(session.clone()),
            MeetingIntent::Stop => None,
        };
        let services = cx.try_global::<MeetingServices>().cloned();
        let player = services.as_ref().map(|services| {
            cx.new(|cx| player_view::PlayerView::new(services.playback.clone(), cx))
        });
        let ai_services = cx.try_global::<ai_view::AiViewServices>().cloned();
        let ai = session
            .as_ref()
            .zip(ai_services)
            .map(|(session, services)| {
                cx.new(|cx| ai_view::AiPane::new(session.clone(), services, cx))
            });
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        });
        let ai_subscription = ai.as_ref().map(|ai| {
            cx.subscribe(ai, |this, _, event, cx| {
                let ai_view::AiEvent::SummarySaved(document) = event;
                cx.emit(MeetingEvent::NoteEnhanced(document.session_id.clone()));
                this.reload(cx);
            })
        });
        let mut pane = Self {
            context,
            intent: intent.clone(),
            session,
            transcript,
            transcripts: Vec::new(),
            status: "Loading transcript…".into(),
            error: None,
            phase: Phase::Idle,
            capture_revision: 0,
            generation: Generation::default(),
            cancellation: CancellationToken::new(),
            editor: None,
            services,
            player,
            ai,
            ai_open: false,
            devices: None,
            microphone: "Microphone for next recording".into(),
            _subscription: subscription,
            _ai_subscription: ai_subscription,
            _poll: poll,
        };
        pane.reload(cx);
        match intent {
            MeetingIntent::Start { .. } => pane.start(cx),
            MeetingIntent::Stop => pane.stop(cx),
            MeetingIntent::Open(_) => {}
        }
        pane
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        self.cancellation.cancel();
        self.cancellation = CancellationToken::new();
        let generation = self.generation.advance();
        let request =
            TranscriptStore(self.context.runtime.clone()).load(session, self.cancellation.clone());
        cx.spawn(async move |this, cx| {
            let result = match request {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                match result {
                    Ok(snapshot) => {
                        this.transcripts = snapshot.transcripts;
                        this.transcript
                            .update(cx, |view, cx| view.replace(snapshot.segments, cx));
                        this.status = "Transcript".into();
                        this.error = None;
                    }
                    Err(ServiceError::Cancelled) => return,
                    Err(error) => this.failed(error, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let result = self.services.as_ref().ok_or_else(|| ServiceError::Unsupported("Native audio and transcription provider have not been connected by the application.".into()))
            .and_then(|services| {
                services.playback.send(playback::Command::Pause)?;
                services.capture.start(session)
            });
        self.capture_reply(result, cx);
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        let result = self
            .services
            .as_ref()
            .ok_or(ServiceError::Closed)
            .and_then(|services| services.capture.stop());
        self.capture_reply(result, cx);
    }

    fn capture_reply(
        &mut self,
        result: desktop_runtime::Result<
            tokio::sync::oneshot::Receiver<desktop_runtime::Result<()>>,
        >,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(reply) => {
                cx.spawn(async move |this, cx| {
                    if let Err(error) = reply.await.unwrap_or(Err(ServiceError::Closed)) {
                        let _ = this.update(cx, |this, cx| this.failed(error, cx));
                    }
                })
                .detach();
            }
            Err(error) => self.failed(error, cx),
        }
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        let Some(services) = &self.services else {
            return;
        };
        let update = services.capture.take_update(self.capture_revision);
        self.capture_revision = update.revision;
        let playback = services.playback.snapshot();
        if update.session == self.session {
            if update.resync {
                self.reload(cx);
            }
            for (id, segment) in update.changes {
                self.transcript
                    .update(cx, |view, cx| view.live(id, segment, cx));
            }
            if self.phase != update.phase {
                let was_recording = matches!(self.phase, Phase::Listening | Phase::Finalizing);
                self.phase = update.phase;
                if let Some(session_id) = &self.session {
                    cx.emit(MeetingEvent::Recording {
                        session_id: session_id.clone(),
                        active: matches!(
                            self.phase,
                            Phase::Listening | Phase::Loading | Phase::Finalizing
                        ),
                    });
                }
                if was_recording && self.phase == Phase::Idle {
                    self.reload(cx);
                }
                cx.notify();
            }
            if self.status != update.status {
                self.status = update.status;
                cx.notify();
            }
            if let Some(error) = update.error
                && self.error.as_ref().map(ToString::to_string) != Some(error.to_string())
            {
                self.failed(error, cx);
            }
        }
        self.transcript.update(cx, |view, cx| {
            view.playback_position(playback.position.as_millis() as i64, cx)
        });
    }

    fn failed(&mut self, error: ServiceError, cx: &mut Context<Self>) {
        self.error = Some(error.clone());
        cx.emit(MeetingEvent::Failed(error));
        cx.notify();
    }

    fn open_audio(&mut self, cx: &mut Context<Self>) {
        let (Some(services), Some(session), Some(player)) =
            (&self.services, &self.session, &self.player)
        else {
            return;
        };
        let player = player.clone();
        match services.capture.audio_path(session.clone()) {
            Ok(reply) => cx
                .spawn(async move |this, cx| {
                    let result = reply.await.map_err(failure).and_then(|result| result);
                    let _ = this.update(cx, |this, cx| match result {
                        Ok(path) => player.update(cx, |player, cx| player.open(path, cx)),
                        Err(error) => this.failed(error, cx),
                    });
                })
                .detach(),
            Err(error) => self.failed(error, cx),
        }
    }

    fn devices(&mut self, cx: &mut Context<Self>) {
        let Some(services) = &self.services else {
            return;
        };
        if self.devices.is_some() {
            self.devices = None;
            cx.notify();
            return;
        }
        let request = services.capture.devices();
        cx.spawn(async move |this, cx| {
            let result = match request {
                Ok(reply) => reply.await.map_err(failure).and_then(|result| result),
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(devices) => this.devices = Some(devices),
                    Err(error) => this.failed(error, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn microphone(&mut self, device: Option<String>, cx: &mut Context<Self>) {
        let Some(services) = &self.services else {
            return;
        };
        let label = device.clone().unwrap_or_else(|| "Current default".into());
        let request = services.capture.microphone(device);
        cx.spawn(async move |this, cx| {
            let result = match request {
                Ok(reply) => reply.await.map_err(failure).and_then(|result| result),
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.microphone = label;
                        this.devices = None;
                    }
                    Err(error) => this.failed(error, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn action(&mut self, action: &TranscriptAction, cx: &mut Context<Self>) {
        match action {
            TranscriptAction::Seek(time) => {
                if !matches!(self.phase, Phase::Idle | Phase::Failed) {
                    return;
                }
                if let Some(services) = &self.services
                    && let Err(error) =
                        services
                            .playback
                            .send(playback::Command::Seek(Duration::from_millis(
                                (*time).max(0) as u64,
                            )))
                {
                    self.failed(error, cx);
                }
            }
            TranscriptAction::Edit(id) => {
                for transcript in &self.transcripts {
                    if let Some(word) = transcript.words.iter().find(|word| &word.id == id) {
                        let input = cx.new(|cx| TextInput::new("Transcript text", cx));
                        input.update(cx, |input, cx| input.set_text(word.text.clone(), cx));
                        self.editor = Some(Editor::Word {
                            transcript: transcript.id.clone(),
                            base: word.clone(),
                            input,
                        });
                        cx.notify();
                        return;
                    }
                }
            }
            TranscriptAction::Speaker(segment) => {
                let Some(anchor) = segment.words.iter().find_map(|word| word.id.clone()) else {
                    return;
                };
                let Some(transcript) = self
                    .transcripts
                    .iter()
                    .find(|transcript| transcript.words.iter().any(|word| word.id == anchor))
                else {
                    return;
                };
                let ids: Vec<_> = segment
                    .words
                    .iter()
                    .filter_map(|word| word.id.clone())
                    .collect();
                let all = segment
                    .key
                    .speaker_index
                    .map(|speaker| (segment.key.channel as i32, speaker));
                let assignment = SpeakerAssignment {
                    transcript_id: transcript.id.clone(),
                    anchor,
                    human_id: String::new(),
                    scope: all
                        .map(|(channel, speaker_index)| SpeakerScope::All {
                            channel,
                            speaker_index,
                        })
                        .unwrap_or_else(|| SpeakerScope::Segment {
                            word_ids: ids.clone(),
                        }),
                };
                let input = cx.new(|cx| TextInput::new("Search or create a person…", cx));
                let search = cx.subscribe(&input, |this, _, event, cx| {
                    if matches!(event, InputEvent::Changed) {
                        this.search_speakers(cx);
                    }
                });
                self.editor = Some(Editor::Speaker {
                    assignment,
                    humans: Vec::new(),
                    segment_ids: ids,
                    all,
                    previous_human: segment.key.speaker_human_id.clone(),
                    input,
                    _search: search,
                });
                self.search_speakers(cx);
                cx.notify();
            }
        }
    }

    fn search_speakers(&mut self, cx: &mut Context<Self>) {
        let Some(Editor::Speaker { input, .. }) = &self.editor else {
            return;
        };
        let query = input.read(cx).buffer.text.trim().to_owned();
        let expected = query.clone();
        let session = self.session.clone();
        let request = self.context.runtime.read(self.cancellation.clone(), move |services| async move {
                    let rows = services.executor.execute("SELECT h.id, h.name FROM humans h WHERE h.deleted_at IS NULL AND (instr(lower(h.name), lower(?)) > 0 OR instr(lower(h.email), lower(?)) > 0) ORDER BY EXISTS(SELECT 1 FROM session_participants p WHERE p.session_id = ? AND p.human_id = h.id AND p.deleted_at IS NULL) DESC, h.name, h.id LIMIT 200".into(), vec![serde_json::json!(query), serde_json::json!(query), serde_json::json!(session)]).await.map_err(failure)?;
                    rows.iter().map(|row| Ok((store::string(row, "id")?.to_owned(), store::string(row, "name")?.to_owned()))).collect::<desktop_runtime::Result<Vec<_>>>()
                });
        cx.spawn(async move |this, cx| {
            let result = match request {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(humans) => {
                        if let Some(Editor::Speaker {
                            humans: current,
                            input,
                            ..
                        }) = &mut this.editor
                            && input.read(cx).buffer.text.trim() == expected
                        {
                            *current = humans;
                        }
                    }
                    Err(error) => this.failed(error, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save_editor(&mut self, delete: bool, cx: &mut Context<Self>) {
        let store = TranscriptStore(self.context.runtime.clone());
        let request = match &self.editor {
            Some(Editor::Word {
                transcript,
                base,
                input,
            }) => store.edit_word(
                transcript.clone(),
                base.clone(),
                (!delete).then(|| input.read(cx).buffer.text.clone()),
            ),
            Some(Editor::Speaker {
                assignment,
                previous_human,
                input,
                ..
            }) => {
                let Some(session) = self.session.clone() else {
                    return;
                };
                let name = input.read(cx).buffer.text.trim().to_owned();
                let mut assignment = assignment.clone();
                let new_name = if assignment.human_id.is_empty() {
                    if name.is_empty() {
                        return;
                    }
                    assignment.human_id = uuid::Uuid::new_v4().to_string();
                    Some(name)
                } else {
                    None
                };
                let previous = matches!(assignment.scope, SpeakerScope::All { .. })
                    .then(|| previous_human.clone())
                    .flatten();
                store.assign_participant(session, assignment, previous, new_name)
            }
            _ => return,
        };
        cx.spawn(async move |this, cx| {
            let result = match request {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.editor = None;
                    this.reload(cx);
                }
                Err(error) => this.failed(error, cx),
            });
        })
        .detach();
    }
}

impl Drop for MeetingPane {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

impl EventEmitter<MeetingEvent> for MeetingPane {}

impl Render for MeetingPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let running = matches!(self.phase, Phase::Listening | Phase::Loading);
        let mut panel = div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .text_color(colors.foreground)
            .bg(colors.background)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .w_full()
                    .gap_3()
                    .p_3()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .child(SharedString::from(self.status.clone())),
                    )
                    .when(self.phase != Phase::Finalizing, |view| {
                        view.child(
                            div()
                                .id("capture")
                                .cursor_pointer()
                                .child(if running {
                                    "Stop listening"
                                } else {
                                    "Start / resume listening"
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if running {
                                        this.stop(cx);
                                    } else {
                                        this.start(cx);
                                    }
                                })),
                        )
                    })
                    .child(
                        div()
                            .id("reload-transcript")
                            .cursor_pointer()
                            .child("Reload")
                            .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
                    )
                    .child(div().id("meeting-ai").cursor_pointer().child(if self.ai_open { "Transcript" } else { "Chat / summary" }).on_click(cx.listener(|this, _, _, cx| {
                        if this.ai.is_some() {
                            this.ai_open = !this.ai_open;
                            if this.ai_open && let Some(services) = &this.services { let _ = services.playback.send(playback::Command::Pause); }
                            cx.notify();
                        } else { this.failed(ServiceError::Unsupported("Meeting AI provider and context services have not been installed.".into()), cx); }
                    })))
                    .when(!running && self.phase != Phase::Finalizing, |view| {
                        view.child(div().id("microphone").max_w_full().truncate().cursor_pointer().child(self.microphone.clone()).on_click(cx.listener(|this, _, _, cx| this.devices(cx))))
                    })
                    .when(!self.ai_open && !running && self.phase != Phase::Finalizing, |view| {
                        view.child(
                            div()
                                .id("open-audio")
                                .cursor_pointer()
                                .child("Open audio")
                                .on_click(cx.listener(|this, _, _, cx| this.open_audio(cx))),
                        )
                    }),
            )
            .when_some(self.error.as_ref(), |view, error| {
                view.child(
                    div()
                        .p_3()
                        .text_color(colors.destructive)
                        .child(error.to_string()),
                )
            })
            .when_some(self.devices.as_ref(), |view, devices| {
                let mut selector = div().flex().flex_wrap().gap_2().p_2().child(div().id("default-microphone").cursor_pointer()
                    .child(format!("Current default ({})", devices.default))
                    .on_click(cx.listener(|this, _, _, cx| this.microphone(None, cx))));
                for microphone in &devices.microphones {
                    let device = microphone.clone();
                    selector = selector.child(div().id(SharedString::from(microphone.clone())).cursor_pointer().child(microphone.clone())
                        .on_click(cx.listener(move |this, _, _, cx| this.microphone(Some(device.clone()), cx))));
                }
                view.child(selector)
            })
            .when(!self.ai_open && !running && self.phase != Phase::Finalizing, |view| {
                view.when_some(self.player.as_ref(), |view, player| {
                    view.child(player.clone())
                })
            })
            .when(!self.ai_open, |view| view.child(div().flex_1().min_h_0().child(self.transcript.clone())))
            .when(self.ai_open, |view| view.when_some(self.ai.as_ref(), |view, ai| view.child(div().flex().flex_1().min_h_0().child(ai.clone()))));
        if let Some(editor) = &self.editor {
            let mut form = div()
                .p_3()
                .border_t_1()
                .border_color(colors.border)
                .flex()
                .flex_col()
                .gap_2();
            form = match editor {
                Editor::Word { input, .. } => form
                    .child("Edit transcript word")
                    .child(input.clone())
                    .child(
                        div()
                            .id("delete-word")
                            .cursor_pointer()
                            .child("Delete word")
                            .on_click(cx.listener(|this, _, _, cx| this.save_editor(true, cx))),
                    ),
                Editor::Speaker {
                    assignment,
                    humans,
                    all,
                    input,
                    ..
                } => {
                    let mut selector = div()
                        .id("speaker-results")
                        .max_h(gpui::px(180.))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_1();
                    for (id, name) in humans {
                        let human = id.clone();
                        selector = selector.child(
                            div()
                                .id(SharedString::from(id.clone()))
                                .px_2()
                                .cursor_pointer()
                                .when(assignment.human_id == *id, |view| view.bg(colors.accent))
                                .child(name.clone())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(Editor::Speaker { assignment, .. }) =
                                        &mut this.editor
                                    {
                                        assignment.human_id = human.clone();
                                    }
                                    cx.notify();
                                })),
                        );
                    }
                    let name = input.read(cx).buffer.text.trim();
                    form.child("Assign speaker")
                        .child(input.clone())
                        .child(selector)
                        .when(!name.is_empty(), |form| {
                            form.child(
                                div()
                                    .id("create-speaker")
                                    .cursor_pointer()
                                    .child(format!("Create “{name}”"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(Editor::Speaker { assignment, .. }) =
                                            &mut this.editor
                                        {
                                            assignment.human_id.clear();
                                        }
                                        this.save_editor(false, cx);
                                    })),
                            )
                        })
                        .child(
                            div()
                                .id("scope")
                                .cursor_pointer()
                                .child(if matches!(assignment.scope, SpeakerScope::All { .. }) {
                                    "All matching speaker occurrences"
                                } else {
                                    "Only this segment"
                                })
                                .when(all.is_some(), |view| {
                                    view.on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(Editor::Speaker {
                                            assignment,
                                            segment_ids,
                                            all,
                                            ..
                                        }) = &mut this.editor
                                        {
                                            assignment.scope = match assignment.scope {
                                                SpeakerScope::All { .. } => SpeakerScope::Segment {
                                                    word_ids: segment_ids.clone(),
                                                },
                                                SpeakerScope::Segment { .. } => all
                                                    .map(|(channel, speaker_index)| {
                                                        SpeakerScope::All {
                                                            channel,
                                                            speaker_index,
                                                        }
                                                    })
                                                    .unwrap_or_else(|| SpeakerScope::Segment {
                                                        word_ids: segment_ids.clone(),
                                                    }),
                                            };
                                        }
                                        cx.notify();
                                    }))
                                }),
                        )
                }
            };
            panel = panel.child(
                form.child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            div()
                                .id("confirm-edit")
                                .cursor_pointer()
                                .child("Confirm")
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.save_editor(false, cx)),
                                ),
                        )
                        .child(
                            div()
                                .id("cancel-edit")
                                .cursor_pointer()
                                .child("Cancel")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.editor = None;
                                    cx.notify();
                                })),
                        ),
                ),
            );
        }
        panel
    }
}
