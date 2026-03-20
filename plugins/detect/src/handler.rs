use tauri::{AppHandle, Manager, Runtime};
use tokio_util::sync::CancellationToken;

use crate::{
    DetectEvent, ProcessorState,
    env::{Env, TauriEnv},
    mic_usage_tracker,
    policy::{MicEventType, PolicyContext},
};

pub fn setup<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    let env = TauriEnv {
        app_handle: app.app_handle().clone(),
    };
    let processor = app.state::<ProcessorState>().inner().clone();

    let callback = hypr_detect::new_callback(move |event| {
        let env = env.clone();
        let processor = processor.clone();
        tauri::async_runtime::spawn(async move {
            handle_detect_event(&env, &processor, event);
        });
    });

    let detector_state = app.state::<crate::DetectorState>();
    let mut detector = detector_state.lock().unwrap_or_else(|e| e.into_inner());
    detector.start(callback);
    drop(detector);

    Ok(())
}

pub fn handle_detect_event<E: Env>(
    env: &E,
    state: &ProcessorState,
    event: hypr_detect::DetectEvent,
) {
    match event {
        hypr_detect::DetectEvent::MicStarted(apps) => {
            if !env.is_detect_enabled() {
                return;
            }
            handle_mic_started(env, state, apps);
        }
        hypr_detect::DetectEvent::MicStopped(apps) => {
            if !env.is_detect_enabled() {
                let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
                for app in &apps {
                    guard.mic_usage_tracker.cancel_app(&app.id);
                }
                return;
            }
            handle_mic_stopped(env, state, apps);
        }
        #[cfg(all(target_os = "macos", feature = "zoom"))]
        hypr_detect::DetectEvent::ZoomMuteStateChanged { value } => {
            env.emit(DetectEvent::MicMuteStateChanged { value });
        }
        #[cfg(all(target_os = "macos", feature = "sleep"))]
        hypr_detect::DetectEvent::SleepStateChanged { value } => {
            env.emit(DetectEvent::SleepStateChanged { value });
        }
    }
}

fn handle_mic_started<E: Env>(
    env: &E,
    state: &ProcessorState,
    apps: Vec<hypr_detect::InstalledApp>,
) {
    let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());

    let to_track: Vec<_> = apps
        .iter()
        .filter(|app| {
            guard.policy.should_track_app(&app.id)
                && !guard.mic_usage_tracker.is_tracking(&app.id)
                && !guard.mic_usage_tracker.is_in_cooldown(&app.id)
        })
        .cloned()
        .collect();

    let threshold_secs = guard.mic_active_threshold_secs;

    for app in &to_track {
        let token = CancellationToken::new();
        let generation = guard
            .mic_usage_tracker
            .start_tracking(app.id.clone(), token.clone());
        mic_usage_tracker::spawn_timer(
            env.clone(),
            state.clone(),
            app.clone(),
            generation,
            token,
            threshold_secs,
        );
    }
}

fn handle_mic_stopped<E: Env>(
    env: &E,
    state: &ProcessorState,
    apps: Vec<hypr_detect::InstalledApp>,
) {
    let is_dnd = env.is_do_not_disturb();

    let policy_result = {
        let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());

        for app in &apps {
            guard.mic_usage_tracker.cancel_app(&app.id);
        }

        let ctx = PolicyContext {
            apps: &apps,
            is_dnd,
            event_type: MicEventType::Stopped,
        };
        guard.policy.evaluate(&ctx)
    };

    match policy_result {
        Ok(result) => {
            env.emit(DetectEvent::MicStopped {
                apps: result.filtered_apps,
            });
        }
        Err(reason) => {
            tracing::info!(?reason, "skip_mic_stopped");
        }
    }
}
