use std::cell::RefCell;
use std::time::{Duration, Instant};

use anlg_overlay_kit::macos::timer::RepeatingTimer;
use objc2::rc::Retained;
use objc2_app_kit::{NSPanel, NSWorkspace};
use objc2_foundation::{MainThreadMarker, NSString, NSURL};

use anlg_overlay_kit::macos::main_thread::run_on_main_thread;

use crate::guidance::{SettingsGuidance, privacy_settings_deep_link_urls};
use crate::{Error, Permission, PermissionStatus, Result};

use super::animation::{self, prepare_panel_for_reveal, reveal_panel};
use super::host_app::HostApp;
use super::overlay::create_overlay_window;
use super::positioning::{PositionMode, position_overlay, refresh_overlay_position};
use super::settings_window::SettingsWindowSnapshot;

const ASSISTANT_TIMER_INTERVAL: Duration = Duration::from_millis(100);
const INITIAL_POSITION_FALLBACK_AFTER: Duration = Duration::from_millis(1200);
const INITIAL_POSITION_STABLE_FOR: Duration = Duration::from_millis(160);
const INITIAL_POSITION_STABLE_THRESHOLD: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AssistedPane {
    pub(super) anchor: &'static str,
    pub(super) title: &'static str,
}

impl AssistedPane {
    fn from_permission(permission: Permission) -> Option<Self> {
        match permission.settings_guidance() {
            SettingsGuidance::Assisted { anchor, pane_title } => Some(Self {
                anchor,
                title: pane_title,
            }),
            SettingsGuidance::Native { .. } => None,
        }
    }
}

#[derive(Debug)]
struct AssistantState {
    panel: Option<Retained<NSPanel>>,
    pending_snapshot: Option<PendingSnapshot>,
    timer: RepeatingTimer,
}

#[derive(Clone, Copy, Debug)]
struct PendingSnapshot {
    snapshot: SettingsWindowSnapshot,
    observed_at: Instant,
}

impl AssistantState {
    fn close_immediate(self) {
        self.timer.invalidate();
        if let Some(panel) = self.panel {
            panel.close();
        }
    }

    fn close_animated(self) {
        let Self { panel, timer, .. } = self;
        let Some(panel) = panel else {
            timer.invalidate();
            return;
        };
        let panel_for_close = panel.clone();
        animation::dismiss_panel(&panel, move || {
            timer.invalidate();
            panel_for_close.close();
        });
    }
}

thread_local! {
    static ACTIVE_ASSISTANT: RefCell<Option<AssistantState>> = const { RefCell::new(None) };
}

pub(crate) fn open_assisted(permission: Permission) -> Result<bool> {
    let Some(pane) = AssistedPane::from_permission(permission) else {
        return Ok(false);
    };

    run_on_main_thread(move || present(permission, pane))?;
    Ok(true)
}

pub(crate) fn dismiss_current() {
    ACTIVE_ASSISTANT.with(|slot| {
        if let Some(state) = slot.borrow_mut().take() {
            state.close_animated();
        }
    });
}

fn dismiss_current_immediate() {
    ACTIVE_ASSISTANT.with(|slot| {
        if let Some(state) = slot.borrow_mut().take() {
            state.close_immediate();
        }
    });
}

fn present(permission: Permission, assisted_pane: AssistedPane) -> Result<()> {
    let mtm = MainThreadMarker::new().ok_or_else(|| {
        Error::Assistant("permission assistant must run on the main thread".to_string())
    })?;

    dismiss_current_immediate();
    open_settings_url(assisted_pane)?;

    let host_app = HostApp::current()?;
    let started_at = Instant::now();
    let initial_panel = SettingsWindowSnapshot::locate_visible_frontmost_window()
        .map(|snapshot| create_and_reveal(mtm, assisted_pane, &host_app, snapshot));

    let timer = RepeatingTimer::schedule(mtm, ASSISTANT_TIMER_INTERVAL, move || {
        if matches!(
            crate::assisted_status(permission),
            Some(PermissionStatus::Authorized)
        ) {
            dismiss_current();
            return;
        }

        let panel = ACTIVE_ASSISTANT.with(|slot| {
            let mut slot = slot.borrow_mut();
            let state = slot.as_mut()?;
            if state.panel.is_none() {
                let snapshot = settled_initial_position_snapshot(state, started_at)?;
                state.pending_snapshot = None;
                state.panel = Some(create_and_reveal(mtm, assisted_pane, &host_app, snapshot));
            }
            state.panel.clone()
        });
        if let Some(panel) = panel {
            refresh_overlay_position(&panel, PositionMode::Tracking);
        }
    });

    ACTIVE_ASSISTANT.with(|slot| {
        *slot.borrow_mut() = Some(AssistantState {
            panel: initial_panel,
            pending_snapshot: None,
            timer,
        });
    });

    Ok(())
}

fn settled_initial_position_snapshot(
    state: &mut AssistantState,
    started_at: Instant,
) -> Option<SettingsWindowSnapshot> {
    let now = Instant::now();

    if let Some(snapshot) = SettingsWindowSnapshot::locate_visible_frontmost_window() {
        match state.pending_snapshot {
            Some(pending)
                if snapshot_delta(pending.snapshot, snapshot)
                    <= INITIAL_POSITION_STABLE_THRESHOLD =>
            {
                state.pending_snapshot = Some(PendingSnapshot {
                    snapshot,
                    observed_at: pending.observed_at,
                });
                if now.duration_since(pending.observed_at) >= INITIAL_POSITION_STABLE_FOR {
                    return Some(snapshot);
                }
            }
            _ => {
                state.pending_snapshot = Some(PendingSnapshot {
                    snapshot,
                    observed_at: now,
                });
            }
        }
    }

    if now.duration_since(started_at) >= INITIAL_POSITION_FALLBACK_AFTER {
        SettingsWindowSnapshot::locate_with_launch_fallback()
    } else {
        None
    }
}

fn snapshot_delta(left: SettingsWindowSnapshot, right: SettingsWindowSnapshot) -> f64 {
    animation::frame_delta(left.frame, right.frame).max(animation::frame_delta(
        left.visible_frame,
        right.visible_frame,
    ))
}

fn create_and_reveal(
    mtm: MainThreadMarker,
    assisted_pane: AssistedPane,
    host_app: &HostApp,
    snapshot: SettingsWindowSnapshot,
) -> Retained<NSPanel> {
    let panel = create_overlay_window(mtm, assisted_pane, host_app);
    prepare_panel_for_reveal(&panel);
    position_overlay(&panel, snapshot, PositionMode::Initial);
    reveal_panel(&panel);
    panel
}

fn open_settings_url(assisted_pane: AssistedPane) -> Result<()> {
    let workspace = NSWorkspace::sharedWorkspace();
    for url in privacy_settings_deep_link_urls(assisted_pane.anchor) {
        let Some(ns_url) = NSURL::URLWithString(&NSString::from_str(&url)) else {
            continue;
        };
        if workspace.openURL(&ns_url) {
            return Ok(());
        }
    }

    Err(Error::Assistant(
        "failed to open System Settings".to_string(),
    ))
}
