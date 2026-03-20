use std::time::{Duration, Instant};

use cidre::{ax, ns};

use crate::{BackgroundTask, DetectCallback, DetectEvent};

const ZOOM_BUNDLE_ID: &str = "us.zoom.xos";

pub struct ZoomMuteWatcher {
    background: BackgroundTask,
}

impl Default for ZoomMuteWatcher {
    fn default() -> Self {
        Self {
            background: BackgroundTask::default(),
        }
    }
}

struct WatcherState {
    last_mute_state: Option<bool>,
    last_check: Instant,
    poll_interval: Duration,
}

impl WatcherState {
    fn new() -> Self {
        Self {
            last_mute_state: None,
            last_check: Instant::now(),
            poll_interval: Duration::from_millis(1000),
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

fn is_zoom_using_mic() -> bool {
    let apps = crate::list_mic_using_apps();
    apps.iter().any(|app| app.id == ZOOM_BUNDLE_ID)
}

impl crate::Observer for ZoomMuteWatcher {
    fn start(&mut self, f: DetectCallback) {
        if self.background.is_running() {
            return;
        }

        if !macos_accessibility_client::accessibility::application_is_trusted() {
            return;
        }

        self.background.start(|running, mut rx| async move {
            let mut state = WatcherState::new();

            loop {
                tokio::select! {
                    _ = &mut rx => {
                        break;
                    }
                    _ = tokio::time::sleep(state.poll_interval) => {
                        if !running.load(std::sync::atomic::Ordering::SeqCst) {
                            break;
                        }

                        if !is_zoom_using_mic() {
                            if state.last_mute_state.is_some() {
                                tracing::debug!("zoom no longer using mic, clearing state");
                                state.last_mute_state = None;
                            }
                            continue;
                        }

                        if let Some(muted) = check_zoom_mute_state() {
                            if state.last_mute_state != Some(muted) {
                                tracing::info!(muted = muted, "zoom mute state changed");
                                state.last_mute_state = Some(muted);

                                let event = DetectEvent::ZoomMuteStateChanged { value: muted };
                                f(event);
                            }
                        }

                        state.last_check = Instant::now();
                    }
                }
            }

            tracing::info!("zoom mute watcher stopped");
        });
    }

    fn stop(&mut self) {
        self.background.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Observer, new_callback};
    use std::time::Duration;

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
