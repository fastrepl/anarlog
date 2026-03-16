use std::sync::Arc;

use hypr_listener_core::actors::{RootActor, RootArgs, RootMsg, SessionParams};
use hypr_listener_core::{RecordingMode, StopSessionParams, TranscriptionMode};
use ractor::Actor;
use tokio::sync::mpsc;

pub use crate::cli::AudioMode;
use crate::commands::SttGlobalArgs;
use crate::config::desktop;
use crate::config::stt::{ResolvedSttConfig, resolve_config};
use crate::error::{CliError, CliResult};
use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};

mod action;
mod app;
mod audio_drop;
mod effect;
mod runtime;
mod ui;

use action::Action;
use app::App;
use audio_drop::AudioDropRequest;
use effect::Effect;
use runtime::Runtime;

use crate::commands::batch::runtime::ChannelBatchRuntime;

pub struct Args {
    pub stt: SttGlobalArgs,
    pub record: bool,
    pub audio: AudioMode,
}

fn spawn_batch_transcription(
    request: AudioDropRequest,
    batch_runtime: Arc<ChannelBatchRuntime>,
    resolved: &ResolvedSttConfig,
) {
    let batch_session_id = uuid::Uuid::new_v4().to_string();
    let params = resolved.to_batch_params(batch_session_id, request.file_path, vec![]);

    tokio::spawn(async move {
        let _ = hypr_listener2_core::run_batch(batch_runtime, params).await;
    });
}

use crate::output::format_hhmmss;

const ANIMATION_FRAME: std::time::Duration = std::time::Duration::from_millis(33);
const IDLE_FRAME: std::time::Duration = std::time::Duration::from_secs(1);

struct Output {
    elapsed: std::time::Duration,
    force_quit: bool,
}

enum ExternalEvent {
    Listener(runtime::RuntimeEvent),
    Batch(hypr_listener2_core::BatchEvent),
}

struct ListenScreen {
    app: App,
    batch_runtime: Arc<ChannelBatchRuntime>,
    resolved: ResolvedSttConfig,
}

impl ListenScreen {
    fn new(batch_runtime: Arc<ChannelBatchRuntime>, resolved: ResolvedSttConfig) -> Self {
        Self {
            app: App::new(),
            batch_runtime,
            resolved,
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<Output> {
        for effect in effects {
            match effect {
                Effect::StartBatch(request) => {
                    spawn_batch_transcription(request, self.batch_runtime.clone(), &self.resolved);
                }
                Effect::Exit { force } => {
                    return ScreenControl::Exit(Output {
                        elapsed: self.app.elapsed(),
                        force_quit: force,
                    });
                }
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for ListenScreen {
    type ExternalEvent = ExternalEvent;
    type Output = Output;

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(pasted) => {
                let effects = self.app.dispatch(Action::Paste(pasted));
                self.apply_effects(effects)
            }
            TuiEvent::Draw => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let action = match event {
            ExternalEvent::Listener(event) => Action::RuntimeEvent(event),
            ExternalEvent::Batch(event) => Action::BatchEvent(event),
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        format!(
            "char: {} ({})",
            self.app.status(),
            format_hhmmss(self.app.elapsed())
        )
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        if self.app.has_active_animations() {
            ANIMATION_FRAME
        } else {
            IDLE_FRAME
        }
    }
}

pub async fn run(args: Args) -> CliResult<()> {
    let Args {
        stt,
        record,
        audio: audio_mode,
    } = args;

    let resolved = resolve_config(
        stt.provider,
        stt.base_url,
        stt.api_key,
        stt.model,
        stt.language,
    )
    .await?;
    let _ = resolved.server.as_ref();
    let languages = vec![resolved.language.clone()];

    let session_id = uuid::Uuid::new_v4().to_string();
    let session_label = session_id.clone();
    let vault_base = desktop::resolve_paths().vault_base;

    let (listener_tx, mut listener_rx) = tokio::sync::mpsc::unbounded_channel();
    let runtime = Arc::new(Runtime::new(vault_base, listener_tx));

    let audio: Arc<dyn hypr_audio_actual::AudioProvider> = match audio_mode {
        AudioMode::Dual => Arc::new(hypr_audio_actual::ActualAudio),
        #[cfg(feature = "mock-audio")]
        AudioMode::Mock => Arc::new(hypr_audio_mock::MockAudio::new(1)),
    };

    let (root_ref, _handle) = Actor::spawn(
        Some(RootActor::name()),
        RootActor,
        RootArgs {
            runtime: runtime.clone(),
            audio,
        },
    )
    .await
    .map_err(|e| CliError::operation_failed("spawn root actor", e.to_string()))?;

    let params = SessionParams {
        session_id,
        languages,
        onboarding: false,
        transcription_mode: TranscriptionMode::Live,
        recording_mode: if record {
            RecordingMode::Disk
        } else {
            RecordingMode::Memory
        },
        model: resolved.model.clone(),
        base_url: resolved.base_url.clone(),
        api_key: resolved.api_key.clone(),
        keywords: vec![],
    };

    ractor::call!(root_ref, RootMsg::StartSession, params)
        .map_err(|e| CliError::operation_failed("start session", e.to_string()))?
        .map_err(|e| CliError::operation_failed("start session", format!("{e:?}")))?;

    let (batch_tx, mut batch_rx) = mpsc::unbounded_channel();
    let batch_runtime = Arc::new(ChannelBatchRuntime { tx: batch_tx });
    let (external_tx, external_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(listener_event) = listener_rx.recv() => {
                    if external_tx.send(ExternalEvent::Listener(listener_event)).is_err() {
                        break;
                    }
                }
                Some(batch_event) = batch_rx.recv() => {
                    if external_tx.send(ExternalEvent::Batch(batch_event)).is_err() {
                        break;
                    }
                }
                else => break,
            }
        }
    });

    let output = run_screen(
        ListenScreen::new(batch_runtime, resolved),
        Some(external_rx),
    )
    .await
    .map_err(|e| CliError::operation_failed("listen tui", e.to_string()))?;

    print_exit_summary(&session_label, output.elapsed);

    if !output.force_quit {
        let _ = ractor::call!(root_ref, RootMsg::StopSession, StopSessionParams::default());
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    Ok(())
}

fn print_exit_summary(session_id: &str, elapsed: std::time::Duration) {
    let dim = "\x1b[2m";
    let reset = "\x1b[0m";
    let bold = "\x1b[1m";
    let cyan = "\x1b[36m";

    let chat_cmd = format!("char chat --session {session_id} --api-key <KEY> --model <MODEL>");
    let session_line = format!("Session   {session_id}");
    let duration_line = format!("Duration  {}", format_hhmmss(elapsed));
    let chat_label = "Chat with this session:";

    let inner_width = [
        session_line.len(),
        duration_line.len(),
        chat_label.len(),
        chat_cmd.len(),
    ]
    .into_iter()
    .max()
    .unwrap()
        + 2;

    let top = format!("  ╭{}╮", "─".repeat(inner_width));
    let bot = format!("  ╰{}╯", "─".repeat(inner_width));
    let sep = format!("  ├{}┤", "─".repeat(inner_width));
    let empty = format!("  │{}│", " ".repeat(inner_width));

    let pad = |s: &str, visible_len: usize| {
        let padding = inner_width - 1 - visible_len;
        format!(" {s}{}", " ".repeat(padding))
    };

    println!();
    println!("{top}");
    println!(
        "  │{}│",
        pad(&format!("{dim}{session_line}{reset}"), session_line.len())
    );
    println!(
        "  │{}│",
        pad(&format!("{dim}{duration_line}{reset}"), duration_line.len())
    );
    println!("{sep}");
    println!(
        "  │{}│",
        pad(&format!("{dim}{chat_label}{reset}"), chat_label.len())
    );
    println!("{empty}");
    println!(
        "  │{}│",
        pad(&format!("{bold}{cyan}{chat_cmd}{reset}"), chat_cmd.len())
    );
    println!("{bot}");
    println!();
}
