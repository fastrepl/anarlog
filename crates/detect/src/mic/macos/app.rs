use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::{DetectEvent, InstalledApp};

use super::SharedContext;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

pub(super) fn spawn(ctx: SharedContext) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(POLL_INTERVAL);

            if !ctx.polling_active.load(Ordering::SeqCst) {
                continue;
            }

            let current_apps = crate::list_mic_using_apps();
            let Ok(mut state_guard) = ctx.state.lock() else {
                continue;
            };

            let (started, stopped) = diff_apps(&state_guard.active_apps, &current_apps);
            state_guard.active_apps = current_apps;
            drop(state_guard);

            if !started.is_empty() {
                let event = DetectEvent::MicStarted(started);
                tracing::info!(?event, "detected_via_polling");
                if let Ok(guard) = ctx.callback.lock() {
                    (*guard)(event);
                }
            }

            if !stopped.is_empty() {
                let event = DetectEvent::MicStopped(stopped);
                tracing::info!(?event, "detected_via_polling");
                if let Ok(guard) = ctx.callback.lock() {
                    (*guard)(event);
                }
            }
        }
    });
}

fn diff_apps(
    previous: &[InstalledApp],
    current: &[InstalledApp],
) -> (Vec<InstalledApp>, Vec<InstalledApp>) {
    let previous_ids: HashSet<_> = previous.iter().map(|app| &app.id).collect();
    let current_ids: HashSet<_> = current.iter().map(|app| &app.id).collect();

    let started = current
        .iter()
        .filter(|app| !previous_ids.contains(&app.id))
        .cloned()
        .collect();

    let stopped = previous
        .iter()
        .filter(|app| !current_ids.contains(&app.id))
        .cloned()
        .collect();

    (started, stopped)
}
