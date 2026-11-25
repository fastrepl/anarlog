//! `settings/sync/{index,health,log}.tsx` and `settings/general/e2ee-setup.tsx` for a Pro
//! account: the sync switch and status, Sync now, health counters, the sync log, devices and
//! their slots, and the recovery-key setup dialog.

use std::{collections::HashSet, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use gpui::{
    AnyElement, Context, Div, Entity, FontWeight, Render, SharedString, Subscription, Task, Window,
    div, prelude::*, px, rgb,
};
use reqwest::Method;
use serde_json::Value;
use zeroize::Zeroizing;

use super::{
    calls::{CloudCalls, SyncBlock, SyncCommand, SyncState, SyncStatus},
    teams_view::{Variant, button, field, spinner},
};
use crate::ui::a11y::Accessible;
use crate::{
    chat::state::distance_to_now,
    product::controls,
    ui::{
        input::{InputEvent, TextInput},
        theme::{MEDIUM, Theme, monospace_font, theme},
    },
    workspace::resource_list as list,
};

type Load<T> = Option<std::result::Result<T, String>>;

const POLL: Duration = Duration::from_secs(10);
const DEVICE_POLL: Duration = Duration::from_secs(5);
const CLIPBOARD_TTL: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusKind {
    Paused,
    Local,
    Error,
    Syncing,
    Synced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusView {
    pub kind: StatusKind,
    pub label: &'static str,
    pub description: String,
    pub detail: Option<String>,
}

/// `statusView` from `settings/sync/index.tsx`, in its order of precedence.
pub fn status_view(
    preferred: bool,
    block: Option<&SyncBlock>,
    status: Option<&SyncStatus>,
    syncing_now: bool,
    now: DateTime<Utc>,
) -> StatusView {
    let view = |kind, label, description: &str| StatusView {
        kind,
        label,
        description: description.to_owned(),
        detail: None,
    };
    if !preferred {
        return view(
            StatusKind::Paused,
            "Sync paused",
            "Changes stay on this device until you resume sync.",
        );
    }
    if let Some(block) = block {
        return match block {
            SyncBlock::ApprovalPending => view(
                StatusKind::Local,
                "Connecting this device",
                "Open Anarlog on an existing synced device signed in to the same account. This device will connect automatically.",
            ),
            SyncBlock::DeviceLimit => view(
                StatusKind::Error,
                "Device limit reached",
                "Disconnect another device below. This device will continue automatically.",
            ),
            SyncBlock::UpdateRequired => view(
                StatusKind::Error,
                "Update Anarlog to keep syncing",
                "This version can no longer sync. Your notes stay on this device, and changes made here will sync after you update.",
            ),
            SyncBlock::ActivationFailed(detail) => StatusView {
                detail: Some(detail.clone()),
                ..view(
                    StatusKind::Error,
                    "Sync needs attention",
                    "Anarlog could not start cloud sync on this device. It will keep retrying.",
                )
            },
            SyncBlock::SetupRequired => view(
                StatusKind::Error,
                "Sync needs attention",
                "Set up your recovery key to start encrypted cloud sync.",
            ),
            SyncBlock::KeychainAccess => view(
                StatusKind::Error,
                "Sync needs attention",
                "macOS could not access your recovery key. Repair Keychain access, then resume sync.",
            ),
        };
    }
    let Some(status) = status else {
        return view(
            StatusKind::Error,
            "Sync status unavailable",
            "Your notes remain available locally. Try again in a moment.",
        );
    };
    if status.consecutive_failures > 0 {
        return view(
            StatusKind::Error,
            "Sync needs attention",
            "Anarlog will keep retrying.",
        );
    }
    if status.activity_paused {
        return view(
            StatusKind::Local,
            "Saved locally",
            "Cloud sync resumes when the current activity finishes.",
        );
    }
    if !status.configured || !status.running {
        return view(
            StatusKind::Syncing,
            "Connecting...",
            "Setting up encrypted cloud sync.",
        );
    }
    match status.last_sync_at_ms {
        Some(last) if !syncing_now && !status.has_unsent_changes => {
            let at = DateTime::<Utc>::from_timestamp_millis(last as i64).unwrap_or(now);
            view(
                StatusKind::Synced,
                "Synced",
                &format!("Last synced {}", distance_to_now(at, now)),
            )
        }
        _ => view(
            StatusKind::Syncing,
            "Syncing...",
            "Sending and receiving your latest changes.",
        ),
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Device {
    fingerprint: Arc<str>,
    name: Option<Arc<str>>,
    last_seen: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq)]
struct PendingDevice {
    fingerprint: Arc<str>,
    name: Option<Arc<str>>,
    sealed: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Devices {
    devices: Vec<Device>,
    pending: Vec<PendingDevice>,
}

impl Devices {
    fn parse(value: &Value) -> Option<Self> {
        let name = |row: &Value| {
            row["deviceName"]
                .as_str()
                .filter(|name| !name.is_empty())
                .map(Arc::from)
        };
        let fingerprint = |row: &Value| row["deviceFingerprint"].as_str().map(Arc::from);
        Some(Self {
            devices: value["devices"]
                .as_array()?
                .iter()
                .filter_map(|row| {
                    Some(Device {
                        fingerprint: fingerprint(row)?,
                        name: name(row),
                        last_seen: row["lastSeenAt"]
                            .as_str()
                            .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
                            .map(|at| at.with_timezone(&Utc)),
                    })
                })
                .collect(),
            pending: value["pendingDevices"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|row| {
                    Some(PendingDevice {
                        fingerprint: fingerprint(row)?,
                        name: name(row),
                        sealed: row["status"] == "sealed",
                    })
                })
                .collect(),
        })
    }

    /// Devices and pending enrollments both keep a fingerprint entry.
    fn used(&self) -> usize {
        self.devices
            .iter()
            .map(|device| &device.fingerprint)
            .chain(self.pending.iter().map(|device| &device.fingerprint))
            .collect::<HashSet<_>>()
            .len()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    Toggle(bool),
    SyncNow,
    Repair,
    Remove,
    Rename,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SetupMode {
    Choose,
    Import,
}

struct Setup {
    mode: SetupMode,
    key: Option<Zeroizing<String>>,
    input: Entity<TextInput>,
    pending: bool,
    error: Option<String>,
    _subscription: Subscription,
}

struct Rename {
    fingerprint: Arc<str>,
    input: Entity<TextInput>,
    error: Option<String>,
    _subscription: Subscription,
}

pub struct SyncView {
    calls: Arc<dyn CloudCalls>,
    state: Load<SyncState>,
    devices: Load<Devices>,
    pending: Option<(Pending, Option<Arc<str>>)>,
    /// The last switch or Sync now failure (`mutationError`).
    error: Option<String>,
    keychain_error: bool,
    repair_error: Option<String>,
    device_error: Option<String>,
    log_open: bool,
    add_device: bool,
    setup: Option<Setup>,
    rename: Option<Rename>,
    _poll: Task<()>,
}

impl SyncView {
    pub fn new(calls: Arc<dyn CloudCalls>, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            let mut ticks = 0u32;
            loop {
                cx.background_executor().timer(DEVICE_POLL).await;
                ticks += 1;
                let alive = this.update(cx, |this, cx| {
                    // Status refreshes every 10 s; devices every 5 s while one is pending.
                    if ticks.is_multiple_of((POLL.as_secs() / DEVICE_POLL.as_secs()) as u32) {
                        this.load_state(cx);
                    }
                    if this
                        .devices
                        .as_ref()
                        .and_then(|devices| devices.as_ref().ok())
                        .is_some_and(|devices| !devices.pending.is_empty())
                    {
                        this.load_devices(cx);
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        let mut this = Self {
            calls,
            state: None,
            devices: None,
            pending: None,
            error: None,
            keychain_error: false,
            repair_error: None,
            device_error: None,
            log_open: false,
            add_device: false,
            setup: None,
            rename: None,
            _poll: poll,
        };
        this.load_state(cx);
        this.load_devices(cx);
        this
    }

    pub fn has_unsaved(&self) -> bool {
        self.pending.is_some() || self.setup.as_ref().is_some_and(|setup| setup.pending)
    }

    fn load_state(&mut self, cx: &mut Context<Self>) {
        let state = self.calls.sync_state();
        cx.spawn(async move |this, cx| {
            let result = state.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(state) => this.state = Some(Ok(state)),
                    Err(error) => {
                        if !matches!(this.state, Some(Ok(_))) {
                            this.state = Some(Err(error.to_string()));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_devices(&mut self, cx: &mut Context<Self>) {
        let devices = self.calls.api(Method::GET, "/sync/devices".into(), None);
        cx.spawn(async move |this, cx| {
            let result = devices.await.and_then(|value| {
                Devices::parse(&value).ok_or_else(|| {
                    desktop_runtime::ServiceError::Failed("Invalid device list".into())
                })
            });
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(devices) => this.devices = Some(Ok(devices)),
                    Err(error) => {
                        if !matches!(this.devices, Some(Ok(_))) {
                            this.devices = Some(Err(error.to_string()));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn state(&self) -> Option<&SyncState> {
        self.state.as_ref().and_then(|state| state.as_ref().ok())
    }

    /// Runs a sync command, then refreshes the status and devices.
    fn run(
        &mut self,
        pending: Pending,
        device: Option<Arc<str>>,
        command: SyncCommand,
        done: impl FnOnce(
            &mut Self,
            std::result::Result<Option<Zeroizing<String>>, String>,
            &mut Context<Self>,
        ) + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.pending.is_some() {
            return;
        }
        self.pending = Some((pending, device));
        let work = self.calls.sync(command);
        cx.spawn(async move |this, cx| {
            let result = work.await.map_err(|error| error.to_string());
            let _ = this.update(cx, |this, cx| {
                this.pending = None;
                done(this, result, cx);
                this.load_state(cx);
                this.load_devices(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn set_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.error = None;
        self.keychain_error = false;
        self.run(
            Pending::Toggle(enabled),
            None,
            SyncCommand::SetEnabled(enabled),
            move |this, result, cx| match result {
                // Enabling without a recovery key opens the setup dialog.
                Ok(_) if enabled => this.open_setup_when_required(cx),
                Ok(_) => {}
                Err(error) => {
                    this.keychain_error = error.contains("could not access your recovery key");
                    this.error = Some(error);
                }
            },
            cx,
        );
    }

    /// Opens the setup dialog once the refreshed state reports a missing recovery key.
    fn open_setup_when_required(&mut self, cx: &mut Context<Self>) {
        let state = self.calls.sync_state();
        cx.spawn(async move |this, cx| {
            let result = state.await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(state) = result {
                    if state.block == Some(SyncBlock::SetupRequired) {
                        this.open_setup(cx);
                    }
                    this.state = Some(Ok(state));
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_setup(&mut self, cx: &mut Context<Self>) {
        let input = cx.new(|cx| TextInput::new("anarlog-e2ee-v1:...", cx).inline());
        let subscription = cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Submitted) {
                this.unlock(cx);
            }
            if let Some(setup) = &mut this.setup {
                setup.error = None;
            }
            cx.notify();
        });
        self.setup = Some(Setup {
            mode: SetupMode::Choose,
            key: None,
            input,
            pending: false,
            error: None,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn close_setup(&mut self, cx: &mut Context<Self>) {
        if self.setup.as_ref().is_some_and(|setup| setup.pending) {
            return;
        }
        self.setup = None;
        cx.notify();
    }

    fn setup_command(&mut self, command: SyncCommand, cx: &mut Context<Self>) {
        let Some(setup) = &mut self.setup else {
            return;
        };
        if setup.pending {
            return;
        }
        setup.pending = true;
        setup.error = None;
        let ready = matches!(command, SyncCommand::UseRecoveryKey(_));
        let work = self.calls.sync(command);
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                let Some(setup) = &mut this.setup else {
                    return;
                };
                setup.pending = false;
                match result {
                    Ok(Some(key)) => setup.key = Some(key),
                    Ok(None) if ready => {
                        // `onReady`: close, then turn sync on.
                        this.setup = None;
                        this.set_enabled(true, cx);
                    }
                    Ok(None) => {}
                    Err(error) => setup.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn unlock(&mut self, cx: &mut Context<Self>) {
        let Some(setup) = &self.setup else {
            return;
        };
        let key = match &setup.key {
            Some(key) => key.clone(),
            None => Zeroizing::new(setup.input.read(cx).buffer.text.trim().to_owned()),
        };
        if key.is_empty() {
            return;
        }
        self.setup_command(SyncCommand::UseRecoveryKey(key), cx);
    }

    /// Copies the key and clears the clipboard after 60 s if it still holds it.
    fn copy_key(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.setup.as_ref().and_then(|setup| setup.key.clone()) else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(key.to_string()));
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(CLIPBOARD_TTL).await;
            cx.update(|cx| {
                if cx
                    .read_from_clipboard()
                    .and_then(|item| item.text())
                    .is_some_and(|text| text == key.as_str())
                {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(String::new()));
                }
            });
        })
        .detach();
    }

    fn repair(&mut self, cx: &mut Context<Self>) {
        self.repair_error = None;
        self.run(
            Pending::Repair,
            None,
            SyncCommand::RepairKeychain,
            |this, result, _| match result {
                Ok(_) => {
                    this.error = None;
                    this.keychain_error = false;
                }
                Err(error) => this.repair_error = Some(error),
            },
            cx,
        );
    }

    fn device_command(
        &mut self,
        pending: Pending,
        fingerprint: Arc<str>,
        command: SyncCommand,
        cx: &mut Context<Self>,
    ) {
        self.device_error = None;
        self.run(
            pending,
            Some(fingerprint),
            command,
            |this, result, _| {
                if let Err(error) = result {
                    if this.rename.is_some() {
                        if let Some(rename) = &mut this.rename {
                            rename.error = Some(error);
                        }
                    } else {
                        this.device_error = Some(error);
                    }
                } else {
                    this.rename = None;
                }
            },
            cx,
        );
    }

    fn open_rename(&mut self, device: Arc<str>, name: Option<Arc<str>>, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            let mut input = TextInput::new("", cx).inline();
            input.set_text(name.as_deref().unwrap_or_default().to_owned(), cx);
            input
        });
        let subscription = cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Submitted) {
                this.save_rename(cx);
            }
            cx.notify();
        });
        self.rename = Some(Rename {
            fingerprint: device,
            input,
            error: None,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn save_rename(&mut self, cx: &mut Context<Self>) {
        let Some(rename) = &self.rename else {
            return;
        };
        let name = rename.input.read(cx).buffer.text.trim().to_owned();
        if !valid_device_name(&name) {
            return;
        }
        let fingerprint = rename.fingerprint.clone();
        self.device_command(
            Pending::Rename,
            fingerprint.clone(),
            SyncCommand::RenameDevice {
                fingerprint: fingerprint.to_string(),
                name,
            },
            cx,
        );
    }
}

fn valid_device_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty() && name.len() <= 128
}

/// `amber-500`; the design tokens have no amber.
fn amber() -> gpui::Hsla {
    rgb(0xf59e0b).into()
}

fn emerald() -> gpui::Hsla {
    rgb(0x10b981).into()
}

fn red() -> gpui::Hsla {
    rgb(0xef4444).into()
}

fn small_text(text: impl Into<SharedString>, color: gpui::Hsla) -> Div {
    div()
        .text_xs()
        .line_height(px(16.))
        .text_color(color)
        .child(text.into())
}

impl SyncView {
    fn status_section(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let state = self.state();
        let preferred = match self.pending {
            Some((Pending::Toggle(value), _)) => value,
            _ => state.is_none_or(|state| state.enabled),
        };
        let block = state.and_then(|state| state.block.as_ref());
        let status = state.map(|state| &state.status);
        let syncing_now = matches!(self.pending, Some((Pending::SyncNow, _)));
        let view = status_view(preferred, block, status, syncing_now, Utc::now());
        let icon = match view.kind {
            StatusKind::Syncing => {
                controls::icon("product/RefreshIcon.svg", 16., rgb(0x3b82f6).into())
            }
            StatusKind::Synced => {
                controls::icon("product/CheckmarkCircle02Icon.svg", 16., emerald())
            }
            StatusKind::Error => controls::icon("product/TriangleAlertIcon.svg", 16., amber()),
            StatusKind::Paused | StatusKind::Local => {
                controls::icon("product/CloudOffIcon.svg", 16., colors.muted_foreground)
            }
        };
        let switch_checked = preferred && block != Some(&SyncBlock::SetupRequired);
        let toggling = matches!(self.pending, Some((Pending::Toggle(_), _)));
        let switch_enabled = !toggling && state.is_some();
        let keychain = block == Some(&SyncBlock::KeychainAccess) || self.keychain_error;
        let sync_now_enabled = preferred
            && block.is_none()
            && self.pending.is_none()
            && status.is_some_and(|status| {
                status.configured && status.running && !status.activity_paused
            });
        let approval = block == Some(&SyncBlock::ApprovalPending);
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap_4()
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .gap_3()
                            .child(
                                div()
                                    .size(px(36.))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_sm()
                                            .line_height(px(20.))
                                            .font_weight(MEDIUM)
                                            .child(view.label),
                                    )
                                    .child(
                                        div()
                                            .mt_1()
                                            .text_xs()
                                            .line_height(px(20.))
                                            .text_color(colors.muted_foreground)
                                            .child(view.description.clone()),
                                    )
                                    .children(view.detail.clone().map(|detail| {
                                        div()
                                            .mt_1()
                                            .font_family(monospace_font(cx))
                                            .text_size(px(11.))
                                            .line_height(px(16.))
                                            .text_color(colors.muted_foreground)
                                            .child(detail)
                                    }))
                                    .when(approval, |view| {
                                        view.child(
                                            div().mt_3().flex().child(
                                                button(
                                                    "use-recovery-key",
                                                    "Use recovery key instead",
                                                    Variant::Outline,
                                                    true,
                                                    None,
                                                    true,
                                                    colors,
                                                )
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.open_setup(cx)
                                                })),
                                            ),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        controls::switch(
                            "cloud-sync-switch",
                            switch_checked,
                            switch_enabled,
                            colors,
                            cx.listener(move |this, _, _, cx| {
                                this.set_enabled(!switch_checked, cx)
                            }),
                        )
                        .aria_label("Cloud sync"),
                    ),
            )
            .children(
                self.error
                    .as_ref()
                    .map(|error| small_text(error.clone(), red())),
            )
            .when(keychain, |view| {
                let pending = matches!(self.pending, Some((Pending::Repair, _)));
                view.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_start()
                        .gap_2()
                        .child(
                            button(
                                "repair-keychain",
                                "Repair Keychain Access",
                                Variant::Outline,
                                true,
                                pending.then_some("product/LoaderCircleIcon.svg"),
                                !pending,
                                colors,
                            )
                            .when(!pending, |view| {
                                view.on_click(cx.listener(|this, _, _, cx| this.repair(cx)))
                            }),
                        )
                        .children(
                            self.repair_error
                                .as_ref()
                                .map(|error| small_text(error.clone(), red())),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(small_text(
                        "Keep notes current automatically.",
                        colors.muted_foreground,
                    ))
                    .child(
                        button(
                            "sync-now",
                            "Sync now",
                            Variant::Outline,
                            true,
                            Some("product/RefreshIcon.svg"),
                            sync_now_enabled,
                            colors,
                        )
                        .when(sync_now_enabled, |view| {
                            view.on_click(cx.listener(|this, _, _, cx| {
                                this.error = None;
                                this.run(
                                    Pending::SyncNow,
                                    None,
                                    SyncCommand::SyncNow,
                                    |this, result, _| {
                                        if let Err(error) = result {
                                            this.keychain_error = error
                                                .contains("could not access your recovery key");
                                            this.error = Some(error);
                                        }
                                    },
                                    cx,
                                )
                            }))
                        }),
                    ),
            )
            .children(state.and_then(|state| health(state, colors)))
            .child(self.log(colors, cx))
    }

    fn log(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let open = self.log_open;
        let border = colors.border.opacity(0.6);
        let hover = colors.muted.opacity(0.4);
        let lines = self
            .state()
            .map(|state| state.log.clone())
            .unwrap_or_default();
        div()
            .overflow_hidden()
            .border_1()
            .border_color(border)
            .child(
                div()
                    .id("sync-log-toggle")
                    .w_full()
                    .px_4()
                    .py_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .child(
                        div()
                            .child(
                                div()
                                    .text_xs()
                                    .line_height(px(16.))
                                    .font_weight(MEDIUM)
                                    .child("Sync log"),
                            )
                            .child(
                                div()
                                    .mt(px(2.))
                                    .text_size(px(11.))
                                    .line_height(px(16.))
                                    .text_color(colors.muted_foreground)
                                    .child("Recent CloudSync entries from the app log."),
                            ),
                    )
                    .child(
                        controls::icon("product/ArrowDown01Icon.svg", 14., colors.muted_foreground)
                            .when(open, |icon| {
                                icon.with_transformation(gpui::Transformation::rotate(
                                    gpui::radians(std::f32::consts::PI),
                                ))
                            }),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.log_open = !this.log_open;
                        if this.log_open {
                            this.load_state(cx);
                        }
                        cx.notify();
                    })),
            )
            .when(open, |view| {
                view.child(div().border_t_1().border_color(border).px_4().py_3().child(
                    if lines.is_empty() {
                        div()
                            .py_2()
                            .text_center()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child("No sync activity yet.")
                            .into_any_element()
                    } else {
                        div()
                            .id("sync-log-lines")
                            .max_h(px(256.))
                            .overflow_y_scroll()
                            .font_family(monospace_font(cx))
                            .text_size(px(11.))
                            .line_height(px(20.))
                            .children(lines.into_iter().enumerate().map(|(index, line)| {
                                div()
                                    .when(index > 0, |view| {
                                        view.pt_2().border_t_1().border_color(border)
                                    })
                                    .pb_2()
                                    .child(line)
                            }))
                            .into_any_element()
                    },
                ))
            })
    }

    fn devices_section(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let devices = self.devices.as_ref();
        let loaded = devices.and_then(|devices| devices.as_ref().ok());
        let fingerprint = self.state().map(|state| state.fingerprint.clone());
        let border = colors.border.opacity(0.6);
        let pending_device = |kind: Pending, device: &Arc<str>| {
            self.pending.as_ref() == Some(&(kind, Some(device.clone())))
        };
        let any_pending = self.pending.is_some();
        let header = div()
            .mb_4()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .child(controls::section_title("Devices"))
                    .children(loaded.map(|devices| {
                        div()
                            .ml_2()
                            .text_size(px(18.))
                            .line_height(px(28.))
                            .text_color(colors.muted_foreground)
                            .child(devices.used().to_string())
                    })),
            )
            .child(
                div().flex().items_center().gap_2().child(
                    button(
                        "add-device",
                        "Add device",
                        Variant::Outline,
                        true,
                        Some("product/PlusIcon.svg"),
                        true,
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.add_device = true;
                        cx.notify();
                    })),
                ),
            );
        let title = |name: Option<Arc<str>>,
                     current: bool,
                     rename: Option<Arc<str>>,
                     cx: &mut Context<Self>| {
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .truncate()
                        .text_sm()
                        .line_height(px(20.))
                        .font_weight(MEDIUM)
                        .child(name.as_deref().unwrap_or("Unnamed device").to_owned()),
                )
                .when(current, |view| {
                    view.child(
                        div()
                            .flex_shrink_0()
                            .px_2()
                            .py(px(2.))
                            .bg(colors.secondary)
                            .text_color(colors.secondary_foreground)
                            .text_xs()
                            .line_height(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("This device"),
                    )
                })
                .when_some(rename.filter(|_| current), |view, fingerprint| {
                    let name = name.clone();
                    view.child(
                        div()
                            .id("rename-device")
                            .size(px(24.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|style| style.bg(colors.accent))
                            .child(controls::icon(
                                "product/PencilEdit01Icon.svg",
                                14.,
                                colors.muted_foreground,
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_rename(fingerprint.clone(), name.clone(), cx)
                            })),
                    )
                })
        };
        let disconnect = |device: Arc<str>, cx: &mut Context<Self>| {
            let spinning = pending_device(Pending::Remove, &device);
            button(
                SharedString::from(format!("disconnect-{device}")),
                "Disconnect",
                Variant::OutlineDestructive,
                true,
                Some(if spinning {
                    "product/LoaderCircleIcon.svg"
                } else {
                    "product/Plug01Icon.svg"
                }),
                !any_pending,
                colors,
            )
            .when(!any_pending, |view| {
                view.on_click(cx.listener(move |this, _, _, cx| {
                    let fingerprint = device.clone();
                    this.device_command(
                        Pending::Remove,
                        fingerprint.clone(),
                        SyncCommand::RemoveDevice(fingerprint.to_string()),
                        cx,
                    )
                }))
            })
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        match devices {
            None => rows.push(
                div()
                    .px_4()
                    .py_5()
                    .flex()
                    .justify_center()
                    .child(spinner(16., colors.muted_foreground))
                    .into_any_element(),
            ),
            Some(Err(_)) => rows.push(
                div()
                    .px_4()
                    .py_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(small_text("Could not load your devices.", red()))
                    .child(
                        button(
                            "devices-retry",
                            "Retry",
                            Variant::Outline,
                            true,
                            None,
                            true,
                            colors,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.load_devices(cx))),
                    )
                    .into_any_element(),
            ),
            Some(Ok(list)) => {
                for device in &list.devices {
                    let current = fingerprint.as_ref() == Some(&device.fingerprint);
                    let last_seen = device.last_seen.map_or_else(String::new, |at| {
                        let distance = distance_to_now(at, Utc::now());
                        format!("Last seen {}", distance.trim_end_matches(" ago"))
                    });
                    let fp = device.fingerprint.clone();
                    rows.push(
                        div()
                            .px_4()
                            .py_3()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .child(title(
                                        device.name.clone(),
                                        current,
                                        Some(fp.clone()),
                                        cx,
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .line_height(px(16.))
                                            .text_color(colors.muted_foreground)
                                            .child(last_seen),
                                    ),
                            )
                            .when(!current, |view| view.child(disconnect(fp.clone(), cx)))
                            .into_any_element(),
                    );
                }
                for device in &list.pending {
                    let current = fingerprint.as_ref() == Some(&device.fingerprint);
                    let fp = device.fingerprint.clone();
                    rows.push(
                        div()
                            .px_4()
                            .py_3()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .child(title(
                                        device.name.clone(),
                                        current,
                                        Some(fp.clone()),
                                        cx,
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .line_height(px(16.))
                                            .text_color(colors.muted_foreground)
                                            .child(if device.sealed {
                                                "Connecting — waiting for this device to finish"
                                            } else if current {
                                                "Waiting for an existing device"
                                            } else {
                                                "Connecting automatically"
                                            }),
                                    ),
                            )
                            .when(!current && device.sealed, |view| {
                                view.child(small_text("Connecting", emerald()))
                            })
                            .when(!current, |view| view.child(disconnect(fp.clone(), cx)))
                            .into_any_element(),
                    );
                }
                if list.devices.is_empty() && list.pending.is_empty() {
                    rows.push(
                        div()
                            .px_4()
                            .py_5()
                            .text_center()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child("No devices registered yet.")
                            .into_any_element(),
                    );
                }
            }
        }
        div()
            .child(header)
            .child(
                div()
                    .overflow_hidden()
                    .border_1()
                    .border_color(border)
                    .children(rows.into_iter().enumerate().map(|(index, row)| {
                        div()
                            .when(index > 0, |view| view.border_t_1().border_color(border))
                            .child(row)
                    })),
            )
            .children(
                self.device_error
                    .as_ref()
                    .map(|error| small_text(error.clone(), red()).mt_2()),
            )
    }

    fn dialogs(
        &self,
        window: &Window,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if let Some(rename) = &self.rename {
            let pending = matches!(self.pending, Some((Pending::Rename, _)));
            let valid = valid_device_name(&rename.input.read(cx).buffer.text);
            return Some(plain_dialog(
                "Rename this device",
                "This name is synced to your other devices signed in to this account.",
                div()
                    .mt_4()
                    .child(field(&rename.input, colors).bg(colors.background))
                    .children(
                        rename
                            .error
                            .as_ref()
                            .map(|error| small_text(error.clone(), red()).mt_2()),
                    )
                    .into_any_element(),
                vec![
                    button(
                        "rename-cancel",
                        "Cancel",
                        Variant::Outline,
                        false,
                        None,
                        !pending,
                        colors,
                    )
                    .when(!pending, |view| {
                        view.on_click(cx.listener(|this, _, _, cx| {
                            this.rename = None;
                            cx.notify();
                        }))
                    })
                    .into_any_element(),
                    button(
                        "rename-save",
                        "Save",
                        Variant::Primary,
                        false,
                        pending.then_some("product/LoaderCircleIcon.svg"),
                        !pending && valid,
                        colors,
                    )
                    .when(!pending && valid, |view| {
                        view.on_click(cx.listener(|this, _, _, cx| this.save_rename(cx)))
                    })
                    .into_any_element(),
                ],
                window,
                colors,
            ));
        }
        if self.add_device {
            return Some(plain_dialog(
                "Add another device",
                "Install Anarlog and sign in with this account on the new device. Keep this device online and the new device will connect automatically.",
                div()
                    .text_xs()
                    .line_height(px(20.))
                    .text_color(colors.muted_foreground)
                    .child("Keep your recovery key saved somewhere safe. You can still use it if another synced device is unavailable.")
                    .into_any_element(),
                vec![button("add-device-done", "Done", Variant::Primary, false, None, true, colors)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.add_device = false;
                        cx.notify();
                    }))
                    .into_any_element()],
                window,
                colors,
            ));
        }
        let setup = self.setup.as_ref()?;
        Some(self.setup_dialog(setup, window, colors, cx))
    }

    /// `E2eeSetupDialog`.
    fn setup_dialog(
        &self,
        setup: &Setup,
        window: &Window,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pending = setup.pending;
        let pill = |id: &'static str,
                    label: &'static str,
                    variant: Variant,
                    icon: Option<&'static str>,
                    enabled: bool| {
            button(id, label, variant, true, icon, enabled, colors)
                .h(px(32.))
                .px_4()
                .w_full()
        };
        let body = if let Some(key) = &setup.key {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_center()
                        .text_xs()
                        .line_height(px(20.))
                        .text_color(colors.muted_foreground)
                        .child("Save this key in a password manager. New devices can usually be approved from another device; this key is your fallback and will not be shown again."),
                )
                .child(
                    div()
                        .max_h(px(112.))
                        .bg(colors.muted)
                        .p_3()
                        .font_family(monospace_font(cx))
                        .text_size(px(11.))
                        .line_height(px(20.))
                        .child(key.to_string()),
                )
                .child(
                    pill("copy-recovery-key", "Copy recovery key", Variant::Outline, Some("product/CopyIcon.svg"), true)
                        .on_click(cx.listener(|this, _, _, cx| this.copy_key(cx))),
                )
                .child(
                    pill("download-recovery-key", "Download recovery key (.txt)", Variant::Outline, Some(if pending { "product/LoaderCircleIcon.svg" } else { "product/Download01Icon.svg" }), !pending)
                        .when(!pending, |view| {
                            view.on_click(cx.listener(|this, _, _, cx| {
                                if let Some(key) = this.setup.as_ref().and_then(|setup| setup.key.clone()) {
                                    // Downloading must not turn into "I saved it"; keep the key shown.
                                    let work = this.calls.sync(SyncCommand::DownloadRecoveryKey(key));
                                    cx.spawn(async move |this, cx| {
                                        let result = work.await;
                                        let _ = this.update(cx, |this, cx| {
                                            if let (Some(setup), Err(error)) = (&mut this.setup, result) {
                                                setup.error = Some(error.to_string());
                                            }
                                            cx.notify();
                                        });
                                    })
                                    .detach();
                                }
                            }))
                        }),
                )
                .child(
                    div()
                        .text_center()
                        .text_size(px(11.))
                        .line_height(px(16.))
                        .text_color(colors.muted_foreground)
                        .child("Clipboard copies clear after 60 seconds when supported."),
                )
        } else if setup.mode == SetupMode::Import {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_xs()
                        .line_height(px(20.))
                        .text_color(colors.muted_foreground)
                        .child("Enter your saved recovery key when another approved device is unavailable."),
                )
                .child(
                    field(&setup.input, colors)
                        .bg(colors.background)
                        .font_family(monospace_font(cx))
                        .text_xs(),
                )
        } else {
            div()
                .grid()
                .gap_2()
                .child(
                    pill(
                        "create-recovery-key",
                        "Create a recovery key",
                        Variant::Primary,
                        pending.then_some("product/LoaderCircleIcon.svg"),
                        !pending,
                    )
                    .when(!pending, |view| {
                        view.on_click(cx.listener(|this, _, _, cx| {
                            this.setup_command(SyncCommand::CreateRecoveryKey, cx)
                        }))
                    }),
                )
                .child(
                    pill(
                        "use-existing-key",
                        "Use an existing key",
                        Variant::Outline,
                        None,
                        !pending,
                    )
                    .when(!pending, |view| {
                        view.on_click(cx.listener(|this, _, _, cx| {
                            if let Some(setup) = &mut this.setup {
                                setup.mode = SetupMode::Import;
                            }
                            cx.notify();
                        }))
                    }),
                )
                .child(
                    div().flex().justify_center().child(
                        div()
                            .id("setup-cancel")
                            .a11y_button("Cancel")
                            .h(px(24.))
                            .px_3()
                            .flex()
                            .items_center()
                            .text_size(px(11.))
                            .text_color(colors.muted_foreground)
                            .cursor_pointer()
                            .hover(|style| style.text_color(colors.foreground))
                            .child("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.close_setup(cx))),
                    ),
                )
        };
        let footer = (setup.key.is_some() || setup.mode == SetupMode::Import).then(|| {
            let importing = setup.key.is_none();
            let typed = !setup.input.read(cx).buffer.text.trim().is_empty();
            let confirm_enabled = !pending && (!importing || typed);
            div()
                .grid()
                .grid_cols(2)
                .gap_2()
                .pt_4()
                .child(
                    pill(
                        "setup-back",
                        if importing { "Back" } else { "Cancel" },
                        Variant::Ghost,
                        None,
                        !pending,
                    )
                    .bg(colors.accent.opacity(0.8))
                    .when(!pending, |view| {
                        view.on_click(cx.listener(move |this, _, _, cx| {
                            if importing {
                                if let Some(setup) = &mut this.setup {
                                    setup.mode = SetupMode::Choose;
                                    setup.error = None;
                                }
                                cx.notify();
                            } else {
                                this.close_setup(cx);
                            }
                        }))
                    }),
                )
                .child(
                    pill(
                        "setup-confirm",
                        if importing {
                            "Unlock sync"
                        } else {
                            "I saved it"
                        },
                        Variant::Primary,
                        pending.then_some("product/LoaderCircleIcon.svg"),
                        confirm_enabled,
                    )
                    .when(confirm_enabled, |view| {
                        view.on_click(cx.listener(|this, _, _, cx| this.unlock(cx)))
                    }),
                )
        });
        list::modal(
            window,
            div()
                .id("e2ee-setup")
                .w(px(320.))
                .overflow_hidden()
                .border_1()
                .border_color(colors.border.opacity(0.45))
                .bg(colors.card.opacity(0.95))
                .shadow(vec![gpui::BoxShadow {
                    color: gpui::hsla(0., 0., 0., 0.32),
                    offset: gpui::point(px(0.), px(24.)),
                    blur_radius: px(70.),
                    spread_radius: px(0.),
                    inset: false,
                }])
                .occlude()
                .child(
                    div()
                        .px_5()
                        .pt_7()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .text_center()
                        .child(
                            div()
                                .size(px(36.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(colors.accent)
                                .child(controls::icon("product/Key01Icon.svg", 16., colors.foreground)),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .line_height(px(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Protect cloud sync"),
                        )
                        .child(
                            div()
                                .max_w(px(260.))
                                .text_size(px(13.))
                                .line_height(px(17.7))
                                .child("Your recovery key encrypts synced notes before they leave this device. Anarlog cannot read or recover it."),
                        ),
                )
                .child(
                    div()
                        .px_4()
                        .pt_4()
                        .when(footer.is_none(), |view| view.pb_4())
                        .child(body)
                        .children(setup.error.as_ref().map(|error| {
                            div()
                                .mt_3()
                                .text_center()
                                .text_xs()
                                .text_color(red())
                                .child(error.clone())
                        })),
                )
                .children(footer.map(|footer| div().px_4().pb_4().child(footer)))
                .into_any_element(),
        )
    }
}

/// `SyncHealthSection`: shown only when something needs attention.
fn health(state: &SyncState, colors: &Theme) -> Option<Div> {
    let health = &state.health;
    if health.conflicted_notes == 0 && health.pending_update == 0 && health.too_large == 0 {
        return None;
    }
    let plural =
        |count: u32, one: &str, many: String| if count == 1 { one.to_owned() } else { many };
    let mut lines = Vec::new();
    if health.conflicted_notes > 0 {
        lines.push(plural(
            health.conflicted_notes,
            "1 note has a version from another device",
            format!(
                "{} notes have a version from another device",
                health.conflicted_notes
            ),
        ));
    }
    if health.pending_update > 0 {
        lines.push(plural(
            health.pending_update,
            "1 record is waiting for an app update",
            format!(
                "{} records are waiting for an app update",
                health.pending_update
            ),
        ));
    }
    if health.too_large > 0 {
        lines.push(plural(
            health.too_large,
            "1 record is too large to apply",
            format!("{} records are too large to apply", health.too_large),
        ));
    }
    let amber = amber();
    Some(
        div()
            .border_1()
            .border_color(amber.opacity(0.4))
            .bg(amber.opacity(0.05))
            .p_5()
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .size(px(36.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(amber.opacity(0.1))
                            .child(controls::icon("product/TriangleAlertIcon.svg", 16., amber)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                div()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .font_weight(MEDIUM)
                                    .child("Sync health"),
                            )
                            .child(
                                div()
                                    .mt_1()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .text_xs()
                                    .line_height(px(20.))
                                    .text_color(colors.muted_foreground)
                                    .children(lines),
                            ),
                    ),
            ),
    )
}

/// `Dialog` with `DialogContent className="max-w-sm"`: title, description, body and footer.
fn plain_dialog(
    title: &'static str,
    description: &'static str,
    body: AnyElement,
    footer: Vec<AnyElement>,
    window: &Window,
    colors: &Theme,
) -> AnyElement {
    list::modal(
        window,
        div()
            .id("sync-dialog")
            .w(px(384.))
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .border_1()
            .border_color(colors.border)
            .bg(colors.background)
            .shadow_lg()
            .occlude()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_lg()
                            .line_height(px(28.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(colors.muted_foreground)
                            .child(description),
                    ),
            )
            .child(body)
            .child(div().flex().justify_end().gap_2().children(footer))
            .into_any_element(),
    )
}

impl Render for SyncView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let page = div()
            .flex()
            .flex_col()
            .gap_8()
            .child(controls::page_title("Sync"));
        if self.state.is_none() {
            return page
                .child(
                    div()
                        .min_h(px(192.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(spinner(20., colors.muted_foreground)),
                )
                .into_any_element();
        }
        let status = self.status_section(&colors, cx);
        let devices = self.devices_section(&colors, cx);
        let dialog = self.dialogs(window, &colors, cx);
        page.child(status)
            .child(devices)
            .children(dialog)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_status_follows_shippings_precedence() {
        let now = Utc::now();
        let running = SyncStatus {
            configured: true,
            running: true,
            last_sync_at_ms: Some((now.timestamp_millis() - 5 * 60_000) as u64),
            ..SyncStatus::default()
        };
        let label = |preferred, block: Option<SyncBlock>, status: Option<&SyncStatus>, syncing| {
            status_view(preferred, block.as_ref(), status, syncing, now).label
        };
        // A paused switch wins over any block; a block wins over the engine's status.
        assert_eq!(
            label(false, Some(SyncBlock::DeviceLimit), Some(&running), false),
            "Sync paused"
        );
        assert_eq!(
            label(true, Some(SyncBlock::DeviceLimit), Some(&running), false),
            "Device limit reached"
        );
        assert_eq!(label(true, None, None, false), "Sync status unavailable");
        assert_eq!(
            status_view(true, None, Some(&running), false, now).description,
            "Last synced 5 minutes ago"
        );
        assert_eq!(label(true, None, Some(&running), true), "Syncing...");
        let failing = SyncStatus {
            consecutive_failures: 2,
            ..running.clone()
        };
        assert_eq!(
            label(true, None, Some(&failing), false),
            "Sync needs attention"
        );
        let stopped = SyncStatus {
            running: false,
            ..running
        };
        assert_eq!(label(true, None, Some(&stopped), false), "Connecting...");
    }
}
