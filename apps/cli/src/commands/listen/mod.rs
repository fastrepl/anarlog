use std::sync::Arc;

use hypr_listener_core::actors::{RootActor, RootArgs, RootMsg, SessionParams};
use hypr_listener_core::{AudioRetention, StopSessionParams};
use hypr_listener2_core::{BatchParams, BatchProvider};
use ractor::Actor;
use tokio::sync::mpsc;

use crate::commands::cactus_server::resolve_and_spawn_cactus;
use crate::error::{CliError, CliResult};
use crate::{
    event::{EventHandler, TuiEvent},
    frame::FrameRequester,
    terminal::TerminalGuard,
};

mod app;
mod audio_drop;
mod runtime;
mod ui;

use app::App;
use audio_drop::AudioDropRequest;
use runtime::{ListenBatchRuntime, ListenRuntime};

pub struct Args {
    pub base_url: Option<String>,
    pub api_key: String,
    pub model: String,
    pub language: String,
    pub record: bool,
}

fn spawn_batch_transcription(
    request: AudioDropRequest,
    batch_runtime: Arc<ListenBatchRuntime>,
    base_url: String,
    api_key: String,
    model: String,
    language: hypr_language::Language,
) {
    let batch_session_id = uuid::Uuid::new_v4().to_string();
    let params = BatchParams {
        session_id: batch_session_id.clone(),
        provider: BatchProvider::Am,
        file_path: request.file_path,
        model: if model.is_empty() { None } else { Some(model) },
        base_url,
        api_key,
        languages: vec![language],
        keywords: vec![],
    };

    tokio::spawn(async move {
        let _ = hypr_listener2_core::run_batch(batch_runtime, params).await;
    });
}

pub async fn run(args: Args) -> CliResult<()> {
    let Args {
        base_url,
        api_key,
        model,
        language: language_code,
        record,
    } = args;

    let language = language_code
        .parse::<hypr_language::Language>()
        .map_err(|e| {
            CliError::invalid_argument("--language", language_code.clone(), e.to_string())
        })?;
    let languages = vec![language.clone()];

    let is_cactus = model.starts_with("cactus-") || model == "cactus";

    let _server;
    let base_url = if is_cactus {
        let model_name = if model == "cactus" {
            None
        } else {
            Some(model.as_str())
        };
        let (server, url) = resolve_and_spawn_cactus(model_name).await?;
        _server = Some(server);
        url
    } else {
        _server = None;
        base_url.ok_or_else(|| CliError::required_argument("--base-url (or CHAR_BASE_URL)"))?
    };

    let session_id = uuid::Uuid::new_v4().to_string();
    let session_label = session_id.clone();
    let vault_base = std::env::temp_dir().join("char-cli");

    let (listener_tx, mut listener_rx) = tokio::sync::mpsc::unbounded_channel();
    let runtime = Arc::new(ListenRuntime::new(vault_base, listener_tx));

    let (root_ref, _handle) = Actor::spawn(
        Some(RootActor::name()),
        RootActor,
        RootArgs {
            runtime: runtime.clone(),
        },
    )
    .await
    .map_err(|e| CliError::external_action_failed("spawn root actor", e.to_string()))?;

    let params = SessionParams {
        session_id,
        languages,
        onboarding: false,
        audio_retention: if record {
            AudioRetention::Disk
        } else {
            AudioRetention::Memory
        },
        model: model.clone(),
        base_url: base_url.clone(),
        api_key: api_key.clone(),
        keywords: vec![],
    };

    let started = ractor::call!(root_ref, RootMsg::StartSession, params)
        .map_err(|e| CliError::operation_failed("start session", e.to_string()))?;

    if !started {
        return Err(CliError::operation_failed(
            "start session",
            "session did not transition to active",
        ));
    }

    let mut terminal = TerminalGuard::new();
    let (draw_tx, draw_rx) = tokio::sync::broadcast::channel(16);
    let (batch_tx, mut batch_rx) = mpsc::unbounded_channel();
    let batch_runtime = Arc::new(ListenBatchRuntime {
        tx: batch_tx.clone(),
    });
    let frame_requester = FrameRequester::new(draw_tx);
    let mut app = App::new(frame_requester.clone());
    let mut events = EventHandler::new(draw_rx);
    events.resume_events();

    frame_requester.schedule_frame();

    loop {
        tokio::select! {
            Some(tui_event) = events.next() => {
                match tui_event {
                    TuiEvent::Key(key) => app.handle_key(key),
                    TuiEvent::Paste(pasted) => {
                        if let Some(request) = app.handle_paste(pasted) {
                            spawn_batch_transcription(
                                request,
                                batch_runtime.clone(),
                                base_url.clone(),
                                api_key.clone(),
                                model.clone(),
                                language.clone(),
                            );
                        }
                    }
                    TuiEvent::Draw => {
                        terminal
                            .terminal_mut()
                            .draw(|frame| ui::draw(frame, &mut app))
                            .ok();
                        frame_requester.schedule_frame_in(std::time::Duration::from_secs(1));
                    }
                }
            }
            Some(listener_event) = listener_rx.recv() => {
                app.handle_listener_event(listener_event);
            }
            Some(batch_event) = batch_rx.recv() => {
                app.handle_batch_event(batch_event);
            }
            else => break,
        }

        if app.should_quit {
            break;
        }
    }

    let elapsed = app.elapsed();
    let word_count = app.words.len();

    events.pause_events();
    drop(terminal);

    print_exit_summary(&session_label, elapsed, word_count);

    let _ = ractor::call!(root_ref, RootMsg::StopSession, StopSessionParams::default());
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    Ok(())
}

fn print_exit_summary(session_id: &str, elapsed: std::time::Duration, word_count: usize) {
    let secs = elapsed.as_secs();
    let duration = format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    );

    println!();
    println!("\x1b[2mSession\x1b[0m   {session_id}");
    println!("\x1b[2mDuration\x1b[0m  {duration}");
    println!("\x1b[2mWords\x1b[0m     {word_count}");
    println!();
}
