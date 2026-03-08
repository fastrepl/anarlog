use std::sync::Arc;

use listener_core::{
    ListenerRuntime, SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent,
    SessionProgressEvent,
    actors::{RootActor, RootArgs, RootMsg, SessionParams},
};
use ractor::Actor;

struct CliRuntime {
    vault_base: std::path::PathBuf,
}

impl hypr_storage::StorageRuntime for CliRuntime {
    fn global_base(&self) -> Result<std::path::PathBuf, hypr_storage::Error> {
        Ok(self.vault_base.clone())
    }

    fn vault_base(&self) -> Result<std::path::PathBuf, hypr_storage::Error> {
        Ok(self.vault_base.clone())
    }
}

impl ListenerRuntime for CliRuntime {
    fn emit_lifecycle(&self, event: SessionLifecycleEvent) {
        match &event {
            SessionLifecycleEvent::Active { session_id, error } => {
                if let Some(err) = error {
                    eprintln!("[lifecycle] active (degraded) session={session_id} error={err:?}");
                } else {
                    eprintln!("[lifecycle] active session={session_id}");
                }
            }
            SessionLifecycleEvent::Inactive { session_id, error } => {
                eprintln!("[lifecycle] inactive session={session_id} error={error:?}");
            }
            SessionLifecycleEvent::Finalizing { session_id } => {
                eprintln!("[lifecycle] finalizing session={session_id}");
            }
        }
    }

    fn emit_progress(&self, event: SessionProgressEvent) {
        match &event {
            SessionProgressEvent::AudioInitializing { .. } => {
                eprintln!("[progress] audio initializing...");
            }
            SessionProgressEvent::AudioReady { device, .. } => {
                eprintln!("[progress] audio ready device={device:?}");
            }
            SessionProgressEvent::Connecting { .. } => {
                eprintln!("[progress] connecting to STT...");
            }
            SessionProgressEvent::Connected { adapter, .. } => {
                eprintln!("[progress] connected via {adapter}");
            }
        }
    }

    fn emit_error(&self, event: SessionErrorEvent) {
        match &event {
            SessionErrorEvent::AudioError { error, device, .. } => {
                eprintln!("[error] audio: {error} device={device:?}");
            }
            SessionErrorEvent::ConnectionError { error, .. } => {
                eprintln!("[error] connection: {error}");
            }
        }
    }

    fn emit_data(&self, event: SessionDataEvent) {
        match &event {
            SessionDataEvent::AudioAmplitude { mic, speaker, .. } => {
                let mic_bar = "█".repeat((*mic as usize) / 50);
                let spk_bar = "█".repeat((*speaker as usize) / 50);
                eprint!("\r[audio] mic {mic_bar:<20} | spk {spk_bar:<20}");
            }
            SessionDataEvent::StreamResponse { response, .. } => {
                println!("{}", serde_json::to_string(&response).unwrap_or_default());
            }
            SessionDataEvent::MicMuted { value, .. } => {
                eprintln!("[data] mic muted={value}");
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let base_url = std::env::var("LISTENER_BASE_URL").unwrap_or_else(|_| {
        eprintln!("Usage: LISTENER_BASE_URL=... LISTENER_API_KEY=... cargo run --example cli");
        eprintln!();
        eprintln!("  LISTENER_BASE_URL   STT provider URL (required)");
        eprintln!("  LISTENER_API_KEY    API key (default: empty)");
        eprintln!("  LISTENER_MODEL      Model name (default: empty)");
        eprintln!("  LISTENER_LANGUAGE   Language code (default: en)");
        eprintln!("  LISTENER_RECORD     Enable WAV recording (default: false)");
        std::process::exit(1);
    });

    let api_key = std::env::var("LISTENER_API_KEY").unwrap_or_default();
    let model = std::env::var("LISTENER_MODEL").unwrap_or_default();
    let language = std::env::var("LISTENER_LANGUAGE").unwrap_or_else(|_| "en".into());
    let record_enabled = std::env::var("LISTENER_RECORD")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);

    let languages = vec![
        language
            .parse::<hypr_language::Language>()
            .expect("invalid language code"),
    ];

    let session_id = uuid::Uuid::new_v4().to_string();
    let vault_base = std::env::temp_dir().join("listener-cli");

    let runtime = Arc::new(CliRuntime { vault_base });

    let (root_ref, _handle) = Actor::spawn(
        Some(RootActor::name()),
        RootActor,
        RootArgs {
            runtime: runtime.clone(),
        },
    )
    .await
    .expect("failed to spawn root actor");

    eprintln!("Starting session {session_id}...");
    eprintln!("Press Ctrl+C to stop.");
    eprintln!();

    let params = SessionParams {
        session_id: session_id.clone(),
        languages,
        onboarding: false,
        record_enabled,
        model,
        base_url,
        api_key,
        keywords: vec![],
    };

    let started = ractor::call!(root_ref, RootMsg::StartSession, params)
        .expect("failed to send start message");

    if !started {
        eprintln!("Failed to start session");
        std::process::exit(1);
    }

    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl+c");

    eprintln!();
    eprintln!("Stopping session...");

    let _ = ractor::call!(root_ref, RootMsg::StopSession);

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    eprintln!("Done.");
}
