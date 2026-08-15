use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cidre::{ax, ns};

use crate::{BackgroundTask, DetectCallback, DetectEvent};

const ZOOM_BUNDLE_ID: &str = "us.zoom.xos";
const ACTIVE_POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Default)]
pub(crate) struct ZoomMicUsage {
    active: Arc<AtomicBool>,
    changed: Arc<tokio::sync::Notify>,
}

impl ZoomMicUsage {
    pub(crate) fn set(&self, active: bool) {
        if self.active.swap(active, Ordering::SeqCst) != active {
            self.changed.notify_one();
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    pub(crate) fn update_from_mic_event(&self, event: &DetectEvent) {
        match event {
            DetectEvent::MicStarted(apps) if apps.iter().any(|app| app.id == ZOOM_BUNDLE_ID) => {
                self.set(true);
            }
            DetectEvent::MicStopped(apps) if apps.iter().any(|app| app.id == ZOOM_BUNDLE_ID) => {
                self.set(false);
            }
            _ => {}
        }
    }
}

#[derive(Default)]
pub struct ZoomMuteWatcher {
    background: BackgroundTask,
}

struct WatcherState {
    last_mute_state: Option<bool>,
}

impl WatcherState {
    fn new() -> Self {
        Self {
            last_mute_state: None,
        }
    }
}

fn find_zoom_pid() -> Option<i32> {
    let bundle_id = ns::String::with_str(ZOOM_BUNDLE_ID);
    let apps = ns::RunningApp::with_bundle_id(&bundle_id);
    let app = apps.get(0).ok()?;
    Some(app.pid())
}

fn ax_element_title(elem: &ax::UiElement) -> Option<String> {
    let value = elem.attr_value(ax::attr::title()).ok()?;
    let s = value.try_as_string()?;
    Some(s.to_string())
}

fn check_zoom_mute_state() -> Option<bool> {
    let pid = find_zoom_pid()?;
    let app = ax::UiElement::with_app_pid(pid);

    let children = app.children().ok()?;
    let menu_bar = children.iter().find(|child| {
        child
            .role()
            .ok()
            .map(|r| r.equal(ax::role::menu_bar()))
            .unwrap_or(false)
    })?;

    let menu_bar_items = menu_bar.children().ok()?;
    let meeting_item = menu_bar_items.iter().find(|item| {
        ax_element_title(item)
            .map(|t| t == "Meeting")
            .unwrap_or(false)
    })?;

    let menu_children = meeting_item.children().ok()?;
    let meeting_menu = menu_children.iter().next()?;

    let menu_items = meeting_menu.children().ok()?;
    for item in menu_items.iter() {
        if let Some(title) = ax_element_title(item) {
            match title.as_str() {
                "Mute Audio" | "Mute audio" => return Some(false),
                "Unmute Audio" | "Unmute audio" => return Some(true),
                _ => continue,
            }
        }
    }

    tracing::debug!("zoom mute state unknown (likely not in meeting)");
    None
}

fn is_zoom_using_mic() -> Result<bool, crate::Error> {
    crate::list_mic_using_apps().map(|apps| apps.iter().any(|app| app.id == ZOOM_BUNDLE_ID))
}

fn reconcile_zoom_mute_state(
    state: &mut WatcherState,
    mic_usage: Result<bool, crate::Error>,
    mute_state: Option<bool>,
) -> Option<DetectEvent> {
    match mic_usage {
        Ok(false) => {
            if state.last_mute_state.is_some() {
                tracing::debug!("zoom no longer using mic, clearing state");
                state.last_mute_state = None;
            }
            None
        }
        Err(error) => {
            tracing::warn!(?error, "zoom_mic_usage_check_failed");
            None
        }
        Ok(true) => {
            let muted = mute_state?;

            if state.last_mute_state == Some(muted) {
                return None;
            }

            tracing::info!(muted = muted, "zoom mute state changed");
            state.last_mute_state = Some(muted);
            Some(DetectEvent::ZoomMuteStateChanged { value: muted })
        }
    }
}

impl crate::Observer for ZoomMuteWatcher {
    fn start(&mut self, f: DetectCallback) {
        self.start_inner(f, None);
    }

    fn stop(&mut self) {
        self.background.stop();
    }
}

impl ZoomMuteWatcher {
    pub(crate) fn start_with_mic_usage(&mut self, f: DetectCallback, mic_usage: ZoomMicUsage) {
        self.start_inner(f, Some(mic_usage));
    }

    fn start_inner(&mut self, f: DetectCallback, mic_usage: Option<ZoomMicUsage>) {
        if self.background.is_running() {
            return;
        }

        if !macos_accessibility_client::accessibility::application_is_trusted() {
            return;
        }

        self.background.start(move |running, mut rx| async move {
            let mut state = WatcherState::new();
            if let Some(mic_usage) = &mic_usage {
                mic_usage.set(is_zoom_using_mic().unwrap_or(false));
            }

            loop {
                if let Some(mic_usage) = &mic_usage
                    && !mic_usage.is_active()
                {
                    if state.last_mute_state.is_some() {
                        let _ = reconcile_zoom_mute_state(&mut state, Ok(false), None);
                    }
                    tokio::select! {
                        _ = &mut rx => break,
                        _ = mic_usage.changed.notified() => continue,
                    }
                }

                tokio::select! {
                    _ = &mut rx => {
                        break;
                    }
                    _ = async {
                        if let Some(mic_usage) = &mic_usage {
                            mic_usage.changed.notified().await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    } => {
                        continue;
                    }
                    _ = tokio::time::sleep(ACTIVE_POLL_INTERVAL) => {
                        if !running.load(std::sync::atomic::Ordering::SeqCst) {
                            break;
                        }

                        let current_mic_usage = mic_usage
                            .as_ref()
                            .map_or_else(is_zoom_using_mic, |usage| Ok(usage.is_active()));
                        let mute_state = match current_mic_usage {
                            Ok(true) => check_zoom_mute_state(),
                            Ok(false) | Err(_) => None,
                        };

                        if let Some(event) = reconcile_zoom_mute_state(&mut state, current_mic_usage, mute_state) {
                            f(event);
                        }
                    }
                }
            }

            tracing::info!("zoom mute watcher stopped");
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Observer, new_callback};
    use std::time::Duration;

    #[test]
    fn test_reconcile_zoom_mute_state_keeps_state_on_mic_usage_error() {
        let mut state = WatcherState::new();
        state.last_mute_state = Some(true);

        let event = reconcile_zoom_mute_state(
            &mut state,
            Err(crate::Error::AudioProcessState(
                "snapshot failed".to_string(),
            )),
            None,
        );

        assert!(event.is_none());
        assert_eq!(state.last_mute_state, Some(true));
    }

    #[test]
    fn test_reconcile_zoom_mute_state_does_not_duplicate_after_error() {
        let mut state = WatcherState::new();
        state.last_mute_state = Some(true);

        let event = reconcile_zoom_mute_state(&mut state, Ok(true), Some(true));

        assert!(event.is_none());
        assert_eq!(state.last_mute_state, Some(true));
    }

    #[test]
    fn test_reconcile_zoom_mute_state_clears_state_when_zoom_stops_using_mic() {
        let mut state = WatcherState::new();
        state.last_mute_state = Some(false);

        let event = reconcile_zoom_mute_state(&mut state, Ok(false), None);

        assert!(event.is_none());
        assert_eq!(state.last_mute_state, None);
    }

    #[test]
    fn test_zoom_mic_usage_ignores_other_app_deltas() {
        let usage = ZoomMicUsage::default();
        usage.set(true);

        usage.update_from_mic_event(&DetectEvent::MicStarted(vec![crate::InstalledApp {
            id: "com.example.recorder".to_string(),
            name: "Recorder".to_string(),
        }]));
        assert!(usage.is_active());

        usage.update_from_mic_event(&DetectEvent::MicStopped(vec![crate::InstalledApp {
            id: "com.example.recorder".to_string(),
            name: "Recorder".to_string(),
        }]));
        assert!(usage.is_active());
    }

    // cargo test --package detect --lib --features mic,list,zoom -- zoom::tests::test_watcher --exact --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_watcher() {
        let mut watcher = ZoomMuteWatcher::default();
        watcher.start(new_callback(|v| {
            println!("{:?}", v);
        }));

        tokio::time::sleep(Duration::from_secs(60)).await;
        watcher.stop();
    }
}
